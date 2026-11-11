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
