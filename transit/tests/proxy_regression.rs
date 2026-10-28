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
            let bad = header.starts_with(b"GET /bad ");
            let (status, body) = if bad {
                (
                    "400 Bad Request",
                    r#"{"error":{"type":"invalid_request_error","message":"Unsupported test parameter"}}"#,
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
            "version = 1\ndefault_provider = 'fixture'\n[server]\nlisten = '{address}'\n[logging]\ndirectory = '{}'\n[providers.fixture]\norigin = 'http://{upstream_address}'\n",
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
    let failure = request(address, "/bad");
    assert!(failure.starts_with("HTTP/1.1 400"), "{failure}");
    assert!(failure.ends_with(
        r#"{"error":{"type":"invalid_request_error","message":"Unsupported test parameter"}}"#
    ));
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
    assert!(
        records
            .iter()
            .any(|v| v["fields"]["event"] == "upstream_error_detail"
                && v["fields"]["message"] == "Unsupported test parameter"),
        "{records:?}"
    );
}
