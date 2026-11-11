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

#[gpui_kit::test]
fn sidebar_selection_and_preview_dropdowns_open_the_matching_editor(cx: &mut TestAppContext) {
    let (handle, editor, files) = setup(cx);
    let original = std::fs::read(&files.account).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.render_frame(cx);
        window.click("sidebar-tab-bytes:overview-0", cx);
        window.render_frame(cx);
        assert!(editor.read(cx).settings_open);
        assert_eq!(
            editor.read(cx).settings.read(cx).selected_tab.as_deref(),
            Some("int:0")
        );
        window.find("overview-tab");
        assert!(window.try_find("layout-settings").is_none());
        assert!(window.try_find("display-page-overview").is_none());
        window.click("layout-fighters", cx);
        window.render_frame(cx);
        window.click("fighter-count-5", cx);
        window.render_frame(cx);
        assert_eq!(editor.read(cx).scene, Scene::Space);
        assert_eq!(editor.read(cx).fighter_count, 5);
        window.find("preview-fighter-squadron-4");
        window.click("layout-targets", cx);
        window.render_frame(cx);
        window.click("target-count-3", cx);
        window.render_frame(cx);
        window.find("preview-target-2");
        assert!(window.try_find("preview-target-3").is_none());
        assert_eq!(editor.read(cx).fighter_count, 5);
        assert!(!editor.read(cx).has_pending_edits());
        window.click("sidebar-neocom", cx);
        window.render_frame(cx);
        window.find("display-field-NeocomWidth");
        let sidebar = window.find("layout-sidebar").bounds();
        let start = point(sidebar.right(), sidebar.top() + px(100.));
        window.drag(start, start + point(px(60.), px(0.)), cx);
        window.render_frame(cx);
        window.render_frame(cx);
        assert!(window.find("layout-sidebar").bounds().size.width > sidebar.size.width + px(30.));
        assert!(!editor.read(cx).has_pending_edits());
    })
    .unwrap();
    assert_eq!(std::fs::read(&files.account).unwrap(), original);
}

