use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, Response, StatusCode, header},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    fs,
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

struct Service {
    child: Child,
    directory: PathBuf,
}
impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.directory);
    }
}
async fn start(upstream: std::net::SocketAddr, directory: PathBuf) -> (Service, String) {
    let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    fs::create_dir_all(&directory).unwrap();
    let config = directory.join("transit.toml");
    fs::write(
        &config,
        format!(
            r#"
version = 1
default_provider = "fixture"
[server]
listen = "{address}"
max_rewrite_bytes = 1024
[logging]
directory = "{}"
[audit]
enabled = true
database = "{}"
max_response_json_bytes = 512
[capture]
enabled = true
directory = "{}"
request_body = true
max_body_bytes = 1024
[admission]
strict = true
[providers.fixture]
origin = "http://{upstream}"
[providers.fixture.model_aliases]
chat-latest = "fixture-model"
[[routes]]
id = "anthropic"
path_prefix = "/anthropic"
strip_prefix = true
[[routes]]
id = "edit"
method = "POST"
path = "/edit"
upstream_path = "/v1/chat/completions"
adapter = "completions_to_chat"
prompt_mode = "raw_user"
[[routes]]
id = "custom"
method = "POST"
path = "/custom"
upstream_path = "/vendor/chat"
protocol = "openai_chat"
"#,
            directory.join("logs").display(),
            directory.join("audit.sqlite3").display(),
            directory.join("captures").display()
        ),
    )
    .unwrap();
    drop(reserved);
    let child = Command::new(env!("CARGO_BIN_EXE_transit"))
        .args(["serve", "--config"])
        .arg(&config)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut service = Service { child, directory };
    for _ in 0..200 {
        assert!(
            service.child.try_wait().unwrap().is_none(),
            "service exited"
        );
        if tokio::net::TcpStream::connect(address).await.is_ok() {
            return (service, format!("http://{address}"));
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("service failed to start")
}
async fn call(
    base: &str,
    method: &str,
    path: &str,
    body: &str,
    auth: Option<(&str, &str)>,
) -> (u16, hyper::HeaderMap, String) {
    let client = Client::builder(TokioExecutor::new()).build_http::<Full<Bytes>>();
    let mut request = Request::builder()
        .method(method)
        .uri(format!("{base}{path}"))
        .header("content-type", "application/json");
    if let Some((name, value)) = auth {
        request = request.header(name, value);
    }
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        client.request(
            request
                .body(Full::new(Bytes::from(body.to_owned())))
                .unwrap(),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let body = tokio::time::timeout(Duration::from_secs(5), response.into_body().collect())
        .await
        .unwrap()
        .unwrap()
        .to_bytes();
    (status, headers, String::from_utf8(body.to_vec()).unwrap())
}
fn rows(service: &Service) -> Vec<Value> {
    let connection = Connection::open(service.directory.join("audit.sqlite3")).unwrap();
    let mut query=connection.prepare("SELECT request_id,outcome,client_protocol,upstream_protocol,http_status,upstream_status,input_tokens,output_tokens,total_tokens,usage_status,error_source,error_code,error_message,request_path,upstream_path,requested_model,resolved_model,usage_json FROM requests ORDER BY started_at_ms,rowid").unwrap();
    query.query_map([],|row|Ok(json!({"request_id":row.get::<_,String>(0)?,"outcome":row.get::<_,String>(1)?,"client_protocol":row.get::<_,String>(2)?,"upstream_protocol":row.get::<_,String>(3)?,"http_status":row.get::<_,Option<i64>>(4)?,"upstream_status":row.get::<_,Option<i64>>(5)?,"input_tokens":row.get::<_,Option<i64>>(6)?,"output_tokens":row.get::<_,Option<i64>>(7)?,"total_tokens":row.get::<_,Option<i64>>(8)?,"usage_status":row.get::<_,Option<String>>(9)?,"error_source":row.get::<_,Option<String>>(10)?,"error_code":row.get::<_,Option<String>>(11)?,"error_message":row.get::<_,Option<String>>(12)?,"request_path":row.get::<_,String>(13)?,"upstream_path":row.get::<_,String>(14)?,"requested_model":row.get::<_,Option<String>>(15)?,"resolved_model":row.get::<_,Option<String>>(16)?,"usage_json":row.get::<_,Option<String>>(17)?}))).unwrap().map(Result::unwrap).collect()
}

#[tokio::test]
async fn strict_admission_never_forwards_probes_and_audit_observes_without_rewriting_responses() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream = listener.local_addr().unwrap();
    let directory =
        std::env::temp_dir().join(format!("transit-integration-{}", uuid::Uuid::new_v4()));
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let chat_stream = "data: {\"model\":\"fixture-model\",\"choices\":[{\"delta\":{\"content\":\"hello\"},\"finish_reason\":\"stop\"}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":100,\"completion_tokens\":20,\"total_tokens\":120,\"prompt_tokens_details\":{\"cached_tokens\":40},\"completion_tokens_details\":{\"reasoning_tokens\":5}}}\n\ndata: [DONE]\n\n";
    let app=Router::new().fallback(move|request:Request<Body>|{
        let count=count.clone();async move {
            count.fetch_add(1,Ordering::Relaxed);
            let path=request.uri().path().to_string();
            assert!(request.headers().contains_key("authorization")||request.headers().contains_key("x-api-key"));
            let body=to_bytes(request.into_body(),4096).await.unwrap();
            let value:Value=serde_json::from_slice(&body).unwrap_or(Value::Null);
            match value["test_case"].as_str() {
                Some("encoded")=>return Response::builder().header(header::CONTENT_TYPE,"text/event-stream").header(header::CONTENT_ENCODING,"gzip").body(Body::from(compressed_response())).unwrap(),
                Some("http_error")=>return Response::builder().status(StatusCode::TOO_MANY_REQUESTS).header(header::CONTENT_TYPE,"application/json").body(Body::from(r#"{"error":{"code":"rate_limit","message":"retry fixture-secret"}}"#)).unwrap(),
                Some("oversize")=>return Response::builder().header(header::CONTENT_TYPE,"application/json").body(Body::from(json!({"choices":[{"message":{"content":"x".repeat(2048)}}],"usage":{"prompt_tokens":10,"completion_tokens":20}}).to_string())).unwrap(),
                Some("cancel")=>{
                    let stream=async_stream::stream! {
                        yield Ok::<_,std::io::Error>(Bytes::from_static(b"data: {\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":0}}\n\n"));
                        std::future::pending::<()>().await;
                    };
                    return Response::builder().header(header::CONTENT_TYPE,"text/event-stream").body(Body::from_stream(stream)).unwrap();
                },
                _=>{}
            }
            let (media,body)=match path.as_str(){
                "/v1/models"=>("application/json",json!({"data":[]}).to_string()),
                "/v1/chat/completions"|"/vendor/chat"=>{
                    assert_eq!(value["model"],"fixture-model");
                    if value["stream"]==true {("text/event-stream",chat_stream.into())}
                    else {("application/json",json!({"model":"fixture-model","choices":[{"index":0,"message":{"content":"code"},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}}).to_string())}
                },
                "/v1/messages"=>{
                    let events=[json!({"type":"message_start","message":{"model":"fixture-model","usage":{"input_tokens":10,"cache_creation_input_tokens":20,"cache_read_input_tokens":30,"output_tokens":0}}}),json!({"type":"message_delta","usage":{"output_tokens":5}}),json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":7}}),json!({"type":"message_stop"})];
                    ("text/event-stream",events.iter().map(|v|format!("event: {}\ndata: {v}\n\n",v["type"].as_str().unwrap())).collect())
                },
                "/v1/responses"=>("text/plain",format!("event: response.failed\ndata: {}\n\n",json!({"type":"response.failed","response":{"status":"failed","error":{"code":"engine_overloaded","message":"retry fixture-secret"}}}))),
                _=>panic!("unexpected request reached upstream: {path}"),
            };
            Response::builder().status(StatusCode::OK).header(header::CONTENT_TYPE,media).body(Body::from(body)).unwrap()
        }
    });
    let fixture = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let (service, base) = start(upstream, directory).await;
    let auth = Some(("authorization", "Bearer fixture-secret"));
    let chat=json!({"model":"chat-latest","messages":[{"role":"user","content":"private input"}],"stream":true,"vendor_extension":true}).to_string();
    let probes = [
        ("GET", "/.env", "", auth),
        ("GET", "/_transit/health/ready", "", None),
        ("POST", "/_transit/health/live", "", None),
        ("POST", "/v1/chat/completions", chat.as_str(), None),
        ("POST", "/v1/chat/completions", "{malformed", auth),
        ("POST", "/v1/chat/completions", "{}", auth),
        ("GET", "/v1/chat/completions", "", auth),
        ("GET", "/v1/models", "unexpected body", auth),
    ];
    for (method, path, body, auth) in probes {
        let (status, headers, body) = call(&base, method, path, body, auth).await;
        assert_eq!(status, 404);
        assert!(body.is_empty());
        assert!(!headers.contains_key("x-transit-request-id"));
    }
    let oversized =
        json!({"model":"chat-latest","messages":[{"role":"user","content":"x".repeat(2048)}]})
            .to_string();
    assert_eq!(
        call(&base, "POST", "/v1/chat/completions", &oversized, auth)
            .await
            .0,
        404
    );
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert!(rows(&service).is_empty());
    let (status, _, body) = call(
        &base,
        "POST",
        "/v1/chat/completions?api_key=fixture-query-secret",
        &chat,
        auth,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body, chat_stream);
    let anthropic=json!({"model":"fixture-model","max_tokens":100,"messages":[{"role":"user","content":"private input"}],"stream":true}).to_string();
    assert_eq!(
        call(
            &base,
            "POST",
            "/anthropic/v1/messages",
            &anthropic,
            Some(("x-api-key", "fixture-secret"))
        )
        .await
        .0,
        200
    );
    let response =
        json!({"model":"fixture-model","input":"private input","stream":true}).to_string();
    let (status, _, body) = call(&base, "POST", "/v1/responses", &response, auth).await;
    assert_eq!(status, 200);
    assert!(
        body.contains("fixture-secret"),
        "upstream error must remain unmodified downstream"
    );
    let edit = json!({"model":"chat-latest","prompt":"complete code"}).to_string();
    let (status, _, body) = call(&base, "POST", "/edit", &edit, auth).await;
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["choices"][0]["text"],
        "code"
    );
    assert_eq!(call(&base, "GET", "/v1/models", "", auth).await.0, 200);
    let plain =
        json!({"model":"chat-latest","messages":[{"role":"user","content":"private input"}]})
            .to_string();
    assert_eq!(call(&base, "POST", "/custom", &plain, auth).await.0, 200);
    let mut extra: Value = serde_json::from_str(&plain).unwrap();
    extra["test_case"] = json!("http_error");
    let (status, _, body) = call(
        &base,
        "POST",
        "/v1/chat/completions",
        &extra.to_string(),
        auth,
    )
    .await;
    assert_eq!(status, 429);
    assert!(body.contains("fixture-secret"));
    extra["test_case"] = json!("oversize");
    let (status, _, body) = call(
        &base,
        "POST",
        "/v1/chat/completions",
        &extra.to_string(),
        auth,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["choices"][0]["message"]["content"],
        "x".repeat(2048)
    );
    extra["test_case"] = json!("cancel");
    extra["stream"] = json!(true);
    let client = Client::builder(TokioExecutor::new()).build_http::<Full<Bytes>>();
    let request = Request::builder()
        .method("POST")
        .uri(format!("{base}/v1/chat/completions"))
        .header("content-type", "application/json")
        .header("authorization", "Bearer fixture-secret")
        .body(Full::new(Bytes::from(extra.to_string())))
        .unwrap();
    let mut response = tokio::time::timeout(Duration::from_secs(5), client.request(request))
        .await
        .unwrap()
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), response.body_mut().frame())
            .await
            .unwrap()
            .unwrap()
            .is_ok()
    );
    drop(response);
    drop(client);
    extra["test_case"] = json!("encoded");
    let client = Client::builder(TokioExecutor::new()).build_http::<Full<Bytes>>();
    let request = Request::builder()
        .method("POST")
        .uri(format!("{base}/v1/chat/completions"))
        .header("content-type", "application/json")
        .header("authorization", "Bearer fixture-secret")
        .body(Full::new(Bytes::from(extra.to_string())))
        .unwrap();
    let response = tokio::time::timeout(Duration::from_secs(5), client.request(request))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-encoding"], "gzip");
    let encoded = tokio::time::timeout(Duration::from_secs(5), response.into_body().collect())
        .await
        .unwrap()
        .unwrap()
        .to_bytes();
    assert_eq!(encoded.as_ref(), compressed_response());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let records = loop {
        let records = rows(&service);
        if records.len() == 10 && records.iter().all(|r| r["outcome"] != "in_progress") {
            break records;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "audit did not finish: {records:?}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert_eq!(calls.load(Ordering::Relaxed), 10);
    assert_eq!(records[0]["input_tokens"], 100);
    assert_eq!(records[0]["total_tokens"], 120);
    assert_eq!(records[0]["usage_status"], "complete");
    assert_eq!(records[0]["request_path"], "/v1/chat/completions");
    assert_eq!(records[1]["input_tokens"], 60);
    assert_eq!(records[1]["output_tokens"], 7);
    assert_eq!(records[1]["total_tokens"], 67);
    assert_eq!(records[1]["upstream_path"], "/v1/messages");
    assert_eq!(records[2]["http_status"], 200);
    assert_eq!(records[2]["outcome"], "failed");
    assert_eq!(records[2]["error_source"], "upstream_body");
    assert_eq!(records[2]["error_code"], "engine_overloaded");
    assert!(records[2]["input_tokens"].is_null());
    assert_eq!(records[3]["client_protocol"], "openai_completions");
    assert_eq!(records[3]["upstream_protocol"], "openai_chat");
    assert_eq!(records[3]["total_tokens"], 12);
    assert_eq!(records[4]["usage_status"], "missing");
    assert!(records[4]["input_tokens"].is_null());
    assert_eq!(records[6]["outcome"], "failed");
    assert_eq!(records[6]["error_source"], "upstream_http");
    assert_eq!(records[6]["error_code"], "rate_limit");
    assert_eq!(records[7]["outcome"], "completed");
    assert_eq!(records[7]["usage_status"], "missing");
    assert_eq!(records[8]["outcome"], "cancelled");
    assert_eq!(records[8]["error_source"], "client");
    assert_eq!(records[8]["input_tokens"], 10);
    assert_eq!(records[8]["usage_status"], "partial");
    assert_eq!(records[9]["usage_status"], "complete");
    assert_eq!(records[9]["total_tokens"], 12);
    let captures = wait_captures(&service, 10).await;
    assert_eq!(
        captured_body(
            &captures[records[0]["request_id"].as_str().unwrap()],
            "request"
        ),
        chat.as_bytes()
    );
    assert_eq!(
        captured_body(
            &captures[records[0]["request_id"].as_str().unwrap()],
            "response"
        ),
        chat_stream.as_bytes()
    );
    let limited = &captures[records[7]["request_id"].as_str().unwrap()];
    assert_eq!(limited.last().unwrap()["capture_complete"], false);
    assert_eq!(limited.last().unwrap()["response_body"]["truncated"], true);
    let cancelled = &captures[records[8]["request_id"].as_str().unwrap()];
    assert_eq!(cancelled.last().unwrap()["capture_complete"], false);
    let compressed = &captures[records[9]["request_id"].as_str().unwrap()];
    assert_eq!(captured_body(compressed, "response"), compressed_response());
    assert!(
        compressed
            .iter()
            .any(|r| r["type"] == "response_headers"
                && r["headers"]["content-encoding"][0] == "gzip")
    );
    for capture in captures.values() {
        let first = &capture[0];
        assert!(first["headers"].get("authorization").is_none());
        assert!(first["headers"].get("x-api-key").is_none());
    }
    let stored = serde_json::to_string(&records).unwrap();
    assert!(!stored.contains("fixture-secret"));
    assert!(!stored.contains("fixture-query-secret"));
    assert!(!stored.contains("private input"));
    fixture.abort();
}

fn compressed_response() -> Vec<u8> {
    STANDARD.decode("H4sIAAAAAAAC/0tJLEm0UqhWSs7Iz0xOLVayio7VUSotTkxPVbKqViooys8tKIkvyc9OzQPKGRroKCUDRXJSSzLz8+DCRjpKJfkliTkIdUa1tVxcKWCjo138/VxjubgAAL2Jf2kAAAA=").unwrap()
}
fn capture_files(service: &Service) -> std::collections::HashMap<String, Vec<Value>> {
    let mut captures = std::collections::HashMap::new();
    if let Ok(days) = fs::read_dir(service.directory.join("captures")) {
        for day in days.flatten() {
            if !day.file_type().unwrap().is_dir() {
                continue;
            }
            for file in fs::read_dir(day.path()).unwrap().flatten() {
                if file.path().extension().is_none_or(|e| e != "jsonl") {
                    continue;
                }
                let data = fs::read_to_string(file.path()).unwrap();
                let rows = data
                    .lines()
                    .filter_map(|s| serde_json::from_str::<Value>(s).ok())
                    .collect::<Vec<_>>();
                if rows.last().is_some_and(|r| r["type"] == "request_finished") {
                    captures.insert(
                        file.path().file_stem().unwrap().to_str().unwrap().into(),
                        rows,
                    );
                }
            }
        }
    }
    captures
}
async fn wait_captures(
    service: &Service,
    count: usize,
) -> std::collections::HashMap<String, Vec<Value>> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let captures = capture_files(service);
        if captures.len() == count {
            return captures;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "capture files did not finish: {}",
            captures.len()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
fn captured_body(capture: &[Value], direction: &str) -> Vec<u8> {
    capture
        .iter()
        .filter(|r| r["type"] == "body_chunk" && r["direction"] == direction)
        .flat_map(|r| STANDARD.decode(r["data_base64"].as_str().unwrap()).unwrap())
        .collect()
}

#[cfg(unix)]
#[tokio::test]
async fn capture_hot_reload_and_graceful_shutdown_preserve_an_existing_stream() {
    const START: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n";
    const END: &str = "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}}\n\ndata: [DONE]\n\n";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream = listener.local_addr().unwrap();
    let release = Arc::new(tokio::sync::Notify::new());
    let notify = release.clone();
    let fixture = tokio::spawn(async move {
        let app = Router::new().fallback(move |request: Request<Body>| {
            let notify = notify.clone();
            async move {
                let body = to_bytes(request.into_body(), 4096).await.unwrap();
                let value: Value = serde_json::from_slice(&body).unwrap();
                let hold = value["hold"] == true;
                let stream = async_stream::stream! {
                    yield Ok::<_,std::io::Error>(Bytes::from_static(START.as_bytes()));
                    if hold {notify.notified().await;}
                    yield Ok::<_,std::io::Error>(Bytes::from_static(END.as_bytes()));
                };
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .body(Body::from_stream(stream))
                    .unwrap()
            }
        });
        axum::serve(listener, app).await.unwrap();
    });
    let directory = std::env::temp_dir().join(format!("transit-reload-{}", uuid::Uuid::new_v4()));
    let (mut service, base) = start(upstream, directory).await;
    let body = json!({"model":"chat-latest","messages":[{"role":"user","content":"fixture"}],"stream":true,"hold":true});
    let client = Client::builder(TokioExecutor::new()).build_http::<Full<Bytes>>();
    let request = Request::builder()
        .method("POST")
        .uri(format!("{base}/v1/chat/completions"))
        .header("authorization", "Bearer fixture-secret")
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap();
    let mut response = tokio::time::timeout(Duration::from_secs(5), client.request(request))
        .await
        .unwrap()
        .unwrap();
    let first = tokio::time::timeout(Duration::from_secs(5), response.body_mut().frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
    assert_eq!(first.as_ref(), START.as_bytes());
    let config = service.directory.join("transit.toml");
    let updated = fs::read_to_string(&config)
        .unwrap()
        .replace("[capture]\nenabled = true", "[capture]\nenabled = false");
    fs::write(&config, updated).unwrap();
    assert!(
        Command::new("kill")
            .args(["-HUP", &service.child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let reloaded = fs::read_dir(service.directory.join("logs"))
            .unwrap()
            .flatten()
            .filter(|f| f.path().extension().is_some_and(|e| e == "jsonl"))
            .any(|f| {
                fs::read_to_string(f.path())
                    .unwrap()
                    .lines()
                    .filter_map(|s| serde_json::from_str::<Value>(s).ok())
                    .any(|r| {
                        r["fields"]["event"] == "reload_completed"
                            && r["fields"]["config_generation"] == 2
                    })
            });
        if reloaded {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "configuration did not reload"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let mut second = body.clone();
    second["hold"] = json!(false);
    let (status, _, received) = call(
        &base,
        "POST",
        "/v1/chat/completions",
        &second.to_string(),
        Some(("authorization", "Bearer fixture-secret")),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(received, format!("{START}{END}"));
    assert!(
        Command::new("kill")
            .args(["-TERM", &service.child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        service.child.try_wait().unwrap().is_none(),
        "shutdown failed to wait for active stream"
    );
    release.notify_one();
    let rest = tokio::time::timeout(Duration::from_secs(5), response.into_body().collect())
        .await
        .unwrap()
        .unwrap()
        .to_bytes();
    assert_eq!(rest.as_ref(), END.as_bytes());
    drop(client);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = service.child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "service did not drain"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let captures = capture_files(&service);
    assert_eq!(
        captures.len(),
        1,
        "capture toggle should affect only new requests"
    );
    let captured = captures.values().next().unwrap();
    assert_eq!(
        captured_body(captured, "response"),
        format!("{START}{END}").as_bytes()
    );
    assert_eq!(captured.last().unwrap()["capture_complete"], true);
    assert_eq!(captured.last().unwrap()["summary"]["config_generation"], 1);
    let records = rows(&service);
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .all(|r| r["outcome"] == "completed" && r["usage_status"] == "complete")
    );
    fixture.abort();
}

#[tokio::test]
async fn a_locked_audit_database_does_not_delay_model_responses() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream = listener.local_addr().unwrap();
    let fixture = tokio::spawn(async move {
        let app=Router::new().fallback(||async {
            Response::builder().header("content-type","application/json").body(Body::from(r#"{"choices":[{"message":{"content":"ok"}}],"usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}}"#)).unwrap()
        });
        axum::serve(listener, app).await.unwrap();
    });
    let directory =
        std::env::temp_dir().join(format!("transit-audit-lock-{}", uuid::Uuid::new_v4()));
    let (service, base) = start(upstream, directory).await;
    let lock = Connection::open(service.directory.join("audit.sqlite3")).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE;").unwrap();
    let request =
        json!({"model":"chat-latest","messages":[{"role":"user","content":"fixture"}]}).to_string();
    // SQLite's writer busy timeout is two seconds. A response must arrive while
    // the external write lock is still held, well before that timeout.
    let response = tokio::time::timeout(
        Duration::from_secs(1),
        call(
            &base,
            "POST",
            "/v1/chat/completions",
            &request,
            Some(("authorization", "Bearer fixture-secret")),
        ),
    )
    .await
    .expect("model response waited for the audit database");
    assert_eq!(response.0, 200);
    assert!(response.2.contains("ok"));
    assert!(
        rows(&service).is_empty(),
        "the locked writer unexpectedly committed"
    );
    lock.execute_batch("COMMIT;").unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let records = rows(&service);
        if records.len() == 1 && records[0]["usage_status"] == "complete" {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "deferred audit update did not finish: {records:?}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    fixture.abort();
}
