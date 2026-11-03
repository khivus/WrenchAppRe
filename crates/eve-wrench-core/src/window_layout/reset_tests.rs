use super::*;
use std::path::PathBuf;

struct Fixture {
    dir: PathBuf,
    path: PathBuf,
    root: Value,
    original: Vec<u8>,
    expected: Vec<WindowState>,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "wrench-geometry-reset-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("core_char_12345.dat");
        let root = json!({
            "bytes:windows": {
                "bytes:__version__": {"tuple": ["long:1", 1]},
                "bytes:windowSizesAndPositions_1": {"tuple": ["long:1", {
                    "bytes:overview": {"tuple": [100, 200, 300, 400, 1920, 1080]},
                    "bytes:548": {"tuple": [500, 600, 300, 200, 1920, 1080]},
                    "utf8:chatchannel_fleet": {"tuple": [500, 600, 300, 200, 1920, 1080]},
                    "utf8:future_window": {"bytes:future": "utf8:untouched"}
                }]},
                "bytes:openWindows": {"tuple": ["long:2", {"bytes:overview": true, "bytes:548": true, "utf8:chatchannel_fleet": true, "bytes:future_flag": "long:7"}]},
                "bytes:lockedWindows": {"tuple": ["long:3", {"bytes:overview": true}]},
                "bytes:minimizedWindows": {"tuple": ["long:4", {"bytes:548": true}]},
                "bytes:compactWindows": {"tuple": ["long:5", {"bytes:548": false}]},
                "bytes:isLightBackgroundWindows": {"tuple": ["long:6", {"bytes:overview": true}]},
                "bytes:stacksWindows": {"tuple": ["long:7", {"utf8:chatchannel_fleet": "bytes:548"}]},
                "bytes:future_map": {"tuple": ["long:8", {"bytes:keep": "utf8:data"}]}
            },
            "bytes:ui": {"bytes:overviewPresets": {"tuple": ["long:1", "utf8:keep"]}, "bytes:neocomWidth": {"tuple": ["long:1", 55]}},
            "bytes:other": {"tuple": ["long:1", "utf8:keep"]}
        });
        let original = encode_settings_json(&root, true).unwrap();
        fs::write(&path, &original).unwrap();
        let expected = read_window_layout(&path).unwrap().windows;
        Self {
            dir,
            path,
            root,
            original,
            expected,
        }
    }

    fn draft(&self, keys: &[&str]) -> (Vec<WindowState>, HashSet<String>) {
        let reset_keys: HashSet<_> = keys.iter().map(|key| key.to_string()).collect();
        let mut draft = self.expected.clone();
        for w in &mut draft {
            if reset_keys.contains(&w.key) {
                w.rect = None;
            }
        }
        (draft, reset_keys)
    }

    fn reset(&self, keys: &[&str]) -> Result<(), String> {
        let (draft, keys) = self.draft(keys);
        write_window_layout_with_geometry_resets(&self.path, &draft, &keys, &self.expected, false)
    }

    fn backups(&self) -> Vec<PathBuf> {
        let path = self.dir.join("backups");
        if !path.exists() {
            return Vec::new();
        }
        fs::read_dir(path)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect()
    }

    fn assert_only_rectangles_removed(&self, keys: &[&str]) {
        let decoded = blue_marshal::decode(&fs::read(&self.path).unwrap()).unwrap();
        assert!(decoded.had_crc);
        let mut actual = blue_marshal::to_json(&decoded.value);
        let mut wanted = self.root.clone();
        for key in keys {
            wanted[WINDOWS_KEY][RECTS_KEY]["tuple"][1]
                .as_object_mut()
                .unwrap()
                .remove(*key);
        }
        actual[WINDOWS_KEY][RECTS_KEY]["tuple"][0] =
            wanted[WINDOWS_KEY][RECTS_KEY]["tuple"][0].clone();
        assert_eq!(
            actual, wanted,
            "reset must preserve all flags, styles, stack links and unknown data"
        );
        let backups = self.backups();
        assert_eq!(
            backups.len(),
            1,
            "a reset must back up even when ordinary backups are off"
        );
        assert_eq!(fs::read(&backups[0]).unwrap(), self.original);
        assert!(crate::scan::parse_backup_file(&backups[0]).is_some());
        assert!(
            !fs::read_dir(&self.dir).unwrap().any(|e| e
                .unwrap()
                .path()
                .extension()
                .is_some_and(|s| s == "tmp"))
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let dir = self.dir.canonicalize().unwrap();
        assert!(dir.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(
            dir.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("wrench-geometry-reset-")
        );
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn reset_removes_only_selected_geometry_preserves_crc_and_always_backs_up() {
    let f = Fixture::new();
    f.reset(&["bytes:overview"]).unwrap();
    f.assert_only_rectangles_removed(&["bytes:overview"]);
}

#[test]
fn reset_keeps_settings_without_a_checksum_in_the_same_format() {
    let mut f = Fixture::new();
    f.original = encode_settings_json(&f.root, false).unwrap();
    fs::write(&f.path, &f.original).unwrap();
    f.reset(&["bytes:overview"]).unwrap();
    let decoded = blue_marshal::decode(&fs::read(&f.path).unwrap()).unwrap();
    assert!(!decoded.had_crc);
    assert_eq!(fs::read(&f.backups()[0]).unwrap(), f.original);
    assert_eq!(
        blue_marshal::to_json(&decoded.value)[WINDOWS_KEY][STACKS_KEY],
        f.root[WINDOWS_KEY][STACKS_KEY]
    );
}

#[test]
fn stack_reset_removes_member_geometry_without_detaching_or_deleting_tabs() {
    let f = Fixture::new();
    let keys = ["bytes:548", "utf8:chatchannel_fleet"];
    f.reset(&keys).unwrap();
    f.assert_only_rectangles_removed(&keys);
}

#[test]
fn partial_stack_and_individual_tab_resets_are_rejected_without_writing() {
    let f = Fixture::new();
    assert!(f.reset(&["bytes:548"]).is_err());
    assert!(f.reset(&["utf8:chatchannel_fleet"]).is_err());
    assert_eq!(fs::read(&f.path).unwrap(), f.original);
    assert!(f.backups().is_empty());
}

#[test]
fn stale_drafts_unknown_targets_and_uncleared_rectangles_leave_the_file_untouched() {
    let f = Fixture::new();
    assert!(f.reset(&["bytes:not_a_window"]).is_err());
    let keys = HashSet::from(["bytes:overview".to_string()]);
    assert!(
        write_window_layout_with_geometry_resets(&f.path, &f.expected, &keys, &f.expected, false)
            .is_err()
    );
    let mut changed = f.root.clone();
    changed[WINDOWS_KEY][OPEN_KEY]["tuple"][1]["bytes:overview"] = json!(false);
    let changed = encode_settings_json(&changed, true).unwrap();
    fs::write(&f.path, &changed).unwrap();
    assert!(
        f.reset(&["bytes:overview"])
            .unwrap_err()
            .contains("changed on disk")
    );
    assert_eq!(fs::read(&f.path).unwrap(), changed);
    assert!(f.backups().is_empty());
}

#[test]
fn backup_failure_keeps_original_and_removes_the_staged_file() {
    let f = Fixture::new();
    fs::write(f.dir.join("backups"), b"blocked").unwrap();
    assert!(f.reset(&["bytes:overview"]).is_err());
    assert_eq!(fs::read(&f.path).unwrap(), f.original);
    assert!(
        !fs::read_dir(&f.dir).unwrap().any(|e| e
            .unwrap()
            .path()
            .extension()
            .is_some_and(|s| s == "tmp"))
    );
}

#[test]
fn malformed_geometry_records_and_non_character_files_are_not_rewritten() {
    let mut f = Fixture::new();
    let user_path = f.dir.join("core_user_12345.dat");
    fs::copy(&f.path, &user_path).unwrap();
    let (draft, keys) = f.draft(&["bytes:overview"]);
    assert!(
        write_window_layout_with_geometry_resets(&user_path, &draft, &keys, &f.expected, false)
            .is_err()
    );
    assert_eq!(fs::read(user_path).unwrap(), f.original);
    f.root[WINDOWS_KEY][RECTS_KEY] = json!({"tuple": ["long:1", "utf8:unsupported"]});
    let malformed = encode_settings_json(&f.root, true).unwrap();
    fs::write(&f.path, &malformed).unwrap();
    f.expected = read_window_layout(&f.path).unwrap().windows;
    assert!(
        f.reset(&["bytes:overview"])
            .unwrap_err()
            .contains("Unexpected window geometry map")
    );
    assert_eq!(fs::read(&f.path).unwrap(), malformed);
    assert!(f.backups().is_empty());
}

#[test]
fn repeated_resets_keep_distinct_recoverable_backups() {
    let f = Fixture::new();
    f.reset(&["bytes:overview"]).unwrap();
    let after_first = fs::read(&f.path).unwrap();
    let expected = read_window_layout(&f.path).unwrap().windows;
    let keys = HashSet::from([
        "bytes:548".to_string(),
        "utf8:chatchannel_fleet".to_string(),
    ]);
    let mut draft = expected.clone();
    for w in &mut draft {
        if keys.contains(&w.key) {
            w.rect = None;
        }
    }
    write_window_layout_with_geometry_resets(&f.path, &draft, &keys, &expected, false).unwrap();
    let copies: Vec<_> = f
        .backups()
        .into_iter()
        .map(|p| fs::read(p).unwrap())
        .collect();
    assert_eq!(copies.len(), 2);
    assert!(copies.contains(&f.original));
    assert!(copies.contains(&after_first));
}

#[test]
fn reset_can_save_explicit_flag_edits_while_preserving_other_records() {
    let f = Fixture::new();
    let (mut draft, keys) = f.draft(&["bytes:overview"]);
    draft
        .iter_mut()
        .find(|w| w.key == "bytes:548")
        .unwrap()
        .locked = true;
    write_window_layout_with_geometry_resets(&f.path, &draft, &keys, &f.expected, false).unwrap();
    let (mut actual, crc) = load_settings_json(&f.path).unwrap();
    assert!(crc);
    let mut wanted = f.root.clone();
    wanted[WINDOWS_KEY][RECTS_KEY]["tuple"][1]
        .as_object_mut()
        .unwrap()
        .remove("bytes:overview");
    wanted[WINDOWS_KEY][LOCKED_KEY]["tuple"][1]["bytes:548"] = json!(true);
    for key in [RECTS_KEY, LOCKED_KEY] {
        actual[WINDOWS_KEY][key]["tuple"][0] = wanted[WINDOWS_KEY][key]["tuple"][0].clone();
    }
    assert_eq!(actual, wanted);
    assert_eq!(fs::read(&f.backups()[0]).unwrap(), f.original);
}

#[test]
fn unknown_geometry_and_duplicate_draft_rows_are_rejected_without_writing() {
    let mut f = Fixture::new();
    let (mut draft, keys) = f.draft(&["bytes:overview"]);
    draft.push(draft[0].clone());
    assert!(
        write_window_layout_with_geometry_resets(&f.path, &draft, &keys, &f.expected, false)
            .is_err()
    );
    f.root[WINDOWS_KEY][RECTS_KEY]["tuple"][1]["bytes:overview"] = json!({"tuple": [1, 2]});
    let unsupported = encode_settings_json(&f.root, true).unwrap();
    fs::write(&f.path, &unsupported).unwrap();
    f.expected = read_window_layout(&f.path).unwrap().windows;
    assert!(
        f.reset(&["bytes:overview"])
            .unwrap_err()
            .contains("Unsupported geometry")
    );
    assert_eq!(fs::read(&f.path).unwrap(), unsupported);
    assert!(f.backups().is_empty());
}

#[test]
fn read_only_files_are_not_replaced_or_backed_up() {
    let f = Fixture::new();
    let original_permissions = fs::metadata(&f.path).unwrap().permissions();
    let mut read_only = original_permissions.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&f.path, read_only).unwrap();
    let result = f.reset(&["bytes:overview"]);
    fs::set_permissions(&f.path, original_permissions).unwrap();
    assert!(result.unwrap_err().contains("read-only"));
    assert_eq!(fs::read(&f.path).unwrap(), f.original);
    assert!(f.backups().is_empty());
}
