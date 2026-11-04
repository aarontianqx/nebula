use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
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

fn request(address: SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn events(directory: &std::path::Path) -> Vec<serde_json::Value> {
    fs::read_dir(directory.join("logs"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .flat_map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn content_length_responses_finish_and_report_upstream_errors() {
    let upstream = TcpListener::bind("127.0.0.1:0").unwrap();
    let upstream_address = upstream.local_addr().unwrap();
    upstream.set_nonblocking(true).unwrap();
    let fixture = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut served = 0;
        while served < 2 && Instant::now() < deadline {
            let (mut connection, _) = match upstream.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(error) => panic!("fixture accept: {error}"),
            };
            connection
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                connection.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let bad = header.starts_with(b"GET /upstream/bad?");
            if bad {
                assert!(String::from_utf8_lossy(&header).contains("api_key=fixture-query-secret"));
            } else {
                assert!(header.starts_with(b"GET /upstream/ok "));
            }
            let (status, body) = if bad {
                (
                    "404 Not Found",
                    r#"{"error":{"type":"invalid_request_error","message":"route not found"}}"#,
                )
            } else {
                ("200 OK", r#"{"ok":true}"#)
            };
            write!(connection,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            served += 1;
        }
        assert_eq!(served, 2, "fixture did not receive both requests");
    });
    let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    let directory = std::env::temp_dir().join(format!("transit-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    let config = directory.join("transit.toml");
    fs::write(
        &config,
        format!(
            "version = 1\ndefault_provider = 'fixture'\n[server]\nlisten = '{address}'\n[logging]\ndirectory = '{}'\n[providers.fixture]\norigin = 'http://{upstream_address}'\nbase_path = '/upstream'\n[[routes]]\nid = 'client-prefix'\npath_prefix = '/client'\nstrip_prefix = true\n",
            directory.join("logs").display()
        ),
    )
    .unwrap();
    drop(reserved);
    let child = Command::new(env!("CARGO_BIN_EXE_transit"))
        .args(["serve", "--config"])
        .arg(config)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut service = Service { child, directory };
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            service.child.try_wait().unwrap().is_none(),
            "service exited"
        );
        if TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "service did not start");
        thread::sleep(Duration::from_millis(10));
    }
    let success = request(address, "/ok");
    assert!(success.starts_with("HTTP/1.1 200"), "{success}");
    assert!(success.ends_with(r#"{"ok":true}"#), "{success}");
    let failure = request(address, "/client/bad?api_key=fixture-query-secret");
    assert!(failure.starts_with("HTTP/1.1 404"), "{failure}");
    assert!(
        failure
            .ends_with(r#"{"error":{"type":"invalid_request_error","message":"route not found"}}"#)
    );
    fixture.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let records = loop {
        let records = events(&service.directory);
        if records
            .iter()
            .filter(|v| v["fields"]["event"] == "request_finished")
            .count()
            == 2
        {
            break records;
        }
        assert!(Instant::now() < deadline, "missing request log records");
        thread::sleep(Duration::from_millis(10));
    };
    let finished: Vec<_> = records
        .iter()
        .filter(|v| v["fields"]["event"] == "request_finished")
        .map(|v| &v["fields"])
        .collect();
    assert_eq!(finished[0]["outcome"], "completed", "{finished:?}");
    assert_eq!(finished[1]["outcome"], "failed", "{finished:?}");
    assert_eq!(finished[1]["status"], 404);
    assert_eq!(finished[1]["upstream_status"], 404);
    for event in ["request_started", "request_finished"] {
        let paths: Vec<_> = records
            .iter()
            .filter(|r| r["fields"]["event"] == event)
            .map(|r| (&r["fields"]["request_path"], &r["fields"]["upstream_path"]))
            .collect();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].0, "/ok");
        assert_eq!(paths[0].1, "/upstream/ok");
        assert_eq!(paths[1].0, "/client/bad");
        assert_eq!(paths[1].1, "/upstream/bad");
    }
    let logged = serde_json::to_string(&records).unwrap();
    assert!(!logged.contains("fixture-query-secret"));
    assert!(!logged.contains("api_key="));

    assert!(
        records
            .iter()
            .any(|v| v["fields"]["event"] == "upstream_error_detail"
                && v["fields"]["message"] == "route not found"),
        "{records:?}"
    );
}

#[test]
fn provider_profiles_apply_after_conversion_and_leave_default_requests_unchanged() {
    let upstream = TcpListener::bind("127.0.0.1:0").unwrap();
    let upstream_address = upstream.local_addr().unwrap();
    upstream.set_nonblocking(true).unwrap();
    let fixture = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut requests = Vec::new();
        while requests.len() < 4 && Instant::now() < deadline {
            let (mut connection, _) = match upstream.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(error) => panic!("fixture accept: {error}"),
            };
            connection
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                connection.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let header = String::from_utf8(header).unwrap();
            let size = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .expect("upstream request must have known length");
            let mut body = vec![0; size];
            connection.read_exact(&mut body).unwrap();
            requests.push((header, body));
            let body = r#"{"choices":[{"index":0,"message":{"content":"return a + b"},"finish_reason":"stop"}]}"#;
            write!(connection,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
        assert_eq!(requests.len(), 4);
        requests
    });
    let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    let directory = std::env::temp_dir().join(format!("transit-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    let config = directory.join("transit.toml");
    fs::write(
        &config,
        format!(
            r#"
version = 1
default_provider = "primary"
[server]
listen = "{address}"
[logging]
directory = "{}"
[providers.primary]
origin = "http://{upstream_address}"
[providers.primary.model_aliases]
edit-fast = "fixture-model"
[providers.primary.request_profiles.edit]
set = {{ thinking = {{ type = "disabled" }} }}
remove = ["temperature", "reasoning_effort"]
[providers.secondary]
origin = "http://{upstream_address}"
[providers.secondary.request_profiles.edit]
set = {{ reasoning_effort = "low" }}
[[routes]]
id = "edit"
method = "POST"
path = "/edit"
upstream_path = "/v1/chat/completions"
adapter = "completions_to_chat"
prompt_mode = "fim_qwen"
request_profile = "edit"
[[routes]]
id = "plain"
method = "POST"
path = "/plain"
request_profile = "edit"
[[routes]]
id = "secondary-edit"
method = "POST"
path = "/secondary-edit"
provider = "secondary"
upstream_path = "/v1/chat/completions"
adapter = "completions_to_chat"
prompt_mode = "fim_qwen"
request_profile = "edit"
"#,
            directory.join("logs").display()
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
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            service.child.try_wait().unwrap().is_none(),
            "service exited"
        );
        if TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "service did not start");
        thread::sleep(Duration::from_millis(10));
    }
    let ordinary = r#"{ "model": "fixture-model", "messages": [], "temperature": 0.2, "thinking": {"type":"enabled"}, "reasoning_effort":"high", "custom":true }"#;
    let edit = r#"{"model":"edit-fast","prompt":"<|fim_prefix|>def add(a,b):\n    <|fim_suffix|>\n<|fim_middle|>","temperature":0.2,"max_tokens":512}"#;
    for (path, body) in [
        ("/v1/chat/completions", ordinary),
        ("/edit", edit),
        ("/plain", ordinary),
        ("/secondary-edit", edit),
    ] {
        let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(stream,"POST {path} HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer fixture-credential\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        if path.ends_with("edit") {
            assert!(response.contains(r#""text":"return a + b""#), "{response}");
        }
    }
    let requests = fixture.join().unwrap();
    for (header, _) in &requests {
        assert!(
            header
                .to_ascii_lowercase()
                .contains("authorization: bearer fixture-credential")
        );
    }
    assert_eq!(requests[0].1, ordinary.as_bytes());
    let converted: serde_json::Value = serde_json::from_slice(&requests[1].1).unwrap();
    assert_eq!(converted["model"], "fixture-model");
    assert_eq!(
        converted["thinking"],
        serde_json::json!({"type":"disabled"})
    );
    assert!(converted.get("temperature").is_none());
    assert!(converted.get("prompt").is_none());
    assert_eq!(converted["messages"][0]["role"], "system");
    let plain: serde_json::Value = serde_json::from_slice(&requests[2].1).unwrap();
    assert_eq!(plain["thinking"]["type"], "disabled");
    assert!(plain.get("temperature").is_none());
    assert!(plain.get("reasoning_effort").is_none());
    assert_eq!(plain["custom"], true);
    let secondary: serde_json::Value = serde_json::from_slice(&requests[3].1).unwrap();
    assert_eq!(secondary["reasoning_effort"], "low");
    assert_eq!(secondary["temperature"], 0.2);
    assert!(secondary.get("thinking").is_none());
}

#[tokio::test]
async fn large_json_and_reasoning_only_completions_preserve_success_and_log_warnings() {
    use bytes::Bytes;
    use http_body_util::{BodyExt, Full};
    use hyper_util::{client::legacy::Client, rt::TokioExecutor};
    use serde_json::{Value, json};

    let upstream = TcpListener::bind("127.0.0.1:0").unwrap();
    let upstream_address = upstream.local_addr().unwrap();
    upstream.set_nonblocking(true).unwrap();
    let reasoning_json = json!({"id":"fixture","choices":[{"index":0,"message":{"content":null,"reasoning_content":"private thought"},"finish_reason":"length"}],"usage":{"completion_tokens":512}}).to_string();
    let expected_json = reasoning_json.clone();
    let fixture = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut served = 0;
        while served < 4 && Instant::now() < deadline {
            let (mut connection, _) = match upstream.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(error) => panic!("fixture accept: {error}"),
            };
            connection
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                connection.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            let header = String::from_utf8(header).unwrap();
            let size = header
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            let mut body = vec![0; size];
            connection.read_exact(&mut body).unwrap();
            let value: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(value["model"], "fixture-model");
            if served == 0 {
                assert!(size > 2 * 1024 * 1024);
                assert_eq!(
                    value["messages"][0]["content"].as_str().unwrap().len(),
                    2 * 1024 * 1024
                );
            }
            let (media, body) = if served == 2 {
                let reasoning = json!({"choices":[{"index":0,"delta":{"reasoning_content":"private thought"},"finish_reason":null}]});
                let finish = json!({"choices":[{"index":0,"delta":{},"finish_reason":"length"}]});
                let usage = json!({"choices":[],"usage":{"completion_tokens":512}});
                (
                    "text/event-stream",
                    format!(
                        "data: {reasoning}\n\ndata: {finish}\n\ndata: {usage}\n\ndata: [DONE]\n\n"
                    ),
                )
            } else {
                ("application/json", reasoning_json.clone())
            };
            write!(connection,"HTTP/1.1 200 OK\r\nContent-Type: {media}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            served += 1;
        }
        assert_eq!(served, 4);
    });
    let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    let directory = std::env::temp_dir().join(format!("transit-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    let config = directory.join("transit.toml");
    fs::write(
        &config,
        format!(
            r#"
version = 1
default_provider = "fixture"
[server]
listen = "{address}"
[logging]
directory = "{}"
[providers.fixture]
origin = "http://{upstream_address}"
[providers.fixture.model_aliases]
chat-latest = "fixture-model"
[[routes]]
id = "edit"
method = "POST"
path = "/edit"
upstream_path = "/v1/chat/completions"
adapter = "completions_to_chat"
prompt_mode = "raw_user"
[[routes]]
id = "tiny-edit"
method = "POST"
path = "/tiny-edit"
upstream_path = "/v1/chat/completions"
adapter = "completions_to_chat"
prompt_mode = "raw_user"
max_request_bytes = 64
"#,
            directory.join("logs").display()
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
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            service.child.try_wait().unwrap().is_none(),
            "service exited"
        );
        if TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "service did not start");
        thread::sleep(Duration::from_millis(10));
    }
    let client = Client::builder(TokioExecutor::new()).build_http::<Full<Bytes>>();
    let large = json!({"model":"chat-latest","messages":[{"role":"user","content":"x".repeat(2*1024*1024)}]}).to_string();
    let edit = json!({"model":"chat-latest","prompt":"complete code","stream":false}).to_string();
    let streaming =
        json!({"model":"chat-latest","prompt":"complete code","stream":true}).to_string();
    let plain = json!({"model":"chat-latest","messages":[]}).to_string();
    let mut edit_ids = Vec::new();
    for (index, (path, body)) in [
        ("/v1/chat/completions", large),
        ("/edit", edit.clone()),
        ("/edit", streaming),
        ("/v1/chat/completions", plain),
        ("/tiny-edit", format!("{edit}{}", " ".repeat(64))),
    ]
    .into_iter()
    .enumerate()
    {
        let request = hyper::Request::post(format!("http://{address}{path}"))
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(body)))
            .unwrap();
        let response = tokio::time::timeout(Duration::from_secs(5), client.request(request))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            response.status().as_u16(),
            if index == 4 { 413 } else { 200 }
        );
        if path == "/edit" {
            edit_ids.push(
                response.headers()["x-transit-request-id"]
                    .to_str()
                    .unwrap()
                    .to_string(),
            );
        }
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(body.to_vec()).unwrap();
        match index {
            0 | 3 => assert_eq!(body, expected_json),
            1 => {
                let value: Value = serde_json::from_str(&body).unwrap();
                assert_eq!(value["choices"][0]["text"], "");
                assert_eq!(value["choices"][0]["finish_reason"], "length");
                assert_eq!(value["usage"]["completion_tokens"], 512);
                assert!(!body.contains("private thought"));
            }
            2 => {
                let events: Vec<_> = body
                    .split("\n\n")
                    .filter_map(|event| event.strip_prefix("data: "))
                    .collect();
                assert_eq!(events.len(), 3);
                let finish: Value = serde_json::from_str(events[0]).unwrap();
                assert_eq!(finish["choices"][0]["text"], "");
                assert_eq!(finish["choices"][0]["finish_reason"], "length");
                let usage: Value = serde_json::from_str(events[1]).unwrap();
                assert_eq!(usage["usage"]["completion_tokens"], 512);
                assert_eq!(events[2], "[DONE]");
                assert!(!body.contains("private thought"));
            }
            _ => assert!(body.contains("request_too_large")),
        }
    }
    fixture.join().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let records = loop {
        let records = events(&service.directory);
        if records
            .iter()
            .filter(|r| r["fields"]["event"] == "request_finished")
            .count()
            == 5
        {
            break records;
        }
        assert!(Instant::now() < deadline, "missing request log records");
        thread::sleep(Duration::from_millis(10));
    };
    let warnings: Vec<_> = records
        .iter()
        .filter(|r| r["fields"]["event"] == "empty_completion")
        .collect();
    assert_eq!(warnings.len(), 2);
    for (index, warning) in warnings.iter().enumerate() {
        assert_eq!(warning["level"], "WARN");
        assert_eq!(warning["fields"]["reason"], "reasoning_only");
        assert_eq!(warning["fields"]["request_id"], edit_ids[index]);
        assert_eq!(warning["fields"]["streaming"], index == 1);
    }
    assert!(
        !serde_json::to_string(&records)
            .unwrap()
            .contains("private thought")
    );
    assert_eq!(
        records
            .iter()
            .filter(|r| r["fields"]["event"] == "request_finished"
                && r["fields"]["outcome"] == "completed")
            .count(),
        4
    );
}
