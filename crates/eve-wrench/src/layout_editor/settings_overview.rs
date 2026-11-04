use super::*;
use gpui_kit::base::ScrollbarHandle as _;

impl DisplayEditor {
    pub(super) fn picker(
        &self,
        id: &str,
        label: String,
        options: Vec<(String, String)>,
        selected: String,
        apply: impl Fn(&mut Self, String, &mut Context<Self>) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = cx.entity().downgrade();
        Button::new(SharedString::from(id.to_owned()))
            .outline()
            .small()
            .w_full()
            .label(label)
            .dropdown_menu(move |menu, _, _| {
                options.iter().fold(
                    menu.max_h(px(320.)).scrollable(true),
                    |menu, (value, label)| {
                        let value = value.clone();
                        let checked = value == selected;
                        let view = view.clone();
                        let apply = apply.clone();
                        menu.item(PopupMenuItem::new(label.clone()).checked(checked).on_click(
                            move |_, _, cx| {
                                view.update(cx, |this, cx| apply(this, value.clone(), cx))
                                    .ok();
                            },
                        ))
                    },
                )
            })
            .into_any_element()
    }
    fn set_tab(&mut self, tab: &str, field: &str, value: Value, cx: &mut Context<Self>) {
        self.status = None;
        let result = self
            .doc_mut(SettingsKind::User)
            .and_then(|doc| overview::set_tab(doc, tab, field, value));
        self.error = result.err();
        cx.emit(DisplayChanged);
        cx.notify();
    }
    pub(super) fn render_overview(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(doc) = self.doc(SettingsKind::User) else {
            return hint("display.chooseAccount", cx);
        };
        let tabs = overview::tabs(doc);
        let Some(tab) = tabs
            .iter()
            .find(|t| Some(&t.key) == self.selected_tab.as_ref())
            .or_else(|| tabs.first())
            .cloned()
        else {
            return hint("display.noOverviewTabs", cx);
        };
        let choices = tabs
            .iter()
            .map(|t| (t.key.clone(), format!("{} · {}", t.id + 1, t.name)))
            .collect();
        let selected = tab.key.clone();
        let picker = self.picker(
            "overview-tab",
            format!("{}: {}", t!("display.tab"), tab.name),
            choices,
            selected,
            |this, key, cx| {
                this.selected_tab = Some(key);
                this.selected_preset = None;
                this.scroll.reset();
                cx.emit(DisplayChanged);
                cx.notify();
            },
            cx,
        );
        let section = match self.overview_section {
            0 => self.render_tab_columns(&tab, window, cx),
            1 => self.render_filters(&tab, cx),
            _ => self.render_appearance(cx),
        };
        v_flex()
            .gap_3()
            .child(picker)
            .child(
                h_flex().gap_1().children(
                    ["tabsColumns", "filters", "colors"]
                        .into_iter()
                        .enumerate()
                        .map(|(index, key)| {
                            Button::new(SharedString::from(format!("overview-section-{index}")))
                                .ghost()
                                .xsmall()
                                .flex_1()
                                .selected(self.overview_section == index)
                                .label(t!(format!("display.{key}")).to_string())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.overview_section = index;
                                    this.scroll.reset();
                                    cx.emit(DisplayChanged);
                                    cx.notify();
                                }))
                        }),
                ),
            )
            .child(section)
            .into_any_element()
    }
    fn preset_options(&self) -> Vec<(String, String)> {
        let mut options: Vec<_> = self
            .doc(SettingsKind::User)
            .and_then(|d| d.value("overview", "overviewProfilePresets"))
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .map(|(key, _)| (key.clone(), text(&json!(key)).unwrap_or(key).to_owned()))
            .collect();
        options.sort_by_cached_key(|(_, name)| name.to_lowercase());
        options
    }
    fn render_tab_columns(
        &mut self,
        tab: &overview::OverviewTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = self.input(
            Field::TabName(tab.key.clone()),
            t!("display.tabName").to_string(),
            window,
            cx,
        );
        let color = self.input(
            Field::TabColor(tab.key.clone()),
            t!("display.tabColor").to_string(),
            window,
            cx,
        );
        let raw = self
            .doc(SettingsKind::User)
            .and_then(|d| d.value("overview", "tabsettings_new"))
            .and_then(|v| v.get(&tab.key))
            .cloned()
            .unwrap_or(json!({}));
        let options = |field: &str| {
            let mut choices = self.preset_options();
            if let Some(value) = raw.get(field).and_then(Value::as_str)
                && let Some(name) = raw.get(field).and_then(text)
                && !choices.iter().any(|(_, n)| n == name)
            {
                choices.push((value.to_owned(), name.to_owned()));
            }
            choices
        };
        let presets = options("bytes:overview");
        let bracket_presets = options("bytes:bracket");
        let key = tab.key.clone();
        let key2 = key.clone();
        let preset_key = presets
            .iter()
            .find(|(_, name)| name == &tab.preset)
            .map(|(key, _)| key.clone())
            .unwrap_or_default();
        let filter = self.picker(
            "overview-filter",
            format!("{}: {}", t!("display.filterPreset"), tab.preset),
            presets.clone(),
            preset_key,
            move |this, preset, cx| this.set_tab(&key, "overview", json!(preset), cx),
            cx,
        );
        let bracket_key = bracket_presets
            .iter()
            .find(|(_, name)| name == &tab.bracket)
            .map(|(key, _)| key.clone())
            .unwrap_or_default();
        let bracket = self.picker(
            "overview-bracket",
            format!("{}: {}", t!("display.bracketPreset"), tab.bracket),
            bracket_presets,
            bracket_key,
            move |this, preset, cx| this.set_tab(&key2, "bracket", json!(preset), cx),
            cx,
        );
        let shown = raw
            .get("bytes:tabColumns")
            .map(|v| overview::strings(Some(v)))
            .unwrap_or_else(|| {
                overview::DEFAULT_COLUMNS
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            });
        let mode = if raw.get("bytes:showNone").and_then(boolean) == Some(true) {
            2
        } else if raw.get("bytes:showAll").and_then(boolean) == Some(true) {
            1
        } else {
            0
        };
        let modes = h_flex().gap_1().children(
            ["bracketsPreset", "bracketsAll", "bracketsNone"]
                .into_iter()
                .enumerate()
                .map(|(index, name)| {
                    let key = tab.key.clone();
                    Button::new(SharedString::from(format!("bracket-mode-{index}")))
                        .outline()
                        .xsmall()
                        .flex_1()
                        .selected(mode == index)
                        .label(t!(format!("display.{name}")).to_string())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.edit(
                                SettingsKind::User,
                                "overview",
                                "tabsettings_new",
                                |v| {
                                    if let Some(t) = v.get_mut(&key).and_then(Value::as_object_mut)
                                    {
                                        t.insert("bytes:showAll".into(), json!(index == 1));
                                        t.insert("bytes:showNone".into(), json!(index == 2));
                                    }
                                },
                                cx,
                            )
                        }))
                }),
        );
        let mut rows = Vec::new();
        // Text inputs are relatively expensive to lay out. Keep their state in
        // `inputs`, but only build controls around the visible column rows.
        const ROW_HEIGHT: f32 = 80.;
        let offset = -f32::from(self.scroll.handle.offset().y);
        let height = f32::from(self.scroll.handle.viewport_bounds().size.height).max(400.);
        let first = (((offset - self.columns_top.get()) / ROW_HEIGHT)
            .floor()
            .max(0.) as usize)
            .saturating_sub(2)
            .min(tab.columns.len());
        let end = ((((offset + height - self.columns_top.get()) / ROW_HEIGHT)
            .ceil()
            .max(0.) as usize)
            + 2)
        .max(first)
        .min(tab.columns.len());
        for (index, column) in tab.columns.iter().enumerate().take(end).skip(first) {
            let Some((_, _, _)) = overview::COLUMNS.iter().find(|c| c.0 == column) else {
                continue;
            };
            let checked = shown.contains(column);
            let column_id = column.clone();
            let key = tab.key.clone();
            let up_key = key.clone();
            let down_key = key.clone();
            let width = self.input(
                Field::Width(tab.id, column.clone()),
                t!("display.columnWidth").to_string(),
                window,
                cx,
            );
            rows.push(
                v_flex()
                    .id(SharedString::from(format!("overview-column-row-{column}")))
                    .test_support()
                    .h(px(ROW_HEIGHT))
                    .flex_none()
                    .gap_1()
                    .pb_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .child(t!(format!("display.columns.{column}")).to_string()),
                            )
                            .child(
                                Switch::new(SharedString::from(format!("column-visible-{column}")))
                                    .small()
                                    .checked(checked)
                                    .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                        let checked = *checked;
                                        let key = key.clone();
                                        let column = column_id.clone();
                                        this.edit(
                                            SettingsKind::User,
                                            "overview",
                                            "tabsettings_new",
                                            move |v| {
                                                let Some(tab) =
                                                    v.get_mut(&key).and_then(Value::as_object_mut)
                                                else {
                                                    return;
                                                };
                                                let values = tab
                                                    .entry("bytes:tabColumns")
                                                    .or_insert_with(|| {
                                                        json!(
                                                            overview::DEFAULT_COLUMNS
                                                                .iter()
                                                                .map(|s| format!("bytes:{s}"))
                                                                .collect::<Vec<_>>()
                                                        )
                                                    });
                                                if let Some(a) = values.as_array_mut() {
                                                    a.retain(|v| text(v) != Some(column.as_str()));
                                                    if checked {
                                                        a.push(json!(format!("bytes:{column}")));
                                                    }
                                                }
                                            },
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("column-up-{column}")))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ArrowUp)
                                    .disabled(index == 0)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.move_column(&up_key, index, -1, cx)
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("column-down-{column}")))
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::ArrowDown)
                                    .disabled(index + 1 >= tab.columns.len())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.move_column(&down_key, index, 1, cx)
                                    })),
                            ),
                    )
                    .child(width),
            );
        }
        v_flex()
            .gap_3()
            .child(name)
            .child(color)
            .child(filter)
            .child(bracket)
            .child(
                Button::new("brackets-clear")
                    .outline()
                    .small()
                    .w_full()
                    .label(t!("display.clearBrackets").to_string())
                    .tooltip(t!("display.clearBracketsHint").to_string())
                    .on_click(cx.listener({
                        let key = tab.key.clone();
                        move |this, _, _, cx| {
                            this.error = this
                                .doc_mut(SettingsKind::User)
                                .and_then(|doc| overview::set_clear_brackets(doc, &key))
                                .err();
                            this.status = None;
                            cx.emit(DisplayChanged);
                            cx.notify();
                        }
                    })),
            )
            .child(modes)
            .child(hint("display.columnsHint", cx))
            .child(
                v_flex()
                    .relative()
                    .child(div().h(px(first as f32 * ROW_HEIGHT)).flex_none())
                    .children(rows)
                    .child(
                        div()
                            .h(px((tab.columns.len() - end) as f32 * ROW_HEIGHT))
                            .flex_none(),
                    )
                    .child({
                        let top = self.columns_top.clone();
                        let scroll = self.scroll.handle.clone();
                        canvas(
                            move |bounds, _, _| {
                                let offset = f32::from(
                                    bounds.top()
                                        - scroll.viewport_bounds().top()
                                        - scroll.offset().y,
                                );
                                if (offset - top.get()).abs() > 0.5 {
                                    top.set(offset);
                                    true
                                } else {
                                    false
                                }
                            },
                            |_, changed, window, cx| {
                                if changed {
                                    cx.notify(window.current_view());
                                }
                            },
                        )
                        .absolute()
                        .inset_0()
                    }),
            )
            .into_any_element()
    }
    fn move_column(&mut self, key: &str, index: usize, delta: isize, cx: &mut Context<Self>) {
        let key = key.to_string();
        self.edit(
            SettingsKind::User,
            "overview",
            "tabsettings_new",
            move |v| {
                if let Some(tab) = v.get_mut(&key).and_then(Value::as_object_mut) {
                    let order = tab.entry("bytes:tabColumnOrder").or_insert_with(|| {
                        json!(
                            overview::COLUMNS
                                .iter()
                                .map(|c| format!("bytes:{}", c.0))
                                .collect::<Vec<_>>()
                        )
                    });
                    if let Some(a) = order.as_array_mut() {
                        let dest = index.saturating_add_signed(delta);
                        if index < a.len() && dest < a.len() {
                            a.swap(index, dest);
                        }
                    }
                }
            },
            cx,
        );
    }
    fn filter_list(
        &mut self,
        preset: &str,
        field: &'static str,
        id: i64,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        let preset = preset.to_owned();
        self.edit(
            SettingsKind::User,
            "overview",
            "overviewProfilePresets",
            move |v| {
                if let Some(p) = v.get_mut(&preset).and_then(Value::as_object_mut) {
                    let a = p
                        .entry(format!("bytes:{field}"))
                        .or_insert_with(|| json!([]));
                    if let Some(a) = a.as_array_mut() {
                        a.retain(|v| integer(v) != Some(id));
                        if enabled {
                            a.push(json!(id));
                        }
                    }
                    let opposite = match field {
                        "filteredStates" => Some("bytes:alwaysShownStates"),
                        "alwaysShownStates" => Some("bytes:filteredStates"),
                        _ => None,
                    };
                    if enabled
                        && let Some(other) = opposite
                            .and_then(|k| p.get_mut(k))
                            .and_then(Value::as_array_mut)
                    {
                        other.retain(|v| integer(v) != Some(id));
                    }
                }
            },
            cx,
        );
    }
    fn render_filters(
        &mut self,
        tab: &overview::OverviewTab,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let presets = self.preset_options();
        let chosen = self
            .selected_preset
            .as_ref()
            .and_then(|key| presets.iter().find(|(k, _)| k == key))
            .or_else(|| presets.iter().find(|(_, name)| name == &tab.preset))
            .cloned();
        let selected = chosen.as_ref().map(|(k, _)| k.clone()).unwrap_or_default();
        let picker = self.picker(
            "filter-preset-editor",
            chosen
                .as_ref()
                .map(|(_, name)| name.clone())
                .unwrap_or_else(|| t!("display.choosePreset").to_string()),
            presets,
            selected,
            |this, key, cx| {
                this.selected_preset = Some(key);
                this.group_page = 0;
                this.scroll.reset();
                cx.emit(DisplayChanged);
                cx.notify();
            },
            cx,
        );
        let Some((preset, _)) = chosen else {
            return v_flex()
                .gap_3()
                .child(picker)
                .child(hint("display.storedPresetsHint", cx))
                .into_any_element();
        };
        let raw = self
            .doc(SettingsKind::User)
            .and_then(|d| d.value("overview", "overviewProfilePresets"))
            .and_then(|v| v.get(&preset))
            .cloned()
            .unwrap_or(json!({}));
        let included = overview::ids(raw.get("bytes:groups"));
        let query = self.group_search.read(cx).value().trim().to_lowercase();
        let mut groups = overview::group_catalog().to_vec();
        for id in &included {
            if !groups.iter().any(|(n, _)| n == id) {
                groups.push((*id, format!("{} {id}", t!("display.group"))));
            }
        }
        let groups: Vec<_> = groups
            .into_iter()
            .filter(|(id, name)| {
                (!self.selected_groups_only || !query.is_empty() || included.contains(id))
                    && query
                        .split_whitespace()
                        .all(|s| name.to_lowercase().contains(s) || id.to_string().contains(s))
            })
            .collect();
        let pages = groups.len().div_ceil(40).max(1);
        let page = self.group_page.min(pages - 1);
        let group_rows: Vec<_> = groups
            .iter()
            .skip(page * 40)
            .take(40)
            .map(|(id, name)| {
                let id = *id;
                let checked = included.contains(&id);
                let preset = preset.clone();
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .child(format!("{name} ({id})")),
                    )
                    .child(
                        Switch::new(SharedString::from(format!("filter-group-{id}")))
                            .small()
                            .checked(checked)
                            .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                this.filter_list(&preset, "groups", id, *checked, cx)
                            })),
                    )
            })
            .collect();
        let state_rows: Vec<_> = overview::STATES
            .iter()
            .map(|(id, name)| {
                let id = *id;
                let hide = overview::ids(raw.get("bytes:filteredStates")).contains(&id);
                let always = overview::ids(raw.get("bytes:alwaysShownStates")).contains(&id);
                let p1 = preset.clone();
                let p2 = preset.clone();
                h_flex()
                    .gap_2()
                    .child(div().flex_1().text_sm().child(name.to_string()))
                    .child(
                        Switch::new(SharedString::from(format!("filter-hide-{id}")))
                            .small()
                            .checked(hide)
                            .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                this.filter_list(&p1, "filteredStates", id, *on, cx)
                            })),
                    )
                    .child(
                        Switch::new(SharedString::from(format!("filter-always-{id}")))
                            .small()
                            .checked(always)
                            .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                this.filter_list(&p2, "alwaysShownStates", id, *on, cx)
                            })),
                    )
            })
            .collect();
        v_flex()
            .gap_3()
            .child(picker)
            .child(hint("display.presetSharedHint", cx))
            .child(
                Input::new(&self.group_search)
                    .id("overview-groups-search")
                    .small()
                    .cleanable(true),
            )
            .child(
                Button::new("overview-selected-groups")
                    .ghost()
                    .small()
                    .selected(self.selected_groups_only)
                    .label(t!("display.selectedGroupsOnly").to_string())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selected_groups_only = !this.selected_groups_only;
                        this.group_page = 0;
                        cx.emit(DisplayChanged);
                        cx.notify();
                    })),
            )
            .children(group_rows)
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("groups-previous")
                            .ghost()
                            .small()
                            .label("←")
                            .disabled(page == 0)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.group_page = this.group_page.saturating_sub(1);
                                cx.emit(DisplayChanged);
                                cx.notify();
                            })),
                    )
                    .child(format!("{} / {} · {}", page + 1, pages, groups.len()))
                    .child(
                        Button::new("groups-next")
                            .ghost()
                            .small()
                            .label("→")
                            .disabled(page + 1 >= pages)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.group_page += 1;
                                cx.emit(DisplayChanged);
                                cx.notify();
                            })),
                    ),
            )
            .child(ui::section_divider(
                t!("display.stateFilters").to_string(),
                None,
                cx,
            ))
            .child(hint("display.stateFiltersHint", cx))
            .children(state_rows)
            .into_any_element()
    }
    fn render_appearance(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let background = self.appearance_background;
        let style = if background { "background" } else { "flag" };
        let enabled_key = if background {
            "backgroundStates2"
        } else {
            "flagStates2"
        };
        let enabled = self
            .doc(SettingsKind::User)
            .and_then(|d| d.value("overview", enabled_key))
            .filter(|v| v.is_array())
            .map(|v| overview::ids(Some(v)))
            .unwrap_or_else(|| overview::default_states(background).to_vec());
        let rows: Vec<_> = overview::STATES
            .iter()
            .map(|(id, name)| {
                let id = *id;
                let key = overview::color_key(style, id);
                let color = self
                    .doc(SettingsKind::User)
                    .and_then(|d| d.value("overview", "stateColors"))
                    .and_then(|v| v.get(&key))
                    .and_then(|v| v.get("tuple"))
                    .and_then(Value::as_array);
                let hex = color
                    .filter(|a| a.len() >= 3)
                    .map(|a| {
                        format!(
                            "{:02X}{:02X}{:02X}",
                            (a[0].as_f64().unwrap_or(0.) * 255.) as u8,
                            (a[1].as_f64().unwrap_or(0.) * 255.) as u8,
                            (a[2].as_f64().unwrap_or(0.) * 255.) as u8
                        )
                    })
                    .unwrap_or_else(|| t!("display.defaultColor").to_string());
                let color_key = key.clone();
                let options = [
                    "default", "FF3333", "FF9900", "FFFF33", "33CC33", "33CCCC", "3388FF",
                    "AA44CC", "FFFFFF", "999999",
                ]
                .into_iter()
                .map(|s| {
                    (
                        s.to_owned(),
                        if s == "default" {
                            t!("display.defaultColor").to_string()
                        } else {
                            format!("#{s}")
                        },
                    )
                })
                .collect();
                let color_picker = self.picker(
                    &format!("state-color-{id}"),
                    hex,
                    options,
                    String::new(),
                    move |this, choice, cx| {
                        this.edit(
                            SettingsKind::User,
                            "overview",
                            "stateColors",
                            |v| {
                                if let Some(map) = v.as_object_mut() {
                                    if let Some(rgb) = parse_color(&choice) {
                                        map.insert(
                                            color_key.clone(),
                                            json!({"tuple":[rgb[0],rgb[1],rgb[2],1.0]}),
                                        );
                                    } else {
                                        map.remove(&color_key);
                                    }
                                }
                            },
                            cx,
                        )
                    },
                    cx,
                );
                let blink = self
                    .doc(SettingsKind::User)
                    .and_then(|d| d.value("overview", "stateBlinks"))
                    .and_then(|v| v.get(&key))
                    .and_then(boolean)
                    .unwrap_or_else(|| overview::default_blink(background, id));
                let on = enabled.contains(&id);
                v_flex()
                    .gap_1()
                    .pb_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().text_sm().child(name.to_string()))
                            .child(
                                Switch::new(SharedString::from(format!("state-enabled-{id}")))
                                    .small()
                                    .checked(on)
                                    .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                        let on = *on;
                                        this.edit(
                                            SettingsKind::User,
                                            "overview",
                                            enabled_key,
                                            |v| {
                                                if !v.is_array() {
                                                    *v =
                                                        json!(overview::default_states(background));
                                                }
                                                if let Some(a) = v.as_array_mut() {
                                                    a.retain(|v| integer(v) != Some(id));
                                                    if on {
                                                        a.push(json!(id));
                                                    }
                                                }
                                            },
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().child(color_picker))
                            .child(div().text_xs().child(t!("display.blink").to_string()))
                            .child(
                                Switch::new(SharedString::from(format!("state-blink-{id}")))
                                    .small()
                                    .checked(blink)
                                    .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                        let on = *on;
                                        this.edit(
                                            SettingsKind::User,
                                            "overview",
                                            "stateBlinks",
                                            |v| {
                                                if let Some(map) = v.as_object_mut() {
                                                    map.insert(key.clone(), json!(on));
                                                }
                                            },
                                            cx,
                                        );
                                    })),
                            ),
                    )
            })
            .collect();
        v_flex()
            .gap_3()
            .child(hint("display.appearanceHint", cx))
            .child(
                h_flex()
                    .gap_1()
                    .children([false, true].into_iter().map(|background| {
                        Button::new(if background {
                            "appearance-background"
                        } else {
                            "appearance-tags"
                        })
                        .outline()
                        .small()
                        .flex_1()
                        .selected(self.appearance_background == background)
                        .label(
                            if background {
                                t!("display.backgroundColors")
                            } else {
                                t!("display.tagColors")
                            }
                            .to_string(),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.appearance_background = background;
                            this.scroll.reset();
                            cx.emit(DisplayChanged);
                            cx.notify();
                        }))
                    })),
            )
            .children(rows)
            .into_any_element()
    }
}
