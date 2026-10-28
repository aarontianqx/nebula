use crate::{
    adapter::{self, Sse, StreamOutput},
    config::{Server, Snapshot},
    error::ProxyError,
    logging::LogState,
};
use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, Method, Request, Response, StatusCode, Uri, header},
    response::IntoResponse,
};
use bytes::{Bytes, BytesMut};
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper_rustls::HttpsConnector;
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use serde_json::{Value, json};
use std::{
    error::Error as StdError,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

type HttpClient = Client<HttpsConnector<HttpConnector>, Body>;
pub struct App {
    pub snapshot: RwLock<Arc<Snapshot>>,
    pub logs: Arc<LogState>,
    pub stopping: CancellationToken,
    pub force_stop: CancellationToken,
    pub semaphore: Arc<Semaphore>,
    pub active: AtomicU64,
    client: HttpClient,
}
impl App {
    pub fn new(snapshot: Snapshot, logs: Arc<LogState>) -> Arc<Self> {
        let mut connector = HttpConnector::new();
        connector.enforce_http(false);
        connector.set_connect_timeout(Some(Duration::from_secs(
            snapshot.config.server.connect_timeout_seconds,
        )));
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .wrap_connector(connector);
        let mut builder = Client::builder(TokioExecutor::new());
        builder
            .retry_canceled_requests(false)
            .pool_max_idle_per_host(32)
            .pool_idle_timeout(Duration::from_secs(60));
        let client = builder.build(https);
        let max = snapshot.config.server.max_in_flight;
        Arc::new(Self {
            snapshot: RwLock::new(Arc::new(snapshot)),
            logs,
            stopping: CancellationToken::new(),
            force_stop: CancellationToken::new(),
            semaphore: Arc::new(Semaphore::new(max)),
            active: AtomicU64::new(0),
            client,
        })
    }
    pub fn router(self: &Arc<Self>) -> Router {
        Router::new()
            .route("/_transit/health/live", axum::routing::get(live))
            .route("/_transit/health/ready", axum::routing::get(ready))
            .fallback(proxy)
            .with_state(self.clone())
    }
}
async fn live(State(app): State<Arc<App>>) -> impl IntoResponse {
    Json(
        json!({"status":"alive","logging_degraded":app.logs.health.degraded.load(Ordering::Relaxed),"log_write_failures":app.logs.health.failures.load(Ordering::Relaxed),"log_events_dropped":app.logs.dropped.dropped_lines()}),
    )
}
async fn ready(State(app): State<Arc<App>>) -> impl IntoResponse {
    let snapshot = app
        .snapshot
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let status = if app.stopping.is_cancelled() {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };
    (
        status,
        Json(
            json!({"ready":status==StatusCode::OK,"config_generation":snapshot.generation,"active_requests":app.active.load(Ordering::Relaxed),"logging_degraded":app.logs.health.degraded.load(Ordering::Relaxed)}),
        ),
    )
}

struct Progress {
    status: Option<u16>,
    upstream_status: Option<u16>,
    headers_ms: Option<u64>,
    first_byte_ms: Option<u64>,
    stage: &'static str,
    last_activity: Instant,
}
struct Record {
    id: String,
    provider: String,
    rule: String,
    generation: u64,
    method: String,
    adapter: bool,
    requested_model: Mutex<Option<(String, String)>>,
    started: Instant,
    progress: Mutex<Progress>,
    upload_error: Mutex<Option<ProxyError>>,
    request_bytes: AtomicU64,
    response_bytes: AtomicU64,
    finished: AtomicBool,
}
impl Record {
    fn finish(&self, outcome: &str, error: Option<&ProxyError>) {
        if self.finished.swap(true, Ordering::Relaxed) {
            return;
        }
        let progress = self.progress.lock().unwrap_or_else(|e| e.into_inner());
        let models = self
            .requested_model
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let requested = models.as_ref().map(|m| m.0.as_str());
        let resolved = models.as_ref().map(|m| m.1.as_str());
        let code = error.map(|e| e.code).unwrap_or("");
        let stage = error.map(|e| e.stage).unwrap_or(progress.stage);
        let message = error.map(|e| e.message.as_str()).unwrap_or("");
        macro_rules! event {($level:ident)=>{tracing::$level!(event="request_finished",request_id=%self.id,config_generation=self.generation,provider=%self.provider,rule_id=%self.rule,method=%self.method,adapter=self.adapter,requested_model=requested,resolved_model=resolved,status=progress.status,upstream_status=progress.upstream_status,outcome,error_code=code,stage,message,duration_ms=self.started.elapsed().as_millis() as u64,response_headers_ms=progress.headers_ms,first_body_byte_ms=progress.first_byte_ms,request_bytes=self.request_bytes.load(Ordering::Relaxed),response_bytes=self.response_bytes.load(Ordering::Relaxed),idle_ms=progress.last_activity.elapsed().as_millis() as u64)}}
        if error.is_some_and(|e| e.status.is_server_error())
            || progress.upstream_status.is_some_and(|s| s >= 500)
        {
            event!(error);
        } else if outcome == "cancelled" && !progress.upstream_status.is_some_and(|s| s >= 400) {
            event!(debug);
        } else if error.is_some() || progress.upstream_status.is_some_and(|s| s >= 400) {
            event!(warn);
        } else {
            event!(info);
        }
    }
    fn stage(&self, stage: &'static str) {
        let mut p = self.progress.lock().unwrap_or_else(|e| e.into_inner());
        p.stage = stage;
        p.last_activity = Instant::now();
    }
    fn bytes(&self, count: usize) {
        self.response_bytes
            .fetch_add(count as u64, Ordering::Relaxed);
        let mut p = self.progress.lock().unwrap_or_else(|e| e.into_inner());
        p.last_activity = Instant::now();
        if p.first_byte_ms.is_none() {
            p.first_byte_ms = Some(self.started.elapsed().as_millis() as u64);
        }
    }
}
struct Lifetime {
    record: Arc<Record>,
    cancel: CancellationToken,
    app: Arc<App>,
    _permit: OwnedSemaphorePermit,
}
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.cancel.cancel();
        if !self.record.finished.load(Ordering::Relaxed) {
            let error = if self.app.force_stop.is_cancelled() {
                ProxyError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "shutdown_deadline",
                    "shutdown",
                    "Request terminated at shutdown deadline",
                )
            } else {
                ProxyError::new(
                    StatusCode::BAD_REQUEST,
                    "client_cancelled",
                    "downstream",
                    "Client disconnected before response completed",
                )
            };
            self.record.finish(
                if self.app.force_stop.is_cancelled() {
                    "failed"
                } else {
                    "cancelled"
                },
                Some(&error),
            );
        }
        self.app.active.fetch_sub(1, Ordering::Relaxed);
    }
}
async fn proxy(State(app): State<Arc<App>>, request: Request<Body>) -> Response<Body> {
    let id = uuid::Uuid::new_v4().to_string();
    let permit = match app.semaphore.clone().try_acquire_owned() {
        Ok(p) if !app.stopping.is_cancelled() => p,
        _ => {
            tracing::warn!(event="request_rejected",request_id=%id,error_code="capacity_or_draining");
            return identify(
                ProxyError::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "capacity_or_draining",
                    "admission",
                    "Service is draining or at capacity",
                )
                .into_response(),
                &id,
            );
        }
    };
    let snapshot = app
        .snapshot
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    let uri = request.uri();
    if uri.scheme().is_some()
        || uri.authority().is_some()
        || !uri.path().starts_with('/')
        || uri.path().contains('\\')
    {
        tracing::warn!(event="request_rejected",request_id=%id,error_code="invalid_request_target");
        return identify(
            ProxyError::bad(
                "invalid_request_target",
                "Expected an origin-form HTTP path",
            )
            .into_response(),
            &id,
        );
    }
    let plan = snapshot.config.plan(request.method(), request.uri().path());
    let record = Arc::new(Record {
        id: id.clone(),
        provider: plan.provider_name.into(),
        rule: plan
            .route
            .map(|r| r.id.clone())
            .unwrap_or_else(|| "default".into()),
        generation: snapshot.generation,
        method: request.method().to_string(),
        adapter: plan.route.is_some_and(|r| r.adapter.is_some()),
        requested_model: Mutex::new(None),
        started: Instant::now(),
        progress: Mutex::new(Progress {
            status: None,
            upstream_status: None,
            headers_ms: None,
            first_byte_ms: None,
            stage: "routing",
            last_activity: Instant::now(),
        }),
        upload_error: Mutex::new(None),
        request_bytes: AtomicU64::new(0),
        response_bytes: AtomicU64::new(0),
        finished: AtomicBool::new(false),
    });
    app.active.fetch_add(1, Ordering::Relaxed);
    let lifetime = Lifetime {
        record: record.clone(),
        cancel: CancellationToken::new(),
        app: app.clone(),
        _permit: permit,
    };
    tracing::debug!(event="request_started",request_id=%id,provider=%record.provider,rule_id=%record.rule,config_generation=snapshot.generation,request_profile=plan.route.and_then(|route|route.request_profile.as_deref()));
    let result = execute(app, &snapshot, request, lifetime).await;
    match result {
        Ok(response) => identify(response, &id),
        Err(error) => {
            record
                .progress
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .status = Some(error.status.as_u16());
            record.finish("failed", Some(&error));
            identify(error.into_response(), &id)
        }
    }
}
fn identify(mut response: Response<Body>, id: &str) -> Response<Body> {
    if let Ok(value) = HeaderValue::from_str(id) {
        response.headers_mut().insert("x-transit-request-id", value);
    }
    response
}

