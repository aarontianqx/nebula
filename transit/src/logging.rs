use crate::config::{Logging, default_log_dir};
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
};
use time::{Date, OffsetDateTime, macros::format_description};
use tracing::{Level, Metadata, Subscriber};
use tracing_appender::{
    non_blocking::{ErrorCounter, WorkerGuard},
    rolling::{RollingFileAppender, Rotation},
};
use tracing_subscriber::{
    layer::{Context, Filter, SubscriberExt},
    util::SubscriberInitExt,
};

pub struct LogState {
    pub settings: RwLock<Logging>,
    pub directory: PathBuf,
    pub health: Arc<WriterHealth>,
    pub dropped: ErrorCounter,
    reported_dropped: AtomicU64,
}
pub struct LogGuard {
    _worker: WorkerGuard,
    _lock: File,
}
pub struct WriterHealth {
    pub degraded: AtomicBool,
    pub failures: AtomicU64,
    active_file: Mutex<PathBuf>,
    last_alert: AtomicU64,
}
struct Writer {
    inner: RollingFileAppender,
    directory: PathBuf,
    health: Arc<WriterHealth>,
}
impl Writer {
    fn report(&self, error: &io::Error) {
        self.health.failures.fetch_add(1, Ordering::Relaxed);
        self.health.degraded.store(true, Ordering::Relaxed);
        let now = unix_seconds();
        let previous = self.health.last_alert.load(Ordering::Relaxed);
        if now.saturating_sub(previous) >= 30
            && self
                .health
                .last_alert
                .compare_exchange(previous, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            eprintln!(
                "transit logging_degraded: {:?}; inspect log directory permissions and disk space",
                error.kind()
            );
        }
    }
    fn write_event(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > 16 * 1024 {
            return Err(io::Error::other("log event exceeded 16 KiB"));
        }
        // Coordinate rollover with the janitor even if the wall clock moves backwards.
        let mut active = self
            .health
            .active_file
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let expected = self.directory.join(file_name());
        // Create with private permissions before the library opens the new day file.
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        if expected != *active || !expected.is_file() {
            options.open(&expected)?;
            self.inner = RollingFileAppender::builder()
                .rotation(Rotation::DAILY)
                .filename_prefix("transit")
                .filename_suffix("jsonl")
                .build(&self.directory)
                .map_err(io::Error::other)?;
        }
        self.inner.write_all(buf)?;
        // The library reports rollover-open errors only to stderr and may retain the old file.
        if !expected.is_file() {
            return Err(io::Error::other("daily rollover target is unavailable"));
        }
        *active = expected;
        Ok(buf.len())
    }
}
impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let result = self.write_event(buf);
        match &result {
            Err(error) => self.report(error),
            Ok(_) => {
                if self.health.degraded.swap(false, Ordering::Relaxed) {
                    eprintln!("transit logging_recovered");
                }
            }
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        let result = self.inner.flush();
        if let Err(ref error) = result {
            self.report(error);
        }
        result
    }
}
#[derive(Clone)]
struct DynamicFilter {
    settings: Arc<LogState>,
    stderr: bool,
}
impl<S: Subscriber> Filter<S> for DynamicFilter {
    fn enabled(&self, metadata: &Metadata<'_>, _: &Context<'_, S>) -> bool {
        let settings = self
            .settings
            .settings
            .read()
            .unwrap_or_else(|e| e.into_inner());
        let configured = if self.stderr {
            &settings.stderr_level
        } else {
            &settings.level
        };
        let limit = if metadata.target().starts_with("transit") {
            level_number(configured)
        } else {
            2
        };
        event_number(metadata.level()) <= limit
    }
}
fn level_number(level: &str) -> u8 {
    match level {
        "error" => 1,
        "warn" => 2,
        "info" => 3,
        "debug" => 4,
        _ => 5,
    }
}
fn event_number(level: &Level) -> u8 {
    match *level {
        Level::ERROR => 1,
        Level::WARN => 2,
        Level::INFO => 3,
        Level::DEBUG => 4,
        Level::TRACE => 5,
    }
}
fn file_name() -> String {
    format!("transit.{}.jsonl", OffsetDateTime::now_utc().date())
}
fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn initialize(settings: &Logging) -> Result<(Arc<LogState>, LogGuard), String> {
    let directory = match &settings.directory {
        Some(path) => path.clone(),
        None => default_log_dir()?,
    };
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(&directory)
        .map_err(|e| format!("cannot create log directory {}: {e}", directory.display()))?;
    let lock_path = directory.join(".transit.lock");
    let lock = open_private(&lock_path)?;
    lock.try_lock_exclusive().map_err(|_| {
        format!(
            "log directory {} is locked by another instance",
            directory.display()
        )
    })?;
    let current = directory.join(file_name());
    let mut initial = open_private(&current)?;
    writeln!(initial,"{}",serde_json::json!({"timestamp":OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap_or_default(),"level":"INFO","event":"logging_initialized","directory":directory,"rotation":"daily_utc","retention_days":settings.retention_days})).map_err(|e|format!("cannot write initial log: {e}"))?;
    initial
        .flush()
        .map_err(|e| format!("cannot flush initial log: {e}"))?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("transit")
        .filename_suffix("jsonl")
        .build(&directory)
        .map_err(|e| format!("cannot initialize log appender: {e}"))?;
    let health = Arc::new(WriterHealth {
        degraded: AtomicBool::new(false),
        failures: AtomicU64::new(0),
        active_file: Mutex::new(current),
        last_alert: AtomicU64::new(0),
    });
    let writer = Writer {
        inner: appender,
        directory: directory.clone(),
        health: health.clone(),
    };
    let (non_blocking, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(8192)
        .lossy(true)
        .finish(writer);
    let state = Arc::new(LogState {
        settings: RwLock::new(settings.clone()),
        directory,
        health,
        dropped: non_blocking.error_counter(),
        reported_dropped: AtomicU64::new(0),
    });
    use tracing_subscriber::Layer;
    let file = tracing_subscriber::fmt::layer()
        .json()
        .with_ansi(false)
        .with_target(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_writer(non_blocking)
        .with_filter(DynamicFilter {
            settings: state.clone(),
            stderr: false,
        });
    let console = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(io::stderr)
        .with_filter(DynamicFilter {
            settings: state.clone(),
            stderr: true,
        });
    tracing_subscriber::registry()
        .with(file)
        .with(console)
        .try_init()
        .map_err(|e| format!("cannot initialize subscriber: {e}"))?;
    Ok((
        state,
        LogGuard {
            _worker: guard,
            _lock: lock,
        },
    ))
}
fn open_private(path: &std::path::Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.create(true).append(true).read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))
}
impl LogState {
    pub fn preview_limit(&self) -> usize {
        self.settings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .error_preview_bytes
    }
    pub fn check_drops(&self) {
        let count = self.dropped.dropped_lines() as u64;
        let previous = self.reported_dropped.swap(count, Ordering::Relaxed);
        if count > previous {
            self.health.degraded.store(true, Ordering::Relaxed);
            eprintln!(
                "transit log_events_dropped: delta={} total={count}",
                count - previous
            );
        }
    }
    pub fn sweep(&self) {
        let settings = self
            .settings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let cutoff = SystemTime::now().checked_sub(Duration::from_secs(
            u64::from(settings.retention_days) * 86400,
        ));
        let Some(cutoff) = cutoff else { return };
        let entries = match fs::read_dir(&self.directory) {
            Ok(e) => e,
            Err(error) => {
                tracing::warn!(event="janitor_failed",error_kind=?error.kind());
                return;
            }
        };
        let mut deleted = 0u64;
        let mut released = 0u64;
        let mut failed = 0u64;
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(error) => {
                    failed += 1;
                    tracing::warn!(event="janitor_entry_failed",error_kind=?error.kind());
                    continue;
                }
            };
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(date) = name
                .strip_prefix("transit.")
                .and_then(|s| s.strip_suffix(".jsonl"))
            else {
                continue;
            };
            if date.len() != 10
                || !date.bytes().enumerate().all(|(index, byte)| {
                    if index == 4 || index == 7 {
                        byte == b'-'
                    } else {
                        byte.is_ascii_digit()
                    }
                })
                || Date::parse(date, format_description!("[year]-[month]-[day]")).is_err()
            {
                continue;
            }
            let active = self
                .health
                .active_file
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if entry.path() == *active {
                continue;
            }
            let metadata = match fs::symlink_metadata(entry.path()) {
                Ok(m) => m,
                Err(error) => {
                    failed += 1;
                    tracing::warn!(event="janitor_metadata_failed",file=name,error_kind=?error.kind());
                    continue;
                }
            };
            if !metadata.file_type().is_file() {
                continue;
            }
            match metadata.modified() {
                Ok(modified) if modified < cutoff => match fs::remove_file(entry.path()) {
                    Ok(()) => {
                        deleted += 1;
                        released += metadata.len()
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => {
                        failed += 1;
                        tracing::warn!(event="janitor_delete_failed",file=name,error_kind=?error.kind());
                    }
                },
                Ok(modified) if modified > SystemTime::now() => {
                    tracing::warn!(event = "janitor_future_timestamp", file = name);
                }
                Err(error) => {
                    failed += 1;
                    tracing::warn!(event="janitor_metadata_failed",file=name,error_kind=?error.kind());
                }
                _ => {}
            }
        }
        tracing::info!(
            event = "janitor_completed",
            deleted,
            released_bytes = released,
            failed
        );
    }
}
