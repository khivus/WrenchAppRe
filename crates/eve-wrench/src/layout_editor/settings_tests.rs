use super::super::{LayoutEditor, Scene};
use super::{Field, SettingsKind, Store, overview};
use gpui_kit::ScrollDelta;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Entity, TestAppContext, Window, WindowBounds,
    WindowOptions, point, px, size,
};
use serde_json::{Value, json};
use std::path::PathBuf;

fn encode(value: Value) -> Vec<u8> {
    blue_marshal::encode(
        &blue_marshal::from_json(&value).unwrap(),
        &blue_marshal::EncodeOptions {
            checksum: true,
            ..Default::default()
        },
    )
    .unwrap()
}
fn read(path: &str) -> Value {
    blue_marshal::to_json(
        &blue_marshal::decode(&std::fs::read(path).unwrap())
            .unwrap()
            .value,
    )
}

fn setup(
    cx: &mut TestAppContext,
) -> (
    AnyWindowHandle,
    Entity<LayoutEditor>,
    crate::main_window::editors_tests::Files,
) {
    let (_, _, files) = crate::main_window::editors_tests::setup(cx);
    let rects: serde_json::Map<_, _> = [
        "overview",
        "InventoryStation",
        "InventoryStructure",
        "lobbyWnd",
    ]
    .into_iter()
    .map(|id| {
        (
            format!("bytes:{id}"),
            json!({"tuple":[100,100,300,400,1920,1080]}),
        )
    })
    .collect();
    let open: serde_json::Map<_, _> = rects.keys().map(|id| (id.clone(), json!(true))).collect();
    let button = |id: &str| json!({"instance":{"class":"utillib.KeyVal","state":{"bytes:id":format!("bytes:{id}"),"bytes:children":[],"bytes:future":"utf8:keep"}}});
    std::fs::write(
        &files.character,
        encode(json!({"bytes:ui":{
            "bytes:neocomWidth":{"tuple":["long:1",48]},
            "bytes:neocomButtonRawData":{"tuple":["long:1",[button("inventory"),button("wallet")]]},
            "bytes:fightersDetachedPosition":{"tuple":["long:1",{"tuple":[600,300]}]},
            "bytes:future":{"tuple":["long:1","utf8:character"]}
        },"bytes:windows":{
            "bytes:windowSizesAndPositions_1":{"tuple":["long:1",rects]},
            "bytes:openWindows":{"tuple":["long:1",open]}
        }})),
    )
    .unwrap();
    std::fs::write(&files.account,encode(json!({"bytes:ui":{
        "bytes:targetOrigin":{"tuple":["long:1",{"tuple":[0.8,0.1]}]},
        "bytes:alignHorizontally":{"tuple":["long:1",true]},
        "bytes:detachFighterUI":{"tuple":["long:1",true]},
        "bytes:future":{"tuple":["long:1","utf8:account"]}
    },"bytes:overview":{
        "bytes:tabsByWindowInstanceID":{"tuple":["long:1",[[0]]]},
        "bytes:tabsettings_new":{"tuple":["long:1",{"int:0":{"bytes:name":"utf8:Main","bytes:overview":"utf8:My preset","bytes:bracket":"bytes:_BracketFilterShowAll","bytes:tabColumns":["bytes:ICON","bytes:DISTANCE","bytes:NAME"],"bytes:future":"utf8:tab"}}]},
        "bytes:overviewProfilePresets":{"tuple":["long:1",{"utf8:My preset":{"bytes:groups":[27,547,27],"bytes:filteredStates":[11],"bytes:alwaysShownStates":[],"bytes:future":"utf8:preset"}}]},
        "bytes:stateColors":{"tuple":["long:1",{}]},"bytes:stateBlinks":{"tuple":["long:1",{}]}
    }}))).unwrap();
    let path = PathBuf::from(&files.character);
    let (handle, editor) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1440.), px(1000.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| LayoutEditor::new(path, window, cx)),
        )
        .unwrap()
    });
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        let settings = editor.read(cx).settings.clone();
        settings.update(cx, |settings, cx| {
            settings.select_account(PathBuf::from(&files.account), window, cx)
        });
    })
    .unwrap();
    cx.run_until_parked();
    (handle, editor, files)
}

