use std::time::Duration;

use eve_wrench_core::updates::{self, UpdateInfo};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex};
use gpui_kit::*;
use rust_i18n::t;

use super::MainView;
use crate::{ui, updater};

#[derive(Default)]
pub(super) struct UpdateUi {
    pub info: Option<UpdateInfo>,
    pub checking: bool,
    pub installing: bool,
    pub status: Option<String>,
    pub detail: Option<String>,
    pub prepared: Option<updater::PreparedUpdate>,
    pub preview: bool,
    pub install_error: bool,
}

impl MainView {
    pub(super) fn schedule_update_checks(window: &mut Window, cx: &mut Context<Self>) {
        if cfg!(test) {
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            loop {
                if this
                    .update_in(cx, |main, window, cx| {
                        main.check_for_updates(false, window, cx)
                    })
                    .is_err()
                {
                    break;
                }
                cx.background_executor()
                    .timer(Duration::from_secs(6 * 60 * 60))
                    .await;
            }
        })
        .detach();
    }

    pub(super) fn check_for_updates(
        &mut self,
        manual: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.update.checking || self.update.installing || self.update.prepared.is_some() {
            return;
        }
        self.update.checking = true;
        if manual {
            self.update.install_error = false;
            self.update.status = Some(t!("update.checking").to_string());
        }
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async { updates::check_for_update() })
                .await;
            if let Err(error) = &result {
                log::warn!("Update check failed: {error}");
            }
            this.update_in(cx, |main, _, cx| {
                main.update.checking = false;
                match result {
                    Ok(info) => {
                        main.update.info = info;
                        if !main.update.install_error {
                            main.update.detail = None;
                            main.update.status = if manual && main.update.info.is_none() {
                                Some(t!("update.upToDate").to_string())
                            } else {
                                None
                            };
                        }
                    }
                    Err(error) if !main.update.install_error => {
                        main.update.status = Some(t!("update.checkFailed").to_string());
                        main.update.detail = Some(error);
                    }
                    Err(_) => {}
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn update_has_unsaved_edits(&self, cx: &App) -> bool {
        self.probe_editors
            .values()
            .any(|editor| editor.read(cx).is_dirty())
            || self
                .drone_editors
                .values()
                .any(|editor| editor.read(cx).is_dirty(cx))
    }

    fn install_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.update.installing || self.update.checking {
            return;
        }
        if self.copying || self.update_has_unsaved_edits(cx) {
            self.update.status = Some(t!("update.saveEdits").to_string());
            self.update.detail = self.update.status.clone();
            cx.notify();
            return;
        }
        if let Some(prepared) = &self.update.prepared {
            match updater::launch(prepared) {
                Ok(()) => cx.quit(),
                Err(error) => {
                    self.update.install_error = true;
                    self.update.status = Some(t!("update.installFailed").to_string());
                    self.update.detail = Some(error);
                    cx.notify();
                }
            }
            return;
        }
        let Some(info) = &self.update.info else {
            if self.update.preview {
                self.update.status = Some(t!("update.previewOnly").to_string());
                self.update.detail = self.update.status.clone();
                cx.notify();
            } else {
                self.check_for_updates(true, window, cx);
            }
            return;
        };
        let Some(asset) = info.asset.clone() else {
            cx.open_url(&info.release_url);
            return;
        };
        self.update.installing = true;
        self.update.install_error = false;
        self.update.status = Some(t!("update.installing").to_string());
        self.update.detail = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { updater::prepare(&asset) })
                .await;
            this.update_in(cx, |main, window, cx| {
                main.update.installing = false;
                match result {
                    Ok(prepared) => {
                        main.update.prepared = Some(prepared);
                        main.update.status = Some(t!("update.restart").to_string());
                        // Edits made while downloading must be saved before quitting.
                        main.install_update(window, cx);
                    }
                    Err(error) => {
                        main.update.install_error = true;
                        main.update.status = Some(t!("update.installFailed").to_string());
                        main.update.detail = Some(error);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn render_update_indicator(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.update;
        if state.info.is_none() && !state.preview && state.status.is_none() {
            return div().into_any_element();
        }
        let label = state.status.clone().unwrap_or_else(|| {
            state
                .info
                .as_ref()
                .map(|info| {
                    t!("update.availableVersion", version = info.latest_version).to_string()
                })
                .unwrap_or_else(|| t!("update.previewAvailable").to_string())
        });
        let detail = state.detail.clone().unwrap_or_else(|| {
            if state.preview && state.info.is_none() {
                t!("update.previewOnly").to_string()
            } else {
                t!("update.portableHint").to_string()
            }
        });
        let button_label = if state.installing {
            t!("update.installing").to_string()
        } else if state.prepared.is_some() {
            t!("update.restart").to_string()
        } else if state.info.as_ref().is_some_and(|info| info.asset.is_none()) {
            t!("update.releasePage").to_string()
        } else if state.info.is_some() || state.preview {
            t!("update.button").to_string()
        } else {
            t!("update.retry").to_string()
        };
        h_flex()
            .id("update-indicator")
            .test_support()
            .flex_none()
            .gap_2()
            .px_2()
            .child(div().size(px(6.)).rounded_full().bg(ui::target_color()))
            .child(
                div()
                    .max_w(px(190.))
                    .text_xs()
                    .truncate()
                    .text_color(cx.theme().muted_foreground)
                    .child(label),
            )
            .child(
                Button::new("update-button")
                    .outline()
                    .small()
                    .icon(IconName::Download)
                    .label(button_label)
                    .tooltip(detail)
                    .loading(state.installing)
                    .disabled(state.installing || state.checking || self.copying)
                    .on_click(cx.listener(|main, _, window, cx| main.install_update(window, cx))),
            )
            .into_any_element()
    }
}
