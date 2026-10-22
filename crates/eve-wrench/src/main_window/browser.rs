use std::rc::Rc;

use eve_wrench_core::{
    AppData, BackupEntry, ProfileData, Server, ServerData, SettingsEntry, SettingsKind,
};
use gpui_kit::assets::IconName;
use gpui_kit::base::{Scrollbar, ScrollbarMode};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use rust_i18n::t;

use super::entry_columns::{EntryTableLayout, column_id, column_label, entry_column_text};
use super::ordering::compare_entries;
use super::scroll::{TableKind, visible_rows};
use super::title_bar::view_action;
use super::{DragPreview, DraggedSettings, MainView, Sort, SortColumn, Source, Tab};
use crate::table::{self, Column};
use crate::ui::{self, StyleExt as _};

// Selection, avatar, name, date, menu
const BACKUP_COLUMNS: [Column; 5] = [
    Column::fixed(px(32.)),
    Column::fixed(px(36.)),
    Column::fill(),
    Column::fixed(px(112.)),
    Column::fixed(px(40.)).end(),
];

// Per-render labels, looked up once instead of once per row.
struct RowLabels {
    source: SharedString,
    target: SharedString,
    source_path: Option<String>,
}

// Data and search changes rebuild ordering; scrolling and moving the window reuse it.
pub(super) struct BrowserRows {
    data: Rc<AppData>,
    server: Server,
    profile_path: Option<String>,
    query: super::search::SearchQuery,
    account_sort: Sort,
    character_sort: Sort,
    locale: String,
    minute: u64,
    sections: Vec<(usize, SettingsKind, Vec<usize>)>,
}

impl MainView {
    pub(super) fn render_navigation(
        &self,
        data: &AppData,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let mut tabs: Vec<_> = data
            .servers
            .iter()
            .map(|s| Tab::Server(s.info.id))
            .collect();
        tabs.extend([Tab::Backups, Tab::Probes, Tab::Drones]);
        let active = self
            .tab
            .filter(|tab| tabs.contains(tab))
            .or(tabs.first().copied());
        self.render_tab_strip(&tabs, active, data, cx)
    }
    pub(super) fn entry_matches_search(&self, entry: &SettingsEntry) -> bool {
        if self.entry_query.is_empty() {
            return true;
        }
        let kind = match entry.kind {
            SettingsKind::User => t!("list.accounts"),
            SettingsKind::Char => t!("list.characters"),
        };
        self.entry_query
            .matches(entry, &kind, &ui::relative_time(entry.modified_time))
    }

