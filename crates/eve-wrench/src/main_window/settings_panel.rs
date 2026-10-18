use eve_wrench_core::updates::{APP_VERSION, is_preview_version};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, Theme, ThemeMode, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use rust_i18n::t;

use super::MainView;
use crate::LOCALES;
use crate::store::Store;
use crate::ui::{self, StyleExt as _};

// Contents of the settings popover in the title bar.
pub struct SettingsPanel {
    store: Entity<Store>,
    main: WeakEntity<MainView>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsPanel {
    pub fn new(main: WeakEntity<MainView>, cx: &mut Context<Self>) -> Self {
        let store = Store::global(cx);
        let subscriptions = vec![
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.observe_global::<Theme>(|_, cx| cx.notify()),
        ];
        Self {
            store,
            main,
            _subscriptions: subscriptions,
        }
    }

    fn on_main(
        &self,
        f: impl Fn(&mut MainView, &mut Window, &mut Context<MainView>) + 'static,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let main = self.main.clone();
        move |_, window, cx| {
            main.update(cx, |this, cx| f(this, window, cx)).ok();
        }
    }

    fn section(title: String, cx: &App) -> Div {
        ui::section_header(cx).child(ui::section_title(title))
    }

    fn row(cx: &App) -> Div {
        h_flex()
            .flex_none()
            .px_4()
            .py_2p5()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.6))
    }
}

