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
            "Example Character",
            "Caldari Citizen",
            "Amarr Citizen",
            "Station services",
            "Minmatar Citizen",
        ][index]
            .into(),
        "TYPE" => ["Arazu", "Amarr Shuttle", "Basilisk", "Astrahus", "Loki"][index].into(),
        "DISTANCE" => ["3,651 m", "4,455 m", "15.6 km", "1.2 AU", "87.3 km"][index].into(),
        "TAG" => ["A", "B", "C", "D", "E"][index].into(),
        "CORPORATION" => "[WWWWW]".into(),
        "ALLIANCE" => "[WWWWW]".into(),
        "FACTION" => "Caldari State".into(),
        "MILITIA" => "State Protectorate".into(),
        "SIZE" => format!("{} m", [135, 25, 115, 1000, 160][index]),
        "VELOCITY" => ["528", "527", "678,31", "–", "1 450"][index].into(),
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

// One cached representative per eligible group; sample states only for visible rows. No captured
// player names, ship identities, locations or display measurements enter it.
#[derive(Clone)]
struct GridObject {
    group: i64,
    kind: String,
    states: Vec<i64>,
    variant: usize,
}
fn grid() -> &'static [GridObject] {
    static GRID: std::sync::OnceLock<Vec<GridObject>> = std::sync::OnceLock::new();
    GRID.get_or_init(|| {
        let names: std::collections::HashMap<_, _> =
            overview::group_catalog().iter().cloned().collect();
        overview::categories()
            .into_iter()
            .flat_map(|(_, groups)| groups)
            .flat_map(|group| {
                let kind = match group {
                    25 => "Merlin",
                    26 => "Caracal",
                    27 => "Raven",
                    547 => "Thanatos",
                    6 => "Sun",
                    7 => "Planet",
                    15 => "Caldari Navy Assembly Plant",
                    _ => names
                        .get(&group)
                        .map(String::as_str)
                        .unwrap_or("Unknown object"),
                }
                .to_owned();
                [GridObject {
                    group,
                    kind,
                    states: vec![17],
                    variant: 0,
                }]
            })
            .collect()
    })
}
fn matches_preset(object: &GridObject, groups: &[i64], hidden: &[i64], always: &[i64]) -> bool {
    groups.contains(&object.group)
        && (object.states.iter().any(|id| always.contains(id))
            || !object.states.iter().any(|id| hidden.contains(id)))
}
fn is_player_ship(group: i64) -> bool {
    static SHIPS: std::sync::OnceLock<std::collections::HashSet<i64>> = std::sync::OnceLock::new();
    SHIPS
        .get_or_init(|| {
            overview::categories()
                .into_iter()
                .filter(|(name, _)| name == "Ship")
                .flat_map(|(_, ids)| ids)
                .collect()
        })
        .contains(&group)
}
// Relationship/security flags supplement a standing, never replace it.
fn ship_states(primary: i64) -> Vec<i64> {
    if [15, 16, 17, 18, 19, 48].contains(&primary) {
        vec![primary]
    } else {
        vec![primary, 48]
    }
}
fn preview_name(object: &GridObject, characters: &[String], seed: u64) -> String {
    if !is_player_ship(object.group) {
        return object.kind.clone();
    }
    if characters.is_empty() {
        return grid_cell(object, "NAME");
    }
    characters[(random_key(seed) as usize % characters.len() + object.variant) % characters.len()]
        .clone()
}
// Blend over the dark game backdrop instead of painting a solid RGB stripe.
fn muted_background(color: Rgba, alpha: f32) -> Rgba {
    let opacity = 0.22 * alpha.clamp(0., 1.);
    Rgba {
        r: 0.031 + (color.r - 0.031) * opacity,
        g: 0.031 + (color.g - 0.031) * opacity,
        b: 0.031 + (color.b - 0.031) * opacity,
        a: 1.,
    }
}
fn grid_cell(object: &GridObject, column: &str) -> String {
    match column {
        "TYPE" => object.kind.clone(),
        "NAME" => [
            "Example Character",
            "Caldari Citizen",
            "Amarr Citizen",
            "Minmatar Citizen",
            "Gallente Citizen",
        ][object.variant % 5]
            .into(),
        _ => sample(column, object.variant % 5),
    }
}

