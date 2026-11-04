//! The XMPP window name omits the `chatchannel_` window-ID prefix. These
//! dividers belong to settings.user.ui, not the character geometry file.
use super::{SettingsDocument, boolean, integer};
use serde_json::{Value, json};

pub fn channel(window_id: &str) -> Option<&str> {
    let name = window_id.strip_prefix("chatchannel_")?;
    valid_name(name).then_some(name)
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 160
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
pub fn setting_key(key: &str) -> Option<(&str, &str)> {
    if let Some(name) = key.strip_prefix("chatinputsize_") {
        return valid_name(name).then_some((name, "input"));
    }
    for (suffix, field) in [("_userlistwidth", "members"), ("_usermode", "visible")] {
        if let Some(name) = key.strip_suffix(suffix) {
            return valid_name(name).then_some((name, field));
        }
    }
    None
}
pub(super) fn validate(key: &str, value: &Value) -> bool {
    match setting_key(key).map(|(_, field)| field) {
        Some("members") => integer(value).is_some_and(|n| (50..=200).contains(&n)),
        Some("input") => integer(value).is_some_and(|n| (30..=96).contains(&n)),
        Some("visible") => boolean(value).is_some(),
        _ => false,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatLayout {
    pub member_width: i64,
    pub input_height: i64,
    pub show_members: bool,
}
impl ChatLayout {
    pub fn read(doc: Option<&SettingsDocument>, window_id: &str) -> Self {
        let name = channel(window_id).unwrap_or("local");
        let value = |key: &str| doc.and_then(|d| d.value("ui", key));
        Self {
            member_width: value(&format!("{name}_userlistwidth"))
                .and_then(integer)
                .unwrap_or(128)
                .clamp(50, 200),
            input_height: value(&format!("chatinputsize_{name}"))
                .and_then(integer)
                .unwrap_or(64)
                .clamp(30, 96),
            show_members: value(&format!("{name}_usermode"))
                .and_then(boolean)
                .unwrap_or(true),
        }
    }
    pub fn set(
        doc: &mut SettingsDocument,
        name: &str,
        field: &str,
        value: i64,
    ) -> Result<(), String> {
        if !valid_name(name) {
            return Err("Unsupported chat name".into());
        }
        let key = match field {
            "members" => format!("{name}_userlistwidth"),
            "input" => format!("chatinputsize_{name}"),
            "visible" => format!("{name}_usermode"),
            _ => return Err("Unsupported chat divider".into()),
        };
        let value = if field == "visible" {
            json!(value != 0)
        } else {
            json!(value)
        };
        doc.set("ui", &key, value)
    }
}
