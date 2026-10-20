// Selective settings copy: copies curated groups of settings between files of
// the same kind instead of overwriting the whole file. A group is either whole
// top-level sections or a set of key prefixes inside the "ui" section.

use std::path::Path;

use serde_json::{Map, Value};

use crate::backups::auto_backup;
use crate::marshal::{encode_settings_json, load_settings_json, strip_type_prefix};
use crate::model::SettingsKind;

enum GroupRule {
    Sections(&'static [&'static str]),
    UiPrefixes(&'static [&'static str]),
    DroneAssignments,
}

pub struct CopyGroup {
    pub id: &'static str,
    kinds: &'static [SettingsKind],
    // Off for groups people legitimately customize per identity and would be
    // annoyed to have overwritten (HUD module slots, drones, search history).
    pub default_on: bool,
    rule: GroupRule,
}

impl CopyGroup {
    pub fn applies_to(&self, kind: SettingsKind) -> bool {
        self.kinds.contains(&kind)
    }
}

const USER: &[SettingsKind] = &[SettingsKind::User];
const CHAR: &[SettingsKind] = &[SettingsKind::Char];
const BOTH: &[SettingsKind] = &[SettingsKind::User, SettingsKind::Char];

pub const COPY_GROUPS: &[CopyGroup] = &[
    CopyGroup {
        id: "overview",
        kinds: USER,
        default_on: true,
        rule: GroupRule::Sections(&["overview", "defaultoverview"]),
    },
    CopyGroup {
        id: "probes",
        kinds: USER,
        default_on: true,
        rule: GroupRule::UiPrefixes(&["probescanning."]),
    },
    CopyGroup {
        id: "drone_groups",
        kinds: USER,
        default_on: true,
        // Favorite references travel with their group definitions. Membership
        // is optional, controlled by the separate drone_assignments choice.
        rule: GroupRule::UiPrefixes(&["droneBlah2", "drones_favoriteGroupID_"]),
    },
    CopyGroup {
        id: "drone_assignments",
        kinds: USER,
        default_on: false,
        rule: GroupRule::DroneAssignments,
    },
    CopyGroup {
        id: "suppress",
        kinds: USER,
        default_on: true,
        rule: GroupRule::Sections(&["suppress"]),
    },
    CopyGroup {
        id: "audio",
        kinds: USER,
        default_on: true,
        rule: GroupRule::Sections(&["audio"]),
    },
    CopyGroup {
        id: "camera_graphics",
        kinds: USER,
        default_on: true,
        rule: GroupRule::UiPrefixes(&[
            "camera",
            "spaceMouse",
            "offsetUIwithCamera",
            "invertCameraZoom",
            "advancedCamera",
            "missilesEnabled",
            "turretsEnabled",
            "trailsEnabled",
            "effectsEnabled",
            "explosionEffectsEnabled",
            "gpuParticlesEnabled",
            "droneModelsEnabled",
            "modelSkinsInSpaceEnabled",
            "UI_ASTEROID_",
        ]),
    },
    CopyGroup {
        id: "market",
        kinds: USER,
        default_on: true,
        rule: GroupRule::UiPrefixes(&[
            "market_",
            "minEdit_market",
            "maxEdit_market",
            "quickbar",
            "contracts_search_",
            "mycontracts_",
            "pricehistorytype",
        ]),
    },
    CopyGroup {
        id: "slots",
        kinds: USER,
        default_on: false,
        rule: GroupRule::UiPrefixes(&["slotOrder", "linkedWeapons_"]),
    },
    CopyGroup {
        id: "tabgroups",
        kinds: USER,
        default_on: true,
        rule: GroupRule::Sections(&["tabgroups"]),
    },
    CopyGroup {
        id: "windows",
        kinds: CHAR,
        default_on: true,
        rule: GroupRule::Sections(&["windows"]),
    },
    CopyGroup {
        id: "neocom",
        kinds: CHAR,
        default_on: true,
        rule: GroupRule::UiPrefixes(&["neocomButtonRawData"]),
    },
    CopyGroup {
        id: "chat",
        kinds: CHAR,
        default_on: true,
        rule: GroupRule::UiPrefixes(&["chatchannels"]),
    },
    CopyGroup {
        id: "infopanels",
        kinds: CHAR,
        default_on: true,
        rule: GroupRule::UiPrefixes(&["InfoPanelModes_"]),
    },
    CopyGroup {
        id: "dockpanels",
        kinds: CHAR,
        default_on: true,
        rule: GroupRule::Sections(&["dockPanels"]),
    },
    // Typed-text autocomplete and recent-search data; keys missing from a
    // source are simply skipped
    CopyGroup {
        id: "search_history",
        kinds: BOTH,
        default_on: false,
        rule: GroupRule::UiPrefixes(&[
            "editHistory",
            "contracts_history",
            "market_searchText",
            "assetsSearch",
        ]),
    },
];

