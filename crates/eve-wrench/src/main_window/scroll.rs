use super::MainView;
pub(super) use crate::scrolling::SmoothScroll as TableScroll;
#[cfg(test)]
use crate::scrolling::WheelMotion;
use eve_wrench_core::SettingsKind;
use gpui_kit::*;
use std::ops::Range;

// Keep two extra rows on each side so small wheel movements need little new layout.
pub(super) fn visible_rows(
    count: usize,
    row_top: f32,
    scroll_top: f32,
    viewport: f32,
    row_height: f32,
) -> Range<usize> {
    let overscan = row_height * 2.;
    let start = ((scroll_top - overscan - row_top) / row_height)
        .floor()
        .max(0.) as usize;
    let end = ((scroll_top + viewport + overscan - row_top) / row_height)
        .ceil()
        .max(0.) as usize;
    start.min(count)..end.max(start).min(count)
}

#[derive(Clone, Copy)]
pub(super) enum TableKind {
    Accounts,
    Characters,
    Backups,
}

impl From<SettingsKind> for TableKind {
    fn from(kind: SettingsKind) -> Self {
        match kind {
            SettingsKind::User => Self::Accounts,
            SettingsKind::Char => Self::Characters,
        }
    }
}

impl MainView {
    pub(super) fn reset_browser_scroll(&mut self) {
        for scroll in [
            &mut self.account_scroll,
            &mut self.character_scroll,
            &mut self.backup_scroll,
        ] {
            scroll.reset();
        }
    }

    pub(super) fn table_scroll(&self, kind: TableKind) -> &TableScroll {
        match kind {
            TableKind::Accounts => &self.account_scroll,
            TableKind::Characters => &self.character_scroll,
            TableKind::Backups => &self.backup_scroll,
        }
    }

    fn table_scroll_mut(&mut self, kind: TableKind) -> &mut TableScroll {
        match kind {
            TableKind::Accounts => &mut self.account_scroll,
            TableKind::Characters => &mut self.character_scroll,
            TableKind::Backups => &mut self.backup_scroll,
        }
    }

    pub(super) fn table_wheel_layer(
        &self,
        kind: TableKind,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        self.table_scroll(kind)
            .wheel_layer(move |this| this.table_scroll_mut(kind), cx)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{MainView, Tab};
    use super::{WheelMotion, visible_rows};
    use crate::store::Store;
    use eve_wrench_core::{
        AppData, BackupEntry, ProfileData, Server, ServerData, ServerInfo, SettingsEntry,
        SettingsKind,
    };
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext, Bounds, MouseMoveEvent, PlatformInput, ScrollDelta, ScrollWheelEvent,
        TestAppContext, WindowBounds, WindowOptions, point, px, size,
    };

