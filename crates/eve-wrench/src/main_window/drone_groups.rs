use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use eve_wrench_core::SettingsEntry;
use eve_wrench_core::drone_groups::{
    self, DEFAULT_PATTERN, DroneCategory, DroneGroupOrder, DroneGroupPreset, PATTERN_STYLES,
    PATTERN_TOKENS, PatternError,
};
use eve_wrench_core::esi;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use rust_i18n::t;

use super::MainView;
use crate::store::StoreEvent;
use crate::ui::{self, StyleExt as _};

#[derive(Clone)]
struct DroneGroupDraft {
    selected: HashSet<String>,
    pattern: String,
    order: DroneGroupOrder,
}

impl MainView {
    pub(super) fn create_drone_editor(
        &self,
        entry: SettingsEntry,
        present: Option<HashSet<String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<DroneGroupsEditor> {
        let main = cx.entity().downgrade();
        let pattern = self
            .store
            .read(cx)
            .config()
            .drone_group_pattern
            .clone()
            .unwrap_or_else(|| DEFAULT_PATTERN.to_string());
        let pattern_input = cx.new(|cx| InputState::new(window, cx).default_value(pattern));
        let saved_pattern = pattern_input.read(cx).value().to_string();
        let order = self.store.read(cx).config().drone_group_order.clone();
        let store = self.store.clone();
        let loading = present.is_none();
        let present = present.unwrap_or_default();
        let editor = cx.new(|cx: &mut Context<DroneGroupsEditor>| DroneGroupsEditor {
            path: PathBuf::from(&entry.path),
            step: Step::Groups,
            scroll: crate::scrolling::SmoothScroll::default(),
            selected: present.clone(),
            present,
            main,
            saved_pattern,
            saved_order: order.clone(),
            order,
            disk_changed: false,
            loading,
            error: None,
            saving: false,
            icons: HashMap::new(),
            _subscription: cx.subscribe(&pattern_input, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
            _store_subscription: cx.subscribe_in(&store, window, |this, _, event, window, cx| {
                if matches!(event, StoreEvent::DataChanged) && !this.saving && !this.loading {
                    if this.is_dirty(cx) {
                        this.disk_changed = true;
                        cx.notify();
                    } else {
                        this.load(window, cx);
                    }
                }
            }),
            pattern_input,
        });
        if loading {
            editor.update(cx, |editor, cx| editor.load(window, cx));
        }
        if !cfg!(test) {
            let type_ids: Vec<i64> = drone_groups::catalog()
                .iter()
                .filter_map(|p| p.type_id)
                .collect();
            let cache_dir = self.store.read(cx).locations().data_dir().join("icons");
            let view = editor.downgrade();
            cx.spawn(async move |_, cx| {
                let icons = cx
                    .background_spawn(async move { esi::type_icons(&type_ids, &cache_dir) })
                    .await;
                view.update(cx, |editor, cx| {
                    editor.icons = icons
                        .into_iter()
                        .map(|(id, png)| {
                            (
                                id,
                                Arc::new(Image::from_bytes(ImageFormat::Png, png.to_vec())),
                            )
                        })
                        .collect();
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        editor
    }

    fn prompt_drone_group_save(
        &mut self,
        path: PathBuf,
        draft: DroneGroupDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let main = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let choices = [
                (
                    "drone-save-update",
                    "droneGroups.updateNames",
                    drone_groups::DroneSaveMode::Update,
                ),
                (
                    "drone-save-append",
                    "droneGroups.append",
                    drone_groups::DroneSaveMode::Append,
                ),
                (
                    "drone-save-replace",
                    "droneGroups.replace",
                    drone_groups::DroneSaveMode::Replace,
                ),
            ];
            dialog
                .title(t!("droneGroups.saveModeTitle").to_string())
                .child(
                    v_flex()
                        .gap_4()
                        .child(t!("droneGroups.saveModeHint").to_string())
                        .child(
                            h_flex()
                                .flex_wrap()
                                .gap_2()
                                .children(choices.into_iter().map(|(id, label, mode)| {
                                    let main = main.clone();
                                    let path = path.clone();
                                    let draft = draft.clone();
                                    Button::new(id)
                                        .outline()
                                        .when(
                                            matches!(mode, drone_groups::DroneSaveMode::Replace),
                                            |button| button.danger(),
                                        )
                                        .label(t!(label).to_string())
                                        .on_click(move |_, window, cx| {
                                            window.close_dialog(cx);
                                            main.update(cx, |main, cx| {
                                                main.save_drone_groups(
                                                    path.clone(),
                                                    draft.clone(),
                                                    mode,
                                                    window,
                                                    cx,
                                                )
                                            })
                                            .ok();
                                        })
                                })),
                        ),
                )
        });
    }

    fn save_drone_groups(
        &mut self,
        path: PathBuf,
        draft: DroneGroupDraft,
        mode: drone_groups::DroneSaveMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let DroneGroupDraft {
            selected,
            pattern,
            order,
        } = draft;
        let backup = self.store.read(cx).config().auto_backup;
        let stored = (pattern != DEFAULT_PATTERN).then(|| pattern.clone());
        if let Err(e) = self.store.update(cx, |store, cx| {
            store.update_config(cx, |config| {
                config.drone_group_pattern = stored;
                config.drone_group_order = order.clone();
            })
        }) {
            ui::notify_error(t!("toast.updateSettingFailed"), e, window, cx);
            return;
        }
        let key = path.to_string_lossy().into_owned();
        if let Some(editor) = self.drone_editors.get(&key) {
            editor.update(cx, |editor, cx| {
                editor.saving = true;
                cx.notify();
            });
        }
        let source = key.clone();
        let saved_selected = selected.clone();
        let saved_pattern = pattern.clone();
        let saved_order = order.clone();
        self.run_job(
            window,
            cx,
            move |_| {
                drone_groups::write_drone_groups_with_order(
                    &path, &selected, &pattern, &order, mode, backup,
                )?;
                drone_groups::read_drone_groups_with_order(&path, &pattern, &order)
            },
            move |main, result, window, cx| {
                if let Some(editor) = main.drone_editors.get(&key) {
                    editor.update(cx, |editor, cx| {
                        editor.saving = false;
                        if let Ok(present) = &result {
                            editor.present = present.clone();
                            if editor.selected == saved_selected {
                                editor.selected = present.clone();
                            }
                            editor.saved_pattern = saved_pattern.clone();
                            editor.saved_order = saved_order.clone();
                            editor.disk_changed = false;
                        }
                        cx.notify();
                    });
                }
                match result {
                    Ok(present) => {
                        ui::notify_success(
                            t!("toast.droneGroupsSaved"),
                            t!("toast.droneGroupsSavedDesc", count = present.len()),
                            window,
                            cx,
                        );
                        main.prompt_editor_sync(
                            source.clone(),
                            super::editors::EditorSyncKind::Drones(mode),
                            window,
                            cx,
                        );
                    }
                    Err(e) => ui::notify_error(t!("toast.droneGroupsSaveFailed"), e, window, cx),
                }
            },
        );
    }
}

const CATEGORIES: [DroneCategory; 8] = drone_groups::DEFAULT_GROUP_ORDER;

// Examples for the naming step: a drone, a sentry and an EWAR group
const EXAMPLE_IDS: [&str; 3] = ["hobgoblin", "garde", "light-ecm"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Groups,
    Naming,
    Review,
}

impl Step {
    const ALL: [Step; 3] = [Step::Groups, Step::Naming, Step::Review];

    fn id(self) -> &'static str {
        match self {
            Step::Groups => "groups",
            Step::Naming => "naming",
            Step::Review => "review",
        }
    }
}

pub(super) struct DroneGroupsEditor {
    saved_pattern: String,
    saved_order: DroneGroupOrder,
    order: DroneGroupOrder,
    disk_changed: bool,
    loading: bool,
    saving: bool,
    error: Option<String>,
    path: PathBuf,
    step: Step,
    scroll: crate::scrolling::SmoothScroll,
    // Presets the account has now, and the ones it will have
    present: HashSet<String>,
    selected: HashSet<String>,
    main: WeakEntity<MainView>,
    pattern_input: Entity<InputState>,
    icons: HashMap<i64, Arc<Image>>,
    _subscription: Subscription,
    _store_subscription: Subscription,
}

impl DroneGroupsEditor {
    pub(super) fn is_dirty(&self, cx: &App) -> bool {
        self.selected != self.present
            || self.pattern(cx) != self.saved_pattern
            || self.order != self.saved_order
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.loading = true;
        self.error = None;
        self.disk_changed = false;
        let path = self.path.clone();
        let pattern = self.saved_pattern.clone();
        let order = self.saved_order.clone();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    drone_groups::read_drone_groups_with_order(&path, &pattern, &order)
                })
                .await;
            this.update_in(cx, |this, _, cx| {
                this.loading = false;
                match result {
                    Ok(present) => {
                        this.present = present.clone();
                        this.selected = present;
                    }
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn pattern(&self, cx: &App) -> String {
        self.pattern_input.read(cx).value().to_string()
    }

    fn icon(&self, preset: &DroneGroupPreset, size: Pixels, cx: &App) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let placeholder = move || {
            Icon::new(IconName::Drone)
                .small()
                .text_color(muted)
                .into_any_element()
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(size)
            .rounded(px(4.))
            .overflow_hidden()
            .bg(cx.theme().muted)
            .map(
                |this| match preset.type_id.and_then(|id| self.icons.get(&id)) {
                    Some(image) => this.child(
                        img(image.clone())
                            .size_full()
                            .object_fit(ObjectFit::Cover)
                            .with_fallback(placeholder),
                    ),
                    None => this.child(placeholder()),
                },
            )
            .into_any_element()
    }

    fn render_steps(&self, invalid: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let labels = Step::ALL.into_iter().enumerate().map(|(i, step)| {
            format!(
                "{}. {}",
                i + 1,
                t!(format!("droneGroups.steps.{}", step.id()))
            )
        });
        let current = Step::ALL.iter().position(|s| *s == self.step);
        let view = cx.entity().downgrade();
        h_flex().child(
            ui::segmented(
                "drone-steps",
                labels,
                current,
                move |i, _, cx| {
                    let step = Step::ALL[i];
                    if step == Step::Review && invalid {
                        return;
                    }
                    view.update(cx, |this, cx| {
                        this.step = step;
                        this.scroll.reset();
                        cx.notify();
                    })
                    .ok();
                },
                cx,
            )
            .flex_none()
            .w(px(360.)),
        )
    }

    // Uppercase title, an optional count and a hairline running to the edge,
    // in the style of the main window's section headers.
    fn section_header(title: String, count: Option<String>, cx: &App) -> Div {
        let theme = cx.theme();
        h_flex()
            .gap_2()
            .py_1()
            .text_xs()
            .font_medium()
            .text_color(theme.muted_foreground)
            .child(ui::section_title(title))
            .when_some(count, |this, count| {
                this.child(
                    div()
                        .text_color(theme.muted_foreground.opacity(0.6))
                        .child(count),
                )
            })
            .child(div().flex_1().h(px(1.)).bg(theme.border.opacity(0.6)))
    }

    fn render_groups(&self, presets: &[DroneGroupPreset], cx: &mut Context<Self>) -> Div {
        v_flex()
            .gap_5()
            .children(CATEGORIES.into_iter().map(|category| {
                let section: Vec<&DroneGroupPreset> =
                    presets.iter().filter(|p| p.category == category).collect();
                let ids: Vec<String> = section.iter().map(|p| p.id.clone()).collect();
                let chosen = ids.iter().filter(|id| self.selected.contains(*id)).count();
                let all_selected = chosen == ids.len();
                let hover = cx.theme().foreground;
                // Long names (maintenance bots, big mining drones) get three columns
                let columns = if section.iter().any(|p| p.name.len() > 18) {
                    3
                } else {
                    4
                };

                v_flex()
                    .gap_2()
                    .child(
                        Self::section_header(
                            t!(format!("droneGroups.category.{}", category.id())).to_string(),
                            Some(format!("{chosen}/{}", ids.len())),
                            cx,
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!(
                                    "drone-toggle-{}",
                                    category.id()
                                )))
                                .test_support()
                                .cursor_pointer()
                                .hover(move |s| s.text_color(hover))
                                .child(if all_selected {
                                    t!("importDialog.deselectAll").to_string()
                                } else {
                                    t!("importDialog.selectAll").to_string()
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    for id in &ids {
                                        if all_selected {
                                            this.selected.remove(id);
                                        } else {
                                            this.selected.insert(id.clone());
                                        }
                                    }
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(
                        h_flex().flex_wrap().gap_y_2().children(
                            section
                                .into_iter()
                                .map(|preset| self.render_tile(preset, columns, cx)),
                        ),
                    )
            }))
    }

    fn render_tile(
        &self,
        preset: &DroneGroupPreset,
        columns: usize,
        cx: &mut Context<Self>,
    ) -> Div {
        let id = preset.id.clone();
        let selected = self.selected.contains(&preset.id);
        let theme = cx.theme();
        let accent = ui::target_color();
        let (border, hover, fg, muted) = (
            theme.border.opacity(0.5),
            theme.muted.opacity(0.5),
            theme.foreground,
            theme.muted_foreground,
        );
        let detail = preset
            .stats
            .damage
            .clone()
            .or_else(|| preset.stats.effect.clone())
            .unwrap_or_default();

        let cell = if columns == 3 {
            div().w_1_3()
        } else {
            div().w_1_4()
        };
        cell.pr_2().child(
            h_flex()
                .id(SharedString::from(format!("drone-tile-{}", preset.id)))
                .test_support()
                .relative()
                .gap_2()
                .p_1p5()
                .rounded_lg()
                .border_1()
                .cursor_pointer()
                .map(|this| {
                    if selected {
                        this.bg(accent.opacity(0.07))
                            .border_color(accent.opacity(0.22))
                    } else {
                        this.border_color(border).hover(move |s| s.bg(hover))
                    }
                })
                .child(
                    div()
                        .when(!selected, |this| this.opacity(0.35))
                        .child(self.icon(preset, px(32.), cx)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_sm()
                                .truncate()
                                .text_color(if selected { fg } else { muted })
                                .child(preset.name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .truncate()
                                .text_color(muted.opacity(if selected { 1.0 } else { 0.6 }))
                                .child(detail),
                        ),
                )
                .child(check_mark(selected, cx))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.selected.remove(&id) {
                        this.selected.insert(id.clone());
                    }
                    cx.notify();
                })),
        )
    }

    fn render_order(&self, cx: &mut Context<Self>) -> Div {
        let categories = self.order.categories();
        let rows: Vec<_> = categories
            .iter()
            .copied()
            .enumerate()
            .map(|(index, category)| {
                let label = t!(format!("droneGroups.category.{}", category.id())).to_string();
                h_flex()
                    .w_1_2()
                    .gap_1()
                    .pr_3()
                    .py_0p5()
                    .child(
                        div()
                            .w(px(28.))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{:02}", index + 1)),
                    )
                    .child(div().flex_1().text_sm().child(label.clone()))
                    .children(
                        [
                            (-1, "up", IconName::ChevronUp),
                            (1, "down", IconName::ChevronDown),
                        ]
                        .into_iter()
                        .map(|(delta, direction, icon)| {
                            let disabled = (delta < 0 && index == 0)
                                || (delta > 0 && index + 1 == categories.len());
                            Button::new(SharedString::from(format!(
                                "drone-order-{}-{direction}",
                                category.id()
                            )))
                            .ghost()
                            .xsmall()
                            .icon(icon)
                            .disabled(disabled)
                            .tooltip(
                                t!(format!("droneGroups.move{direction}"), name = label)
                                    .to_string(),
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.order.move_category(category, delta);
                                    cx.notify();
                                },
                            ))
                        }),
                    )
            })
            .collect();
        v_flex()
            .gap_1()
            .child(
                Self::section_header(t!("droneGroups.typeOrder").to_string(), None, cx).child(
                    Button::new("drone-order-reset")
                        .ghost()
                        .xsmall()
                        .label(t!("droneGroups.resetOrder").to_string())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.order = DroneGroupOrder::default();
                            cx.notify();
                        })),
                ),
            )
            .child(h_flex().flex_wrap().children(rows))
    }

    fn render_naming(
        &self,
        presets: &[DroneGroupPreset],
        error: Option<PatternError>,
        cx: &mut Context<Self>,
    ) -> Div {
        let current = self.pattern(cx);
        let theme = cx.theme();
        let accent = ui::target_color();
        let (border, hover, muted, danger, panel) = (
            theme.border.opacity(0.5),
            theme.muted.opacity(0.5),
            theme.muted_foreground,
            theme.danger,
            theme.muted.opacity(0.35),
        );
        let examples: Vec<&DroneGroupPreset> = presets
            .iter()
            .filter(|p| EXAMPLE_IDS.contains(&p.id.as_str()))
            .collect();
        let sorted = current.starts_with("{order} ");
        let base = current.strip_prefix("{order} ").unwrap_or(&current);
        let custom = !PATTERN_STYLES.iter().any(|(_, p)| *p == base);

        let styles: Vec<_> = PATTERN_STYLES
            .iter()
            .map(|(id, pattern)| {
                let selected = base == *pattern;
                let pattern = if sorted {
                    format!("{{order}} {pattern}")
                } else {
                    pattern.to_string()
                };
                v_flex()
                    .id(SharedString::from(format!("drone-style-{id}")))
                    .test_support()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .cursor_pointer()
                    .map(|this| {
                        if selected {
                            this.bg(accent.opacity(0.07))
                                .border_color(accent.opacity(0.22))
                        } else {
                            this.border_color(border).hover(move |s| s.bg(hover))
                        }
                    })
                    .child(
                        h_flex()
                            .pb_1()
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .font_medium()
                                    .child(t!(format!("droneGroups.style.{id}")).to_string()),
                            )
                            .child(check_mark(selected, cx)),
                    )
                    .children(examples.iter().map(|preset| {
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(muted)
                            .child(preset.label_with_order(&pattern, &self.order))
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.pattern_input.update(cx, |input, cx| {
                            input.set_value(pattern.to_string(), window, cx)
                        });
                        cx.notify();
                    }))
            })
            .collect();

        let tokens: Vec<Div> = PATTERN_TOKENS
            .iter()
            .map(|token| {
                h_flex()
                    .w_1_2()
                    .items_start()
                    .gap_2()
                    .py_0p5()
                    .text_xs()
                    .child(
                        div()
                            .w(px(92.))
                            .flex_none()
                            .font_medium()
                            .child(format!("{{{token}}}")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(muted)
                            .child(t!(format!("droneGroups.tokens.{token}")).to_string()),
                    )
            })
            .collect();

        let invalid = error.is_some();
        let status = match error {
            Some(PatternError::MissingName) => t!("droneGroups.patternName").to_string(),
            Some(PatternError::UnknownToken(token)) => {
                t!("droneGroups.patternToken", token = token).to_string()
            }
            Some(PatternError::Unbalanced) => t!("droneGroups.patternBrackets").to_string(),
            None => examples
                .iter()
                .map(|p| p.label_with_order(&current, &self.order))
                .collect::<Vec<_>>()
                .join("   ·   "),
        };

        v_flex()
            .gap_5()
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        Checkbox::new("drone-sort-types")
                            .label(t!("droneGroups.sortTypes").to_string())
                            .checked(current.starts_with("{order} "))
                            .on_change(cx.listener(|this, enabled: &bool, window, cx| {
                                let current = this.pattern(cx);
                                let base = current.strip_prefix("{order} ").unwrap_or(&current);
                                let pattern = if *enabled {
                                    format!("{{order}} {base}")
                                } else {
                                    base.to_string()
                                };
                                this.pattern_input
                                    .update(cx, |input, cx| input.set_value(pattern, window, cx));
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(t!("droneGroups.sortTypesHint").to_string()),
                    ),
            )
            .when(sorted, |this| this.child(self.render_order(cx)))
            .child(
                v_flex()
                    .gap_2()
                    .child(Self::section_header(
                        t!("droneGroups.styles").to_string(),
                        None,
                        cx,
                    ))
                    .child(h_flex().gap_2().children(styles)),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        Self::section_header(t!("droneGroups.custom").to_string(), None, cx)
                            .child(check_mark(custom, cx)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(t!("droneGroups.patternHint").to_string()),
                    )
                    .child(
                        div()
                            .id("drone-pattern-input")
                            .test_support()
                            .child(Input::new(&self.pattern_input).small()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(if invalid { danger } else { muted })
                            .child(status),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .bg(panel)
                    .child(h_flex().flex_wrap().children(tokens))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(t!("droneGroups.optionalHint").to_string()),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(t!("droneGroups.namingHint").to_string()),
            )
    }

    fn render_review(&self, presets: &[DroneGroupPreset], cx: &mut Context<Self>) -> Div {
        let pattern = self.pattern(cx);
        let theme = cx.theme();
        let accent = ui::target_color();
        let (muted, pill) = (theme.muted_foreground, theme.muted.opacity(0.7));
        let categories = if pattern.contains("{order}") {
            self.order.categories().to_vec()
        } else {
            CATEGORIES.to_vec()
        };
        let removed: Vec<&DroneGroupPreset> = presets
            .iter()
            .filter(|p| self.present.contains(&p.id) && !self.selected.contains(&p.id))
            .collect();

        let tag = |text: String, color: Hsla, bg: Hsla| {
            div()
                .flex_none()
                .px_1p5()
                .rounded(px(4.))
                .bg(bg)
                .text_color(color)
                .text_size(px(10.))
                .font_medium()
                .child(text)
        };
        let row = |preset: &DroneGroupPreset, badge: Option<Div>, struck: bool| {
            h_flex()
                .gap_2()
                .py_1()
                .px_1()
                .border_b_1()
                .border_color(theme.border.opacity(0.4))
                .child(
                    div()
                        .when(struck, |this| this.opacity(0.4))
                        .child(self.icon(preset, px(20.), cx)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .truncate()
                        .when(struck, |this| this.line_through().text_color(muted))
                        .child(if struck {
                            preset.name.clone()
                        } else {
                            preset.label_with_order(&pattern, &self.order)
                        }),
                )
                .children(badge)
        };

        v_flex()
            .gap_5()
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(t!("droneGroups.reviewHint", count = self.selected.len()).to_string()),
            )
            .children(categories.into_iter().filter_map(|category| {
                let section: Vec<&DroneGroupPreset> = presets
                    .iter()
                    .filter(|p| p.category == category && self.selected.contains(&p.id))
                    .collect();
                (!section.is_empty()).then(|| {
                    let title = t!(format!("droneGroups.category.{}", category.id())).to_string();
                    v_flex()
                        .child(Self::section_header(
                            title,
                            Some(section.len().to_string()),
                            cx,
                        ))
                        .children(section.into_iter().map(|preset| {
                            let badge = if self.present.contains(&preset.id) {
                                tag(t!("droneGroups.updated").to_string(), muted, pill)
                            } else {
                                tag(
                                    t!("droneGroups.new").to_string(),
                                    accent,
                                    accent.opacity(0.12),
                                )
                            };
                            row(preset, Some(badge), false)
                        }))
                })
            }))
            .when(!removed.is_empty(), |this| {
                this.child(
                    v_flex()
                        .child(Self::section_header(
                            t!("droneGroups.removed").to_string(),
                            Some(removed.len().to_string()),
                            cx,
                        ))
                        .children(removed.into_iter().map(|preset| row(preset, None, true)))
                        .child(
                            div()
                                .pt_1()
                                .text_xs()
                                .text_color(muted)
                                .child(t!("droneGroups.removedHint").to_string()),
                        ),
                )
            })
    }

    fn render_footer(&self, invalid: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let step = self.step;
        let back = match step {
            Step::Groups => None,
            Step::Naming => Some(Step::Groups),
            Step::Review => Some(Step::Naming),
        };
        let next = match step {
            Step::Groups => Some(Step::Naming),
            Step::Naming => Some(Step::Review),
            Step::Review => None,
        };

        h_flex()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        t!(
                            "droneGroups.selectedCount",
                            selected = self.selected.len(),
                            total = drone_groups::catalog().len()
                        )
                        .to_string(),
                    ),
            )
            .child(
                Button::new("drone-groups-cancel")
                    .ghost()
                    .label(t!("formationEditor.reset").to_string())
                    .disabled(self.saving)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.selected = this.present.clone();
                        this.step = Step::Groups;
                        this.scroll.reset();
                        this.order = this.saved_order.clone();
                        let pattern = this.saved_pattern.clone();
                        this.pattern_input
                            .update(cx, |input, cx| input.set_value(pattern, window, cx));
                        cx.notify();
                    })),
            )
            .when_some(back, |this, back| {
                this.child(
                    Button::new("drone-groups-back")
                        .outline()
                        .icon(IconName::ChevronLeft)
                        .label(t!("droneGroups.back").to_string())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.step = back;
                            this.scroll.reset();
                            cx.notify();
                        })),
                )
            })
            .map(|this| match next {
                Some(next) => this.child(
                    Button::new("drone-groups-next")
                        .primary()
                        .label(t!("droneGroups.next").to_string())
                        .disabled(step == Step::Naming && invalid)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.step = next;
                            this.scroll.reset();
                            cx.notify();
                        })),
                ),
                None => this.child(
                    Button::new("drone-groups-save")
                        .primary()
                        .label(t!("droneGroups.save", count = self.selected.len()).to_string())
                        .disabled(invalid || self.saving)
                        .on_click(cx.listener(|this, _, window, cx| {
                            let path = this.path.clone();
                            let draft = DroneGroupDraft {
                                selected: this.selected.clone(),
                                pattern: this.pattern(cx),
                                order: this.order.clone(),
                            };
                            this.main
                                .update(cx, |main, cx| {
                                    main.prompt_drone_group_save(path, draft, window, cx)
                                })
                                .ok();
                        })),
                ),
            })
    }
}