    pub(super) fn render_browser(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let Some(data) = self.store.read(cx).data() else {
            return div().flex_1();
        };

        let mut tabs: Vec<Tab> = data
            .servers
            .iter()
            .map(|s| Tab::Server(s.info.id))
            .collect();
        tabs.extend([Tab::Backups, Tab::Probes, Tab::Drones]);
        let active = self
            .tab
            .filter(|t| tabs.contains(t))
            .or(tabs.first().copied());

        let content = match active {
            Some(Tab::Server(server)) => data
                .servers
                .iter()
                .find(|s| s.info.id == server)
                .map(|s| self.render_server(s, &data, window, cx)),
            Some(Tab::Backups) => Some(self.render_backups(&data, window, cx).into_any_element()),
            Some(tab @ (Tab::Probes | Tab::Drones)) => {
                Some(self.render_editor(tab, &data, window, cx))
            }
            None => None,
        };

        v_flex()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .text_sm()
            .when_some(active, |this, active| match active {
                Tab::Server(server) => this.children(
                    data.servers
                        .iter()
                        .find(|s| s.info.id == server)
                        .map(|server| self.render_profile_controls(server, &data, cx)),
                ),
                _ => this,
            })
            .when_some(active, |this, active| match active {
                Tab::Server(server) => this.children(
                    data.servers
                        .iter()
                        .find(|s| s.info.id == server)
                        .map(|server| self.render_default_sources(&data, server, cx)),
                ),
                _ => this,
            })
            .when(matches!(active, Some(Tab::Server(_))), |this| {
                this.child(
                    h_flex()
                        .flex_none()
                        .px_4()
                        .py_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            Input::new(&self.entry_search)
                                .small()
                                .prefix(Icon::new(IconName::Search).xsmall())
                                .cleanable(true),
                        ),
                )
            })
            .children(content)
    }

    fn render_tab_strip(
        &self,
        tabs: &[Tab],
        active: Option<Tab>,
        data: &AppData,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let (fg, muted, border) = (theme.foreground, theme.muted_foreground, theme.border);

        let items: Vec<AnyElement> = tabs
            .iter()
            .map(|tab| {
                let selected = active == Some(*tab);
                let (id, accent, lead, label, count) = match tab {
                    Tab::Server(server) => {
                        let color = ui::server_color(*server);
                        (
                            SharedString::from(format!("tab-{}", server.id())),
                            color,
                            div()
                                .size(px(6.))
                                .rounded_full()
                                .bg(color)
                                .into_any_element(),
                            SharedString::from(server.display_name()),
                            None,
                        )
                    }
                    Tab::Backups => (
                        SharedString::from("tab-backups"),
                        fg,
                        Icon::new(IconName::Archive).xsmall().into_any_element(),
                        SharedString::from(t!("titleBar.backups").to_string()),
                        Some(data.backups.len()),
                    ),
                    Tab::Probes => (
                        SharedString::from("tab-probes"),
                        fg,
                        Icon::new(IconName::Radar).xsmall().into_any_element(),
                        SharedString::from(t!("actions.editFormations").to_string()),
                        None,
                    ),
                    Tab::Drones => (
                        SharedString::from("tab-drones"),
                        fg,
                        Icon::new(IconName::Drone).xsmall().into_any_element(),
                        SharedString::from(t!("actions.editDroneGroups").to_string()),
                        None,
                    ),
                };
                let tab = *tab;
                h_flex()
                    .id(id)
                    .test_support()
                    .flex_none()
                    .relative()
                    .h_full()
                    .px_3()
                    .gap_2()
                    .cursor_pointer()
                    .text_size(px(13.))
                    .font_medium()
                    .text_color(if selected { fg } else { muted })
                    .when(!selected, |this| this.hover(move |s| s.text_color(fg)))
                    .child(lead)
                    .child(label)
                    .children(count.map(|n| {
                        div()
                            .text_xs()
                            .text_color(muted.opacity(0.8))
                            .child(n.to_string())
                    }))
                    .when(selected, |this| {
                        this.child(
                            div()
                                .absolute()
                                .left_3()
                                .right_3()
                                .bottom_0()
                                .h(px(2.))
                                .rounded_t(px(2.))
                                .bg(accent),
                        )
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.tab != Some(tab) {
                            if let Some(Tab::Server(server)) = this.tab {
                                this.editor_server = Some(server);
                            }
                            if let Tab::Server(server) = tab {
                                this.editor_server = Some(server);
                            }
                            this.tab = Some(tab);
                            this.alias_edit = None;
                            this.reset_browser_scroll();
                            cx.notify();
                        }
                    }))
                    .into_any_element()
            })
            .collect();

        h_flex()
            .flex_none()
            .h(px(40.))
            .px_1()
            .border_b_1()
            .border_color(border)
            .child(
                h_flex()
                    .id("navigation-tabs")
                    .h_full()
                    .flex_1()
                    .min_w_0()
                    .overflow_x_scroll()
                    .children(items),
            )
            .child(self.render_app_controls(cx))
    }

    fn ordered_browser_rows(&mut self, server: &ServerData, data: &Rc<AppData>) -> Rc<BrowserRows> {
        let profile_path = self
            .selected_profile(server)
            .map(|profile| profile.path.clone());
        let locale = rust_i18n::locale().to_string();
        let minute = if self.entry_query.is_empty() {
            0
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                / 60
        };
        if let Some(rows) = &self.browser_rows
            && Rc::ptr_eq(&rows.data, data)
            && rows.server == server.info.id
            && rows.profile_path == profile_path
            && rows.query == self.entry_query
            && rows.account_sort == self.account_sort
            && rows.character_sort == self.character_sort
            && rows.locale == locale
            && rows.minute == minute
        {
            return rows.clone();
        }
        let view: &MainView = self;
        let sections = server
            .profiles
            .iter()
            .enumerate()
            .filter(|(_, profile)| Some(&profile.path) == profile_path.as_ref())
            .flat_map(|(index, profile)| {
                [SettingsKind::Char, SettingsKind::User]
                    .into_iter()
                    .filter_map(move |kind| {
                        let mut entries: Vec<usize> = profile
                            .entries(kind)
                            .iter()
                            .enumerate()
                            .filter(|(_, entry)| view.entry_matches_search(entry))
                            .map(|(index, _)| index)
                            .collect();
                        view.sort_entries(&mut entries, profile, kind);
                        (!entries.is_empty()).then_some((index, kind, entries))
                    })
            })
            .collect();
        let rows = Rc::new(BrowserRows {
            data: data.clone(),
            server: server.info.id,
            profile_path,
            query: self.entry_query.clone(),
            account_sort: self.account_sort,
            character_sort: self.character_sort,
            locale,
            minute,
            sections,
        });
        self.browser_rows = Some(rows.clone());
        rows
    }

    fn render_server(
        &mut self,
        server: &ServerData,
        data: &Rc<AppData>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ordered = self.ordered_browser_rows(server, data);
        let Some(profile) = self.selected_profile(server) else {
            return div()
                .p_4()
                .child(t!("profiles.none").to_string())
                .into_any_element();
        };
        h_flex()
            .flex_1()
            .min_h_0()
            .items_stretch()
            .children(
                [SettingsKind::Char, SettingsKind::User]
                    .into_iter()
                    .map(|kind| {
                        let entries = ordered
                            .sections
                            .iter()
                            .find(|(_, section_kind, _)| *section_kind == kind)
                            .map(|(_, _, entries)| entries.as_slice())
                            .unwrap_or_default();
                        self.render_entry_table(profile, kind, entries, data, window, cx)
                    }),
            )
            .into_any_element()
    }

    // Each table keeps its header outside its independent, virtualized row scroller.
    fn render_entry_table(
        &self,
        profile: &ProfileData,
        kind: SettingsKind,
        entries: &[usize],
        data: &Rc<AppData>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let scroll = self.table_scroll(kind.into()).handle.clone();
        let layout = EntryTableLayout::new(kind, self.store.read(cx).config().character_columns);
        let viewport = scroll
            .bounds()
            .size
            .height
            .max(window.viewport_size().height);
        let range = visible_rows(
            entries.len(),
            0.,
            -scroll.offset().y.as_f32(),
            viewport.as_f32(),
            ui::ROW_HEIGHT.as_f32(),
        );
        let labels = RowLabels {
            source: t!("actions.source").to_string().into(),
            target: t!("actions.target").to_string().into(),
            source_path: self
                .source_for(kind, cx)
                .map(|source| source.path().to_string()),
        };
        let rows: Vec<_> = entries[range.clone()]
            .iter()
            .map(|index| {
                self.render_entry_row(
                    &profile.entries(kind)[*index],
                    data,
                    &labels,
                    &layout,
                    window,
                    cx,
                )
            })
            .collect();
        let content = v_flex()
            .w_full()
            .min_w(layout.min_width)
            .pr_4()
            .pb_4()
            .child(div().h(ui::ROW_HEIGHT * range.start as f32).flex_none())
            .children(rows)
            .child(
                div()
                    .h(ui::ROW_HEIGHT * (entries.len() - range.end) as f32)
                    .flex_none(),
            )
            .when(entries.is_empty(), |this| {
                this.child(
                    div().p_4().text_color(cx.theme().muted_foreground).child(
                        if self.entry_query.is_empty() {
                            t!("list.noEntries")
                        } else {
                            t!("list.noMatchingEntries")
                        }
                        .to_string(),
                    ),
                )
            });
        v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .when(kind == SettingsKind::Char, |this| {
                this.border_r_1().border_color(cx.theme().border)
            })
            .child(self.profile_header(
                profile,
                kind,
                entries.len(),
                &layout,
                scroll.offset().x,
                cx,
            ))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        div()
                            .id(match kind {
                                SettingsKind::User => "accounts-scroll",
                                SettingsKind::Char => "characters-scroll",
                            })
                            .size_full()
                            .overflow_scroll()
                            .track_scroll(&scroll)
                            .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
                            .child(content),
                    )
                    .child(self.table_wheel_layer(kind.into(), cx))
                    .child(
                        Scrollbar::new(&scroll)
                            .id(match kind {
                                SettingsKind::Char => "characters-scrollbar",
                                SettingsKind::User => "accounts-scrollbar",
                            })
                            .mode(ScrollbarMode::Always),
                    ),
            )
            .into_any_element()
    }

    fn sort_entries(&self, entries: &mut [usize], profile: &ProfileData, kind: SettingsKind) {
        let sort = match kind {
            SettingsKind::User => self.account_sort,
            SettingsKind::Char => self.character_sort,
        };
        entries.sort_by(|a, b| {
            sort.apply(compare_entries(
                &profile.entries(kind)[*a],
                &profile.entries(kind)[*b],
                sort.column,
            ))
        });
    }

    // Only column headers sort rows; table titles are plain labels.
    fn sort_control(
        &self,
        id: SharedString,
        label: impl IntoElement,
        column: SortColumn,
        sort: Sort,
        on_sort: impl Fn(&mut Self, SortColumn) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let arrow = (sort.column == column).then(|| {
            Icon::new(if sort.ascending {
                IconName::ChevronUp
            } else {
                IconName::ChevronDown
            })
            .xsmall()
        });
        let hover = cx.theme().foreground;
        h_flex()
            .id(id)
            .gap_1()
            .cursor_pointer()
            .hover(move |s| s.text_color(hover))
            .child(label)
            .children(arrow)
            .on_click(cx.listener(move |this, _, _, cx| {
                on_sort(this, column);
                cx.notify();
            }))
            .test_support()
            .into_any_element()
    }

    fn profile_header(
        &self,
        profile: &ProfileData,
        kind: SettingsKind,
        count: usize,
        layout: &EntryTableLayout,
        horizontal_offset: Pixels,
        cx: &mut Context<Self>,
    ) -> Div {
        let sort = match kind {
            SettingsKind::User => self.account_sort,
            SettingsKind::Char => self.character_sort,
        };
        let title = match kind {
            SettingsKind::User => t!("list.accounts"),
            SettingsKind::Char => t!("list.characters"),
        };
        let section_id = format!("{}-{kind:?}", profile.path);
        let set_sort = move |this: &mut Self, column| match kind {
            SettingsKind::User => this.account_sort.toggle(column, column == SortColumn::Time),
            SettingsKind::Char => this
                .character_sort
                .toggle(column, column == SortColumn::Time),
        };
        let title = h_flex()
            .id(match kind {
                SettingsKind::Char => "table-title-characters",
                SettingsKind::User => "table-title-accounts",
            })
            .test_support()
            .gap_2()
            .child(ui::section_title(title.to_string()))
            .child(ui::count_label(count, cx));
        let can_add_all = self.source_for(kind, cx).is_some() && !self.copying;
        let profile_path = profile.path.clone();
        let add_all = Button::new(SharedString::from(format!("add-all-{section_id}")))
            .ghost()
            .xsmall()
            .icon(IconName::ArrowDownToLine)
            .label(t!("table.addAll").to_string())
            .disabled(!can_add_all)
            .when(!can_add_all, |this| {
                this.tooltip(t!("toast.noSourceSelectedDesc").to_string())
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                let data = this.store.read(cx).data();
                if let Some(profile) = data.as_ref().and_then(|data| {
                    data.servers
                        .iter()
                        .flat_map(|server| &server.profiles)
                        .find(|profile| profile.path == profile_path)
                }) {
                    this.add_all_from_profile(profile, kind, window, cx);
                }
            }));
        let mut cells = vec![table::empty()];
        cells.extend(layout.fields.iter().map(|column| {
            self.sort_control(
                format!("sort-{}-{section_id}", column_id(*column)).into(),
                ui::section_title(column_label(*column, kind)),
                *column,
                sort,
                set_sort,
                cx,
            )
            .into_any_element()
        }));
        cells.push(table::empty());
        v_flex()
            .flex_none()
            .child(
                ui::section_header(cx)
                    .gap_2()
                    .child(title)
                    .when(kind == SettingsKind::Char, |this| {
                        this.child(self.character_columns_menu(cx))
                    })
                    .child(div().flex_1())
                    .child(add_all),
            )
            .child(
                div().w_full().overflow_hidden().child(table::cells(
                    ui::section_header(cx)
                        .w_full()
                        .min_w(layout.min_width)
                        .relative()
                        .left(horizontal_offset)
                        .h(px(28.))
                        .pr(px(32.))
                        .gap_0(),
                    &layout.columns,
                    cells,
                )),
            )
    }

    fn render_entry_row(
        &self,
        entry: &SettingsEntry,
        data: &Rc<AppData>,
        labels: &RowLabels,
        layout: &EntryTableLayout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_source = labels.source_path.as_deref() == Some(entry.path.as_str());
        let is_target = self.targets.contains_path(&entry.path);
        let can_target = labels.source_path.is_some() && !is_source && !self.copying;
        let marker = if is_source {
            Some(ui::source_color())
        } else if is_target {
            Some(ui::target_color())
        } else {
            None
        };
        let portrait = self.store.read(cx).portrait(&entry.id);
        let row_id = SharedString::from(entry.path.clone());

        let actions = h_flex()
            .gap_1()
            .child(
                Button::new(SharedString::from(format!("{}:source", row_id)))
                    .ghost()
                    .xsmall()
                    .icon(IconName::ArrowUpFromLine)
                    .disabled(is_source || self.copying)
                    .tooltip(labels.source.clone())
                    .on_click(cx.listener({
                        let entry = entry.clone();
                        move |this, _, _, cx| this.set_source(Source::Entry(entry.clone()), cx)
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("{}:target", row_id)))
                    .ghost()
                    .xsmall()
                    .icon(IconName::ArrowDownToLine)
                    .disabled(!can_target)
                    .tooltip(labels.target.clone())
                    .on_click(cx.listener({
                        let entry = entry.clone();
                        move |this, _, window, cx| this.add_target(entry.clone(), window, cx)
                    })),
            )
            .child(self.render_entry_menu(entry, data, row_id.clone(), cx));

        let dragged = Source::Entry(entry.clone());
        let drag_portrait = portrait.clone();
        let mut cells = vec![ui::entry_avatar(entry.kind, portrait, px(20.), cx)];
        cells.extend(layout.fields.iter().map(|column| {
            if *column == SortColumn::Name {
                self.render_entry_name(entry, window, cx)
            } else {
                div()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(entry_column_text(entry, *column))
                    .into_any_element()
            }
        }));
        cells.push(actions.into_any_element());
        table::cells(
            ui::list_row(ElementId::Name(row_id), marker, cx)
                .h(ui::ROW_HEIGHT)
                .group("entry-row")
                .on_drag(DraggedSettings(dragged), move |dragged, grab, _, cx| {
                    let portrait = drag_portrait.clone();
                    cx.new(|_| DragPreview::new(&dragged.0, grab, portrait))
                }),
            &layout.columns,
            cells,
        )
        .into_any_element()
    }

    fn render_entry_name(
        &self,
        entry: &SettingsEntry,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if entry.kind == SettingsKind::Char {
            return div()
                .min_w_0()
                .truncate()
                .child(entry.display_name.clone())
                .into_any_element();
        }

        if let Some(edit) = self.alias_edit.as_ref().filter(|e| e.id == entry.id) {
            let id = entry.id.clone();
            let input = edit.input.clone();
            return h_flex()
                .id("alias-editor")
                .test_support()
                .w_full()
                .min_w_0()
                .gap_1()
                .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        this.alias_edit = None;
                        cx.notify();
                    }
                }))
                .child(
                    div()
                        .id("alias-input")
                        .test_support()
                        .flex_1()
                        .min_w_0()
                        .child(Input::new(&edit.input).small()),
                )
                .child(
                    Button::new("alias-save")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Check)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            let alias = input.read(cx).value().to_string();
                            this.save_alias(id.clone(), alias, window, cx)
                        })),
                )
                .child(
                    Button::new("alias-cancel")
                        .ghost()
                        .xsmall()
                        .icon(IconName::X)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.alias_edit = None;
                            cx.notify();
                        })),
                )
                .into_any_element();
        }

        let entry_for_edit = entry.clone();
        h_flex()
            .w_full()
            .gap_2()
            .min_w_0()
            .child(
                div()
                    .flex_none()
                    .opacity(0.)
                    .group_hover("entry-row", |s| s.opacity(1.))
                    .child(
                        Button::new(SharedString::from(format!("alias-edit-{}", entry.path)))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Pencil)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.start_alias_edit(&entry_for_edit, window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(entry.alias.clone().unwrap_or_else(|| "\u{2014}".into())),
            )
            .into_any_element()
    }

    // Menu contents are built only when the menu opens.
    fn render_entry_menu(
        &self,
        entry: &SettingsEntry,
        data: &Rc<AppData>,
        row_id: SharedString,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let view = cx.entity().downgrade();
        let entry = entry.clone();
        let data = data.clone();

        Button::new(SharedString::from(format!("{}:menu", row_id)))
            .ghost()
            .xsmall()
            .icon(IconName::Ellipsis)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, window, cx| {
                let menu = menu.item(
                    PopupMenuItem::new(t!("actions.createBackup").to_string())
                        .icon(IconName::Save)
                        .on_click(view_action(&view, {
                            let entry = entry.clone();
                            move |this, window, cx| this.create_backup(entry.clone(), window, cx)
                        })),
                );

                let backups: Vec<BackupEntry> = data.backups_for(&entry).cloned().collect();
                if backups.is_empty() {
                    return menu.item(
                        PopupMenuItem::new(t!("actions.noBackupsAvailable").to_string())
                            .icon(IconName::RotateCcw)
                            .disabled(true),
                    );
                }

                let view = view.clone();
                let entry = entry.clone();
                menu.submenu_with_icon(
                    Some(Icon::new(IconName::RotateCcw)),
                    t!("actions.restoreFromBackup").to_string(),
                    window,
                    cx,
                    move |menu, _, _| {
                        backups.iter().fold(
                            menu.max_h(px(320.)).scrollable(true),
                            |menu, backup| {
                                let entry = entry.clone();
                                let backup = backup.clone();
                                menu.item(
                                    PopupMenuItem::new(format!(
                                        "{}  ·  {}",
                                        backup.name,
                                        ui::relative_time(backup.timestamp)
                                    ))
                                    .on_click(view_action(
                                        &view,
                                        move |this, window, cx| {
                                            this.restore_backup(
                                                entry.clone(),
                                                backup.clone(),
                                                window,
                                                cx,
                                            )
                                        },
                                    )),
                                )
                            },
                        )
                    },
                )
            })
    }

    fn sorted_backups(&self, data: &AppData) -> Vec<BackupEntry> {
        let sort = self.backup_sort;
        let mut backups = data.backups.clone();
        backups.sort_by(|a, b| {
            sort.apply(match sort.column {
                SortColumn::Time => a.timestamp.cmp(&b.timestamp),
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            })
        });
        backups
    }

    fn render_backups(
        &self,
        data: &Rc<AppData>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let sort = self.backup_sort;
        let all_selected = !data.backups.is_empty()
            && data
                .backups
                .iter()
                .all(|b| self.selected_backups.contains(&b.id));
        let selected_count = self.selected_backups.len();
        let all_ids: Vec<String> = data.backups.iter().map(|b| b.id.clone()).collect();
        let set_sort = |this: &mut Self, column| this.backup_sort.toggle(column, true);

        let header = ui::section_header(cx)
            .gap_0()
            .child(table::cell(
                &BACKUP_COLUMNS[0],
                Checkbox::new("backups-select-all")
                    .checked(all_selected)
                    .on_change(cx.listener(move |this, checked: &bool, _, cx| {
                        this.selected_backups = if *checked {
                            all_ids.iter().cloned().collect()
                        } else {
                            Default::default()
                        };
                        cx.notify();
                    })),
            ))
            .child(table::cell(&BACKUP_COLUMNS[1], table::empty()))
            .child(table::cell(
                &BACKUP_COLUMNS[2],
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(
                        self.sort_control(
                            "sort-backup-name".into(),
                            h_flex()
                                .gap_2()
                                .child(ui::section_title(t!("titleBar.backups").to_string()))
                                .child(ui::count_label(data.backups.len(), cx)),
                            SortColumn::Name,
                            sort,
                            set_sort,
                            cx,
                        ),
                    )
                    .when(selected_count > 0, |this| {
                        this.child(
                            h_flex()
                                .gap_2()
                                .pr_4()
                                .child(
                                    t!("backup.selectedCount", count = selected_count).to_string(),
                                )
                                .child(
                                    Button::new("delete-selected-backups")
                                        .danger()
                                        .xsmall()
                                        .icon(IconName::Trash)
                                        .label(t!("backup.deleteSelected").to_string())
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.delete_selected_backups(window, cx)
                                        })),
                                ),
                        )
                    }),
            ))
            .child(table::cell(
                &BACKUP_COLUMNS[3],
                self.sort_control(
                    "sort-backup-time".into(),
                    ui::section_title(t!("backup.date").to_string()),
                    SortColumn::Time,
                    sort,
                    set_sort,
                    cx,
                ),
            ))
            .child(table::cell(&BACKUP_COLUMNS[4], table::empty()));

        let scroll = self.backup_scroll.handle.clone();
        let viewport = scroll
            .bounds()
            .size
            .height
            .max(window.viewport_size().height);
        let backups = self.sorted_backups(data);
        let range = visible_rows(
            backups.len(),
            0.,
            -scroll.offset().y.as_f32(),
            viewport.as_f32(),
            ui::ROW_HEIGHT.as_f32(),
        );
        let rows: Vec<_> = backups[range.clone()]
            .iter()
            .map(|backup| self.render_backup_row(backup, data, cx))
            .collect();
        let content = v_flex()
            .w_full()
            .pr_4()
            .pb_4()
            .child(div().h(ui::ROW_HEIGHT * range.start as f32).flex_none())
            .children(rows)
            .child(
                div()
                    .h(ui::ROW_HEIGHT * (backups.len() - range.end) as f32)
                    .flex_none(),
            );
        v_flex()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .child(header.pr_8())
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        div()
                            .id("backups")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&scroll)
                            .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
                            .child(content),
                    )
                    .child(self.table_wheel_layer(TableKind::Backups, cx))
                    .child(
                        Scrollbar::vertical(&scroll)
                            .id("backups-scrollbar")
                            .mode(ScrollbarMode::Always),
                    ),
            )
    }

    fn render_backup_row(
        &self,
        backup: &BackupEntry,
        data: &Rc<AppData>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_source = matches!(&self.source, Some(Source::Backup(b)) if b.id == backup.id);
        let selected = self.selected_backups.contains(&backup.id);
        let marker = if is_source {
            Some(ui::source_color())
        } else if selected {
            Some(cx.theme().muted_foreground.opacity(0.6))
        } else {
            None
        };
        let row_id = SharedString::from(backup.path.clone());
        let view = cx.entity().downgrade();
        let data = data.clone();
        let menu_backup = backup.clone();
        let backup_id = backup.id.clone();
        let muted = cx.theme().muted_foreground;

        ui::list_row(ElementId::Name(row_id.clone()), marker, cx)
            .h(ui::ROW_HEIGHT)
            .w_full()
            .on_drag(
                DraggedSettings(Source::Backup(backup.clone())),
                |dragged, grab, _, cx| cx.new(|_| DragPreview::new(&dragged.0, grab, None)),
            )
            .child(table::cell(
                &BACKUP_COLUMNS[0],
                Checkbox::new(SharedString::from(format!("{}:select", row_id)))
                    .checked(selected)
                    .on_change(cx.listener(move |this, checked: &bool, _, cx| {
                        if *checked {
                            this.selected_backups.insert(backup_id.clone());
                        } else {
                            this.selected_backups.remove(&backup_id);
                        }
                        cx.notify();
                    })),
            ))
            .child(table::cell(
                &BACKUP_COLUMNS[1],
                ui::entry_avatar(backup.kind, None, px(24.), cx),
            ))
            .child(table::cell(
                &BACKUP_COLUMNS[2],
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().truncate().child(backup.name.clone()))
                    .child(
                        div().truncate().text_xs().text_color(muted).child(
                            backup
                                .original_name
                                .clone()
                                .unwrap_or_else(|| backup.original_id.clone()),
                        ),
                    ),
            ))
            .child(table::cell(
                &BACKUP_COLUMNS[3],
                div()
                    .text_color(muted)
                    .child(ui::relative_time(backup.timestamp)),
            ))
            .child(table::cell(
                &BACKUP_COLUMNS[4],
                h_flex().child(
                    Button::new(SharedString::from(format!("{}:menu", row_id)))
                        .ghost()
                        .small()
                        .icon(IconName::Ellipsis)
                        .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, window, cx| {
                            let backup = menu_backup.clone();
                            let menu = menu.item(
                                PopupMenuItem::new(t!("actions.useAsSource").to_string())
                                    .icon(IconName::ArrowUpFromLine)
                                    .disabled(is_source)
                                    .on_click(view_action(&view, {
                                        let backup = backup.clone();
                                        move |this, _, cx| {
                                            this.set_source(Source::Backup(backup.clone()), cx)
                                        }
                                    })),
                            );
                            let targets: Vec<SettingsEntry> = data
                                .entries()
                                .filter(|e| e.kind == backup.kind)
                                .cloned()
                                .collect();
                            let menu = if targets.is_empty() {
                                menu
                            } else {
                                let view = view.clone();
                                let backup = backup.clone();
                                menu.submenu_with_icon(
                                    Some(Icon::new(IconName::ArrowDownToLine)),
                                    t!("actions.applyTo").to_string(),
                                    window,
                                    cx,
                                    move |menu, _, _| {
                                        targets.iter().fold(
                                            menu.max_h(px(256.)).scrollable(true),
                                            |menu, target| {
                                                let target = target.clone();
                                                let backup = backup.clone();
                                                menu.item(
                                                    PopupMenuItem::new(target.display_name.clone())
                                                        .on_click(view_action(
                                                            &view,
                                                            move |this, window, cx| {
                                                                this.apply_backup(
                                                                    backup.clone(),
                                                                    target.clone(),
                                                                    window,
                                                                    cx,
                                                                )
                                                            },
                                                        )),
                                                )
                                            },
                                        )
                                    },
                                )
                            };
                            menu.separator().item(
                                PopupMenuItem::new(t!("actions.delete").to_string())
                                    .icon(IconName::Trash)
                                    .on_click(view_action(&view, move |this, window, cx| {
                                        this.delete_backups(vec![backup.clone()], window, cx)
                                    })),
                            )
                        }),
                ),
            ))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::super::profiles::tests::sample_data;
    use super::super::{MainView, Source};
    use crate::store::Store;
    use eve_wrench_core::Server;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, WindowOptions, point, px};
    use std::rc::Rc;

    #[gpui_kit::test]
    fn account_custom_name_can_be_added_and_edited_from_the_row(cx: &mut TestAppContext) {
        let (handle, view, locations, account_path) = cx.update(|cx| {
            gpui_kit::init(cx);
            Store::install_for_test(sample_data(), cx);
            let store = Store::global(cx);
            let locations = store.read(cx).locations().clone();
            let eve_root = locations.data_dir().join("eve");
            let profile = eve_root
                .join("c_ccp_eve_tq_tranquility")
                .join("settings_Default");
            std::fs::create_dir_all(&profile).unwrap();
            let account_path = profile.join("core_user_2.dat");
            std::fs::write(&account_path, b"account settings stay unchanged").unwrap();
            store.update(cx, |store, cx| {
                store
                    .update_config(cx, |config| {
                        config.custom_eve_path = Some(eve_root.to_string_lossy().into_owned());
                    })
                    .unwrap();
            });
            let (handle, view) =
                gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                    cx.new(|cx| MainView::new(window, cx))
                })
                .unwrap();
            (handle, view, locations, account_path)
        });
        cx.run_until_parked();
        let pencil = format!("alias-edit-{}", account_path.to_string_lossy());
        let long_name = "A long custom account name that must leave the pencil visible — 主账号";
        for (typed, expected, enter) in [
            ("  Main account  ", Some("Main account"), false),
            (long_name, Some(long_name), true),
            ("   ", None, false),
        ] {
            cx.update_window(handle, |_, window, cx| {
                window.render_frame(cx);
                assert!(window.find(pencil.clone()).visible());
                window.hover(pencil.clone(), cx);
                window.click(pencil.clone(), cx);
                assert!(
                    view.read(cx).alias_edit.is_some(),
                    "Pencil must open the editor"
                );
                window.render_frame(cx);
                let input = window.find("alias-input").bounds();
                assert!(
                    input.size.width >= px(60.),
                    "Alias input must have room to type"
                );
                window.click("alias-input", cx);
                window.press("secondary-a", cx);
                window.input(typed, cx);
                assert_eq!(
                    view.read(cx)
                        .alias_edit
                        .as_ref()
                        .unwrap()
                        .input
                        .read(cx)
                        .value(),
                    typed
                );
                if enter {
                    window.press("enter", cx);
                } else {
                    window.click("alias-save", cx);
                }
            })
            .unwrap();
            cx.run_until_parked();
            cx.update(|cx| {
                assert!(
                    view.read(cx).alias_edit.is_none(),
                    "Save must close the editor (enter={enter})"
                );
                let data = Store::global(cx).read(cx).data().unwrap();
                let account = data.find_entry(&account_path.to_string_lossy()).unwrap();
                assert_eq!(account.alias.as_deref(), expected);
                assert_eq!(account.display_name, expected.unwrap_or("2"));
                let aliases: std::collections::HashMap<String, String> =
                    serde_json::from_slice(&std::fs::read(locations.aliases_file()).unwrap())
                        .unwrap();
                assert_eq!(aliases.get("2").map(String::as_str), expected);
            });
        }
        cx.update_window(handle, |_, window, cx| {
            for escape in [false, true] {
                window.click(pencil.clone(), cx);
                window.input("Cancelled name", cx);
                if escape {
                    window.press("escape", cx);
                } else {
                    window.click("alias-cancel", cx);
                }
                assert!(view.read(cx).alias_edit.is_none());
            }
        })
        .unwrap();
        cx.run_until_parked();
        let aliases: serde_json::Value =
            serde_json::from_slice(&std::fs::read(locations.aliases_file()).unwrap()).unwrap();
        assert!(aliases.get("2").is_none());
        assert_eq!(
            std::fs::read(&account_path).unwrap(),
            b"account settings stay unchanged"
        );
        let test_dir = locations.data_dir().canonicalize().unwrap();
        assert!(test_dir.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        std::fs::remove_dir_all(test_dir).unwrap();
    }

    #[gpui_kit::test]
    fn profiles_show_separately_and_switch_without_stale_cached_rows(cx: &mut TestAppContext) {
        let (handle, view) = cx.update(|cx| {
            gpui_kit::init(cx);
            Store::install_for_test(sample_data(), cx);
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| MainView::new(window, cx))
            })
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            view.update(cx, |view, cx| {
                let data = view.store.read(cx).data().unwrap();
                let server = &data.servers[0];
                let default = view.ordered_browser_rows(server, &data);
                assert_eq!(default.sections.len(), 2);
                assert!(
                    default
                        .sections
                        .iter()
                        .all(|(profile, _, entries)| *profile == 1 && *entries == [0, 1])
                );
                assert!(Rc::ptr_eq(
                    &default,
                    &view.ordered_browser_rows(server, &data)
                ));

                let source = Source::Entry(server.profiles[1].characters[0].clone());
                view.set_source(source.clone(), cx);
                view.targets.push(server.profiles[1].characters[1].clone());
                view.character_scroll
                    .handle
                    .set_offset(point(px(0.), px(-100.)));
                view.select_profile(server.info.id, server.profiles[0].path.clone(), cx);
                assert_eq!(view.character_scroll.handle.offset().y, px(0.));
                assert!(view.source == Some(source));
                assert_eq!(view.targets.len(), 1);
                let alternate = view.ordered_browser_rows(server, &data);
                assert!(!Rc::ptr_eq(&default, &alternate));
                assert!(
                    alternate
                        .sections
                        .iter()
                        .all(|(profile, _, _)| *profile == 0)
                );
                assert_eq!(
                    view.selected_profile(&data.servers[1]).unwrap().name,
                    "Default"
                );
                view.select_profile(
                    Server::Singularity,
                    data.servers[1].profiles[0].path.clone(),
                    cx,
                );
                assert_eq!(view.selected_profile(server).unwrap().name, "Alternate");
                view.select_profile(server.info.id, "/missing/profile".into(), cx);
                assert_eq!(view.selected_profile(server).unwrap().name, "Alternate");

                let mut refreshed = (*data).clone();
                refreshed.servers[0].profiles.remove(0);
                let refreshed = Rc::new(refreshed);
                assert_eq!(
                    view.selected_profile(&refreshed.servers[0]).unwrap().name,
                    "Default"
                );
                let rows = view.ordered_browser_rows(&refreshed.servers[0], &refreshed);
                assert!(rows.sections.iter().all(|(profile, _, _)| *profile == 0));
            });
            window.render_frame(cx);
        })
        .unwrap();
    }
}
