use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Theme configuration loaded from themes.yaml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThemeConfig {
    /// Name of the active theme
    #[serde(default = "default_active_theme")]
    pub active_theme: String,

    /// Map of theme name to theme definition
    #[serde(default)]
    pub themes: HashMap<String, Theme>,
}

fn default_active_theme() -> String {
    "ocean-dark".to_string()
}

/// A single theme definition containing color tokens
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct Theme {
    #[serde(default = "default_bg_app")]
    pub bg_app: String,
    #[serde(default = "default_bg_panel")]
    pub bg_panel: String,
    #[serde(default = "default_bg_surface")]
    pub bg_surface: String,
    #[serde(default = "default_bg_hover")]
    pub bg_hover: String,
    #[serde(default = "default_border")]
    pub border: String,
    #[serde(default = "default_text_primary")]
    pub text_primary: String,
    #[serde(default = "default_text_secondary")]
    pub text_secondary: String,
    #[serde(default = "default_text_muted")]
    pub text_muted: String,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default = "default_accent_hover")]
    pub accent_hover: String,
    #[serde(default = "default_accent_fg")]
    pub accent_fg: String,
    #[serde(default = "default_success")]
    pub success: String,
    #[serde(default = "default_warning")]
    pub warning: String,
    #[serde(default = "default_error")]
    pub error: String,
}

// Default color values (ocean-dark theme)
fn default_bg_app() -> String { "#0f172a".to_string() }
fn default_bg_panel() -> String { "#1e293b".to_string() }
fn default_bg_surface() -> String { "#334155".to_string() }
fn default_bg_hover() -> String { "#3f4f63".to_string() }
fn default_border() -> String { "#475569".to_string() }
fn default_text_primary() -> String { "#f8fafc".to_string() }
fn default_text_secondary() -> String { "#94a3b8".to_string() }
fn default_text_muted() -> String { "#64748b".to_string() }
fn default_accent() -> String { "#3b82f6".to_string() }
fn default_accent_hover() -> String { "#2563eb".to_string() }
fn default_accent_fg() -> String { "#ffffff".to_string() }
fn default_success() -> String { "#22c55e".to_string() }
fn default_warning() -> String { "#f59e0b".to_string() }
fn default_error() -> String { "#ef4444".to_string() }

impl Theme {
    /// Convert theme to a flat map of CSS variable names to values
    pub fn to_css_vars(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();
        vars.insert("--color-bg-app".to_string(), self.bg_app.clone());
        vars.insert("--color-bg-panel".to_string(), self.bg_panel.clone());
        vars.insert("--color-bg-surface".to_string(), self.bg_surface.clone());
        vars.insert("--color-bg-hover".to_string(), self.bg_hover.clone());
        vars.insert("--color-border".to_string(), self.border.clone());
        vars.insert("--color-text-primary".to_string(), self.text_primary.clone());
        vars.insert("--color-text-secondary".to_string(), self.text_secondary.clone());
        vars.insert("--color-text-muted".to_string(), self.text_muted.clone());
        vars.insert("--color-accent".to_string(), self.accent.clone());
        vars.insert("--color-accent-hover".to_string(), self.accent_hover.clone());
        vars.insert("--color-accent-fg".to_string(), self.accent_fg.clone());
        vars.insert("--color-success".to_string(), self.success.clone());
        vars.insert("--color-warning".to_string(), self.warning.clone());
        vars.insert("--color-error".to_string(), self.error.clone());
        vars
    }
}

/// Response sent to frontend with active theme's CSS variables
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeResponse {
    /// Name of the active theme
    pub active_theme: String,
    /// CSS variable name -> value mapping
    pub css_vars: HashMap<String, String>,
    /// List of available theme names
    pub available_themes: Vec<String>,
}