async fn execute(
    app: Arc<App>,
    snapshot: &Snapshot,
    request: Request<Body>,
    lifetime: Lifetime,
) -> Result<Response<Body>, ProxyError> {
    let record = lifetime.record.clone();
    let server = &snapshot.config.server;
    let (mut parts, body) = request.into_parts();
    if parts.method == Method::CONNECT || parts.headers.contains_key(header::UPGRADE) {
        return fail(
            lifetime,
            ProxyError::bad(
                "unsupported_transport",
                "CONNECT and Upgrade are not supported",
            ),
        );
    }
    if parts.headers.contains_key(header::TRANSFER_ENCODING)
        && parts.headers.contains_key(header::CONTENT_LENGTH)
    {
        return fail(
            lifetime,
            ProxyError::bad("ambiguous_framing", "Conflicting body framing headers"),
        );
    }
    let plan = snapshot.config.plan(&parts.method, parts.uri.path());
    let adapter_route = plan.route.filter(|r| r.adapter.is_some());
    let profile = plan
        .route
        .and_then(|route| route.request_profile.as_ref())
        .and_then(|name| plan.provider.request_profiles.get(name));
    let rewrite = adapter_route.is_some()
        || profile.is_some()
        || (!plan.provider.model_aliases.is_empty()
            && parts.method == Method::POST
            && matches!(
                plan.logical_path.as_str(),
                "/v1/chat/completions" | "/v1/completions" | "/v1/responses" | "/v1/messages"
            ));
    let credentials = credentials(&parts.headers);
    let cancel = lifetime.cancel.clone();
    let mut streaming = false;
    let outbound_body = if rewrite {
        if let Err(error) = require_json(&parts.headers) {
            return fail(lifetime, error);
        }
        let limit = adapter_route
            .map_or(server.max_rewrite_bytes, |r| r.max_request_bytes)
            .min(server.max_upload_bytes);
        let raw = match collect_body(
            body,
            limit,
            server.idle_timeout(),
            "request_decode",
            &cancel,
            &app.force_stop,
        )
        .await
        {
            Ok(b) => b,
            Err(e) => return fail(lifetime, e),
        };
        record
            .request_bytes
            .store(raw.len() as u64, Ordering::Relaxed);
        let mut value: Value = match serde_json::from_slice(&raw) {
            Ok(v) => v,
            Err(_) => {
                return fail(
                    lifetime,
                    ProxyError::bad("invalid_json", "Request body is not valid JSON"),
                );
            }
        };
        let models = match adapter::rewrite_model(&mut value, &plan.provider.model_aliases) {
            Ok(m) => m,
            Err(e) => return fail(lifetime, e),
        };
        let mut changed = models
            .as_ref()
            .is_some_and(|(requested, resolved)| requested != resolved);
        *record
            .requested_model
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = models;
        if let Some(route) = adapter_route {
            let Some(prompt_mode) = route.prompt_mode else {
                return fail(
                    lifetime,
                    ProxyError::bad("invalid_adapter_config", "Missing prompt_mode"),
                );
            };
            let (mut converted, is_streaming) = match adapter::request(value, prompt_mode) {
                Ok(v) => v,
                Err(e) => return fail(lifetime, e),
            };
            if let Some(profile) = profile
                && let Err(error) = adapter::apply_profile(&mut converted, profile)
            {
                return fail(lifetime, error);
            }
            streaming = is_streaming;
            let bytes = match serde_json::to_vec(&converted) {
                Ok(bytes) => bytes,
                Err(_) => {
                    return fail(
                        lifetime,
                        ProxyError::bad("invalid_json", "Cannot serialize request"),
                    );
                }
            };
            representation_changed(&mut parts.headers);
            parts.headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            parts.headers.insert(
                header::ACCEPT,
                HeaderValue::from_static(if streaming {
                    "text/event-stream"
                } else {
                    "application/json"
                }),
            );
            parts.headers.insert(
                header::ACCEPT_ENCODING,
                HeaderValue::from_static("identity"),
            );
            Body::from(bytes)
        } else {
            if let Some(profile) = profile {
                match adapter::apply_profile(&mut value, profile) {
                    Ok(applied) => changed |= applied,
                    Err(error) => return fail(lifetime, error),
                }
            }
            if changed {
                representation_changed(&mut parts.headers);
                let bytes = match serde_json::to_vec(&value) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        return fail(
                            lifetime,
                            ProxyError::bad("invalid_json", "Cannot serialize request"),
                        );
                    }
                };
                Body::from(bytes)
            } else {
                Body::from(raw)
            }
        }
    } else {
        let request_record = record.clone();
        let upload_cancel = cancel.clone();
        let force = app.force_stop.clone();
        let limit = server.max_upload_bytes;
        let idle = server.idle_timeout();
        if parts
            .headers
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok())
            .is_some_and(|n| n > limit)
        {
            return fail(
                lifetime,
                ProxyError::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "upload_too_large",
                    "request_upload",
                    "Request exceeds upload limit",
                ),
            );
        }
        let mut source = body.into_data_stream();
        body_from_stream(async_stream::try_stream! {
            use futures_util::StreamExt;
            let mut total=0usize;
            loop {
                let next=tokio::select!{_ = upload_cancel.cancelled()=>break,_=force.cancelled()=>break,result=tokio::time::timeout(idle,source.next())=>result};
                let next=match next {
                    Ok(next)=>next,
                    Err(_)=>{*request_record.upload_error.lock().unwrap_or_else(|e|e.into_inner())=Some(ProxyError::timeout("request_upload",idle.as_secs())); Err(ProxyError::timeout("request_upload",idle.as_secs()))?; unreachable!()}
                };
                let Some(chunk)=next else{break};
                let chunk=match chunk {
                    Ok(chunk)=>chunk,
                    Err(_)=>{*request_record.upload_error.lock().unwrap_or_else(|e|e.into_inner())=Some(ProxyError::bad("request_body_failed","Client request body failed"));Err(ProxyError::bad("request_body_failed","Client request body failed"))?;unreachable!()}
                };
                total=total.saturating_add(chunk.len());
                if total>limit {
                    *request_record.upload_error.lock().unwrap_or_else(|e|e.into_inner())=Some(ProxyError::new(StatusCode::PAYLOAD_TOO_LARGE,"upload_too_large","request_upload","Request exceeds upload limit"));
                    Err(ProxyError::new(StatusCode::PAYLOAD_TOO_LARGE,"upload_too_large","request_upload","Request exceeds upload limit"))?;
                }
                request_record.request_bytes.fetch_add(chunk.len() as u64,Ordering::Relaxed);
                yield chunk;
            }
        })
    };
    record.stage("connect_or_response_headers");
    let target_kind = match plan.upstream_path.as_str() {
        path if path.ends_with("/chat/completions") => "chat_completions",
        path if path.ends_with("/completions") => "completions",
        path if path.ends_with("/messages") => "messages",
        path if path.ends_with("/responses") => "responses",
        _ => "other",
    };
    tracing::debug!(event="upstream_started",request_id=%record.id,endpoint_kind=target_kind,model_alias_applied=record.requested_model.lock().unwrap_or_else(|e|e.into_inner()).as_ref().is_some_and(|(a,b)|a!=b));
    let query = parts
        .uri
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let target = format!(
        "{}{}{}",
        plan.provider.origin.trim_end_matches('/'),
        plan.upstream_path,
        query
    );
    let uri: Uri = match target.parse() {
        Ok(u) => u,
        Err(_) => {
            return fail(
                lifetime,
                ProxyError::bad("invalid_upstream_path", "Cannot construct upstream URL"),
            );
        }
    };
    filter_headers(&mut parts.headers, true);
    parts.uri = uri;
    parts.version = axum::http::Version::HTTP_11;
    let request = Request::from_parts(parts, outbound_body);
    let wait = Duration::from_secs(server.response_headers_timeout_seconds);
    let upstream = tokio::select! {
        _=app.force_stop.cancelled()=>return fail(lifetime,ProxyError::new(StatusCode::SERVICE_UNAVAILABLE,"shutdown_deadline","shutdown","Service shutdown deadline exceeded")),
        result=tokio::time::timeout(wait,app.client.request(request))=>match result{
            Err(_)=>return fail(lifetime,ProxyError::timeout("response_headers",wait.as_secs())),
            Ok(Err(error))=>{
                let upload_error=record.upload_error.lock().unwrap_or_else(|e|e.into_inner()).take();
                if let Some(error)=upload_error {return fail(lifetime,error)}
                let detail=transport_detail(&error,&credentials);
                let mut cause:Option<&(dyn StdError+'static)>=Some(&error);
                let mut timed_out=false;
                while let Some(source)=cause {
                    timed_out |= source.downcast_ref::<std::io::Error>().is_some_and(|e|e.kind()==std::io::ErrorKind::TimedOut) || source.downcast_ref::<hyper::Error>().is_some_and(hyper::Error::is_timeout);
                    cause=source.source();
                }
                return fail(lifetime,ProxyError::new(if timed_out{StatusCode::GATEWAY_TIMEOUT}else{StatusCode::BAD_GATEWAY},if timed_out{"upstream_timeout"}else{"upstream_transport_failed"},"connect_or_request",detail))
            },
            Ok(Ok(response))=>response,
        }
    };
    let (mut response_parts, incoming) = upstream.into_parts();
    {
        let mut p = record.progress.lock().unwrap_or_else(|e| e.into_inner());
        p.status = Some(response_parts.status.as_u16());
        p.upstream_status = p.status;
        p.headers_ms = Some(record.started.elapsed().as_millis() as u64);
        p.stage = "response_stream";
        p.last_activity = Instant::now();
    }
    tracing::debug!(event="upstream_headers",request_id=%record.id,status=response_parts.status.as_u16(),upstream_request_id=safe_header(&response_parts.headers,"x-request-id"));
    let mut preview = ErrorPreview::new(
        &record.id,
        response_parts.status,
        &response_parts.headers,
        app.logs.preview_limit(),
        credentials,
    );
    filter_headers(&mut response_parts.headers, false);
    if let Some(route) = adapter_route.filter(|_| response_parts.status.is_success()) {
        if !identity_encoded(&response_parts.headers) {
            return fail(
                lifetime,
                ProxyError::upstream(
                    "unsupported_content_encoding",
                    "response_headers",
                    "Adapter requires an uncompressed upstream response",
                ),
            );
        }
        let media = content_type(&response_parts.headers);
        if streaming && media != "text/event-stream"
            || !streaming && media != "application/json" && !media.ends_with("+json")
        {
            return fail(
                lifetime,
                ProxyError::upstream(
                    "unexpected_content_type",
                    "response_headers",
                    "Upstream returned an unexpected media type",
                ),
            );
        }
        representation_changed(&mut response_parts.headers);
        if !streaming {
            let raw = match collect_incoming(
                incoming,
                route.max_json_response_bytes,
                server,
                &cancel,
                &app.force_stop,
                &record,
            )
            .await
            {
                Ok(b) => b,
                Err(e) => return fail(lifetime, e),
            };
            let value: Value = match serde_json::from_slice(&raw) {
                Ok(v) => v,
                Err(_) => {
                    return fail(
                        lifetime,
                        ProxyError::upstream(
                            "invalid_upstream_json",
                            "adapter",
                            "Upstream response is not valid JSON",
                        ),
                    );
                }
            };
            let value = match adapter::response(&value) {
                Ok(v) => v,
                Err(e) => return fail(lifetime, e),
            };
            let bytes = match serde_json::to_vec(&value) {
                Ok(bytes) => bytes,
                Err(_) => {
                    return fail(
                        lifetime,
                        ProxyError::upstream(
                            "response_encoding_failed",
                            "adapter",
                            "Cannot encode completion response",
                        ),
                    );
                }
            };
            response_parts.headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            let body = body_from_stream(async_stream::try_stream! {
                let guard=lifetime;
                guard.record.finish("completed",None);
                yield Bytes::from(bytes);
                Result::<(),ProxyError>::Ok(())?;
            });
            return Ok(Response::from_parts(response_parts, body));
        }
        response_parts.headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/event-stream"),
        );
        response_parts
            .headers
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        let body = stream_response(
            incoming,
            lifetime,
            Some(route.max_sse_event_bytes),
            preview,
            server.idle_timeout(),
        );
        Ok(Response::from_parts(response_parts, body))
    } else {
        if hyper::body::Body::is_end_stream(&incoming) {
            if response_parts.status.as_u16() >= 400 {
                preview.report();
                record.finish("failed", None);
            } else {
                record.finish("completed", None);
            }
            return Ok(Response::from_parts(response_parts, Body::empty()));
        }
        let body = stream_response(incoming, lifetime, None, preview, server.idle_timeout());
        Ok(Response::from_parts(response_parts, body))
    }
}
fn fail<T>(lifetime: Lifetime, error: ProxyError) -> Result<T, ProxyError> {
    lifetime
        .record
        .progress
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .status = Some(error.status.as_u16());
    lifetime.record.finish("failed", Some(&error));
    Err(error)
}
fn stream_response(
    mut incoming: Incoming,
    lifetime: Lifetime,
    adapter_limit: Option<usize>,
    mut preview: ErrorPreview,
    idle: Duration,
) -> Body {
    body_from_stream(async_stream::try_stream! {
        let guard=lifetime;let record=guard.record.clone();let mut parser=adapter_limit.map(Sse::new);
        loop {
            let next:Result<_,ProxyError>=tokio::select! {
                _=guard.app.force_stop.cancelled()=>Err(ProxyError::new(StatusCode::SERVICE_UNAVAILABLE,"shutdown_deadline","shutdown","Service shutdown deadline exceeded")),
                result=tokio::time::timeout(idle,incoming.frame())=>match result {Err(_)=>Err(ProxyError::timeout("response_stream",idle.as_secs())),Ok(Some(Err(_)))=>Err(ProxyError::upstream("upstream_stream_failed","response_stream","Upstream response stream failed or was truncated")),Ok(Some(Ok(frame)))=>Ok(Some(frame)),Ok(None)=>Ok(None)},
            };
            let frame=match next {Ok(frame)=>frame,Err(error)=>{record.finish("failed",Some(&error));Err(error)?;unreachable!()}};
            let Some(frame)=frame else {
                if let Some(ref sse)=parser && !sse.done {let error=ProxyError::upstream("truncated_sse","adapter","Chat stream ended without DONE");record.finish("failed",Some(&error));Err(error)?;}
                finish_response(&record, &mut preview)?;
                break
            };
            let Ok(chunk)=frame.into_data() else{continue};
            record.bytes(chunk.len());preview.capture(&chunk);
            if let Some(ref mut sse)=parser {
                for byte in chunk {
                    let output=match sse.byte(byte){Ok(output)=>output,Err(error)=>{tracing::error!(event="sse_conversion_failed",request_id=%record.id,event_sequence=sse.sequence);record.finish("failed",Some(&error));Err(error)?;unreachable!()}};
                    match output {
                        Some(StreamOutput::Data(data)|StreamOutput::Heartbeat(data))=>yield Bytes::from(data),
                        Some(StreamOutput::Done)=>{finish_response(&record, &mut preview)?;yield Bytes::from_static(b"data: [DONE]\n\n");return},
                        Some(StreamOutput::Error(value))=>{preview.report_value(&value,&record.id);let error=ProxyError::upstream("upstream_stream_error","adapter","Upstream reported an error inside the response stream");record.finish("failed",Some(&error));Err(error)?;},
                        _=>{}
                    }
                }
            }else{
                // Hyper may stop polling as soon as Content-Length bytes are sent.
                // Finalize before yielding the last frame, rather than waiting for EOF.
                if hyper::body::Body::is_end_stream(&incoming) {
                    finish_response(&record, &mut preview)?;
                }
                yield chunk;
            }
        }
        Result::<(),ProxyError>::Ok(())?;
    })
}

