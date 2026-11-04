use crate::{
    scrolling::SmoothScroll,
    store::{Store, StoreEvent},
    ui,
};
use eve_wrench_core::{
    SettingsKind,
    display_settings::{SettingsDocument, boolean, integer, overview, scene::TargetLayout, text},
    window_layout::{Neocom, NeocomSide},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    assets::IconName,
    component::{
        ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        input::{Input, InputEvent, InputState},
        menu::{DropdownMenu as _, PopupMenuItem},
        switch::Switch,
        v_flex,
    },
    *,
};
use rust_i18n::t;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[path = "settings_overview.rs"]
mod overview_ui;
#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    Overview,
    Targets,
    Neocom,
    Fighters,
}
impl Page {
    const ALL: [Self; 4] = [Self::Overview, Self::Targets, Self::Neocom, Self::Fighters];
    fn id(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Targets => "targets",
            Self::Neocom => "neocom",
            Self::Fighters => "fighters",
        }
    }
}

#[derive(Clone, Debug)]
enum Field {
    NeocomWidth,
    FighterX,
    FighterY,
    TargetX,
    TargetY,
    TabName(String),
    TabColor(String),
    Width(i64, String),
}
impl Field {
    fn kind(&self) -> SettingsKind {
        match self {
            Self::NeocomWidth | Self::FighterX | Self::FighterY | Self::Width(..) => {
                SettingsKind::Char
            }
            _ => SettingsKind::User,
        }
    }
}

pub(super) struct DisplayEditor {
    store: Entity<Store>,
    pub(super) character: PathBuf,
    account: Option<PathBuf>,
    docs: HashMap<PathBuf, SettingsDocument>,
    loading: HashSet<PathBuf>,
    saving: HashSet<PathBuf>,
    error: Option<String>,
    status: Option<String>,
    page: Page,
    inputs: HashMap<String, Entity<InputState>>,
    invalid: HashMap<String, SettingsKind>,
    subscriptions: Vec<Subscription>,
    pub(super) scroll: SmoothScroll,
    pub(super) layout_dirty: bool,
    pub(super) target_count: usize,
    pub(super) screen: (i64, i64),
    pub(super) default_target_origin: [f64; 2],
    selected_tab: Option<String>,
    selected_preset: Option<String>,
    group_search: Entity<InputState>,
    group_page: usize,
    overview_section: usize,
    selected_groups_only: bool,
    appearance_background: bool,
}

