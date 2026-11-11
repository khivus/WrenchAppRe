use std::cell::Cell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use eve_wrench_core::confirmations::{self, Confirmation, ConfirmationKind};
use gpui_kit::assets::IconName;
use gpui_kit::base::{Scrollbar, ScrollbarMode};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use rust_i18n::t;

use crate::store::{Store, StoreEvent};
use crate::ui::{self, StyleExt as _};

pub(super) fn embedded(
    path: String,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ConfirmationsEditor> {
    cx.new(|cx| ConfirmationsEditor::new(PathBuf::from(path), window, cx))
}

pub(super) struct ConfirmationsSaved {
    pub path: String,
}
impl EventEmitter<ConfirmationsSaved> for ConfirmationsEditor {}

// The sidebar's groups: the catalog's kinds, then suppressed dialogs the
// catalog doesn't know.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Kind(ConfirmationKind),
    Other,
}

const SECTIONS: [Section; 5] = [
    Section::Kind(ConfirmationKind::Question),
    Section::Kind(ConfirmationKind::Warning),
    Section::Kind(ConfirmationKind::Notice),
    Section::Kind(ConfirmationKind::Welcome),
    Section::Other,
];

impl Section {
    fn id(self) -> &'static str {
        match self {
            Section::Kind(ConfirmationKind::Question) => "question",
            Section::Kind(ConfirmationKind::Warning) => "warning",
            Section::Kind(ConfirmationKind::Notice) => "notice",
            Section::Kind(ConfirmationKind::Welcome) => "welcome",
            Section::Other => "other",
        }
    }

    fn label(self) -> String {
        t!(format!("confirmations.kind.{}", self.id())).to_string()
    }

    fn icon(self) -> IconName {
        match self {
            Section::Kind(ConfirmationKind::Question) => IconName::MessageCircleQuestionMark,
            Section::Kind(ConfirmationKind::Warning) => IconName::TriangleAlert,
            Section::Kind(ConfirmationKind::Notice) => IconName::Info,
            Section::Kind(ConfirmationKind::Welcome) => IconName::Sparkles,
            Section::Other => IconName::Inbox,
        }
    }
}

// One dialog as the list shows it.
struct Row {
    key: String,
    section: Section,
    title: SharedString,
    body: Option<SharedString>,
    preview: Option<SharedString>,
    search_text: String,
    answer: Option<u8>,
}

impl Row {
    fn new(
        key: String,
        section: Section,
        title: String,
        body: Option<String>,
        answer: Option<u8>,
    ) -> Self {
        let preview = body
            .as_ref()
            .map(|body| body.split_whitespace().collect::<Vec<_>>().join(" ").into());
        let search_text =
            format!("{key}\n{title}\n{}", body.as_deref().unwrap_or_default()).to_lowercase();
        Self {
            key,
            section,
            title: title.into(),
            body: body.map(Into::into),
            preview,
            search_text,
            answer,
        }
    }
}

struct RowCache {
    catalog: Arc<[Confirmation]>,
    language: &'static str,
    stored: HashSet<String>,
    rows: Rc<Vec<Row>>,
}