fn random_key(value: u64) -> u64 {
    static SEED: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    let seed = *SEED.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    });
    let mut x = value ^ seed;
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
fn type_catalog() -> &'static std::collections::HashMap<i64, Vec<String>> {
    static TYPES: std::sync::OnceLock<std::collections::HashMap<i64, Vec<String>>> =
        std::sync::OnceLock::new();
    TYPES.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../assets/overview-types.json"))
            .expect("bundled EVE types")
    })
}
fn background_color(
    doc: Option<&eve_wrench_core::display_settings::SettingsDocument>,
    state: i64,
) -> Option<Rgba> {
    let enabled = doc
        .and_then(|d| d.value("overview", "backgroundStates2"))
        .map(|v| overview::ids(Some(v)))
        .unwrap_or_else(|| overview::default_states(true).to_vec());
    if !enabled.contains(&state) {
        return None;
    }
    if let Some(color) = doc
        .and_then(|d| d.value("overview", "stateColors"))
        .and_then(|v| v.get(overview::color_key("background", state)))
        .and_then(|v| v.get("tuple"))
        .and_then(serde_json::Value::as_array)
        .filter(|a| a.len() >= 3)
    {
        return Some(muted_background(
            rgb(
                ((color[0].as_f64().unwrap_or(0.) * 255.).round() as u32) << 16
                    | ((color[1].as_f64().unwrap_or(0.) * 255.).round() as u32) << 8
                    | (color[2].as_f64().unwrap_or(0.) * 255.).round() as u32,
            ),
            color
                .get(3)
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(1.) as f32,
        ));
    }
    Some(muted_background(
        rgb(match state {
            11 => 0x800080,
            12 | 66 => 0x008000,
            14 => 0x0080ff,
            15 => 0x0000ff,
            16 => 0x8080ff,
            13 | 44 | 19 | 51 => 0xff0000,
            18 => 0xff8000,
            50 | 52 => 0xffff00,
            _ => 0x808080,
        }),
        1.,
    ))
}
fn sampled_objects(
    groups: &[i64],
    hidden: &[i64],
    always: &[i64],
    all: bool,
    count: usize,
    seed: u64,
    doc: Option<&eve_wrench_core::display_settings::SettingsDocument>,
) -> Vec<GridObject> {
    let types = type_catalog();
    let mut groups: Vec<_> = grid()
        .iter()
        .filter(|o| {
            (all || groups.contains(&o.group)) && types.get(&o.group).is_some_and(|v| !v.is_empty())
        })
        .map(|o| o.group)
        .collect();
    groups.sort_by_cached_key(|g| random_key(*g as u64 ^ seed));
    let mut states: Vec<_> = overview::STATES
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| {
            let states = ship_states(*id);
            states.iter().any(|id| always.contains(id))
                || !states.iter().any(|id| hidden.contains(id))
        })
        .collect();
    states.sort_by_cached_key(|id| random_key(*id as u64 ^ seed));
    // Prefer enabled backgrounds, with one state for each distinct color first.
    let colored: Vec<_> = states
        .iter()
        .copied()
        .filter(|id| background_color(doc, *id).is_some())
        .collect();
    let mut colors = std::collections::HashSet::new();
    let distinct: Vec<_> = colored
        .into_iter()
        .filter(|id| {
            let c = background_color(doc, *id).unwrap();
            colors.insert((
                (c.r * 255.).round() as u32,
                (c.g * 255.).round() as u32,
                (c.b * 255.).round() as u32,
            ))
        })
        .collect();
    let choices = if distinct.is_empty() {
        &states
    } else {
        &distinct
    };
    if choices.is_empty() {
        groups.retain(|group| !is_player_ship(*group));
    }
    if groups.is_empty() {
        return Vec::new();
    }
    let fallback = [48];
    let choices = if choices.is_empty() {
        &fallback[..]
    } else {
        choices.as_slice()
    };
    let mut result = Vec::new();
    let mut used = std::collections::HashSet::new();
    for i in 0..count.clamp(1, 50) * 50 {
        let group = groups[i % groups.len()];
        let names = &types[&group];
        let round = i / groups.len();
        let kind = names[(random_key(group as u64 ^ seed) as usize % names.len()
            + round / choices.len()
            + round % choices.len())
            % names.len()]
        .clone();
        let state = choices[(i % groups.len() + round) % choices.len()];
        if used.insert((kind.clone(), state)) {
            result.push(GridObject {
                group,
                kind,
                states: if is_player_ship(group) {
                    ship_states(state)
                } else {
                    Vec::new()
                },
                variant: i,
            });
            if result.len() >= count.clamp(1, 50) {
                break;
            }
        }
    }
    result
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
            .bg(rgb(0x080808))
            .font_family("Bahnschrift SemiCondensed")
            .text_color(rgb(0xd8d8d8))
            .text_size(px(10.8 * scale))
            .line_height(px(16.2 * scale))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener({
                    let key = state.key.clone();
                    move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.select(Some(key.clone()), window, cx);
                    }
                }),
            )
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
                    .h(px(16.2 * scale))
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
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(label),
                            )
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
        let preset = self
            .preview_settings
            .doc(SettingsKind::User)
            .and_then(|doc| doc.value("overview", "overviewProfilePresets"))
            .and_then(|presets| {
                presets
                    .as_object()?
                    .iter()
                    .find(|(key, _)| {
                        eve_wrench_core::display_settings::text(&serde_json::json!(key))
                            == Some(tab.preset.as_str())
                    })
                    .map(|(_, value)| value)
            });
        if preset.is_none() {
            return Some(
                body.child(line("Filter preset unavailable in this account", scale))
                    .into_any_element(),
            );
        }
        let groups = overview::ids(preset.and_then(|p| p.get("bytes:groups")));
        let hidden = overview::ids(preset.and_then(|p| p.get("bytes:filteredStates")));
        let always = overview::ids(preset.and_then(|p| p.get("bytes:alwaysShownStates")));
        static POLICIES: std::sync::OnceLock<std::collections::HashMap<i64, u8>> =
            std::sync::OnceLock::new();
        let policies = POLICIES.get_or_init(|| {
            overview::categories()
                .into_iter()
                .flat_map(|(name, ids)| {
                    let policy = match name.as_str() {
                        "Ship" | "Drone" | "Fighter" | "Entity" | "NPC" => 0,
                        "Structure" | "Station" | "Starbase" | "Sovereignty Structures" => 1,
                        _ => 2,
                    };
                    ids.into_iter().map(move |id| (id, policy))
                })
                .collect()
        });
        let option = |key| {
            self.preview_settings
                .doc(SettingsKind::User)
                .and_then(|d| d.value("overview", key))
                .and_then(eve_wrench_core::display_settings::boolean)
                .unwrap_or(false)
        };
        let structures = option("applyToStructures");
        let others = option("applyToOtherObjects");
        let count = self
            .store
            .read(cx)
            .config()
            .overview_preview_rows
            .clamp(1, 50);
        let mut objects = sampled_objects(
            &groups,
            &hidden,
            &always,
            preset.is_none(),
            count,
            tab.id as u64 ^ instance as u64,
            self.preview_settings.doc(SettingsKind::User),
        );
        objects.retain(|o| {
            let apply = match policies.get(&o.group) {
                Some(0) => true,
                Some(1) => structures,
                _ => others,
            };
            preset.is_none()
                || matches_preset(
                    o,
                    &groups,
                    if apply { &hidden } else { &[] },
                    if apply { &always } else { &[] },
                )
        });
        if let Some((column, ascending)) = &sort {
            objects.sort_by_cached_key(|object| {
                let numeric = match column.as_str() {
                    "DISTANCE" => Some([3651i64, 4455, 15600, 179517444840, 87300]),
                    "VELOCITY" => Some([528, 527, 678, 0, 1450]),
                    "SIZE" => Some([135, 25, 115, 1000, 160]),
                    "RADIALVELOCITY" => Some([-24, 0, 85, 0, -112]),
                    "TRANSVERSALVELOCITY" => Some([74, 0, 67, 0, 350]),
                    "ANGULARVELOCITY" => Some([21, 0, 4, 0, 12]),
                    _ => None,
                };
                (
                    numeric.map(|v| v[object.variant % 5]),
                    if numeric.is_none() {
                        grid_cell(object, column)
                    } else {
                        String::new()
                    },
                )
            });
            if !ascending {
                objects.reverse();
            }
        }
        let mut character_names: Vec<_> = self
            .store
            .read(cx)
            .data()
            .map(|data| {
                data.entries()
                    .filter(|entry| entry.kind == SettingsKind::Char)
                    .filter_map(|entry| {
                        entry
                            .character
                            .as_ref()
                            .map(|c| c.name.clone())
                            .or_else(|| {
                                let name = entry.display_name.trim();
                                (!name.is_empty()
                                    && name != entry.id
                                    && !name.chars().all(|c| c.is_ascii_digit()))
                                .then(|| name.to_owned())
                            })
                    })
                    .filter(|name| !name.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        // Stable per-overview permutation: zooming must not reshuffle pilots.
        character_names.sort();
        character_names.dedup();
        let name_seed = tab.id as u64 ^ (instance as u64).wrapping_mul(0x9e3779b9);
        character_names.sort_by_cached_key(|name| {
            use std::hash::{Hash, Hasher};
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            name.hash(&mut hash);
            random_key(hash.finish() ^ name_seed)
        });
        let limit = count;
        for (index, object) in objects.into_iter().take(limit).enumerate() {
            body = body.child(
                h_flex()
                    .id(SharedString::from(format!(
                        "overview-sample-{}-{index}",
                        state.key
                    )))
                    .test_support()
                    .h(px(16.2 * scale))
                    .flex_none()
                    .overflow_hidden()
                    .bg(object
                        .states
                        .first()
                        .and_then(|id| {
                            background_color(self.preview_settings.doc(SettingsKind::User), *id)
                        })
                        .unwrap_or(rgb(0x080808)))
                    .children(columns.iter().map(|(c, w)| {
                        cell(
                            if c == "NAME" {
                                preview_name(&object, &character_names, name_seed)
                            } else {
                                grid_cell(&object, c)
                            },
                            *w as f32,
                            scale,
                            cx,
                        )
                    })),
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
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        cx.stop_propagation();
                                        this.preview_stack_tabs.insert(stack.clone(), key.clone());
                                        this.select(Some(stack.clone()), window, cx);
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
                .children([("[16:02] EVE System > Channel changed",0xe55b54), ("Caldari Citizen > Gate is clear.",0x68bfe4),("Example Character > Fleet ready.",0xcccccc),("Amarr Citizen > Align to the station and wait for the next fleet broadcast.",0xcccccc)].into_iter().enumerate().map(|(index,(message,color))|div().id(SharedString::from(format!("chat-message-{}-{index}", state.key))).test_support().flex_none().px(px(4.*scale)).text_color(rgb(color)).child(message)));
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
                        "♟ Caldari Citizen",
                        "♟ Example Character",
                        "♟ Amarr Citizen",
                        "♟ Minmatar Citizen",
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
            let input = div()
                .id(SharedString::from(format!("chat-input-{}", state.key)))
                .test_support()
                .h(px(layout.input_height as f32 * scale))
                .flex_none()
                .border_t_1()
                .border_color(cx.theme().border)
                .overflow_hidden()
                .p(px(4. * scale))
                .child(t!("display.sampleInput").to_string());
            return Some(
                base()
                    .id(SharedString::from(format!("chat-preview-{}", state.key)))
                    .test_support()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let key = state.key.clone();
                            move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.select(Some(key.clone()), window, cx);
                            }
                        }),
                    )
                    .child(
                        h_flex()
                            .flex_1()
                            .min_h_0()
                            .items_stretch()
                            .overflow_hidden()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .min_h_0()
                                    .overflow_hidden()
                                    .child(messages)
                                    .child(input),
                            )
                            .when(layout.show_members, |this| this.child(names)),
                    )
                    .into_any_element(),
            );
        }
        // Window geometry is editable, but internal controls without a supported
        // settings editor must not pretend to be an accurate game preview.
        None
    }
}

