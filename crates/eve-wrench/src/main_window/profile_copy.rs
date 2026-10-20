use std::path::{Path, PathBuf};

use eve_wrench_core::{ProfileData, Server, ServerData, profile_copy};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use rust_i18n::t;

use super::MainView;
use crate::ui;

struct CopyRequest {
    source: PathBuf,
    server: PathBuf,
    name: String,
    backup: bool,
}

struct ProfileCopyDialog {
    main: WeakEntity<MainView>,
    source: ProfileData,
    source_label: String,
    servers: Vec<ServerData>,
    destination_server: PathBuf,
    destination_label: String,
    name_input: Entity<InputState>,
    backup: bool,
    busy: bool,
    error: Option<String>,
    _subscription: Subscription,
}

impl MainView {
    pub(super) fn prompt_profile_copy(
        &mut self,
        server: Server,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.copying {
            return;
        }
        let Some(data) = self.store.read(cx).data() else {
            return;
        };
        let Some(server) = data.servers.iter().find(|s| s.info.id == server) else {
            return;
        };
        let Some(source) = self.selected_profile(server).cloned() else {
            return;
        };
        let destination = data
            .servers
            .iter()
            .find(|s| s.info.id != server.info.id)
            .unwrap_or(server);
        let name = if destination.info.id == server.info.id {
            format!("{} copy", source.name)
        } else {
            source.name.clone()
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let main = cx.entity().downgrade();
        let backup = self.store.read(cx).config().auto_backup;
        let dialog_view = cx.new(|cx: &mut Context<ProfileCopyDialog>| ProfileCopyDialog {
            main,
            source_label: format!("{} / {}", server.info.id.display_name(), source.name),
            source,
            servers: data.servers.clone(),
            destination_server: PathBuf::from(&destination.info.server_path),
            destination_label: destination.info.id.display_name().to_string(),
            backup,
            busy: false,
            error: None,
            _subscription: cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.error = None;
                    cx.notify();
                }
            }),
            name_input: input,
        });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(t!("profileSync.title").to_string())
                .w(px(660.))
                .child(dialog_view.clone())
        });
    }

    fn copy_profile(
        &mut self,
        request: CopyRequest,
        dialog: WeakEntity<ProfileCopyDialog>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.copying = true;
        cx.notify();
        self.run_job(
            window,
            cx,
            move |_| {
                profile_copy::copy_profile(
                    &request.source,
                    &request.server,
                    &request.name,
                    request.backup,
                )
            },
            move |main, result, window, cx| {
                main.copying = false;
                match result {
                    Ok(result) => {
                        window.close_dialog(cx);
                        let mut description = t!(
                            "profileSync.copiedDesc",
                            accounts = result.accounts,
                            characters = result.characters,
                            files = result.files,
                            destination = result.destination.display()
                        )
                        .to_string();
                        if let Some(backup) = result.backup {
                            description.push_str(&format!(
                                "\n{}",
                                t!("profileSync.backupPath", path = backup.display())
                            ));
                        }
                        ui::notify_success(t!("profileSync.copied"), description, window, cx);
                    }
                    Err(error) => {
                        dialog
                            .update(cx, |dialog, cx| {
                                dialog.busy = false;
                                dialog.error = Some(error);
                                cx.notify();
                            })
                            .ok();
                    }
                }
                cx.notify();
            },
        );
    }
}

impl ProfileCopyDialog {
    fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t!("profileSync.chooseFolder").to_string().into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |this, _, cx| {
                this.destination_label = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                this.destination_server = path;
                this.error = None;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let name = self.name_input.read(cx).value().to_string();
        if profile_copy::profile_name(&name).is_err() {
            return;
        }
        self.busy = true;
        self.error = None;
        let request = CopyRequest {
            source: PathBuf::from(&self.source.path),
            server: self.destination_server.clone(),
            name,
            backup: self.backup,
        };
        let dialog = cx.entity().downgrade();
        self.main
            .update(cx, |main, cx| {
                main.copy_profile(request, dialog, window, cx)
            })
            .ok();
        cx.notify();
    }
}

impl Render for ProfileCopyDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let name = self.name_input.read(cx).value();
        let invalid = profile_copy::profile_name(&name).is_err();
        let servers = self.servers.clone();
        let selected = self.destination_server.clone();
        let view = cx.entity().downgrade();
        let existing: Vec<_> = self
            .servers
            .iter()
            .find(|s| Path::new(&s.info.server_path) == selected)
            .map(|s| s.profiles.iter().map(|p| p.name.clone()).collect())
            .unwrap_or_default();
        let input = self.name_input.clone();
        v_flex()
            .gap_3()
            .child(t!("profileSync.source", source = self.source_label).to_string())
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("profileSync.hint").to_string()),
            )
            .child(t!("profileSync.destinationServer").to_string())
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div().flex_1().min_w_0().child(
                            Button::new("profile-copy-server")
                                .outline()
                                .small()
                                .w_full()
                                .label(self.destination_label.clone())
                                .tooltip(self.destination_server.to_string_lossy().into_owned())
                                .disabled(self.busy)
                                .dropdown_menu(move |menu, _, _| {
                                    servers.iter().fold(menu, |menu, server| {
                                        let path = PathBuf::from(&server.info.server_path);
                                        let label = server.info.id.display_name().to_string();
                                        let view = view.clone();
                                        menu.item(
                                            PopupMenuItem::new(label.clone())
                                                .checked(path == selected)
                                                .on_click(move |_, _, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.destination_server = path.clone();
                                                        this.destination_label = label.clone();
                                                        this.error = None;
                                                        cx.notify();
                                                    })
                                                    .ok();
                                                }),
                                        )
                                    })
                                }),
                        ),
                    )
                    .child(
                        Button::new("profile-copy-folder")
                            .outline()
                            .small()
                            .icon(IconName::Folder)
                            .label(t!("profileSync.chooseFolder").to_string())
                            .disabled(self.busy)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.choose_folder(window, cx)),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .justify_between()
                    .child(t!("profileSync.name").to_string())
                    .child(
                        Button::new("profile-copy-existing")
                            .ghost()
                            .xsmall()
                            .label(t!("profileSync.existing").to_string())
                            .disabled(self.busy || existing.is_empty())
                            .dropdown_menu(move |menu, _, _| {
                                existing.iter().fold(menu, |menu, name| {
                                    let name = name.clone();
                                    let input = input.clone();
                                    menu.item(PopupMenuItem::new(name.clone()).on_click(
                                        move |_, window, cx| {
                                            input.update(cx, |state, cx| {
                                                state.set_value(name.clone(), window, cx)
                                            });
                                        },
                                    ))
                                })
                            }),
                    ),
            )
            .child(
                div()
                    .id("profile-copy-name")
                    .test_support()
                    .child(Input::new(&self.name_input).small().disabled(self.busy)),
            )
            .when(invalid, |this| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(t!("profileSync.invalidName").to_string()),
                )
            })
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.backup {
                        t!("profileSync.backupOn").to_string()
                    } else {
                        t!("profileSync.backupOff").to_string()
                    }),
            )
            .when_some(self.error.clone(), |this, error| {
                this.child(div().text_sm().text_color(cx.theme().danger).child(error))
            })
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("profile-copy-cancel")
                            .ghost()
                            .label(t!("common.cancel").to_string())
                            .disabled(self.busy)
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("profile-copy-apply")
                            .primary()
                            .label(t!("profileSync.copy").to_string())
                            .loading(self.busy)
                            .disabled(self.busy || invalid)
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    ),
            )
    }
}
