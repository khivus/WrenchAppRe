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
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use rust_i18n::t;

use super::title_bar::view_action;
use super::{MainView, Tab};
use crate::ui;

#[derive(Clone, Copy)]
pub(super) enum EditorSyncKind {
    Probes,
    Drones(DroneSaveMode),
    Confirmations,
    WindowLayout,
}

impl EditorSyncKind {
    fn settings_kind(self) -> SettingsKind {
        match self {
            Self::WindowLayout => SettingsKind::Char,
            _ => SettingsKind::User,
        }
    }
}

impl MainView {
    pub(super) fn editor_account(&self, data: &AppData, cx: &App) -> Option<SettingsEntry> {
        self.editor_entry(data, SettingsKind::User, cx)
    }

    pub(super) fn editor_entry(
        &self,
        data: &AppData,
        kind: SettingsKind,
        cx: &App,
    ) -> Option<SettingsEntry> {
        let server = self
            .editor_server
            .and_then(|id| data.servers.iter().find(|s| s.info.id == id))
            .or_else(|| data.servers.first())?;
        let profile = self.selected_profile(server)?;
        let (entries, selections) = match kind {
            SettingsKind::User => (&profile.accounts, &self.editor_accounts),
            SettingsKind::Char => (&profile.characters, &self.editor_characters),
        };
        let selected = selections.get(&profile.path).map(String::as_str);
        let source = self.source_for(kind, cx);
        entries
            .iter()
            .find(|entry| selected == Some(entry.path.as_str()))
            .or_else(|| {
                entries
                    .iter()
                    .find(|entry| source.as_ref().is_some_and(|s| s.path() == entry.path))
            })
            .or_else(|| {
                let default = self.store.read(cx).config().default_sources.path(kind);
                entries
                    .iter()
                    .find(|entry| default == Some(entry.path.as_str()))
            })
            .or_else(|| {
                entries
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
        let kind = if tab == Tab::WindowLayout {
            SettingsKind::Char
        } else {
            SettingsKind::User
        };
        let entry = if kind == SettingsKind::User {
            self.editor_account(data, cx)
        } else {
            self.editor_entry(data, kind, cx)
        };
        let server_id = server.info.id;
        let view = cx.entity().downgrade();
        let server_view = view.clone();
        let server_data = data.clone();
        let profiles = server.profiles.clone();
        let profile_view = view.clone();
        let selected_profile = profile.map(|p| p.path.clone());
        let account_view = view;
        let accounts = profile
            .map(|p| match kind {
                SettingsKind::User => p.accounts.clone(),
                SettingsKind::Char => p.characters.clone(),
            })
            .unwrap_or_default();
        let selected_account = entry.as_ref().map(|e| e.path.clone());
        let layout_account = self.editor_account(data, cx);
        let layout_accounts = profile.map(|p| p.accounts.clone()).unwrap_or_default();
        let layout_account_view = cx.entity().downgrade();
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
                    Button::new(if kind == SettingsKind::User {
                        "editor-account"
                    } else {
                        "editor-character"
                    })
                    .outline()
                    .small()
                    .w_full()
                    .label(format!(
                        "{}: {}",
                        if kind == SettingsKind::User {
                            t!("defaultSource.account")
                        } else {
                            t!("defaultSource.character")
                        },
                        entry
                            .as_ref()
                            .map(|e| format!("{} · {}", e.display_name, e.id))
                            .unwrap_or_else(|| if kind == SettingsKind::User {
                                t!("editors.noAccount").to_string()
                            } else {
                                t!("editors.noCharacter").to_string()
                            })
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
                                        selected_account.as_deref() == Some(entry.path.as_str()),
                                    )
                                    .on_click(view_action(
                                        &account_view,
                                        move |main, _, cx| {
                                            // The profile path identifies the account selector's scope.
                                            if let Some(data) = main.store.read(cx).data()
                                                && let Some(profile) = data
                                                    .servers
                                                    .iter()
                                                    .flat_map(|s| &s.profiles)
                                                    .find(|p| {
                                                        p.accounts
                                                            .iter()
                                                            .chain(&p.characters)
                                                            .any(|e| e.path == entry.path)
                                                    })
                                            {
                                                let selections = if kind == SettingsKind::User {
                                                    &mut main.editor_accounts
                                                } else {
                                                    &mut main.editor_characters
                                                };
                                                selections.insert(
                                                    profile.path.clone(),
                                                    entry.path.clone(),
                                                );
                                            }
                                            cx.notify();
                                        },
                                    )),
                                )
                            },
                        )
                    }),
                ),
            )
            .when(tab == Tab::WindowLayout, |this| {
                this.child(
                    div().flex_1().min_w_0().child(
                        Button::new("editor-account")
                            .outline()
                            .small()
                            .w_full()
                            .label(format!(
                                "{}: {}",
                                t!("defaultSource.account"),
                                layout_account
                                    .as_ref()
                                    .map(|e| format!("{} · {}", e.display_name, e.id))
                                    .unwrap_or_else(|| t!("editors.noAccount").to_string())
                            ))
                            .disabled(layout_accounts.is_empty())
                            .dropdown_menu(move |menu, _, _| {
                                layout_accounts.iter().fold(
                                    menu.max_h(px(280.)).scrollable(true),
                                    |menu, entry| {
                                        let path = entry.path.clone();
                                        let current =
                                            layout_account.as_ref().is_some_and(|e| e.path == path);
                                        menu.item(
                                            PopupMenuItem::new(format!(
                                                "{} · {}",
                                                entry.display_name, entry.id
                                            ))
                                            .checked(current)
                                            .on_click(view_action(
                                                &layout_account_view,
                                                move |main, _, cx| {
                                                    if let Some(profile) =
                                                        main.store.read(cx).data().and_then(|d| {
                                                            d.servers
                                                                .iter()
                                                                .flat_map(|s| &s.profiles)
                                                                .find(|p| {
                                                                    p.accounts
                                                                        .iter()
                                                                        .any(|e| e.path == path)
                                                                })
                                                                .map(|p| p.path.clone())
                                                        })
                                                    {
                                                        main.editor_accounts
                                                            .insert(profile, path.clone());
                                                    }
                                                    cx.notify();
                                                },
                                            )),
                                        )
                                    },
                                )
                            }),
                    ),
                )
            });
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
            } else if tab == Tab::Drones {
                if !self.drone_editors.contains_key(&path) {
                    let editor = self.create_drone_editor(entry, None, window, cx);
                    self.drone_editors.insert(path.clone(), editor);
                }
                self.drone_editors[&path].clone().into_any_element()
            } else if tab == Tab::Confirmations {
                if !self.confirmation_editors.contains_key(&path) {
                    let editor = super::confirmations::embedded(path.clone(), window, cx);
                    self._subscriptions.push(cx.subscribe_in(
                        &editor,
                        window,
                        |main, _, event: &super::confirmations::ConfirmationsSaved, window, cx| {
                            main.prompt_editor_sync(
                                event.path.clone(),
                                EditorSyncKind::Confirmations,
                                window,
                                cx,
                            );
                        },
                    ));
                    self.confirmation_editors.insert(path.clone(), editor);
                }
                self.confirmation_editors[&path].clone().into_any_element()
            } else {
                if !self.layout_editors.contains_key(&path) {
                    let editor = crate::layout_editor::embedded(path.clone(), window, cx);
                    self._subscriptions.push(cx.subscribe_in(
                        &editor,
                        window,
                        |main, _, event: &crate::layout_editor::LayoutSaved, window, cx| {
                            main.prompt_editor_sync(
                                event.path.clone(),
                                EditorSyncKind::WindowLayout,
                                window,
                                cx,
                            );
                        },
                    ));
                    self.layout_editors.insert(path.clone(), editor);
                }
                let editor = self.layout_editors[&path].clone();
                if let Some(account) = self.editor_account(data, cx) {
                    editor.update(cx, |editor, cx| {
                        editor.select_account(account.path.into(), window, cx)
                    });
                }
                editor.into_any_element()
            }
        } else {
            div()
                .p_4()
                .child(if kind == SettingsKind::User {
                    t!("editors.noAccount").to_string()
                } else {
                    t!("editors.noCharacter").to_string()
                })
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
            .filter(|e| e.kind == kind.settings_kind() && e.path != source)
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
            scroll: crate::scrolling::SmoothScroll::default(),
        });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(match kind {
                    EditorSyncKind::Probes => t!("editors.syncTitle").to_string(),
                    EditorSyncKind::Confirmations => {
                        t!("editors.syncConfirmationsTitle").to_string()
                    }
                    EditorSyncKind::WindowLayout => t!("editors.syncLayoutTitle").to_string(),
                    EditorSyncKind::Drones(DroneSaveMode::Append) => {
                        t!("editors.syncAppendTitle").to_string()
                    }
                    EditorSyncKind::Drones(DroneSaveMode::Update) => {
                        t!("editors.syncUpdateTitle").to_string()
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
                    EditorSyncKind::Confirmations => copy::copy_settings_groups(
                        Path::new(&source),
                        &targets,
                        &["suppress"],
                        backup,
                    )?,
                    EditorSyncKind::WindowLayout => copy::copy_settings_groups(
                        Path::new(&source),
                        &targets,
                        &["windows"],
                        backup,
                    )?,
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
    scroll: crate::scrolling::SmoothScroll,
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
                EditorSyncKind::Confirmations => t!("editors.syncConfirmationsHint").to_string(),
                EditorSyncKind::WindowLayout => t!("editors.syncLayoutHint").to_string(),
                EditorSyncKind::Drones(DroneSaveMode::Append) => {
                    t!("editors.syncAppendHint").to_string()
                }
                EditorSyncKind::Drones(DroneSaveMode::Update) => {
                    t!("editors.syncUpdateHint").to_string()
                }
                EditorSyncKind::Drones(DroneSaveMode::Replace) => {
                    t!("editors.syncReplaceHint").to_string()
                }
            })
            .child(
                Checkbox::new("sync-all")
                    .label(if matches!(self.kind, EditorSyncKind::WindowLayout) {
                        t!("editors.selectAllCharacters").to_string()
                    } else {
                        t!("editors.selectAll").to_string()
                    })
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
                self.scroll
                    .view(
                        "sync-accounts",
                        v_flex().children(self.entries.iter().enumerate().map(|(index, entry)| {
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
                        |this| &mut this.scroll,
                        cx,
                    )
                    .max_h(px(260.)),
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
                            .label(if matches!(self.kind, EditorSyncKind::WindowLayout) {
                                t!("editors.syncCharacters").to_string()
                            } else {
                                t!("editors.sync").to_string()
                            })
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
