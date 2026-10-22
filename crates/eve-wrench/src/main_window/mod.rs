mod actions;
mod browser;
mod copy_panel;
mod copy_plan;
mod default_sources;
mod dialogs;
mod drone_groups;
mod editors;
#[cfg(test)]
mod editors_tests;
mod entry_columns;
mod ordering;
mod profile_copy;
mod profiles;
mod scroll;
mod search;
mod settings_panel;
mod targets;
mod title_bar;
mod updating;

use std::collections::{HashMap, HashSet};

use eve_wrench_core::{BackupEntry, Server, SettingsEntry, SettingsKind};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::ui::StyleExt as _;
use rust_i18n::t;

use crate::store::{Store, StoreEvent};
use crate::ui;
use search::SearchQuery;

pub fn open(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1440.), px(800.)), cx)),
        window_min_size: Some(size(px(1200.), px(600.))),
        app_id: Some("eve-wrench".into()),
        window_decorations: cfg!(target_os = "linux").then_some(WindowDecorations::Client),
        ..TitleBar::window_options()
    };
    gpui_kit::open_window(options, cx, |window, cx| {
        window.set_window_title("EVE Wrench");
        cx.new(|cx| MainView::new(window, cx))
    })
    .expect("failed to open the main window");
}

#[derive(Clone, PartialEq)]
pub enum Source {
    Entry(SettingsEntry),
    Backup(BackupEntry),
}

impl Source {
    pub fn kind(&self) -> SettingsKind {
        match self {
            Source::Entry(e) => e.kind,
            Source::Backup(b) => b.kind,
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Source::Entry(e) => &e.path,
            Source::Backup(b) => &b.path,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Source::Entry(e) => &e.display_name,
            Source::Backup(b) => &b.name,
        }
    }
}

// Payload while a row is dragged onto the copy panel.
#[derive(Clone)]
pub struct DraggedSettings(pub Source);

pub struct DragPreview {
    // Where the drag started inside the row; GPUI anchors the preview there,
    // so it's shifted by this much to sit at the pointer instead
    grab: Point<Pixels>,
    kind: SettingsKind,
    name: SharedString,
    portrait: Option<std::sync::Arc<Image>>,
}

impl DragPreview {
    pub fn new(
        source: &Source,
        grab: Point<Pixels>,
        portrait: Option<std::sync::Arc<Image>>,
    ) -> Self {
        Self {
            grab,
            kind: source.kind(),
            name: source.display_name().to_string().into(),
            portrait,
        }
    }
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pill = h_flex()
            .gap_2()
            .pl_1p5()
            .pr_3()
            .py_1()
            .rounded(px(6.))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .shadow_md()
            .text_sm()
            .text_color(cx.theme().popover_foreground)
            .child(ui::entry_avatar(
                self.kind,
                self.portrait.clone(),
                px(20.),
                cx,
            ))
            .child(self.name.clone());
        div()
            .pl(self.grab.x + px(8.))
            .pt(self.grab.y + px(8.))
            .child(pill)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SortColumn {
    Id,
    Name,
    Time,
    Corporation,
    Alliance,
    CreationDate,
}

#[derive(Clone, Copy, PartialEq)]
struct Sort {
    column: SortColumn,
    ascending: bool,
}

impl Sort {
    // Clicking the active column flips direction; a new column starts
    // ascending, except dates when `newest_first` is set.
    fn toggle(&mut self, column: SortColumn, newest_first: bool) {
        if self.column == column {
            self.ascending = !self.ascending;
        } else {
            self.column = column;
            self.ascending = !(newest_first && column == SortColumn::Time);
        }
    }

