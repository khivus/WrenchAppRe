use super::{SettingsDocument, boolean, integer, text};
use serde_json::{Value, json};
use std::collections::HashSet;

pub const COLUMNS: &[(&str, &str, i64)] = &[
    ("ICON", "Icon", 28),
    ("DISTANCE", "Distance", 80),
    ("NAME", "Name", 160),
    ("TYPE", "Type", 120),
    ("TAG", "Tag", 40),
    ("CORPORATION", "Corporation", 90),
    ("ALLIANCE", "Alliance", 75),
    ("FACTION", "Faction", 90),
    ("MILITIA", "Militia", 80),
    ("SIZE", "Size", 60),
    ("VELOCITY", "Velocity", 80),
    ("RADIALVELOCITY", "Radial velocity", 90),
    ("TRANSVERSALVELOCITY", "Transversal velocity", 100),
    ("ANGULARVELOCITY", "Angular velocity", 100),
];
pub const DEFAULT_COLUMNS: &[&str] = &["ICON", "DISTANCE", "NAME", "TYPE", "VELOCITY"];
pub const STATES: &[(i64, &str)] = &[
    (11, "Same fleet"),
    (12, "Same player corporation"),
    (14, "Same alliance"),
    (15, "Excellent standing"),
    (16, "Good standing"),
    (13, "At war"),
    (44, "Enemy militia"),
    (52, "Limited engagement"),
    (51, "Criminal"),
    (50, "Suspect"),
    (53, "Kill right available"),
    (9, "Outlaw"),
    (19, "Terrible standing"),
    (18, "Bad standing"),
    (17, "Neutral standing"),
    (49, "Allies in war"),
    (45, "Same militia"),
    (21, "Agent"),
    (48, "No standing"),
    (10, "Dangerous"),
    (66, "Same NPC corporation"),
    (20, "Wanted"),
    (68, "Disapproval"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct OverviewTab {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub preset: String,
    pub bracket: String,
    pub color: Option<[f32; 3]>,
    pub columns: Vec<String>,
}

pub fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(text)
        .map(str::to_owned)
        .collect()
}
pub fn ids(value: Option<&Value>) -> Vec<i64> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(integer)
        .collect()
}
pub fn tabs(doc: &SettingsDocument) -> Vec<OverviewTab> {
    let mut result: Vec<_> = doc
        .value("overview", "tabsettings_new")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(key, value)| {
            let id = key.split_once(':')?.1.parse::<i64>().ok()?;
            let shown = value
                .get("bytes:tabColumns")
                .map(|v| strings(Some(v)))
                .unwrap_or_else(|| DEFAULT_COLUMNS.iter().map(|s| s.to_string()).collect());
            let mut columns = value
                .get("bytes:tabColumnOrder")
                .map(|v| strings(Some(v)))
                .unwrap_or_else(|| COLUMNS.iter().map(|c| c.0.to_owned()).collect());
            for column in &shown {
                if !columns.contains(column) {
                    columns.push(column.clone());
                }
            }
            let color = value
                .get("bytes:color")
                .and_then(|v| v.get("tuple"))
                .and_then(Value::as_array)
                .and_then(|a| {
                    if a.len() >= 3 {
                        Some([
                            a[0].as_f64()? as f32,
                            a[1].as_f64()? as f32,
                            a[2].as_f64()? as f32,
                        ])
                    } else {
                        None
                    }
                });
            Some(OverviewTab {
                id,
                key: key.clone(),
                name: value
                    .get("bytes:name")
                    .and_then(text)
                    .unwrap_or("Unnamed tab")
                    .into(),
                preset: value
                    .get("bytes:overview")
                    .and_then(text)
                    .unwrap_or("Default")
                    .into(),
                bracket: value
                    .get("bytes:bracket")
                    .and_then(text)
                    .unwrap_or("Default")
                    .into(),
                color,
                columns,
            })
        })
        .collect();
    result.sort_by_key(|t| t.id);
    result
}

pub fn set_tab(
    doc: &mut SettingsDocument,
    key: &str,
    field: &str,
    value: Value,
) -> Result<(), String> {
    let mut tabs = doc
        .value("overview", "tabsettings_new")
        .cloned()
        .ok_or("No editable overview tabs in this account")?;
    let tab = tabs
        .get_mut(key)
        .and_then(Value::as_object_mut)
        .ok_or("Overview tab no longer exists")?;
    tab.insert(format!("bytes:{field}"), value);
    doc.set("overview", "tabsettings_new", tabs)
}