fn finish_response(record: &Record, preview: &mut ErrorPreview) -> Result<(), ProxyError> {
    let upload_error = record
        .upload_error
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(error) = upload_error {
        record.finish("failed", Some(&error));
        return Err(error);
    }
    let failed = record
        .progress
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .upstream_status
        .is_some_and(|status| status >= 400);
    if failed {
        preview.report();
        record.finish("failed", None);
    } else {
        record.finish("completed", None);
    }
    Ok(())
}

fn body_from_stream<S>(stream: S) -> Body
where
    S: futures_util::Stream<Item = Result<Bytes, ProxyError>> + Send + 'static,
{
    Body::from_stream(stream)
}

fn credentials(headers: &HeaderMap) -> Vec<String> {
    ["authorization", "x-api-key", "api-key"]
        .iter()
        .flat_map(|name| headers.get_all(*name).iter())
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| {
            [
                v.to_string(),
                v.strip_prefix("Bearer ").unwrap_or(v).to_string(),
            ]
        })
        .filter(|v| !v.is_empty())
        .collect()
}
fn identity_encoded(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_ENCODING)
        .is_none_or(|v| v.as_bytes().eq_ignore_ascii_case(b"identity"))
}
fn content_type(headers: &HeaderMap) -> &str {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
}
fn require_json(headers: &HeaderMap) -> Result<(), ProxyError> {
    let media = content_type(headers);
    if (media == "application/json" || media.ends_with("+json")) && identity_encoded(headers) {
        Ok(())
    } else {
        Err(ProxyError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "request_decode",
            "JSON model rewriting requires uncompressed application/json",
        ))
    }
}
fn representation_changed(headers: &mut HeaderMap) {
    for key in [
        "content-length",
        "content-encoding",
        "etag",
        "content-md5",
        "digest",
        "content-digest",
        "repr-digest",
        "signature",
        "signature-input",
    ] {
        headers.remove(key);
    }
}
fn filter_headers(headers: &mut HeaderMap, request: bool) {
    let named: Vec<String> = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(',').map(|s| s.trim().to_ascii_lowercase()))
        .collect();
    for key in named {
        headers.remove(key);
    }
    for key in [
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ] {
        headers.remove(key);
    }
    if request {
        for key in [
            "host",
            "forwarded",
            "x-forwarded-for",
            "x-forwarded-host",
            "x-forwarded-proto",
            "x-transit-request-id",
        ] {
            headers.remove(key);
        }
    }
}
fn safe_header(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "-_:.".contains(*c))
        .take(128)
        .collect()
}
fn scrub(value: &str, secrets: &[String], limit: usize) -> String {
    let mut text = value.chars().take(4096).collect::<String>();
    for secret in secrets {
        text = text.replace(secret, "[redacted]");
    }
    let lower = text.to_ascii_lowercase();
    if text.contains(['\n', '\r', '`', '{', '}'])
        || [
            "<|fim_",
            "bearer ",
            "prompt:",
            "prompt=",
            "completion:",
            "content:",
            "source code",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return "[sensitive error detail omitted]".into();
    }
    // Remove quoted fragments, since providers sometimes echo user input in errors.
    let mut quoted = None;
    let mut safe = String::new();
    for character in text.chars() {
        if let Some(delimiter) = quoted {
            if character == delimiter {
                quoted = None;
            }
            continue;
        }
        if character == '\'' || character == '"' {
            quoted = Some(character);
            safe.push_str("[quoted detail omitted]");
        } else {
            safe.push(character);
        }
    }
    safe.split_whitespace()
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            if word.contains("://")
                || word.contains(['?', '=', '\\'])
                || word.len() > 96
                || [
                    "sk-",
                    "key-",
                    "api_key",
                    "api-key",
                    "token:",
                    "authorization:",
                ]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "[redacted]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(limit)
        .collect()
}

