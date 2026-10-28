use axum::http::{Method, Uri};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashSet},
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub default_provider: String,
    #[serde(default)]
    pub server: Server,
    #[serde(default)]
    pub logging: Logging,
    pub providers: BTreeMap<String, Provider>,
    #[serde(default)]
    pub routes: Vec<Route>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Server {
    pub listen: SocketAddr,
    pub shutdown_grace_seconds: u64,
    pub connect_timeout_seconds: u64,
    pub response_headers_timeout_seconds: u64,
    pub stream_idle_timeout_seconds: u64,
    pub max_in_flight: usize,
    pub max_connections: usize,
    pub max_upload_bytes: usize,
    pub max_rewrite_bytes: usize,
}
impl Default for Server {
    fn default() -> Self {
        Self {
            listen: SocketAddr::from(([127, 0, 0, 1], 8787)),
            shutdown_grace_seconds: 30,
            connect_timeout_seconds: 15,
            response_headers_timeout_seconds: 300,
            stream_idle_timeout_seconds: 180,
            max_in_flight: 256,
            max_connections: 1024,
            max_upload_bytes: 64 * 1024 * 1024,
            max_rewrite_bytes: 2 * 1024 * 1024,
        }
    }
}
impl Server {
    pub fn idle_timeout(&self) -> Duration {
        Duration::from_secs(self.stream_idle_timeout_seconds)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Logging {
    pub directory: Option<PathBuf>,
    pub level: String,
    pub stderr_level: String,
    pub rotation: String,
    pub retention_days: u32,
    pub cleanup_interval_seconds: u64,
    pub error_preview_bytes: usize,
}
impl Default for Logging {
    fn default() -> Self {
        Self {
            directory: None,
            level: "debug".into(),
            stderr_level: "warn".into(),
            rotation: "daily".into(),
            retention_days: 14,
            cleanup_interval_seconds: 3600,
            error_preview_bytes: 4096,
        }
    }
}
impl Logging {
    pub fn restart_settings(&self) -> (Option<&Path>, &str, u32, u64) {
        (
            self.directory.as_deref(),
            &self.rotation,
            self.retention_days,
            self.cleanup_interval_seconds,
        )
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub origin: String,
    #[serde(default)]
    pub base_path: String,
    #[serde(default)]
    pub model_aliases: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub id: String,
    pub method: Option<String>,
    pub path: Option<String>,
    pub path_prefix: Option<String>,
    #[serde(default)]
    pub strip_prefix: bool,
    pub provider: Option<String>,
    pub upstream_path: Option<String>,
    pub adapter: Option<Adapter>,
    pub prompt_mode: Option<PromptMode>,
    #[serde(default = "reject")]
    pub unknown_fields: String,
    #[serde(default = "request_limit")]
    pub max_request_bytes: usize,
    #[serde(default = "response_limit")]
    pub max_json_response_bytes: usize,
    #[serde(default = "event_limit")]
    pub max_sse_event_bytes: usize,
}
fn reject() -> String {
    "reject".into()
}
fn request_limit() -> usize {
    2 * 1024 * 1024
}
fn response_limit() -> usize {
    8 * 1024 * 1024
}
fn event_limit() -> usize {
    1024 * 1024
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Adapter {
    CompletionsToChat,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptMode {
    RawUser,
    FimQwen,
}

#[derive(Clone)]
pub struct Snapshot {
    pub generation: u64,
    pub config: Config,
}
pub struct Plan<'a> {
    pub provider_name: &'a str,
    pub provider: &'a Provider,
    pub route: Option<&'a Route>,
    pub logical_path: String,
    pub upstream_path: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read config {}: {e}", path.display()))?;
        // TOML diagnostics may contain source lines, which could contain credentials.
        let value: Self = toml::from_str(&data).map_err(|e| {
            let location = e
                .span()
                .map(|s| {
                    let prefix = &data.as_bytes()[..s.start.min(data.len())];
                    let line = prefix.iter().filter(|byte| **byte == b'\n').count() + 1;
                    format!(" at line {line}, byte {}", s.start)
                })
                .unwrap_or_default();
            format!("invalid TOML/config schema{location}; check field names and value types")
        })?;
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("unsupported config version".into());
        }
        if !self.providers.contains_key(&self.default_provider) {
            return Err("default_provider must reference a configured provider".into());
        }
        let server = &self.server;
        if server.max_in_flight == 0
            || server.max_in_flight > 65536
            || server.max_connections == 0
            || server.max_connections > 65536
            || server.max_rewrite_bytes == 0
            || server.max_upload_bytes == 0
            || server.connect_timeout_seconds == 0
            || server.response_headers_timeout_seconds == 0
            || server.stream_idle_timeout_seconds == 0
        {
            return Err(
                "server limits and timeouts must be positive; concurrency limits <= 65536".into(),
            );
        }
        if self.logging.rotation != "daily"
            || self.logging.retention_days == 0
            || self.logging.cleanup_interval_seconds == 0
            || self.logging.error_preview_bytes > 65536
        {
            return Err("invalid logging rotation, retention, interval or preview limit".into());
        }
        if self
            .logging
            .directory
            .as_ref()
            .is_some_and(|p| !p.is_absolute())
        {
            return Err("logging.directory must be absolute".into());
        }
        for level in [&self.logging.level, &self.logging.stderr_level] {
            if !["error", "warn", "info", "debug", "trace"].contains(&level.as_str()) {
                return Err("logging levels must be error/warn/info/debug/trace".into());
            }
        }
        for (name, provider) in &self.providers {
            if !identifier(name) {
                return Err("provider names must be bounded identifiers".into());
            }
            let uri: Uri = provider
                .origin
                .parse()
                .map_err(|_| format!("invalid origin for provider {name}"))?;
            if !matches!(uri.scheme_str(), Some("http" | "https"))
                || uri.authority().is_none()
                || uri.authority().is_some_and(|a| a.as_str().contains('@'))
                || uri.path_and_query().is_some_and(|p| p.as_str() != "/")
            {
                return Err(format!(
                    "provider {name}: origin must contain only http(s) scheme and authority"
                ));
            }
            if !provider.base_path.is_empty()
                && (!valid_path(&provider.base_path) || provider.base_path.ends_with('/'))
            {
                return Err(format!(
                    "provider {name}: base_path must be empty or a path without trailing slash"
                ));
            }
            for (alias, model) in &provider.model_aliases {
                if !model_name(alias) || !model_name(model) {
                    return Err(format!(
                        "provider {name}: model names must be 1-256 ASCII letters/digits or -_.:/@+"
                    ));
                }
            }
        }
        let mut ids = HashSet::new();
        for route in &self.routes {
            if !identifier(&route.id) || !ids.insert(&route.id) {
                return Err("invalid or duplicate route id".into());
            }
            if route.path.is_some() == route.path_prefix.is_some() {
                return Err(format!(
                    "route {}: specify exactly one path or path_prefix",
                    route.id
                ));
            }
            if route
                .path
                .iter()
                .chain(route.path_prefix.iter())
                .chain(route.upstream_path.iter())
                .any(|p| !valid_path(p))
            {
                return Err(format!("route {}: invalid path", route.id));
            }
            if route
                .path_prefix
                .as_ref()
                .is_some_and(|p| p != "/" && p.ends_with('/'))
                || (route.strip_prefix && route.path_prefix.is_none())
            {
                return Err(format!("route {}: invalid prefix stripping", route.id));
            }
            if route.method.as_ref().is_some_and(|m| {
                Method::from_bytes(m.as_bytes()).is_err() || m != &m.to_ascii_uppercase()
            }) {
                return Err(format!("route {}: invalid method", route.id));
            }
            if route
                .provider
                .as_ref()
                .is_some_and(|p| !self.providers.contains_key(p))
            {
                return Err(format!("route {}: unknown provider", route.id));
            }
            if route.adapter.is_some()
                && (route.method.as_deref() != Some("POST")
                    || route.path.is_none()
                    || route.prompt_mode.is_none())
            {
                return Err(format!(
                    "route {}: adapter requires exact POST path and prompt_mode",
                    route.id
                ));
            }
            if route.adapter.is_none() && route.prompt_mode.is_some() {
                return Err(format!("route {}: prompt_mode requires adapter", route.id));
            }
            if route.unknown_fields != "reject"
                || route.max_request_bytes == 0
                || route.max_json_response_bytes == 0
                || route.max_sse_event_bytes == 0
            {
                return Err(format!(
                    "route {}: invalid adapter limits or unknown_fields policy",
                    route.id
                ));
            }
        }
        for (index, left) in self.routes.iter().enumerate() {
            for right in &self.routes[index + 1..] {
                if left.path == right.path
                    && left.path_prefix == right.path_prefix
                    && (left.method == right.method
                        || left.method.is_none()
                        || right.method.is_none())
                {
                    return Err("ambiguous route selectors".into());
                }
            }
        }
        Ok(())
    }
    pub fn plan(&self, method: &Method, path: &str) -> Plan<'_> {
        let route = self
            .routes
            .iter()
            .filter(|r| r.method.as_deref().is_none_or(|m| m == method.as_str()))
            .filter(|r| {
                r.path.as_deref() == Some(path)
                    || r.path_prefix
                        .as_deref()
                        .is_some_and(|prefix| prefix_match(prefix, path))
            })
            .max_by_key(|r| {
                (
                    r.path.is_some(),
                    r.path_prefix.as_ref().map_or(0, String::len),
                )
            });
        let provider_name = route
            .and_then(|r| r.provider.as_deref())
            .unwrap_or(&self.default_provider);
        let provider = &self.providers[provider_name];
        let logical_path = route
            .filter(|r| r.strip_prefix)
            .and_then(|r| r.path_prefix.as_ref())
            .map(|p| {
                if p == "/" {
                    path.to_string()
                } else {
                    let suffix = &path[p.len()..];
                    if suffix.is_empty() {
                        "/".into()
                    } else {
                        suffix.into()
                    }
                }
            })
            .unwrap_or_else(|| path.into());
        let upstream_path = route
            .and_then(|r| r.upstream_path.clone())
            .unwrap_or_else(|| format!("{}{}", provider.base_path, logical_path));
        Plan {
            provider_name,
            provider,
            route,
            logical_path,
            upstream_path,
        }
    }
}
pub fn prefix_match(prefix: &str, path: &str) -> bool {
    prefix == "/"
        || path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|s| s.starts_with('/'))
}
fn valid_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.contains(['?', '#', '\\'])
        && !path.bytes().any(|b| b <= 32 || b >= 127)
        && path.parse::<axum::http::uri::PathAndQuery>().is_ok()
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
pub fn model_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:/@+".contains(&b))
}
pub fn default_config_path() -> Result<PathBuf, String> {
    #[cfg(target_os = "linux")]
    let root = xdg("XDG_CONFIG_HOME", ".config")?;
    #[cfg(not(target_os = "linux"))]
    let root = home()?.join(".config");
    Ok(root.join("transit/transit.toml"))
}
pub fn default_log_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "linux")]
    let root = xdg("XDG_STATE_HOME", ".local/state")?.join("transit/logs");
    #[cfg(target_os = "macos")]
    let root = home()?.join("Library/Logs/transit");
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let root = home()?.join(".local/state/transit/logs");
    Ok(root)
}
fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or_else(|| "HOME must identify an absolute directory".into())
}
#[cfg(target_os = "linux")]
fn xdg(name: &str, fallback: &str) -> Result<PathBuf, String> {
    match std::env::var_os(name).filter(|s| !s.is_empty()) {
        Some(value) => {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                Ok(path)
            } else {
                Err(format!("{name} must be absolute"))
            }
        }
        None => Ok(home()?.join(fallback)),
    }
}
