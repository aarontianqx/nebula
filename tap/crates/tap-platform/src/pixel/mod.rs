//! Pixel color detection for condition evaluation.
//!
//! Provides functionality for:
//! - Reading pixel color at screen coordinates
//! - Color comparison with tolerance
//!
//! Platform implementations:
//! - Windows: Uses GDI API (`windows.rs`)
//! - macOS: Stub implementation (`macos.rs`, TODO)

use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

/// RGB color value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Create color from hex string (e.g., "#FF0000" or "FF0000").
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }

        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

        Some(Self { r, g, b })
    }

    /// Convert to hex string (e.g., "#FF0000").
    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Calculate the difference between two colors.
    /// Returns the sum of absolute differences for each channel.
    pub fn difference(&self, other: &Color) -> u32 {
        let dr = (self.r as i32 - other.r as i32).unsigned_abs();
        let dg = (self.g as i32 - other.g as i32).unsigned_abs();
        let db = (self.b as i32 - other.b as i32).unsigned_abs();
        dr + dg + db
    }

    /// Check if this color matches another within a tolerance.
    /// Tolerance is the maximum allowed sum of channel differences.
    pub fn matches(&self, other: &Color, tolerance: u8) -> bool {
        self.difference(other) <= tolerance as u32
    }
}

impl Default for Color {
    fn default() -> Self {
        Self { r: 0, g: 0, b: 0 }
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Get the color of a pixel at the given screen coordinates.
pub fn get_pixel_color(x: i32, y: i32) -> Option<Color> {
    #[cfg(windows)]
    {
        windows::get_pixel_color(x, y)
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_pixel_color(x, y)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (x, y);
        None
    }
}

/// Check if the pixel at the given coordinates matches the expected color.
pub fn pixel_matches(x: i32, y: i32, expected: &Color, tolerance: u8) -> bool {
    get_pixel_color(x, y)
        .map(|c| c.matches(expected, tolerance))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_hex() {
        let color = Color::from_hex("#FF0000").unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 0);

        let color = Color::from_hex("00FF00").unwrap();
        assert_eq!(color.r, 0);
        assert_eq!(color.g, 255);
        assert_eq!(color.b, 0);
    }

    #[test]
    fn test_color_to_hex() {
        let color = Color::new(255, 128, 0);
        assert_eq!(color.to_hex(), "#FF8000");
    }

    #[test]
    fn test_color_difference() {
        let c1 = Color::new(100, 100, 100);
        let c2 = Color::new(110, 90, 105);
        assert_eq!(c1.difference(&c2), 25); // 10 + 10 + 5
    }

    #[test]
    fn test_color_matches() {
        let c1 = Color::new(100, 100, 100);
        let c2 = Color::new(105, 100, 100);
        assert!(c1.matches(&c2, 10));
        assert!(!c1.matches(&c2, 4));
    }
}

