//! Saved `openWindows` entries include historical dialogs and old instances.
//! Reconstruct a useful saved scene from the records the client actually uses:
//! XmppChatService.Login/StoreChannelList, overview tabsByWindowInstanceID,
//! scannerUtil embedding preferences and sessionChangeWindowOpener. This is
//! an offline preview, not the running client's window registry.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::{WindowState, section};
use crate::marshal::{load_settings_json, strip_type_prefix};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowActivity {
    pub chat_windows: Option<HashSet<String>>,
    pub overview_instances: Option<HashSet<usize>>,
    pub probe_embedded: Option<bool>,
    pub directional_embedded: Option<bool>,
    pub collapsed: HashSet<String>,
}

impl WindowActivity {
    pub fn is_likely_visible(&self, window: &WindowState, windows: &[WindowState]) -> bool {
        self.visible_inner(window, windows, None, &mut HashSet::new())
    }

    // Explicit edits to the stored open flag take precedence while previewing
    // a draft, including edits to tabs inside a stack.
    pub fn is_likely_visible_in_draft(
        &self,
        window: &WindowState,
        windows: &[WindowState],
        saved: &[WindowState],
    ) -> bool {
        self.visible_inner(window, windows, Some(saved), &mut HashSet::new())
    }

    fn visible_inner(
        &self,
        window: &WindowState,
        windows: &[WindowState],
        saved: Option<&[WindowState]>,
        visited: &mut HashSet<String>,
    ) -> bool {
        if window.stack.is_none() && window.minimized {
            return false;
        }
        if saved
            .and_then(|saved| saved.iter().find(|w| w.key == window.key))
            .is_some_and(|old| old.open != window.open)
        {
            return window.open;
        }
        let id = window.id();
        if window.is_stack() {
            if !visited.insert(window.key.clone()) {
                return false;
            }
            return windows
                .iter()
                .filter(|w| w.stack.as_deref() == Some(window.key.as_str()))
                .any(|member| self.visible_inner(member, windows, saved, visited));
        }
        if id.starts_with("chatchannel_") {
            return self
                .chat_windows
                .as_ref()
                .map_or(window.open, |channels| channels.contains(id));
        }
        let overview = if id == "overview" {
            Some(0)
        } else {
            id.strip_prefix("overview_")
                .and_then(|n| n.parse::<usize>().ok())
        };
        if let Some(instance) = overview {
            // TacticalSvc.OpenOverview restores the currently assigned overview
            // instances even when historical open flags disagree.
            return self
                .overview_instances
                .as_ref()
                .map_or(window.open, |instances| instances.contains(&instance));
        }
        if !window.open {
            return false;
        }
        if id == "probeScannerWindow" && self.probe_embedded == Some(true)
            || id == "directionalScannerWindow" && self.directional_embedded == Some(true)
        {
            return false;
        }
        // Stable, ordinary window IDs used by the client's restoration and
        // flight UI. Modal dialogs and generated invitation/contact instance
        // IDs are deliberately left for the All saved windows view.
        matches!(
            id,
            "mail"
                | "walletWindow"
                | "assets"
                | "XmppChatChannels"
                | "journal"
                | "logger"
                | "charactersheet"
                | "addressbook"
                | "locations"
                | "market"
                | "notepad"
                | "standaloneBookmarkWnd"
                | "droneview"
                | "selecteditemview"
                | "probeScannerWindow"
                | "directionalScannerWindow"
                | "MoonScanner"
                | "fleetwindow"
                | "watchlistpanel"
                | "fittingWnd"
                | "lobbyWnd"
                | "InventorySpace"
                | "InventoryStation"
                | "InventoryStructure"
                | "StationItems"
                | "StationShips"
                | "ActiveShipCargo"
                | "StationCorpDeliveries"
                | "CapsuleerDeliveries"
                | "StationCorpHangars"
                | "solar_system_map_panel"
        ) || id.starts_with("StationCorpHangar_")
    }
}

fn setting<'a>(root: &'a Value, section: &str, key: &str) -> Option<&'a Value> {
    root.get(section)?.get(key)?.get("tuple")?.get(1)
}

fn embedded(root: &Value, key: &str) -> Option<bool> {
    match setting(root, "bytes:ui", key) {
        None => Some(true), // scannerUtil's client default is embedded.
        Some(value) => value.as_bool(),
    }
}

pub(super) fn read(root: &Value, profile: Option<&Path>) -> WindowActivity {
    let chat_windows = setting(root, "bytes:ui", "bytes:chatchannels")
        .and_then(Value::as_array)
        .and_then(|channels| {
            channels
                .iter()
                .map(|channel| {
                    let id = channel.get("tuple")?.get(0)?.as_str()?;
                    Some(format!("chatchannel_{}", strip_type_prefix(id)))
                })
                .collect::<Option<HashSet<_>>>()
        });
    let collapsed = root
        .get("bytes:windows")
        .and_then(Value::as_object)
        .and_then(|windows| section(windows, "bytes:collapsedWindows"))
        .into_iter()
        .flatten()
        .filter(|(_, value)| value.as_bool() == Some(true))
        .map(|(key, _)| key.clone())
        .collect();
    WindowActivity {
        chat_windows,
        overview_instances: profile.and_then(shared_overviews),
        probe_embedded: embedded(root, "bytes:mapProbePanelEmbedded"),
        directional_embedded: embedded(root, "bytes:mapDirectionPanelEmbedded"),
        collapsed,
    }
}

