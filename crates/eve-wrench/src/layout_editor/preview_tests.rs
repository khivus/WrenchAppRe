use super::{LayoutEditor, WindowRect, WindowState, window_layout};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, point, px, size};
use std::path::PathBuf;

#[gpui_kit::test]
fn bulk_visibility_and_search_are_scoped_reversible_and_preview_only(cx: &mut TestAppContext) {
    let (_, _, files) = crate::main_window::editors_tests::setup(cx);
    let path = PathBuf::from(&files.character);
    let windows: Vec<_> = [
        "overview",
        "overview_1",
        "droneview",
        "market",
        "walletWindow",
        "charactersheet",
        "lobbyWnd",
        "overview_99",
        "notepad",
        "100",
        "chatchannel_player_4321",
        "chatchannel_player_9876",
        "999",
        "chatchannel_player_2468",
    ]
    .into_iter()
    .map(|id| WindowState {
        key: format!("bytes:{id}"),
        rect: (id != "notepad").then_some(WindowRect {
            x: 100,
            y: 100,
            width: 300,
            height: 400,
            screen_width: 1920,
            screen_height: 1080,
        }),
        open: !matches!(
            id,
            "overview_99" | "chatchannel_player_9876" | "chatchannel_player_2468"
        ),
        locked: false,
        minimized: false,
        compact: None,
        light_background: None,
        stack: match id {
            "chatchannel_player_4321" | "chatchannel_player_9876" => Some("bytes:100".into()),
            "chatchannel_player_2468" => Some("bytes:999".into()),
            _ => None,
        },
    })
    .collect();
    let decoded = blue_marshal::decode(&std::fs::read(&path).unwrap()).unwrap();
    let mut root = blue_marshal::to_json(&decoded.value);
    root["bytes:windows"] = serde_json::json!({"bytes:stacksWindows": {"tuple": ["long:1", {
        "bytes:chatchannel_player_4321": "bytes:100",
        "bytes:chatchannel_player_9876": "bytes:100",
        "bytes:chatchannel_player_2468": "bytes:999"
    }]}});
    std::fs::write(
        &path,
        blue_marshal::encode(
            &blue_marshal::from_json(&root).unwrap(),
            &blue_marshal::EncodeOptions::default(),
        )
        .unwrap(),
    )
    .unwrap();
    window_layout::write_window_layout(&path, &windows, false).unwrap();
    let original = std::fs::read(&path).unwrap();
    let (handle, editor) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.), px(0.)),
                    size: size(px(1440.), px(1000.)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| LayoutEditor::new(path.clone(), window, cx)),
        )
        .unwrap()
    });
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        let search = |value: &str, window: &mut gpui_kit::Window, cx: &mut gpui_kit::App| {
            window.click("layout-search", cx);
            window.press("secondary-a", cx);
            window.press("backspace", cx);
            window.input(value, cx);
            window.render_frame(cx);
        };
        window.click("list-bytes:overview", cx);
        window.render_frame(cx);
        window.click("preview-show-all", cx);
        window.click("preview-category-other", cx);
        assert!(editor.read(cx).preview_hidden.is_empty());
        window.click("preview-category-space", cx);
        window.render_frame(cx);
        for id in ["overview", "overview_1", "droneview"] {
            assert!(window.try_find(format!("window-bytes:{id}")).is_none());
            window.find(format!("list-bytes:{id}"));
        }
        window.find("window-bytes:market");
        window.click("layout-window-properties", cx);
        window.render_frame(cx);
        assert_eq!(window.find("flag-preview-hidden").checked(), Some(true));
        assert_eq!(editor.read(cx).selected.as_deref(), Some("bytes:overview"));
        // A mixed category first hides its remaining previews, then shows all.
        window.click("preview-visibility-bytes:overview", cx);
        window.render_frame(cx);
        window.click("preview-category-space", cx);
        window.render_frame(cx);
        assert!(window.try_find("window-bytes:overview").is_none());
        window.click("preview-category-space", cx);
        window.render_frame(cx);
        window.find("window-bytes:overview");
        window.find("window-bytes:droneview");
        window.click("preview-hide-all", cx);
        window.render_frame(cx);
        assert_eq!(editor.read(cx).visible().count(), 0);
        for key in [
            "bytes:lobbyWnd",
            "bytes:overview_99",
            "bytes:notepad",
            "bytes:chatchannel_player_4321",
        ] {
            assert!(!editor.read(cx).preview_hidden.contains(key), "{key}");
        }
        window.click("preview-show-all", cx);
        window.render_frame(cx);
        assert!(editor.read(cx).preview_hidden.is_empty());
        let visible = editor.read(cx).visible().count();
        search("  OvErViEw  ", window, cx);
        window.find("list-bytes:overview_1");
        window.find("list-bytes:overview_99");
        assert!(window.try_find("window-bytes:overview_99").is_none());
        assert!(!editor.read(cx).show_all_windows);
        assert!(window.try_find("list-bytes:market").is_none());
        assert_eq!(
            editor.read(cx).visible().count(),
            visible,
            "search must not change the canvas"
        );
        assert!(
            window.find("preview-hide-all").bounds().right()
                <= window.find("layout-search").bounds().right()
        );
        window.click("preview-hide-all", cx);
        window.render_frame(cx);
        assert!(window.try_find("window-bytes:overview").is_none());
        assert!(window.try_find("window-bytes:overview_1").is_none());
        window.find("window-bytes:droneview");
        window.find("window-bytes:market");
        window.click("preview-show-all", cx);
        window.render_frame(cx);
        search("lobby", window, cx);
        window.find("list-bytes:lobbyWnd");
        assert!(window.try_find("window-bytes:lobbyWnd").is_none());
        search("9876", window, cx);
        window.find("list-bytes:100");
        search("2468", window, cx);
        window.find("list-bytes:999");
        assert!(window.try_find("window-bytes:999").is_none());
        search("overview 1", window, cx);
        window.find("list-bytes:overview_1");
        assert!(window.try_find("list-bytes:overview").is_none());
        window.click("preview-category-space", cx);
        window.render_frame(cx);
        window.find("window-bytes:overview");
        assert!(window.try_find("window-bytes:overview_1").is_none());
        search("4321", window, cx);
        window.find("list-bytes:100");
        window.click("preview-category-social", cx);
        window.render_frame(cx);
        assert!(window.try_find("window-bytes:100").is_none());
        assert!(
            !editor
                .read(cx)
                .preview_hidden
                .contains("bytes:chatchannel_player_4321")
        );
        search("does not exist", window, cx);
        window.find("window-list-empty");
        let hidden = editor.read(cx).preview_hidden.clone();
        window.click("preview-show-all", cx);
        window.click("preview-hide-all", cx);
        assert_eq!(editor.read(cx).preview_hidden, hidden);
        search("", window, cx);
        window.find("list-bytes:100");
        window.click("preview-show-all", cx);
        window.render_frame(cx);
        assert_eq!(editor.read(cx).visible().count(), visible);
        assert!(window.try_find("list-bytes:overview_99").is_none());
        assert!(window.try_find("list-bytes:999").is_none());
        assert!(window.try_find("list-bytes:lobbyWnd").is_none());
        assert!(!editor.read(cx).is_dirty());
        assert!(!editor.read(cx).history.can_undo());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    })
    .unwrap();
}
