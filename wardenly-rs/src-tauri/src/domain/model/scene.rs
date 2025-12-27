use image::DynamicImage;
use serde::{Deserialize, Serialize};

/// Scene represents a recognizable game state defined by color points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    /// Unique identifier for this scene
    pub name: String,

    /// Category groups related scenes (e.g., "city", "battle", "loading")
    #[serde(default)]
    pub category: String,

    /// Color checkpoints used to identify this scene
    pub points: Vec<ColorPoint>,

    /// Predefined actions available in this scene
    #[serde(default)]
    pub actions: std::collections::HashMap<String, SceneAction>,
}

/// ColorPoint represents a coordinate with an expected color for scene matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPoint {
    pub x: i32,
    pub y: i32,
    #[serde(flatten)]
    pub color: ColorValue,
}

/// Color value with optional tolerance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorValue {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default)]
    pub a: Option<u8>,
}

/// Predefined action within a scene
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneAction {
    #[serde(rename = "type")]
    pub action_type: String,
    pub point: Option<ActionPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPoint {
    pub x: f64,
    pub y: f64,
}

/// Scene matcher with configurable threshold
pub struct SceneMatcher {
    /// Maximum average color difference allowed for a match
    pub threshold: f64,
}

impl Default for SceneMatcher {
    fn default() -> Self {
        Self { threshold: 5.0 }
    }
}

impl SceneMatcher {
    pub fn new(threshold: f64) -> Self {
        Self {
            threshold: if threshold <= 0.0 { 5.0 } else { threshold },
        }
    }

    /// Check if the given image matches the scene
    pub fn matches(&self, scene: &Scene, image: &DynamicImage) -> bool {
        if scene.points.is_empty() {
            return false;
        }

        let rgb = image.to_rgb8();
        let (width, height) = (rgb.width(), rgb.height());

        let mut total_diff = 0.0;
        let mut valid_points = 0;

        for point in &scene.points {
            if point.x < 0 || point.y < 0 {
                continue;
            }

            let x = point.x as u32;
            let y = point.y as u32;

            if x >= width || y >= height {
                continue;
            }

            let pixel = rgb.get_pixel(x, y);
            let diff = color_diff(
                pixel[0],
                pixel[1],
                pixel[2],
                point.color.r,
                point.color.g,
                point.color.b,
            );
            total_diff += diff;
            valid_points += 1;
        }

        if valid_points == 0 {
            return false;
        }

        let avg_diff = total_diff / valid_points as f64;
        avg_diff <= self.threshold
    }

    /// Match with detailed results for debugging
    pub fn match_with_details(&self, scene: &Scene, image: &DynamicImage) -> MatchResult {
        let rgb = image.to_rgb8();
        let (width, height) = (rgb.width(), rgb.height());

        let mut point_diffs = Vec::with_capacity(scene.points.len());
        let mut total_diff = 0.0;
        let mut valid_points = 0;

        for point in &scene.points {
            if point.x < 0 || point.y < 0 {
                point_diffs.push(f64::MAX);
                continue;
            }

            let x = point.x as u32;
            let y = point.y as u32;

            if x >= width || y >= height {
                point_diffs.push(f64::MAX);
                continue;
            }

            let pixel = rgb.get_pixel(x, y);
            let diff = color_diff(
                pixel[0],
                pixel[1],
                pixel[2],
                point.color.r,
                point.color.g,
                point.color.b,
            );
            point_diffs.push(diff);
            total_diff += diff;
            valid_points += 1;
        }

        let avg_diff = if valid_points > 0 {
            total_diff / valid_points as f64
        } else {
            f64::MAX
        };

        MatchResult {
            scene_name: scene.name.clone(),
            matched: avg_diff <= self.threshold,
            avg_diff,
            point_diffs,
        }
    }
}

/// Result of a scene match attempt
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub scene_name: String,
    pub matched: bool,
    pub avg_diff: f64,
    pub point_diffs: Vec<f64>,
}

/// Calculate average color difference between two colors
fn color_diff(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> f64 {
    let dr = (r1 as i32 - r2 as i32).abs() as f64;
    let dg = (g1 as i32 - g2 as i32).abs() as f64;
    let db = (b1 as i32 - b2 as i32).abs() as f64;
    (dr + dg + db) / 3.0
}

impl Scene {
    /// Convenience method to check if image matches this scene using default threshold
    pub fn matches(&self, image: &DynamicImage) -> bool {
        SceneMatcher::default().matches(self, image)
    }
}