impl DisplayEditor {
    pub(super) fn new(character: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::global(cx);
        let group_search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("display.searchGroups").to_string())
        });
        let subscriptions = vec![
            cx.subscribe_in(&store, window, |this, _, event, window, cx| {
                if matches!(event, StoreEvent::DataChanged) {
                    let paths: Vec<_> = this.docs.keys().cloned().collect();
                    for path in paths {
                        this.load(path, false, window, cx);
                    }
                }
            }),
            cx.subscribe(&group_search, |this, _, _: &InputEvent, cx| {
                this.group_page = 0;
                cx.notify();
            }),
        ];
        let mut result = Self {
            store,
            character: character.clone(),
            account: None,
            docs: HashMap::new(),
            loading: HashSet::new(),
            saving: HashSet::new(),
            error: None,
            status: None,
            page: Page::Overview,
            inputs: HashMap::new(),
            invalid: HashMap::new(),
            subscriptions,
            scroll: Default::default(),
            layout_dirty: false,
            target_count: 6,
            screen: (1920, 1080),
            default_target_origin: [1620.0 / 1920.0, 16.0 / 1080.0],
            selected_tab: None,
            selected_preset: None,
            group_search,
            group_page: 0,
            overview_section: 0,
            selected_groups_only: true,
            appearance_background: false,
        };
        result.load(character, false, window, cx);
        result
    }
    fn path(&self, kind: SettingsKind) -> Option<&PathBuf> {
        if kind == SettingsKind::Char {
            Some(&self.character)
        } else {
            self.account.as_ref()
        }
    }
    pub(super) fn doc(&self, kind: SettingsKind) -> Option<&SettingsDocument> {
        self.path(kind).and_then(|p| self.docs.get(p))
    }
    fn doc_mut(&mut self, kind: SettingsKind) -> Result<&mut SettingsDocument, String> {
        let path = self
            .path(kind)
            .cloned()
            .ok_or_else(|| t!("display.chooseAccount").to_string())?;
        self.docs
            .get_mut(&path)
            .ok_or_else(|| t!("formationEditor.loading").to_string())
    }
    pub(super) fn pending(&self) -> bool {
        self.docs.values().any(SettingsDocument::is_dirty)
            || !self.invalid.is_empty()
            || !self.saving.is_empty()
    }
    pub(super) fn neocom(&self, fallback: Neocom) -> Neocom {
        let Some(doc) = self.doc(SettingsKind::Char) else {
            return fallback;
        };
        Neocom {
            width: doc
                .value("ui", "neocomWidth")
                .and_then(integer)
                .unwrap_or(fallback.width),
            side: match doc.value("ui", "neocomAlign").and_then(integer) {
                Some(6) => NeocomSide::Right,
                Some(5) => NeocomSide::Left,
                _ => fallback.side,
            },
        }
    }
    pub(super) fn targets(&self) -> TargetLayout {
        self.doc(SettingsKind::User)
            .map(TargetLayout::read)
            .unwrap_or_default()
    }
    pub(super) fn open_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.scroll.reset();
        cx.notify();
    }
    pub(super) fn geometry_context(
        &mut self,
        screen: (i64, i64),
        origin: [f64; 2],
        dirty: bool,
        cx: &mut Context<Self>,
    ) {
        if screen != self.screen
            && self
                .doc(SettingsKind::Char)
                .and_then(|d| d.value("ui", "fightersDetachedPosition"))
                .is_none()
            && !self.invalid_for(SettingsKind::Char)
        {
            self.inputs
                .retain(|id, _| !id.ends_with("FighterX") && !id.ends_with("FighterY"));
        }
        if origin != self.default_target_origin
            && self
                .doc(SettingsKind::User)
                .and_then(|d| d.value("ui", "targetOrigin"))
                .is_none()
            && !self.invalid_for(SettingsKind::User)
        {
            let prefix = self
                .account
                .as_ref()
                .map(|p| format!("{}-", p.to_string_lossy()))
                .unwrap_or_default();
            self.inputs.retain(|id, _| {
                !id.starts_with(&prefix) || (!id.ends_with("TargetX") && !id.ends_with("TargetY"))
            });
        }
        self.screen = screen;
        self.default_target_origin = origin;
        self.layout_dirty = dirty;
        cx.notify();
    }
    fn load(&mut self, path: PathBuf, discard: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading.contains(&path) || self.saving.contains(&path) {
            return;
        }
        let old = if discard {
            None
        } else {
            self.docs.get(&path).cloned()
        };
        let prefix = format!("{}-", path.to_string_lossy());
        let was_dirty = old.as_ref().is_some_and(SettingsDocument::is_dirty)
            || (!discard && self.invalid.keys().any(|key| key.starts_with(&prefix)));
        self.loading.insert(path.clone());
        cx.spawn_in(window, async move |this, cx| {
            let task_path = path.clone();
            let result = cx
                .background_spawn(async move {
                    if let Some(old) = old {
                        old.refresh()
                    } else {
                        SettingsDocument::load(&task_path)
                    }
                })
                .await;
            this.update_in(cx, |this, _, cx| {
                this.loading.remove(&path);
                match result {
                    Ok(doc) => {
                        this.docs.insert(path, doc);
                        if !was_dirty {
                            this.inputs.retain(|key, _| !key.starts_with(&prefix));
                            this.invalid.retain(|key, _| !key.starts_with(&prefix));
                        }
                        this.error = None;
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    fn save(&mut self, kind: SettingsKind, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.path(kind).cloned() else {
            return;
        };
        if self.loading.contains(&path)
            || self.saving.contains(&path)
            || self.invalid_for(kind)
            || (kind == SettingsKind::Char && self.layout_dirty)
        {
            return;
        }
        let Some(doc) = self
            .docs
            .get(&path)
            .cloned()
            .filter(SettingsDocument::is_dirty)
        else {
            return;
        };
        self.saving.insert(path.clone());
        self.error = None;
        self.status = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx.background_spawn(async move { doc.save() }).await;
            this.update_in(cx, |this, _, cx| {
                this.saving.remove(&path);
                match result {
                    Ok((doc, _)) => {
                        this.docs.insert(path, doc);
                        this.status = Some(
                            t!(if kind == SettingsKind::User {
                                "display.accountSaved"
                            } else {
                                "display.characterSaved"
                            })
                            .to_string(),
                        );
                        this.store.update(cx, |store, cx| store.reload(cx));
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    fn edit(
        &mut self,
        kind: SettingsKind,
        section: &str,
        key: &str,
        edit: impl FnOnce(&mut Value),
        cx: &mut Context<Self>,
    ) {
        self.status = None;
        let result = (|| {
            let doc = self.doc_mut(kind)?;
            let mut value = doc.value(section, key).cloned().unwrap_or(json!({}));
            edit(&mut value);
            doc.set(section, key, value)
        })();
        if let Err(error) = result {
            self.error = Some(error);
        } else {
            self.error = None;
        }
        cx.notify();
    }
    fn set(
        &mut self,
        kind: SettingsKind,
        section: &str,
        key: &str,
        value: Value,
        cx: &mut Context<Self>,
    ) {
        self.status = None;
        let result = self
            .doc_mut(kind)
            .and_then(|doc| doc.set(section, key, value));
        if let Err(error) = result {
            self.error = Some(error);
        } else {
            self.error = None;
        }
        cx.notify();
    }
    fn field_value(&self, field: &Field) -> String {
        let doc = self.doc(field.kind());
        match field {
            Field::NeocomWidth => doc
                .and_then(|d| d.value("ui", "neocomWidth"))
                .and_then(integer)
                .unwrap_or(48)
                .to_string(),
            Field::FighterX | Field::FighterY => {
                let i = if matches!(field, Field::FighterX) {
                    0
                } else {
                    1
                };
                doc.and_then(|d| d.value("ui", "fightersDetachedPosition"))
                    .and_then(|v| v.get("tuple"))
                    .and_then(|v| v.get(i))
                    .and_then(integer)
                    .unwrap_or(if i == 0 {
                        self.screen.0 / 2
                    } else {
                        self.screen.1 / 2 - 60
                    })
                    .to_string()
            }
            Field::TargetX | Field::TargetY => {
                let targets = self.targets();
                let i = if matches!(field, Field::TargetX) {
                    0
                } else {
                    1
                };
                format!(
                    "{:.2}",
                    targets.origin.unwrap_or(self.default_target_origin)[i] * 100.
                )
            }
            Field::TabName(tab) => doc
                .and_then(|d| d.value("overview", "tabsettings_new"))
                .and_then(|v| v.get(tab))
                .and_then(|v| v.get("bytes:name"))
                .and_then(text)
                .unwrap_or("")
                .to_owned(),
            Field::TabColor(tab) => {
                let color = doc
                    .and_then(|d| overview::tabs(d).into_iter().find(|t| &t.key == tab))
                    .and_then(|t| t.color)
                    .unwrap_or([0.8, 0.8, 0.8]);
                format!(
                    "{:02X}{:02X}{:02X}",
                    (color[0] * 255.).round() as u8,
                    (color[1] * 255.).round() as u8,
                    (color[2] * 255.).round() as u8
                )
            }
            Field::Width(tab, column) => doc
                .map(|d| overview::column_width(d, *tab, column))
                .unwrap_or(100)
                .to_string(),
        }
    }
    fn apply_field(&mut self, field: &Field, value: &str) -> Result<(), String> {
        let screen = self.screen;
        let default_origin = self.default_target_origin;
        let doc = self.doc_mut(field.kind())?;
        let bad = || t!("display.invalidValue").to_string();
        match field {
            Field::NeocomWidth => doc.set(
                "ui",
                "neocomWidth",
                json!(value.parse::<i64>().map_err(|_| bad())?),
            ),
            Field::FighterX | Field::FighterY => {
                let mut position = doc
                    .value("ui", "fightersDetachedPosition")
                    .cloned()
                    .unwrap_or(json!({"tuple":[screen.0/2,screen.1/2-60]}));
                let coords = position
                    .get_mut("tuple")
                    .and_then(Value::as_array_mut)
                    .filter(|a| a.len() == 2)
                    .ok_or_else(bad)?;
                coords[if matches!(field, Field::FighterX) {
                    0
                } else {
                    1
                }] = json!(value.parse::<i64>().map_err(|_| bad())?);
                doc.set("ui", "fightersDetachedPosition", position)
            }
            Field::TargetX | Field::TargetY => {
                let mut origin = doc
                    .value("ui", "targetOrigin")
                    .cloned()
                    .unwrap_or(json!({"tuple":default_origin}));
                let coords = origin
                    .get_mut("tuple")
                    .and_then(Value::as_array_mut)
                    .filter(|a| a.len() == 2)
                    .ok_or_else(bad)?;
                coords[if matches!(field, Field::TargetX) {
                    0
                } else {
                    1
                }] = json!(value.parse::<f64>().map_err(|_| bad())? / 100.);
                doc.set("ui", "targetOrigin", origin)
            }
            Field::TabName(tab) => {
                overview::set_tab(doc, tab, "name", json!(format!("utf8:{value}")))
            }
            Field::TabColor(tab) => {
                let rgb = parse_color(value).ok_or_else(bad)?;
                overview::set_tab(doc, tab, "color", json!({"tuple":rgb}))
            }
            Field::Width(tab, column) => {
                overview::set_width(doc, *tab, column, value.parse().map_err(|_| bad())?)
            }
        }
    }
    fn input(
        &mut self,
        field: Field,
        label: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let kind = field.kind();
        let element_id = format!("display-field-{field:?}");
        let id = format!(
            "{}-{field:?}",
            self.path(field.kind())
                .map(|p| p.to_string_lossy())
                .unwrap_or_default()
        );
        if !self.inputs.contains_key(&id) {
            let value = self.field_value(&field);
            let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
            let key = id.clone();
            let kind = field.kind();
            self.subscriptions.push(cx.subscribe(
                &input,
                move |this, input, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.status = None;
                        let value = input.read(cx).value();
                        match this.apply_field(&field, value.trim()) {
                            Ok(()) => {
                                this.invalid.remove(&key);
                                this.error = None;
                            }
                            Err(error) => {
                                this.invalid.insert(key.clone(), kind);
                                this.error = Some(error);
                            }
                        }
                        cx.notify();
                    }
                },
            ));
            self.inputs.insert(id.clone(), input);
        }
        h_flex()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(if self.invalid.contains_key(&id) {
                        cx.theme().danger
                    } else {
                        cx.theme().foreground
                    })
                    .child(label),
            )
            .child(
                div().w(px(122.)).child(
                    Input::new(&self.inputs[&id])
                        .id(SharedString::from(element_id))
                        .small()
                        .disabled(
                            self.doc(kind).is_none()
                                || self.path(kind).is_some_and(|p| self.saving.contains(p)),
                        ),
                ),
            )
            .into_any_element()
    }
    fn account_toggle(
        &self,
        id: &str,
        label: String,
        checked: bool,
        key: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        h_flex()
            .gap_2()
            .child(div().flex_1().text_sm().child(label))
            .child(
                Switch::new(SharedString::from(id.to_owned()))
                    .small()
                    .checked(checked)
                    .on_click(cx.listener(move |this, value: &bool, _, cx| {
                        this.set(SettingsKind::User, "ui", key, json!(*value), cx)
                    })),
            )
            .into_any_element()
    }
    fn render_targets(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.doc(SettingsKind::User).is_none() {
            return hint("display.chooseAccount", cx);
        }
        let horizontal = self.targets().horizontal;
        v_flex()
            .gap_3()
            .child(hint("display.targetsHint", cx))
            .child(self.input(
                Field::TargetX,
                t!("display.originX").to_string(),
                window,
                cx,
            ))
            .child(self.input(
                Field::TargetY,
                t!("display.originY").to_string(),
                window,
                cx,
            ))
            .child(self.account_toggle(
                "targets-horizontal",
                t!("display.horizontal").to_string(),
                horizontal,
                "alignHorizontally",
                cx,
            ))
            .child(
                self.account_toggle(
                    "target-anchor-locked",
                    t!("display.anchorLocked").to_string(),
                    self.doc(SettingsKind::User)
                        .and_then(|d| d.value("ui", "targetOriginLocked"))
                        .and_then(boolean)
                        .unwrap_or(false),
                    "targetOriginLocked",
                    cx,
                ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .child(t!("display.targetCount").to_string()),
                    )
                    .child(
                        Button::new("target-count-less")
                            .ghost()
                            .small()
                            .label("−")
                            .disabled(self.target_count <= 1)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.target_count = this.target_count.saturating_sub(1).max(1);
                                cx.notify();
                            })),
                    )
                    .child(self.target_count.to_string())
                    .child(
                        Button::new("target-count-more")
                            .ghost()
                            .small()
                            .label("+")
                            .disabled(self.target_count >= 12)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.target_count = (this.target_count + 1).min(12);
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
    fn render_fighters(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap_3()
            .child(hint("display.fightersHint", cx))
            .child(self.input(
                Field::FighterX,
                t!("display.positionX").to_string(),
                window,
                cx,
            ))
            .child(self.input(
                Field::FighterY,
                t!("display.positionY").to_string(),
                window,
                cx,
            ))
            .when(self.doc(SettingsKind::User).is_some(), |this| {
                this.child(
                    self.account_toggle(
                        "fighters-detached",
                        t!("display.detachFighters").to_string(),
                        self.doc(SettingsKind::User)
                            .and_then(|d| d.value("ui", "detachFighterUI"))
                            .and_then(boolean)
                            .unwrap_or(false),
                        "detachFighterUI",
                        cx,
                    ),
                )
            })
            .into_any_element()
    }
    fn render_neocom(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(doc) = self.doc(SettingsKind::Char) else {
            return hint("formationEditor.loading", cx);
        };
        let right = doc.value("ui", "neocomAlign").and_then(integer) == Some(6);
        let buttons = doc
            .value("ui", "neocomButtonRawData")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let rows: Vec<_> = buttons
            .iter()
            .enumerate()
            .map(|(i, button)| {
                let id = button
                    .get("instance")
                    .and_then(|v| v.get("state"))
                    .and_then(|v| v.get("bytes:id"))
                    .and_then(text)
                    .unwrap_or("?");
                let custom = button
                    .get("instance")
                    .and_then(|v| v.get("state"))
                    .and_then(|v| v.get("bytes:label"))
                    .and_then(text)
                    .filter(|s| !s.starts_with("UI/"));
                h_flex()
                    .gap_2()
                    .child(
                        div().flex_1().text_sm().child(
                            custom
                                .map(str::to_owned)
                                .unwrap_or_else(|| super::model::window_name(id)),
                        ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("neocom-up-{i}")))
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowUp)
                            .disabled(i == 0)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.move_button(i, -1, cx)),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("neocom-down-{i}")))
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowDown)
                            .disabled(i + 1 >= buttons.len())
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.move_button(i, 1, cx)),
                            ),
                    )
            })
            .collect();
        v_flex()
            .gap_3()
            .child(hint("display.neocomHint", cx))
            .child(self.input(
                Field::NeocomWidth,
                t!("display.neocomWidth").to_string(),
                window,
                cx,
            ))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .child(t!("display.side").to_string()),
                    )
                    .children([false, true].into_iter().map(|pick| {
                        Button::new(if pick { "neocom-right" } else { "neocom-left" })
                            .outline()
                            .small()
                            .selected(right == pick)
                            .label(
                                if pick {
                                    t!("display.right")
                                } else {
                                    t!("display.left")
                                }
                                .to_string(),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set(
                                    SettingsKind::Char,
                                    "ui",
                                    "neocomAlign",
                                    json!(if pick { 6 } else { 5 }),
                                    cx,
                                )
                            }))
                    })),
            )
            .child(ui::section_divider(
                t!("display.buttonOrder").to_string(),
                None,
                cx,
            ))
            .children(rows)
            .into_any_element()
    }
    fn move_button(&mut self, index: usize, delta: isize, cx: &mut Context<Self>) {
        self.edit(
            SettingsKind::Char,
            "ui",
            "neocomButtonRawData",
            move |v| {
                if let Some(list) = v.as_array_mut() {
                    let dest = index.saturating_add_signed(delta);
                    if index < list.len() && dest < list.len() {
                        list.swap(index, dest);
                    }
                }
            },
            cx,
        );
    }
    fn invalid_for(&self, kind: SettingsKind) -> bool {
        self.path(kind).is_some_and(|path| {
            let prefix = format!("{}-", path.to_string_lossy());
            self.invalid
                .iter()
                .any(|(key, k)| *k == kind && key.starts_with(&prefix))
        })
    }
    pub(super) fn select_account(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.status = None;
        self.account = Some(path.clone());
        self.selected_tab = None;
        self.selected_preset = None;
        self.load(path, false, window, cx);
        cx.notify();
    }
    fn accounts(&self, cx: &App) -> Vec<(PathBuf, String)> {
        self.store
            .read(cx)
            .data()
            .into_iter()
            .flat_map(|d| {
                d.entries()
                    .filter(|e| {
                        e.kind == SettingsKind::User
                            && PathBuf::from(&e.path).parent() == self.character.parent()
                    })
                    .map(|e| {
                        (
                            PathBuf::from(&e.path),
                            format!("{} ({})", e.sort_name(), e.id),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

pub(super) fn parse_color(value: &str) -> Option<[f64; 3]> {
    let value = value.trim_start_matches('#');
    if value.len() != 6 || !value.is_ascii() {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()? as f64 / 255.,
        u8::from_str_radix(&value[2..4], 16).ok()? as f64 / 255.,
        u8::from_str_radix(&value[4..6], 16).ok()? as f64 / 255.,
    ])
}
fn hint(key: &str, cx: &App) -> AnyElement {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(t!(key).to_string())
        .into_any_element()
}

impl Render for DisplayEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accounts = self.accounts(cx);
        let selected = self.account.clone();
        let view = cx.entity().downgrade();
        let account_label = selected
            .as_ref()
            .and_then(|p| accounts.iter().find(|(q, _)| p == q))
            .map(|(_, n)| n.clone())
            .unwrap_or_else(|| t!("display.chooseAccount").to_string());
        let content = match self.page {
            Page::Overview => self.render_overview(window, cx),
            Page::Targets => self.render_targets(window, cx),
            Page::Neocom => self.render_neocom(window, cx),
            Page::Fighters => self.render_fighters(window, cx),
        };
        let saves = [SettingsKind::User, SettingsKind::Char]
            .into_iter()
            .map(|kind| {
                let path = self.path(kind);
                let dirty = self.doc(kind).is_some_and(SettingsDocument::is_dirty);
                let busy =
                    path.is_some_and(|p| self.loading.contains(p) || self.saving.contains(p));
                h_flex()
                    .gap_1()
                    .child(
                        Button::new(if kind == SettingsKind::Char {
                            "display-save-character"
                        } else {
                            "display-save-account"
                        })
                        .outline()
                        .small()
                        .flex_1()
                        .label(
                            if kind == SettingsKind::Char {
                                t!("display.saveCharacter")
                            } else {
                                t!("display.saveAccount")
                            }
                            .to_string(),
                        )
                        .disabled(
                            !dirty
                                || busy
                                || self.invalid_for(kind)
                                || (kind == SettingsKind::Char && self.layout_dirty),
                        )
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.save(kind, window, cx)),
                        ),
                    )
                    .child(
                        Button::new(if kind == SettingsKind::Char {
                            "display-reload-character"
                        } else {
                            "display-reload-account"
                        })
                        .ghost()
                        .small()
                        .icon(IconName::RotateCcw)
                        .tooltip(t!("display.discardReload").to_string())
                        .disabled(path.is_none() || busy)
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                if let Some(path) = this.path(kind).cloned() {
                                    this.load(path, true, window, cx);
                                }
                            },
                        )),
                    )
            })
            .collect::<Vec<_>>();
        v_flex()
            .id("display-settings")
            .test_support()
            // These files have separate save buttons and no layout undo history.
            .on_action(|_: &super::Undo, _, cx| cx.stop_propagation())
            .on_action(|_: &super::Redo, _, cx| cx.stop_propagation())
            .on_action(|_: &super::Save, _, cx| cx.stop_propagation())
            .w(px(392.))
            .flex_none()
            .min_h_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .child(
                v_flex()
                    .flex_none()
                    .p_3()
                    .gap_2()
                    .child(
                        Button::new("display-account")
                            .outline()
                            .small()
                            .w_full()
                            .label(account_label)
                            .tooltip(t!("display.accountHint").to_string())
                            .dropdown_menu(move |menu, _, _| {
                                accounts.iter().fold(
                                    menu.max_h(px(300.)).scrollable(true),
                                    |menu, (path, name)| {
                                        let path = path.clone();
                                        let current = selected.as_ref() == Some(&path);
                                        let view = view.clone();
                                        menu.item(
                                            PopupMenuItem::new(name.clone())
                                                .checked(current)
                                                .on_click(move |_, window, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.select_account(
                                                            path.clone(),
                                                            window,
                                                            cx,
                                                        );
                                                    })
                                                    .ok();
                                                }),
                                        )
                                    },
                                )
                            }),
                    )
                    .child(h_flex().gap_1().children(Page::ALL.into_iter().map(|page| {
                        Button::new(SharedString::from(format!("display-page-{}", page.id())))
                            .ghost()
                            .xsmall()
                            .flex_1()
                            .selected(self.page == page)
                            .label(t!(format!("display.pages.{}", page.id())).to_string())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.page = page;
                                this.scroll.reset();
                                cx.notify();
                            }))
                    }))),
            )
            .child(
                self.scroll
                    .view(
                        "display-settings-list",
                        v_flex().p_3().gap_3().child(content),
                        |this| &mut this.scroll,
                        cx,
                    )
                    .flex_1(),
            )
            .child(
                v_flex()
                    .flex_none()
                    .p_3()
                    .gap_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .when(self.layout_dirty, |this| {
                        this.child(hint("display.saveLayoutFirst", cx))
                    })
                    .when_some(self.error.clone(), |this, error| {
                        this.child(div().text_xs().text_color(cx.theme().danger).child(error))
                    })
                    .when_some(self.status.clone(), |this, status| {
                        this.child(div().text_xs().text_color(cx.theme().primary).child(status))
                    })
                    .child(hint("display.saveHint", cx))
                    .children(saves),
            )
    }
}
