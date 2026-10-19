use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::{AppData, SettingsEntry, SettingsKind};
use serde::{Deserialize, Serialize};

// Persistent app settings. The file and key names match what the Tauri store
// plugin wrote, so existing installs keep their folder and backup choices.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_eve_path: Option<String>,
    #[serde(default = "default_true")]
    pub auto_backup: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<ThemePreference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(default)]
    pub default_sources: DefaultSources,
    #[serde(default)]
    pub copy_groups: HashMap<String, bool>,
    #[serde(default)]
    pub character_columns: CharacterColumns,
    // None means the built-in default pattern
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drone_group_pattern: Option<String>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct CharacterColumns {
    pub corporation: bool,
    pub alliance: bool,
    pub creation_date: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DefaultSources {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_path: Option<String>,
}

impl DefaultSources {
    pub fn path(&self, kind: SettingsKind) -> Option<&str> {
        match kind {
            SettingsKind::Char => self.character_path.as_deref(),
            SettingsKind::User => self.account_path.as_deref(),
        }
    }

    pub fn set_path(&mut self, kind: SettingsKind, path: Option<String>) {
        match kind {
            SettingsKind::Char => self.character_path = path,
            SettingsKind::User => self.account_path = path,
        }
    }

    pub fn has_any(&self) -> bool {
        self.character_path.is_some() || self.account_path.is_some()
    }

    pub fn resolve<'a>(&self, data: &'a AppData, kind: SettingsKind) -> Option<&'a SettingsEntry> {
        data.find_entry(self.path(kind)?)
            .filter(|entry| entry.kind == kind)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    Light,
    Dark,
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            custom_eve_path: None,
            auto_backup: true,
            theme: None,
            locale: None,
            default_sources: DefaultSources::default(),
            copy_groups: HashMap::new(),
            character_columns: CharacterColumns::default(),
            drone_group_pattern: None,
            other: serde_json::Map::new(),
        }
    }
}

impl Config {
    pub fn copy_group_enabled(&self, group: &crate::copy::CopyGroup) -> bool {
        self.copy_groups
            .get(group.id)
            .copied()
            .unwrap_or(group.default_on)
    }