fn transport_detail(error: &(dyn StdError + 'static), secrets: &[String]) -> String {
    let mut parts = Vec::new();
    let mut next = Some(error);
    while let Some(error) = next {
        parts.push(scrub(&error.to_string(), secrets, 256));
        if parts.len() == 4 {
            break;
        }
        next = error.source();
    }
    parts.join("; ")
}
struct ErrorPreview {
    request_id: String,
    http_error: bool,
    reported: bool,
    bytes: Vec<u8>,
    limit: usize,
    enabled: bool,
    reason: &'static str,
    secrets: Vec<String>,
    upstream_request_id: String,
}
impl ErrorPreview {
    fn new(
        id: &str,
        status: StatusCode,
        headers: &HeaderMap,
        limit: usize,
        secrets: Vec<String>,
    ) -> Self {
        let media = content_type(headers);
        let enabled = limit > 0
            && (media == "application/json" || media.ends_with("+json"))
            && identity_encoded(headers);
        Self {
            request_id: id.into(),
            http_error: status.is_client_error() || status.is_server_error(),
            reported: false,
            bytes: Vec::new(),
            limit,
            enabled: enabled && (status.is_client_error() || status.is_server_error()),
            reason: if limit == 0 {
                "preview_disabled"
            } else if enabled {
                "invalid_or_truncated_json"
            } else {
                "unsupported_media_or_encoding"
            },
            secrets,
            upstream_request_id: safe_header(headers, "x-request-id"),
        }
    }
    fn capture(&mut self, bytes: &[u8]) {
        if self.enabled {
            let remaining = self.limit.saturating_sub(self.bytes.len());
            self.bytes
                .extend_from_slice(&bytes[..remaining.min(bytes.len())]);
        }
    }
    fn report(&mut self) {
        if self.reported {
            return;
        }
        self.reported = true;
        let id = &self.request_id;
        if let Ok(value) = serde_json::from_slice::<Value>(&self.bytes) {
            self.report_value(&value, id)
        } else {
            tracing::warn!(event="upstream_error_detail_unavailable",request_id=id,reason=self.reason,upstream_request_id=%self.upstream_request_id);
        }
    }
    fn report_value(&self, value: &Value, id: &str) {
        let error = value.get("error").unwrap_or(value);
        let field = |name: &str| {
            error
                .get(name)
                .and_then(Value::as_str)
                .map(|s| scrub(s, &self.secrets, 512))
                .unwrap_or_default()
        };
        tracing::warn!(event="upstream_error_detail",request_id=id,error_type=field("type"),error_code=field("code"),error_param=field("param"),message=field("message"),upstream_request_id=%self.upstream_request_id);
    }
}
impl Drop for ErrorPreview {
    fn drop(&mut self) {
        // Preserve any captured HTTP error details even when the client cancels
        // or the upstream stream fails before normal completion.
        if self.http_error {
            self.report();
        }
    }
}
async fn collect_body(
    mut body: Body,
    limit: usize,
    idle: Duration,
    stage: &'static str,
    cancel: &CancellationToken,
    force: &CancellationToken,
) -> Result<Bytes, ProxyError> {
    let mut bytes = BytesMut::new();
    loop {
        let frame = tokio::select! {_=cancel.cancelled()=>return Err(ProxyError::bad("client_cancelled","Request cancelled")),_=force.cancelled()=>return Err(ProxyError::new(StatusCode::SERVICE_UNAVAILABLE,"shutdown_deadline","shutdown","Shutdown deadline exceeded")),frame=tokio::time::timeout(idle,body.frame())=>frame.map_err(|_|ProxyError::timeout(stage,idle.as_secs()))?};
        let Some(frame) = frame else { break };
        let frame = frame
            .map_err(|_| ProxyError::bad("request_body_failed", "Cannot read request body"))?;
        if let Ok(chunk) = frame.into_data() {
            if bytes.len().saturating_add(chunk.len()) > limit {
                return Err(ProxyError::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "request_too_large",
                    stage,
                    "Request exceeds configured rewrite limit",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
    }
    Ok(bytes.freeze())
}
async fn collect_incoming(
    mut body: Incoming,
    limit: usize,
    server: &Server,
    cancel: &CancellationToken,
    force: &CancellationToken,
    record: &Record,
) -> Result<Bytes, ProxyError> {
    let mut bytes = BytesMut::new();
    loop {
        let frame = tokio::select! {_=cancel.cancelled()=>return Err(ProxyError::upstream("client_cancelled","response_stream","Request cancelled")),_=force.cancelled()=>return Err(ProxyError::new(StatusCode::SERVICE_UNAVAILABLE,"shutdown_deadline","shutdown","Shutdown deadline exceeded")),frame=tokio::time::timeout(server.idle_timeout(),body.frame())=>frame.map_err(|_|ProxyError::timeout("response_stream",server.stream_idle_timeout_seconds))?};
        let Some(frame) = frame else { break };
        let frame = frame.map_err(|_| {
            ProxyError::upstream(
                "upstream_stream_failed",
                "response_stream",
                "Cannot read upstream response",
            )
        })?;
        if let Ok(chunk) = frame.into_data() {
            record.bytes(chunk.len());
            if bytes.len().saturating_add(chunk.len()) > limit {
                return Err(ProxyError::upstream(
                    "response_too_large",
                    "adapter",
                    "Upstream JSON exceeds configured limit",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
    }
    Ok(bytes.freeze())
}
