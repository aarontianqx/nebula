//! Best-effort response observation on an independent, bounded CPU runtime.
use crate::{
    audit::Ticket,
    config::{Audit, Protocol, model_name},
    observer::Observer,
};
use async_compression::tokio::bufread::{BrotliDecoder, GzipDecoder, ZlibDecoder, ZstdDecoder};
use bytes::Bytes;
use serde_json::{Value, json};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};
use tokio::{
    io::{AsyncBufRead, AsyncReadExt, BufReader},
    runtime::Handle,
    sync::{Notify, OwnedSemaphorePermit, Semaphore, mpsc, oneshot},
};
use tokio_util::io::StreamReader;

const CHUNK_BYTES: usize = 16 * 1024;
const REQUEST_QUEUE: usize = 64;
const QUEUE_FULL: u8 = 1;
const WORKER_FAILED: u8 = 2;

#[derive(Default)]
pub struct Health {
    pub active: AtomicU64,
    pub skipped: AtomicU64,
    pub failures: AtomicU64,
}
pub struct Pool {
    runtime: Handle,
    slots: Arc<Semaphore>,
    bytes: Arc<Semaphore>,
    stopped: AtomicBool,
    pub health: Arc<Health>,
    idle: Arc<Notify>,
}
pub struct Guard {
    pool: Arc<Pool>,
    stop: oneshot::Sender<()>,
    worker: thread::JoinHandle<()>,
}
struct Chunk {
    bytes: Bytes,
    _budget: OwnedSemaphorePermit,
}
struct Headers {
    media: String,
    encoding: String,
}
struct Completion {
    row: Value,
    transport_complete: bool,
}
pub struct Session {
    sender: Option<mpsc::Sender<Chunk>>,
    request_sender: Option<mpsc::Sender<Chunk>>,
    headers: Option<oneshot::Sender<Headers>>,
    completion: Option<oneshot::Sender<Completion>>,
    state: Arc<AtomicU8>,
    accepting: Arc<AtomicBool>,
    pool: Arc<Pool>,
    id: String,
}
struct Active {
    _slot: OwnedSemaphorePermit,
    health: Arc<Health>,
    idle: Arc<Notify>,
}
impl Drop for Active {
    fn drop(&mut self) {
        self.health.active.fetch_sub(1, Ordering::Relaxed);
        self.idle.notify_one();
    }
}
impl Pool {
    pub fn begin(
        self: &Arc<Self>,
        id: &str,
        protocol: Protocol,
        settings: &Audit,
        secrets: Vec<String>,
        ticket: Ticket,
    ) -> Result<Session, Ticket> {
        let slot = if self.stopped.load(Ordering::Relaxed) {
            None
        } else {
            self.slots.clone().try_acquire_owned().ok()
        };
        let Some(slot) = slot else {
            self.health.skipped.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                event = "observation_skipped",
                request_id = id,
                code = "observer_capacity"
            );
            return Err(ticket);
        };
        let (sender, receiver) = mpsc::channel(REQUEST_QUEUE);
        let (request_sender, request_receiver) = mpsc::channel(REQUEST_QUEUE);
        // Native Gemini request model/stream metadata is already known from the URL.
        let request_sender = (!protocol.is_gemini()).then_some(request_sender);
        let (headers_tx, headers_rx) = oneshot::channel();
        let (completion_tx, completion_rx) = oneshot::channel();
        let state = Arc::new(AtomicU8::new(0));
        let accepting = Arc::new(AtomicBool::new(true));
        self.health.active.fetch_add(1, Ordering::Relaxed);
        let active = Active {
            _slot: slot,
            health: self.health.clone(),
            idle: self.idle.clone(),
        };
        let worker_state = state.clone();
        let worker_accepting = accepting.clone();
        let health = self.health.clone();
        let settings = settings.clone();
        let memory = self.bytes.clone();
        let request_id = id.to_string();
        let panic_health = self.health.clone();
        self.runtime.spawn(async move {
            use futures_util::FutureExt;
            let future = async move {
                let _active = active;
                let mut observer = Observer::new(protocol, &settings, secrets);
                let response = async {
                    if let Ok(headers) = headers_rx.await {
                        let headers: Headers = headers;
                        observer.headers(&headers.media, true);
                        observe(receiver, &headers, &settings, &mut observer, memory).await;
                    }
                    worker_accepting.store(false, Ordering::Relaxed);
                };
                let request = request_metadata(request_receiver, settings.max_request_json_bytes, &request_id);
                let (_, request) = tokio::join!(response, request);
                if let Ok(Completion { mut row, transport_complete }) = completion_rx.await {
                    if let Some((model, stream)) = request {
                        if row["requested_model"].is_null() {
                            row["requested_model"] = json!(model);
                            row["resolved_model"] = row["requested_model"].clone();
                        }
                        if row["streaming"].is_null() {
                            row["streaming"] = json!(stream);
                        }
                    }
                    let reason = match worker_state.load(Ordering::Relaxed) {
                        QUEUE_FULL => Some("observer_queue_full"),
                        WORKER_FAILED => Some("observer_worker_unavailable"),
                        _ => None,
                    };
                    if let Some(reason) = reason {
                        observer.degrade(reason);
                    }
                    let observation = observer.finish(transport_complete);
                    let id = row["request_id"].as_str().unwrap_or("").to_owned();
                    if observation.observer_error.is_some() {
                        health.failures.fetch_add(1, Ordering::Relaxed);
                        tracing::warn!(
                            event = "observation_failed", request_id = %id,
                            code = observation.observer_error.as_deref(),
                            observer_status = %observation.observer_status
                        );
                    }
                    let business_error = observation.error_source.is_some();
                    let incomplete = matches!(observation.response_status.as_deref(), Some("incomplete" | "cancelled"));
                    if row["outcome"] == "completed" {
                        if business_error {
                            row["outcome"] = json!("failed");
                        } else if incomplete {
                            row["outcome"] = json!("incomplete");
                        }
                    }
                    let mut fields = serde_json::to_value(&observation)
                        .expect("observation serializes").as_object()
                        .expect("observation object").clone();
                    // Transport/local errors take precedence over an observed business error.
                    for key in ["error_source", "error_code", "error_message"] {
                        if row.get(key).is_some_and(|v| !v.is_null()) {
                            fields.remove(key);
                        }
                    }
                    row.as_object_mut().expect("audit object").extend(fields);
                    if business_error || incomplete {
                        tracing::warn!(
                            event = "observation_result", request_id = %id,
                            outcome = row["outcome"].as_str(),
                            error_code = observation.error_code.as_deref(),
                            response_status = observation.response_status.as_deref()
                        );
                    } else {
                        tracing::debug!(event = "observation_completed", request_id = %id, usage_status = %observation.usage_status);
                    }
                    ticket.finish(row);
                }
            };
            if std::panic::AssertUnwindSafe(future).catch_unwind().await.is_err() {
                panic_health.failures.fetch_add(1, Ordering::Relaxed);
                tracing::error!(event = "observation_task_panicked");
            }
        });
        Ok(Session {
            sender: Some(sender),
            request_sender,
            headers: Some(headers_tx),
            completion: Some(completion_tx),
            state,
            accepting,
            pool: self.clone(),
            id: id.into(),
        })
    }
}
impl Session {
    pub fn headers(&mut self, media: &str, encoding: &str) {
        if let Some(sender) = self.headers.take() {
            let _ = sender.send(Headers {
                media: media.into(),
                encoding: encoding.into(),
            });
        }
    }
    pub fn bytes(&mut self, bytes: &[u8]) {
        if !self.accepting.load(Ordering::Relaxed) || self.state.load(Ordering::Relaxed) != 0 {
            return;
        }
        for part in bytes.chunks(CHUNK_BYTES) {
            let budget = match self
                .pool
                .bytes
                .clone()
                .try_acquire_many_owned(part.len() as u32)
            {
                Ok(budget) => budget,
                Err(_) => {
                    self.stop(QUEUE_FULL);
                    return;
                }
            };
            let chunk = Chunk {
                bytes: Bytes::copy_from_slice(part),
                _budget: budget,
            };
            match self
                .sender
                .as_ref()
                .expect("unfinished observer")
                .try_send(chunk)
            {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => {
                    self.stop(QUEUE_FULL);
                    return;
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    if self.accepting.load(Ordering::Relaxed) {
                        self.stop(WORKER_FAILED);
                    }
                    return;
                }
            }
        }
    }
    pub fn request_bytes(&mut self, bytes: &[u8]) {
        for part in bytes.chunks(CHUNK_BYTES) {
            let Some(sender) = &self.request_sender else {
                return;
            };
            let budget = match self
                .pool
                .bytes
                .clone()
                .try_acquire_many_owned(part.len() as u32)
            {
                Ok(budget) => budget,
                Err(_) => {
                    self.request_sender.take();
                    tracing::warn!(event="request_observation_skipped",request_id=%self.id,code="request_queue_full");
                    return;
                }
            };
            let chunk = Chunk {
                bytes: Bytes::copy_from_slice(part),
                _budget: budget,
            };
            if sender.try_send(chunk).is_err() {
                self.request_sender.take();
                tracing::warn!(event="request_observation_skipped",request_id=%self.id,code="request_queue_unavailable");
                return;
            }
        }
    }
    pub fn request_complete(&mut self) {
        self.request_sender.take();
    }
    fn stop(&mut self, reason: u8) {
        self.state.store(reason, Ordering::Relaxed);
        self.accepting.store(false, Ordering::Relaxed);
        self.sender.take();
    }
    pub fn finish(mut self, row: Value, transport_complete: bool) {
        self.request_sender.take();
        self.sender.take();
        self.headers.take();
        if let Some(sender) = self.completion.take() {
            let _ = sender.send(Completion {
                row,
                transport_complete,
            });
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.request_sender.take();
        self.sender.take();
        self.headers.take();
        // If completion was lost, the worker drops its audit ticket, which records interruption.
        self.completion.take();
    }
}

async fn request_metadata(
    mut receiver: mpsc::Receiver<Chunk>,
    limit: usize,
    id: &str,
) -> Option<(String, bool)> {
    let mut body = Vec::new();
    let mut budget: Option<OwnedSemaphorePermit> = None;
    while let Some(chunk) = receiver.recv().await {
        if body.len().saturating_add(chunk.bytes.len()) > limit {
            tracing::warn!(
                event = "request_observation_skipped",
                request_id = id,
                code = "request_json_limit"
            );
            return None;
        }
        body.extend_from_slice(&chunk.bytes);
        if let Some(budget) = &mut budget {
            budget.merge(chunk._budget);
        } else {
            budget = Some(chunk._budget);
        }
    }
    let value: Value = serde_json::from_slice(&body).ok()?;
    let model = value.get("model")?.as_str()?.to_string();
    if !model_name(&model) {
        return None;
    }
    Some((
        model,
        value
            .get("stream")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    ))
}

type Reader = Box<dyn AsyncBufRead + Send + Unpin>;
fn decoded_reader(mut reader: Reader, encoding: &str) -> Result<Reader, &'static str> {
    let encodings = encoding
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if encodings.len() > 3 {
        return Err("content_encoding_depth");
    }
    for coding in encodings.iter().rev() {
        reader = match coding.to_ascii_lowercase().as_str() {
            "identity" => reader,
            "gzip" | "x-gzip" => {
                let mut decoder = GzipDecoder::new(reader);
                decoder.multiple_members(true);
                Box::new(BufReader::new(decoder))
            }
            // HTTP deflate denotes a zlib-wrapped stream, not raw DEFLATE.
            "deflate" => Box::new(BufReader::new(ZlibDecoder::new(reader))),
            "br" => Box::new(BufReader::new(BrotliDecoder::new(reader))),
            "zstd" => {
                let params =
                    [async_compression::codecs::zstd::params::DParameter::window_log_max(23)];
                let mut decoder = ZstdDecoder::with_params(reader, &params);
                decoder.multiple_members(true);
                Box::new(BufReader::new(decoder))
            }
            _ => return Err("unsupported_content_encoding"),
        };
    }
    Ok(reader)
}
async fn observe(
    mut receiver: mpsc::Receiver<Chunk>,
    headers: &Headers,
    settings: &Audit,
    observer: &mut Observer,
    memory: Arc<Semaphore>,
) {
    let stream = async_stream::stream! {
        while let Some(chunk)=receiver.recv().await {
            // Hold the byte budget until StreamReader has consumed this chunk.
            let _budget=chunk._budget;
            yield Ok::<_,io::Error>(chunk.bytes);
        }
    };
    let source: Reader = Box::new(StreamReader::new(Box::pin(stream)));
    let mut reader = match decoded_reader(source, &headers.encoding) {
        Ok(reader) => reader,
        Err(code) => {
            observer.unsupported(code);
            return;
        }
    };
    let mut buffer = [0u8; CHUNK_BYTES];
    let mut decoded = 0usize;
    let mut retained = memory
        .clone()
        .try_acquire_many_owned(0)
        .expect("zero-sized budget");
    loop {
        match reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(count) => {
                decoded = decoded.saturating_add(count);
                if decoded > settings.max_decoded_response_bytes {
                    observer.degrade("decoded_response_limit");
                    break;
                }
                let budget = match memory.clone().try_acquire_many_owned(count as u32) {
                    Ok(budget) => budget,
                    Err(_) => {
                        observer.degrade("observer_memory_limit");
                        break;
                    }
                };
                retained.merge(budget);
                observer.bytes(&buffer[..count]);
                let needed = observer.buffered_bytes();
                if retained.num_permits() > needed {
                    drop(retained.split(retained.num_permits() - needed));
                }
                if observer.stopped() {
                    break;
                }
                // Prevent a highly compressed response from monopolizing either observation worker.
                tokio::task::yield_now().await;
            }
            Err(_) => {
                observer.degrade("response_decode_failed");
                break;
            }
        }
    }
}
impl Guard {
    pub async fn shutdown(self) {
        self.pool.stopped.store(true, Ordering::Relaxed);
        let _ = self.stop.send(());
        if !matches!(
            tokio::task::spawn_blocking(move || self.worker.join()).await,
            Ok(Ok(()))
        ) {
            tracing::error!(event = "observation_runtime_panicked");
        }
    }
}
pub fn initialize(settings: &Audit) -> Result<(Arc<Pool>, Guard), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("transit-observe")
        .enable_all()
        .build()
        .map_err(|_| "cannot initialize observation runtime")?;
    let (stop, stopped) = oneshot::channel();
    let pool = Arc::new(Pool {
        runtime: runtime.handle().clone(),
        slots: Arc::new(Semaphore::new(settings.max_pending_observations)),
        bytes: Arc::new(Semaphore::new(settings.observer_queue_bytes)),
        stopped: AtomicBool::new(false),
        health: Arc::new(Health::default()),
        idle: Arc::new(Notify::new()),
    });
    let worker_pool = pool.clone();
    let worker = thread::Builder::new()
        .name("transit-observe-supervisor".into())
        .spawn(move || {
            runtime.block_on(async move {
                let _ = stopped.await;
                let drain = async {
                    while worker_pool.health.active.load(Ordering::Relaxed) != 0 {
                        worker_pool.idle.notified().await;
                    }
                };
                if tokio::time::timeout(Duration::from_secs(10), drain)
                    .await
                    .is_err()
                {
                    tracing::warn!(
                        event = "observation_shutdown_deadline",
                        pending = worker_pool.health.active.load(Ordering::Relaxed)
                    );
                }
            });
            runtime.shutdown_timeout(Duration::from_secs(2));
        })
        .map_err(|_| "cannot start observation runtime")?;
    Ok((pool.clone(), Guard { pool, stop, worker }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit;
    use tokio::io::AsyncRead;

    const CHAT: &[u8] = b"data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}}\n\ndata: [DONE]\n\n";
    async fn encode(bytes: &[u8], encoding: &str) -> Vec<u8> {
        use async_compression::tokio::bufread::{
            BrotliEncoder, GzipEncoder, ZlibEncoder, ZstdEncoder,
        };
        let mut reader: Box<dyn AsyncRead + Send + Unpin + '_> = match encoding {
            "gzip" => Box::new(GzipEncoder::new(bytes)),
            "deflate" => Box::new(ZlibEncoder::new(bytes)),
            "br" => Box::new(BrotliEncoder::new(bytes)),
            "zstd" => Box::new(ZstdEncoder::new(bytes)),
            _ => Box::new(bytes),
        };
        let mut output = Vec::new();
        reader.read_to_end(&mut output).await.unwrap();
        output
    }
    fn settings() -> Audit {
        Audit {
            enabled: true,
            database: Some(
                std::env::temp_dir()
                    .join(format!("transit-observation-{}", uuid::Uuid::new_v4()))
                    .join("requests.sqlite3"),
            ),
            ..Default::default()
        }
    }
    fn row(id: &str, finished: bool) -> Value {
        json!({"request_id":id,"started_at_ms":audit::now_ms(),"finished_at_ms":if finished {Some(audit::now_ms())} else {None},"outcome":if finished {"completed"} else {"in_progress"}})
    }
    fn result(settings: &Audit, id: &str) -> Value {
        let db = rusqlite::Connection::open(settings.database.as_ref().unwrap()).unwrap();
        db.query_row("select observer_status,observer_error,usage_status,total_tokens,requested_model,streaming,outcome from requests where request_id=?1",[id],|r| Ok(json!({"observer_status":r.get::<_,Option<String>>(0)?,"observer_error":r.get::<_,Option<String>>(1)?,"usage_status":r.get::<_,Option<String>>(2)?,"total_tokens":r.get::<_,Option<i64>>(3)?,"requested_model":r.get::<_,Option<String>>(4)?,"streaming":r.get::<_,Option<bool>>(5)?,"outcome":r.get::<_,String>(6)?}))).unwrap()
    }
    fn clean(settings: &Audit) {
        std::fs::remove_dir_all(settings.database.as_ref().unwrap().parent().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn compressed_and_mislabelled_responses_are_observed_off_thread() {
        let settings = settings();
        let (audit, audit_guard) = audit::open(&settings, 16).unwrap().unwrap();
        let (pool, guard) = initialize(&settings).unwrap();
        for encoding in ["identity", "gzip", "deflate", "br", "zstd", "gzip, br"] {
            let wire = if encoding == "gzip, br" {
                encode(&encode(CHAT, "gzip").await, "br").await
            } else {
                encode(CHAT, encoding).await
            };
            let ticket = audit.begin(row(encoding, false)).unwrap();
            let mut session = pool
                .begin(encoding, Protocol::OpenaiChat, &settings, vec![], ticket)
                .ok()
                .unwrap();
            session.request_bytes(b"{\"model\":\"fixture-model\",\"stream\":true}");
            session.headers("text/plain", encoding);
            for part in wire.chunks(13) {
                session.bytes(part);
                tokio::task::yield_now().await;
            }
            session.finish(row(encoding, true), true);
        }
        guard.shutdown().await;
        audit_guard.shutdown().await;
        for encoding in ["identity", "gzip", "deflate", "br", "zstd", "gzip, br"] {
            let r = result(&settings, encoding);
            assert_eq!(r["usage_status"], "complete", "{encoding}: {r}");
            assert_eq!(r["total_tokens"], 12);
            assert_eq!(r["requested_model"], "fixture-model");
            assert_eq!(r["streaming"], true);
        }
        clean(&settings);
    }
    #[tokio::test]
    async fn queue_capacity_and_memory_pressure_preserve_forwarding_outcomes() {
        let mut settings = settings();
        settings.max_pending_observations = 1;
        let (audit, audit_guard) = audit::open(&settings, 4).unwrap().unwrap();
        let (pool, guard) = initialize(&settings).unwrap();
        let mut session = pool
            .begin(
                "busy",
                Protocol::OpenaiChat,
                &settings,
                vec![],
                audit.begin(row("busy", false)).unwrap(),
            )
            .ok()
            .unwrap();
        let rejected = pool.begin(
            "overflow",
            Protocol::OpenaiChat,
            &settings,
            vec![],
            audit.begin(row("overflow", false)).unwrap(),
        );
        assert!(rejected.is_err());
        rejected.err().unwrap().finish(row("overflow", true));
        let budget = pool
            .bytes
            .clone()
            .try_acquire_many_owned(settings.observer_queue_bytes as u32)
            .unwrap();
        session.headers("text/event-stream", "");
        session.bytes(CHAT);
        session.finish(row("busy", true), true);
        drop(budget);
        guard.shutdown().await;
        audit_guard.shutdown().await;
        let r = result(&settings, "busy");
        assert_eq!(r["observer_error"], "observer_queue_full");
        assert_eq!(r["usage_status"], "missing");
        assert_eq!(r["outcome"], "completed");
        assert_eq!(pool.health.skipped.load(Ordering::Relaxed), 1);
        clean(&settings);
    }
    #[tokio::test]
    async fn corrupt_unknown_and_excessive_compression_only_degrades_statistics() {
        let mut settings = settings();
        settings.max_decoded_response_bytes = 64;
        let (audit, audit_guard) = audit::open(&settings, 8).unwrap().unwrap();
        let (pool, guard) = initialize(&settings).unwrap();
        for (id, encoding, wire) in [
            ("corrupt", "gzip", b"broken gzip".to_vec()),
            ("unknown", "vendor", CHAT.to_vec()),
            ("limit", "gzip", encode(CHAT, "gzip").await),
        ] {
            let mut session = pool
                .begin(
                    id,
                    Protocol::OpenaiChat,
                    &settings,
                    vec![],
                    audit.begin(row(id, false)).unwrap(),
                )
                .ok()
                .unwrap();
            session.headers("text/event-stream", encoding);
            session.bytes(&wire);
            session.finish(row(id, true), true);
        }
        guard.shutdown().await;
        audit_guard.shutdown().await;
        for (id, code) in [
            ("corrupt", "response_decode_failed"),
            ("unknown", "unsupported_content_encoding"),
            ("limit", "decoded_response_limit"),
        ] {
            let r = result(&settings, id);
            assert_eq!(r["observer_error"], code, "{r}");
            assert_eq!(r["outcome"], "completed");
            assert_ne!(r["usage_status"], "complete");
        }
        clean(&settings);
    }
    #[tokio::test]
    #[ignore = "requires private capture files via TRANSIT_REPLAY_DIRECTORY"]
    async fn private_capture_replay() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        let source = std::path::PathBuf::from(
            std::env::var_os("TRANSIT_REPLAY_DIRECTORY").expect("capture directory required"),
        );
        let settings = settings();
        let (audit, audit_guard) = audit::open(&settings, 32).unwrap().unwrap();
        let (pool, guard) = initialize(&settings).unwrap();
        let mut ids = Vec::new();
        for entry in std::fs::read_dir(source).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            let events = std::fs::read_to_string(path)
                .unwrap()
                .lines()
                .map(|s| serde_json::from_str::<Value>(s).unwrap())
                .collect::<Vec<_>>();
            let start = &events[0]["metadata"];
            let end = events
                .iter()
                .find(|e| e["type"] == "request_finished")
                .unwrap();
            assert_eq!(end["capture_complete"], true);
            let id = start["request_id"].as_str().unwrap();
            let protocol: Protocol =
                serde_json::from_value(start["upstream_protocol"].clone()).unwrap();
            let headers = events
                .iter()
                .find(|e| e["type"] == "response_headers")
                .unwrap();
            let media = headers["headers"]["content-type"][0].as_str().unwrap_or("");
            let encoding = headers["headers"]["content-encoding"][0]
                .as_str()
                .unwrap_or("");
            let mut session = pool
                .begin(
                    id,
                    protocol,
                    &settings,
                    vec![],
                    audit.begin(row(id, false)).unwrap(),
                )
                .ok()
                .unwrap();
            session.headers(media, encoding);
            let wire = events
                .iter()
                .filter(|e| e["type"] == "body_chunk" && e["direction"] == "response")
                .flat_map(|e| STANDARD.decode(e["data_base64"].as_str().unwrap()).unwrap())
                .collect::<Vec<_>>();
            for part in wire.chunks(CHUNK_BYTES) {
                session.bytes(part);
                tokio::task::yield_now().await;
            }
            session.finish(row(id, true), true);
            ids.push(id.to_string());
        }
        guard.shutdown().await;
        audit_guard.shutdown().await;
        let mut tokens = 0;
        for id in &ids {
            let r = result(&settings, id);
            assert_eq!(r["usage_status"], "complete", "{id}: {r}");
            tokens += r["total_tokens"].as_i64().unwrap();
        }
        if let Ok(expected) = std::env::var("TRANSIT_REPLAY_EXPECTED_TOKENS") {
            assert_eq!(tokens, expected.parse::<i64>().unwrap());
        }
        assert!(!ids.is_empty());
        println!(
            "replayed {} captures; total_tokens={tokens}; all usage complete",
            ids.len()
        );
        clean(&settings);
    }
}