// A filled accent dot with a check when selected, an empty ring otherwise.
fn check_mark(selected: bool, cx: &App) -> Div {
    let accent = ui::target_color();
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded_full()
        .map(|this| {
            if selected {
                this.bg(accent).child(
                    Icon::new(IconName::Check)
                        .xsmall()
                        .text_color(cx.theme().background),
                )
            } else {
                this.border_1().border_color(cx.theme().border)
            }
        })
}

impl Render for DroneGroupsEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.loading {
            return div()
                .flex_1()
                .p_4()
                .child(t!("formationEditor.loading").to_string())
                .into_any_element();
        }
        if let Some(error) = &self.error {
            return v_flex()
                .p_4()
                .gap_3()
                .child(error.clone())
                .child(
                    Button::new("drone-retry")
                        .outline()
                        .label(t!("common.refresh").to_string())
                        .on_click(cx.listener(|this, _, window, cx| this.load(window, cx))),
                )
                .into_any_element();
        }
        let presets = drone_groups::catalog();
        let error = drone_groups::validate_pattern(&self.pattern(cx)).err();
        let invalid = error.is_some();
        let body = match self.step {
            Step::Groups => self.render_groups(&presets, cx),
            Step::Naming => self.render_naming(&presets, error, cx),
            Step::Review => self.render_review(&presets, cx),
        };

        v_flex()
            .id("drone-editor")
            .test_support()
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_3()
            .when(self.disk_changed, |this| {
                this.child(
                    h_flex()
                        .gap_2()
                        .child(t!("formationEditor.fileChanged").to_string())
                        .child(
                            Button::new("drone-reload")
                                .outline()
                                .small()
                                .label(t!("common.refresh").to_string())
                                .on_click(cx.listener(|this, _, window, cx| this.load(window, cx))),
                        ),
                )
            })
            .child(self.render_steps(invalid, cx))
            .child(
                self.scroll
                    .view("drone-groups-body", body, |this| &mut this.scroll, cx)
                    .flex_1(),
            )
            .child(self.render_footer(invalid, cx))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::MainView;
    use crate::main_window::profiles::tests::sample_data;
    use crate::store::Store;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, point, px, size,
    };

    #[gpui_kit::test]
    fn account_dialog_opens_and_supports_group_selection_naming_and_review(
        cx: &mut TestAppContext,
    ) {
        let data = sample_data();
        let account = data.servers[0].profiles[1].accounts[0].clone();
        let (handle, main) = cx.update(|cx| {
            gpui_kit::init(cx);
            Store::install_for_test(data, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.), px(0.)),
                        size: size(px(1440.), px(900.)),
                    })),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| MainView::new(window, cx)),
            )
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            main.update(cx, |main, cx| {
                main.tab = Some(super::super::Tab::Drones);
                main.editor_server = Some(account.server);
                let path = account.path.clone();
                let editor = main.create_drone_editor(
                    account,
                    Some(HashSet::from(["hobgoblin".into()])),
                    window,
                    cx,
                );
                main.drone_editors.insert(path, editor);
            });
            window.render_frame(cx);
            let editor = main.read(cx).drone_editors.values().next().unwrap().clone();
            crate::scrolling::tests::check_vertical(
                &editor,
                |editor| &mut editor.scroll,
                window,
                cx,
            );
            window.click("drone-tile-hobgoblin", cx);
            window.click("drone-toggle-light", cx);
            window.click("drone-groups-next", cx);
            window.click("drone-pattern-input", cx);
            window.press("secondary-a", cx);
            window.input("{dps}", cx);
            window.click("drone-groups-next", cx);
            assert!(window.try_find("drone-groups-save").is_none());
            window.click("drone-style-compact", cx);
            let sort = window.find("drone-sort-types").bounds();
            window.click_at("drone-sort-types", point(px(8.), sort.size.height / 2.), cx);
            assert!(editor.read(cx).pattern(cx).starts_with("{order} "));
            window.click("drone-order-light-down", cx);
            assert_eq!(
                editor.read(cx).order.categories()[0],
                eve_wrench_core::drone_groups::DroneCategory::Medium
            );
            assert_eq!(
                editor.read(cx).order.categories()[1],
                eve_wrench_core::drone_groups::DroneCategory::Light
            );
            assert!(editor.read(cx).is_dirty(cx));
            let hobgoblin = eve_wrench_core::drone_groups::catalog()
                .iter()
                .find(|preset| preset.id == "hobgoblin")
                .unwrap()
                .clone();
            assert!(
                hobgoblin
                    .label_with_order(&editor.read(cx).pattern(cx), &editor.read(cx).order)
                    .starts_with("02 ")
            );
            window.click("drone-order-reset", cx);
            assert_eq!(
                editor.read(cx).order,
                eve_wrench_core::drone_groups::DroneGroupOrder::default()
            );
            window.click("drone-order-industry-up", cx);
            window.click("drone-style-plain", cx);
            assert_eq!(editor.read(cx).pattern(cx), "{order} {name}");
            window.click("drone-groups-next", cx);
            window.find("drone-groups-save");
            window.click("drone-groups-back", cx);
            window.find("drone-style-plain");
            window.click("drone-groups-cancel", cx);
            assert_eq!(editor.read(cx).order, editor.read(cx).saved_order);
            assert!(!editor.read(cx).is_dirty(cx));
            window.render_frame(cx);
        })
        .unwrap();
    }
}
