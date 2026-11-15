use super::*;

// JSON pointers identify existing leaves without changing dictionary key types,
// tuple encodings or timestamps. Large presets are paged before inputs are built.
fn leaves(
    value: &Value,
    pointer: &str,
    label: &str,
    out: &mut Vec<(String, String, Value)>,
    labels: bool,
) {
    if labels && pointer.ends_with("/bytes:color") {
        out.push((
            pointer.to_owned(),
            label.trim_start_matches(" / ").to_owned(),
            value.clone(),
        ));
        return;
    }
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                let escaped = key.replace('~', "~0").replace('/', "~1");
                let name = key
                    .strip_prefix("bytes:")
                    .or_else(|| key.strip_prefix("utf8:"))
                    .unwrap_or(key);
                leaves(
                    value,
                    &format!("{pointer}/{escaped}"),
                    &format!("{label} / {name}"),
                    out,
                    labels,
                );
            }
        }
        Value::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                leaves(
                    value,
                    &format!("{pointer}/{index}"),
                    &format!("{label} / {}", index + 1),
                    out,
                    labels,
                );
            }
        }
        Value::Null
            if [
                "bytes:fontsize",
                "bytes:color",
                "bytes:pre",
                "bytes:post",
                "bytes:type",
                "bytes:state",
                "bytes:bold",
                "bytes:italic",
                "bytes:underline",
            ]
            .iter()
            .any(|key| pointer.ends_with(&format!("/{key}"))) =>
        {
            out.push((
                pointer.to_owned(),
                label.trim_start_matches(" / ").to_owned(),
                Value::Null,
            ));
        }
        Value::Null => {}
        _ => out.push((
            pointer.to_owned(),
            label.trim_start_matches(" / ").to_owned(),
            value.clone(),
        )),
    }
}

