use super::{SettingsDocument, boolean, integer, text};
use crate::window_layout::{WindowRect, WindowState};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Space,
    Station,
    Citadel,
    CitadelControl,
    Carrier,
    Supercarrier,
}
impl Scene {
    pub const ALL: [Self; 6] = [
        Self::Space,
        Self::Station,
        Self::Citadel,
        Self::CitadelControl,
        Self::Carrier,
        Self::Supercarrier,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::Space => "space",
            Self::Station => "station",
            Self::Citadel => "citadel",
            Self::CitadelControl => "citadelControl",
            Self::Carrier => "carrier",
            Self::Supercarrier => "supercarrier",
        }
    }
    pub fn fighter_count(self) -> usize {
        match self {
            Self::CitadelControl => 3,
            Self::Carrier => 4,
            Self::Supercarrier => 5,
            _ => 0,
        }
    }
    pub fn docked(self) -> bool {
        matches!(self, Self::Station | Self::Citadel)
    }
    pub fn allows(self, window: &WindowState) -> bool {
        let id = window.id();
        let station = id.starts_with("InventoryStation")
            || id.starts_with("StationItems")
            || id.starts_with("StationShips")
            || id.starts_with("StationCorp");
        let citadel = id == "InventoryStructure"
            || id.starts_with("StructureItemHangar")
            || id.starts_with("StructureShipHangar")
            || id.starts_with("StructureCorpHangar");
        let flight = id == "overview"
            || id.starts_with("overview_")
            || matches!(
                id,
                "droneview"
                    | "selecteditemview"
                    | "probeScannerWindow"
                    | "directionalScannerWindow"
                    | "MoonScanner"
                    | "InventorySpace"
            );
        match self {
            Self::Station => !citadel && !flight,
            Self::Citadel => !station && !flight,
            _ => !station && !citadel && !matches!(id, "lobbyWnd" | "repairshop" | "tradeWnd"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TargetLayout {
    pub origin: Option<[f64; 2]>,
    pub horizontal: bool,
}
impl Default for TargetLayout {
    fn default() -> Self {
        Self {
            origin: None,
            horizontal: true,
        }
    }
}
impl TargetLayout {
    pub fn read(doc: &SettingsDocument) -> Self {
        let origin = doc
            .value("ui", "targetOrigin")
            .and_then(|v| v.get("tuple"))
            .and_then(Value::as_array)
            .and_then(|v| {
                if v.len() == 2 {
                    Some([v[0].as_f64()?, v[1].as_f64()?])
                } else {
                    None
                }
            });
        Self {
            origin,
            horizontal: doc
                .value("ui", "alignHorizontally")
                .and_then(boolean)
                .unwrap_or(true),
        }
    }
    pub fn rectangles(
        &self,
        width: i64,
        height: i64,
        count: usize,
        windows: &[WindowState],
    ) -> Vec<WindowRect> {
        let origin = self.origin.unwrap_or_else(|| {
            windows
                .iter()
                .find(|w| w.id() == "selecteditemview")
                .and_then(|w| w.rect)
                .map(|r| [(r.x - 15) as f64 / width as f64, r.y as f64 / height as f64])
                .unwrap_or([0.8, 16.0 / height as f64])
        });
        let left = origin[0] > 0.5;
        let up = origin[1] > 0.5;
        let (tw, th) = if self.horizontal {
            (110, 150)
        } else {
            (160, 80)
        };
        let (x, y) = (
            (origin[0] * width as f64) as i64,
            (origin[1] * height as f64) as i64,
        );
        let primary = if self.horizontal {
            if left { x } else { width - x }
        } else if up {
            y
        } else {
            height - y
        };
        let step = if self.horizontal { tw + 10 } else { th + 10 };
        let per_line = (primary / step).max(1) as usize;
        (0..count.min(12))
            .map(|i| {
                let (major, minor) = ((i % per_line) as i64, (i / per_line) as i64);
                let (dx, dy) = if self.horizontal {
                    (major * (tw + 10), minor * (th + 10))
                } else {
                    (minor * (tw + 10), major * (th + 10))
                };
                WindowRect {
                    x: if left { x - tw - dx } else { x + dx },
                    y: if up { y - th - dy } else { y + dy },
                    width: tw,
                    height: th,
                    screen_width: width,
                    screen_height: height,
                }
            })
            .collect()
    }
}

pub fn fighters_position(doc: &SettingsDocument, width: i64, height: i64) -> (i64, i64) {
    doc.value("ui", "fightersDetachedPosition")
        .and_then(|v| v.get("tuple"))
        .and_then(Value::as_array)
        .and_then(|v| Some((integer(v.first()?)?, integer(v.get(1)?)?)))
        .unwrap_or((width / 2, height / 2 - 60))
}
pub fn neocom_buttons(doc: &SettingsDocument) -> Vec<String> {
    doc.value("ui", "neocomButtonRawData")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| {
            v.get("instance")?
                .get("state")?
                .get("bytes:id")
                .and_then(text)
                .map(str::to_owned)
        })
        .collect()
}