#[cfg(test)]
mod grid_tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn friendly_samples_do_not_bypass_standing_exclusions_with_security_flags() {
        let hidden = [17, 18, 19, 48];
        let always = [11, 12, 14, 15, 16];
        let rows = sampled_objects(&[30], &hidden, &always, false, 50, 4, None);
        assert!(rows.len() >= 5 && rows.len() <= 50);
        assert!(
            rows.iter()
                .all(|o| o.states.iter().any(|id| always.contains(id)))
        );
        for state in [9, 10, 50, 51, 52, 53] {
            let object = GridObject {
                group: 30,
                kind: "Avatar".into(),
                states: ship_states(state),
                variant: 0,
            };
            assert!(!matches_preset(&object, &[30], &hidden, &always));
        }
    }
    #[::core::prelude::v1::test]
    fn character_names_are_ship_only_and_backgrounds_are_dimmed() {
        let names = vec!["Pilot A".to_owned(), "Pilot B".to_owned()];
        let mut object = GridObject {
            group: 30,
            kind: "Avatar".into(),
            states: vec![11],
            variant: 0,
        };
        assert!(names.contains(&preview_name(&object, &names, 5)));
        assert_eq!(
            preview_name(&object, &names, 5),
            preview_name(&object, &names, 5)
        );
        object.group = 15;
        object.kind = "Caldari Navy Assembly Plant".into();
        assert_eq!(preview_name(&object, &names, 5), object.kind);
        let color = muted_background(rgb(0xff0000), 1.);
        assert!(color.r < 0.3 && color.r > color.g);
        let transparent = muted_background(rgb(0xff0000), 0.);
        assert!((transparent.r - 0.031).abs() < 0.001);
    }
    #[::core::prelude::v1::test]
    fn sampled_grid_is_small_stable_and_uses_actual_filtered_types() {
        let a = sampled_objects(&[30], &[], &[], false, 5, 123, None);
        let b = sampled_objects(&[30], &[], &[], false, 5, 123, None);
        assert_eq!(a.len(), 5);
        assert_eq!(
            a.iter().map(|o| &o.kind).collect::<Vec<_>>(),
            b.iter().map(|o| &o.kind).collect::<Vec<_>>()
        );
        assert!(
            a.iter()
                .all(|o| o.group == 30 && type_catalog()[&30].contains(&o.kind))
        );
        assert!(type_catalog()[&30].contains(&"Avatar".to_owned()));
        let colors: std::collections::HashSet<_> = a
            .iter()
            .map(|o| {
                let c = background_color(None, o.states[0]).unwrap();
                (
                    (c.r * 255.).round() as u32,
                    (c.g * 255.).round() as u32,
                    (c.b * 255.).round() as u32,
                )
            })
            .collect();
        assert_eq!(colors.len(), 5);
        assert!(sampled_objects(&[], &[], &[], false, 5, 1, None).is_empty());
        let hidden: Vec<_> = overview::STATES.iter().map(|(id, _)| *id).collect();
        assert!(sampled_objects(&[30], &hidden, &[], false, 5, 1, None).is_empty());
    }
    #[::core::prelude::v1::test]
    fn synthetic_grid_covers_categories_and_state_filter_precedence() {
        for (_, ids) in overview::categories() {
            for id in ids {
                assert!(grid().iter().any(|o| o.group == id));
            }
        }
        let object = GridObject {
            group: 27,
            kind: "Raven".into(),
            states: vec![11, 16],
            variant: 0,
        };
        assert!(!matches_preset(&object, &[27], &[16], &[]));
        assert!(matches_preset(&object, &[27], &[16], &[11]));
        assert!(!matches_preset(&object, &[25], &[], &[11]));
        assert_eq!(grid_cell(&object, "TYPE"), "Raven");
    }
}
