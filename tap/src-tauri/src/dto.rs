//! Shapes sent to the frontend that no lower layer has a reason to own.

use tap_core::{Color, Point};
use tap_platform::WindowInfo;

#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowInfoResponse {
    pub handle: usize,
    pub title: String,
    pub process_name: String,
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl From<WindowInfo> for WindowInfoResponse {
    fn from(w: WindowInfo) -> Self {
        Self {
            handle: w.handle,
            title: w.title,
            process_name: w.process_name,
            pid: w.pid,
            x: w.rect.x,
            y: w.rect.y,
            width: w.rect.width,
            height: w.rect.height,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColorResponse {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub hex: String,
}

impl From<Color> for ColorResponse {
    fn from(c: Color) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
            hex: c.to_hex(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MousePositionUpdate {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PositionPickedEvent {
    pub x: i32,
    pub y: i32,
    /// The color under the click, only when the picker was opened to read one. `None` there means
    /// the color could not be read (for example, missing Screen Recording permission).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorResponse>,
    /// Where the target window's top-left corner is on the screen, when the document measures
    /// positions from that window. The editor subtracts it from `x` / `y` for the field that asked;
    /// Simple mode keeps the screen position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_origin: Option<Point>,
    /// The document measures positions from its target window but that window was not found, so
    /// no window-relative position can be offered.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub window_missing: bool,
}