pub fn width_key(tab: i64) -> String {
    format!("json:{{\"tuple\":[\"bytes:overviewScroll2\",{tab}]}}")
}
pub fn column_width(doc: &SettingsDocument, tab: i64, column: &str) -> i64 {
    doc.value("ui", "SortHeadersSizes")
        .and_then(|v| v.get(width_key(tab)))
        .and_then(|v| v.get(format!("bytes:{column}")))
        .and_then(integer)
        .unwrap_or_else(|| COLUMNS.iter().find(|c| c.0 == column).map_or(100, |c| c.2))
}
pub fn set_width(
    doc: &mut SettingsDocument,
    tab: i64,
    column: &str,
    width: i64,
) -> Result<(), String> {
    if !COLUMNS.iter().any(|c| c.0 == column) {
        return Err("Unsupported overview column".into());
    }
    let mut all = doc
        .value("ui", "SortHeadersSizes")
        .cloned()
        .unwrap_or(json!({}));
    let map = all.as_object_mut().ok_or("Unsupported column sizes")?;
    let row = map
        .entry(width_key(tab))
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("Unsupported tab column sizes")?;
    row.insert(format!("bytes:{column}"), json!(width));
    doc.set("ui", "SortHeadersSizes", all)
}

pub fn color_key(style: &str, state: i64) -> String {
    format!("json:{{\"tuple\":[\"bytes:{style}\",{state}]}}")
}

pub(super) fn validate_widths(value: &Value) -> bool {
    value.as_object().is_some_and(|rows| {
        rows.iter().all(|(key, v)| {
            if !key.contains("overviewScroll2") {
                return true;
            }
            v.as_object().is_some_and(|widths| {
                widths
                    .values()
                    .all(|w| integer(w).is_some_and(|n| (12..=2000).contains(&n)))
            })
        })
    })
}
fn string_list(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|a| a.len() <= 100 && a.iter().all(|v| text(v).is_some()))
}
fn id_list(value: &Value) -> bool {
    value.as_array().is_some_and(|a| {
        a.len() <= 10000
            && a.iter().all(|v| integer(v).is_some_and(|n| n >= 0))
            && ids(Some(value)).into_iter().collect::<HashSet<_>>().len() == a.len()
    })
}
fn color(value: &Value, len: usize) -> bool {
    value
        .get("tuple")
        .and_then(Value::as_array)
        .is_some_and(|a| {
            a.len() == len
                && a.iter().all(|v| {
                    v.as_f64()
                        .is_some_and(|n| n.is_finite() && (0.0..=1.0).contains(&n))
                })
        })
}
pub(super) fn validate_setting(key: &str, value: &Value) -> bool {
    match key {
        "tabsettings_new" => value.as_object().is_some_and(|tabs| {
            !tabs.is_empty()
                && tabs.len() <= 100
                && tabs.iter().all(|(id, tab)| {
                    id.split_once(':')
                        .and_then(|(_, s)| s.parse::<i64>().ok())
                        .is_some_and(|n| n >= 0)
                        && tab.as_object().is_some_and(|fields| {
                            fields
                                .iter()
                                .all(|(k, v)| match text(&json!(k)).unwrap_or(k) {
                                    "name" => text(v).is_some_and(|s| {
                                        !s.trim().is_empty() && s.chars().count() <= 15
                                    }),
                                    "overview" | "bracket" => {
                                        text(v).is_some() || v.is_null() || v.get("tuple").is_some()
                                    }
                                    "tabColumns" | "tabColumnOrder" => string_list(v),
                                    "color" => v.is_null() || color(v, 3),
                                    "showAll" | "showNone" | "showSpecials" => boolean(v).is_some(),
                                    _ => true,
                                })
                        })
                })
        }),
        "overviewProfilePresets" => value.as_object().is_some_and(|presets| {
            presets.values().all(|p| {
                p.as_object().is_some_and(|fields| {
                    fields.iter().all(|(key, v)| match key.as_str() {
                        "bytes:groups" | "bytes:filteredStates" | "bytes:alwaysShownStates" => {
                            id_list(v)
                        }
                        _ => true,
                    })
                })
            })
        }),
        "stateColors" => value
            .as_object()
            .is_some_and(|m| m.values().all(|v| color(v, 4))),
        "stateBlinks" => value
            .as_object()
            .is_some_and(|m| m.values().all(|v| boolean(v).is_some())),
        "flagStates2" | "backgroundStates2" | "flagOrder2" | "backgroundOrder2" => id_list(value),
        _ => false,
    }
}
