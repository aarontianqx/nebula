//! macOS implementation of pixel color detection.
//!
//! TODO: Implement using CGDisplayCreateImageForRect or similar.

use super::Color;

/// Get the color of a pixel at the given screen coordinates.
pub fn get_pixel_color(x: i32, y: i32) -> Option<Color> {
    let _ = (x, y);
    // TODO: Implement using Core Graphics
    None
}

