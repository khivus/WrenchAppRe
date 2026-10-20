use std::collections::HashSet;
use std::rc::Rc;

use eve_wrench_core::{AppData, ProfileData, Server, ServerData, SettingsEntry};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use rust_i18n::t;

use super::title_bar::view_action;
use super::{MainView, Source};

#[derive(Clone)]
pub(super) enum TargetScope {
    Profile { server: Server, path: String },
    Server(Server),
    All,
}

// Scope buttons deliberately include entries hidden by the row search.
fn additional_targets(
    data: &AppData,
    scope: &TargetScope,
    sources: &[Source],
    selected: &[SettingsEntry],
) -> Vec<SettingsEntry> {
    let mut paths: HashSet<&str> = selected.iter().map(|entry| entry.path.as_str()).collect();
    data.servers
        .iter()
        .filter(|server| match scope {
            TargetScope::Profile { server: id, .. } | TargetScope::Server(id) => {
                *id == server.info.id
            }
            TargetScope::All => true,
        })
        .flat_map(|server| &server.profiles)
        .filter(|profile| match scope {
            TargetScope::Profile { path, .. } => *path == profile.path,
            _ => true,
        })
        .flat_map(|profile| profile.accounts.iter().chain(&profile.characters))
        .filter(|entry| {
            sources
                .iter()
                .any(|source| source.kind() == entry.kind && source.path() != entry.path)
                && paths.insert(entry.path.as_str())
        })
        .cloned()
        .collect()
}