#[gpui_kit::test]
fn saved_record_controls_edit_real_leaf_types_and_stage_optional_label_fields(
    cx: &mut TestAppContext,
) {
    let (handle, editor, files) = setup(cx);
    let mut account = read(&files.account);
    account["bytes:overview"]["bytes:shipLabels"] = json!({"tuple": ["long:1", [
        {"bytes:type": "utf8:pilot name", "bytes:pre": "utf8:[", "bytes:state": 1, "bytes:fontsize": null, "bytes:color": null},
        {"bytes:type": "utf8:ship type", "bytes:pre": "utf8:<", "bytes:state": 0, "bytes:fontsize": null, "bytes:color": null}
    ]]});
    let original = encode(account);
    std::fs::write(&files.account, &original).unwrap();
    cx.update_window(handle, |_, window, cx| {
        editor.read(cx).settings.clone().update(cx, |settings, cx| {
            settings.load(PathBuf::from(&files.account), true, window, cx)
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.overview_records_open = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("sidebar-record-shipLabels", cx);
        window.render_frame(cx);
        enter(
            Field::OverviewLeaf("shipLabels".into(), "/0/bytes:fontsize".into()),
            "14",
            window,
            cx,
        );
        enter(
            Field::OverviewLeaf("shipLabels".into(), "/0/bytes:color".into()),
            "12ABEF",
            window,
            cx,
        );
        enter(
            Field::OverviewLeaf("shipLabels".into(), "/0/bytes:pre".into()),
            "(",
            window,
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        {
            let s = editor.read(cx).settings.read(cx);
            assert!(
                s.invalid.is_empty(),
                "invalid={:?}, error={:?}",
                s.invalid,
                s.error
            );
            assert_eq!(
                s.doc(SettingsKind::User)
                    .unwrap()
                    .value("overview", "shipLabels")
                    .unwrap()[0]["bytes:pre"],
                "utf8:(",
                "error={:?}",
                s.error
            );
        }
        click("record-down-0", window, cx);
        window.render_frame(cx);
        let settings = editor.read(cx).settings.read(cx);
        let labels = settings
            .doc(SettingsKind::User)
            .unwrap()
            .value("overview", "shipLabels")
            .unwrap();
        assert_eq!(labels[1]["bytes:pre"], "utf8:(");
        assert_eq!(labels[1]["bytes:fontsize"], 14);
        assert_eq!(labels[1]["bytes:color"]["tuple"][3], 1.);
        assert_eq!(labels[1]["bytes:state"], 1);
        assert!(editor.read(cx).has_pending_edits());
    })
    .unwrap();
    assert_eq!(std::fs::read(&files.account).unwrap(), original);
}
fn read(path: &str) -> Value {
    blue_marshal::to_json(
        &blue_marshal::decode(&std::fs::read(path).unwrap())
            .unwrap()
            .value,
    )
}

#[gpui_kit::test]
fn configured_default_account_loads_before_opening_display_settings(cx: &mut TestAppContext) {
    let (handle, _, files) = crate::main_window::editors_tests::setup(cx);
    let editor = cx
        .update_window(handle, |_, window, cx| {
            Store::global(cx).update(cx, |store, cx| {
                store
                    .update_config(cx, |config| {
                        config.default_sources.account_path = Some(files.target.clone())
                    })
                    .unwrap()
            });
            cx.new(|cx| LayoutEditor::new(PathBuf::from(&files.character), window, cx))
        })
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let settings = editor.read(cx).settings.read(cx);
        assert_eq!(settings.account, Some(PathBuf::from(&files.target)));
        assert_eq!(
            settings.doc(SettingsKind::User).unwrap().path,
            PathBuf::from(&files.target)
        );
        assert!(!editor.read(cx).has_pending_edits());
    });
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
    let mut account = read(&files.account);
    account["bytes:overview"]["bytes:tabsByWindowInstanceID"]["tuple"][1] = json!([[0, 1], [2]]);
    let tabs = account["bytes:overview"]["bytes:tabsettings_new"]["tuple"][1]
        .as_object_mut()
        .unwrap();
    let mut all = tabs["int:0"].clone();
    all["bytes:name"] = json!("utf8:All fields");
    all["bytes:tabColumns"] = json!(
        overview::COLUMNS
            .iter()
            .map(|c| format!("bytes:{}", c.0))
            .collect::<Vec<_>>()
    );
    tabs.insert("int:1".into(), all);
    let mut gates = tabs["int:0"].clone();
    gates["bytes:name"] = json!("utf8:Gates");
    tabs.insert("int:2".into(), gates);
    std::fs::write(&files.account, encode(account)).unwrap();
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
    cx.update(|cx| {
        assert_eq!(
            editor.read(cx).settings.read(cx).account,
            Some(PathBuf::from(&files.account))
        )
    });
    (handle, editor, files)
}

#[gpui_kit::test]
fn preview_tabs_sample_windows_chat_dividers_and_sorting_follow_the_selected_account(
    cx: &mut TestAppContext,
) {
    let (handle, editor, files) = setup(cx);
    let original_char = std::fs::read(&files.character).unwrap();
    let original_account = std::fs::read(&files.account).unwrap();
    cx.update_window(handle, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            editor.show_all_windows = true;
            editor.windows.retain(|w| w.id() == "overview");
            let mut first = editor.windows[0].clone();
            first.rect.as_mut().unwrap().x = 700;
            first.rect.as_mut().unwrap().width = 1100;
            editor.windows[0] = first.clone();
            for (index, id) in [
                "overview_1",
                "chatchannel_local",
                "directionalScannerWindow",
                "ActiveShipCargo",
                "droneview",
                "fleetwindow",
                "watchlistpanel",
                "probeScannerWindow",
            ]
            .into_iter()
            .enumerate()
            {
                let mut item = first.clone();
                item.key = format!("bytes:{id}");
                let r = item.rect.as_mut().unwrap();
                r.x = if index == 0 {
                    700
                } else {
                    50 + (index as i64 % 3) * 190
                };
                r.y = if index == 0 {
                    540
                } else {
                    (index as i64 / 3) * 300
                };
                r.width = if index == 0 { 700 } else { 240 };
                r.height = 250;
                editor.windows.push(item);
            }
            editor.saved = editor.windows.clone();
            cx.notify();
        });
        window.render_frame(cx);
        window.render_frame(cx);
        window.find("overview-tab-bytes:overview-0");
        window.find("overview-tab-bytes:overview-1");
        assert!(window.try_find("overview-tab-bytes:overview-2").is_none());
        window.find("overview-tab-bytes:overview_1-2");
        window.click("overview-tab-bytes:overview-1", cx);
        window.render_frame(cx);
        assert!(!editor.read(cx).has_pending_edits());
        for (column, _, _) in overview::COLUMNS {
            window.find(format!("overview-header-bytes:overview-{column}"));
        }
        for id in ["overview", "overview_1"] {
            for i in 0..5 {
                window.find(format!("overview-sample-bytes:{id}-{i}"));
            }
        }
        let icon = window.find("overview-header-bytes:overview-ICON").bounds();
        let distance = window
            .find("overview-header-bytes:overview-DISTANCE")
            .bounds();
        assert_eq!(icon.size.height, distance.size.height);
        window.find("overview-sort-bytes:overview-DISTANCE");
        window.click("overview-header-bytes:overview-NAME", cx);
        window.render_frame(cx);
        window.find("overview-sort-bytes:overview-NAME");
        window.click("overview-header-bytes:overview-NAME", cx);
        window.render_frame(cx);
        assert_eq!(
            overview::sort(
                editor
                    .read(cx)
                    .settings
                    .read(cx)
                    .doc(SettingsKind::Char)
                    .unwrap(),
                1,
                &["NAME".into()]
            ),
            Some(("NAME".into(), false))
        );
        for id in [
            "directionalScannerWindow",
            "ActiveShipCargo",
            "droneview",
            "fleetwindow",
            "watchlistpanel",
            "probeScannerWindow",
        ] {
            window.find(format!("sample-content-bytes:{id}"));
        }
        window.find("chat-preview-bytes:chatchannel_local");
        window.click("sidebar-overview", cx);
        window.render_frame(cx);
        window.render_frame(cx);
        editor.update(cx, |editor, cx| {
            editor
                .settings
                .update(cx, |s, cx| s.select_chat("chatchannel_local", cx));
        });
        window.render_frame(cx);
        enter(Field::ChatWidth("local".into()), "180", window, cx);
        enter(Field::ChatInput("local".into()), "40", window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let members = window.find("chat-members-bytes:chatchannel_local").bounds();
        let input = window.find("chat-input-bytes:chatchannel_local").bounds();
        let scale = editor.read(cx).mapping().scale;
        assert!((f32::from(members.size.width) - 180. * scale).abs() < 1.);
        assert!((f32::from(input.size.height) - 40. * scale).abs() < 1.);
        window.click("chat-show-members", cx);
        window.render_frame(cx);
        assert!(
            window
                .try_find("chat-members-bytes:chatchannel_local")
                .is_none()
        );
        assert_eq!(std::fs::read(&files.account).unwrap(), original_account);
        assert_eq!(std::fs::read(&files.character).unwrap(), original_char);
        window.click("display-save-account", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        read(&files.account)["bytes:ui"]["bytes:local_userlistwidth"]["tuple"][1],
        180
    );
    assert_eq!(
        read(&files.account)["bytes:ui"]["bytes:chatinputsize_local"]["tuple"][1],
        40
    );
    assert_eq!(
        read(&files.account)["bytes:ui"]["bytes:local_usermode"]["tuple"][1],
        false
    );
    assert_eq!(std::fs::read(&files.character).unwrap(), original_char);
}

#[gpui_kit::test]
fn overview_scrolling_keeps_column_controls_bounded_and_reuses_the_canvas(cx: &mut TestAppContext) {
    let (handle, editor, _files) = setup(cx);
    cx.update_window(handle, |_, window, cx| {
        window.click("sidebar-overview", cx);
        window.render_frame(cx);
        window.render_frame(cx);
        let settings = editor.read(cx).settings.clone();
        let canvas = editor.read(cx).canvas_preview.clone().unwrap();
        let builds = canvas.read(cx).builds;
        window.scroll(
            "display-settings-list",
            ScrollDelta::Lines(point(0., -3.)),
            cx,
        );
        for _ in 0..30 {
            settings.update(cx, |settings, _| {
                settings.scroll.last_frame =
                    std::time::Instant::now() - std::time::Duration::from_millis(16)
            });
            window.simulate_next_frame(cx);
            // `render_frame` deliberately forces a complete cache refresh.
            window.draw(cx).clear(cx);
        }
        assert!(
            canvas.read(cx).builds <= builds + 5,
            "scrolling must reuse the canvas: before {builds}, after {}",
            canvas.read(cx).builds
        );
        assert!(
            window
                .try_find("overview-column-row-ANGULARVELOCITY")
                .is_none()
        );
        click("column-visible-ANGULARVELOCITY", window, cx);
        window.render_frame(cx);
        assert!(window.try_find("overview-column-row-ICON").is_none());
        click("brackets-clear", window, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let settings = editor.read(cx).settings.read(cx);
        let doc = settings.doc(SettingsKind::User).unwrap();
        assert_eq!(overview::tabs(doc)[0].bracket, "Wrench Clear Brackets");
        assert!(doc.is_dirty());
    });
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
                editor.fighter_count = scene.fighter_count();
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
        editor.update(cx, |editor, cx| {
            editor.show_targets = true;
            cx.notify();
        });
        window.render_frame(cx);
        let first = window.find("preview-target-0").bounds();
        let second = window.find("preview-target-1").bounds();
        assert_eq!(first.origin.y, second.origin.y);
        assert!(second.origin.x < first.origin.x);
        window.click("sidebar-overview", cx);
        window.render_frame(cx);
        window.click("sidebar-targets", cx);
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

#[gpui_kit::test]
fn stacked_chat_preview_switches_members_without_changing_saved_layout(cx: &mut TestAppContext) {
    let (handle, editor, _files) = setup(cx);
    cx.update_window(handle, |_, window, cx| {
        editor.update(cx, |editor, cx| {
            let mut stack = editor.windows[0].clone();
            stack.key = "bytes:999".into();
            let mut local = stack.clone();
            local.key = "bytes:chatchannel_local".into();
            local.stack = Some(stack.key.clone());
            let mut fleet = local.clone();
            fleet.key = "bytes:chatchannel_fleet".into();
            editor.windows = vec![stack, local, fleet];
            editor.saved = editor.windows.clone();
            editor.show_all_windows = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.render_frame(cx);
        window.find("stack-preview-tab-bytes:999-bytes:chatchannel_local");
        window.click("stack-preview-tab-bytes:999-bytes:chatchannel_fleet", cx);
        window.render_frame(cx);
        window.find("chat-preview-bytes:chatchannel_fleet");
        assert!(
            window
                .try_find("chat-preview-bytes:chatchannel_local")
                .is_none()
        );
        assert!(!editor.read(cx).has_pending_edits());
    })
    .unwrap();
}

fn enter(field: Field, value: &str, window: &mut Window, cx: &mut App) {
    click(&format!("display-field-{field:?}"), window, cx);
    window.press("secondary-a", cx);
    window.input(value, cx);
}

fn click(id: &str, window: &mut Window, cx: &mut App) {
    if window.try_find(id.to_owned()).is_none() {
        window.scroll(
            "display-settings-list",
            ScrollDelta::Pixels(point(px(0.), px(100000.))),
            cx,
        );
        window.render_frame(cx);
        for _ in 0..30 {
            if window.try_find(id.to_owned()).is_some() {
                break;
            }
            window.scroll(
                "display-settings-list",
                ScrollDelta::Pixels(point(px(0.), px(-120.))),
                cx,
            );
            window.render_frame(cx);
        }
    }
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
        window.click("sidebar-overview", cx);
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
        click("overview-section-1", window, cx);
        window.render_frame(cx);
        window.click("filter-group-27", cx);
        window.render_frame(cx);
        window.click("filter-always-11", cx);
        window.render_frame(cx);
        click("overview-section-2", window, cx);
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
        window.click("sidebar-neocom", cx);
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
            s.saving.insert(PathBuf::from(&files.account));
            assert!(
                s.apply_field(&Field::ChatWidth("local".into()), "180")
                    .is_err()
            );
            s.saving.clear();
            s.loading.insert(PathBuf::from(&files.character));
            assert!(s.apply_field(&Field::NeocomWidth, "64").is_err());
            s.loading.clear();
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