fn find_group(id: &str) -> Option<&'static CopyGroup> {
    COPY_GROUPS.iter().find(|g| g.id == id)
}

fn key_matches(key: &str, prefixes: &[&str]) -> bool {
    let stripped = strip_type_prefix(key);
    prefixes.iter().any(|p| stripped.starts_with(p))
}

// Sections are usually "bytes:<name>"; fall back to constructing it
fn find_section_key(root: &Map<String, Value>, section: &str) -> String {
    root.keys()
        .find(|k| strip_type_prefix(k) == section)
        .cloned()
        .unwrap_or_else(|| format!("bytes:{}", section))
}

// The copy starts as a full clone of the source; excluded groups are then
// restored to exactly what the target had, so everything not explicitly
// opted out (including keys no group covers) behaves like a normal copy.
fn preserve_excluded_groups(
    result: &mut Value,
    target: &Value,
    excluded_groups: &[&str],
) -> Result<(), String> {
    let result_map = result
        .as_object_mut()
        .ok_or("Source file has an unexpected structure")?;
    let target_map = target
        .as_object()
        .ok_or("Target file has an unexpected structure")?;

    for group in excluded_groups.iter().filter_map(|id| find_group(id)) {
        match group.rule {
            GroupRule::DroneAssignments => {} // Merged after the ordinary groups.
            GroupRule::Sections(sections) => {
                for section in sections {
                    let key = find_section_key(result_map, section);
                    result_map.remove(&key);
                    let target_key = find_section_key(target_map, section);
                    if let Some(value) = target_map.get(&target_key) {
                        result_map.insert(target_key, value.clone());
                    }
                }
            }
            GroupRule::UiPrefixes(prefixes) => {
                let ui_key = find_section_key(result_map, "ui");
                let empty = Map::new();
                let target_ui = target_map
                    .get(&find_section_key(target_map, "ui"))
                    .and_then(|v| v.as_object())
                    .unwrap_or(&empty);

                let result_ui = result_map
                    .entry(ui_key)
                    .or_insert_with(|| Value::Object(Map::new()));
                let Some(result_ui) = result_ui.as_object_mut() else {
                    continue;
                };

                result_ui.retain(|k, _| !key_matches(k, prefixes));
                for (k, v) in target_ui {
                    if key_matches(k, prefixes) {
                        result_ui.insert(k.clone(), v.clone());
                    }
                }
            }
        }
    }

    // With group copying off, all target memberships are already preserved.
    // Merge last so checkbox/exclusion order cannot affect favorite references.
    if excluded_groups.contains(&"drone_assignments") && !excluded_groups.contains(&"drone_groups")
    {
        crate::drone_groups::preserve_assignments(result, target)?;
    }
    Ok(())
}

// Returns how many targets were written. Targets that fail to decode, encode
// or back up are skipped without touching them.
pub fn copy_settings_selective(
    source: &Path,
    targets: &[String],
    excluded_groups: &[&str],
    backup: bool,
) -> Result<usize, String> {
    copy_settings_impl(source, targets, excluded_groups, backup, false, None)
}

/// Copies only these sections, leaving all other target data untouched.
pub fn copy_settings_groups(
    source: &Path,
    targets: &[String],
    groups: &[&str],
    backup: bool,
) -> Result<usize, String> {
    copy_settings_impl(source, targets, groups, backup, true, None)
}

/// Syncs drone definitions using the editor's explicit Append/Replace choice.
pub fn sync_drone_groups(
    source: &Path,
    targets: &[String],
    mode: crate::drone_groups::DroneSaveMode,
    backup: bool,
) -> Result<usize, String> {
    copy_settings_impl(source, targets, &["drone_groups"], backup, true, Some(mode))
}