    #[gpui_kit::test]
    fn tables_scroll_independently_without_panicking_during_animation(cx: &mut TestAppContext) {
        let (handle, view) = cx.update(|cx| {
            gpui_kit::init(cx);
            Store::install_for_test(
                AppData {
                    servers: vec![ServerData {
                        info: ServerInfo {
                            id: Server::Tranquility,
                            brackets_always_show: false,
                            server_path: "/test".into(),
                        },
                        profiles: vec![ProfileData {
                            name: "Default".into(),
                            path: "/test/Default".into(),
                            accounts: (0..300)
                                .map(|index| SettingsEntry {
                                    path: format!("/test/core_user_{index}.dat"),
                                    id: index.to_string(),
                                    kind: SettingsKind::User,
                                    server: Server::Tranquility,
                                    profile: "Default".into(),
                                    display_name: format!("Account {index}"),
                                    character: None,
                                    alias: None,
                                    modified_time: 0,
                                })
                                .collect(),
                            characters: (0..300)
                                .map(|index| SettingsEntry {
                                    path: format!("/test/core_char_{index}.dat"),
                                    id: index.to_string(),
                                    kind: SettingsKind::Char,
                                    server: Server::Tranquility,
                                    profile: "Default".into(),
                                    display_name: format!("Pilot {index}"),
                                    character: None,
                                    alias: None,
                                    modified_time: 0,
                                })
                                .collect(),
                        }],
                    }],
                    backups: Vec::new(),
                },
                cx,
            );
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.), px(0.)),
                        size: size(px(1200.), px(700.)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| MainView::new(window, cx)),
            )
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let accounts = view.read(cx).account_scroll.handle.bounds();
            let characters = view.read(cx).character_scroll.handle.bounds();
            assert!(accounts.size.width >= px(440.));
            assert!(characters.right() <= accounts.left());
            assert_eq!(accounts.top(), characters.top());
            assert_eq!(accounts.size.height, characters.size.height);
            let position = characters.center();
            assert!(view.read(cx).character_scroll.handle.max_offset().y > px(0.));
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    ..Default::default()
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position,
                    delta: ScrollDelta::Lines(point(0., -3.)),
                    ..Default::default()
                }),
                cx,
            );
            assert!(view.read(cx).character_scroll.frame_pending);
            for _ in 0..30 {
                view.update(cx, |view, _| {
                    view.character_scroll.last_frame =
                        std::time::Instant::now() - std::time::Duration::from_millis(16);
                });
                window.simulate_next_frame(cx);
                window.render_frame(cx);
            }
            assert!(view.read(cx).character_scroll.handle.offset().y < px(0.));
            assert!(!view.read(cx).character_scroll.frame_pending);
            assert_eq!(view.read(cx).account_scroll.handle.offset().y, px(0.));
            let before_reverse = view.read(cx).character_scroll.handle.offset().y;
            window.dispatch_event(
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position,
                    delta: ScrollDelta::Lines(point(0., 3.)),
                    ..Default::default()
                }),
                cx,
            );
            for _ in 0..30 {
                view.update(cx, |view, _| {
                    view.character_scroll.last_frame =
                        std::time::Instant::now() - std::time::Duration::from_millis(16);
                });
                window.simulate_next_frame(cx);
                window.render_frame(cx);
            }
            assert!(view.read(cx).character_scroll.handle.offset().y > before_reverse);
            assert!(!view.read(cx).character_scroll.frame_pending);
            let char_offset = view.read(cx).character_scroll.handle.offset().y;
            let position = accounts.center();
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    ..Default::default()
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position,
                    delta: ScrollDelta::Lines(point(0., -3.)),
                    ..Default::default()
                }),
                cx,
            );
            for _ in 0..30 {
                view.update(cx, |view, _| {
                    view.account_scroll.last_frame =
                        std::time::Instant::now() - std::time::Duration::from_millis(16);
                });
                window.simulate_next_frame(cx);
                window.render_frame(cx);
            }
            assert!(view.read(cx).account_scroll.handle.offset().y < px(0.));
            assert_eq!(
                view.read(cx).character_scroll.handle.offset().y,
                char_offset
            );
            assert!(!view.read(cx).account_scroll.frame_pending);

            view.update(cx, |view, _| view.reset_browser_scroll());
            window.render_frame(cx);
            // Start inside each painted thumb, then drag down its track.
            let from = point(characters.right() - px(7.), characters.top() + px(10.));
            window.drag(from, from + point(px(0.), px(100.)), cx);
            window.render_frame(cx);
            assert!(view.read(cx).character_scroll.handle.offset().y < px(0.));
            assert_eq!(view.read(cx).account_scroll.handle.offset().y, px(0.));
            let char_offset = view.read(cx).character_scroll.handle.offset().y;
            let from = point(accounts.right() - px(7.), accounts.top() + px(10.));
            window.drag(from, from + point(px(0.), px(100.)), cx);
            window.render_frame(cx);
            assert!(view.read(cx).account_scroll.handle.offset().y < px(0.));
            assert_eq!(
                view.read(cx).character_scroll.handle.offset().y,
                char_offset
            );
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn backups_use_smooth_wheel_scrolling_and_a_draggable_scrollbar(cx: &mut TestAppContext) {
        let (handle, view) = cx.update(|cx| {
            gpui_kit::init(cx);
            let mut data = super::super::profiles::tests::sample_data();
            data.backups = (0..300)
                .map(|index| BackupEntry {
                    id: index.to_string(),
                    name: format!("Backup {index}"),
                    path: format!("/test/backup_{index}.bak"),
                    timestamp: index,
                    kind: SettingsKind::User,
                    original_id: "1".into(),
                    original_name: None,
                })
                .collect();
            Store::install_for_test(data, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.), px(0.)),
                        size: size(px(1200.), px(700.)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| MainView::new(window, cx)),
            )
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click("tab-backups", cx);
            window.render_frame(cx);
            let bounds = view.read(cx).backup_scroll.handle.bounds();
            assert!(bounds.size.height > px(0.));
            assert!(view.read(cx).backup_scroll.handle.max_offset().y > px(0.));
            let position = bounds.center();
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position,
                    ..Default::default()
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position,
                    delta: ScrollDelta::Lines(point(0., -3.)),
                    ..Default::default()
                }),
                cx,
            );
            assert!(view.read(cx).backup_scroll.frame_pending);
            assert_eq!(view.read(cx).backup_scroll.handle.offset().y, px(0.));
            for frame in 0..30 {
                view.update(cx, |view, _| {
                    view.backup_scroll.last_frame =
                        std::time::Instant::now() - std::time::Duration::from_millis(16);
                });
                window.simulate_next_frame(cx);
                window.render_frame(cx);
                if frame == 0 {
                    let offset = view.read(cx).backup_scroll.handle.offset().y;
                    assert!(offset < px(0.) && offset > px(-30.));
                }
            }
            assert_eq!(view.read(cx).backup_scroll.handle.offset().y, px(-30.));
            assert!(!view.read(cx).backup_scroll.frame_pending);
            assert_eq!(view.read(cx).account_scroll.handle.offset().y, px(0.));
            assert_eq!(view.read(cx).character_scroll.handle.offset().y, px(0.));

            view.update(cx, |view, _| view.reset_browser_scroll());
            window.render_frame(cx);
            let from = point(bounds.right() - px(7.), bounds.top() + px(10.));
            window.drag(from, from + point(px(0.), px(100.)), cx);
            window.render_frame(cx);
            assert!(view.read(cx).backup_scroll.handle.offset().y < px(-30.));

            // A tab change resets the backup viewport, just like the main tables.
            window.click(
                gpui_kit::SharedString::from(format!("tab-{}", Server::Tranquility.id())),
                cx,
            );
            window.render_frame(cx);
            assert!(view.read(cx).tab == Some(Tab::Server(Server::Tranquility)));
            assert_eq!(view.read(cx).backup_scroll.handle.offset().y, px(0.));
        })
        .unwrap();
    }

    #[test]
    fn large_lists_render_a_bounded_window_including_at_the_end() {
        let range = visible_rows(10_000, 36., 200_000., 600., 44.);
        assert!(range.len() <= 19);
        assert!(range.start > 0 && range.end < 10_000);
        let end = visible_rows(10_000, 36., 440_000., 600., 44.);
        assert_eq!(end.end, 10_000);
        assert_eq!(visible_rows(0, 36., 0., 600., 44.), 0..0);
    }

    #[test]
    fn wheel_motion_is_smooth_clamped_and_yields_to_manual_scroll() {
        let mut motion = WheelMotion::default();
        motion.push(0., -30., 1_000.);
        let first = motion.step(0., 1_000., 0.016).unwrap();
        assert!(first < 0. && first > -30.);
        let mut current = first;
        for _ in 0..30 {
            if let Some(next) = motion.step(current, 1_000., 0.016) {
                current = next;
            }
        }
        assert_eq!(current, -30.);
        motion.push(current, -500., 60.);
        for _ in 0..30 {
            if let Some(next) = motion.step(current, 60., 0.016) {
                current = next;
            }
        }
        assert_eq!(current, -60.);
        motion.push(current, 30., 60.);
        assert!(motion.step(-10., 60., 0.016).is_none());
    }

    #[test]
    fn reversing_the_wheel_reacts_without_finishing_the_old_motion() {
        let mut motion = WheelMotion::default();
        motion.push(-100., -300., 1_000.);
        let current = motion.step(-100., 1_000., 0.016).unwrap();
        motion.push(current, 30., 1_000.);
        assert!(motion.step(current, 1_000., 0.016).unwrap() > current);
    }
}
