use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

use eve_wrench_core::drone_groups::DroneSaveMode;
use eve_wrench_core::{AppData, SettingsEntry, SettingsKind, copy};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::*;
use rust_i18n::t;

use super::title_bar::view_action;
use super::{MainView, Tab};
use crate::ui;

#[derive(Clone, Copy)]
pub(super) enum EditorSyncKind {
    Probes,
    Drones(DroneSaveMode),
}

impl MainView {
    pub(super) fn editor_account(&self, data: &AppData, cx: &App) -> Option<SettingsEntry> {
        let server = self
            .editor_server
            .and_then(|id| data.servers.iter().find(|s| s.info.id == id))
            .or_else(|| data.servers.first())?;
        let profile = self.selected_profile(server)?;
        let selected = self.editor_accounts.get(&profile.path).map(String::as_str);
        let source = self.source_for(SettingsKind::User, cx);
        profile
            .accounts
            .iter()
            .find(|entry| selected == Some(entry.path.as_str()))
            .or_else(|| {
                profile
                    .accounts
                    .iter()
                    .find(|entry| source.as_ref().is_some_and(|s| s.path() == entry.path))
            })
            .or_else(|| {
                let default = self
                    .store
                    .read(cx)
                    .config()
                    .default_sources
                    .path(SettingsKind::User);
                profile
                    .accounts
                    .iter()
                    .find(|entry| default == Some(entry.path.as_str()))
            })
            .or_else(|| {
                profile
                    .accounts
                    .iter()
                    .min_by(|a, b| super::ordering::compare_entry_ids(a, b))
            })
            .cloned()
    }

