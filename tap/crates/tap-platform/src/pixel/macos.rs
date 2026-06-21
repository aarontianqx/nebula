//! macOS implementation of pixel color detection.
//!
//! Captures a 1x1 region at the requested point with
//! `CGWindowListCreateImage` and reads the top-left pixel. The coordinates are
//! interpreted in the global *point* space (the same space used for injection
//! on macOS); on Retina displays the captured image is denser than one device
//! pixel, and we sample its top-left pixel.
//!
//! Note: screen capture requires the Screen Recording permission. Without it
//! the returned image is empty/blank and this function yields `None` or a
//! black pixel.

use super::Color;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::window::{
    create_image, kCGNullWindowID, kCGWindowImageDefault, kCGWindowListOptionOnScreenOnly,
};

/// Get the color of a pixel at the given screen coordinates (points).
pub fn get_pixel_color(x: i32, y: i32) -> Option<Color> {
    let rect = CGRect::new(&CGPoint::new(x as f64, y as f64), &CGSize::new(1.0, 1.0));
    let image = create_image(
        rect,
        kCGWindowListOptionOnScreenOnly,
        kCGNullWindowID,
        kCGWindowImageDefault,
    )?;

    if image.width() == 0 || image.height() == 0 || image.bits_per_pixel() < 32 {
        return None;
    }

    let data = image.data();
    let bytes = data.bytes();
    if bytes.len() < 4 {
        return None;
    }

    // CGWindowListCreateImage yields 32-bit pixels in little-endian BGRA order,
    // so the first four bytes of the top-left pixel are B, G, R, A.
    Some(Color::new(bytes[2], bytes[1], bytes[0]))
}
