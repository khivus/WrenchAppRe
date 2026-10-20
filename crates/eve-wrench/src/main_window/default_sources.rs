use eve_wrench_core::{AppData, ServerData, SettingsEntry, SettingsKind};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use rust_i18n::t;

use super::ordering::compare_entry_ids;
use super::title_bar::view_action;
use super::{MainView, Source};
use crate::ui;
use std::rc::Rc;

impl MainView {
    pub(super) fn active_sources(&self, cx: &App) -> Vec<Source> {
        if !self.using_default_sources {
            return self
                .source
                .iter()
                .chain(&self.other_source)
                .cloned()
                .collect();
        }
        let store = self.store.read(cx);
        let Some(data) = store.data() else {
            return Vec::new();
        };
        [SettingsKind::Char, SettingsKind::User]
            .into_iter()
            .filter_map(|kind| store.config().default_sources.resolve(&data, kind))
            .cloned()
            .map(Source::Entry)
            .collect()
    }

    pub(super) fn source_for(&self, kind: SettingsKind, cx: &App) -> Option<Source> {
        if !self.using_default_sources {
            return self
                .source
                .as_ref()
                .filter(|source| source.kind() == kind)
                .or_else(|| {
                    self.other_source
                        .as_ref()
                        .filter(|source| source.kind() == kind)
                })
                .cloned();
        }
        let store = self.store.read(cx);
        let data = store.data()?;
        store
            .config()
            .default_sources
            .resolve(&data, kind)
            .cloned()
            .map(Source::Entry)
    }

    pub(super) fn activate_default_sources(&mut self, cx: &mut Context<Self>) {
        if self.copying {
            return;
        }
        self.using_default_sources = self.store.read(cx).config().default_sources.has_any();
        self.source = None;
        self.other_source = None;
        let sources = self.active_sources(cx);
        self.targets.retain(|target| {
            sources
                .iter()
                .any(|source| source.kind() == target.kind && source.path() != target.path)
        });
        cx.notify();
    }

    fn select_default_source(
        &mut self,
        kind: SettingsKind,
        path: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.copying {
            return;
        }
        if let Some(path) = &path {
            let valid = self.store.read(cx).data().is_some_and(|data| {
                data.find_entry(path)
                    .is_some_and(|entry| entry.kind == kind)
            });
            if !valid {
                return;
            }
        }
        let result = self.store.update(cx, |store, cx| {
            store.update_config(cx, |config| config.default_sources.set_path(kind, path))
        });
        match result {
            Ok(()) => self.activate_default_sources(cx),
            Err(error) => ui::notify_error(t!("defaultSource.saveFailed"), error, window, cx),
        }
    }

    pub(super) fn render_default_sources(
        &self,
        data: &Rc<AppData>,
        server: &ServerData,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let has_default = self.store.read(cx).config().default_sources.has_any();
        v_flex()
            .id("default-sources-controls")
            .test_support()
            .flex_none()
            .px_4()
            .pt_2()
            .gap_1()
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t!("defaultSource.title").to_string()),
                    )
                    .when(has_default && !self.using_default_sources, |this| {
                        this.child(
                            Button::new("use-default-sources")
                                .ghost()
                                .xsmall()
                                .label(t!("defaultSource.useDefault").to_string())
                                .disabled(self.copying)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.activate_default_sources(cx)),
                                ),
                        )
                    }),
            )
            .child(
                h_flex()
                    .id("default-source-pickers")
                    .test_support()
                    .gap_2()
                    .child(self.default_source_picker(SettingsKind::Char, data, cx))
                    .child(self.default_source_picker(SettingsKind::User, data, cx)),
            )
            .child(self.render_add_targets(server, cx))
    }

    fn default_source_picker(
        &self,
        kind: SettingsKind,
        data: &Rc<AppData>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let selected = self
            .store
            .read(cx)
            .config()
            .default_sources
            .path(kind)
            .map(str::to_string);
        let kind_label = match kind {
            SettingsKind::Char => t!("defaultSource.character"),
            SettingsKind::User => t!("defaultSource.account"),
        };
        let selected_label = selected
            .as_ref()
            .and_then(|path| data.find_entry(path))
            .map(entry_label)
            .unwrap_or_else(|| {
                if selected.is_some() {
                    t!("defaultSource.unavailable").to_string()
                } else {
                    t!("defaultSource.choose").to_string()
                }
            });
        let view = cx.entity().downgrade();
        let data = data.clone();
        div().flex_1().min_w_0().child(
            Button::new(match kind {
                SettingsKind::Char => "default-character",
                SettingsKind::User => "default-account",
            })
            .outline()
            .small()
            .w_full()
            .label(format!("{kind_label}: {selected_label}"))
            .tooltip(format!("{kind_label}: {selected_label}"))
            .icon(IconName::ChevronDown)
            .disabled(self.copying)
            .dropdown_menu(move |menu, _, _| {
                // Build choices when the menu opens, not on every window repaint.
                let mut entries: Vec<_> =
                    data.entries().filter(|entry| entry.kind == kind).collect();
                entries.sort_by(|a, b| compare_entry_ids(a, b));
                let menu = menu.max_h(px(320.)).scrollable(true).item(
                    PopupMenuItem::new(t!("defaultSource.none").to_string())
                        .checked(selected.is_none())
                        .on_click(view_action(&view, move |this, window, cx| {
                            this.select_default_source(kind, None, window, cx)
                        })),
                );
                entries.iter().fold(menu, |menu, entry| {
                    let path = entry.path.clone();
                    menu.item(
                        PopupMenuItem::new(entry_label(entry))
                            .checked(selected.as_deref() == Some(entry.path.as_str()))
                            .on_click(view_action(&view, move |this, window, cx| {
                                this.select_default_source(kind, Some(path.clone()), window, cx)
                            })),
                    )
                })
            }),
        )
    }
}

fn entry_label(entry: &SettingsEntry) -> String {
    format!(
        "{} · {} · {} · {}",
        entry.display_name,
        entry.id,
        entry.server.short_name(),
        entry.profile
    )
}
