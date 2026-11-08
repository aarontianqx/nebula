use rusqlite::{Connection, params};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
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
fn get(address: std::net::SocketAddr, path: &str, extra: &str) -> (u16, String, String) {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(6)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n{extra}\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    (
        headers.split_whitespace().nth(1).unwrap().parse().unwrap(),
        headers.to_string(),
        body.to_string(),
    )
}
#[test]
fn dashboard_is_separate_read_only_filtered_and_preserves_usage_quality() {
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_address = proxy.local_addr().unwrap();
    let dashboard = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = dashboard.local_addr().unwrap();
    let directory =
        std::env::temp_dir().join(format!("transit-dashboard-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    let config = directory.join("transit.toml");
    let database = directory.join("audit.sqlite3");
    fs::write(
        &config,
        format!(
            r#"version=1
default_provider="fixture"
[server]
listen="{proxy_address}"
[logging]
directory="{}"
[audit]
enabled=true
database="{}"
[dashboard]
enabled=true
listen="{address}"
[admission]
strict=true
[providers.fixture]
origin="http://127.0.0.1:1"
"#,
            directory.join("logs").display(),
            database.display()
        ),
    )
    .unwrap();
    drop(proxy);
    drop(dashboard);
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
        if TcpStream::connect(address).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(20));
    }
    let c = Connection::open(&database).unwrap();
    c.busy_timeout(Duration::from_secs(1)).unwrap();
    let timestamp = 1_700_000_000_000i64;
    let ids: Vec<String> = (1..=4)
        .map(|n| format!("00000000-0000-4000-8000-{n:012}"))
        .collect();
    for (index, id) in ids.iter().enumerate() {
        let (provider, model, outcome, usage, input, output, total) = match index {
            0 => (
                "primary",
                "fixture-chat",
                "completed",
                "complete",
                Some(100),
                Some(20),
                Some(120),
            ),
            1 => (
                "primary",
                "fixture-chat",
                "failed",
                "partial",
                Some(25),
                None,
                Some(25),
            ),
            2 => (
                "secondary",
                "fixture-other",
                "completed",
                "missing",
                None,
                None,
                None,
            ),
            _ => (
                "primary",
                "fixture-chat",
                "in_progress",
                "missing",
                None,
                None,
                None,
            ),
        };
        c.execute("INSERT INTO requests(request_id,started_at_ms,provider,request_path,requested_model,resolved_model,outcome,usage_status,input_tokens,output_tokens,total_tokens,duration_ms,error_message) VALUES(?1,?2,?3,'/v1/responses',?4,?4,?5,?6,?7,?8,?9,1000,?10)",params![id,timestamp,provider,model,outcome,usage,input,output,total,"<img src=x onerror=alert(1)>"]).unwrap();
    }
    let window = format!("from_ms={}&to_ms={}", timestamp - 1000, timestamp + 1000);
    c.execute(
        "UPDATE requests SET upstream_protocol='openai_responses'",
        [],
    )
    .unwrap();
    let (code, headers, body) = get(address, &format!("/api/stats?{window}&limit=2"), "");
    assert_eq!(code, 200, "{body}");
    assert!(headers.to_lowercase().contains("cache-control: no-store"));
    assert!(headers.to_lowercase().contains("content-security-policy:"));
    let value: Value = serde_json::from_str(&body).unwrap();
    let s = &value["summary"];
    assert_eq!(s["requests"], 4);
    assert_eq!(s["pending"], 1);
    assert_eq!(s["completed"], 2);
    assert_eq!(s["unsuccessful"], 1);
    assert_eq!(s["complete_usage"], 1);
    assert_eq!(s["partial_usage"], 1);
    assert_eq!(s["missing_usage"], 1);
    assert_eq!(s["total_tokens"], 120);
    assert_eq!(s["completed_generation_requests"], 2);
    assert_eq!(s["completed_usage_issues"], 1);
    for (outcome, issues) in [
        ("completed", 1),
        ("cancelled", 0),
        ("failed", 0),
        ("incomplete", 0),
        ("interrupted", 0),
        ("in_progress", 0),
    ] {
        c.execute(
            "UPDATE requests SET outcome=?1 WHERE request_id=?2",
            params![outcome, ids[2]],
        )
        .unwrap();
        let (_, _, body) = get(address, &format!("/api/stats?{window}"), "");
        let measured: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            measured["summary"]["completed_usage_issues"], issues,
            "{outcome}"
        );
        assert_eq!(
            measured["summary"]["cancelled"],
            i64::from(outcome == "cancelled")
        );
    }
    // Non-generation endpoints legitimately have no model token usage.
    c.execute(
        "UPDATE requests SET outcome='completed',upstream_protocol='models' WHERE request_id=?1",
        [&ids[2]],
    )
    .unwrap();
    let (_, _, body) = get(address, &format!("/api/stats?{window}"), "");
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["summary"]["completed_usage_issues"],
        0
    );
    c.execute(
        "UPDATE requests SET upstream_protocol='openai_responses' WHERE request_id=?1",
        [&ids[2]],
    )
    .unwrap();
    // Missing cache fields stay unknown; a reported zero is a valid miss.
    // Out-of-range counters must not produce a misleading hit ratio.
    for (cached, samples, denominator, hits) in [
        (None, 0, 0, 0),
        (Some(0), 1, 100, 0),
        (Some(80), 1, 100, 80),
        (Some(101), 0, 0, 0),
        (Some(-1), 0, 0, 0),
    ] {
        c.execute(
            "UPDATE requests SET cache_read_tokens=?1 WHERE request_id=?2",
            params![cached, ids[0]],
        )
        .unwrap();
        // A partial response may contain a cache count but is not a complete sample.
        c.execute(
            "UPDATE requests SET cache_read_tokens=20 WHERE request_id=?1",
            [&ids[1]],
        )
        .unwrap();
        let (_, _, body) = get(address, &format!("/api/stats?{window}"), "");
        let measured: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(measured["summary"]["cache_usage"], samples);
        assert_eq!(measured["summary"]["cache_input_tokens"], denominator);
        assert_eq!(measured["summary"]["cache_read_tokens"], hits);
        assert_eq!(measured["summary"]["input_tokens"], 100);
        let request = measured["requests"]
            .as_array()
            .unwrap()
            .iter()
            .find(|request| request["request_id"] == ids[0])
            .unwrap();
        assert_eq!(request["cache_read_tokens"], serde_json::json!(cached));
    }
    assert_eq!(value["requests"].as_array().unwrap().len(), 2);
    let cursor = &value["next_cursor"];
    let (_, _, body) = get(
        address,
        &format!(
            "/api/stats?{window}&limit=2&before_ms={}&before_id={}",
            cursor["before_ms"],
            cursor["before_id"].as_str().unwrap()
        ),
        "",
    );
    let next: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(next["requests"].as_array().unwrap().len(), 2);
    assert!(next["next_cursor"].is_null());
    assert_ne!(
        next["requests"][0]["request_id"],
        value["requests"][0]["request_id"]
    );
    let (_, _, body) = get(
        address,
        &format!("/api/stats?{window}&provider=secondary"),
        "",
    );
    let filtered: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(filtered["summary"]["requests"], 1);
    let (_, _, body) = get(
        address,
        &format!("/api/stats?{window}&model=%27%20OR%201%3D1--"),
        "",
    );
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["summary"]["requests"],
        0
    );
    c.execute(
        "UPDATE requests SET cache_read_tokens=80,cache_write_tokens=10 WHERE request_id=?1",
        [&ids[0]],
    )
    .unwrap();
    let (_, _, body) = get(address, &format!("/api/requests/{}", ids[0]), "");
    let detail: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(detail["error_message"], "<img src=x onerror=alert(1)>");
    assert_eq!(detail["cache_read_tokens"], 80);
    assert_eq!(detail["cache_write_tokens"], 10);
    assert!(detail.get("cached_input_tokens").is_none());
    assert!(detail.get("cache_write_input_tokens").is_none());
    // An aligned chart must not widen the requested statistics window.
    let hour = 3_600_000i64;
    let offset = 330 * 60_000i64;
    let origin = (timestamp + offset) / hour * hour - offset;
    let start = origin + 48 * 60_000;
    let end = start + 24 * hour;
    for (id, time) in ids.iter().zip([start - 1, start, origin + hour, end]) {
        c.execute(
            "UPDATE requests SET started_at_ms=?1 WHERE request_id=?2",
            params![time, id],
        )
        .unwrap();
    }
    let (code, _, body) = get(
        address,
        &format!("/api/stats?from_ms={start}&to_ms={end}&utc_offset_minutes=330"),
        "",
    );
    assert_eq!(code, 200, "{body}");
    let aligned: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(aligned["from_ms"], start);
    assert_eq!(aligned["to_ms"], end);
    assert_eq!(aligned["bucket_ms"], hour);
    assert_eq!(aligned["chart_from_ms"], origin);
    assert_eq!(aligned["chart_to_ms"], origin + 25 * hour);
    assert_eq!(aligned["summary"]["requests"], 2);
    assert_eq!(aligned["series"][0]["time_ms"], origin);
    assert_eq!(aligned["series"][0]["requests"], 1);
    assert_eq!(aligned["series"][1]["time_ms"], origin + hour);
    assert_eq!(aligned["series"][1]["requests"], 1);
    assert_eq!(aligned["series"].as_array().unwrap().len(), 2);
    for path in [
        "/api/stats?from_ms=10&to_ms=1",
        "/api/stats?limit=101",
        "/api/stats?utc_offset_minutes=841",
        "/api/stats?utc_offset_minutes=-841",
        "/api/stats?from_ms=9223372036854775806&to_ms=9223372036854775807",
        "/api/stats?outcome=unknown",
        "/api/stats?from_ms=0&to_ms=99999999999999",
        "/api/requests/invalid",
    ] {
        assert_eq!(get(address, path, "").0, 400, "{path}");
    }
    assert_eq!(
        get(
            address,
            "/api/requests/11111111-1111-4111-8111-111111111111",
            ""
        )
        .0,
        404
    );
    assert_eq!(
        get(address, "/api/stats", "Sec-Fetch-Site: cross-site\r\n").0,
        403
    );
    let (_, _, html) = get(address, "/", "");
    assert!(html.contains("/dashboard.js"));
    assert!(html.contains("请求观测"));
    assert!(html.contains("type=\"module\""));
    assert!(html.contains("id=\"language-switch\""));
    // ESM dependencies must ship with the binary and use a JavaScript MIME type.
    for (path, marker) in [
        ("/dashboard.js", "./i18n.js"),
        ("/i18n.js", "./locales.js"),
        ("/locales.js", "export const languages"),
    ] {
        let (code, headers, script) = get(address, path, "");
        assert_eq!(code, 200, "{path}");
        assert!(headers.contains("text/javascript"), "{path}");
        assert!(headers.to_lowercase().contains("content-security-policy:"));
        assert!(script.contains(marker), "{path}");
        assert_eq!(get(proxy_address, path, "").0, 404, "{path}");
    }
    assert_eq!(get(proxy_address, "/api/stats", "").0, 404);
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM requests", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        4
    );
    #[cfg(unix)]
    {
        assert!(
            Command::new("kill")
                .args(["-TERM", &service.child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(status) = service.child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < deadline, "graceful stop timed out");
            thread::sleep(Duration::from_millis(20));
        }
    }
}
