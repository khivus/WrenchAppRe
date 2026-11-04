use super::*;
use super::{overview::*, scene::*};
use crate::window_layout::WindowState;

struct Fixture(PathBuf);

#[test]
fn chat_dividers_and_sorting_save_only_their_settings_with_backups() {
    let file = Fixture::new("user", true);
    let original = fs::read(&file.0).unwrap();
    let mut doc = SettingsDocument::load(&file.0).unwrap();
    chat::ChatLayout::set(&mut doc, "local", "members", 180).unwrap();
    chat::ChatLayout::set(&mut doc, "local", "input", 40).unwrap();
    chat::ChatLayout::set(&mut doc, "local", "visible", 0).unwrap();
    assert!(chat::ChatLayout::set(&mut doc, "local", "members", 201).is_err());
    assert!(chat::ChatLayout::set(&mut doc, "local", "input", 20).is_err());
    assert!(chat::ChatLayout::set(&mut doc, "../local", "members", 100).is_err());
    let (doc, backup) = doc.save().unwrap();
    assert_eq!(fs::read(backup.unwrap()).unwrap(), original);
    assert_eq!(
        chat::ChatLayout::read(Some(&doc), "chatchannel_local"),
        chat::ChatLayout {
            member_width: 180,
            input_height: 40,
            show_members: false
        }
    );
    assert_eq!(
        chat::ChatLayout::read(Some(&doc), "chatchannel_fleet").member_width,
        128
    );
    assert_eq!(doc.value("ui", "neocomWidth"), Some(&json!(48)));
    let file = Fixture::new("char", false);
    let mut doc = SettingsDocument::load(&file.0).unwrap();
    let visible = vec!["ICON".into(), "NAME".into(), "DISTANCE".into()];
    assert_eq!(sort(&doc, 0, &visible), Some(("DISTANCE".into(), true)));
    set_sort(&mut doc, 0, "NAME", false).unwrap();
    assert_eq!(sort(&doc, 0, &visible), Some(("NAME".into(), false)));
    assert_eq!(sort(&doc, 1, &visible), Some(("DISTANCE".into(), true)));
    assert!(chat::ChatLayout::set(&mut doc, "local", "members", 150).is_err());
    let (doc, backup) = doc.save().unwrap();
    assert!(backup.is_some());
    assert_eq!(sort(&doc, 0, &visible), Some(("NAME".into(), false)));
}