pub(super) struct ConfirmationsEditor {
    path: PathBuf,
    // Suppressed in the file, and after saving
    stored: HashSet<String>,
    selected: HashSet<String>,
    // None shows every group
    section: Option<Section>,
    suppressed_only: bool,
    // Rows showing their full text
    expanded: HashSet<String>,
    store: Entity<Store>,
    loading: bool,
    saving: bool,
    disk_changed: bool,
    error: Option<String>,
    list_scroll: crate::scrolling::SmoothScroll<ListState>,
    row_cache: Option<RowCache>,
    listed_rows: Rc<Vec<Row>>,
    visible_rows: Rc<Vec<usize>>,
    list_hinted_width: Rc<Cell<Option<Pixels>>>,
    #[cfg(test)]
    rows_rendered: usize,
    #[cfg(test)]
    row_cache_builds: usize,
    sidebar_scroll: crate::scrolling::SmoothScroll,
    search: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl ConfirmationsEditor {
    fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::global(cx);
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("confirmations.search").to_string())
        });
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.list_scroll.reset();
                    cx.notify();
                }
            }),
            cx.subscribe_in(&store, window, |this, _, event, window, cx| {
                if matches!(event, StoreEvent::DataChanged) && !this.loading && !this.saving {
                    this.check_disk(window, cx);
                }
            }),
        ];
        let mut editor = Self {
            path,
            store,
            search,
            _subscriptions: subscriptions,
            stored: HashSet::new(),
            selected: HashSet::new(),
            section: None,
            suppressed_only: false,
            expanded: HashSet::new(),
            loading: true,
            saving: false,
            disk_changed: false,
            error: None,
            list_scroll: crate::scrolling::SmoothScroll::new(ListState::new(
                0,
                ListAlignment::Top,
                px(120.),
            )),
            row_cache: None,
            listed_rows: Rc::new(Vec::new()),
            visible_rows: Rc::new(Vec::new()),
            list_hinted_width: Rc::new(Cell::new(None)),
            #[cfg(test)]
            rows_rendered: 0,
            #[cfg(test)]
            row_cache_builds: 0,
            sidebar_scroll: Default::default(),
        };
        editor.load(window, cx);
        editor
    }

    pub(super) fn has_pending_edits(&self) -> bool {
        self.selected != self.stored || self.saving
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.loading = true;
        self.error = None;
        let path = self.path.clone();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { confirmations::read_suppressed(&path) })
                .await;
            this.update_in(cx, |this, _, cx| {
                this.loading = false;
                match result {
                    Ok(stored) => {
                        this.selected = stored.clone();
                        this.stored = stored;
                        this.disk_changed = false;
                        this.list_scroll.reset();
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn check_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = self.path.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { confirmations::read_suppressed(&path) })
                .await;
            this.update_in(cx, |this, _, cx| {
                if this.loading || this.saving {
                    return;
                }
                match result {
                    Ok(stored) if stored != this.stored => {
                        if this.has_pending_edits() {
                            this.disk_changed = true;
                        } else {
                            this.selected = stored.clone();
                            this.stored = stored;
                            this.list_scroll.reset();
                        }
                    }
                    Err(_) => this.disk_changed = true,
                    _ => {}
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || self.saving || self.error.is_some() || !self.has_pending_edits() {
            return;
        }
        let path = self.path.clone();
        let saved = self.selected.clone();
        let payload = saved.clone();
        let backup = self.store.read(cx).config().auto_backup;
        self.saving = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    confirmations::write_suppressed(&path, &payload, backup)
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.saving = false;
                match result {
                    Ok(()) => {
                        this.stored = saved;
                        this.disk_changed = false;
                        ui::notify_success(
                            t!("toast.confirmationsSaved"),
                            t!("toast.confirmationsSavedDesc", count = this.stored.len()),
                            window,
                            cx,
                        );
                        this.store.update(cx, |store, cx| store.reload(cx));
                        cx.emit(ConfirmationsSaved {
                            path: this.path.to_string_lossy().into_owned(),
                        });
                    }
                    Err(error) => {
                        ui::notify_error(t!("toast.confirmationsSaveFailed"), error, window, cx)
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // Dialogs without a known answer can't be suppressed from here, only
    // cleared and restored if the file already has them.
    fn rows(&mut self) -> Rc<Vec<Row>> {
        let catalog = confirmations::catalog();
        let language = ui::catalog_language();
        if let Some(cache) = &self.row_cache
            && Arc::ptr_eq(&cache.catalog, &catalog)
            && cache.language == language
            && cache.stored == self.stored
        {
            return cache.rows.clone();
        }
        let mut rows: Vec<Row> = catalog
            .iter()
            .filter(|d| d.can_suppress() || self.stored.contains(&d.key))
            .map(|d| {
                Row::new(
                    d.key.clone(),
                    Section::Kind(d.kind),
                    d.title(language).to_string(),
                    d.body(language).map(str::to_string),
                    d.answer,
                )
            })
            .collect();
        rows.extend(
            self.stored
                .iter()
                .filter(|key| !catalog.iter().any(|d| &d.key == *key))
                .map(|key| Row::new(key.clone(), Section::Other, key.clone(), None, None)),
        );
        rows.sort_by_cached_key(|r| r.title.to_lowercase());
        let rows = Rc::new(rows);
        self.row_cache = Some(RowCache {
            catalog,
            language,
            stored: self.stored.clone(),
            rows: rows.clone(),
        });
        #[cfg(test)]
        {
            self.row_cache_builds += 1;
        }
        rows
    }

    fn toggle_expanded(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.expanded.remove(key) {
            self.expanded.insert(key.to_string());
        }
        if let Some(index) = self
            .visible_rows
            .iter()
            .position(|&index| self.listed_rows[index].key == key)
        {
            self.list_scroll.handle.remeasure_items(index..index + 1);
        }
        cx.notify();
    }

    fn set(&mut self, key: &str, on: bool, cx: &mut Context<Self>) {
        if self.loading || self.saving || self.error.is_some() {
            return;
        }
        if on {
            self.selected.insert(key.to_string());
        } else {
            self.selected.remove(key);
        }
        cx.notify();
    }

    fn render_nav_item(
        &self,
        section: Option<Section>,
        rows: &[Row],
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = cx.theme();
        let active = self.section == section;
        let in_section: Vec<&Row> = rows
            .iter()
            .filter(|r| section.is_none_or(|s| r.section == s))
            .collect();
        let on = in_section
            .iter()
            .filter(|r| self.selected.contains(&r.key))
            .count();
        let (icon, label) = match section {
            Some(section) => (section.icon(), section.label()),
            None => (
                IconName::LayoutDashboard,
                t!("confirmations.all").to_string(),
            ),
        };
        let (fg, muted, hover, active_bg) = (
            theme.foreground,
            theme.muted_foreground,
            theme.muted.opacity(0.5),
            theme.muted,
        );

        h_flex()
            .id(SharedString::from(format!(
                "confirmations-nav-{}",
                section.map_or("all", |s| s.id())
            )))
            .gap_2p5()
            .px_2p5()
            .py_1p5()
            .rounded_md()
            .cursor_pointer()
            .text_sm()
            .map(|this| {
                if active {
                    this.bg(active_bg).text_color(fg).font_medium()
                } else {
                    this.text_color(muted).hover(move |s| s.bg(hover))
                }
            })
            .child(Icon::new(icon).small())
            .child(div().flex_1().min_w_0().truncate().child(label))
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(format!("{on}/{}", in_section.len())),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.section = section;
                this.list_scroll.reset();
                cx.notify();
            }))
    }

    fn render_sidebar(&self, rows: &[Row], cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let sections: Vec<Section> = SECTIONS
            .into_iter()
            .filter(|s| rows.iter().any(|r| r.section == *s))
            .collect();
        let content = v_flex()
            .flex_none()
            .gap_0p5()
            .child(self.render_nav_item(None, rows, cx))
            .child(
                div()
                    .h(px(1.))
                    .my_1p5()
                    .mx_2p5()
                    .bg(cx.theme().border.opacity(0.5)),
            )
            .children(
                sections
                    .into_iter()
                    .map(|s| self.render_nav_item(Some(s), rows, cx)),
            );
        self.sidebar_scroll
            .view(
                "confirmations-sidebar",
                content,
                |this| &mut this.sidebar_scroll,
                cx,
            )
            .w_full()
            .h_full()
            .flex_none()
            .border_r_1()
            .border_color(cx.theme().border.opacity(0.6))
    }

    // A flat row with a divider below; clicking it shows the full text.
    fn render_row(&self, row: &Row, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let on = self.selected.contains(&row.key);
        let theme = cx.theme();
        let (fg, muted, divider, hover) = (
            theme.foreground,
            theme.muted_foreground,
            theme.border.opacity(0.5),
            theme.muted.opacity(0.3),
        );
        let answer = match row.answer {
            Some(6) => Some(t!("confirmations.answerYes").to_string()),
            Some(1) => Some(t!("confirmations.answerOk").to_string()),
            _ => None,
        };
        let (row_key, switch_key) = (row.key.clone(), row.key.clone());
        let expanded = self.expanded.contains(&row.key);

        h_flex()
            .id(SharedString::from(format!("confirmation-{}", row.key)))
            .test_support()
            .w_full()
            .flex_none()
            .items_start()
            .gap_3()
            .px_2()
            .py_2p5()
            .border_b_1()
            .border_color(divider)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .child(
                div().pt(px(2.)).child(
                    Icon::new(row.section.icon())
                        .small()
                        .text_color(muted.opacity(0.7)),
                ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .text_sm()
                            .when(!expanded, |this| this.truncate())
                            .text_color(if on { fg } else { fg.opacity(0.85) })
                            .child(row.title.clone()),
                    )
                    .map(
                        |this| match (expanded, row.body.clone(), row.preview.clone()) {
                            (true, Some(body), _) => {
                                this.child(div().text_xs().text_color(muted).child(body))
                            }
                            (false, _, Some(preview)) => this
                                .child(div().text_xs().truncate().text_color(muted).child(preview)),
                            _ => this,
                        },
                    )
                    .when(expanded, |this| {
                        this.child(
                            div()
                                .pt_1()
                                .text_xs()
                                .text_color(muted.opacity(0.6))
                                .child(row.key.clone()),
                        )
                    }),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_3()
                    .when_some(answer.filter(|_| on), |this, answer| {
                        this.child(div().text_xs().text_color(muted).child(answer))
                    })
                    // The switch toggles; clicks elsewhere on the row expand it
                    .child(
                        div()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(
                                Switch::new(SharedString::from(format!("switch-{}", row.key)))
                                    .small()
                                    .checked(on)
                                    .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                        this.set(&switch_key, *checked, cx)
                                    })),
                            ),
                    )
                    .child(
                        Icon::new(if expanded {
                            IconName::ChevronUp
                        } else {
                            IconName::ChevronDown
                        })
                        .small()
                        .text_color(muted.opacity(0.6)),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_expanded(&row_key, cx);
            }))
    }

    // Turns every row in view on or off: the group, search and filter decide
    // what that is.
    fn set_all(&mut self, rows: &[Row], indices: &[usize], on: bool, cx: &mut Context<Self>) {
        if self.loading || self.saving || self.error.is_some() {
            return;
        }
        for &index in indices {
            let key = &rows[index].key;
            if on {
                self.selected.insert(key.clone());
            } else {
                self.selected.remove(key);
            }
        }
        cx.notify();
    }

    fn render_list(
        &mut self,
        rows: &Rc<Vec<Row>>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let visible: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| self.section.is_none_or(|s| r.section == s))
            .filter(|(_, r)| !self.suppressed_only || self.selected.contains(&r.key))
            .filter(|(_, r)| query.is_empty() || r.search_text.contains(&query))
            .map(|(index, _)| index)
            .collect();
        if !Rc::ptr_eq(rows, &self.listed_rows) || *self.visible_rows != visible {
            self.listed_rows = rows.clone();
            self.visible_rows = Rc::new(visible);
            self.list_hinted_width.set(None);
            // Collapsed rows have two text lines. The hint gives an accurate
            // scrollbar immediately without laying out the whole catalog.
            self.list_scroll
                .handle
                .reset_with_uniform_height(self.visible_rows.len(), px(65.));
            self.list_scroll.reset();
        }
        let muted = cx.theme().muted_foreground;

        let toolbar = h_flex()
            .flex_none()
            .gap_2()
            .child(
                div().flex_1().child(
                    Input::new(&self.search)
                        .id("confirmations-search")
                        .small()
                        .cleanable(true)
                        .prefix(Icon::new(IconName::Search).small().text_color(muted)),
                ),
            )
            .child(
                Button::new("confirmations-suppressed-only")
                    .outline()
                    .small()
                    .icon(IconName::BellOff)
                    .label(t!("confirmations.suppressedOnly").to_string())
                    .selected(self.suppressed_only)
                    .toggled(self.suppressed_only)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.suppressed_only = !this.suppressed_only;
                        this.list_scroll.reset();
                        cx.notify();
                    })),
            );

        let count = self.visible_rows.len();
        let on = self
            .visible_rows
            .iter()
            .filter(|&&index| self.selected.contains(&rows[index].key))
            .count();
        let (on_rows, off_rows) = (rows.clone(), rows.clone());
        let (on_indices, off_indices) = (self.visible_rows.clone(), self.visible_rows.clone());
        let bulk = h_flex()
            .flex_none()
            .gap_1()
            .px_2()
            .pb_1()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.5))
            .text_xs()
            .text_color(muted)
            .child(
                div()
                    .flex_1()
                    .child(t!("confirmations.shown", on = on, count = count).to_string()),
            )
            .child(
                Button::new("confirmations-all-on")
                    .ghost()
                    .xsmall()
                    .label(t!("confirmations.allOn").to_string())
                    .disabled(on == count)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_all(&on_rows, &on_indices, true, cx)
                    })),
            )
            .child(
                Button::new("confirmations-all-off")
                    .ghost()
                    .xsmall()
                    .label(t!("confirmations.allOff").to_string())
                    .disabled(on == 0)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_all(&off_rows, &off_indices, false, cx)
                    })),
            );

        let list = if self.visible_rows.is_empty() {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_2()
                .text_sm()
                .text_color(muted)
                .child(Icon::new(IconName::Search).large().text_color(muted))
                .child(t!("confirmations.empty").to_string())
                .into_any_element()
        } else {
            let rows = rows.clone();
            let visible = self.visible_rows.clone();
            let view = cx.entity();
            list(self.list_scroll.handle.clone(), move |index, _, cx| {
                view.update(cx, |this, cx| {
                    #[cfg(test)]
                    {
                        this.rows_rendered += 1;
                    }
                    this.render_row(&rows[visible[index]], cx)
                        .into_any_element()
                })
            })
            .size_full()
            .min_h_0()
            .pr_4()
            .into_any_element()
        };

        let list_state = self.list_scroll.handle.clone();
        let hinted_width = self.list_hinted_width.clone();
        // List clears every height hint during its first paint and after a
        // width change. Refill them after it lays out the visible rows, before
        // painting the scrollbar, so the thumb spans the entire catalog.
        let size_hints = canvas(
            move |_, _, _| {
                let width = list_state.viewport_bounds().size.width;
                if hinted_width.replace(Some(width)) != Some(width) {
                    list_state.clone().with_uniform_item_height(px(65.));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0();

        v_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .gap_3()
            .child(toolbar)
            .child(
                v_flex().flex_1().min_h_0().child(bulk).child(
                    v_flex()
                        .id("confirmations-body")
                        .test_support()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .child(list)
                        .child(size_hints)
                        .child(
                            self.list_scroll
                                .wheel_layer(|this| &mut this.list_scroll, cx),
                        )
                        .child(
                            Scrollbar::vertical(&self.list_scroll.handle)
                                .id("confirmations-body-scrollbar")
                                .mode(ScrollbarMode::Always),
                        ),
                ),
            )
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let changed = self.selected.symmetric_difference(&self.stored).count();
        let muted = cx.theme().muted_foreground;
        h_flex()
            .flex_none()
            .gap_2()
            .child(
                h_flex()
                    .flex_1()
                    .gap_2()
                    .text_xs()
                    .text_color(muted)
                    .child(Icon::new(IconName::BellOff).xsmall().text_color(muted))
                    .child(t!("confirmations.summary", count = self.selected.len()).to_string())
                    .when(changed > 0, |this| {
                        this.child(
                            div()
                                .px_1p5()
                                .rounded(px(4.))
                                .bg(ui::amber(cx).opacity(0.15))
                                .text_color(ui::amber(cx))
                                .child(t!("confirmations.changes", count = changed).to_string()),
                        )
                    }),
            )
            .child(
                Button::new("confirmations-reset")
                    .ghost()
                    .label(t!("formationEditor.reset").to_string())
                    .disabled(self.loading || self.saving || changed == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selected = this.stored.clone();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("confirmations-save")
                    .primary()
                    .label(t!("confirmations.save").to_string())
                    .loading(self.saving)
                    .disabled(self.loading || self.saving || self.error.is_some() || changed == 0)
                    .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
            )
    }
}

impl Render for ConfirmationsEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.rows();
        v_flex()
            .id("confirmations-editor")
            .test_support()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .p_4()
            .gap_4()
            .child(
                div()
                    .flex_none()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("confirmations.hint").to_string()),
            )
            .when(self.disk_changed || self.error.is_some(), |view| {
                view.child(
                    h_flex()
                        .flex_none()
                        .gap_2()
                        .child(
                            div().flex_1().text_xs().text_color(ui::amber(cx)).child(
                                self.error.clone().unwrap_or_else(|| {
                                    t!("formationEditor.fileChanged").to_string()
                                }),
                            ),
                        )
                        .child(
                            Button::new("confirmations-reload")
                                .outline()
                                .small()
                                .label(t!("formationEditor.reload").to_string())
                                .disabled(self.loading || self.saving)
                                .on_click(cx.listener(|this, _, window, cx| this.load(window, cx))),
                        ),
                )
            })
            .child(if self.loading {
                div()
                    .flex_1()
                    .child(t!("common.loading").to_string())
                    .into_any_element()
            } else {
                gpui_kit::component::resizable::h_resizable("confirmation-columns")
                    .child(ui::pane(Some(210.), self.render_sidebar(&rows, cx)))
                    .child(ui::pane(None, self.render_list(&rows, cx)))
                    .into_any_element()
            })
            .child(self.render_footer(cx))
    }
}