impl Render for SettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let config = self.store.read(cx).config().clone();
        let muted = cx.theme().muted_foreground;
        let current_locale = rust_i18n::locale().to_string();
        let locale_index = LOCALES
            .iter()
            .position(|(code, _)| *code == current_locale)
            .unwrap_or(0);
        let main = self.main.clone();
        let data = self.store.read(cx).data();

        let folder = match &config.custom_eve_path {
            Some(path) => div().truncate().child(path.clone()),
            None => div()
                .text_color(muted)
                .child(t!("settings.defaultLocation").to_string()),
        };

        let mut version = format!("EVE Wrench {}", APP_VERSION);
        if is_preview_version(APP_VERSION) {
            version = format!("{} · {}", version, t!("titleBar.preview"));
        }

        v_flex()
            .id("settings-panel")
            .test_support()
            .w(px(320.))
            .max_h((window.viewport_size().height - px(80.)).max(px(160.)))
            .overflow_y_scroll()
            .text_sm()
            .child(Self::section(t!("settings.appearance").to_string(), cx))
            .child(
                Self::row(cx)
                    .child(div().flex_1().child(t!("settings.darkTheme").to_string()))
                    .child(
                        div().id("theme-switch").test_support().child(
                            Switch::new("dark-theme")
                                .small()
                                .checked(cx.theme().is_dark())
                                .on_change(|enabled, window, cx| {
                                    crate::set_theme(
                                        if *enabled {
                                            ThemeMode::Dark
                                        } else {
                                            ThemeMode::Light
                                        },
                                        window,
                                        cx,
                                    );
                                }),
                        ),
                    ),
            )
            .child(Self::section(
                t!("settings.eveSettingsFolder").to_string(),
                cx,
            ))
            .child(
                Self::row(cx)
                    .child(div().flex_1().min_w_0().text_xs().child(folder))
                    .child(
                        Button::new("change-folder")
                            .outline()
                            .xsmall()
                            .icon(IconName::FolderOpen)
                            .label(t!("settings.change").to_string())
                            .on_click(self.on_main(|this, window, cx| {
                                this.select_custom_eve_path(window, cx)
                            })),
                    )
                    .when(config.custom_eve_path.is_some(), |this| {
                        this.child(
                            Button::new("reset-folder")
                                .ghost()
                                .xsmall()
                                .icon(IconName::RotateCcw)
                                .tooltip(t!("settings.resetToDefault").to_string())
                                .on_click(self.on_main(|this, window, cx| {
                                    this.clear_custom_eve_path(window, cx)
                                })),
                        )
                    }),
            )
            .child(Self::section(t!("settings.backups").to_string(), cx))
            .child(
                Self::row(cx)
                    .child(div().flex_1().child(t!("settings.autoBackup").to_string()))
                    .child(
                        Switch::new("auto-backup")
                            .small()
                            .checked(config.auto_backup)
                            .on_change(move |_, _, cx| {
                                main.update(cx, |this, cx| this.toggle_auto_backup(cx)).ok();
                            }),
                    ),
            )
            .child(Self::section(t!("extra.title").to_string(), cx))
            .child(
                div()
                    .flex_none()
                    .px_4()
                    .py_2()
                    .text_xs()
                    .text_color(muted)
                    .child(t!("extra.alwaysShowBracketText").to_string())
                    .child(
                        div()
                            .mt_1()
                            .whitespace_normal()
                            .child(t!("extra.alwaysShowBracketTextDesc").to_string()),
                    ),
            )
            .children(data.iter().flat_map(|data| &data.servers).map(|server| {
                let server_path = server.info.server_path.clone();
                let main = self.main.clone();
                Self::row(cx)
                    .child(div().flex_1().child(server.info.id.display_name()))
                    .child(
                        Switch::new(SharedString::from(format!(
                            "brackets-{}",
                            server.info.id.id()
                        )))
                        .small()
                        .checked(server.info.brackets_always_show)
                        .disabled(self.store.read(cx).is_loading())
                        .on_change(move |enabled, window, cx| {
                            main.update(cx, |this, cx| {
                                this.set_brackets_always_show(
                                    server_path.clone(),
                                    *enabled,
                                    window,
                                    cx,
                                )
                            })
                            .ok();
                        }),
                    )
            }))
            .child(Self::section(t!("settings.language").to_string(), cx))
            .child(Self::row(cx).child(ui::segmented(
                "language",
                LOCALES.iter().map(|(_, name)| *name),
                Some(locale_index),
                {
                    let main = self.main.clone();
                    move |index, _, cx| {
                        if let Some((code, _)) = LOCALES.get(index) {
                            main.update(cx, |this, cx| this.set_locale(code, cx)).ok();
                        }
                    }
                },
                cx,
            )))
            .child(Self::section(
                format!(
                    "{} / {}",
                    t!("importExport.import"),
                    t!("importExport.export")
                ),
                cx,
            ))
            .child(
                Self::row(cx)
                    .child(
                        Button::new("export-settings")
                            .outline()
                            .small()
                            .flex_1()
                            .icon(IconName::Download)
                            .label(t!("importExport.export").to_string())
                            .on_click(
                                self.on_main(|this, window, cx| this.export_settings(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("import-settings")
                            .outline()
                            .small()
                            .flex_1()
                            .icon(IconName::Upload)
                            .label(t!("importExport.import").to_string())
                            .on_click(
                                self.on_main(|this, window, cx| this.import_settings(window, cx)),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .px_4()
                    .py_2p5()
                    .gap_3()
                    .bg(cx.theme().muted.opacity(0.35))
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(muted)
                            .font_medium()
                            .child(version),
                    ), // Upstream update checking is disabled for this fork.
                       /*
                       .child(
                           Button::new("check-updates")
                               .ghost()
                               .xsmall()
                               .icon(IconName::RefreshCw)
                               .label(t!("update.checkForUpdates").to_string())
                               .on_click(
                                   self.on_main(|this, window, cx| this.check_for_updates(window, cx)),
                               ),
                       ),
                       */
            )
    }
}

#[cfg(test)]
mod tests {
    use crate::main_window::{MainView, profiles::tests::sample_data};
    use crate::store::Store;
    use eve_wrench_core::config::{Config, ThemePreference};
    use gpui_kit::component::{ActiveTheme as _, Theme, ThemeMode};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, point, px, size,
    };

    #[gpui_kit::test]
    fn cogwheel_theme_switch_changes_palette_both_ways_and_saves(cx: &mut TestAppContext) {
        let (handle, _) = cx.update(|cx| {
            gpui_kit::init(cx);
            Store::install_for_test(sample_data(), cx);
            Theme::change(ThemeMode::Light, None, cx);
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
            let light_background = cx.theme().background;
            window.click("settings-trigger", cx);
            window.render_frame(cx);
            assert!(window.find("theme-switch").visible());
            assert!(window.find("settings-panel").bounds().size.height <= px(620.));
            window.click("theme-switch", cx);
            window.render_frame(cx);
            assert!(cx.theme().is_dark());
            assert_ne!(cx.theme().background, light_background);
            assert_eq!(cx.theme().tokens.background.color, cx.theme().background);
            assert_eq!(
                gpui_kit::base::Theme::global(cx).appearance,
                gpui_kit::base::ThemeAppearance::Dark
            );
            let store = Store::global(cx);
            let config_file = store.read(cx).locations().config_file();
            assert_eq!(store.read(cx).config().theme, Some(ThemePreference::Dark));
            assert_eq!(
                Config::load(&config_file).theme,
                Some(ThemePreference::Dark)
            );
            window.click("theme-switch", cx);
            window.render_frame(cx);
            assert!(!cx.theme().is_dark());
            assert_eq!(cx.theme().background, light_background);
            assert_eq!(
                gpui_kit::base::Theme::global(cx).appearance,
                gpui_kit::base::ThemeAppearance::Light
            );
            assert_eq!(
                Config::load(&config_file).theme,
                Some(ThemePreference::Light)
            );
            std::fs::remove_file(&config_file).unwrap();
            std::fs::remove_dir(config_file.parent().unwrap()).unwrap();
        })
        .unwrap();
    }
}
