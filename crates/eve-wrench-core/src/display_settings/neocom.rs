//! Neocom shortcuts use saved KeyVal objects, including nested groups. Retain
//! their complete state when removing/restoring or adding a saved definition.
use super::{SettingsDocument, text};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

// Standard shortcut IDs observed in local EVE DATs. These are application
// commands, not character/item identities. EVE resolves their default icons.
pub const SHORTCUTS: &[(&str, &str)] = &[
    ("agency", "The Agency"),
    ("assets", "Personal assets"),
    ("contracts", "Contracts"),
    ("fitting", "Fitting"),
    ("fleet", "Fleet"),
    ("help", "Help"),
    ("job_board", "Opportunities"),
    ("logoff", "Log off"),
    ("mail", "Mail"),
    ("map_beta", "Map"),
    ("market", "Market"),
    ("multibuy", "Multibuy"),
    ("ProjectDiscovery", "Project Discovery"),
    ("quitGame", "Quit game"),
    ("shipTree", "Ship tree"),
    ("wallet", "Wallet"),
];

pub fn id(button: &Value) -> Option<&str> {
    button
        .get("instance")?
        .get("state")?
        .get("bytes:id")
        .and_then(text)
}

pub fn label(button: &Value) -> String {
    if let Some(label) = button
        .get("instance")
        .and_then(|v| v.get("state"))
        .and_then(|v| v.get("bytes:label"))
        .and_then(text)
        .filter(|s| !s.starts_with("UI/"))
    {
        return label.to_owned();
    }
    let id = id(button).unwrap_or("Unknown button");
    SHORTCUTS
        .iter()
        .find(|(key, _)| *key == id)
        .map_or(id, |(_, label)| label)
        .to_owned()
}

fn visit(button: &Value, f: &mut impl FnMut(&Value)) {
    f(button);
    if let Some(children) = button
        .get("instance")
        .and_then(|v| v.get("state"))
        .and_then(|v| v.get("bytes:children"))
        .and_then(Value::as_array)
    {
        for child in children {
            visit(child, f);
        }
    }
}

pub fn available(doc: &SettingsDocument, removed: &[Value]) -> Vec<(String, String, Value)> {
    let mut catalog: BTreeMap<String, Value> = SHORTCUTS
        .iter()
        .map(|(id, _)| {
            (
                (*id).to_owned(),
                json!({"instance": {"class": "utillib.KeyVal", "state": {
                    "bytes:id": format!("bytes:{id}"), "bytes:children": null, "bytes:btnType": 1
                }}}),
            )
        })
        .collect();
    for (id, path) in [
        ("job_board", "res:/ui/Texture/WindowIcons/opportunities.png"),
        ("agency", "res:/UI/Texture/WindowIcons/theAgency.png"),
        ("assets", "res:/ui/Texture/WindowIcons/assets.png"),
        ("wallet", "res:/ui/Texture/WindowIcons/wallet.png"),
        ("market", "res:/ui/Texture/WindowIcons/market.png"),
        ("multibuy", "res:/UI/Texture/classes/MultiSell/multiBuy.png"),
        ("contracts", "res:/ui/Texture/WindowIcons/contracts.png"),
        ("shipTree", "res:/ui/texture/windowIcons/ISIS.png"),
        ("fitting", "res:/ui/Texture/WindowIcons/fitting.png"),
        ("fleet", "res:/ui/Texture/WindowIcons/fleet.png"),
        ("logoff", "res:/ui/texture/WindowIcons/logOut.png"),
        ("quitGame", "res:/ui/texture/WindowIcons/quitGame.png"),
        ("mail", "res:/ui/Texture/WindowIcons/evemail.png"),
        ("map_beta", "res:/UI/Texture/windowIcons/map.png"),
        (
            "ProjectDiscovery",
            "res:/ui/texture/WindowIcons/projectdiscovery.png",
        ),
        ("help", "res:/ui/Texture/WindowIcons/help.png"),
    ] {
        catalog.get_mut(id).unwrap()["instance"]["state"]["bytes:iconPath"] =
            json!(format!("bytes:{path}"));
    }
    // Saved definitions take precedence over a generic command and keep custom
    // icons, group contents and any future client fields exactly as saved.
    for key in ["neocomButtonRawDataOriginal", "neocomButtonRawData"] {
        if let Some(buttons) = doc.value("ui", key).and_then(Value::as_array) {
            for button in buttons {
                visit(button, &mut |button| {
                    if let Some(id) = id(button) {
                        catalog.insert(id.to_owned(), button.clone());
                    }
                });
            }
        }
    }
    for button in removed {
        visit(button, &mut |button| {
            if let Some(id) = id(button) {
                catalog.insert(id.to_owned(), button.clone());
            }
        });
    }
    let mut pinned = HashSet::new();
    if let Some(buttons) = doc
        .value("ui", "neocomButtonRawData")
        .and_then(Value::as_array)
    {
        for button in buttons {
            visit(button, &mut |b| {
                if let Some(id) = id(b) {
                    pinned.insert(id.to_owned());
                }
            });
        }
    }
    let mut result: Vec<_> = catalog
        .into_iter()
        .filter_map(|(id, button)| {
            let mut duplicate = false;
            visit(&button, &mut |b| {
                duplicate |= self::id(b).is_some_and(|id| pinned.contains(id));
            });
            (!duplicate).then(|| (id, label(&button), button))
        })
        .collect();
    result.sort_by_cached_key(|(_, label, _)| label.to_lowercase());
    result
}

pub fn add(doc: &mut SettingsDocument, shortcut: &str, removed: &[Value]) -> Result<(), String> {
    let button = available(doc, removed)
        .into_iter()
        .find(|(id, _, _)| id == shortcut)
        .map(|(_, _, button)| button)
        .ok_or("Button is already pinned or has no saved definition")?;
    let mut buttons = doc
        .value("ui", "neocomButtonRawData")
        .cloned()
        .unwrap_or(json!([]));
    buttons
        .as_array_mut()
        .ok_or("Unsupported Neocom button list")?
        .push(button);
    doc.set("ui", "neocomButtonRawData", buttons)
}

pub fn remove(doc: &mut SettingsDocument, index: usize) -> Result<Value, String> {
    let mut buttons = doc
        .value("ui", "neocomButtonRawData")
        .cloned()
        .ok_or("No Neocom buttons")?;
    let list = buttons
        .as_array_mut()
        .ok_or("Unsupported Neocom button list")?;
    if index >= list.len() {
        return Err("Neocom button no longer exists".into());
    }
    let removed = list.remove(index);
    doc.set("ui", "neocomButtonRawData", buttons)?;
    Ok(removed)
}
