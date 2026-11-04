//! Narrow settings edits for the layout display editor. Unknown values and
//! marshal key types survive; every save checks the complete original file,
//! writes a sibling staging file and makes a verified, unique backup.

use crate::{
    SettingsKind,
    backups::auto_backup,
    marshal::{encode_settings_json, filetime_now, strip_type_prefix},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub mod overview;
pub mod scene;

#[derive(Clone, Debug)]
pub struct SettingsDocument {
    pub path: PathBuf,
    pub kind: SettingsKind,
    original: Vec<u8>,
    original_root: Value,
    root: Value,
    checksum: bool,
    changed: BTreeSet<(String, String)>,
}

pub fn text(value: &Value) -> Option<&str> {
    value.as_str().map(strip_type_prefix)
}
pub fn integer(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str()?.strip_prefix("long:")?.parse().ok())
}
pub fn boolean(value: &Value) -> Option<bool> {
    value.as_bool().or_else(|| {
        integer(value)
            .filter(|v| *v == 0 || *v == 1)
            .map(|v| v != 0)
    })
}

impl SettingsDocument {
    pub fn load(path: &Path) -> Result<Self, String> {
        let kind = crate::scan::parse_core_filename(path)?.0;
        let original = fs::read(path).map_err(|e| e.to_string())?;
        let decoded = blue_marshal::decode(&original).map_err(|e| e.to_string())?;
        let root = blue_marshal::to_json(&decoded.value);
        if !root.is_object() {
            return Err("Unsupported settings root".into());
        }
        Ok(Self {
            path: path.into(),
            kind,
            original,
            original_root: root.clone(),
            root,
            checksum: decoded.had_crc,
            changed: BTreeSet::new(),
        })
    }

    pub fn value(&self, section: &str, key: &str) -> Option<&Value> {
        Self::get(&self.root, section, key)
    }

    fn get<'a>(root: &'a Value, section: &str, key: &str) -> Option<&'a Value> {
        root.get(format!("bytes:{section}"))?
            .get(format!("bytes:{key}"))?
            .get("tuple")?
            .get(1)
    }

    pub fn is_dirty(&self) -> bool {
        !self.changed.is_empty()
    }

    pub fn reset(&mut self) {
        self.root = self.original_root.clone();
        self.changed.clear();
    }

    pub fn set(&mut self, section: &str, key: &str, value: Value) -> Result<(), String> {
        validate(self.kind, section, key, &value)?;
        let section_key = format!("bytes:{section}");
        let key_name = format!("bytes:{key}");
        let root = self
            .root
            .as_object_mut()
            .ok_or("Unsupported settings root")?;
        let map = root
            .entry(section_key)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or("Unsupported settings section")?;
        if let Some(record) = map.get_mut(&key_name) {
            let tuple = record
                .get_mut("tuple")
                .and_then(Value::as_array_mut)
                .filter(|a| a.len() == 2)
                .ok_or("Unsupported settings record; reload or edit this setting in EVE")?;
            tuple[1] = value;
        } else {
            map.insert(key_name, json!({"tuple": ["long:0", value]}));
        }
        let pair = (section.to_string(), key.to_string());
        if self.value(section, key) == Self::get(&self.original_root, section, key) {
            self.changed.remove(&pair);
        } else {
            self.changed.insert(pair);
        }
        Ok(())
    }

    pub fn save(mut self) -> Result<(Self, Option<PathBuf>), String> {
        if !self.is_dirty() {
            return Ok((self, None));
        }
        let metadata = fs::symlink_metadata(&self.path).map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.permissions().readonly() {
            return Err("A writable regular settings file is required".into());
        }
        if crate::scan::parse_core_filename(&self.path)?.0 != self.kind {
            return Err("Settings kind changed".into());
        }
        let check = || -> Result<(), String> {
            if fs::read(&self.path).map_err(|e| e.to_string())? != self.original {
                return Err("Settings changed on disk. Reload before saving.".into());
            }
            Ok(())
        };
        check()?;
        let mut output = self.original_root.clone();
        for (section, key) in &self.changed {
            let value = self.value(section, key).ok_or("Missing changed setting")?;
            validate(self.kind, section, key, value)?;
            let section_key = format!("bytes:{section}");
            let map = output
                .as_object_mut()
                .unwrap()
                .entry(section_key)
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or("Unsupported section")?;
            map.insert(
                format!("bytes:{key}"),
                json!({"tuple": [format!("long:{}", filetime_now()), value]}),
            );
        }
        let bytes = encode_settings_json(&output, self.checksum)?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let staged_path = self.path.with_file_name(format!(
            ".wrench-display-{}-{nonce}.tmp",
            std::process::id()
        ));
        let mut staged = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&staged_path)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            staged.write_all(&bytes).map_err(|e| e.to_string())?;
            staged.sync_all().map_err(|e| e.to_string())?;
            fs::set_permissions(&staged_path, metadata.permissions()).map_err(|e| e.to_string())?;
            drop(staged);
            check()?;
            let backup = auto_backup(&self.path, &format!("pre-display-{nonce}"))?;
            if fs::read(&backup).map_err(|e| e.to_string())? != self.original {
                return Err("Settings changed during backup; original kept".into());
            }
            check()?;
            fs::rename(&staged_path, &self.path)
                .map_err(|e| format!("Could not replace settings; original kept: {e}"))?;
            Ok(backup)
        })();
        let backup = match result {
            Ok(path) => path,
            Err(error) => {
                let _ = fs::remove_file(&staged_path);
                return Err(error);
            }
        };
        self.original = bytes;
        self.original_root = output.clone();
        self.root = output;
        self.changed.clear();
        Ok((self, Some(backup)))
    }
}

fn validate(kind: SettingsKind, section: &str, key: &str, value: &Value) -> Result<(), String> {
    let valid = match (kind, section, key) {
        (SettingsKind::Char, "ui", "neocomWidth") => {
            integer(value).is_some_and(|v| (32..=72).contains(&v))
        }
        (SettingsKind::Char, "ui", "neocomAlign") => {
            integer(value).is_some_and(|v| v == 5 || v == 6)
        }
        (SettingsKind::Char, "ui", "fightersDetachedPosition") => value
            .get("tuple")
            .and_then(Value::as_array)
            .is_some_and(|a| {
                a.len() == 2
                    && a.iter()
                        .all(|v| integer(v).is_some_and(|n| (-32768..=32768).contains(&n)))
            }),
        (SettingsKind::User, "ui", "targetOrigin") => value
            .get("tuple")
            .and_then(Value::as_array)
            .is_some_and(|a| {
                a.len() == 2
                    && a.iter().all(|v| {
                        v.as_f64()
                            .is_some_and(|n| n.is_finite() && (0.0..=1.0).contains(&n))
                    })
            }),
        (
            SettingsKind::User,
            "ui",
            "alignHorizontally" | "targetOriginLocked" | "targetPositionLocked" | "detachFighterUI",
        ) => boolean(value).is_some(),
        (SettingsKind::Char, "ui", "neocomButtonRawData") => value.as_array().is_some_and(|a| {
            a.len() <= 100
                && a.iter().all(|v| {
                    v.get("instance")
                        .and_then(|v| v.get("state"))
                        .is_some_and(Value::is_object)
                })
        }),
        (SettingsKind::Char, "ui", "SortHeadersSizes") => overview::validate_widths(value),
        (SettingsKind::User, "overview", key) => overview::validate_setting(key, value),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "Unsupported or invalid {section}.{key}; no settings were written"
        ))
    }
}

#[cfg(test)]
mod tests;
