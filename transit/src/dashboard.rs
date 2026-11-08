//! Read-only management surface. SQLite work never runs on the proxy runtime.
use crate::config::{Audit, default_state_dir};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use rusqlite::{
    Connection, OpenFlags, params_from_iter,
    types::{Value as SqlValue, ValueRef},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    net::IpAddr,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tokio::sync::oneshot;

const WINDOW_MS: i64 = 366 * 86_400_000;
const BUCKET_MINUTES: [i64; 14] = [
    1, 2, 5, 15, 30, 60, 120, 180, 360, 720, 1440, 2880, 7200, 14400,
];

#[derive(Debug)]
struct Timeline {
    from_ms: i64,
    to_ms: i64,
    bucket_ms: i64,
}
impl Timeline {
    fn new(start: i64, end: i64, utc_offset_minutes: i32) -> Self {
        let offset_ms = i64::from(utc_offset_minutes) * 60_000;
        BUCKET_MINUTES
            .into_iter()
            .map(|minutes| {
                let bucket_ms = minutes * 60_000;
                // Align to wall-clock boundaries using the browser's UTC offset.
                // The multi-day grid is anchored at local 1970-01-01 midnight.
                let remainder = (start.rem_euclid(bucket_ms) + offset_ms).rem_euclid(bucket_ms);
                let from_ms = start - remainder;
                let count = (end - from_ms + bucket_ms - 1) / bucket_ms;
                Self {
                    from_ms,
                    to_ms: from_ms + count * bucket_ms,
                    bucket_ms,
                }
            })
            .min_by_key(|timeline| {
                let count = (timeline.to_ms - timeline.from_ms) / timeline.bucket_ms;
                let distance = if count < 20 {
                    20 - count
                } else {
                    (count - 30).max(0)
                };
                (distance, (count - 25).abs(), timeline.bucket_ms)
            })
            .expect("bucket intervals are nonempty")
    }
}
const MODEL: &str = "COALESCE(response_model,resolved_model,requested_model)";
const FILTER: &str = "started_at_ms >= ?1 AND started_at_ms < ?2
    AND (?3 IS NULL OR provider = ?3)
    AND (?4 IS NULL OR requested_model = ?4 OR resolved_model = ?4 OR response_model = ?4)
    AND (?5 IS NULL OR request_path = ?5)
    AND (?6 IS NULL OR outcome = ?6)";

#[derive(Clone)]
pub struct Reader {
    sender: mpsc::SyncSender<Job>,
}
pub struct Guard {
    worker: thread::JoinHandle<()>,
}
struct Job {
    query: Read,
    deadline: Instant,
    reply: oneshot::Sender<Result<Value, String>>,
}
enum Read {
    Stats(Filters),
    Detail(String),
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filters {
    from_ms: Option<i64>,
    to_ms: Option<i64>,
    utc_offset_minutes: Option<i32>,
    provider: Option<String>,
    model: Option<String>,
    path: Option<String>,
    outcome: Option<String>,
    before_ms: Option<i64>,
    before_id: Option<String>,
    limit: Option<usize>,
}
impl Filters {
    fn validate(&mut self) -> Result<(), &'static str> {
        let end = self.to_ms.unwrap_or_else(crate::audit::now_ms);
        let start = self.from_ms.unwrap_or(end.saturating_sub(86_400_000));
        if start < 0
            || end <= start
            || end.saturating_sub(start) > WINDOW_MS
            || end > i64::MAX - 10 * 86_400_000
        {
            return Err("时间范围须为 1 毫秒至 366 天");
        }
        if self
            .utc_offset_minutes
            .is_some_and(|offset| !(-840..=840).contains(&offset))
        {
            return Err("无效的 UTC 时区偏移");
        }
        self.from_ms = Some(start);
        self.to_ms = Some(end);
        for (field, maximum) in [
            (&mut self.provider, 128),
            (&mut self.model, 256),
            (&mut self.path, 1024),
        ] {
            if field.as_ref().is_some_and(|s| s.is_empty()) {
                *field = None;
            }
            if field
                .as_ref()
                .is_some_and(|s| s.len() > maximum || s.chars().any(char::is_control))
            {
                return Err("筛选字段过长或包含控制字符");
            }
        }
        if self.outcome.as_deref() == Some("") {
            self.outcome = None;
        }
        if self.outcome.as_deref().is_some_and(|s| {
            ![
                "completed",
                "failed",
                "incomplete",
                "cancelled",
                "interrupted",
                "in_progress",
            ]
            .contains(&s)
        }) {
            return Err("未知请求结果");
        }
        if self.limit.is_some_and(|v| v == 0 || v > 100)
            || self.before_ms.is_some() != self.before_id.is_some()
            || self.before_ms.is_some_and(|v| v < start || v >= end)
            || self
                .before_id
                .as_ref()
                .is_some_and(|v| uuid::Uuid::parse_str(v).is_err())
        {
            return Err("无效的分页参数");
        }
        Ok(())
    }
    fn parameters(&self) -> Vec<SqlValue> {
        vec![
            self.from_ms.unwrap().into(),
            self.to_ms.unwrap().into(),
            self.provider.clone().into(),
            self.model.clone().into(),
            self.path.clone().into(),
            self.outcome.clone().into(),
        ]
    }
}

pub fn initialize(settings: &Audit) -> Result<(Reader, Guard), String> {
    let path = settings
        .database
        .clone()
        .unwrap_or(default_state_dir()?.join("transit.sqlite3"));
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| "cannot open dashboard read-only database")?;
    connection
        .busy_timeout(Duration::from_millis(100))
        .map_err(|_| "cannot configure dashboard reader")?;
    connection
        .execute_batch("PRAGMA query_only=ON; PRAGMA cache_size=-2048;")
        .map_err(|_| "cannot configure dashboard read-only database")?;
    let (sender, receiver) = mpsc::sync_channel::<Job>(8);
    let worker = thread::Builder::new()
        .name("transit-dashboard-db".into())
        .spawn(move || {
            for job in receiver {
                if job.reply.is_closed() {
                    continue;
                }
                let deadline = job.deadline;
                if Instant::now() >= deadline {
                    let _ = job.reply.send(Err("query deadline exceeded".into()));
                    continue;
                }
                if connection
                    .progress_handler(1000, Some(move || Instant::now() >= deadline))
                    .is_err()
                {
                    tracing::error!(
                        event = "dashboard_query_failed",
                        reason = "cannot install query deadline"
                    );
                    let _ = job.reply.send(Err("query deadline unavailable".into()));
                    continue;
                }
                let result = match job.query {
                    Read::Stats(filters) => statistics(&connection, &filters),
                    Read::Detail(id) => rows(
                        &connection,
                        "SELECT * FROM requests WHERE request_id=?1",
                        &[id.into()],
                    )
                    .map(|mut rows| rows.pop().unwrap_or(Value::Null)),
                };
                let reset = connection.progress_handler(0, None::<fn() -> bool>);
                if result.is_err() {
                    tracing::warn!(
                        event = "dashboard_query_failed",
                        reason = "SQLite query failed or exceeded deadline"
                    );
                }
                let _ = job
                    .reply
                    .send(result.map_err(|_| "statistics query unavailable".into()));
                if reset.is_err() {
                    tracing::error!(
                        event = "dashboard_query_failed",
                        reason = "cannot reset query deadline"
                    );
                    break;
                }
            }
        })
        .map_err(|_| "cannot start dashboard reader thread")?;
    Ok((Reader { sender }, Guard { worker }))
}
impl Guard {
    // All Reader clones must be dropped before joining.
    pub async fn shutdown(self) {
        if !matches!(
            tokio::task::spawn_blocking(move || self.worker.join()).await,
            Ok(Ok(()))
        ) {
            tracing::error!(event = "dashboard_reader_panicked");
        }
    }
}
impl Reader {
    async fn read(&self, query: Read) -> Result<Value, (StatusCode, Json<Value>)> {
        let (reply, receiver) = oneshot::channel();
        self.sender
            .try_send(Job {
                query,
                deadline: Instant::now() + Duration::from_secs(3),
                reply,
            })
            .map_err(|_| api_error(StatusCode::SERVICE_UNAVAILABLE, "查询繁忙，请稍后重试"))?;
        match tokio::time::timeout(Duration::from_secs(4), receiver).await {
            Ok(Ok(Ok(value))) => Ok(value),
            _ => Err(api_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "查询暂不可用，请稍后重试",
            )),
        }
    }
}
fn api_error(code: StatusCode, message: &str) -> (StatusCode, Json<Value>) {
    (code, Json(json!({"error":message})))
}
fn rows(
    connection: &Connection,
    sql: &str,
    parameters: &[SqlValue],
) -> rusqlite::Result<Vec<Value>> {
    let mut statement = connection.prepare(sql)?;
    let names: Vec<String> = statement
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    statement
        .query_map(params_from_iter(parameters), |row| {
            let mut object = serde_json::Map::new();
            for (index, name) in names.iter().enumerate() {
                let value = match row.get_ref(index)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(v) => json!(v),
                    ValueRef::Real(v) => json!(v),
                    ValueRef::Text(v) => Value::String(String::from_utf8_lossy(v).into_owned()),
                    ValueRef::Blob(_) => Value::Null,
                };
                object.insert(name.clone(), value);
            }
            Ok(Value::Object(object))
        })?
        .collect()
}
fn statistics(connection: &Connection, filters: &Filters) -> rusqlite::Result<Value> {
    let transaction = connection.unchecked_transaction()?;
    let parameters = filters.parameters();
    let summary = rows(&transaction,&format!("SELECT COUNT(*) AS requests,
        COALESCE(SUM(outcome='completed'),0) AS completed,
        COALESCE(SUM(outcome IN ('failed','incomplete','cancelled','interrupted')),0) AS unsuccessful,
        COALESCE(SUM(outcome='cancelled'),0) AS cancelled,
        COALESCE(SUM(outcome IN ('failed','incomplete','interrupted')),0) AS abnormal,
        COALESCE(SUM(outcome='in_progress'),0) AS pending,
        COALESCE(SUM(usage_status='complete'),0) AS complete_usage,
        COALESCE(SUM(usage_status='partial'),0) AS partial_usage,
        COALESCE(SUM((usage_status IS NULL OR usage_status='missing') AND outcome!='in_progress'),0) AS missing_usage,
        COALESCE(SUM(outcome='completed' AND upstream_protocol IN
            ('openai_chat','openai_completions','openai_responses','anthropic_messages','gemini_generate_content')),0) AS completed_generation_requests,
        COALESCE(SUM(outcome='completed' AND upstream_protocol IN
            ('openai_chat','openai_completions','openai_responses','anthropic_messages','gemini_generate_content')
            AND (usage_status IS NULL OR usage_status!='complete')),0) AS completed_usage_issues,
        AVG(CASE WHEN outcome!='in_progress' THEN duration_ms END) AS avg_duration_ms,
        AVG(CASE WHEN outcome!='in_progress' THEN first_body_byte_ms END) AS avg_first_byte_ms,
        COALESCE(SUM(CASE WHEN usage_status='complete' THEN input_tokens END),0) AS input_tokens,
        COALESCE(SUM(CASE WHEN usage_status='complete' THEN output_tokens END),0) AS output_tokens,
        COALESCE(SUM(CASE WHEN usage_status='complete' THEN total_tokens END),0) AS total_tokens,
        COALESCE(SUM(usage_status='complete' AND cache_read_tokens IS NOT NULL
            AND cache_read_tokens >= 0 AND cache_read_tokens <= input_tokens),0) AS cache_usage,
        COALESCE(SUM(CASE WHEN usage_status='complete' AND cache_read_tokens >= 0
            AND cache_read_tokens <= input_tokens THEN input_tokens END),0) AS cache_input_tokens,
        COALESCE(SUM(CASE WHEN usage_status='complete' AND cache_read_tokens >= 0
            AND cache_read_tokens <= input_tokens THEN cache_read_tokens END),0) AS cache_read_tokens,
        COALESCE(SUM(CASE WHEN usage_status='complete' THEN reasoning_tokens END),0) AS reasoning_tokens
        FROM requests WHERE {FILTER}"), &parameters)?.pop().unwrap();
    let start = filters.from_ms.unwrap();
    let end = filters.to_ms.unwrap();
    let timeline = Timeline::new(start, end, filters.utc_offset_minutes.unwrap_or(0));
    let mut series_parameters = parameters.clone();
    series_parameters.push(timeline.from_ms.into());
    series_parameters.push(timeline.bucket_ms.into());
    let series = rows(&transaction,&format!("SELECT ((started_at_ms-?7)/?8)*?8+?7 AS time_ms,
        COUNT(*) AS requests,COALESCE(SUM(outcome IN ('failed','incomplete','cancelled','interrupted')),0) AS unsuccessful,
        COALESCE(SUM(CASE WHEN usage_status='complete' THEN total_tokens END),0) AS total_tokens
        FROM requests WHERE {FILTER} GROUP BY time_ms ORDER BY time_ms"),&series_parameters)?;
    let groups = rows(&transaction,&format!("SELECT {MODEL} AS model,provider,COUNT(*) AS requests,
        COALESCE(SUM(CASE WHEN usage_status='complete' THEN total_tokens END),0) AS total_tokens,
        COALESCE(SUM(usage_status='complete'),0) AS complete_usage
        FROM requests WHERE {FILTER} GROUP BY {MODEL},provider ORDER BY requests DESC,model,provider LIMIT 12"),&parameters)?;
    let mut list_parameters = parameters.clone();
    list_parameters.push(filters.before_ms.into());
    list_parameters.push(filters.before_id.clone().into());
    let limit = filters.limit.unwrap_or(50);
    list_parameters.push(((limit + 1) as i64).into());
    let mut requests = rows(&transaction,&format!("SELECT request_id,started_at_ms,provider,request_path,
        requested_model,resolved_model,response_model,outcome,http_status,duration_ms,first_body_byte_ms,
        input_tokens,output_tokens,total_tokens,usage_status,upstream_protocol,error_code
        FROM requests WHERE {FILTER} AND (?7 IS NULL OR started_at_ms < ?7 OR (started_at_ms = ?7 AND request_id < ?8))
        ORDER BY started_at_ms DESC,request_id DESC LIMIT ?9"),&list_parameters)?;
    let more = requests.len() > limit;
    requests.truncate(limit);
    let cursor = if more {
        requests
            .last()
            .map(|row| json!({"before_ms":row["started_at_ms"],"before_id":row["request_id"]}))
    } else {
        None
    };
    let option_rows = rows(
        &transaction,
        "SELECT DISTINCT provider AS value,'providers' AS kind FROM requests WHERE started_at_ms>=?1 AND started_at_ms<?2 AND provider IS NOT NULL ORDER BY value LIMIT 200",
        &parameters[..2],
    )?;
    let model_rows = rows(
        &transaction,
        "SELECT requested_model AS value FROM requests WHERE started_at_ms>=?1 AND started_at_ms<?2 AND requested_model IS NOT NULL UNION SELECT resolved_model FROM requests WHERE started_at_ms>=?1 AND started_at_ms<?2 AND resolved_model IS NOT NULL UNION SELECT response_model FROM requests WHERE started_at_ms>=?1 AND started_at_ms<?2 AND response_model IS NOT NULL ORDER BY value LIMIT 200",
        &parameters[..2],
    )?;
    let path_rows = rows(
        &transaction,
        "SELECT DISTINCT request_path AS value FROM requests WHERE started_at_ms>=?1 AND started_at_ms<?2 AND request_path IS NOT NULL ORDER BY value LIMIT 100",
        &parameters[..2],
    )?;
    transaction.commit()?;
    Ok(
        json!({"summary":summary,"series":series,"groups":groups,"requests":requests,"next_cursor":cursor,
        "from_ms":start,"to_ms":end,"bucket_ms":timeline.bucket_ms,
        "chart_from_ms":timeline.from_ms,"chart_to_ms":timeline.to_ms,
        "utc_offset_minutes":filters.utc_offset_minutes.unwrap_or(0),"generated_at_ms":crate::audit::now_ms(),
        "options":{"providers":option_rows.iter().map(|r| &r["value"]).collect::<Vec<_>>(),
        "models":model_rows.iter().map(|r| &r["value"]).collect::<Vec<_>>(),
        "paths":path_rows.iter().map(|r| &r["value"]).collect::<Vec<_>>()}}),
    )
}

pub fn router(reader: Reader) -> Router {
    Router::new()
        .route(
            "/",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    include_str!("../web/index.html"),
                )
            }),
        )
        .route(
            "/dashboard.css",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../web/dashboard.css"),
                )
            }),
        )
        .route(
            "/dashboard.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../web/dashboard.js"),
                )
            }),
        )
        .route("/api/stats", get(stats))
        .route("/api/requests/{id}", get(detail))
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(DefaultBodyLimit::max(1024))
        .layer(middleware::from_fn(private_headers))
        .with_state(reader)
}
async fn stats(
    State(reader): State<Reader>,
    Query(mut filters): Query<Filters>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    filters
        .validate()
        .map_err(|s| api_error(StatusCode::BAD_REQUEST, s))?;
    reader.read(Read::Stats(filters)).await.map(Json)
}
async fn detail(
    State(reader): State<Reader>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if uuid::Uuid::parse_str(&id).is_err() {
        return Err(api_error(StatusCode::BAD_REQUEST, "无效的请求 ID"));
    }
    let value = reader.read(Read::Detail(id)).await?;
    if value.is_null() {
        return Err(api_error(
            StatusCode::NOT_FOUND,
            "未找到记录，可能已过保留期",
        ));
    }
    Ok(Json(value))
}
async fn private_headers(request: Request, next: Next) -> Response {
    // IP-literal hosts prevent an arbitrary website from rebinding its DNS to this private reader.
    // Named public reverse proxies should terminate authentication and rewrite Host to an IP.
    let valid_host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<axum::http::uri::Authority>().ok())
        .is_some_and(|a| {
            a.host() == "localhost" || a.host().trim_matches(['[', ']']).parse::<IpAddr>().is_ok()
        });
    let cross_site = request
        .headers()
        .get("sec-fetch-site")
        .is_some_and(|v| v == "cross-site");
    let mut response = if !valid_host || cross_site {
        StatusCode::FORBIDDEN.into_response()
    } else {
        next.run(request).await
    };
    for (name, value) in [
        ("cache-control", "no-store"),
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        (
            "content-security-policy",
            "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'",
        ),
    ] {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static(name),
            axum::http::HeaderValue::from_static(value),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;

    #[test]
    fn common_windows_use_stable_intervals_near_twenty_to_thirty_bars() {
        let end = 500 * DAY + 12 * HOUR + 48 * MINUTE;
        for (span, bucket, count) in [
            (HOUR, 2 * MINUTE, 30),
            (DAY, HOUR, 25),
            (7 * DAY, 6 * HOUR, 29),
            (30 * DAY, DAY, 31),
        ] {
            let timeline = Timeline::new(end - span, end, 480);
            assert_eq!(timeline.bucket_ms, bucket);
            assert_eq!((timeline.to_ms - timeline.from_ms) / bucket, count);
        }
        let original = Timeline::new(end - DAY, end, 480);
        let refreshed = Timeline::new(end - DAY + MINUTE, end + MINUTE, 480);
        assert_eq!(original.from_ms, refreshed.from_ms);
        assert_eq!(original.to_ms, refreshed.to_ms);
    }

    #[test]
    fn buckets_align_to_local_boundaries_and_include_only_intersecting_periods() {
        let end = 500 * DAY + 12 * HOUR + 48 * MINUTE;
        for offset in [-840, -210, 0, 330, 345, 480, 840] {
            for span in [1, HOUR, DAY, 3 * DAY, 30 * DAY, WINDOW_MS] {
                let start = end - span;
                let timeline = Timeline::new(start, end, offset);
                let shift = i64::from(offset) * MINUTE;
                assert_eq!((timeline.from_ms + shift).rem_euclid(timeline.bucket_ms), 0);
                assert_eq!((timeline.to_ms + shift).rem_euclid(timeline.bucket_ms), 0);
                assert!(timeline.from_ms <= start && start < timeline.from_ms + timeline.bucket_ms);
                assert!(timeline.to_ms - timeline.bucket_ms < end && end <= timeline.to_ms);
            }
        }
        let aligned_end = 500 * DAY + 13 * HOUR;
        let exact = Timeline::new(aligned_end - DAY, aligned_end, 480);
        assert_eq!(exact.to_ms, aligned_end);
        assert_eq!(exact.from_ms, aligned_end - DAY);
        let partial = Timeline::new(end - DAY, end, 480);
        assert_eq!(partial.to_ms, aligned_end);
        assert_eq!(partial.to_ms - partial.bucket_ms, 500 * DAY + 12 * HOUR);
    }
}