fn overview_instances(root: &Value) -> Option<HashSet<usize>> {
    let windows = setting(root, "bytes:overview", "bytes:tabsByWindowInstanceID")?.as_array()?;
    let mut instances = HashSet::new();
    for (index, tabs) in windows.iter().enumerate() {
        if !tabs.as_array()?.is_empty() {
            instances.insert(index);
        }
    }
    Some(instances)
}

// A character file does not identify its account. Only use profile account
// context when every real account agrees; otherwise keep overview visibility
// conservative instead of choosing an arbitrary account.
fn shared_overviews(profile: &Path) -> Option<HashSet<usize>> {
    let mut agreed = None;
    for entry in fs::read_dir(profile).ok()? {
        let path = entry.ok()?.path();
        let Some(id) = path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix("core_user_"))
            .and_then(|s| s.strip_suffix(".dat"))
        else {
            continue;
        };
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let (root, _) = load_settings_json(&path).ok()?;
        let instances = overview_instances(&root)?;
        if agreed
            .as_ref()
            .is_some_and(|previous| previous != &instances)
        {
            return None;
        }
        agreed = Some(instances);
    }
    agreed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marshal::encode_settings_json;
    use serde_json::json;

    fn window(id: &str) -> WindowState {
        WindowState {
            key: format!("bytes:{id}"),
            rect: None,
            open: true,
            locked: false,
            minimized: false,
            compact: None,
            light_background: None,
            stack: None,
        }
    }

    #[test]
    fn saved_chat_membership_filters_historical_tabs_and_empty_stacks() {
        let root = json!({"bytes:ui": {"bytes:chatchannels": {"tuple": ["long:1", [{"tuple": ["bytes:local", "bytes:local_1", "utf8:Local"]}]]}}});
        let activity = read(&root, None);
        let mut local = window("chatchannel_local");
        local.open = false; // Login restores the saved channel list independently.
        local.stack = Some("bytes:1".into());
        let mut historical = window("chatchannel_player_history");
        historical.stack = Some("bytes:2".into());
        let mut windows = vec![local, historical, window("1"), window("2")];
        assert!(activity.is_likely_visible(&windows[0], &windows));
        assert!(!activity.is_likely_visible(&windows[1], &windows));
        assert!(activity.is_likely_visible(&windows[2], &windows));
        assert!(!activity.is_likely_visible(&windows[3], &windows));
        let saved = windows.clone();
        windows[0].open = true;
        assert!(activity.is_likely_visible_in_draft(&windows[2], &windows, &saved));
        windows[0].open = false;
        windows[2].minimized = true;
        assert!(!activity.is_likely_visible(&windows[2], &windows));
    }

    #[test]
    fn assigned_overviews_override_stale_flags_and_scanner_embedding_is_respected() {
        let mut activity = read(
            &json!({"bytes:ui": {
                "bytes:mapDirectionPanelEmbedded": {"tuple": ["long:1", false]},
                "bytes:mapProbePanelEmbedded": {"tuple": ["long:1", true]}
            }}),
            None,
        );
        activity.overview_instances = Some(HashSet::from([0, 2]));
        let mut active = window("overview_2");
        active.open = false;
        assert!(activity.is_likely_visible(&active, &[]));
        assert!(!activity.is_likely_visible(&window("overview_18"), &[]));
        assert!(activity.is_likely_visible(&window("directionalScannerWindow"), &[]));
        assert!(!activity.is_likely_visible(&window("probeScannerWindow"), &[]));
        active.minimized = true;
        assert!(!activity.is_likely_visible(&active, &[]));
        assert!(!activity.is_likely_visible(&window("contactmanagement_123"), &[]));
        assert!(!activity.is_likely_visible(&window("message"), &[]));
        assert!(activity.is_likely_visible(&window("market"), &[]));
        let saved = vec![window("overview")];
        let mut closed = saved[0].clone();
        closed.open = false;
        assert!(!activity.is_likely_visible_in_draft(&closed, &[], &saved));
    }

    #[test]
    fn missing_or_unsupported_membership_does_not_invent_empty_channel_lists() {
        for root in [
            json!({}),
            json!({"bytes:ui": {"bytes:chatchannels": {"tuple": ["long:1", ["utf8:unsupported"]]}}}),
        ] {
            let activity = read(&root, None);
            assert!(activity.chat_windows.is_none());
            assert!(activity.is_likely_visible(&window("chatchannel_local"), &[]));
        }
        let activity = read(
            &json!({"bytes:windows": {"bytes:collapsedWindows": {"tuple": ["long:1", {"bytes:market": true}]}}}),
            None,
        );
        assert!(activity.collapsed.contains("bytes:market"));
        assert!(
            activity.is_likely_visible(&window("market"), &[]),
            "collapsed windows are visible headers, not closed windows"
        );
    }

    #[test]
    fn overview_context_requires_all_accounts_to_agree_and_follows_file_updates() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "wrench-overview-context-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let write = |name: &str, rows: Value| {
            fs::write(
                root.join(name),
                encode_settings_json(
                    &json!({"bytes:overview": {
                        "bytes:tabsByWindowInstanceID": {"tuple": ["long:1", rows]}
                    }}),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
        };
        fs::write(root.join("core_user__.dat"), b"not an account").unwrap();
        write("core_user_1.dat", json!([[0], [], [1]]));
        write("core_user_2.dat", json!([[2], [], [3]]));
        assert_eq!(shared_overviews(&root), Some(HashSet::from([0, 2])));
        write("core_user_2.dat", json!([[0], [1]]));
        assert_eq!(
            shared_overviews(&root),
            None,
            "different account layouts must not be assigned arbitrarily to a character"
        );
        let resolved = root.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        fs::remove_dir_all(resolved).unwrap();
    }
}
