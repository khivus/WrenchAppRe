use super::{LayoutEditor, WindowState, canvas::Mapping, model};
use eve_wrench_core::{
    SettingsKind,
    display_settings::{chat::ChatLayout, overview},
};
use gpui_kit::{
    component::{ActiveTheme as _, h_flex, v_flex},
    prelude::FluentBuilder as _,
    *,
};
use rust_i18n::t;

fn cell(text: impl Into<SharedString>, width: f32, scale: f32, cx: &App) -> Div {
    div()
        .w(px(width * scale))
        .flex_none()
        .overflow_hidden()
        .whitespace_nowrap()
        .border_r_1()
        .border_color(cx.theme().border.opacity(0.4))
        .px(px(3. * scale))
        .child(text.into())
}
fn line(text: impl Into<SharedString>, scale: f32) -> Div {
    div()
        .h(px(20. * scale))
        .flex_none()
        .overflow_hidden()
        .whitespace_nowrap()
        .px(px(4. * scale))
        .child(text.into())
}
fn sample(column: &str, index: usize) -> String {
    match column {
        "ICON" => ["◇", "△", "□", "◈", "○"][index].into(),
        "NAME" => [
            "Example pilot",
            "Scout One",
            "Logistics Two",
            "Station services",
            "Fleet commander",
        ][index]
            .into(),
        "TYPE" => ["Arazu", "Amarr Shuttle", "Basilisk", "Astrahus", "Loki"][index].into(),
        "DISTANCE" => ["3,651 m", "4,455 m", "15.6 km", "1.2 AU", "87.3 km"][index].into(),
        "TAG" => ["A", "B", "C", "D", "E"][index].into(),
        "CORPORATION" => "[DEMO]".into(),
        "ALLIANCE" => "[TEST]".into(),
        "FACTION" => "Caldari State".into(),
        "MILITIA" => "State Protectorate".into(),
        "SIZE" => format!("{} m", [135, 25, 115, 1000, 160][index]),
        "VELOCITY" => format!("{} m/s", [325, 0, 170, 0, 1450][index]),
        "RADIALVELOCITY" => format!("{} m/s", [-24, 0, 85, 0, -112][index]),
        "TRANSVERSALVELOCITY" => format!("{} m/s", [74, 0, 67, 0, 350][index]),
        "ANGULARVELOCITY" => [
            "0.021 rad/s",
            "0 rad/s",
            "0.004 rad/s",
            "0 rad/s",
            "0.012 rad/s",
        ][index]
            .into(),
        _ => "—".into(),
    }
}

