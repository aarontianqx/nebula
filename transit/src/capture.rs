use crate::{
    audit::now_ms,
    config::{Capture, default_state_dir},
    privacy::scrub,
};
use axum::http::HeaderMap;
use base64::{Engine, engine::general_purpose::STANDARD};
use bytes::Bytes;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime},
};
use time::{Date, OffsetDateTime, macros::format_description};
use tokio::sync::mpsc;

const CHUNK_BYTES: usize = 16 * 1024;
const QUEUE_LENGTH: usize = 512;
const END_RESERVE: u64 = 16 * 1024;

#[derive(Default)]
pub struct Health {
    pub degraded: AtomicBool,
    pub failures: AtomicU64,
}
pub struct CaptureLog {
    sender: mpsc::Sender<Command>,
    pub settings: Arc<RwLock<Capture>>,
    pub health: Arc<Health>,
}
pub struct Guard {
    sender: mpsc::Sender<Command>,
    worker: thread::JoinHandle<()>,
}
enum Command {
    Start(String, Value),
    Event(String, Value),
    Body(String, Direction, u64, i64, Bytes),
    Finish(String, Value),
    Maintain,
    Stop,
}
#[derive(Clone, Copy)]
pub enum Direction {
    Request,
    Response,
}
impl Direction {
    fn name(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Response => "response",
        }
    }
}
#[derive(Default)]
struct BodyState {
    seen: u64,
    queued: u64,
    truncated: bool,
}
pub struct Session {
    log: Arc<CaptureLog>,
    id: String,
    terminal: Option<mpsc::OwnedPermit<Command>>,
    settings: Capture,
    request: BodyState,
    response: BodyState,
    queue_error: Option<&'static str>,
}
impl CaptureLog {
    pub fn is_degraded(&self) -> bool {
        self.sender.is_closed() || self.health.degraded.load(Ordering::Relaxed)
    }
    pub fn begin(
        self: &Arc<Self>,
        settings: &Capture,
        provider: &str,
        metadata: Value,
        request_headers: &HeaderMap,
    ) -> Option<Session> {
        if !settings.enabled
            || !settings.providers.is_empty() && !settings.providers.iter().any(|p| p == provider)
        {
            return None;
        }
        let id = metadata.get("request_id")?.as_str()?.to_string();
        // A request never waits for disk or queue capacity. Reserve its final record
        // before admitting body chunks, so queue pressure cannot hide truncation.
        let terminal = match self.sender.clone().try_reserve_owned() {
            Ok(permit) => permit,
            Err(_) => {
                report(&self.health, Some(&id), "queue_unavailable", None);
                return None;
            }
        };
        let event = json!({"type":"request_started","schema_version":1,"timestamp_ms":now_ms(),"metadata":metadata,"headers":headers(request_headers),"request_body_source":"client","response_body_source":"upstream","capture":{"request_body":settings.request_body,"response_body":settings.response_body,"max_body_bytes":settings.max_body_bytes}});
        if self
            .sender
            .try_send(Command::Start(id.clone(), event))
            .is_err()
        {
            report(&self.health, Some(&id), "queue_unavailable", None);
            return None;
        }
        Some(Session {
            log: self.clone(),
            id,
            terminal: Some(terminal),
            settings: settings.clone(),
            request: BodyState::default(),
            response: BodyState::default(),
            queue_error: None,
        })
    }
    pub fn maintain(&self) {
        let _ = self.sender.try_send(Command::Maintain);
    }
}
impl Session {
    pub fn response_headers(&mut self, status: u16, response_headers: &HeaderMap) {
        if self.queue_error.is_none() && self.log.sender.try_send(Command::Event(self.id.clone(),json!({"type":"response_headers","timestamp_ms":now_ms(),"status":status,"headers":headers(response_headers)}))).is_err(){
            self.queue_failed();
        }
    }
    fn queue_failed(&mut self) {
        if self.queue_error.is_none() {
            self.queue_error = Some("queue_unavailable");
            report(&self.log.health, Some(&self.id), "queue_unavailable", None);
        }
    }
    pub fn bytes(&mut self, direction: Direction, bytes: &[u8]) {
        if !match direction {
            Direction::Request => self.settings.request_body,
            Direction::Response => self.settings.response_body,
        } {
            return;
        }
        let state = match direction {
            Direction::Request => &mut self.request,
            Direction::Response => &mut self.response,
        };
        state.seen = state.seen.saturating_add(bytes.len() as u64);
        if self.queue_error.is_some() {
            return;
        }
        let remaining = (self.settings.max_body_bytes as u64).saturating_sub(state.queued) as usize;
        let allowed = remaining.min(bytes.len());
        state.truncated |= allowed < bytes.len();
        let timestamp = now_ms();
        for chunk in bytes[..allowed].chunks(CHUNK_BYTES) {
            let offset = match direction {
                Direction::Request => self.request.queued,
                Direction::Response => self.response.queued,
            };
            let command = Command::Body(
                self.id.clone(),
                direction,
                offset,
                timestamp,
                Bytes::copy_from_slice(chunk),
            );
            if self.log.sender.try_send(command).is_err() {
                self.queue_failed();
                break;
            }
            let state = match direction {
                Direction::Request => &mut self.request,
                Direction::Response => &mut self.response,
            };
            state.queued += chunk.len() as u64;
        }
    }
    pub fn finish(mut self, summary: Value, body_stream_complete: bool) {
        self.finish_inner(summary, body_stream_complete);
    }
    fn finish_inner(&mut self, summary: Value, body_stream_complete: bool) {
        if let Some(permit) = self.terminal.take() {
            let complete = body_stream_complete
                && self.queue_error.is_none()
                && !self.request.truncated
                && !self.response.truncated;
            permit.send(Command::Finish(self.id.clone(),json!({"type":"request_finished","timestamp_ms":now_ms(),"summary":summary,"capture_complete":complete,"body_stream_complete":body_stream_complete,"queue_error":self.queue_error,"request_body":{"enabled":self.settings.request_body,"seen_bytes":self.request.seen,"queued_bytes":self.request.queued,"truncated":self.request.truncated},"response_body":{"enabled":self.settings.response_body,"seen_bytes":self.response.seen,"queued_bytes":self.response.queued,"truncated":self.response.truncated}})));
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.finish_inner(
            json!({"outcome":"interrupted","error_code":"capture_abandoned"}),
            false,
        );
    }
}
impl Guard {
    pub async fn shutdown(self) {
        let _ = self.sender.send(Command::Stop).await;
        if !matches!(
            tokio::task::spawn_blocking(move || self.worker.join()).await,
            Ok(Ok(()))
        ) {
            tracing::error!(event = "capture_worker_panicked");
        }
    }
}
// Deliberate allowlist: authentication, cookies, forwarding headers and arbitrary
// provider extensions never enter captures. Bodies are explicitly opt-in.
fn headers(source: &HeaderMap) -> Value {
    let mut values = serde_json::Map::new();
    for name in [
        "content-type",
        "content-encoding",
        "content-length",
        "accept",
        "accept-encoding",
        "anthropic-version",
        "anthropic-beta",
        "x-request-id",
        "request-id",
    ] {
        let entries = source
            .get_all(name)
            .iter()
            .take(8)
            .filter_map(|v| v.to_str().ok())
            .map(|v| scrub(v, &[], 256))
            .collect::<Vec<_>>();
        if !entries.is_empty() {
            values.insert(name.into(), json!(entries));
        }
    }
    Value::Object(values)
}
fn report(health: &Health, request_id: Option<&str>, code: &str, error: Option<&io::Error>) {
    health.degraded.store(true, Ordering::Relaxed);
    health.failures.fetch_add(1, Ordering::Relaxed);
    tracing::warn!(event="capture_failed",request_id,code,error_kind=?error.map(io::Error::kind));
}
fn private_directory(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    if !fs::symlink_metadata(path)?.file_type().is_dir() {
        return Err(io::Error::other("capture directory must not be a symlink"));
    }
    Ok(())
}
fn private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
struct OpenCapture {
    file: File,
    path: PathBuf,
    stopped: bool,
    stop_reason: Option<&'static str>,
    request_bytes: u64,
    response_bytes: u64,
}
struct StoredFile {
    path: PathBuf,
    bytes: u64,
    modified: SystemTime,
}
struct Writer {
    root: PathBuf,
    settings: Arc<RwLock<Capture>>,
    health: Arc<Health>,
    files: HashMap<String, OpenCapture>,
    active: HashSet<PathBuf>,
    used: u64,
    reserved: u64,
}
impl Writer {
    fn settings(&self) -> Capture {
        self.settings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    fn scan(&self) -> io::Result<Vec<StoredFile>> {
        let mut files = Vec::new();
        match fs::symlink_metadata(&self.root) {
            Ok(metadata) if !metadata.file_type().is_dir() => {
                return Err(io::Error::other(
                    "capture root must be a directory, not a symlink",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(files),
            Err(error) => return Err(error),
            _ => {}
        }
        let days = match fs::read_dir(&self.root) {
            Ok(days) => days,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(files),
            Err(e) => return Err(e),
        };
        for day in days {
            let day = day?;
            let name = day.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.len() != 10
                || Date::parse(name, format_description!("[year]-[month]-[day]")).is_err()
                || !day.file_type()?.is_dir()
            {
                continue;
            }
            for file in fs::read_dir(day.path())? {
                let file = file?;
                let name = file.file_name();
                let Some(id) = name.to_str().and_then(|s| s.strip_suffix(".jsonl")) else {
                    continue;
                };
                if uuid::Uuid::parse_str(id).is_err() || !file.file_type()?.is_file() {
                    continue;
                }
                let metadata = file.metadata()?;
                files.push(StoredFile {
                    path: file.path(),
                    bytes: metadata.len(),
                    modified: metadata.modified()?,
                });
            }
        }
        Ok(files)
    }
    fn sweep(&mut self, needed: u64) -> io::Result<()> {
        let settings = self.settings();
        let mut files = self.scan()?;
        self.used = files.iter().fold(0u64, |n, f| n.saturating_add(f.bytes));
        files.sort_by_key(|f| f.modified);
        let cutoff = SystemTime::now().checked_sub(Duration::from_secs(
            u64::from(settings.retention_days) * 86400,
        ));
        for file in files {
            if self.active.contains(&file.path) {
                continue;
            }
            let expired = cutoff.is_some_and(|cutoff| file.modified < cutoff);
            if expired
                || self
                    .used
                    .saturating_add(self.reserved)
                    .saturating_add(needed)
                    > settings.max_total_bytes
            {
                fs::remove_file(&file.path)?;
                self.used = self.used.saturating_sub(file.bytes);
                if let Some(parent) = file.path.parent() {
                    match fs::remove_dir(parent) {
                        Ok(()) => {}
                        Err(e)
                            if matches!(
                                e.kind(),
                                io::ErrorKind::DirectoryNotEmpty | io::ErrorKind::NotFound
                            ) => {}
                        Err(e) => return Err(e),
                    }
                }
            }
        }
        Ok(())
    }
    fn room(&mut self, bytes: u64) -> io::Result<bool> {
        let limit = self.settings().max_total_bytes;
        if self
            .used
            .saturating_add(self.reserved)
            .saturating_add(bytes)
            > limit
        {
            self.sweep(bytes)?;
        }
        Ok(self
            .used
            .saturating_add(self.reserved)
            .saturating_add(bytes)
            <= limit)
    }
    fn line(event: &Value) -> io::Result<Vec<u8>> {
        let mut line = serde_json::to_vec(event).map_err(io::Error::other)?;
        line.push(b'\n');
        Ok(line)
    }
    fn write(&mut self, file: &mut OpenCapture, event: &Value) -> io::Result<bool> {
        let bytes = Self::line(event)?;
        if !self.room(bytes.len() as u64)? {
            return Ok(false);
        }
        // Charge before writing: a short write remains conservatively accounted
        // until the next directory scan and never permits unbounded disk growth.
        self.used = self.used.saturating_add(bytes.len() as u64);
        file.file.write_all(&bytes)?;
        Ok(true)
    }
    fn start(&mut self, id: String, event: Value) {
        let result = (|| -> io::Result<()> {
            private_directory(&self.root)?;
            self.sweep(END_RESERVE + Self::line(&event)?.len() as u64)?;
            if !self.room(END_RESERVE + Self::line(&event)?.len() as u64)? {
                return Err(io::Error::other("capture storage budget exhausted"));
            }
            let day = self.root.join(OffsetDateTime::now_utc().date().to_string());
            private_directory(&day)?;
            let path = day.join(format!("{id}.jsonl"));
            let file = private_file(&path)?;
            self.reserved += END_RESERVE;
            self.active.insert(path.clone());
            let mut capture = OpenCapture {
                file,
                path,
                stopped: false,
                stop_reason: None,
                request_bytes: 0,
                response_bytes: 0,
            };
            let written = self.write(&mut capture, &event);
            self.files.insert(id.clone(), capture);
            match written? {
                true => Ok(()),
                false => Err(io::Error::other("capture storage budget exhausted")),
            }
        })();
        if let Err(error) = result {
            if let Some(file) = self.files.get_mut(&id) {
                file.stopped = true;
                file.stop_reason = Some("write_failed");
            }
            report(&self.health, Some(&id), "open_failed", Some(&error));
        }
    }
    fn event(&mut self, id: &str, event: Value, direction: Option<(Direction, usize)>) {
        let Some(mut file) = self.files.remove(id) else {
            return;
        };
        if !file.stopped {
            match self.write(&mut file, &event) {
                Ok(true) => {
                    if let Some((direction, bytes)) = direction {
                        match direction {
                            Direction::Request => file.request_bytes += bytes as u64,
                            Direction::Response => file.response_bytes += bytes as u64,
                        }
                    }
                }
                Ok(false) => {
                    file.stopped = true;
                    file.stop_reason = Some("storage_limit");
                    tracing::warn!(
                        event = "capture_incomplete",
                        request_id = id,
                        reason = "storage_limit"
                    );
                }
                Err(error) => {
                    file.stopped = true;
                    file.stop_reason = Some("write_failed");
                    report(&self.health, Some(id), "write_failed", Some(&error));
                }
            }
        }
        self.files.insert(id.into(), file);
    }
    fn finish(&mut self, id: &str, mut event: Value) {
        let Some(mut file) = self.files.remove(id) else {
            return;
        };
        self.reserved = self.reserved.saturating_sub(END_RESERVE);
        let complete = event["capture_complete"] == true && !file.stopped;
        event["capture_complete"] = json!(complete);
        event["storage_error"] = json!(file.stop_reason);
        event["stored_request_body_bytes"] = json!(file.request_bytes);
        event["stored_response_body_bytes"] = json!(file.response_bytes);
        let result = (|| -> io::Result<()> {
            let line = Self::line(&event)?;
            if line.len() as u64 > END_RESERVE {
                return Err(io::Error::other(
                    "capture final metadata exceeds reserved budget",
                ));
            }
            // The reserved footer may exceed a newly lowered live quota; active
            // captures are never removed to enforce a configuration change.
            self.used = self.used.saturating_add(line.len() as u64);
            file.file.write_all(&line)?;
            file.file.flush()
        })();
        self.active.remove(&file.path);
        if let Err(error) = result {
            report(&self.health, Some(id), "finish_failed", Some(&error));
        } else if complete {
            self.health.degraded.store(false, Ordering::Relaxed);
            tracing::debug!(event = "capture_completed", request_id = id);
        } else {
            tracing::warn!(
                event = "capture_incomplete",
                request_id = id,
                reason = file.stop_reason.unwrap_or("body_limit_or_queue_pressure")
            );
        }
    }
}
pub fn initialize(settings: &Capture) -> Result<(Arc<CaptureLog>, Guard), String> {
    let root = settings
        .directory
        .clone()
        .map(Ok)
        .unwrap_or_else(|| default_state_dir().map(|p| p.join("captures")))?;
    let shared = Arc::new(RwLock::new(settings.clone()));
    let health = Arc::new(Health::default());
    let mut writer = Writer {
        root,
        settings: shared.clone(),
        health: health.clone(),
        files: HashMap::new(),
        active: HashSet::new(),
        used: 0,
        reserved: 0,
    };
    let (sender, mut receiver) = mpsc::channel(QUEUE_LENGTH);
    let worker = thread::Builder::new()
        .name("transit-capture".into())
        .spawn(move || {
            let mut activated = writer.settings().enabled;
            if activated && let Err(error) = writer.sweep(0) {
                report(&writer.health, None, "cleanup_failed", Some(&error));
            }
            let mut swept = Instant::now();
            while let Some(command) = receiver.blocking_recv() {
                activated |= writer.settings().enabled;
                match command {
                    Command::Start(id, event) => writer.start(id, event),
                    Command::Event(id, event) => writer.event(&id, event, None),
                    Command::Body(id, direction, offset, timestamp, bytes) => {
                        let event = json!({"type":"body_chunk","direction":direction.name(),"timestamp_ms":timestamp,"offset":offset,"bytes":bytes.len(),"data_base64":STANDARD.encode(&bytes)});
                        writer.event(&id, event, Some((direction, bytes.len())));
                    }
                    Command::Finish(id, event) => writer.finish(&id, event),
                    Command::Maintain => {},
                    Command::Stop => break,
                }
                if activated && swept.elapsed() >= Duration::from_secs(60) {
                    if let Err(error) = writer.sweep(0) {
                        report(&writer.health, None, "cleanup_failed", Some(&error));
                    }
                    swept = Instant::now();
                }
            }
            for id in writer.files.keys().cloned().collect::<Vec<_>>() {
                writer.finish(&id, json!({"type":"request_finished","timestamp_ms":now_ms(),"capture_complete":false,"summary":{"outcome":"interrupted","error_code":"capture_writer_stopped"}}));
            }
        })
        .map_err(|_| "cannot start capture writer")?;
    Ok((
        Arc::new(CaptureLog {
            sender: sender.clone(),
            settings: shared,
            health,
        }),
        Guard { sender, worker },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Capture {
        Capture {
            enabled: true,
            directory: Some(
                std::env::temp_dir().join(format!("transit-capture-{}", uuid::Uuid::new_v4())),
            ),
            ..Default::default()
        }
    }
    fn metadata(id: &str) -> Value {
        json!({"request_id":id,"provider":"fixture","request_path":"/v1/chat/completions"})
    }
    fn captured(root: &Path, id: &str) -> Vec<Value> {
        let path = root
            .join(OffsetDateTime::now_utc().date().to_string())
            .join(format!("{id}.jsonl"));
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect()
    }
    #[tokio::test]
    async fn raw_binary_capture_is_bounded_private_and_filters_headers() {
        let mut config = settings();
        config.max_body_bytes = 8;
        config.providers = vec!["fixture".into()];
        let (log, guard) = initialize(&config).unwrap();
        assert!(
            log.begin(
                &config,
                "other",
                metadata(&uuid::Uuid::new_v4().to_string()),
                &HeaderMap::new()
            )
            .is_none()
        );
        let id = uuid::Uuid::new_v4().to_string();
        let mut h = HeaderMap::new();
        h.insert("authorization", "Bearer fixture-secret".parse().unwrap());
        h.insert("cookie", "private-cookie".parse().unwrap());
        h.insert("content-type", "text/event-stream".parse().unwrap());
        h.insert("content-encoding", "gzip".parse().unwrap());
        h.insert("set-cookie", "private-cookie".parse().unwrap());
        let mut session = log.begin(&config, "fixture", metadata(&id), &h).unwrap();
        session.bytes(Direction::Request, b"not captured");
        session.response_headers(200, &h);
        let binary = [0x1f, 0x8b, 0, 255, 128, 64, 0, 0, 42, 43];
        session.bytes(Direction::Response, &binary);
        session.finish(json!({"outcome":"completed"}), true);
        drop(log);
        guard.shutdown().await;
        let rows = captured(config.directory.as_ref().unwrap(), &id);
        assert_eq!(rows[1]["headers"]["content-encoding"][0], "gzip");
        let chunks = rows
            .iter()
            .filter(|r| r["type"] == "body_chunk")
            .collect::<Vec<_>>();
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            STANDARD
                .decode(chunks[0]["data_base64"].as_str().unwrap())
                .unwrap(),
            binary[..8]
        );
        let end = rows.last().unwrap();
        assert_eq!(end["capture_complete"], false);
        assert_eq!(end["response_body"]["truncated"], true);
        assert_eq!(end["stored_response_body_bytes"], 8);
        assert_eq!(end["stored_request_body_bytes"], 0);
        let text = serde_json::to_string(&rows).unwrap();
        assert!(!text.contains("fixture-secret"));
        assert!(!text.contains("private-cookie"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let root = config.directory.as_ref().unwrap();
            assert_eq!(
                fs::metadata(root).unwrap().permissions().mode() & 0o777,
                0o700
            );
            let file = root
                .join(OffsetDateTime::now_utc().date().to_string())
                .join(format!("{id}.jsonl"));
            assert_eq!(
                fs::metadata(file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(config.directory.unwrap()).unwrap();
    }
    #[tokio::test]
    async fn queue_pressure_preserves_a_final_incomplete_record() {
        let config = settings();
        let (sender, mut receiver) = mpsc::channel(4);
        let log = Arc::new(CaptureLog {
            sender,
            settings: Arc::new(RwLock::new(config.clone())),
            health: Arc::new(Health::default()),
        });
        let id = uuid::Uuid::new_v4().to_string();
        let mut session = log
            .begin(&config, "fixture", metadata(&id), &HeaderMap::new())
            .unwrap();
        session.bytes(Direction::Response, &vec![42; 4 * CHUNK_BYTES]);
        session.finish(json!({"outcome":"completed"}), true);
        assert!(log.is_degraded());
        let mut terminal = None;
        while let Ok(command) = receiver.try_recv() {
            if let Command::Finish(_, event) = command {
                terminal = Some(event);
            }
        }
        let terminal = terminal.unwrap();
        assert_eq!(terminal["capture_complete"], false);
        assert_eq!(terminal["queue_error"], "queue_unavailable");
        assert_eq!(terminal["response_body"]["seen_bytes"], 4 * CHUNK_BYTES);
    }
    #[tokio::test]
    async fn disabled_capture_does_not_touch_storage_and_io_failure_is_visible() {
        let mut config = settings();
        config.enabled = false;
        let (log, guard) = initialize(&config).unwrap();
        drop(log);
        guard.shutdown().await;
        assert!(!config.directory.as_ref().unwrap().exists());
        fs::write(config.directory.as_ref().unwrap(), b"unrelated file").unwrap();
        config.enabled = true;
        let (log, guard) = initialize(&config).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let mut session = log
            .begin(&config, "fixture", metadata(&id), &HeaderMap::new())
            .unwrap();
        session.bytes(Direction::Response, b"still forwarded by caller");
        session.finish(json!({"outcome":"completed"}), true);
        guard.shutdown().await;
        assert!(log.is_degraded());
        assert!(log.health.failures.load(Ordering::Relaxed) > 0);
        assert_eq!(
            fs::read(config.directory.as_ref().unwrap()).unwrap(),
            b"unrelated file"
        );
        fs::remove_file(config.directory.unwrap()).unwrap();
    }
    #[test]
    fn disk_budget_and_retention_preserve_active_unknown_and_symlink_files() {
        let mut config = settings();
        config.max_total_bytes = 64 * 1024;
        let root = config.directory.as_ref().unwrap().clone();
        let day = root.join(OffsetDateTime::now_utc().date().to_string());
        private_directory(&day).unwrap();
        let old = day.join(format!("{}.jsonl", uuid::Uuid::new_v4()));
        let file = private_file(&old).unwrap();
        file.set_times(
            fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(4 * 86400)),
        )
        .unwrap();
        let unknown = day.join("notes.jsonl");
        fs::write(&unknown, b"keep").unwrap();
        #[cfg(unix)]
        let symlink = {
            let link = day.join(format!("{}.jsonl", uuid::Uuid::new_v4()));
            std::os::unix::fs::symlink(&unknown, &link).unwrap();
            link
        };
        let mut writer = Writer {
            root: root.clone(),
            settings: Arc::new(RwLock::new(config)),
            health: Arc::new(Health::default()),
            files: HashMap::new(),
            active: HashSet::new(),
            used: 0,
            reserved: 0,
        };
        writer.sweep(0).unwrap();
        assert!(!old.exists());
        assert!(unknown.exists());
        #[cfg(unix)]
        assert!(
            fs::symlink_metadata(symlink)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let id = uuid::Uuid::new_v4().to_string();
        writer.start(id.clone(), json!({"type":"request_started"}));
        for _ in 0..4 {
            writer.event(
                &id,
                json!({"type":"body_chunk","data_base64":"x".repeat(20000)}),
                Some((Direction::Response, 15000)),
            );
        }
        assert!(writer.files[&id].stopped);
        assert!(writer.files[&id].path.exists());
        writer.finish(
            &id,
            json!({"type":"request_finished","capture_complete":true}),
        );
        let rows = captured(&root, &id);
        let end = rows.last().unwrap();
        assert_eq!(end["capture_complete"], false);
        assert_eq!(end["storage_error"], "storage_limit");
        assert!(writer.scan().unwrap().iter().map(|f| f.bytes).sum::<u64>() <= 64 * 1024);
        fs::remove_dir_all(root).unwrap();
    }
}