impl MainView {
    pub(super) fn selected_profile<'a>(&self, server: &'a ServerData) -> Option<&'a ProfileData> {
        self.selected_profiles
            .get(&server.info.id)
            .and_then(|path| server.profiles.iter().find(|profile| profile.path == *path))
            .or_else(|| {
                server
                    .profiles
                    .iter()
                    .find(|profile| profile.name == "Default")
            })
            .or_else(|| server.profiles.first())
    }

    pub(super) fn select_profile(&mut self, server: Server, path: String, cx: &mut Context<Self>) {
        let valid = self.store.read(cx).data().is_some_and(|data| {
            data.servers.iter().any(|data| {
                data.info.id == server && data.profiles.iter().any(|profile| profile.path == path)
            })
        });
        if !valid {
            return;
        }
        self.selected_profiles.insert(server, path);
        self.alias_edit = None;
        self.reset_browser_scroll();
        cx.notify();
    }

    fn add_targets_in_scope(&mut self, scope: &TargetScope, cx: &mut Context<Self>) {
        if self.copying {
            return;
        }
        let Some(data) = self.store.read(cx).data() else {
            return;
        };
        self.targets.extend(additional_targets(
            &data,
            scope,
            &self.active_sources(cx),
            &self.targets,
        ));
        cx.notify();
    }

    pub(super) fn render_profile_controls(
        &self,
        server: &ServerData,
        data: &Rc<AppData>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let profile = self.selected_profile(server);
        let selected_path = profile.map(|profile| profile.path.clone());
        let label = profile
            .map(|profile| profile.name.clone())
            .unwrap_or_else(|| t!("profiles.none").to_string());
        let server_id = server.info.id;
        let view = cx.entity().downgrade();
        let data = data.clone();

        v_flex()
            .id("profile-controls")
            .test_support()
            .flex_none()
            .px_4()
            .py_2()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_none()
                            .text_color(cx.theme().muted_foreground)
                            .child(t!("profiles.profile").to_string()),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Button::new("profile-selector")
                                .outline()
                                .small()
                                .w_full()
                                .label(label.clone())
                                .tooltip(label)
                                .icon(IconName::ChevronDown)
                                .disabled(profile.is_none())
                                .dropdown_menu(move |menu, _, _| {
                                    let menu = menu.max_h(px(320.)).scrollable(true);
                                    let Some(server) =
                                        data.servers.iter().find(|s| s.info.id == server_id)
                                    else {
                                        return menu;
                                    };
                                    server.profiles.iter().fold(menu, |menu, profile| {
                                        let path = profile.path.clone();
                                        menu.item(
                                            PopupMenuItem::new(profile.name.clone())
                                                .checked(
                                                    selected_path.as_deref() == Some(path.as_str()),
                                                )
                                                .on_click(view_action(
                                                    &view,
                                                    move |this, _, cx| {
                                                        this.select_profile(
                                                            server_id,
                                                            path.clone(),
                                                            cx,
                                                        );
                                                    },
                                                )),
                                        )
                                    })
                                }),
                        ),
                    )
                    .child(
                        Button::new("profile-copy")
                            .outline()
                            .small()
                            .icon(IconName::Copy)
                            .label(t!("profiles.copy").to_string())
                            .disabled(self.copying || profile.is_none())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.prompt_profile_copy(server_id, window, cx);
                            })),
                    ),
            )
    }

    pub(super) fn render_add_targets(
        &self,
        server: &ServerData,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let profile = self.selected_profile(server);
        let selected_path = profile.map(|profile| profile.path.clone());
        let server_id = server.info.id;
        let enabled = !self.copying && !self.active_sources(cx).is_empty();
        let scopes = [
            (
                "targets-profile",
                t!("profiles.thisProfile").to_string(),
                TargetScope::Profile {
                    server: server_id,
                    path: selected_path.clone().unwrap_or_default(),
                },
                enabled && profile.is_some(),
            ),
            (
                "targets-server",
                t!("profiles.thisServer").to_string(),
                TargetScope::Server(server_id),
                enabled,
            ),
            (
                "targets-all",
                t!("profiles.allServers").to_string(),
                TargetScope::All,
                enabled,
            ),
        ];

        h_flex()
            .id("add-target-controls")
            .test_support()
            .pt_1()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("profiles.addTargets").to_string()),
            )
            .children(scopes.into_iter().map(|(id, label, scope, enabled)| {
                Button::new(id)
                    .outline()
                    .xsmall()
                    .label(label)
                    .tooltip(t!("profiles.targetsHint").to_string())
                    .disabled(!enabled)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.add_targets_in_scope(&scope, cx);
                    }))
            }))
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::search::SearchQuery;
    use super::super::{MainView, Source};
    use super::{TargetScope, additional_targets};
    use crate::store::Store;
    use eve_wrench_core::{
        AppData, ProfileData, Server, ServerData, ServerInfo, SettingsEntry, SettingsKind,
    };
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, WindowOptions};

    pub(crate) fn sample_data() -> AppData {
        AppData {
            servers: [Server::Tranquility, Server::Singularity]
                .into_iter()
                .map(|server| ServerData {
                    info: ServerInfo {
                        id: server,
                        brackets_always_show: false,
                        server_path: server.id().into(),
                    },
                    profiles: ["Alternate", "Default"]
                        .into_iter()
                        .map(|profile| {
                            let entries = |kind| {
                                [2, 10]
                                    .into_iter()
                                    .map(|id| SettingsEntry {
                                        path: format!(
                                            "/{}/{profile}/{kind:?}_{id}.dat",
                                            server.id()
                                        ),
                                        id: id.to_string(),
                                        kind,
                                        server,
                                        profile: profile.into(),
                                        display_name: format!("{kind:?} {id}"),
                                        character: None,
                                        alias: None,
                                        modified_time: 0,
                                    })
                                    .collect()
                            };
                            ProfileData {
                                name: profile.into(),
                                path: format!("/{}/{profile}", server.id()),
                                accounts: entries(SettingsKind::User),
                                characters: entries(SettingsKind::Char),
                            }
                        })
                        .collect(),
                })
                .collect(),
            backups: Vec::new(),
        }
    }

    #[test]
    fn scopes_include_only_their_profiles_and_servers() {
        let data = sample_data();
        let server = &data.servers[0];
        let profile = &server.profiles[1];
        let sources = vec![
            Source::Entry(profile.accounts[0].clone()),
            Source::Entry(profile.characters[0].clone()),
        ];
        let profile_targets = additional_targets(
            &data,
            &TargetScope::Profile {
                server: server.info.id,
                path: profile.path.clone(),
            },
            &sources,
            &[],
        );
        assert_eq!(profile_targets.len(), 2);
        assert!(
            profile_targets
                .iter()
                .all(|entry| entry.server == server.info.id && entry.profile == profile.name)
        );
        let server_targets =
            additional_targets(&data, &TargetScope::Server(server.info.id), &sources, &[]);
        assert_eq!(server_targets.len(), 6);
        assert!(
            server_targets
                .iter()
                .all(|entry| entry.server == server.info.id)
        );
        let all = additional_targets(&data, &TargetScope::All, &sources, &[]);
        assert_eq!(all.len(), 14);
        assert!(all.iter().any(|entry| entry.server == Server::Singularity));
        assert!(
            all.iter()
                .all(|entry| sources.iter().all(|source| source.path() != entry.path))
        );
        assert!(additional_targets(&data, &TargetScope::All, &sources, &all).is_empty());
        assert!(
            additional_targets(
                &data,
                &TargetScope::Profile {
                    server: Server::Singularity,
                    path: profile.path.clone()
                },
                &sources,
                &[]
            )
            .is_empty()
        );
    }

    #[test]
    fn bulk_selection_requires_a_matching_source_and_keeps_existing_targets() {
        let data = sample_data();
        let source = Source::Entry(data.servers[0].profiles[0].characters[0].clone());
        let existing = vec![data.servers[1].profiles[0].characters[0].clone()];
        let added = additional_targets(
            &data,
            &TargetScope::All,
            std::slice::from_ref(&source),
            &existing,
        );
        assert_eq!(added.len(), 6);
        assert!(added.iter().all(|entry| entry.kind == SettingsKind::Char
            && entry.path != source.path()
            && entry.path != existing[0].path));
        assert!(additional_targets(&data, &TargetScope::All, &[], &existing).is_empty());
    }

    #[gpui_kit::test]
    fn scope_actions_include_search_hidden_entries_and_do_nothing_during_copy(
        cx: &mut TestAppContext,
    ) {
        let data = sample_data();
        let source = Source::Entry(data.servers[0].profiles[1].characters[0].clone());
        let profile = data.servers[0].profiles[1].path.clone();
        let (handle, view) = cx.update(|cx| {
            gpui_kit::init(cx);
            Store::install_for_test(data, cx);
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| MainView::new(window, cx))
            })
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            view.update(cx, |view, cx| {
                view.set_source(source, cx);
                view.entry_query = SearchQuery::new("no rows match this search");
                view.add_targets_in_scope(
                    &TargetScope::Profile {
                        server: Server::Tranquility,
                        path: profile,
                    },
                    cx,
                );
                assert_eq!(view.targets.len(), 1);
                view.add_targets_in_scope(&TargetScope::Server(Server::Tranquility), cx);
                assert_eq!(view.targets.len(), 3);
                view.copying = true;
                view.add_targets_in_scope(&TargetScope::All, cx);
                assert_eq!(view.targets.len(), 3);
                view.copying = false;
                view.add_targets_in_scope(&TargetScope::All, cx);
                assert_eq!(view.targets.len(), 7);
                view.add_targets_in_scope(&TargetScope::All, cx);
                assert_eq!(view.targets.len(), 7);
                assert_eq!(view.active_sources(cx).len(), 1);
            });
            window.render_frame(cx);
        })
        .unwrap();
    }
}