    // A missing or unreadable file falls back to defaults, which keep
    // auto-backup on: safer to over-back-up than to skip silently.
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, content).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tauri_store_format_and_keeps_unknown_keys() {
        let config: Config =
            serde_json::from_str(r#"{"customEvePath":"/eve","autoBackup":false,"extra":1}"#)
                .unwrap();
        assert_eq!(config.custom_eve_path.as_deref(), Some("/eve"));
        assert!(!config.auto_backup);

        let written = serde_json::to_value(&config).unwrap();
        assert_eq!(written["extra"], 1);
    }

    #[test]
    fn auto_backup_defaults_on() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert!(config.auto_backup);
        assert!(config.drone_group_pattern.is_none());
        assert!(!config.default_sources.has_any());
        for group in crate::copy::COPY_GROUPS {
            assert_eq!(config.copy_group_enabled(group), group.default_on);
        }
    }

    #[test]
    fn drone_pattern_round_trips_with_existing_fork_preferences() {
        let json = serde_json::json!({
            "defaultSources": {
                "characterPath": "/eve/core_char_1.dat",
                "accountPath": "/eve/core_user_2.dat"
            },
            "copyGroups": {"overview": false, "slots": true},
            "characterColumns": {"corporation": false, "alliance": true, "creationDate": false},
            "theme": "dark",
            "locale": "zh-CN",
            "extra": 42
        });
        let mut config: Config = serde_json::from_value(json.clone()).unwrap();
        config.drone_group_pattern = Some("{name}[ {damage}]".into());
        let written = serde_json::to_value(&config).unwrap();
        assert_eq!(written["droneGroupPattern"], "{name}[ {damage}]");
        for (key, value) in json.as_object().unwrap() {
            assert_eq!(&written[key], value, "{key}");
        }
        let restored: Config = serde_json::from_value(written).unwrap();
        assert_eq!(restored, config);
    }

    #[test]
    fn copy_preferences_round_trip_and_keep_defaults_for_missing_groups() {
        let mut config = Config::default();
        config.copy_groups.insert("windows".into(), false);
        config.copy_groups.insert("slots".into(), true);
        config.copy_groups.insert("future_group".into(), false);
        let saved = serde_json::to_string(&config).unwrap();
        let restored: Config = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored.copy_groups, config.copy_groups);
        for group in crate::copy::COPY_GROUPS {
            let expected = match group.id {
                "windows" => false,
                "slots" => true,
                _ => group.default_on,
            };
            assert_eq!(restored.copy_group_enabled(group), expected);
        }
    }

    #[test]
    fn character_columns_default_to_hidden_and_round_trip() {
        let mut config: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(config.character_columns, CharacterColumns::default());
        config.character_columns = CharacterColumns {
            corporation: true,
            alliance: true,
            creation_date: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let restored: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.character_columns, config.character_columns);
        let partial: Config =
            serde_json::from_str(r#"{"characterColumns":{"alliance":true}}"#).unwrap();
        assert!(partial.character_columns.alliance);
        assert!(!partial.character_columns.corporation);
        assert!(!partial.character_columns.creation_date);
    }

    #[test]
    fn saves_default_pair_and_can_clear_one_choice() {
        let path = std::env::temp_dir().join(format!(
            "eve-wrench-default-sources-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let mut config = Config::default();
        config
            .default_sources
            .set_path(SettingsKind::Char, Some("/eve/core_char_1.dat".into()));
        config
            .default_sources
            .set_path(SettingsKind::User, Some("/eve/core_user_2.dat".into()));
        config.save(&path).unwrap();
        assert_eq!(Config::load(&path), config);

        config.default_sources.set_path(SettingsKind::Char, None);
        config.save(&path).unwrap();
        let restored = Config::load(&path);
        assert_eq!(restored.default_sources.path(SettingsKind::Char), None);
        assert_eq!(
            restored.default_sources.path(SettingsKind::User),
            Some("/eve/core_user_2.dat")
        );
        assert!(restored.default_sources.has_any());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn resolves_exact_saved_files_and_never_substitutes_another_entry() {
        use crate::{ProfileData, Server, ServerData, ServerInfo};
        let entry = |kind, path: &str| SettingsEntry {
            path: path.into(),
            id: "123".into(),
            kind,
            server: Server::Tranquility,
            profile: "Default".into(),
            display_name: "Pilot".into(),
            character: None,
            alias: None,
            modified_time: 0,
        };
        let mut data = AppData {
            servers: vec![ServerData {
                info: ServerInfo {
                    id: Server::Tranquility,
                    brackets_always_show: false,
                    server_path: "/eve".into(),
                },
                profiles: vec![ProfileData {
                    name: "Default".into(),
                    path: "/eve".into(),
                    accounts: vec![entry(SettingsKind::User, "/eve/core_user_123.dat")],
                    characters: vec![
                        entry(SettingsKind::Char, "/eve/core_char_123.dat"),
                        entry(SettingsKind::Char, "/other/core_char_123.dat"),
                    ],
                }],
            }],
            backups: Vec::new(),
        };
        let mut defaults = DefaultSources {
            character_path: Some("/eve/core_char_123.dat".into()),
            account_path: Some("/eve/core_user_123.dat".into()),
        };
        for kind in [SettingsKind::Char, SettingsKind::User] {
            assert_eq!(
                defaults.resolve(&data, kind).unwrap().path,
                defaults.path(kind).unwrap()
            );
        }
        data.servers[0].profiles[0].characters.remove(0);
        assert!(defaults.resolve(&data, SettingsKind::Char).is_none());
        defaults.account_path = Some("/other/core_char_123.dat".into());
        assert!(defaults.resolve(&data, SettingsKind::User).is_none());
    }
}
