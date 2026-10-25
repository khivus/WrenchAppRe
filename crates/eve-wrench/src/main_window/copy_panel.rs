use eve_wrench_core::SettingsKind;
use eve_wrench_core::copy::COPY_GROUPS;
use gpui_kit::assets::IconName;
use gpui_kit::base::{Scrollbar, ScrollbarMode};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use rust_i18n::t;
use std::ops::Range;

use super::copy_plan::can_copy;
use super::{DraggedSettings, MainView, Source};
use crate::table::{self, Column};
use crate::ui;

// Avatar, name, server badge, remove. The remove slot is kept on the source
// row too, so badges line up across both lists.
const PANEL_COLUMNS: [Column; 4] = [
    Column::fixed(px(36.)),
    Column::fill(),
    Column::fixed(px(44.)).end(),
    Column::fixed(px(28.)).end(),
];

impl MainView {
    fn set_copy_group(
        &mut self,
        id: &'static str,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.copying {
            return;
        }
        if let Err(error) = self.store.update(cx, |store, cx| {
            store.update_config(cx, |config| {
                config.copy_groups.insert(id.to_string(), enabled);
            })
        }) {
            ui::notify_error(t!("copyPanel.saveFailed"), error, window, cx);
        }
        cx.notify();
    }

    pub(super) fn render_copy_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let sources = self.active_sources(cx);
        let can_copy = can_copy(&sources, &self.targets) && !self.copying;
        let source_kinds: Vec<_> = sources.iter().map(Source::kind).collect();

        v_flex()
            .w(px(300.))
            .flex_none()
            .border_l_1()
            .border_color(cx.theme().border)
            .text_sm()
            .child(self.render_source(cx))
            .child(self.render_targets(cx))
            .when(!source_kinds.is_empty(), |this| {
                this.child(self.render_group_options(&source_kinds, cx))
            })
            .child(
                div()
                    .flex_none()
                    .p_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("copy-settings")
                            .primary()
                            .w_full()
                            .icon(IconName::Copy)
                            .label(if self.copying {
                                t!("copyPanel.copying").to_string()
                            } else {
                                t!("copyPanel.copySettings").to_string()
                            })
                            .loading(self.copying)
                            .disabled(!can_copy)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.execute_copy(window, cx)),
                            ),
                    ),
            )
    }

    fn panel_header(
        &self,
        id: &'static str,
        title: String,
        count: Option<usize>,
        clearable: bool,
        on_clear: fn(&mut Self),
        cx: &mut Context<Self>,
    ) -> Div {
        ui::section_header(cx)
            .justify_between()
            .child(
                h_flex()
                    .gap_2()
                    .child(ui::section_title(title))
                    .children(count.map(|n| ui::count_label(n, cx))),
            )
            .when(clearable, |this| {
                this.child(
                    Button::new(id)
                        .ghost()
                        .xsmall()
                        .disabled(self.copying)
                        .label(t!("common.clear").to_string())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            on_clear(this);
                            cx.notify();
                        })),
                )
            })
    }

    fn empty_hint(text: String, cx: &App) -> Div {
        let muted = cx.theme().muted_foreground;
        v_flex()
            .gap_0p5()
            .px_4()
            .py_3()
            .text_xs()
            .text_color(muted)
            .child(text)
            .child(
                div()
                    .text_color(muted.opacity(0.7))
                    .child(t!("copyPanel.dropHint").to_string()),
            )
    }

    fn render_source(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let sources = self.active_sources(cx);
        let header = self.panel_header(
            "clear-source",
            t!("copyPanel.source").to_string(),
            None,
            !sources.is_empty() || self.using_default_sources,
            |this| {
                this.source = None;
                this.other_source = None;
                this.using_default_sources = false;
            },
            cx,
        );

        let rows: Vec<AnyElement> = sources
            .iter()
            .map(|source| {
                let entry = match source {
                    Source::Entry(e) => Some(e),
                    Source::Backup(_) => None,
                };
                let portrait = entry.and_then(|e| self.store.read(cx).portrait(&e.id));
                let name = v_flex()
                    .min_w_0()
                    .child(div().truncate().child(source.display_name().to_string()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(match source {
                                Source::Backup(_) => t!("titleBar.backups").to_string(),
                                Source::Entry(entry) => match entry.kind {
                                    SettingsKind::Char => t!("defaultSource.character").to_string(),
                                    SettingsKind::User => t!("defaultSource.account").to_string(),
                                },
                            }),
                    );
                table::cells(
                    ui::list_row(
                        SharedString::from(format!("source-row-{}", source.path())),
                        Some(ui::source_color()),
                        cx,
                    )
                    .h(ui::ROW_HEIGHT),
                    &PANEL_COLUMNS,
                    [
                        ui::entry_avatar(source.kind(), portrait, px(24.), cx),
                        name.into_any_element(),
                        entry
                            .map(|e| ui::server_badge(e.server).into_any_element())
                            .unwrap_or_else(table::empty),
                        table::empty(),
                    ],
                )
                .into_any_element()
            })
            .collect();
        let body = if rows.is_empty() {
            Self::empty_hint(t!("copyPanel.noSourceSelected").to_string(), cx).into_any_element()
        } else {
            v_flex().children(rows).into_any_element()
        };

        let tint = ui::source_color().opacity(0.12);
        v_flex()
            .flex_none()
            .child(header)
            .child(body)
            .drag_over::<DraggedSettings>(move |style, _, _, _| style.bg(tint))
            .on_drop(cx.listener(|this, dragged: &DraggedSettings, _, cx| {
                this.set_source(dragged.0.clone(), cx)
            }))
    }

    fn render_targets(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let has_targets = !self.targets.is_empty();
        let header = self
            .panel_header(
                "clear-targets",
                t!("copyPanel.targets").to_string(),
                has_targets.then_some(self.targets.len()),
                has_targets,
                |this| this.targets.clear(),
                cx,
            )
            .border_t_1();

        let source_kinds: Vec<_> = self.active_sources(cx).iter().map(Source::kind).collect();
        let tint = ui::target_color().opacity(0.12);
        v_flex()
            .flex_1()
            .min_h_0()
            .drag_over::<DraggedSettings>(move |style, _, _, _| style.bg(tint))
            .can_drop(move |value, _, _| {
                value
                    .downcast_ref::<DraggedSettings>()
                    .is_some_and(|dragged| match &dragged.0 {
                        Source::Entry(entry) => {
                            source_kinds.is_empty() || source_kinds.contains(&entry.kind)
                        }
                        Source::Backup(_) => false,
                    })
            })
            .on_drop(cx.listener(|this, dragged: &DraggedSettings, window, cx| {
                if let Source::Entry(entry) = &dragged.0 {
                    this.add_target(entry.clone(), window, cx);
                }
            }))
            .child(header)
            .child(
                div()
                    .id("targets-scroll")
                    .test_support()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(if has_targets {
                        uniform_list(
                            "selected-targets",
                            self.targets.len(),
                            cx.processor(|this, range: Range<usize>, _, cx| {
                                this.render_target_rows(range, cx)
                            }),
                        )
                        .size_full()
                        .pr_4()
                        .track_scroll(&self.target_scroll)
                        .into_any_element()
                    } else {
                        Self::empty_hint(t!("copyPanel.noTargetsSelected").to_string(), cx)
                            .into_any_element()
                    })
                    .child(
                        self.target_motion
                            .wheel_layer(|this| &mut this.target_motion, cx),
                    )
                    .when(has_targets, |this| {
                        this.child(
                            Scrollbar::vertical(&self.target_scroll)
                                .id("targets-scrollbar")
                                .mode(ScrollbarMode::Always),
                        )
                    }),
            )
    }

    fn render_target_rows(
        &mut self,
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        #[cfg(test)]
        {
            self.target_rows_built += range.len();
        }
        self.targets
            .get(range)
            .unwrap_or_default()
            .iter()
            .map(|target| {
                let path = target.path.clone();
                let portrait = self.store.read(cx).portrait(&target.id);
                let remove = div()
                    .opacity(0.)
                    .group_hover("target-row", |s| s.opacity(1.))
                    .child(
                        Button::new(SharedString::from(format!("remove-{}", target.path)))
                            .ghost()
                            .xsmall()
                            .icon(IconName::X)
                            .disabled(self.copying)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.targets.retain(|t| t.path != path);
                                cx.notify();
                            })),
                    );
                table::cells(
                    ui::list_row(
                        SharedString::from(format!("target-{}", target.path)),
                        Some(ui::target_color()),
                        cx,
                    )
                    .h(ui::ROW_HEIGHT)
                    .w_full()
                    .group("target-row")
                    .test_support(),
                    &PANEL_COLUMNS,
                    [
                        ui::entry_avatar(target.kind, portrait, px(24.), cx),
                        div()
                            .truncate()
                            .child(target.display_name.clone())
                            .into_any_element(),
                        h_flex()
                            .id(SharedString::from(format!("target-server-{}", target.path)))
                            .test_support()
                            .flex_none()
                            .child(ui::server_badge(target.server))
                            .into_any_element(),
                        remove.into_any_element(),
                    ],
                )
                .into_any_element()
            })
            .collect()
    }

    fn render_group_options(
        &self,
        kinds: &[SettingsKind],
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let hover = cx.theme().muted.opacity(0.6);
        let config = self.store.read(cx).config();
        let copy_drone_groups = COPY_GROUPS
            .iter()
            .any(|group| group.id == "drone_groups" && config.copy_group_enabled(group));
        let groups: Vec<AnyElement> = COPY_GROUPS
            .iter()
            .filter(|g| kinds.iter().any(|kind| g.applies_to(*kind)))
            .map(|group| {
                let id = group.id;
                let checked = self.store.read(cx).config().copy_group_enabled(group);
                h_flex()
                    .id(SharedString::from(format!("group-row-{}", id)))
                    .test_support()
                    .px_4()
                    .py_1p5()
                    .hover(move |s| s.bg(hover))
                    .child(
                        Checkbox::new(SharedString::from(format!("group-{}", id)))
                            .small()
                            .label(t!(format!("copyGroups.{}", id)).to_string())
                            .checked(checked)
                            .disabled(
                                self.copying || (id == "drone_assignments" && !copy_drone_groups),
                            )
                            .on_change(cx.listener(move |this, checked: &bool, window, cx| {
                                this.set_copy_group(id, *checked, window, cx);
                            })),
                    )
                    .into_any_element()
            })
            .collect();

        v_flex()
            .flex_none()
            .child(
                self.panel_header(
                    "copy-options",
                    t!("copyPanel.copyOptions").to_string(),
                    None,
                    false,
                    |_| {},
                    cx,
                )
                .border_t_1(),
            )
            .child(
                self.copy_options_scroll
                    .view(
                        "copy-groups",
                        v_flex().py_1().children(groups),
                        |this| &mut this.copy_options_scroll,
                        cx,
                    )
                    .max_h(px(220.)),
            )
            .child(
                div()
                    .px_4()
                    .pb_3()
                    .pt_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("copyPanel.selectiveHint").to_string()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::super::{MainView, Source, profiles::tests::sample_data};
    use crate::store::Store;
    use eve_wrench_core::config::Config;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext, Bounds, ScrollDelta, ScrollStrategy, SharedString, TestAppContext,
        WindowBounds, WindowOptions, point, px, size,
    };

    #[gpui_kit::test]
    fn copy_options_and_targets_scroll_smoothly_and_independently(cx: &mut TestAppContext) {
        let data = sample_data();
        let account = data.servers[0].profiles[1].accounts[0].clone();
        let character = data.servers[0].profiles[1].characters[0].clone();
        let (handle, view) = cx.update(|cx| {
            gpui_kit::init(cx);
            Store::install_for_test(data, cx);
            gpui_kit::open_window(
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
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            view.update(cx, |view, cx| {
                view.set_source(Source::Entry(account.clone()), cx);
                view.set_source(Source::Entry(character), cx);
                for index in 0..300 {
                    let mut target = account.clone();
                    target.path = format!("/test/target-{index}.dat");
                    view.targets.push(target);
                }
            });
            window.render_frame(cx);
            crate::scrolling::tests::check_vertical(
                &view,
                |view| &mut view.copy_options_scroll,
                window,
                cx,
            );
            assert_eq!(
                view.read(cx)
                    .target_scroll
                    .0
                    .borrow()
                    .base_handle
                    .offset()
                    .y,
                px(0.)
            );
            crate::scrolling::tests::check_vertical(
                &view,
                |view| &mut view.target_motion,
                window,
                cx,
            );
            assert_eq!(view.read(cx).copy_options_scroll.handle.offset().y, px(0.));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn four_hundred_targets_only_build_visible_rows_and_toolbar_is_below_defaults(
        cx: &mut TestAppContext,
    ) {
        let mut data = sample_data();
        let profile = &mut data.servers[0].profiles[1];
        let template = profile.characters[0].clone();
        profile.characters = (0..=400)
            .map(|id| {
                let mut entry = template.clone();
                entry.id = id.to_string();
                entry.path = format!("{}/Char_{id}.dat", profile.path);
                entry.display_name = if id == 1 {
                    "A".to_string()
                } else {
                    format!("Pilot with a longer name {id}")
                };
                entry
            })
            .collect();
        let source = Source::Entry(profile.characters[0].clone());
        let first_path = profile.characters[1].path.clone();
        let second_path = profile.characters[2].path.clone();
        let last_path = profile.characters[400].path.clone();
        let (handle, view) = cx.update(|cx| {
            gpui_kit::init(cx);
            Store::install_for_test(data, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.), px(0.)),
                        size: size(px(1440.), px(800.)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| MainView::new(window, cx)),
            )
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            view.update(cx, |view, cx| view.set_source(source, cx));
            window.render_frame(cx);
            let profile = window.find("profile-controls").bounds();
            let defaults = window.find("default-sources-controls").bounds();
            let pickers = window.find("default-source-pickers").bounds();
            let targets = window.find("add-target-controls").bounds();
            assert!(profile.bottom() <= defaults.top());
            assert!(pickers.bottom() <= targets.top());
            assert!(targets.bottom() <= defaults.bottom());
            window.click("targets-profile", cx);
            assert_eq!(view.read(cx).targets.len(), 400);
            let mut max_rows = 0;
            for _ in 0..12 {
                view.update(cx, |view, cx| { view.target_rows_built = 0; cx.notify(); });
                window.render_frame(cx);
                max_rows = max_rows.max(view.read(cx).target_rows_built);
                assert!(view.read(cx).target_rows_built < 30);
                assert_eq!(view.read(cx).targets.len(), 400);
            }
            assert!(max_rows > 0);
            assert!(window.find(SharedString::from(format!("target-{first_path}"))).visible());
            let viewport = window.find("targets-scroll").bounds();
            let first_row = window.find(SharedString::from(format!("target-{first_path}"))).bounds();
            let second_row = window.find(SharedString::from(format!("target-{second_path}"))).bounds();
            assert_eq!(first_row.left(), viewport.left());
            assert_eq!(first_row.right(), viewport.right() - px(16.));
            assert_eq!(second_row.size.width, first_row.size.width);
            let first_badge = window.find(SharedString::from(format!("target-server-{first_path}"))).bounds();
            let second_badge = window.find(SharedString::from(format!("target-server-{second_path}"))).bounds();
            assert_eq!(first_badge.right(), second_badge.right());
            assert_eq!(first_badge.right(), first_row.right() - px(44.));
            assert!(window.try_find(SharedString::from(format!("target-{last_path}"))).is_none());
            window.scroll("targets-scroll", ScrollDelta::Lines(point(0., -3.)), cx);
            view.update(cx, |view, cx| {
                view.target_rows_built = 0;
                view.target_scroll.scroll_to_item(399, ScrollStrategy::Bottom);
                cx.notify();
            });
            window.render_frame(cx);
            assert!(view.read(cx).target_rows_built < 30);
            let last_row = SharedString::from(format!("target-{last_path}"));
            assert!(window.find(last_row.clone()).visible());
            window.hover(last_row, cx);
            window.click(SharedString::from(format!("remove-{last_path}")), cx);
            assert_eq!(view.read(cx).targets.len(), 399);
            assert!(!view.read(cx).targets.contains_path(&last_path));
            assert!(view.read(cx).targets.contains_path(&first_path));
            window.click("clear-targets", cx);
            assert!(view.read(cx).targets.is_empty());
            assert!(!view.read(cx).targets.contains_path(&first_path));
            eprintln!("400 selected targets: at most {max_rows} target rows built per repaint in the test viewport");
        }).unwrap();
    }

    #[gpui_kit::test]
    fn copy_options_save_on_click_across_source_kinds_and_new_views(cx: &mut TestAppContext) {
        let data = sample_data();
        let character = Source::Entry(data.servers[0].profiles[1].characters[0].clone());
        let account = Source::Entry(data.servers[0].profiles[1].accounts[0].clone());
        let (handle, view) = cx.update(|cx| {
            gpui_kit::init(cx);
            Store::install_for_test(data, cx);
            gpui_kit::open_window(
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
            .unwrap()
        });
        let config_file = cx
            .update_window(handle, |_, window, cx| {
                view.update(cx, |view, cx| view.set_source(character.clone(), cx));
                window.render_frame(cx);
                assert!(window.try_find("group-row-drone_groups").is_none());
                assert!(window.try_find("group-row-drone_assignments").is_none());
                let row = window.find("group-row-windows").bounds();
                window.click_at(
                    "group-row-windows",
                    point(px(24.), row.size.height / 2.),
                    cx,
                );
                let config_file = Store::global(cx).read(cx).locations().config_file();
                assert_eq!(
                    Config::load(&config_file).copy_groups.get("windows"),
                    Some(&false)
                );
                view.update(cx, |view, cx| view.set_source(account, cx));
                window.render_frame(cx);
                let row = window.find("group-row-drone_groups").bounds();
                window.click_at(
                    "group-row-drone_groups",
                    point(px(24.), row.size.height / 2.),
                    cx,
                );
                let row = window.find("group-row-drone_assignments").bounds();
                window.click_at(
                    "group-row-drone_assignments",
                    point(px(24.), row.size.height / 2.),
                    cx,
                );
                assert!(
                    !Config::load(&config_file)
                        .copy_groups
                        .contains_key("drone_assignments")
                );
                let row = window.find("group-row-drone_groups").bounds();
                window.click_at(
                    "group-row-drone_groups",
                    point(px(24.), row.size.height / 2.),
                    cx,
                );
                let row = window.find("group-row-drone_assignments").bounds();
                window.click_at(
                    "group-row-drone_assignments",
                    point(px(24.), row.size.height / 2.),
                    cx,
                );
                let row = window.find("group-row-overview").bounds();
                window.click_at(
                    "group-row-overview",
                    point(px(24.), row.size.height / 2.),
                    cx,
                );
                let saved = Config::load(&config_file);
                assert_eq!(saved.copy_groups.get("windows"), Some(&false));
                assert_eq!(saved.copy_groups.get("overview"), Some(&false));
                assert_eq!(saved.copy_groups.get("drone_groups"), Some(&true));
                assert_eq!(saved.copy_groups.get("drone_assignments"), Some(&true));
                config_file
            })
            .unwrap();
        let (handle, view) = cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| MainView::new(window, cx))
            })
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            view.update(cx, |view, cx| view.set_source(character, cx));
            window.render_frame(cx);
            let row = window.find("group-row-windows").bounds();
            window.click_at(
                "group-row-windows",
                point(px(24.), row.size.height / 2.),
                cx,
            );
            let saved = Config::load(&config_file);
            assert_eq!(saved.copy_groups.get("windows"), Some(&true));
            assert_eq!(saved.copy_groups.get("overview"), Some(&false));
            assert_eq!(saved.copy_groups.get("drone_groups"), Some(&true));
            assert_eq!(saved.copy_groups.get("drone_assignments"), Some(&true));
            assert!(view.read(cx).targets.is_empty());
            std::fs::remove_file(&config_file).unwrap();
            std::fs::remove_dir(config_file.parent().unwrap()).unwrap();
        })
        .unwrap();
    }
}
