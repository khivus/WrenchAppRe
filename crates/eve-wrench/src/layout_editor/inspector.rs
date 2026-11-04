use eve_wrench_core::window_layout::WindowState;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Selectable as _, Sizable as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use rust_i18n::t;

use super::model;
use super::{LayoutEditor, RectField};
use crate::ui::{self, StyleExt as _};

impl LayoutEditor {
    fn render_list_row(
        &self,
        window: &WindowState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let selected = self.selected.as_deref() == Some(window.key.as_str());
        let theme = cx.theme();
        let accent = ui::target_color();
        let hover = theme.muted.opacity(0.6);
        let key = window.key.clone();
        let visibility_key = window.key.clone();
        let hidden = self.preview_hidden.contains(&window.key);
        let active = self.active_window(window) && self.in_location(window);
        let (fg, muted) = (theme.foreground, theme.muted_foreground);
        h_flex()
            .id(SharedString::from(format!("list-{}", window.key)))
            .test_support()
            .flex_none()
            .gap_2()
            .px_3()
            .py_1()
            .text_sm()
            .cursor_pointer()
            .map(|this| {
                if selected {
                    this.bg(accent.opacity(0.1))
                } else {
                    this.hover(move |s| s.bg(hover))
                }
            })
            .child(
                Icon::new(model::icon(window))
                    .small()
                    .text_color(if selected { accent } else { muted }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_color(if active && !hidden {
                        fg
                    } else {
                        muted.opacity(0.7)
                    })
                    .child(self.preview_name(window)),
            )
            .when(window.locked, |this| {
                this.child(Icon::new(IconName::Lock).xsmall().text_color(muted))
            })
            .child(
                Button::new(SharedString::from(format!(
                    "preview-visibility-{}",
                    window.key
                )))
                .ghost()
                .xsmall()
                .icon(if hidden {
                    IconName::EyeOff
                } else {
                    IconName::Eye
                })
                .selected(hidden)
                .toggled(hidden)
                .disabled(window.rect.is_none())
                .tooltip(if hidden {
                    t!("layoutEditor.showInPreview").to_string()
                } else {
                    t!("layoutEditor.hideInPreview").to_string()
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.set_preview_hidden(&visibility_key, !hidden, cx);
                })),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select(Some(key.clone()), window, cx);
            }))
    }

    fn matches_search(&self, window: &WindowState, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let mut text = format!("{} {}", window.key, self.preview_name(window));
        if window.is_stack() {
            for member in model::members(&self.windows, &window.key) {
                text.push_str(&format!(
                    " {} {}",
                    member.key,
                    model::window_name(member.id())
                ));
            }
        }
        let text = text.to_lowercase();
        query.split_whitespace().all(|term| text.contains(term))
    }

    // Search covers every stored window, independent of preview/location filters.
    // It narrows bulk controls without changing what is drawn on the canvas.
    fn listed_windows<'a>(&'a self, query: &'a str) -> impl Iterator<Item = &'a WindowState> {
        self.windows
            .iter()
            .filter(|w| model::listable(w, &self.windows))
            .filter(move |w| {
                !query.is_empty()
                    || (self.in_location(w)
                        && (self.show_all_windows
                            || self.active_window(w)
                            || self.selected.as_deref() == Some(w.key.as_str())
                            || self.preview_hidden.contains(&w.key)
                            || self.geometry_resets.contains(&w.key)
                            || self
                                .saved
                                .iter()
                                .find(|old| old.key == w.key)
                                .is_some_and(|old| old != *w)))
            })
            .filter(move |w| self.matches_search(w, query))
    }

    fn set_scope_preview_hidden(
        &mut self,
        category: Option<model::Category>,
        hidden: bool,
        cx: &mut Context<Self>,
    ) {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let keys: Vec<_> = self
            .listed_windows(&query)
            .filter(|w| w.rect.is_some() && category.is_none_or(|c| model::category(w) == c))
            .map(|w| w.key.clone())
            .collect();
        self.set_previews_hidden(keys, hidden, cx);
    }

    // Windows grouped by what they're for, active ones first in each group.
    pub(super) fn render_list(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let query = self.search.read(cx).value().trim().to_lowercase();
        let listed: Vec<_> = self.listed_windows(&query).collect();
        let can_show = listed
            .iter()
            .any(|w| w.rect.is_some() && self.preview_hidden.contains(&w.key));
        let can_hide = listed
            .iter()
            .any(|w| w.rect.is_some() && !self.preview_hidden.contains(&w.key));

        let mut sections: Vec<Div> = Vec::new();
        for category in model::Category::ALL {
            let mut list: Vec<&WindowState> = listed
                .iter()
                .copied()
                .filter(|w| model::category(w) == category)
                .collect();
            if list.is_empty() {
                continue;
            }
            list.sort_by_cached_key(|w| {
                (!self.active_window(w), self.preview_name(w).to_lowercase())
            });
            let shown = list
                .iter()
                .filter(|w| {
                    w.rect.is_some()
                        && self.in_location(w)
                        && !self.preview_hidden.contains(&w.key)
                        && (self.show_all_windows || self.active_window(w))
                })
                .count();
            let has_geometry = list.iter().any(|w| w.rect.is_some());
            let all_hidden = list
                .iter()
                .filter(|w| w.rect.is_some())
                .all(|w| self.preview_hidden.contains(&w.key));
            let title = t!(format!("layoutEditor.category.{}", category.id())).to_string();
            sections.push(
                v_flex()
                    .child(
                        h_flex()
                            .px_3()
                            .pt_3()
                            .pb_0p5()
                            .gap_1()
                            .child(div().flex_1().min_w_0().child(ui::section_divider(
                                title.clone(),
                                Some(format!("{shown}/{}", list.len())),
                                cx,
                            )))
                            .child(
                                Button::new(SharedString::from(format!(
                                    "preview-category-{}",
                                    category.id()
                                )))
                                .ghost()
                                .xsmall()
                                .icon(if all_hidden {
                                    IconName::EyeOff
                                } else {
                                    IconName::Eye
                                })
                                .selected(all_hidden)
                                .toggled(all_hidden)
                                .disabled(!has_geometry)
                                .tooltip(if all_hidden {
                                    t!("layoutEditor.showCategory", category = title).to_string()
                                } else {
                                    t!("layoutEditor.hideCategory", category = title).to_string()
                                })
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.set_scope_preview_hidden(
                                            Some(category),
                                            !all_hidden,
                                            cx,
                                        );
                                    },
                                )),
                            ),
                    )
                    .children(list.into_iter().map(|w| self.render_list_row(w, cx))),
            );
        }

        v_flex()
            .w(px(240.))
            .flex_none()
            .min_h_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(
                v_flex()
                    .flex_none()
                    .p_2()
                    .gap_2()
                    .child(
                        Input::new(&self.search)
                            .id("layout-search")
                            .small()
                            .cleanable(true)
                            .prefix(
                                Icon::new(IconName::Search)
                                    .xsmall()
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("preview-show-all")
                                    .outline()
                                    .xsmall()
                                    .flex_1()
                                    .min_w_0()
                                    .label(
                                        if query.is_empty() {
                                            t!("layoutEditor.showAll")
                                        } else {
                                            t!("layoutEditor.showMatches")
                                        }
                                        .to_string(),
                                    )
                                    .tooltip(t!("layoutEditor.bulkVisibilityHint").to_string())
                                    .disabled(!can_show)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_scope_preview_hidden(None, false, cx)
                                    })),
                            )
                            .child(
                                Button::new("preview-hide-all")
                                    .outline()
                                    .xsmall()
                                    .flex_1()
                                    .min_w_0()
                                    .label(
                                        if query.is_empty() {
                                            t!("layoutEditor.hideAll")
                                        } else {
                                            t!("layoutEditor.hideMatches")
                                        }
                                        .to_string(),
                                    )
                                    .tooltip(t!("layoutEditor.bulkVisibilityHint").to_string())
                                    .disabled(!can_hide)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_scope_preview_hidden(None, true, cx)
                                    })),
                            ),
                    ),
            )
            .child(
                self.list_scroll
                    .view(
                        "window-list",
                        v_flex()
                            .pb_3()
                            .when(sections.is_empty(), |this| {
                                this.child(
                                    div()
                                        .id("window-list-empty")
                                        .test_support()
                                        .p_4()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(
                                            if query.is_empty() {
                                                t!("layoutEditor.noWindows")
                                            } else {
                                                t!("layoutEditor.noMatchingWindows")
                                            }
                                            .to_string(),
                                        ),
                                )
                            })
                            .children(sections),
                        |this| &mut this.list_scroll,
                        cx,
                    )
                    .flex_1(),
            )
    }

    fn render_flag(
        &self,
        id: &'static str,
        label: String,
        checked: bool,
        set: fn(&mut WindowState, bool),
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        h_flex()
            .px_4()
            .py_1p5()
            .child(div().flex_1().text_sm().child(label))
            .child(
                Switch::new(id)
                    .small()
                    .checked(checked)
                    .on_click(cx.listener(move |this, checked: &bool, window, cx| {
                        let checked = *checked;
                        this.edit(window, cx, |this| {
                            this.update_selected(|w| set(w, checked));
                        });
                    })),
            )
    }

    pub(super) fn render_inspector(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme();
        let (muted, mono, border) = (
            theme.muted_foreground,
            theme.mono_font_family.clone(),
            theme.border,
        );
        let content = match self.selected_window().cloned() {
            None => div()
                .px_4()
                .py_3()
                .text_xs()
                .text_color(muted)
                .child(t!("layoutEditor.noSelection").to_string())
                .into_any_element(),
            Some(window) => {
                let screen = model::screen(&self.windows);
                let saved_on = window
                    .rect
                    .filter(|r| r.screen_width != screen.width || r.screen_height != screen.height);
                let tabs: Vec<String> = self
                    .preview_members(&window.key)
                    .iter()
                    .map(|w| model::window_name(w.id()))
                    .collect();

                v_flex()
                    .child(
                        v_flex()
                            .px_4()
                            .py_3()
                            .gap_0p5()
                            .child(div().font_medium().child(self.preview_name(&window)))
                            .child(
                                div()
                                    .text_xs()
                                    .font_family(mono)
                                    .text_color(muted)
                                    .child(window.id().to_string()),
                            ),
                    )
                    .when(!tabs.is_empty(), |this| {
                        this.child(
                            v_flex()
                                .px_4()
                                .pb_3()
                                .gap_1()
                                .child(ui::section_divider(
                                    t!("layoutEditor.tabs").to_string(),
                                    Some(tabs.len().to_string()),
                                    cx,
                                ))
                                .children(
                                    tabs.into_iter()
                                        .map(|tab| div().text_sm().text_color(muted).child(tab)),
                                ),
                        )
                    })
                    .child(
                        v_flex()
                            .px_4()
                            .gap_2()
                            .child(ui::section_divider(
                                t!("layoutEditor.geometry").to_string(),
                                None,
                                cx,
                            ))
                            .child(h_flex().flex_wrap().gap_y_2().children(
                                RectField::ALL.iter().zip(&self.rect_inputs).map(
                                    |(field, input)| {
                                        h_flex()
                                            .w_1_2()
                                            .pr_2()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .w(px(14.))
                                                    .text_xs()
                                                    .text_color(muted)
                                                    .child(field.label()),
                                            )
                                            .child(
                                                div().flex_1().child(
                                                    Input::new(input).small().disabled(
                                                        window.rect.is_none() || self.saving,
                                                    ),
                                                ),
                                            )
                                    },
                                ),
                            ))
                            .child(
                                Button::new("layout-reset-geometry")
                                    .outline()
                                    .small()
                                    .w_full()
                                    .label(t!("layoutEditor.resetGeometry").to_string())
                                    .disabled(
                                        self.saving
                                            || self.disk_changed
                                            || (window.rect.is_none()
                                                && !model::members(&self.windows, &window.key)
                                                    .iter()
                                                    .any(|w| w.rect.is_some())),
                                    )
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.reset_selected_geometry(window, cx)
                                    })),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(t!("layoutEditor.resetGeometryHint").to_string()),
                            )
                            .when(window.rect.is_none(), |this| {
                                this.child(div().text_xs().text_color(ui::amber(cx)).child(
                                    if self.geometry_resets.contains(&window.key) {
                                        t!("layoutEditor.geometryResetPending").to_string()
                                    } else {
                                        t!("layoutEditor.defaultGeometry").to_string()
                                    },
                                ))
                            })
                            .when_some(saved_on, |this, rect| {
                                this.child(
                                    div().text_xs().text_color(ui::amber(cx)).child(
                                        t!(
                                            "layoutEditor.otherScreen",
                                            width = rect.screen_width,
                                            height = rect.screen_height
                                        )
                                        .to_string(),
                                    ),
                                )
                            }),
                    )
                    .child(div().px_4().pt_4().child(ui::section_divider(
                        t!("layoutEditor.state").to_string(),
                        None,
                        cx,
                    )))
                    .child(
                        h_flex()
                            .px_4()
                            .py_1p5()
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .child(t!("layoutEditor.hideInPreview").to_string()),
                            )
                            .child(
                                Switch::new("flag-preview-hidden")
                                    .small()
                                    .checked(self.preview_hidden.contains(&window.key))
                                    .on_click(cx.listener(|this, hidden: &bool, _, cx| {
                                        if let Some(key) = this.selected.clone() {
                                            this.set_preview_hidden(&key, *hidden, cx);
                                        }
                                    })),
                            ),
                    )
                    .child(self.render_flag(
                        "flag-open",
                        t!("layoutEditor.openFlag").to_string(),
                        window.open,
                        |w, on| w.open = on,
                        cx,
                    ))
                    .child(self.render_flag(
                        "flag-locked",
                        t!("layoutEditor.locked").to_string(),
                        window.locked,
                        |w, on| w.locked = on,
                        cx,
                    ))
                    .child(self.render_flag(
                        "flag-minimized",
                        t!("layoutEditor.minimized").to_string(),
                        window.minimized,
                        |w, on| w.minimized = on,
                        cx,
                    ))
                    .child(self.render_flag(
                        "flag-compact",
                        t!("layoutEditor.compact").to_string(),
                        window.compact.unwrap_or(self.compact_default),
                        |w, on| w.compact = Some(on),
                        cx,
                    ))
                    .child(self.render_flag(
                        "flag-light",
                        t!("layoutEditor.lightBackground").to_string(),
                        window.light_background.unwrap_or(false),
                        |w, on| w.light_background = Some(on),
                        cx,
                    ))
                    .into_any_element()
            }
        };

        self.inspector_scroll
            .view(
                "layout-inspector",
                content,
                |this| &mut this.inspector_scroll,
                cx,
            )
            .w(px(280.))
            .flex_none()
            .border_l_1()
            .border_color(border)
    }
}