impl DisplayEditor {
    pub(super) fn render_ships(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let labels = self
            .doc(SettingsKind::User)
            .and_then(|doc| doc.value("overview", "shipLabels"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut lines = Vec::new();
        let mut current = h_flex().min_h(px(6.));
        for item in labels
            .iter()
            .filter(|item| item.get("bytes:state").and_then(boolean).unwrap_or(true))
        {
            let kind = item.get("bytes:type").and_then(text).unwrap_or("");
            if kind.is_empty() || kind.eq_ignore_ascii_case("linebreak") {
                lines.push(current);
                current = h_flex().min_h(px(6.));
                continue;
            }
            let sample = match kind.to_lowercase().as_str() {
                "ship type" => "Arazu",
                "pilot name" => "Example Character",
                "corp ticker" | "corporation" | "corp" => "WWWWW",
                "alliance" | "alliance short name" => "WWWWW",
                "ship name" => "Example ship",
                "" => "",
                _ => kind,
            };
            let color = item
                .get("bytes:color")
                .and_then(|v| v.get("tuple"))
                .and_then(Value::as_array)
                .filter(|a| a.len() >= 3)
                .map(|a| {
                    rgb(((a[0].as_f64().unwrap_or(1.) * 255.) as u32) << 16
                        | ((a[1].as_f64().unwrap_or(1.) * 255.) as u32) << 8
                        | (a[2].as_f64().unwrap_or(1.) * 255.) as u32)
                })
                .unwrap_or(rgb(0xdddddd));
            let span = div()
                .flex_none()
                .whitespace_nowrap()
                .text_color(color)
                .text_size(px(
                    item.get("bytes:fontsize").and_then(integer).unwrap_or(12) as f32,
                ))
                .when(
                    item.get("bytes:bold").and_then(boolean) == Some(true),
                    |this| this.font_weight(FontWeight::BOLD),
                )
                .when(
                    item.get("bytes:italic").and_then(boolean) == Some(true),
                    |this| this.italic(),
                )
                .when(
                    item.get("bytes:underline").and_then(boolean) == Some(true),
                    |this| this.underline(),
                )
                .min_h(px(6.))
                .child(format!(
                    "{}{}{}",
                    item.get("bytes:pre").and_then(text).unwrap_or(""),
                    sample,
                    item.get("bytes:post").and_then(text).unwrap_or("")
                ));
            current = current.child(span);
        }
        lines.push(current);
        let preview = v_flex()
            .p_2()
            .bg(rgb(0x080808))
            .overflow_hidden()
            .children(lines);
        let mut rows = Vec::new();
        for (index, item) in labels.iter().enumerate() {
            let name = item
                .get("bytes:type")
                .and_then(text)
                .unwrap_or("Linebreak")
                .to_owned();
            let mut row =
                v_flex()
                    .gap_2()
                    .pb_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().text_sm().child(name))
                            .children([(-1isize, "↑"), (1, "↓")].into_iter().map(
                                |(delta, label)| {
                                    Button::new(SharedString::from(format!(
                                        "ship-label-order-{index}-{delta}"
                                    )))
                                    .ghost()
                                    .xsmall()
                                    .icon(if delta < 0 {
                                        IconName::ArrowUp
                                    } else {
                                        IconName::ArrowDown
                                    })
                                    .tooltip(label)
                                    .disabled(
                                        (delta < 0 && index == 0)
                                            || (delta > 0 && index + 1 >= labels.len()),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.reorder_record("shipLabels", index, delta, cx)
                                    }))
                                },
                            )),
                    );
            for (field, title) in [
                ("state", "Show"),
                ("bold", "Bold"),
                ("italic", "Italic"),
                ("underline", "Underline"),
            ] {
                let enabled = item
                    .get(format!("bytes:{field}"))
                    .and_then(boolean)
                    .unwrap_or(false);
                row = row.child(
                    h_flex()
                        .gap_2()
                        .child(div().flex_1().text_xs().child(title))
                        .child(
                            Switch::new(SharedString::from(format!("ship-label-{index}-{field}")))
                                .small()
                                .checked(enabled)
                                .on_click(cx.listener(move |this, on: &bool, _, cx| {
                                    this.edit(
                                        SettingsKind::User,
                                        "overview",
                                        "shipLabels",
                                        |v| {
                                            if let Some(map) =
                                                v.get_mut(index).and_then(Value::as_object_mut)
                                            {
                                                map.insert(format!("bytes:{field}"), json!(*on));
                                            }
                                        },
                                        cx,
                                    );
                                })),
                        ),
                );
            }
            for (field, title) in [
                ("pre", "Prefix"),
                ("post", "Suffix"),
                ("fontsize", "Font size"),
            ] {
                if item.get(format!("bytes:{field}")).is_some() {
                    row = row.child(self.input(
                        Field::OverviewLeaf("shipLabels".into(), format!("/{index}/bytes:{field}")),
                        title.into(),
                        window,
                        cx,
                    ));
                }
            }
            let color = item
                .get("bytes:color")
                .and_then(|v| v.get("tuple"))
                .and_then(Value::as_array)
                .filter(|a| a.len() >= 3)
                .map(|a| {
                    [
                        a[0].as_f64().unwrap_or(1.) as f32,
                        a[1].as_f64().unwrap_or(1.) as f32,
                        a[2].as_f64().unwrap_or(1.) as f32,
                    ]
                });
            row = row.child(self.color_control(
                &format!("ship-label-{index}"),
                color,
                window,
                move |this, color, cx| {
                    this.edit(
                        SettingsKind::User,
                        "overview",
                        "shipLabels",
                        |v| {
                            if let Some(map) = v.get_mut(index).and_then(Value::as_object_mut) {
                                map.insert(
                                    "bytes:color".into(),
                                    color
                                        .map(|rgb| json!({"tuple":[rgb[0],rgb[1],rgb[2],1.]}))
                                        .unwrap_or(Value::Null),
                                );
                            }
                        },
                        cx,
                    );
                },
                cx,
            ));
            rows.push(row);
        }
        v_flex().gap_3().child(div().text_sm().child("Ship / object labels in space"))
            .child(div().text_xs().child("Changes apply to bracket text in game. Enable Always Show Bracket Text in Settings to show labels for unselected objects."))
            .child(preview)
            .children(rows)
            .when(labels.is_empty(), |this| this.child(div().text_sm().child("No saved ship labels. Configure Ships in EVE once, then reload the account.")))
            .into_any_element()
    }

    fn record_membership(&self, key: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        let choices: Vec<(Value, String)> = match key {
            "alwaysShow" | "filterOut" | "unfiltered" => overview::STATES
                .iter()
                .map(|(id, name)| (json!(id), (*name).to_owned()))
                .collect(),
            "overviewColumns" | "overviewColumnOrder" => overview::COLUMNS
                .iter()
                .map(|(id, name, _)| (json!(format!("bytes:{id}")), (*name).to_owned()))
                .collect(),
            _ => return None,
        };
        let current = self
            .doc(SettingsKind::User)
            .and_then(|doc| doc.value("overview", key))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Some(
            v_flex()
                .gap_2()
                .children(
                    choices
                        .into_iter()
                        .enumerate()
                        .map(|(index, (value, name))| {
                            let checked = current.contains(&value);
                            let key = key.to_owned();
                            h_flex()
                                .gap_2()
                                .child(div().flex_1().text_sm().child(name))
                                .child(
                                    Switch::new(SharedString::from(format!(
                                        "record-member-{index}"
                                    )))
                                    .small()
                                    .checked(checked)
                                    .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                        let value = value.clone();
                                        this.edit(
                                            SettingsKind::User,
                                            "overview",
                                            &key,
                                            move |record| {
                                                if record.is_null() {
                                                    *record = json!([]);
                                                }
                                                if let Some(items) = record.as_array_mut() {
                                                    items.retain(|item| item != &value);
                                                    if *checked {
                                                        items.push(value);
                                                    }
                                                }
                                            },
                                            cx,
                                        );
                                        this.inputs.retain(|id, _| !id.contains("OverviewLeaf"));
                                    })),
                                )
                        }),
                )
                .into_any_element(),
        )
    }
    pub(super) fn render_overview_options(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap_3()
            .children(overview::OPTIONS.iter().map(|&(key, label, default)| {
                let saved = self
                    .doc(SettingsKind::User)
                    .and_then(|d| d.value("overview", key));
                h_flex()
                    .gap_2()
                    .child(div().flex_1().text_sm().child(label))
                    .child(
                        Switch::new(SharedString::from(format!("overview-option-{key}")))
                            .small()
                            .checked(saved.and_then(boolean).unwrap_or(default))
                            .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                let value = match this
                                    .doc(SettingsKind::User)
                                    .and_then(|d| d.value("overview", key))
                                {
                                    Some(Value::Number(_)) => json!(i64::from(*checked)),
                                    Some(Value::String(s)) if s.starts_with("long:") => {
                                        json!(format!("long:{}", i64::from(*checked)))
                                    }
                                    _ => json!(*checked),
                                };
                                this.set(SettingsKind::User, "overview", key, value, cx);
                            })),
                    )
            }))
            .into_any_element()
    }

    fn reorder_record(&mut self, key: &str, index: usize, delta: isize, cx: &mut Context<Self>) {
        self.edit(
            SettingsKind::User,
            "overview",
            key,
            |value| {
                if let Some(items) = value.as_array_mut() {
                    let dest = index.saturating_add_signed(delta);
                    if index < items.len() && dest < items.len() {
                        items.swap(index, dest);
                    }
                }
            },
            cx,
        );
        self.inputs.retain(|id, _| !id.contains("OverviewLeaf"));
    }

    pub(super) fn render_record(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(key) = self.selected_record.clone() else {
            return hint("display.chooseAccount", cx);
        };
        let Some(value) = self
            .doc(SettingsKind::User)
            .and_then(|doc| doc.value("overview", &key))
            .cloned()
        else {
            return hint("display.chooseAccount", cx);
        };
        let mut entries = Vec::new();
        leaves(&value, "", "", &mut entries, key == "shipLabels");
        entries.retain(|(_, _, value)| !value.is_null() || key == "shipLabels");
        const PAGE_SIZE: usize = 24;
        self.record_page = self
            .record_page
            .min(entries.len().saturating_sub(1) / PAGE_SIZE);
        let pages = entries.len().div_ceil(PAGE_SIZE).max(1);
        let rows: Vec<_> = entries
            .iter()
            .skip(self.record_page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .map(|(pointer, label, value)| {
                let label = if label.is_empty() {
                    key.clone()
                } else {
                    label.clone()
                };
                if let Value::Bool(checked) = value {
                    let key = key.clone();
                    let pointer = pointer.clone();
                    h_flex()
                        .gap_2()
                        .child(div().flex_1().text_sm().child(label))
                        .child(
                            Switch::new(SharedString::from(format!(
                                "overview-leaf-{key}-{pointer}"
                            )))
                            .small()
                            .checked(*checked)
                            .on_click(cx.listener(
                                move |this, checked: &bool, _, cx| {
                                    let pointer = pointer.clone();
                                    this.edit(
                                        SettingsKind::User,
                                        "overview",
                                        &key,
                                        move |record| {
                                            if let Some(leaf) = record.pointer_mut(&pointer) {
                                                *leaf = json!(*checked);
                                            }
                                        },
                                        cx,
                                    );
                                },
                            )),
                        )
                        .into_any_element()
                } else {
                    self.input(
                        Field::OverviewLeaf(key.clone(), pointer.clone()),
                        label,
                        window,
                        cx,
                    )
                }
            })
            .collect();
        let order: Vec<_> = value
            .as_array()
            .filter(|items| items.len() <= 100)
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(index, item)| {
                let name = item
                    .get("bytes:type")
                    .and_then(text)
                    .map(str::to_owned)
                    .or_else(|| text(item).map(str::to_owned))
                    .unwrap_or_else(|| format!("Entry {}", index + 1));
                let up_key = key.clone();
                let down_key = key.clone();
                h_flex()
                    .gap_2()
                    .child(div().flex_1().text_sm().child(name))
                    .child(
                        Button::new(SharedString::from(format!("record-up-{index}")))
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowUp)
                            .disabled(index == 0 || self.invalid_for(SettingsKind::User))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.reorder_record(&up_key, index, -1, cx)
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("record-down-{index}")))
                            .ghost()
                            .xsmall()
                            .icon(IconName::ArrowDown)
                            .disabled(
                                index + 1 >= value.as_array().map_or(0, Vec::len)
                                    || self.invalid_for(SettingsKind::User),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.reorder_record(&down_key, index, 1, cx)
                            })),
                    )
            })
            .collect();
        v_flex().gap_3()
            .child(div().text_sm().child(key))
            .children(order)
            .when_some(self.record_membership(self.selected_record.as_deref().unwrap_or_default(), cx), |this, membership| this.child(membership))
            .child(div().text_xs().text_color(cx.theme().muted_foreground)
                .child("Saved values retain their structure and value types. Empty label fields use the EVE default; a new label color uses six hex digits. Use Tabs & columns and Filters to change membership."))
            .child(h_flex().gap_2()
                .child(Button::new("overview-record-previous").outline().xsmall().label("Previous")
                    .disabled(self.record_page == 0)
                    .on_click(cx.listener(|this, _, _, cx| { this.record_page = this.record_page.saturating_sub(1); this.scroll.reset(); cx.notify(); })))
                .child(format!("{} / {pages}", self.record_page + 1))
                .child(Button::new("overview-record-next").outline().xsmall().label("Next")
                    .disabled(self.record_page + 1 >= pages)
                    .on_click(cx.listener(|this, _, _, cx| { this.record_page += 1; this.scroll.reset(); cx.notify(); }))))
            .children(rows)
            .into_any_element()
    }
}