fn copy_settings_impl(
    source: &Path,
    targets: &[String],
    groups: &[&str],
    backup: bool,
    only_selected: bool,
    drone_mode: Option<crate::drone_groups::DroneSaveMode>,
) -> Result<usize, String> {
    let (source_json, _) = load_settings_json(source)?;
    let now = filetime::FileTime::now();
    let mut success_count = 0;

    for target in targets.iter().map(Path::new) {
        if target == source {
            continue;
        }
        let Ok((target_json, had_crc)) = load_settings_json(target) else {
            continue;
        };

        let mut result = if only_selected {
            target_json.clone()
        } else {
            source_json.clone()
        };
        let restored = if only_selected {
            &source_json
        } else {
            &target_json
        };
        let prepared = if let Some(mode) = drone_mode {
            crate::drone_groups::apply_group_definitions(&mut result, &source_json, mode)
        } else {
            preserve_excluded_groups(&mut result, restored, groups)
        };
        if prepared.is_err() {
            continue;
        }
        if only_selected
            && drone_mode.is_none()
            && groups.contains(&"drone_groups")
            && !groups.contains(&"drone_assignments")
            && crate::drone_groups::preserve_assignments(&mut result, &target_json).is_err()
        {
            continue;
        }
        let Ok(bytes) = encode_settings_json(&result, had_crc) else {
            continue;
        };

        if backup && auto_backup(target, "pre-selective-copy").is_err() {
            continue;
        }
        if std::fs::write(target, bytes).is_ok() {
            let _ = filetime::set_file_mtime(target, now);
            success_count += 1;
        }
    }

    Ok(success_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn drone_sync_modes_apply_to_every_account_and_back_up_originals() {
        use crate::drone_groups::DroneSaveMode;
        use std::fs;
        let dir = std::env::temp_dir().join(format!(
            "wrench-drone-sync-modes-{}-{}",
            std::process::id(),
            crate::marshal::filetime_now()
        ));
        fs::create_dir_all(&dir).unwrap();
        let source_path = dir.join("core_user_1.dat");
        let mut source = drone_settings("Shared", "long:1", "long:101", "626");
        source["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:New"] = drone_settings(
            "New", "long:3", "long:303", "999",
        )["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:New"]
            .clone();
        let source_bytes = encode_settings_json(&source, true).unwrap();
        fs::write(&source_path, &source_bytes).unwrap();
        for mode in [DroneSaveMode::Append, DroneSaveMode::Replace] {
            let profile = dir.join(format!("{mode:?}"));
            fs::create_dir_all(&profile).unwrap();
            let mut originals = Vec::new();
            for (index, id) in [28812632, 28812633].into_iter().enumerate() {
                let mut target = drone_settings("Old", "long:2", "long:202", "643");
                let groups = &mut target["bytes:ui"]["bytes:droneBlah2"]["tuple"][1];
                groups["utf8:Shared"] = drone_settings("Shared", "long:4", "long:404", "626")
                    ["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:Shared"].clone();
                groups["utf8:Empty old"] = groups["utf8:Old"].clone();
                groups["utf8:Empty old"]["bytes:label"] = json!("utf8:Empty old");
                groups["utf8:Empty old"]["bytes:id"] =
                    json!({"tuple": ["utf8:Empty old", "long:5"]});
                groups["utf8:Empty old"]["bytes:droneIDs"] = json!([]);
                target["bytes:overview"] = json!({"bytes:keep": id});
                let path = profile.join(format!("core_user_{id}.dat"));
                let bytes = encode_settings_json(&target, index == 0).unwrap();
                fs::write(&path, &bytes).unwrap();
                originals.push((path, target, bytes, index == 0));
            }
            let targets: Vec<_> = originals
                .iter()
                .map(|(path, ..)| path.to_string_lossy().into_owned())
                .collect();
            assert_eq!(
                sync_drone_groups(&source_path, &targets, mode, true).unwrap(),
                2
            );
            for (path, target, original, expected_crc) in originals {
                let (copied, crc) = load_settings_json(&path).unwrap();
                assert_eq!(crc, expected_crc);
                assert_eq!(copied["bytes:overview"], target["bytes:overview"]);
                for key in ["bytes:unmapped", "bytes:droneModelsEnabled"] {
                    assert_eq!(copied["bytes:ui"][key], target["bytes:ui"][key]);
                }
                let groups = copied["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]
                    .as_object()
                    .unwrap();
                assert_eq!(groups["utf8:New"]["bytes:droneIDs"], json!([]));
                match mode {
                    DroneSaveMode::Append => {
                        assert_eq!(groups.len(), 4);
                        for (key, old_group) in target["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]
                            .as_object()
                            .unwrap()
                        {
                            assert_eq!(&groups[key], old_group);
                        }
                        assert_eq!(
                            copied["bytes:ui"]["bytes:drones_favoriteGroupID_643"],
                            target["bytes:ui"]["bytes:drones_favoriteGroupID_643"]
                        );
                    }
                    DroneSaveMode::Replace => {
                        assert_eq!(groups.len(), 2);
                        assert!(!groups.contains_key("utf8:Old"));
                        assert!(!groups.contains_key("utf8:Empty old"));
                        assert_eq!(groups["utf8:Shared"]["bytes:droneIDs"], json!([]));
                        assert!(
                            copied["bytes:ui"]
                                .get("bytes:drones_favoriteGroupID_643")
                                .is_none()
                        );
                    }
                }
                assert_eq!(
                    copied["bytes:ui"]["bytes:drones_favoriteGroupID_626"]["tuple"][1],
                    groups["utf8:Shared"]["bytes:id"]
                );
                let (_, id) = crate::scan::parse_core_filename(&path).unwrap();
                let backup = fs::read_dir(profile.join("backups"))
                    .unwrap()
                    .map(|entry| entry.unwrap().path())
                    .find(|path| {
                        path.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .contains(&format!("_user_{id}_"))
                    })
                    .unwrap();
                assert_eq!(fs::read(backup).unwrap(), original);
            }
        }
        assert_eq!(fs::read(source_path).unwrap(), source_bytes);
        let resolved = dir.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        fs::remove_dir_all(resolved).unwrap();
    }

    #[test]
    fn editor_sync_copies_only_its_section_and_keeps_target_drone_items() {
        use std::fs;
        let dir = std::env::temp_dir().join(format!(
            "wrench-section-sync-{}-{}",
            std::process::id(),
            crate::marshal::filetime_now()
        ));
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("core_user_1.dat");
        let dst = dir.join("core_user_2.dat");
        let mut source = drone_settings("Shared", "long:1", "long:101", "626");
        let mut target = drone_settings("Shared", "long:2", "long:202", "643");
        source["bytes:overview"] = json!({"bytes:other": "utf8:source"});
        target["bytes:overview"] = json!({"bytes:other": "utf8:target"});
        source["bytes:ui"]["bytes:probescanning.customFormations"] =
            json!({"tuple": ["long:1", {"int:0": {"tuple": ["utf8:source", []]}}]});
        target["bytes:ui"]["bytes:probescanning.customFormations"] =
            json!({"tuple": ["long:2", {}]});
        fs::write(&src, encode_settings_json(&source, false).unwrap()).unwrap();
        let original = encode_settings_json(&target, true).unwrap();
        fs::write(&dst, &original).unwrap();
        let targets = [dst.to_string_lossy().into_owned()];
        assert_eq!(
            copy_settings_groups(&src, &targets, &["drone_groups"], true).unwrap(),
            1
        );
        let (copied, crc) = load_settings_json(&dst).unwrap();
        assert!(crc);
        for key in ["bytes:unmapped", "bytes:probescanning.customFormations"] {
            assert_eq!(copied["bytes:ui"][key], target["bytes:ui"][key]);
        }
        assert_eq!(copied["bytes:overview"], target["bytes:overview"]);
        assert_eq!(
            copied["bytes:ui"]["bytes:droneBlah2"]["tuple"][1],
            target["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]
        );
        assert_eq!(
            copied["bytes:ui"]["bytes:drones_favoriteGroupID_626"]["tuple"][1],
            target["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:Shared"]["bytes:id"]
        );
        let backup = fs::read_dir(dir.join("backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(fs::read(backup).unwrap(), original);
        fs::write(&dst, &original).unwrap();
        assert_eq!(
            copy_settings_groups(&src, &targets, &["probes"], false).unwrap(),
            1
        );
        let (copied, _) = load_settings_json(&dst).unwrap();
        let mut expected = target;
        expected["bytes:ui"]["bytes:probescanning.customFormations"] =
            source["bytes:ui"]["bytes:probescanning.customFormations"].clone();
        assert_eq!(copied, expected);
        let resolved = dir.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        fs::remove_dir_all(resolved).unwrap();
    }

    #[test]
    fn selective_copy_preserves_only_excluded_groups() {
        let source = json!({
            "bytes:overview": {
                "bytes:shipLabels": {"tuple": ["long:1", "utf8:source-labels"]},
            },
            "bytes:ui": {
                "bytes:probescanning.customFormations": {"tuple": ["long:1", {"int:0": {"tuple": ["utf8:src", []]}}]},
                "bytes:slotOrder": {"tuple": ["long:1", "utf8:source-slots"]},
                "bytes:someUnmappedKey": {"tuple": ["long:1", "utf8:source-unmapped"]},
            },
        });
        let target = json!({
            "bytes:overview": {
                "bytes:shipLabels": {"tuple": ["long:2", "utf8:target-labels"]},
            },
            "bytes:ui": {
                "bytes:probescanning.customFormations": {"tuple": ["long:2", {}]},
                "bytes:slotOrder": {"tuple": ["long:2", "utf8:target-slots"]},
                "bytes:linkedWeapons_groupsDict": {"tuple": ["long:2", "utf8:target-links"]},
                "bytes:someUnmappedKey": {"tuple": ["long:2", "utf8:target-unmapped"]},
            },
        });

        let mut result = source.clone();
        preserve_excluded_groups(&mut result, &target, &["slots"]).unwrap();

        let ui = result.get("bytes:ui").unwrap();
        // Not excluded: mapped and unmapped keys alike come from source
        assert_eq!(
            result["bytes:overview"]["bytes:shipLabels"]["tuple"][1],
            json!("utf8:source-labels")
        );
        assert!(
            ui["bytes:probescanning.customFormations"]["tuple"][1]
                .as_object()
                .unwrap()
                .contains_key("int:0")
        );
        assert_eq!(
            ui["bytes:someUnmappedKey"]["tuple"][1],
            json!("utf8:source-unmapped")
        );
        // Excluded: slot layout keeps exactly what the target had, including
        // keys the source does not have at all
        assert_eq!(
            ui["bytes:slotOrder"]["tuple"][1],
            json!("utf8:target-slots")
        );
        assert_eq!(
            ui["bytes:linkedWeapons_groupsDict"]["tuple"][1],
            json!("utf8:target-links")
        );
    }

    #[test]
    fn group_ids_are_unique() {
        let mut ids: Vec<_> = COPY_GROUPS.iter().map(|g| g.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), COPY_GROUPS.len());
    }

    fn drone_settings(label: &str, stamp: &str, drone_id: &str, ship: &str) -> Value {
        json!({
            "bytes:ui": {
                "bytes:droneBlah2": {"tuple": [stamp, {
                    format!("utf8:{label}"): {
                        "bytes:label": format!("utf8:{label}"),
                        "bytes:id": {"tuple": [format!("utf8:{label}"), stamp]},
                        "bytes:droneGroupID": 100,
                        "bytes:droneIDs": [drone_id],
                    }
                }]},
                format!("bytes:drones_favoriteGroupID_{ship}"):
                    {"tuple": [stamp, {"tuple": [format!("utf8:{label}"), stamp]}]},
                "bytes:droneModelsEnabled": {"tuple": [stamp, true]},
                "bytes:unmapped": {"tuple": [stamp, format!("utf8:{label}")]},
            }
        })
    }

    #[test]
    fn excluded_drone_groups_preserve_membership_and_all_ship_favorites_together() {
        let source = drone_settings("Source", "long:1", "long:101", "626");
        let target = drone_settings("Target", "long:2", "long:202", "643");
        let mut result = source.clone();
        preserve_excluded_groups(&mut result, &target, &["drone_groups"]).unwrap();
        let ui = &result["bytes:ui"];
        assert_eq!(
            ui["bytes:droneBlah2"],
            target["bytes:ui"]["bytes:droneBlah2"]
        );
        assert_eq!(
            ui["bytes:drones_favoriteGroupID_643"],
            target["bytes:ui"]["bytes:drones_favoriteGroupID_643"]
        );
        assert!(ui.get("bytes:drones_favoriteGroupID_626").is_none());
        // The drone group option must not swallow graphics or unrelated data.
        assert_eq!(
            ui["bytes:droneModelsEnabled"],
            source["bytes:ui"]["bytes:droneModelsEnabled"]
        );
        assert_eq!(ui["bytes:unmapped"], source["bytes:ui"]["bytes:unmapped"]);

        let mut result = source.clone();
        preserve_excluded_groups(&mut result, &json!({}), &["drone_groups"]).unwrap();
        assert!(result["bytes:ui"].get("bytes:droneBlah2").is_none());
        assert!(
            result["bytes:ui"]
                .get("bytes:drones_favoriteGroupID_626")
                .is_none()
        );
        assert_eq!(
            result["bytes:ui"]["bytes:unmapped"],
            source["bytes:ui"]["bytes:unmapped"]
        );

        let mut result = json!({});
        preserve_excluded_groups(&mut result, &target, &["drone_groups"]).unwrap();
        assert_eq!(
            result["bytes:ui"]["bytes:droneBlah2"],
            target["bytes:ui"]["bytes:droneBlah2"]
        );
        assert_eq!(
            result["bytes:ui"]["bytes:drones_favoriteGroupID_643"],
            target["bytes:ui"]["bytes:drones_favoriteGroupID_643"]
        );
        let mut result = source.clone();
        preserve_excluded_groups(&mut result, &target, &["drone_assignments", "drone_groups"])
            .unwrap();
        assert_eq!(
            result["bytes:ui"]["bytes:droneBlah2"],
            target["bytes:ui"]["bytes:droneBlah2"]
        );
    }

    #[test]
    fn copying_definitions_preserves_target_memberships_and_remaps_favorites() {
        let mut source = drone_settings("Shared", "long:1", "long:101", "626");
        let mut target = drone_settings("Shared", "long:2", "long:202", "643");
        let new_group = drone_settings("New", "long:3", "long:303", "999");
        let occupied = drone_settings("Target only", "long:4", "long:404", "998");
        let mut empty = drone_settings("Empty", "long:5", "long:505", "997");
        empty["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:Empty"]["bytes:droneIDs"] =
            json!([]);
        source["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:New"] =
            new_group["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:New"].clone();
        source["bytes:ui"]["bytes:drones_favoriteGroupID_999"] =
            new_group["bytes:ui"]["bytes:drones_favoriteGroupID_999"].clone();
        for (label, settings) in [("Target only", &occupied), ("Empty", &empty)] {
            target["bytes:ui"]["bytes:droneBlah2"]["tuple"][1][format!("utf8:{label}")] = settings
                ["bytes:ui"]["bytes:droneBlah2"]["tuple"][1][format!("utf8:{label}")]
            .clone();
        }
        let mut result = source.clone();
        preserve_excluded_groups(&mut result, &target, &["drone_assignments"]).unwrap();
        let groups = &result["bytes:ui"]["bytes:droneBlah2"]["tuple"][1];
        assert_eq!(
            groups["utf8:Shared"],
            target["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:Shared"]
        );
        assert_eq!(groups["utf8:New"]["bytes:droneIDs"], json!([]));
        assert_eq!(
            groups["utf8:Target only"],
            occupied["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:Target only"]
        );
        assert!(groups.get("utf8:Empty").is_none());
        assert_eq!(
            result["bytes:ui"]["bytes:drones_favoriteGroupID_626"]["tuple"][1],
            groups["utf8:Shared"]["bytes:id"]
        );
        assert_eq!(
            result["bytes:ui"]["bytes:drones_favoriteGroupID_999"],
            source["bytes:ui"]["bytes:drones_favoriteGroupID_999"]
        );
        assert!(
            result["bytes:ui"]
                .get("bytes:drones_favoriteGroupID_643")
                .is_none()
        );
        assert_eq!(
            result["bytes:ui"]["bytes:unmapped"],
            source["bytes:ui"]["bytes:unmapped"]
        );
    }

    #[test]
    fn preserving_assignments_handles_absent_groups_and_category_collisions() {
        let source = drone_settings("Shared", "long:1", "long:101", "626");
        let mut target = drone_settings("Shared", "long:2", "long:202", "643");
        target["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:Shared"]["bytes:droneGroupID"] =
            json!(101);
        let mut result = source.clone();
        preserve_excluded_groups(&mut result, &target, &["drone_assignments"]).unwrap();
        assert_eq!(
            result["bytes:ui"]["bytes:droneBlah2"]["tuple"][1],
            target["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]
        );
        assert_eq!(
            result["bytes:ui"]["bytes:drones_favoriteGroupID_626"]["tuple"][1],
            target["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:Shared"]["bytes:id"]
        );

        let mut result = source.clone();
        preserve_excluded_groups(&mut result, &json!({}), &["drone_assignments"]).unwrap();
        assert_eq!(
            result["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:Shared"]["bytes:droneIDs"],
            json!([])
        );
        let mut result = json!({});
        preserve_excluded_groups(&mut result, &target, &["drone_assignments"]).unwrap();
        assert_eq!(
            result["bytes:ui"]["bytes:droneBlah2"]["tuple"][1],
            target["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]
        );
        let mut result = json!({});
        preserve_excluded_groups(&mut result, &json!({}), &["drone_assignments"]).unwrap();
        assert_eq!(result, json!({}));

        target["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:Shared"]["bytes:droneIDs"] =
            json!("utf8:invalid");
        let mut result = source.clone();
        assert!(preserve_excluded_groups(&mut result, &target, &["drone_assignments"]).is_err());
    }

    #[test]
    fn drone_copy_choices_round_trip_files_keep_crc_and_back_up_originals() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let dir = std::env::temp_dir().join(format!(
            "eve-wrench-drone-copy-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("core_user_1.dat");
        let target = dir.join("core_user_2.dat");
        let source_json = drone_settings("Source", "long:1", "long:101", "626");
        let target_json = drone_settings("Target", "long:2", "long:202", "643");
        let original = encode_settings_json(&target_json, true).unwrap();
        fs::write(&source, encode_settings_json(&source_json, false).unwrap()).unwrap();
        fs::write(&target, &original).unwrap();
        let targets = [target.to_string_lossy().into_owned()];
        let mut config = crate::config::Config::default();
        config.copy_groups.insert("drone_groups".into(), false);
        let excluded: Vec<_> = COPY_GROUPS
            .iter()
            .filter(|group| {
                group.applies_to(SettingsKind::User) && !config.copy_group_enabled(group)
            })
            .map(|group| group.id)
            .collect();
        assert_eq!(
            copy_settings_selective(&source, &targets, &excluded, true).unwrap(),
            1
        );
        let (copied, crc) = load_settings_json(&target).unwrap();
        assert!(crc);
        assert_eq!(
            copied["bytes:ui"]["bytes:droneBlah2"],
            target_json["bytes:ui"]["bytes:droneBlah2"]
        );
        assert_eq!(
            copied["bytes:ui"]["bytes:drones_favoriteGroupID_643"],
            target_json["bytes:ui"]["bytes:drones_favoriteGroupID_643"]
        );
        assert!(
            copied["bytes:ui"]
                .get("bytes:drones_favoriteGroupID_626")
                .is_none()
        );
        let backups: Vec<_> = fs::read_dir(dir.join("backups")).unwrap().collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            fs::read(backups[0].as_ref().unwrap().path()).unwrap(),
            original
        );

        // Copy the definitions and favorites while retaining the target's items.
        assert_eq!(
            copy_settings_selective(&source, &targets, &["drone_assignments"], false).unwrap(),
            1
        );
        let (copied, crc) = load_settings_json(&target).unwrap();
        assert!(crc);
        let groups = &copied["bytes:ui"]["bytes:droneBlah2"]["tuple"][1];
        assert_eq!(groups["utf8:Source"]["bytes:droneIDs"], json!([]));
        assert_eq!(
            groups["utf8:Target"],
            target_json["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:Target"]
        );
        assert_eq!(
            copied["bytes:ui"]["bytes:drones_favoriteGroupID_626"],
            source_json["bytes:ui"]["bytes:drones_favoriteGroupID_626"]
        );

        assert_eq!(
            copy_settings_selective(&source, &targets, &[], false).unwrap(),
            1
        );
        let (copied, crc) = load_settings_json(&target).unwrap();
        assert!(crc);
        assert_eq!(copied, source_json);
        // Invalid membership data must skip the target without rewriting it.
        let mut invalid = target_json;
        invalid["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:Target"]["bytes:droneIDs"] =
            json!("utf8:invalid");
        let invalid_bytes = encode_settings_json(&invalid, true).unwrap();
        fs::write(&target, &invalid_bytes).unwrap();
        assert_eq!(
            copy_settings_selective(&source, &targets, &["drone_assignments"], false).unwrap(),
            0
        );
        assert_eq!(fs::read(&target).unwrap(), invalid_bytes);
        let resolved = dir.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        fs::remove_dir_all(&dir).unwrap();
    }
}