    fn apply(self, ordering: std::cmp::Ordering) -> std::cmp::Ordering {
        if self.ascending {
            ordering
        } else {
            ordering.reverse()
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Server(Server),
    Backups,
    Probes,
    Drones,
}

struct AliasEdit {
    id: String,
    input: Entity<InputState>,
    _subscription: Subscription,
}

pub struct MainView {
    update: updating::UpdateUi,
    store: Entity<Store>,
    tab: Option<Tab>,
    selected_profiles: HashMap<Server, String>,
    source: Option<Source>,
    other_source: Option<Source>,
    editor_server: Option<Server>,
    editor_accounts: HashMap<String, String>,
    probe_editors: HashMap<String, Entity<crate::formation_editor::FormationEditor>>,
    drone_editors: HashMap<String, Entity<drone_groups::DroneGroupsEditor>>,
    using_default_sources: bool,
    targets: targets::Targets,
    target_scroll: UniformListScrollHandle,
    #[cfg(test)]
    target_rows_built: usize,
    copying: bool,
    account_sort: Sort,
    character_sort: Sort,
    backup_sort: Sort,
    selected_backups: HashSet<String>,
    alias_edit: Option<AliasEdit>,
    announce_refresh: bool,
    refresh_status: Option<String>,
    #[cfg(test)]
    reload_requests: usize,
    account_scroll: scroll::TableScroll,
    character_scroll: scroll::TableScroll,
    backup_scroll: scroll::TableScroll,
    browser_rows: Option<std::rc::Rc<browser::BrowserRows>>,
    entry_search: Entity<InputState>,
    entry_query: SearchQuery,
    settings_panel: Entity<settings_panel::SettingsPanel>,
    _dialog_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}

impl MainView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::global(cx);
        let using_default_sources = store.read(cx).config().default_sources.has_any();
        let this = cx.weak_entity();
        let settings_panel = cx.new(|cx| settings_panel::SettingsPanel::new(this, cx));
        let entry_search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("list.searchEntries").to_string())
        });
        let subscriptions = vec![
            cx.subscribe_in(&entry_search, window, |this, input, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.entry_query = SearchQuery::new(&input.read(cx).value());
                    this.alias_edit = None;
                    this.reset_browser_scroll();
                    cx.notify();
                }
            }),
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.observe_global::<gpui_kit::component::Theme>(|_, cx| cx.notify()),
            cx.subscribe_in(&store, window, |this, _, event, window, cx| match event {
                StoreEvent::DataChanged => this.on_data_changed(window, cx),
                StoreEvent::LoadFailed(error) => {
                    this.announce_refresh = false;
                    ui::notify_error(t!("toast.loadDataFailed"), error.clone(), window, cx);
                }
            }),
        ];

        // A newer drone catalog only matters the next time the picker opens,
        // so failures stay silent and the built-in one keeps working
        if !cfg!(test) {
            let data_dir = store.read(cx).locations().data_dir().to_path_buf();
            cx.background_spawn(async move {
                eve_wrench_core::drone_groups::load_cached_catalog(&data_dir);
                if let Err(e) = eve_wrench_core::drone_groups::refresh_catalog(&data_dir) {
                    log::warn!("Drone catalog refresh failed: {e}");
                }
            })
            .detach();
        }

        Self::schedule_update_checks(window, cx);

        let id_sort = Sort {
            column: SortColumn::Id,
            ascending: true,
        };
        let update_error = crate::updater::startup_error();
        Self {
            update: updating::UpdateUi {
                preview: cfg!(feature = "update-preview"),
                status: update_error
                    .as_ref()
                    .map(|_| t!("update.installFailed").to_string()),
                install_error: update_error.is_some(),
                detail: update_error,
                ..Default::default()
            },
            store,
            tab: None,
            selected_profiles: HashMap::new(),
            source: None,
            other_source: None,
            editor_server: None,
            editor_accounts: HashMap::new(),
            probe_editors: HashMap::new(),
            drone_editors: HashMap::new(),
            using_default_sources,
            targets: targets::Targets::default(),
            target_scroll: UniformListScrollHandle::new(),
            #[cfg(test)]
            target_rows_built: 0,
            copying: false,
            account_sort: id_sort,
            character_sort: id_sort,
            backup_sort: Sort {
                column: SortColumn::Time,
                ascending: false,
            },
            selected_backups: HashSet::new(),
            alias_edit: None,
            account_scroll: scroll::TableScroll::default(),
            character_scroll: scroll::TableScroll::default(),
            backup_scroll: scroll::TableScroll::default(),
            browser_rows: None,
            entry_search,
            entry_query: SearchQuery::default(),
            settings_panel,
            announce_refresh: false,
            refresh_status: None,
            #[cfg(test)]
            reload_requests: 0,
            _dialog_subscription: None,
            _subscriptions: subscriptions,
        }
    }

    // Keeps the selection pointing at current data: entries are refreshed
    // (new names, portraits) and anything deleted on disk is dropped.
    fn on_data_changed(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(data) = self.store.read(cx).data() else {
            return;
        };
        self.source = match self.source.take() {
            Some(Source::Entry(e)) => data.find_entry(&e.path).cloned().map(Source::Entry),
            Some(Source::Backup(b)) => data
                .backups
                .iter()
                .find(|x| x.path == b.path)
                .cloned()
                .map(Source::Backup),
            None => None,
        };
        self.other_source = self.other_source.take().and_then(|source| match source {
            Source::Entry(entry) => data.find_entry(&entry.path).cloned().map(Source::Entry),
            Source::Backup(_) => None,
        });
        self.targets = self
            .targets
            .iter()
            .filter_map(|t| data.find_entry(&t.path).cloned())
            .collect();
        let backup_ids: HashSet<&str> = data.backups.iter().map(|b| b.id.as_str()).collect();
        self.selected_backups
            .retain(|id| backup_ids.contains(id.as_str()));
        self.selected_profiles.retain(|server, path| {
            data.servers.iter().any(|data| {
                data.info.id == *server && data.profiles.iter().any(|profile| profile.path == *path)
            })
        });

        let tab_exists = match self.tab {
            Some(Tab::Server(server)) => data.servers.iter().any(|s| s.info.id == server),
            Some(Tab::Backups) => true,
            Some(Tab::Probes | Tab::Drones) => true,
            None => false,
        };
        if !tab_exists {
            self.tab = data.servers.first().map(|s| Tab::Server(s.info.id));
        }

        if std::mem::take(&mut self.announce_refresh) && !data.is_empty() {
            let message = t!(
                "titleBar.refreshedCounts",
                servers = data.servers.len(),
                backups = data.backups.len()
            );
            self.refresh_status = Some(message.to_string());
        }
        cx.notify();
    }

    fn render_loading(&self, cx: &App) -> impl IntoElement + use<> {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap_3()
            .child(Spinner::new().large())
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("common.loading").to_string()),
            )
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap_3()
            .text_center()
            .child(
                Icon::new(IconName::Rocket)
                    .size(px(48.))
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .font_semibold()
                    .child(t!("empty.noEveInstallations").to_string()),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("empty.noEveInstallationsDesc").to_string()),
            )
            .child(
                Button::new("set-custom-path")
                    .outline()
                    .small()
                    .mt_2()
                    .icon(IconName::FolderOpen)
                    .label(t!("settings.setCustomPath").to_string())
                    .on_click(
                        cx.listener(|this, _, window, cx| this.select_custom_eve_path(window, cx)),
                    ),
            )
    }
}

impl Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let navigation_data = self.store.read(cx).data();
        let body = match self.store.read(cx).data().map(|d| d.is_empty()) {
            None => self.render_loading(cx).into_any_element(),
            Some(true) => self.render_empty(cx).into_any_element(),
            Some(false) => h_flex()
                .flex_1()
                .items_stretch()
                .overflow_hidden()
                .child(self.render_browser(window, cx))
                .when(
                    !matches!(self.tab, Some(Tab::Probes | Tab::Drones)),
                    |this| this.child(self.render_copy_panel(cx)),
                )
                .into_any_element(),
        };

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_title_bar(window, cx))
            .children(
                navigation_data
                    .filter(|data| !data.is_empty())
                    .map(|data| self.render_navigation(&data, cx)),
            )
            .when(
                self.store
                    .read(cx)
                    .data()
                    .is_none_or(|data| data.is_empty()),
                |this| this.child(h_flex().justify_end().child(self.render_app_controls(cx))),
            )
            .child(body)
    }
}