#[cfg(test)]
mod tests {
    use crate::store::Store;
    use eve_wrench_core::confirmations;
    use gpui_kit::base::ScrollbarHandle;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, TestAppContext, px, size};
    use rust_i18n::t;
    use std::collections::HashSet;

    #[gpui_kit::test]
    fn saved_confirmations_sync_only_accounts_and_preserve_other_settings(cx: &mut TestAppContext) {
        let (handle, main, files) = crate::main_window::editors_tests::setup(cx);
        cx.update_window(handle, |_, window, cx| {
            window.click("tab-confirmations", cx)
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.find("editor-account");
            assert!(window.try_find("copy-settings").is_none());
            assert!(
                window.find("confirmations-editor").bounds().bottom()
                    <= window.viewport_size().height
            );
            window.click("confirmations-search", cx);
            window.input("AskQuitGame", cx);
            window.render_frame(cx);
            window.click("switch-AskQuitGame", cx);
            window.click("confirmations-save", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            let editor = main.read(cx).confirmation_editors[&files.account].read(cx);
            assert!(!editor.has_pending_edits());
            window.render_frame(cx);
            window.click("sync-all", cx);
            window.render_frame(cx);
            window.click("sync-apply", cx);
        })
        .unwrap();
        cx.run_until_parked();
        for path in [&files.account, &files.target] {
            assert_eq!(
                confirmations::read_suppressed(std::path::Path::new(path)).unwrap(),
                HashSet::from(["AskQuitGame".to_string()])
            );
            let decoded = blue_marshal::decode(&std::fs::read(path).unwrap()).unwrap();
            assert_eq!(
                blue_marshal::to_json(&decoded.value)["bytes:ui"]["bytes:unrelated"]["tuple"][1],
                "utf8:keep"
            );
        }
        assert_eq!(
            eve_wrench_core::formations::read_probe_formations(std::path::Path::new(
                &files.account
            ))
            .unwrap()
            .len(),
            1
        );
        for path in [&files.character, &files.target_character] {
            assert!(
                confirmations::read_suppressed(std::path::Path::new(path))
                    .unwrap()
                    .is_empty()
            );
        }
        cx.update(|cx| assert_eq!(cx.windows().len(), 1));
    }

    #[gpui_kit::test]
    fn confirmation_drafts_survive_context_changes_and_block_updates_and_reload(
        cx: &mut TestAppContext,
    ) {
        let (handle, main, files) = crate::main_window::editors_tests::setup(cx);
        cx.update_window(handle, |_, window, cx| {
            window.click("tab-confirmations", cx)
        })
        .unwrap();
        cx.run_until_parked();
        let editor = cx.update(|cx| main.read(cx).confirmation_editors[&files.account].clone());
        cx.update_window(handle, |_, window, cx| {
            editor.update(cx, |editor, cx| editor.set("AskQuitGame", true, cx));
            main.update(cx, |main, cx| {
                main.update.preview = true;
                cx.notify();
            });
            window.render_frame(cx);
            window.click("update-button", cx);
            assert_eq!(
                main.read(cx).update.status.as_deref(),
                Some(t!("update.saveEdits").as_ref())
            );
            main.update(cx, |main, cx| {
                let data = main.store.read(cx).data().unwrap();
                let profile = main
                    .selected_profile(&data.servers[0])
                    .unwrap()
                    .path
                    .clone();
                main.editor_accounts.insert(profile, files.target.clone());
                cx.notify();
            });
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert_eq!(main.read(cx).confirmation_editors.len(), 2);
            assert!(editor.read(cx).selected.contains("AskQuitGame"));
            assert!(
                !main.read(cx).confirmation_editors[&files.target]
                    .read(cx)
                    .has_pending_edits()
            );
            main.update(cx, |main, cx| {
                main.editor_accounts.clear();
                main.tab = Some(crate::main_window::Tab::Drones);
                cx.notify();
            });
            window.render_frame(cx);
            main.update(cx, |main, cx| {
                main.tab = Some(crate::main_window::Tab::Confirmations);
                cx.notify();
            });
        })
        .unwrap();
        confirmations::write_suppressed(
            std::path::Path::new(&files.account),
            &HashSet::from(["PutOffline".to_string()]),
            false,
        )
        .unwrap();
        cx.update(|cx| Store::global(cx).update(cx, |store, cx| store.reload(cx)));
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            assert!(editor.read(cx).disk_changed);
            assert!(editor.read(cx).selected.contains("AskQuitGame"));
            window.render_frame(cx);
            window.click("confirmations-reload", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(!editor.read(cx).has_pending_edits());
            assert_eq!(
                editor.read(cx).selected,
                HashSet::from(["PutOffline".to_string()])
            );
        });
    }

    #[gpui_kit::test]
    fn confirmation_list_and_sidebar_scroll_smoothly_and_independently(cx: &mut TestAppContext) {
        let (handle, main, _files) = crate::main_window::editors_tests::setup(cx);
        cx.update_window(handle, |_, window, cx| {
            window.click("tab-confirmations", cx)
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            let editor = main
                .read(cx)
                .confirmation_editors
                .values()
                .next()
                .unwrap()
                .clone();

            crate::scrolling::tests::check_vertical(&editor, |e| &mut e.list_scroll, window, cx);
            assert_eq!(editor.read(cx).sidebar_scroll.handle.offset().y, px(0.));
        })
        .unwrap();
        cx.simulate_window_resize(handle, size(px(1440.), px(300.)));
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            let editor = main
                .read(cx)
                .confirmation_editors
                .values()
                .next()
                .unwrap()
                .clone();
            window.render_frame(cx);

            crate::scrolling::tests::check_vertical(&editor, |e| &mut e.sidebar_scroll, window, cx);
            assert_eq!(editor.read(cx).list_scroll.handle.offset().y, px(0.));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn all_dialogs_render_a_bounded_window_and_keep_expansion_and_bulk_actions(
        cx: &mut TestAppContext,
    ) {
        let (handle, main, _files) = crate::main_window::editors_tests::setup(cx);
        cx.update_window(handle, |_, window, cx| {
            window.click("tab-confirmations", cx)
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            let editor = main
                .read(cx)
                .confirmation_editors
                .values()
                .next()
                .unwrap()
                .clone();
            let (first, last, count, cache_builds) = {
                let editor = editor.read(cx);
                let count = editor.visible_rows.len();
                assert!(count > 100);
                (
                    editor.listed_rows[editor.visible_rows[0]].key.clone(),
                    editor.listed_rows[*editor.visible_rows.last().unwrap()]
                        .key
                        .clone(),
                    count,
                    editor.row_cache_builds,
                )
            };
            editor.update(cx, |editor, cx| {
                editor.rows_rendered = 0;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(
                editor.read(cx).rows_rendered < 40,
                "only rows near the viewport should be built"
            );
            assert!(
                editor.read(cx).list_scroll.handle.content_size().height > px(count as f32 * 40.),
                "the scrollbar must include the full catalog before offscreen rows are measured"
            );
            assert!(window.try_find(format!("confirmation-{last}")).is_none());
            let collapsed_height = window
                .find(format!("confirmation-{first}"))
                .bounds()
                .size
                .height;
            assert!(
                window
                    .find(format!("confirmation-{first}"))
                    .bounds()
                    .right()
                    <= window.find("confirmations-body").bounds().right(),
                "rows must fit the viewport so text wraps and clicks stay inside it"
            );
            window.click(format!("confirmation-{first}"), cx);
            window.render_frame(cx);
            assert!(
                window
                    .find(format!("confirmation-{first}"))
                    .bounds()
                    .size
                    .height
                    > collapsed_height,
                "row {first}: expanded={}, before={collapsed_height:?}, after={:?}",
                editor.read(cx).expanded.contains(&first),
                window.find(format!("confirmation-{first}")).bounds()
            );
            editor.update(cx, |editor, cx| {
                editor.rows_rendered = 0;
                cx.notify();
            });
            let bounds = editor.read(cx).list_scroll.handle.viewport_bounds();
            window.drag(
                gpui_kit::point(bounds.right() - px(7.), bounds.top() + px(10.)),
                gpui_kit::point(bounds.right() - px(7.), bounds.bottom() - px(1.)),
                cx,
            );
            editor.update(cx, |editor, cx| {
                editor.rows_rendered = 0;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(
                editor.read(cx).rows_rendered < 40,
                "redrawing at the end must not lay out intermediate rows"
            );
            window.find(format!("confirmation-{last}"));
            assert!(window.try_find(format!("confirmation-{first}")).is_none());
            editor.update(cx, |editor, cx| {
                editor.list_scroll.reset();
                cx.notify();
            });
            window.render_frame(cx);
            assert!(
                window
                    .find(format!("confirmation-{first}"))
                    .bounds()
                    .size
                    .height
                    > collapsed_height
            );
            assert_eq!(
                editor.read(cx).row_cache_builds,
                cache_builds,
                "scrolling and expanding must reuse cached text"
            );
            window.click("confirmations-all-on", cx);
            window.render_frame(cx);
            assert_eq!(
                editor.read(cx).selected.len(),
                count,
                "bulk controls apply to all filtered rows, including offscreen rows"
            );
            window.click("confirmations-all-off", cx);
            window.render_frame(cx);
            assert!(editor.read(cx).selected.is_empty());
        })
        .unwrap();
    }
}