    pub(super) fn render_editor(
        &mut self,
        tab: Tab,
        data: &Rc<AppData>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(server) = self
            .editor_server
            .and_then(|id| data.servers.iter().find(|s| s.info.id == id))
            .or_else(|| data.servers.first())
        else {
            return div().into_any_element();
        };
        let profile = self.selected_profile(server);
        let entry = self.editor_account(data, cx);
        let server_id = server.info.id;
        let view = cx.entity().downgrade();
        let server_view = view.clone();
        let server_data = data.clone();
        let profiles = server.profiles.clone();
        let profile_view = view.clone();
        let selected_profile = profile.map(|p| p.path.clone());
        let account_view = view;
        let accounts = profile.map(|p| p.accounts.clone()).unwrap_or_default();
        let selected_account = entry.as_ref().map(|e| e.path.clone());
        let controls = h_flex()
            .id("editor-context")
            .test_support()
            .gap_2()
            .p_3()
            .flex_none()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("editor-server")
                    .outline()
                    .small()
                    .label(server.info.id.display_name())
                    .dropdown_menu(move |menu, _, _| {
                        server_data.servers.iter().fold(menu, |menu, server| {
                            let id = server.info.id;
                            menu.item(
                                PopupMenuItem::new(id.display_name())
                                    .checked(id == server_id)
                                    .on_click(view_action(&server_view, move |main, _, cx| {
                                        main.editor_server = Some(id);
                                        cx.notify();
                                    })),
                            )
                        })
                    }),
            )
            .child(
                Button::new("editor-profile")
                    .outline()
                    .small()
                    .label(format!(
                        "{}: {}",
                        t!("profiles.profile"),
                        profile.map(|p| p.name.as_str()).unwrap_or("—")
                    ))
                    .dropdown_menu(move |menu, _, _| {
                        profiles.iter().fold(menu, |menu, profile| {
                            let path = profile.path.clone();
                            menu.item(
                                PopupMenuItem::new(profile.name.clone())
                                    .checked(selected_profile.as_deref() == Some(path.as_str()))
                                    .on_click(view_action(&profile_view, move |main, _, cx| {
                                        main.select_profile(server_id, path.clone(), cx)
                                    })),
                            )
                        })
                    }),
            )
            .child(
                div().flex_1().min_w_0().child(
                    Button::new("editor-account")
                        .outline()
                        .small()
                        .w_full()
                        .label(format!(
                            "{}: {}",
                            t!("defaultSource.account"),
                            entry
                                .as_ref()
                                .map(|e| format!("{} · {}", e.display_name, e.id))
                                .unwrap_or_else(|| t!("editors.noAccount").to_string())
                        ))
                        .disabled(accounts.is_empty())
                        .dropdown_menu(move |menu, _, _| {
                            accounts.iter().fold(
                                menu.max_h(px(280.)).scrollable(true),
                                |menu, entry| {
                                    let entry = entry.clone();
                                    menu.item(
                                        PopupMenuItem::new(format!(
                                            "{} · {}",
                                            entry.display_name, entry.id
                                        ))
                                        .checked(
                                            selected_account.as_deref()
                                                == Some(entry.path.as_str()),
                                        )
                                        .on_click(
                                            view_action(&account_view, move |main, _, cx| {
                                                // The profile path identifies the account selector's scope.
                                                if let Some(data) = main.store.read(cx).data()
                                                    && let Some(profile) = data
                                                        .servers
                                                        .iter()
                                                        .flat_map(|s| &s.profiles)
                                                        .find(|p| {
                                                            p.accounts
                                                                .iter()
                                                                .any(|e| e.path == entry.path)
                                                        })
                                                {
                                                    main.editor_accounts.insert(
                                                        profile.path.clone(),
                                                        entry.path.clone(),
                                                    );
                                                }
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                },
                            )
                        }),
                ),
            );
        let body = if let Some(entry) = entry {
            let path = entry.path.clone();
            if tab == Tab::Probes {
                if !self.probe_editors.contains_key(&path) {
                    let editor = crate::formation_editor::embedded(path.clone(), window, cx);
                    self._subscriptions.push(cx.subscribe_in(
                        &editor,
                        window,
                        |main, _, event: &crate::formation_editor::FormationSaved, window, cx| {
                            main.prompt_editor_sync(
                                event.path.clone(),
                                EditorSyncKind::Probes,
                                window,
                                cx,
                            );
                        },
                    ));
                    self.probe_editors.insert(path.clone(), editor);
                }
                self.probe_editors[&path].clone().into_any_element()
            } else {
                if !self.drone_editors.contains_key(&path) {
                    let editor = self.create_drone_editor(entry, None, window, cx);
                    self.drone_editors.insert(path.clone(), editor);
                }
                self.drone_editors[&path].clone().into_any_element()
            }
        } else {
            div()
                .p_4()
                .child(t!("editors.noAccount").to_string())
                .into_any_element()
        };
        v_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .child(controls)
            .child(body)
            .into_any_element()
    }

    pub(super) fn prompt_editor_sync(
        &mut self,
        source: String,
        kind: EditorSyncKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(data) = self.store.read(cx).data() else {
            return;
        };
        let entries: Vec<_> = data
            .entries()
            .filter(|e| e.kind == SettingsKind::User && e.path != source)
            .cloned()
            .collect();
        if entries.is_empty() {
            return;
        }
        let main = cx.entity().downgrade();
        let editor = cx.new(|_| EditorSyncDialog {
            main,
            source,
            kind,
            entries,
            selected: HashSet::new(),
        });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(match kind {
                    EditorSyncKind::Probes => t!("editors.syncTitle").to_string(),
                    EditorSyncKind::Drones(DroneSaveMode::Append) => {
                        t!("editors.syncAppendTitle").to_string()
                    }
                    EditorSyncKind::Drones(DroneSaveMode::Replace) => {
                        t!("editors.syncReplaceTitle").to_string()
                    }
                })
                .w(px(640.))
                .child(editor.clone())
        });
    }

    fn sync_editor_settings(
        &mut self,
        source: String,
        kind: EditorSyncKind,
        targets: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let backup = self.store.read(cx).config().auto_backup;
        let count = targets.len();
        self.run_job(
            window,
            cx,
            move |_| {
                let copied = match kind {
                    EditorSyncKind::Probes => copy::copy_settings_groups(
                        Path::new(&source),
                        &targets,
                        &["probes"],
                        backup,
                    )?,
                    EditorSyncKind::Drones(mode) => {
                        copy::sync_drone_groups(Path::new(&source), &targets, mode, backup)?
                    }
                };
                if copied != count {
                    return Err(
                        t!("toast.copyIncompleteDesc", copied = copied, total = count).to_string(),
                    );
                }
                Ok(copied)
            },
            |_, result, window, cx| match result {
                Ok(count) => ui::notify_success(
                    t!("toast.settingsCopied"),
                    t!("toast.settingsCopiedDesc", count = count),
                    window,
                    cx,
                ),
                Err(error) => ui::notify_error(t!("toast.copyFailed"), error, window, cx),
            },
        );
    }
}

struct EditorSyncDialog {
    main: WeakEntity<MainView>,
    source: String,
    kind: EditorSyncKind,
    entries: Vec<SettingsEntry>,
    selected: HashSet<String>,
}

impl Render for EditorSyncDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(match self.kind {
                EditorSyncKind::Probes => t!("editors.syncHint").to_string(),
                EditorSyncKind::Drones(DroneSaveMode::Append) => {
                    t!("editors.syncAppendHint").to_string()
                }
                EditorSyncKind::Drones(DroneSaveMode::Replace) => {
                    t!("editors.syncReplaceHint").to_string()
                }
            })
            .child(
                Checkbox::new("sync-all")
                    .label(t!("editors.selectAll").to_string())
                    .checked(self.selected.len() == self.entries.len())
                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                        this.selected = if *checked {
                            this.entries.iter().map(|e| e.path.clone()).collect()
                        } else {
                            HashSet::new()
                        };
                        cx.notify();
                    })),
            )
            .child(
                v_flex()
                    .id("sync-accounts")
                    .max_h(px(260.))
                    .overflow_y_scroll()
                    .children(self.entries.iter().enumerate().map(|(index, entry)| {
                        let path = entry.path.clone();
                        Checkbox::new(SharedString::from(format!("sync-account-{index}")))
                            .label(format!(
                                "{} · {} · {} · {}",
                                entry.display_name,
                                entry.id,
                                entry.server.short_name(),
                                entry.profile
                            ))
                            .checked(self.selected.contains(&path))
                            .on_change(cx.listener(move |this, checked: &bool, _, cx| {
                                if *checked {
                                    this.selected.insert(path.clone());
                                } else {
                                    this.selected.remove(&path);
                                }
                                cx.notify();
                            }))
                    })),
            )
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("sync-cancel")
                            .ghost()
                            .label(t!("common.cancel").to_string())
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("sync-apply")
                            .primary()
                            .label(t!("editors.sync").to_string())
                            .disabled(self.selected.is_empty())
                            .on_click(cx.listener(|this, _, window, cx| {
                                let source = this.source.clone();
                                let kind = this.kind;
                                let targets = this.selected.iter().cloned().collect();
                                window.close_dialog(cx);
                                this.main
                                    .update(cx, |main, cx| {
                                        main.sync_editor_settings(source, kind, targets, window, cx)
                                    })
                                    .ok();
                            })),
                    ),
            )
    }
}