impl LayoutEditor {
    pub(super) fn render_overview_contents(
        &self,
        state: &WindowState,
        mapping: Mapping,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let instance = if state.id() == "overview" {
            0
        } else {
            state
                .id()
                .strip_prefix("overview_")?
                .parse::<usize>()
                .ok()?
        };
        let (tabs, tab, columns, sort) = {
            let settings = &self.preview_settings;
            let account = settings.doc(SettingsKind::User)?;
            let character = settings.doc(SettingsKind::Char)?;
            let tabs = overview::assigned_tabs(account, instance);
            let tab = tabs
                .iter()
                .find(|t| self.preview_overview_tabs.get(&state.key) == Some(&t.id))
                .or_else(|| tabs.first())?
                .clone();
            let raw = account
                .value("overview", "tabsettings_new")?
                .get(&tab.key)?;
            let shown = raw
                .get("bytes:tabColumns")
                .map(|v| overview::strings(Some(v)))
                .unwrap_or_else(|| {
                    overview::DEFAULT_COLUMNS
                        .iter()
                        .map(|s| s.to_string())
                        .collect()
                });
            let columns: Vec<_> = tab
                .columns
                .iter()
                .filter(|c| shown.contains(c))
                .map(|c| (c.clone(), overview::column_width(character, tab.id, c)))
                .collect();
            let sort = overview::sort(
                character,
                tab.id,
                &columns.iter().map(|(c, _)| c.clone()).collect::<Vec<_>>(),
            );
            (tabs, tab, columns, sort)
        };
        let scale = mapping.scale;
        let mut body = v_flex()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .text_size(px(11. * scale))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                h_flex()
                    .h(px(24. * scale))
                    .flex_none()
                    .overflow_hidden()
                    .children(tabs.into_iter().map(|item| {
                        let key = state.key.clone();
                        let selected = item.id == tab.id;
                        let color = item
                            .color
                            .map(|c| {
                                rgb(((c[0] * 255.) as u32) << 16
                                    | ((c[1] * 255.) as u32) << 8
                                    | (c[2] * 255.) as u32)
                            })
                            .unwrap_or(rgb(0xaaaaaa));
                        div()
                            .id(SharedString::from(format!(
                                "overview-tab-{}-{}",
                                state.key, item.id
                            )))
                            .test_support()
                            .h_full()
                            .px(px(7. * scale))
                            .flex_none()
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .border_b_1()
                            .border_color(if selected {
                                color.into()
                            } else {
                                cx.theme().border
                            })
                            .text_color(color)
                            .cursor_pointer()
                            .child(item.name)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.select(Some(key.clone()), window, cx);
                                this.settings.update(cx, |settings, cx| {
                                    settings.select_overview_tab(item.key.clone(), cx)
                                });
                                this.settings_open = true;
                                this.preview_overview_tabs.insert(key.clone(), item.id);
                                cx.notify();
                            }))
                    })),
            )
            .child(
                h_flex()
                    .h(px(20. * scale))
                    .flex_none()
                    .overflow_hidden()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .children(columns.iter().map(|(column, width)| {
                        let sorted = sort.as_ref().filter(|(c, _)| c == column);
                        let label = if column == "ICON" {
                            "◇".to_string()
                        } else {
                            t!(format!("display.columns.{column}")).to_string()
                        };
                        let column_id = column.clone();
                        let tab_id = tab.id;
                        let settings = self.settings.clone();
                        let ascending = sorted.map(|(_, ascending)| !*ascending).unwrap_or(true);
                        h_flex()
                            .id(SharedString::from(format!(
                                "overview-header-{}-{column}",
                                state.key
                            )))
                            .test_support()
                            .w(px(*width as f32 * scale))
                            .h_full()
                            .flex_none()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .cursor_pointer()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .px(px(2. * scale))
                            .child(div().flex_1().min_w_0().truncate().child(label))
                            .when_some(sorted, |this, (_, ascending)| {
                                this.child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "overview-sort-{}-{column}",
                                            state.key
                                        )))
                                        .test_support()
                                        .flex_none()
                                        .child(if *ascending { "▴" } else { "▾" }),
                                )
                            })
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                settings.update(cx, |settings, cx| {
                                    settings.set_overview_sort(tab_id, &column_id, ascending, cx);
                                });
                            })
                    })),
            );
        let mut indexes: Vec<usize> = (0..5).collect();
        if let Some((column, ascending)) = &sort {
            indexes.sort_by(|a, b| {
                let numeric = match column.as_str() {
                    "DISTANCE" => Some([3651., 4455., 15600., 179517444840., 87300.]),
                    "VELOCITY" => Some([325., 0., 170., 0., 1450.]),
                    "SIZE" => Some([135., 25., 115., 1000., 160.]),
                    "RADIALVELOCITY" => Some([-24., 0., 85., 0., -112.]),
                    "TRANSVERSALVELOCITY" => Some([74., 0., 67., 0., 350.]),
                    "ANGULARVELOCITY" => Some([0.021, 0., 0.004, 0., 0.012]),
                    _ => None,
                };
                let order = numeric
                    .map(|v| v[*a].partial_cmp(&v[*b]).unwrap())
                    .unwrap_or_else(|| sample(column, *a).cmp(&sample(column, *b)));
                if *ascending { order } else { order.reverse() }
            });
        }
        for index in indexes {
            body = body.child(
                h_flex()
                    .id(SharedString::from(format!(
                        "overview-sample-{}-{index}",
                        state.key
                    )))
                    .test_support()
                    .h(px(20. * scale))
                    .flex_none()
                    .overflow_hidden()
                    .bg(if index % 2 == 0 {
                        cx.theme().muted.opacity(0.2)
                    } else {
                        transparent_black()
                    })
                    .children(
                        columns
                            .iter()
                            .map(|(c, w)| cell(sample(c, index), *w as f32, scale, cx)),
                    ),
            );
        }
        Some(body.into_any_element())
    }

    pub(super) fn render_window_contents(
        &self,
        state: &WindowState,
        mapping: Mapping,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if state.is_stack() {
            let members: Vec<_> = model::members(&self.windows, &state.key)
                .into_iter()
                .filter(|w| self.show_all_windows || self.active_window(w))
                .collect();
            let selected = members
                .iter()
                .find(|w| self.preview_stack_tabs.get(&state.key) == Some(&w.key))
                .or_else(|| members.first())?;
            let content = self.render_window_contents(selected, mapping, cx);
            return Some(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .h(px(24. * mapping.scale))
                            .flex_none()
                            .overflow_hidden()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .children(members.iter().map(|member| {
                                let stack = state.key.clone();
                                let key = member.key.clone();
                                div()
                                    .id(SharedString::from(format!(
                                        "stack-preview-tab-{}-{}",
                                        state.key, member.key
                                    )))
                                    .test_support()
                                    .flex_none()
                                    .px(px(6. * mapping.scale))
                                    .text_size(px(11. * mapping.scale))
                                    .whitespace_nowrap()
                                    .border_b_1()
                                    .border_color(if member.key == selected.key {
                                        cx.theme().primary
                                    } else {
                                        cx.theme().border
                                    })
                                    .cursor_pointer()
                                    .child(model::window_name(member.id()))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.preview_stack_tabs.insert(stack.clone(), key.clone());
                                        cx.notify();
                                    }))
                            })),
                    )
                    .when_some(content, |this, content| this.child(content))
                    .into_any_element(),
            );
        }
        if state.id() == "overview" || state.id().starts_with("overview_") {
            return self.render_overview_contents(state, mapping, cx);
        }
        let scale = mapping.scale;
        let id = state.id();
        let base = || {
            v_flex()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_hidden()
                .text_size(px(12. * scale))
        };
        if eve_wrench_core::display_settings::chat::channel(id).is_some() {
            let layout = ChatLayout::read(self.preview_settings.doc(SettingsKind::User), id);
            let messages = v_flex()
                .id(SharedString::from(format!("chat-messages-{}", state.key)))
                .test_support()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .children([("[16:02] EVE System > Channel changed",0xe55b54), ("Scout One > Gate is clear.",0x68bfe4),("Example pilot > Fleet ready.",0xcccccc),("Logistics Two > Align to the station and wait for the next fleet broadcast.",0xcccccc)].into_iter().enumerate().map(|(index,(message,color))|div().id(SharedString::from(format!("chat-message-{}-{index}", state.key))).test_support().flex_none().px(px(4.*scale)).text_color(rgb(color)).child(message)));
            let names = v_flex()
                .id(SharedString::from(format!("chat-members-{}", state.key)))
                .test_support()
                .w(px(layout.member_width as f32 * scale))
                .flex_none()
                .border_l_1()
                .border_color(cx.theme().border)
                .overflow_hidden()
                .children(
                    [
                        "♟ Scout One",
                        "♟ Example pilot",
                        "♟ Logistics Two",
                        "♟ Fleet commander",
                        "♟ Explorer Three",
                        "♟ Industrial Four",
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, name)| {
                        line(name, scale)
                            .id(SharedString::from(format!(
                                "chat-member-{}-{index}",
                                state.key
                            )))
                            .test_support()
                    }),
                );
            return Some(
                base()
                    .id(SharedString::from(format!("chat-preview-{}", state.key)))
                    .test_support()
                    .child(
                        h_flex()
                            .flex_1()
                            .min_h_0()
                            .items_stretch()
                            .overflow_hidden()
                            .child(messages)
                            .when(layout.show_members, |this| this.child(names)),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("chat-input-{}", state.key)))
                            .test_support()
                            .h(px(layout.input_height as f32 * scale))
                            .flex_none()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .overflow_hidden()
                            .p(px(4. * scale))
                            .child(t!("display.sampleInput").to_string()),
                    )
                    .into_any_element(),
            );
        }
        let content = match id {
            "directionalScannerWindow"
            | "directionalScanner"
            | "directionalScannerWnd"
            | "dscanner" => base()
                .child(line("◉   Filter: Ships and structures", scale))
                .child(h_flex().children([
                    cell("Name", 150., scale, cx),
                    cell("Type", 100., scale, cx),
                    cell("Distance ▴", 90., scale, cx),
                ]))
                .children(
                    [
                        ("Scout One", "Zealot", "—"),
                        ("Example pilot", "Arazu", "—"),
                        ("Demo Citadel", "Astrahus", "1,404 km"),
                    ]
                    .map(|(a, b, c)| {
                        h_flex().children([
                            cell(a, 150., scale, cx),
                            cell(b, 100., scale, cx),
                            cell(c, 90., scale, cx),
                        ])
                    }),
                )
                .child(div().flex_1())
                .child(line("14.3 AU     Angle: 360°     Scan", scale)),
            "droneview" => base()
                .child(line("▾ Drones in Bay (20)", scale))
                .children(
                    [
                        "   Hornet EC-300 (5)   ▰▰▰",
                        "   Infiltrator II (5)     ▰▰▰",
                        "   Valkyrie II (5)       ▰▰▰",
                        "   Warrior II (5)        ▰▰▰",
                    ]
                    .map(|s| line(s, scale)),
                )
                .child(div().flex_1())
                .child(line("▾ Drones in Space (0/5)", scale))
                .child(line("   No drones deployed", scale)),
            "fleetwindow" | "fleet" => base()
                .child(line("Fleet   Broadcasts   History", scale))
                .child(line("▾ Fleet (4)", scale))
                .child(line("   ★ Fleet commander", scale))
                .child(line("   ▾ Wing 1", scale))
                .child(line("      ▾ Squad 1", scale))
                .child(line("         Scout One", scale))
                .child(line("         Logistics Two", scale))
                .child(div().flex_1())
                .child(line("⚑ Align to station · 16:02", scale))
                .child(line("✚ Need shield · Example pilot", scale)),
            "watchlistpanel" | "watchlist" | "watchlistWnd" => base().children(
                [
                    "★ Fleet commander   ▰▰▰",
                    "  Scout One              ▰▰▰",
                    "  Logistics Two          ▰▰▰",
                    "  Example pilot          ▰▰▰",
                ]
                .map(|s| line(s, scale)),
            ),
            "selecteditemview" => base()
                .child(line("◇ Example station", scale))
                .child(line("0 m · Astrahus", scale))
                .child(line("↔  ≫  ↪  ◎  ⊙  ⓘ", scale)),
            "probeScannerWindow" | "probeScanner" | "scanner" | "probeScannerWnd" => base()
                .child(line("Analyze     Launch probes", scale))
                .child(line("ID       Group        Signal", scale))
                .child(line("ABC-123  Cosmic signature  74%", scale))
                .child(line("DEF-456  Combat site      100%", scale)),
            id if id.starts_with("Inventory")
                || id.starts_with("inv")
                || id.contains("cargo")
                || id.contains("Cargo")
                || id.contains("Hangar") =>
            {
                base()
                    .child(line("Search                 ▦  ☷", scale))
                    .child(
                        h_flex()
                            .id(SharedString::from(format!("inventory-body-{}", state.key)))
                            .test_support()
                            .flex_1()
                            .min_h_0()
                            .items_stretch()
                            .overflow_hidden()
                            .child(
                                v_flex()
                                    .id(SharedString::from(format!(
                                        "inventory-navigation-{}",
                                        state.key
                                    )))
                                    .test_support()
                                    .w(px(110. * scale))
                                    .flex_none()
                                    .border_r_1()
                                    .border_color(cx.theme().border)
                                    .child(
                                        line("▾ Active ship", scale)
                                            .id(SharedString::from(format!(
                                                "inventory-navigation-first-{}",
                                                state.key
                                            )))
                                            .test_support(),
                                    )
                                    .child(line("   Cargo hold", scale))
                                    .child(line("   Drone bay", scale))
                                    .child(line("   Item hangar", scale)),
                            )
                            .child(
                                v_flex()
                                    .id(SharedString::from(format!(
                                        "inventory-items-{}",
                                        state.key
                                    )))
                                    .test_support()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        line("▧ Nanite Repair Paste ×100", scale)
                                            .id(SharedString::from(format!(
                                                "inventory-item-first-{}",
                                                state.key
                                            )))
                                            .test_support(),
                                    )
                                    .child(line("▧ Liquid Ozone ×550", scale))
                                    .child(line("▧ Cap Booster 800 ×20", scale)),
                            ),
                    )
                    .child(line("3 items · 120 / 450 m³", scale))
            }
            _ => return None,
        };
        Some(
            content
                .id(SharedString::from(format!("sample-content-{}", state.key)))
                .test_support()
                .into_any_element(),
        )
    }
}
