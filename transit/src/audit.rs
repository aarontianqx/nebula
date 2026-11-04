use crate::config::{Audit, default_state_dir};
use fs2::FileExt;
use rusqlite::Connection;
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};
use tokio::sync::mpsc;

const FIELDS: &[(&str, &str)] = &[
    ("request_id", "TEXT PRIMARY KEY"),
    ("started_at_ms", "INTEGER NOT NULL"),
    ("finished_at_ms", "INTEGER"),
    ("config_generation", "INTEGER"),
    ("provider", "TEXT"),
    ("rule_id", "TEXT"),
    ("method", "TEXT"),
    ("request_path", "TEXT"),
    ("upstream_path", "TEXT"),
    ("client_protocol", "TEXT"),
    ("upstream_protocol", "TEXT"),
    ("requested_model", "TEXT"),
    ("resolved_model", "TEXT"),
    ("response_model", "TEXT"),
    ("streaming", "INTEGER"),
    ("http_status", "INTEGER"),
    ("upstream_status", "INTEGER"),
    ("outcome", "TEXT NOT NULL"),
    ("duration_ms", "INTEGER"),
    ("response_headers_ms", "INTEGER"),
    ("first_body_byte_ms", "INTEGER"),
    ("request_bytes", "INTEGER"),
    ("response_bytes", "INTEGER"),
    ("error_source", "TEXT"),
    ("error_stage", "TEXT"),
    ("error_code", "TEXT"),
    ("error_message", "TEXT"),
    ("finish_reason", "TEXT"),
    ("response_status", "TEXT"),
    ("usage_status", "TEXT"),
    ("observer_status", "TEXT"),
    ("observer_error", "TEXT"),
    ("input_tokens", "INTEGER"),
    ("output_tokens", "INTEGER"),
    ("total_tokens", "INTEGER"),
    ("cached_input_tokens", "INTEGER"),
    ("cache_write_input_tokens", "INTEGER"),
    ("reasoning_tokens", "INTEGER"),
    ("usage_json", "TEXT"),
];
#[derive(Default)]
pub struct Health {
    pub degraded: AtomicBool,
    pub failures: AtomicU64,
    pub dropped_records: AtomicU64,
}
pub struct AuditLog {
    sender: mpsc::Sender<Command>,
    pub health: Arc<Health>,
}
pub struct Ticket {
    permit: Option<mpsc::OwnedPermit<Command>>,
    id: String,
}
enum Command {
    Begin(Value),
    Finish(Value),
    Maintain,
    Stop,
}
pub struct Guard {
    sender: mpsc::Sender<Command>,
    worker: thread::JoinHandle<()>,
    _lock: File,
}
impl AuditLog {
    pub fn is_degraded(&self) -> bool {
        self.health.degraded.load(Ordering::Relaxed) || self.sender.is_closed()
    }
    pub fn maintain(&self) {
        // A full queue is already waking the worker, which checks retention after each command.
        let _ = self.sender.try_send(Command::Maintain);
    }
    pub fn begin(self: &Arc<Self>, row: Value) -> Result<Ticket, &'static str> {
        let result = (|| {
            let id = row["request_id"]
                .as_str()
                .ok_or("invalid_audit_record")?
                .to_string();
            // Reserve final metadata without waiting for SQLite or queue capacity.
            let permit = self
                .sender
                .clone()
                .try_reserve_owned()
                .map_err(|_| "audit_queue_unavailable")?;
            self.sender
                .try_send(Command::Begin(row))
                .map_err(|_| "audit_queue_unavailable")?;
            Ok(Ticket {
                permit: Some(permit),
                id,
            })
        })();
        if let Err(code) = result {
            self.health.dropped_records.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(event = "audit_record_dropped", code);
        }
        result
    }
}
impl Ticket {
    pub fn finish(mut self, row: Value) {
        if let Some(permit) = self.permit.take() {
            permit.send(Command::Finish(row));
        }
    }
}
impl Drop for Ticket {
    fn drop(&mut self) {
        if let Some(permit) = self.permit.take() {
            permit.send(Command::Finish(serde_json::json!({"request_id":self.id,"outcome":"interrupted","finished_at_ms":now_ms(),"usage_status":"missing","error_source":"local","error_code":"request_abandoned"})));
        }
    }
}
impl Guard {
    pub async fn shutdown(self) {
        let _ = self.sender.send(Command::Stop).await;
        if !matches!(
            tokio::task::spawn_blocking(move || {
                let result = self.worker.join();
                drop(self._lock);
                result
            })
            .await,
            Ok(Ok(()))
        ) {
            tracing::error!(event = "audit_worker_panicked");
        }
    }
}
pub fn now_ms() -> i64 {
    time::OffsetDateTime::now_utc()
        .unix_timestamp_nanos()
        .checked_div(1_000_000)
        .unwrap_or(0)
        .try_into()
        .unwrap_or(i64::MAX)
}
fn private_file(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|_| "cannot open audit database/lock file".into())
}
pub fn open(
    settings: &Audit,
    max_in_flight: usize,
) -> Result<Option<(Arc<AuditLog>, Guard)>, String> {
    if !settings.enabled {
        return Ok(None);
    }
    let path = settings
        .database
        .clone()
        .unwrap_or(default_state_dir()?.join("transit.sqlite3"));
    let parent = path
        .parent()
        .ok_or("audit database needs a parent directory")?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(parent)
        .map_err(|_| "cannot create audit directory")?;
    let mut lock_path = path.as_os_str().to_os_string();
    lock_path.push(".lock");
    let lock = private_file(Path::new(&lock_path))?;
    lock.try_lock_exclusive()
        .map_err(|_| "audit database is already in use")?;
    drop(private_file(&path)?);
    let connection = Connection::open(&path).map_err(|_| "cannot open SQLite audit database")?;
    connection
        .busy_timeout(Duration::from_secs(2))
        .map_err(|_| "cannot set SQLite busy timeout")?;
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|_| "cannot read audit schema version")?;
    if version > 1 {
        return Err("unsupported audit schema version".into());
    }
    connection
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")
        .map_err(|_| "cannot configure SQLite WAL")?;
    let definitions = FIELDS
        .iter()
        .map(|(name, kind)| format!("{name} {kind}"))
        .collect::<Vec<_>>()
        .join(",");
    connection
        .execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS requests ({definitions});
        CREATE INDEX IF NOT EXISTS requests_time ON requests(started_at_ms);
        CREATE INDEX IF NOT EXISTS requests_provider_time ON requests(provider,started_at_ms);
        CREATE INDEX IF NOT EXISTS requests_model_time ON requests(resolved_model,started_at_ms);
        PRAGMA user_version=1;"
        ))
        .map_err(|_| "cannot initialize audit schema")?;
    connection.execute("UPDATE requests SET outcome='interrupted',finished_at_ms=?1,error_source='local',error_code='service_interrupted',usage_status=CASE WHEN usage_json IS NULL THEN 'missing' ELSE 'partial' END WHERE outcome='in_progress'",[now_ms()]).map_err(|_|"cannot recover interrupted audit requests")?;
    cleanup(&connection, settings.retention_days).map_err(|_| "cannot apply audit retention")?;
    let (sender, mut receiver) = mpsc::channel(2 * max_in_flight + 64);
    let health = Arc::new(Health::default());
    let worker_health = health.clone();
    let retention = settings.retention_days;
    let names = FIELDS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(",");
    let values = FIELDS
        .iter()
        .map(|(n, _)| format!("json_extract(?1,'$.{n}')"))
        .collect::<Vec<_>>()
        .join(",");
    let insert = format!("INSERT INTO requests ({names}) VALUES ({values})");
    // COALESCE retains the persisted start metadata when cancellation only supplies an ID/result.
    let updates = FIELDS
        .iter()
        .skip(1)
        .map(|(n, _)| format!("{n}=COALESCE(json_extract(?1,'$.{n}'),{n})"))
        .collect::<Vec<_>>()
        .join(",");
    let update =
        format!("UPDATE requests SET {updates} WHERE request_id=json_extract(?1,'$.request_id')");
    let worker = thread::Builder::new()
        .name("transit-audit".into())
        .spawn(move || {
            let mut last_cleanup = std::time::Instant::now();
            while let Some(command) = receiver.blocking_recv() {
                let write = matches!(command, Command::Begin(_) | Command::Finish(_));
                let result = match command {
                    Command::Begin(row) => connection.execute(&insert, [row.to_string()]),
                    Command::Finish(row) => connection.execute(&update, [row.to_string()]).and_then(|n| if n == 0 {Err(rusqlite::Error::QueryReturnedNoRows)} else {Ok(n)}),
                    Command::Maintain => Ok(0),
                    Command::Stop => break,
                };
                if let Err(error) = result {
                    worker_health.degraded.store(true, Ordering::Relaxed);
                    worker_health.failures.fetch_add(1, Ordering::Relaxed);
                    tracing::error!(event = "audit_write_failed", sqlite_code = ?error.sqlite_error_code());
                } else if write && worker_health.degraded.swap(false, Ordering::Relaxed) {
                    tracing::info!(event = "audit_recovered");
                }
                if last_cleanup.elapsed() >= Duration::from_secs(3600) {
                    if let Err(error) = cleanup(&connection, retention) {
                        worker_health.degraded.store(true, Ordering::Relaxed);
                        worker_health.failures.fetch_add(1, Ordering::Relaxed);
                        tracing::error!(event = "audit_retention_failed", sqlite_code = ?error.sqlite_error_code());
                    }
                    last_cleanup = std::time::Instant::now();
                }
            }
            if connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);").is_err() {
                tracing::error!(event = "audit_checkpoint_failed");
            }
        })
        .map_err(|_| "cannot start audit writer")?;
    tracing::info!(event="audit_started",database=%path.display());
    Ok(Some((
        Arc::new(AuditLog {
            sender: sender.clone(),
            health,
        }),
        Guard {
            sender,
            worker,
            _lock: lock,
        },
    )))
}
fn cleanup(connection: &Connection, days: u64) -> rusqlite::Result<usize> {
    let cutoff = now_ms().saturating_sub((days * 86400 * 1000).min(i64::MAX as u64) as i64);
    connection.execute(
        "DELETE FROM requests WHERE finished_at_ms < ?1 AND outcome != 'in_progress'",
        [cutoff],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[tokio::test]
    async fn storage_failures_do_not_block_admission_and_recovery_is_visible() {
        let dir = std::env::temp_dir().join(format!("transit-audit-{}", uuid::Uuid::new_v4()));
        let settings = Audit {
            enabled: true,
            database: Some(dir.join("requests.sqlite3")),
            ..Default::default()
        };
        let (log, guard) = open(&settings, 2).unwrap().unwrap();
        let reader = Connection::open(settings.database.as_ref().unwrap()).unwrap();
        reader.execute_batch("CREATE TRIGGER fail_writes BEFORE INSERT ON requests BEGIN SELECT RAISE(FAIL,'fixture storage failure'); END;").unwrap();
        let ticket = log
            .begin(json!({"request_id":"failed","started_at_ms":now_ms(),"outcome":"in_progress"}))
            .unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        while !log.is_degraded() {
            assert!(tokio::time::Instant::now() < deadline);
            tokio::task::yield_now().await;
        }
        assert_eq!(log.health.failures.load(Ordering::Relaxed), 1);
        // A degraded writer still accepts metadata; model requests never wait for it.
        let next = log
            .begin(json!({"request_id":"next","started_at_ms":now_ms(),"outcome":"in_progress"}))
            .unwrap();
        drop(next);
        drop(ticket);
        drop(log);
        guard.shutdown().await;
        reader.execute_batch("DROP TRIGGER fail_writes;").unwrap();
        let (log, guard) = open(&settings, 2).unwrap().unwrap();
        drop(
            log.begin(
                json!({"request_id":"recovered","started_at_ms":now_ms(),"outcome":"in_progress"}),
            )
            .unwrap(),
        );
        drop(log);
        guard.shutdown().await;
        assert_eq!(
            reader
                .query_row(
                    "select outcome from requests where request_id='recovered'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "interrupted"
        );
        drop(reader);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn queued_starts_and_abandoned_requests_are_recoverable() {
        let dir = std::env::temp_dir().join(format!("transit-audit-{}", uuid::Uuid::new_v4()));
        let settings = Audit {
            enabled: true,
            database: Some(dir.join("requests.sqlite3")),
            ..Default::default()
        };
        let (log, guard) = open(&settings, 2).unwrap().unwrap();
        assert!(open(&settings, 2).is_err());
        let ticket=log.begin(json!({"request_id":"fixture","started_at_ms":now_ms(),"outcome":"in_progress","provider":"fixture"})).unwrap();
        let reader = Connection::open(settings.database.as_ref().unwrap()).unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        while reader
            .query_row(
                "SELECT count(*) FROM requests WHERE request_id='fixture'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            == 0
        {
            assert!(tokio::time::Instant::now() < deadline);
            tokio::task::yield_now().await;
        }
        assert_eq!(
            reader
                .query_row(
                    "SELECT outcome FROM requests WHERE request_id='fixture'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "in_progress"
        );
        drop(ticket);
        drop(log);
        guard.shutdown().await;
        assert_eq!(
            reader
                .query_row(
                    "SELECT outcome FROM requests WHERE request_id='fixture'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "interrupted"
        );
        reader.execute("INSERT INTO requests(request_id,started_at_ms,outcome) VALUES('crash',?1,'in_progress')",[now_ms()]).unwrap();
        let (log, guard) = open(&settings, 2).unwrap().unwrap();
        assert_eq!(
            reader
                .query_row(
                    "SELECT outcome FROM requests WHERE request_id='crash'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "interrupted"
        );
        assert_eq!(
            reader
                .query_row(
                    "SELECT input_tokens FROM requests WHERE request_id='crash'",
                    [],
                    |r| r.get::<_, Option<i64>>(0)
                )
                .unwrap(),
            None
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(settings.database.as_ref().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        drop(log);
        guard.shutdown().await;
        drop(reader);
        fs::remove_dir_all(dir).unwrap();
    }
}
