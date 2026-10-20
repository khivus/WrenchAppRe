//! Whole-profile copies are independent of parsed account/character entries.
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::SettingsKind;
use crate::scan::parse_core_filename;

#[derive(Debug)]
pub struct ProfileCopyResult {
    pub destination: PathBuf,
    pub backup: Option<PathBuf>,
    pub files: usize,
    pub accounts: usize,
    pub characters: usize,
}

pub fn profile_name(input: &str) -> Result<&str, String> {
    let name = input
        .trim()
        .strip_prefix("settings_")
        .unwrap_or(input.trim());
    if name.is_empty()
        || matches!(name, "." | "..")
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return Err(
            "Enter a profile name without path separators or reserved filename characters".into(),
        );
    }
    Ok(name)
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Junctions must not make a profile copy recurse outside its folder.
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

fn directory(path: &Path) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.is_dir() || is_link(&metadata) {
        return Err(format!("Expected a regular folder: {}", path.display()));
    }
    path.canonicalize().map_err(|e| e.to_string())
}

fn copy_tree(
    source: &Path,
    destination: &Path,
    result: &mut ProfileCopyResult,
) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if is_link(&metadata) {
            return Err(format!(
                "Profile contains a linked path: {}",
                path.display()
            ));
        }
        let target = destination.join(entry.file_name());
        if metadata.is_dir() {
            fs::create_dir(&target).map_err(|e| e.to_string())?;
            copy_tree(&path, &target, result)?;
        } else if metadata.is_file() {
            fs::copy(&path, &target).map_err(|e| format!("Cannot copy {}: {e}", path.display()))?;
            let _ = filetime::set_file_mtime(
                &target,
                filetime::FileTime::from_last_modification_time(&metadata),
            );
            result.files += 1;
            // Only top-level core files are live settings; backups may contain copies.
            if destination == result.destination {
                match parse_core_filename(&path).ok().map(|(kind, _)| kind) {
                    Some(SettingsKind::User) => result.accounts += 1,
                    Some(SettingsKind::Char) => result.characters += 1,
                    None => {}
                }
            }
        } else {
            return Err(format!("Unsupported profile file: {}", path.display()));
        }
    }
    Ok(())
}

fn remove_staging(path: &Path, server: &Path) -> Result<(), String> {
    let resolved = directory(path)?;
    // Check the resolved absolute scope before any recursive deletion.
    if resolved.parent() != Some(server)
        || !resolved
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(".eve-wrench-profile-"))
    {
        return Err("Refusing to remove a folder outside this copy operation".into());
    }
    fs::remove_dir_all(&resolved).map_err(|e| e.to_string())
}

/// Creates settings_<name>, staging a complete copy before replacing a profile.
/// With backups on, the entire old folder is retained.
pub fn copy_profile(
    source: &Path,
    destination_server: &Path,
    name: &str,
    backup: bool,
) -> Result<ProfileCopyResult, String> {
    let name = profile_name(name)?;
    let source = directory(source)?;
    if !source
        .file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with("settings_"))
    {
        return Err("Choose a settings profile folder as the source".into());
    }
    let server = directory(destination_server)?;
    let mut destination = server.join(format!("settings_{name}"));
    if fs::symlink_metadata(&destination).is_ok() {
        destination = directory(&destination)?;
    }
    if destination == source || destination.starts_with(&source) || source.starts_with(&destination)
    {
        return Err("Source and destination profiles must be separate folders".into());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let staging = server.join(format!(
        ".eve-wrench-profile-copy-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&staging).map_err(|e| format!("Cannot create destination profile: {e}"))?;
    let mut result = ProfileCopyResult {
        destination: staging.clone(),
        backup: None,
        files: 0,
        accounts: 0,
        characters: 0,
    };
    if let Err(error) = copy_tree(&source, &staging, &mut result) {
        let _ = remove_staging(&staging, &server);
        return Err(error);
    }
    result.destination = destination.clone();
    let install = (|| {
        let previous = if destination.exists() {
            let previous = if backup {
                let folder = server.join(".eve-wrench-profile-backups");
                if !folder.exists() {
                    fs::create_dir(&folder).map_err(|e| e.to_string())?;
                }
                let folder = directory(&folder)?;
                if folder.parent() != Some(server.as_path()) {
                    return Err("Invalid profile backup folder".into());
                }
                folder.join(format!("settings_{name}-{nonce}"))
            } else {
                server.join(format!(".eve-wrench-profile-old-{nonce}"))
            };
            fs::rename(&destination, &previous).map_err(|e| {
                format!("Cannot back up or replace the destination; close EVE and retry: {e}")
            })?;
            Some(previous)
        } else {
            None
        };
        if let Err(error) = fs::rename(&staging, &destination) {
            if let Some(previous) = &previous {
                fs::rename(previous, &destination).map_err(|restore| {
                    format!(
                        "Copy failed ({error}); restore the original from {}: {restore}",
                        previous.display()
                    )
                })?;
            }
            return Err(format!(
                "Profile copy failed; destination restored: {error}"
            ));
        }
        if let Some(previous) = previous {
            if backup {
                result.backup = Some(previous);
            } else {
                remove_staging(&previous, &server).map_err(|error| {
                    format!(
                        "Profile copied to {}, but the temporary old folder {} could not be removed: {error}",
                        destination.display(),
                        previous.display()
                    )
                })?;
            }
        }
        Ok(())
    })();
    if let Err(error) = install {
        if staging.exists() {
            let _ = remove_staging(&staging, &server);
        }
        return Err(error);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
