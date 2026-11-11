use std::path::PathBuf;

use eve_wrench_core::{FormationProbe, ProbeFormation, SettingsKind, formations};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Entity, TestAppContext, WindowBounds, WindowOptions,
    point, px, size,
};
use serde_json::json;

use super::{MainView, Source, Tab};
use crate::store::Store;

pub(crate) struct Files {
    pub(crate) dir: PathBuf,
    pub(crate) account: String,
    pub(crate) target: String,
    pub(crate) character: String,
    pub(crate) target_character: String,
}

impl Drop for Files {
    fn drop(&mut self) {
        if let Ok(dir) = self.dir.canonicalize() {
            assert!(dir.starts_with(std::env::temp_dir().canonicalize().unwrap()));
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}

pub(crate) fn setup(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<MainView>, Files) {
    let result = cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        crate::formation_editor::init(cx);
        crate::layout_editor::init(cx);
        Store::install_for_test(super::profiles::tests::sample_data(), cx);
        let store = Store::global(cx);
        let dir = store.read(cx).locations().data_dir().to_path_buf();
        let root = dir.join("eve");
        let profile = root
            .join("c_ccp_eve_tq_tranquility")
            .join("settings_Default");
        std::fs::create_dir_all(&profile).unwrap();
        let value = blue_marshal::from_json(&json!({"bytes:ui": {
            "bytes:unrelated": {"tuple": ["long:1", "utf8:keep"]}
        }}))
        .unwrap();
        let bytes = blue_marshal::encode(&value, &blue_marshal::EncodeOptions::default()).unwrap();
        let names = [
            "core_user_2.dat",
            "core_user_10.dat",
            "core_char_3.dat",
            "core_char_4.dat",
        ];
        let paths: Vec<String> = names
            .iter()
            .map(|name| {
                let path = profile.join(name);
                std::fs::write(&path, &bytes).unwrap();
                path.to_string_lossy().into_owned()
            })
            .collect();
        formations::write_probe_formations(
            std::path::Path::new(&paths[0]),
            &[ProbeFormation {
                id: 0,
                name: "Original".into(),
                probes: (0..8)
                    .map(|i| FormationProbe {
                        x: f64::from(i) * 1_000_000.,
                        y: 0.,
                        z: 0.,
                        range: 149_597_870_700.,
                    })
                    .collect(),
            }],
            false,
        )
        .unwrap();
        store.update(cx, |store, cx| {
            store
                .update_config(cx, |config| {
                    config.custom_eve_path = Some(root.to_string_lossy().into_owned());
                })
                .unwrap()
        });
        let (handle, main) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.), px(0.)),
                    size: size(px(1440.), px(900.)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| MainView::new(window, cx)),
        )
        .unwrap();
        (
            handle,
            main,
            Files {
                dir,
                account: paths[0].clone(),
                target: paths[1].clone(),
                character: paths[2].clone(),
                target_character: paths[3].clone(),
            },
        )
    });
    cx.run_until_parked();
    result
}