#[gpui_kit::test]
fn scenes_targets_and_fighter_counts_preview_without_changing_files(cx: &mut TestAppContext) {
    let (handle, editor, files) = setup(cx);
    let original_char = std::fs::read(&files.character).unwrap();
    let original_account = std::fs::read(&files.account).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.find("layout-scene");
        window.find("preview-neocom-3");
        for scene in Scene::ALL {
            editor.update(cx, |editor, cx| {
                editor.scene = scene;
                cx.notify();
            });
            window.render_frame(cx);
            let docked = scene.docked();
            assert_eq!(window.try_find("window-bytes:lobbyWnd").is_some(), docked);
            assert_eq!(
                window.try_find("window-bytes:InventoryStation").is_some(),
                scene == Scene::Station
            );
            assert_eq!(
                window.try_find("window-bytes:InventoryStructure").is_some(),
                scene == Scene::Citadel
            );
            assert_eq!(window.try_find("window-bytes:overview").is_some(), !docked);
            if scene.fighter_count() > 0 {
                window.find("preview-fighters");
                for i in 0..scene.fighter_count() {
                    window.find(format!("preview-fighter-squadron-{i}"));
                }
                assert!(
                    window
                        .try_find(format!(
                            "preview-fighter-squadron-{}",
                            scene.fighter_count()
                        ))
                        .is_none()
                );
            } else {
                assert!(window.try_find("preview-fighters").is_none());
            }
        }
        assert!(!editor.read(cx).has_pending_edits());
        editor.update(cx, |editor, cx| {
            editor.scene = Scene::Space;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("layout-targets", cx);
        window.render_frame(cx);
        let first = window.find("preview-target-0").bounds();
        let second = window.find("preview-target-1").bounds();
        assert_eq!(first.origin.y, second.origin.y);
        assert!(second.origin.x < first.origin.x);
        window.click("layout-settings", cx);
        window.render_frame(cx);
        window.click("display-page-targets", cx);
        window.render_frame(cx);
        window.click("target-count-more", cx);
        window.render_frame(cx);
        window.find("preview-target-6");
        assert!(!editor.read(cx).has_pending_edits());
        window.click("targets-horizontal", cx);
        window.render_frame(cx);
        let first = window.find("preview-target-0").bounds();
        let second = window.find("preview-target-1").bounds();
        assert_eq!(first.origin.x, second.origin.x);
        assert!(second.origin.y > first.origin.y);
        assert!(!editor.read(cx).is_dirty());
        assert_eq!(std::fs::read(&files.character).unwrap(), original_char);
        assert_eq!(std::fs::read(&files.account).unwrap(), original_account);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(editor.read(cx).has_pending_edits()));
}

fn enter(field: Field, value: &str, window: &mut Window, cx: &mut App) {
    click(&format!("display-field-{field:?}"), window, cx);
    window.press("secondary-a", cx);
    window.input(value, cx);
}

fn click(id: &str, window: &mut Window, cx: &mut App) {
    if !window.find(id.to_owned()).visible() {
        let delta = window.find("display-settings-list").bounds().center().y
            - window.find(id.to_owned()).bounds().center().y;
        window.scroll(
            "display-settings-list",
            ScrollDelta::Pixels(point(px(0.), delta)),
            cx,
        );
        window.render_frame(cx);
    }
    window.click(id.to_owned(), cx);
}

#[gpui_kit::test]
fn overview_and_character_settings_save_separately_with_backups(cx: &mut TestAppContext) {
    let (handle, editor, files) = setup(cx);
    let original_char = std::fs::read(&files.character).unwrap();
    let original_account = std::fs::read(&files.account).unwrap();
    cx.update(|cx| {
        Store::global(cx).update(cx, |store, cx| {
            store
                .update_config(cx, |config| config.auto_backup = false)
                .unwrap()
        })
    });
    cx.update_window(handle, |_, window, cx| {
        window.click("layout-settings", cx);
        window.render_frame(cx);
        let settings = editor.read(cx).settings.clone();
        crate::scrolling::tests::check_vertical(&settings, |s| &mut s.scroll, window, cx);
        enter(Field::TabName("int:0".into()), "Combat", window, cx);
        enter(Field::TabColor("int:0".into()), "12ABEF", window, cx);
        click("column-visible-SIZE", window, cx);
        window.render_frame(cx);
        click("column-down-ICON", window, cx);
        window.render_frame(cx);
        enter(Field::Width(0, "NAME".into()), "240", window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        assert_eq!(std::fs::read(&files.character).unwrap(), original_char);
        assert_eq!(std::fs::read(&files.account).unwrap(), original_account);
        assert!(editor.read(cx).has_pending_edits());
        assert!(!editor.read(cx).is_dirty());
        window.render_frame(cx);
        window.click("overview-section-1", cx);
        window.render_frame(cx);
        window.click("filter-group-27", cx);
        window.render_frame(cx);
        window.click("filter-always-11", cx);
        window.render_frame(cx);
        window.click("overview-section-2", cx);
        window.render_frame(cx);
        window.click("state-blink-11", cx);
        window.render_frame(cx);
        window.click("display-save-account", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(std::fs::read(&files.character).unwrap(), original_char);
    let account = read(&files.account);
    let tabs = &account["bytes:overview"]["bytes:tabsettings_new"]["tuple"][1];
    assert_eq!(tabs["int:0"]["bytes:name"], "utf8:Combat");
    assert_eq!(tabs["int:0"]["bytes:future"], "utf8:tab");
    assert!(
        tabs["int:0"]["bytes:tabColumns"]
            .as_array()
            .unwrap()
            .contains(&json!("bytes:SIZE"))
    );
    let preset =
        &account["bytes:overview"]["bytes:overviewProfilePresets"]["tuple"][1]["utf8:My preset"];
    assert_eq!(preset["bytes:groups"], json!([547]));
    assert_eq!(preset["bytes:filteredStates"], json!([]));
    assert_eq!(preset["bytes:alwaysShownStates"], json!([11]));
    assert_eq!(preset["bytes:future"], "utf8:preset");
    assert_eq!(
        account["bytes:ui"]["bytes:future"]["tuple"][1],
        "utf8:account"
    );
    cx.update_window(handle, |_, window, cx| {
        let settings = editor.read(cx).settings.clone();
        assert_eq!(
            overview::column_width(
                settings.read(cx).doc(SettingsKind::Char).unwrap(),
                0,
                "NAME"
            ),
            240
        );
        window.render_frame(cx);
        window.click("display-page-neocom", cx);
        window.render_frame(cx);
        enter(Field::NeocomWidth, "999", window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        assert!(
            editor
                .read(cx)
                .settings
                .read(cx)
                .invalid_for(SettingsKind::Char)
        );
        window.render_frame(cx);
        window.click("display-save-character", cx);
        assert_eq!(std::fs::read(&files.character).unwrap(), original_char);
        enter(Field::NeocomWidth, "64", window, cx);
        window.click("neocom-down-0", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        assert_eq!(editor.read(cx).desktop(cx).neocom.width, 64);
        window.render_frame(cx);
        window.click("display-save-character", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let character = read(&files.character);
    assert_eq!(character["bytes:ui"]["bytes:neocomWidth"]["tuple"][1], 64);
    let button =
        &character["bytes:ui"]["bytes:neocomButtonRawData"]["tuple"][1][0]["instance"]["state"];
    assert_eq!(button["bytes:id"], "bytes:wallet");
    assert_eq!(button["bytes:future"], "utf8:keep");
    assert_eq!(
        character["bytes:ui"]["bytes:future"]["tuple"][1],
        "utf8:character"
    );
    assert_eq!(
        character["bytes:windows"],
        blue_marshal::to_json(&blue_marshal::decode(&original_char).unwrap().value)["bytes:windows"]
    );
    let backups = std::fs::read_dir(
        PathBuf::from(&files.character)
            .parent()
            .unwrap()
            .join("backups"),
    )
    .unwrap()
    .map(|e| std::fs::read(e.unwrap().path()).unwrap())
    .collect::<Vec<_>>();
    assert!(backups.contains(&original_char));
    assert!(backups.contains(&original_account));
    cx.update(|cx| assert!(!editor.read(cx).has_pending_edits()));
}

#[gpui_kit::test]
fn unsupported_coordinate_records_are_rejected_without_mutating_settings(cx: &mut TestAppContext) {
    let (handle, editor, files) = setup(cx);
    let mut account = read(&files.account);
    account["bytes:ui"]["bytes:targetOrigin"]["tuple"][1] = json!("utf8:future-format");
    let mut character = read(&files.character);
    character["bytes:ui"]["bytes:fightersDetachedPosition"]["tuple"][1] = json!(true);
    let account_bytes = encode(account);
    let char_bytes = encode(character);
    std::fs::write(&files.account, &account_bytes).unwrap();
    std::fs::write(&files.character, &char_bytes).unwrap();
    cx.update_window(handle, |_, window, cx| {
        let settings = editor.read(cx).settings.clone();
        settings.update(cx, |s, cx| {
            s.load(PathBuf::from(&files.account), true, window, cx);
            s.load(PathBuf::from(&files.character), true, window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, _, cx| {
        let settings = editor.read(cx).settings.clone();
        settings.update(cx, |s, _| {
            assert!(s.apply_field(&Field::TargetX, "60").is_err());
            assert!(s.apply_field(&Field::FighterX, "500").is_err());
            assert!(!s.doc(SettingsKind::User).unwrap().is_dirty());
            assert!(!s.doc(SettingsKind::Char).unwrap().is_dirty());
        });
    })
    .unwrap();
    assert_eq!(std::fs::read(&files.account).unwrap(), account_bytes);
    assert_eq!(std::fs::read(&files.character).unwrap(), char_bytes);
}