#[test]
fn clear_bracket_filter_preserves_existing_presets_and_excludes_clutter() {
    let file = Fixture::new("user", true);
    let mut doc = SettingsDocument::load(&file.0).unwrap();
    let original =
        doc.value("overview", "overviewProfilePresets").unwrap()["utf8:My preset"].clone();
    set_clear_brackets(&mut doc, "int:0").unwrap();
    let presets = doc.value("overview", "overviewProfilePresets").unwrap();
    assert_eq!(presets["utf8:My preset"], original);
    let groups = ids(Some(&presets["utf8:Wrench Clear Brackets"]["bytes:groups"]));
    assert!(groups.contains(&27) && groups.contains(&10)); // ships and gates
    for id in [100, 101, 299, 186, 226, 319] {
        assert!(!groups.contains(&id));
    }
    let tab = &doc.value("overview", "tabsettings_new").unwrap()["int:0"];
    assert_eq!(tab["bytes:bracket"], "utf8:Wrench Clear Brackets");
    assert_eq!(tab["bytes:showAll"], false);
    assert_eq!(tab["bytes:future"], 42);
    set_clear_brackets(&mut doc, "int:0").unwrap();
    assert_eq!(
        doc.value("overview", "overviewProfilePresets")
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        2
    );
    assert!(set_clear_brackets(&mut doc, "int:99").is_err());
    assert_eq!(
        doc.value("overview", "overviewProfilePresets")
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        2
    );
}
impl Fixture {
    fn new(kind: &str, checksum: bool) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("wrench-display-{}-{nonce}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join(format!("core_{kind}_1.dat"));
        let root = json!({"bytes:unknown":{"bytes:future":{"tuple":["long:7","utf8:keep"]}},"bytes:ui":{
            "bytes:neocomWidth":{"tuple":["long:1",48]},
            "bytes:targetOrigin":{"tuple":["long:1",{"tuple":[0.8,0.1]}]},
            "bytes:SortHeadersSizes":{"tuple":["long:1",{"bytes:unrelated":{"bytes:other":99}}]}
        },"bytes:overview":{
            "bytes:tabsettings_new":{"tuple":["long:1",{"int:0":{"bytes:name":"utf8:Main","bytes:overview":"utf8:My preset","bytes:bracket":"bytes:_BracketFilterShowAll","bytes:future":42}}]},
            "bytes:overviewProfilePresets":{"tuple":["long:1",{"utf8:My preset":{"bytes:groups":[27,547],"bytes:filteredStates":[11],"bytes:future":"utf8:preserve"}}]}
        }});
        fs::write(&path, encode_settings_json(&root, checksum).unwrap()).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let dir = self.0.parent().unwrap().canonicalize().unwrap();
        assert!(dir.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn account_edits_backup_preserve_unknown_data_and_checksum() {
    for checksum in [true, false] {
        let file = Fixture::new("user", checksum);
        let original = fs::read(&file.0).unwrap();
        let mut doc = SettingsDocument::load(&file.0).unwrap();
        set_tab(&mut doc, "int:0", "name", json!("utf8:PvP")).unwrap();
        doc.set("ui", "alignHorizontally", json!(false)).unwrap();
        let (doc, backup) = doc.save().unwrap();
        assert!(!doc.is_dirty());
        assert_eq!(fs::read(backup.unwrap()).unwrap(), original);
        let data = blue_marshal::decode(&fs::read(&file.0).unwrap()).unwrap();
        assert_eq!(data.had_crc, checksum);
        let json = blue_marshal::to_json(&data.value);
        assert_eq!(
            json["bytes:unknown"]["bytes:future"]["tuple"][1],
            "utf8:keep"
        );
        assert_eq!(
            doc.value("overview", "tabsettings_new").unwrap()["int:0"]["bytes:future"],
            42
        );
        assert_eq!(tabs(&doc)[0].name, "PvP");
        assert!(doc.save().unwrap().1.is_none());
    }
}

#[test]
fn character_edits_keep_other_tabs_and_ui_settings() {
    let file = Fixture::new("char", true);
    let mut doc = SettingsDocument::load(&file.0).unwrap();
    set_width(&mut doc, 0, "NAME", 250).unwrap();
    doc.set("ui", "neocomWidth", json!(64)).unwrap();
    doc.set("ui", "fightersDetachedPosition", json!({"tuple":[600,400]}))
        .unwrap();
    assert_eq!(column_width(&doc, 0, "NAME"), 250);
    assert_eq!(column_width(&doc, 1, "NAME"), 160);
    let (doc, _) = doc.save().unwrap();
    assert_eq!(
        doc.value("ui", "SortHeadersSizes").unwrap()["bytes:unrelated"]["bytes:other"],
        99
    );
    assert_eq!(fighters_position(&doc, 1920, 1080), (600, 400));
}

#[test]
fn external_changes_are_rejected_before_backup_or_replacement() {
    let file = Fixture::new("user", false);
    let mut doc = SettingsDocument::load(&file.0).unwrap();
    doc.set("ui", "alignHorizontally", json!(false)).unwrap();
    let changed = encode_settings_json(&json!({"bytes:external":true}), false).unwrap();
    fs::write(&file.0, &changed).unwrap();
    assert!(doc.save().unwrap_err().contains("changed on disk"));
    assert_eq!(fs::read(&file.0).unwrap(), changed);
    assert!(!file.0.parent().unwrap().join("backups").exists());
}

#[test]
fn backup_failure_keeps_original_and_removes_staging_file() {
    let file = Fixture::new("char", false);
    let original = fs::read(&file.0).unwrap();
    let mut doc = SettingsDocument::load(&file.0).unwrap();
    doc.set("ui", "neocomWidth", json!(60)).unwrap();
    fs::write(file.0.parent().unwrap().join("backups"), b"block").unwrap();
    assert!(doc.save().is_err());
    assert_eq!(fs::read(&file.0).unwrap(), original);
    assert!(
        fs::read_dir(file.0.parent().unwrap()).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp"))
    );
}

#[test]
fn invalid_values_and_cross_kind_settings_cannot_be_written() {
    let file = Fixture::new("char", false);
    let mut doc = SettingsDocument::load(&file.0).unwrap();
    for (key, v) in [
        ("neocomWidth", json!(31)),
        ("neocomWidth", json!(73)),
        ("neocomWidth", json!("utf8:48")),
        ("neocomAlign", json!(4)),
        ("targetOrigin", json!({"tuple":[0.5,0.5]})),
        ("unrelated", json!(true)),
    ] {
        assert!(doc.set("ui", key, v).is_err());
    }
    assert!(set_width(&mut doc, 0, "NAME", -1).is_err());
    assert!(!doc.is_dirty());
    let file = Fixture::new("user", false);
    let mut doc = SettingsDocument::load(&file.0).unwrap();
    assert!(
        doc.set("ui", "targetOrigin", json!({"tuple":[1.2,0.5]}))
            .is_err()
    );
    assert!(set_tab(&mut doc, "int:0", "name", json!("utf8:1234567890123456")).is_err());
    assert!(doc.set("ui", "alignHorizontally", json!(2)).is_err());
    assert!(!doc.is_dirty());
}

#[test]
fn legacy_duplicate_group_ids_survive_unrelated_filter_edits() {
    let file = Fixture::new("user", false);
    let mut doc = SettingsDocument::load(&file.0).unwrap();
    let mut presets = doc
        .value("overview", "overviewProfilePresets")
        .cloned()
        .unwrap();
    presets["utf8:My preset"]["bytes:groups"] = json!([27, 547, 27]);
    doc.set("overview", "overviewProfilePresets", presets)
        .unwrap();
    let (mut doc, _) = doc.save().unwrap();
    let mut presets = doc
        .value("overview", "overviewProfilePresets")
        .cloned()
        .unwrap();
    presets["utf8:My preset"]["bytes:filteredStates"] = json!([12]);
    doc.set("overview", "overviewProfilePresets", presets)
        .unwrap();
    let (doc, _) = doc.save().unwrap();
    assert_eq!(
        doc.value("overview", "overviewProfilePresets").unwrap()["utf8:My preset"]["bytes:groups"],
        json!([27, 547, 27])
    );
}

#[test]
fn reset_and_unchanged_values_create_no_write_or_backup() {
    let file = Fixture::new("char", false);
    let original = fs::read(&file.0).unwrap();
    let mut doc = SettingsDocument::load(&file.0).unwrap();
    doc.set("ui", "neocomWidth", json!(48)).unwrap();
    assert!(!doc.is_dirty());
    doc.set("ui", "neocomWidth", json!(72)).unwrap();
    assert!(doc.is_dirty());
    doc.reset();
    assert!(doc.save().unwrap().1.is_none());
    assert_eq!(fs::read(&file.0).unwrap(), original);
}

#[test]
fn drafts_rebase_unrelated_geometry_and_reject_setting_conflicts() {
    let file = Fixture::new("char", false);
    let mut doc = SettingsDocument::load(&file.0).unwrap();
    doc.set("ui", "neocomWidth", json!(60)).unwrap();
    let mut root = doc.original_root.clone();
    root["bytes:windows"] = json!({"bytes:future":true});
    fs::write(&file.0, encode_settings_json(&root, false).unwrap()).unwrap();
    let refreshed = doc.refresh().unwrap();
    assert!(refreshed.is_dirty());
    let (saved, _) = refreshed.save().unwrap();
    assert_eq!(saved.original_root["bytes:windows"]["bytes:future"], true);
    assert!(doc.refresh().unwrap_err().contains("changed on disk"));
    assert!(group_catalog().iter().any(|(id, _)| *id == 547));
}

#[test]
fn target_preview_respects_orientation_growth_and_wrapping() {
    let horizontal = TargetLayout {
        origin: Some([0.9, 0.1]),
        horizontal: true,
    };
    let rows = horizontal.rectangles(1000, 800, 10, &[]);
    assert_eq!(rows[0].x, 790);
    assert_eq!(rows[0].y, 80);
    assert!(rows[1].x < rows[0].x);
    assert!(rows[7].y > rows[0].y);
    let vertical = TargetLayout {
        origin: Some([0.1, 0.9]),
        horizontal: false,
    };
    let cols = vertical.rectangles(1000, 800, 12, &[]);
    assert_eq!(cols[0].x, 100);
    assert_eq!(cols[0].y, 640);
    assert!(cols[1].y < cols[0].y);
    assert!(cols[8].x > cols[0].x);
}

#[test]
fn station_citadel_and_fighter_scenes_stay_separate() {
    let window = |id: &str| WindowState {
        key: format!("bytes:{id}"),
        rect: None,
        open: true,
        locked: false,
        minimized: false,
        compact: None,
        light_background: None,
        stack: None,
    };
    assert!(Scene::Station.allows(&window("InventoryStation")));
    assert!(!Scene::Station.allows(&window("InventoryStructure")));
    assert!(Scene::Citadel.allows(&window("InventoryStructure")));
    assert!(!Scene::Citadel.allows(&window("StationShips")));
    assert!(!Scene::Carrier.allows(&window("lobbyWnd")));
    assert_eq!(Scene::CitadelControl.fighter_count(), 3);
    assert_eq!(Scene::Carrier.fighter_count(), 4);
    assert_eq!(Scene::Supercarrier.fighter_count(), 5);
}

#[test]
fn missing_target_anchor_follows_selected_item_and_client_defaults() {
    let target = TargetLayout::default();
    assert_eq!(target.resolved_origin(1000, 800, &[]), [0.7, 0.02]);
    let mut selected = WindowState {
        key: "bytes:selecteditemview".into(),
        rect: Some(crate::window_layout::WindowRect {
            x: 100,
            y: 80,
            width: 200,
            height: 200,
            screen_width: 1000,
            screen_height: 800,
        }),
        open: true,
        locked: false,
        minimized: false,
        compact: None,
        light_background: None,
        stack: None,
    };
    assert_eq!(
        target.resolved_origin(1000, 800, &[selected.clone()]),
        [0.285, 0.1]
    );
    selected.rect.as_mut().unwrap().x = 800;
    assert_eq!(target.resolved_origin(1000, 800, &[selected]), [0.785, 0.1]);
    assert!(default_states(false).contains(&11));
    assert!(default_states(true).contains(&51));
    assert!(default_blink(false, 51));
}