#[gpui_kit::test]
fn whole_profile_sync_creates_a_named_profile_on_an_empty_server(cx: &mut TestAppContext) {
    let (handle, main, files) = setup(cx);
    let source = std::path::Path::new(&files.account).parent().unwrap();
    let destination_server = files.dir.join("eve/c_ccp_eve_sisi_singularity");
    std::fs::create_dir_all(&destination_server).unwrap();
    std::fs::write(
        source.join("core_public__.yaml"),
        b"audio:\n  volume: 0.5\n",
    )
    .unwrap();
    std::fs::write(source.join("prefs.ini"), b"languageID=EN\n").unwrap();
    std::fs::create_dir_all(source.join("nested/empty")).unwrap();
    std::fs::write(source.join("nested/extra.txt"), b"whole folder").unwrap();
    cx.update(|cx| Store::global(cx).update(cx, |store, cx| store.reload(cx)));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        let data = main.read(cx).store.read(cx).data().unwrap();
        assert!(data.servers.iter().any(|server| {
            server.info.id == eve_wrench_core::Server::Singularity && server.profiles.is_empty()
        }));
        assert!(main.read(cx).source.is_none());
        assert!(main.read(cx).other_source.is_none());
        window.click("profile-copy", cx);
        window.render_frame(cx);
        assert_ne!(window.find("profile-copy-apply").disabled(), Some(true));
        window.click("profile-copy-name", cx);
        window.press("secondary-a", cx);
        window.input("My copied setup", cx);
        window.click("profile-copy-apply", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let destination = destination_server.join("settings_My copied setup");
    for name in [
        "core_user_2.dat",
        "core_user_10.dat",
        "core_char_3.dat",
        "core_char_4.dat",
        "core_public__.yaml",
        "prefs.ini",
        "nested/extra.txt",
    ] {
        assert_eq!(
            std::fs::read(destination.join(name)).unwrap(),
            std::fs::read(source.join(name)).unwrap()
        );
    }
    assert!(destination.join("nested/empty").is_dir());
    cx.update_window(handle, |_, window, cx| {
        assert!(!main.read(cx).copying);
        let data = main.read(cx).store.read(cx).data().unwrap();
        let server = data
            .servers
            .iter()
            .find(|s| s.info.id == eve_wrench_core::Server::Singularity)
            .unwrap();
        assert_eq!(server.profiles[0].name, "My copied setup");
        assert_eq!(server.profiles[0].accounts.len(), 2);
        assert_eq!(server.profiles[0].characters.len(), 2);
        window.render_frame(cx);
        assert!(window.try_find("profile-copy-apply").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn update_indicator_is_compact_and_preview_never_opens_a_modal(cx: &mut TestAppContext) {
    let (handle, main, _files) = setup(cx);
    cx.update_window(handle, |_, window, cx| {
        main.update(cx, |main, cx| {
            main.update.preview = false;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("update-indicator").is_none());
        main.update(cx, |main, cx| {
            main.update.preview = true;
            cx.notify();
        });
        window.render_frame(cx);
        let button = window.find("update-button").bounds();
        assert!(button.top() >= px(34.));
        assert!(button.right() <= window.viewport_size().width);
        assert!(button.size.height <= px(40.));
        assert!(
            window.find("update-indicator").bounds().right()
                <= window.find("settings-trigger").bounds().left()
        );
        window.click("update-button", cx);
        assert_eq!(cx.windows().len(), 1);
        assert_eq!(
            main.read(cx).update.status.as_deref(),
            Some(rust_i18n::t!("update.previewOnly").as_ref())
        );
        assert!(main.read(cx).update.prepared.is_none());
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| window.click("tab-drones", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("drone-tile-hobgoblin", cx);
        window.click("update-button", cx);
        assert_eq!(
            main.read(cx).update.status.as_deref(),
            Some(rust_i18n::t!("update.saveEdits").as_ref())
        );
        assert!(!main.read(cx).update.installing);
        assert_eq!(cx.windows().len(), 1);
    })
    .unwrap();
}

#[gpui_kit::test]
fn editors_are_inline_after_backups_and_probe_save_offers_section_sync(cx: &mut TestAppContext) {
    let (handle, main, files) = setup(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.find("tab-backups").bounds().right()
                <= window.find("tab-probes").bounds().left()
        );
        assert!(
            window.find("tab-probes").bounds().right() <= window.find("tab-drones").bounds().left()
        );
        window.click("tab-probes", cx);
        window.render_frame(cx);
        window.find("probe-editor");
        assert!(window.find("probe-editor").bounds().bottom() <= window.viewport_size().height);
        window.find("editor-account");
        assert!(window.try_find("copy-settings").is_none());
        assert_eq!(cx.windows().len(), 1);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("duplicate-formation", cx);
        window.click("save", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.find("sync-apply");
        window.click("sync-all", cx);
        assert_eq!(window.find("sync-all").checked(), Some(true));
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_ne!(window.find("sync-apply").disabled(), Some(true));
        window.click("sync-apply", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        formations::read_probe_formations(std::path::Path::new(&files.account))
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        formations::read_probe_formations(std::path::Path::new(&files.target))
            .unwrap()
            .len(),
        2
    );
    cx.update_window(handle, |_, window, cx| {
        window.click("tab-drones", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.find("drone-editor");
        assert!(window.find("drone-editor").bounds().bottom() <= window.viewport_size().height);
        assert!(main.read(cx).tab == Some(Tab::Drones));
        assert_eq!(cx.windows().len(), 1);
    })
    .unwrap();
}

#[gpui_kit::test]
fn drone_save_prompts_for_append_or_replace_then_sync(cx: &mut TestAppContext) {
    let (handle, main, files) = setup(cx);
    cx.update_window(handle, |_, window, cx| window.click("tab-drones", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("drone-tile-hobgoblin", cx);
        window.click("drone-groups-next", cx);
        window.click("drone-pattern-input", cx);
        window.press("secondary-a", cx);
        window.input("{type_short} | {name}", cx);
        let sort = window.find("drone-sort-types").bounds();
        window.click_at("drone-sort-types", point(px(8.), sort.size.height / 2.), cx);
        window.click("drone-order-light-down", cx);
        window.click("drone-groups-next", cx);
        window.click("drone-groups-save", cx);
        window.render_frame(cx);
        window.find("drone-save-append");
        window.find("drone-save-replace");
        window.click("drone-save-append", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        eve_wrench_core::drone_groups::read_drone_groups(std::path::Path::new(&files.account))
            .unwrap(),
        std::collections::HashSet::from(["hobgoblin".to_string()]),
    );
    cx.update_window(handle, |_, window, cx| {
        let config_file = main.read(cx).store.read(cx).locations().config_file();
        let config = eve_wrench_core::config::Config::load(&config_file);
        use eve_wrench_core::drone_groups::DroneCategory;
        assert_eq!(
            config.drone_group_order.categories()[..2],
            [DroneCategory::Medium, DroneCategory::Light]
        );
        assert_eq!(
            config.drone_group_pattern.as_deref(),
            Some("{order} {type_short} | {name}")
        );
        let bytes = std::fs::read(&files.account).unwrap();
        assert!(
            bytes
                .windows(b"02 Lgt | Hobgoblin".len())
                .any(|window| window == b"02 Lgt | Hobgoblin")
        );
        window.render_frame(cx);
        window.find("sync-apply");
        window.click("sync-cancel", cx);
        main.update(cx, |main, cx| {
            main.drone_editors.clear();
            cx.notify();
        });
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("drone-groups-next", cx);
        let medium = window.find("drone-order-medium-up").bounds();
        let light = window.find("drone-order-light-up").bounds();
        assert!(
            medium.left() < light.left(),
            "saved Medium/Light order must survive reopening"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn drone_replace_choice_reaches_every_sync_target(cx: &mut TestAppContext) {
    sync_saved_drone_mode(cx, eve_wrench_core::drone_groups::DroneSaveMode::Replace);
}

#[gpui_kit::test]
fn drone_append_choice_keeps_existing_sync_target_groups(cx: &mut TestAppContext) {
    sync_saved_drone_mode(cx, eve_wrench_core::drone_groups::DroneSaveMode::Append);
}

#[gpui_kit::test]
fn drone_update_names_choice_reaches_every_sync_target(cx: &mut TestAppContext) {
    sync_saved_drone_mode(cx, eve_wrench_core::drone_groups::DroneSaveMode::Update);
}

fn sync_saved_drone_mode(
    cx: &mut TestAppContext,
    mode: eve_wrench_core::drone_groups::DroneSaveMode,
) {
    use eve_wrench_core::drone_groups::DroneSaveMode;
    let (handle, _, files) = setup(cx);
    let profile = std::path::Path::new(&files.target).parent().unwrap();
    let targets = [
        PathBuf::from(&files.target),
        profile.join("core_user_28812632.dat"),
        profile.join("core_user_28812633.dat"),
    ];
    let old = json!({"bytes:ui": {
        "bytes:unrelated": {"tuple": ["long:1", "utf8:keep"]},
        "bytes:droneBlah2": {"tuple": ["long:1", {
            "utf8:Old group": {
                "bytes:label": "utf8:Old group",
                "bytes:id": {"tuple": ["utf8:Old group", "bytes:10"]},
                "bytes:droneGroupID": 100,
                "bytes:droneIDs": ["long:123456"]
            }
        }]}
    }});
    let bytes = blue_marshal::encode(
        &blue_marshal::from_json(&old).unwrap(),
        &blue_marshal::EncodeOptions::default(),
    )
    .unwrap();
    for target in &targets {
        std::fs::write(target, &bytes).unwrap();
    }
    cx.update(|cx| Store::global(cx).update(cx, |store, cx| store.reload(cx)));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.click("tab-drones", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("drone-tile-hobgoblin", cx);
        window.click("drone-groups-next", cx);
        window.click("drone-style-plain", cx);
        window.click("drone-groups-next", cx);
        window.click("drone-groups-save", cx);
        window.render_frame(cx);
        window.click(
            match mode {
                DroneSaveMode::Append => "drone-save-append",
                DroneSaveMode::Update => "drone-save-update",
                DroneSaveMode::Replace => "drone-save-replace",
            },
            cx,
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("sync-all", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("sync-apply", cx);
    })
    .unwrap();
    cx.run_until_parked();
    for target in &targets {
        let bytes = std::fs::read(target).unwrap();
        let decoded = blue_marshal::decode(&bytes).unwrap();
        let root = blue_marshal::to_json(&decoded.value);
        let groups = root["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]
            .as_object()
            .unwrap();
        assert_eq!(groups["utf8:Hobgoblin"]["bytes:droneIDs"], json!([]));
        match mode {
            DroneSaveMode::Append | DroneSaveMode::Update => {
                assert_eq!(groups.len(), 2);
                assert_eq!(
                    groups["utf8:Old group"],
                    old["bytes:ui"]["bytes:droneBlah2"]["tuple"][1]["utf8:Old group"]
                );
            }
            DroneSaveMode::Replace => {
                assert_eq!(groups.len(), 1);
                assert!(!groups.contains_key("utf8:Old group"));
            }
        }
        assert_eq!(
            root["bytes:ui"]["bytes:unrelated"],
            old["bytes:ui"]["bytes:unrelated"]
        );
    }
}

#[gpui_kit::test]
fn manual_sources_pair_and_reload_works_on_repeated_left_clicks(cx: &mut TestAppContext) {
    let (handle, main, files) = setup(cx);
    cx.update_window(handle, |_, window, cx| {
        window.click(format!("{}:source", files.character), cx);
        window.click(format!("{}:target", files.target_character), cx);
        window.click(format!("{}:source", files.account), cx);
        window.click(format!("{}:target", files.target), cx);
        assert_eq!(main.read(cx).active_sources(cx).len(), 2);
        assert_eq!(main.read(cx).targets.len(), 2);
        assert!(main.read(cx).source_for(SettingsKind::Char, cx).is_some());
        assert!(main.read(cx).source_for(SettingsKind::User, cx).is_some());
        assert!(super::copy_plan::can_copy(
            &main.read(cx).active_sources(cx),
            &main.read(cx).targets
        ));
        window.render_frame(cx);
        let refresh = window.find("refresh").bounds();
        assert!(
            refresh.top() >= px(34.),
            "Refresh must be outside native title-bar dragging"
        );
    })
    .unwrap();
    for iteration in 0..3 {
        cx.update_window(handle, |_, window, cx| {
            window.click("refresh", cx);
            assert_eq!(main.read(cx).reload_requests, iteration + 1);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(!Store::global(cx).read(cx).is_loading());
            assert_eq!(main.read(cx).active_sources(cx).len(), 2);
        });
    }
    cx.update_window(handle, |_, window, cx| {
        window.click("clear-source", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(main.read(cx).active_sources(cx).is_empty()));
}

#[gpui_kit::test]
fn editor_account_uses_selected_profile_and_explicit_or_default_account(cx: &mut TestAppContext) {
    let (handle, main, files) = setup(cx);
    let alternate = std::path::Path::new(&files.account)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("settings_Alternate");
    std::fs::create_dir_all(&alternate).unwrap();
    let alternate_account = alternate.join("core_user_20.dat");
    std::fs::copy(&files.account, &alternate_account).unwrap();
    cx.update(|cx| {
        Store::global(cx).update(cx, |store, cx| {
            store
                .update_config(cx, |config| {
                    config
                        .default_sources
                        .set_path(SettingsKind::User, Some(files.target.clone()))
                })
                .unwrap();
            store.reload(cx);
        });
    });
    cx.run_until_parked();
    cx.update_window(handle, |_, _, cx| {
        main.update(cx, |main, cx| {
            let data = main.store.read(cx).data().unwrap();
            assert_eq!(main.editor_account(&data, cx).unwrap().path, files.target);
            main.set_source(
                Source::Entry(data.find_entry(&files.account).unwrap().clone()),
                cx,
            );
            assert_eq!(main.editor_account(&data, cx).unwrap().path, files.account);
            let profile = data.servers[0]
                .profiles
                .iter()
                .find(|p| p.name == "Default")
                .unwrap();
            main.editor_accounts
                .insert(profile.path.clone(), files.target.clone());
            assert_eq!(main.editor_account(&data, cx).unwrap().path, files.target);
            main.select_profile(
                data.servers[0].info.id,
                alternate.to_string_lossy().into_owned(),
                cx,
            );
            assert_eq!(
                main.editor_account(&data, cx).unwrap().path,
                alternate_account.to_string_lossy()
            );
        });
    })
    .unwrap();
}

#[gpui_kit::test]
fn layout_tab_preserves_drafts_blocks_updates_and_syncs_only_characters(cx: &mut TestAppContext) {
    use eve_wrench_core::window_layout::{self, WindowRect, WindowState};
    let (handle, main, files) = setup(cx);
    let initial = WindowState {
        key: "bytes:overview".into(),
        rect: Some(WindowRect {
            x: 100,
            y: 100,
            width: 300,
            height: 400,
            screen_width: 1920,
            screen_height: 1080,
        }),
        open: true,
        locked: false,
        minimized: false,
        compact: None,
        light_background: None,
        stack: None,
    };
    window_layout::write_window_layout(std::path::Path::new(&files.character), &[initial], false)
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.click("tab-window-layout", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.find("editor-character");
        window.find("editor-account");
        assert!(window.try_find("copy-settings").is_none());
        assert!(window.find("layout-editor").bounds().bottom() <= window.viewport_size().height);
        window.click("list-bytes:overview", cx);
        window.render_frame(cx);
        window.click("layout-window-properties", cx);
        window.render_frame(cx);
        window.click("flag-locked", cx);
        assert!(
            main.read(cx).layout_editors[&files.character]
                .read(cx)
                .is_dirty()
        );
        window.render_frame(cx);
        window.click("layout-undo", cx);
        assert!(
            !main.read(cx).layout_editors[&files.character]
                .read(cx)
                .is_dirty()
        );
        window.render_frame(cx);
        window.click("layout-redo", cx);
        main.update(cx, |main, cx| {
            main.update.preview = true;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("update-button", cx);
        assert_eq!(
            main.read(cx).update.status.as_deref(),
            Some(rust_i18n::t!("update.saveEdits").as_ref())
        );
        main.update(cx, |main, cx| {
            let data = main.store.read(cx).data().unwrap();
            let profile = main
                .selected_profile(&data.servers[0])
                .unwrap()
                .path
                .clone();
            main.editor_characters
                .insert(profile, files.target_character.clone());
            cx.notify();
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        assert!(
            main.read(cx).layout_editors[&files.character]
                .read(cx)
                .is_dirty()
        );
        assert!(
            !main.read(cx).layout_editors[&files.target_character]
                .read(cx)
                .is_dirty()
        );
        main.update(cx, |main, cx| {
            main.editor_characters.clear();
            cx.notify();
        });
        window.render_frame(cx);
        window.click("layout-save", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("sync-all", cx);
        window.render_frame(cx);
        window.click("sync-apply", cx);
    })
    .unwrap();
    cx.run_until_parked();
    for path in [&files.character, &files.target_character] {
        let layout = window_layout::read_window_layout(std::path::Path::new(path)).unwrap();
        assert!(
            layout
                .windows
                .iter()
                .find(|w| w.key == "bytes:overview")
                .unwrap()
                .locked
        );
        let decoded = blue_marshal::decode(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(
            blue_marshal::to_json(&decoded.value)["bytes:ui"]["bytes:unrelated"]["tuple"][1],
            "utf8:keep"
        );
    }
    for path in [&files.account, &files.target] {
        assert!(
            window_layout::read_window_layout(std::path::Path::new(path))
                .unwrap()
                .windows
                .is_empty()
        );
    }
    cx.update(|cx| assert_eq!(cx.windows().len(), 1));
}

#[gpui_kit::test]
fn geometry_reset_is_undoable_backs_up_and_preserves_stack_state_without_syncing(
    cx: &mut TestAppContext,
) {
    let (handle, main, files) = setup(cx);
    let path = std::path::Path::new(&files.character);
    let decoded = blue_marshal::decode(&std::fs::read(path).unwrap()).unwrap();
    let mut root = blue_marshal::to_json(&decoded.value);
    root["bytes:windows"] = json!({
        "bytes:windowSizesAndPositions_1": {"tuple": ["long:1", {
            "bytes:overview": {"tuple": [100, 100, 300, 400, 1920, 1080]},
            "bytes:548": {"tuple": [500, 500, 400, 300, 1920, 1080]},
            "utf8:chatchannel_fleet": {"tuple": [500, 500, 400, 300, 1920, 1080]}
        }]},
        "bytes:openWindows": {"tuple": ["long:2", {"bytes:overview": true, "bytes:548": true, "utf8:chatchannel_fleet": true}]},
        "bytes:lockedWindows": {"tuple": ["long:3", {"bytes:overview": true}]},
        "bytes:compactWindows": {"tuple": ["long:4", {"bytes:548": true}]},
        "bytes:stacksWindows": {"tuple": ["long:5", {"utf8:chatchannel_fleet": "bytes:548"}]}
    });
    let original = blue_marshal::encode(
        &blue_marshal::from_json(&root).unwrap(),
        &blue_marshal::EncodeOptions {
            checksum: true,
            ..Default::default()
        },
    )
    .unwrap();
    std::fs::write(path, &original).unwrap();
    cx.update(|cx| {
        Store::global(cx).update(cx, |store, cx| {
            store
                .update_config(cx, |config| config.auto_backup = false)
                .unwrap()
        })
    });
    cx.update_window(handle, |_, window, cx| {
        window.click("tab-window-layout", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("list-bytes:overview", cx);
        window.render_frame(cx);
        window.click("layout-window-properties", cx);
        window.render_frame(cx);
        window.click("layout-reset-geometry", cx);
        window.render_frame(cx);
        assert!(window.try_find("window-bytes:overview").is_none());
        window.find("list-bytes:overview");
        assert!(
            main.read(cx).layout_editors[&files.character]
                .read(cx)
                .is_dirty()
        );
        assert_eq!(
            std::fs::read(path).unwrap(),
            original,
            "reset is only a draft until Save"
        );
        window.click("layout-undo", cx);
        window.render_frame(cx);
        window.find("window-bytes:overview");
        assert!(
            !main.read(cx).layout_editors[&files.character]
                .read(cx)
                .is_dirty()
        );
        window.click("layout-redo", cx);
        window.render_frame(cx);
        assert!(window.try_find("window-bytes:overview").is_none());
        window.click("layout-reset", cx);
        window.render_frame(cx);
        window.find("window-bytes:overview");
        assert!(
            !main.read(cx).layout_editors[&files.character]
                .read(cx)
                .is_dirty()
        );
        window.click("list-bytes:548", cx);
        window.render_frame(cx);
        window.click("layout-window-properties", cx);
        window.render_frame(cx);
        window.click("layout-reset-geometry", cx);
        window.render_frame(cx);
        assert!(window.try_find("window-bytes:548").is_none());
        window.find("list-bytes:548");
        window.find("window-bytes:overview");
        assert_eq!(std::fs::read(path).unwrap(), original);
        window.click("layout-save", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let decoded = blue_marshal::decode(&std::fs::read(path).unwrap()).unwrap();
    assert!(decoded.had_crc);
    let mut actual = blue_marshal::to_json(&decoded.value);
    let mut wanted = root.clone();
    let geometry = wanted["bytes:windows"]["bytes:windowSizesAndPositions_1"]["tuple"][1]
        .as_object_mut()
        .unwrap();
    geometry.remove("bytes:548");
    geometry.remove("utf8:chatchannel_fleet");
    actual["bytes:windows"]["bytes:windowSizesAndPositions_1"]["tuple"][0] = json!("long:1");
    assert_eq!(
        actual, wanted,
        "only the stack family's geometry is removed"
    );
    let backups: Vec<_> = std::fs::read_dir(path.parent().unwrap().join("backups"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(std::fs::read(&backups[0]).unwrap(), original);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.try_find("sync-apply").is_none(),
            "reset saves must not offer whole-layout copying to other characters"
        );
        window.find("list-bytes:548");
        assert!(
            !main.read(cx).layout_editors[&files.character]
                .read(cx)
                .has_pending_edits()
        );
    })
    .unwrap();
}
