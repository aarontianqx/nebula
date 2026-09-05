//! Per-session event journal: every session gets a JSONL file under
//! `logs/sessions/` recording its full protocol traffic — downstream packets,
//! upstream sends (both the game's own UI-driven sends and automation sends,
//! distinguished by a `self` flag) — plus session start/stop markers.
//!
//! Purpose: a readable, per-account ground truth for debugging ("what
//! happened on this account at 19:10?") and for recon of new tasks (play a
//! feature manually, read the journal, write the template).
//!
//! Writes go through a channel + background task so the hot path (bridge
//! forwarder) never blocks on disk IO. Journaling is best-effort: a full
//! channel drops records (with a warn) rather than slowing the session.

use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::infrastructure::config::paths;

/// Downstream protocols excluded from the journal — periodic floods with no
/// diagnostic value (heartbeats, sea-trade ticks, attribute bursts).
const EXCLUDED_DOWNSTREAM: &[&str] = &[
    "S_2_C_KEEP_ALIVE",
    "S_2_C_SEA_TRADE_START",
    "S_2_C_UPDATE_GENARALINFO",
    "S_2_C_GENERAL_EXTRA_ATTR_COMPOSITIONS",
];

/// Channel capacity for journal records; on overflow, records are dropped.
const JOURNAL_CHANNEL: usize = 2048;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum JournalRecord {
    /// Session lifecycle or other notable marker.
    Meta {
        ts: String,
        event: String,
        detail: String,
    },
    /// Downstream packet (server -> client), reported by the page bridge.
    Down {
        ts: String,
        id: u32,
        name: Option<String>,
        data: Value,
    },
    /// Upstream send (client -> server). `self`=true when sent by the
    /// automation (via the bridge's send), false when the game client itself
    /// sent it (e.g. a human clicking in the UI).
    Up {
        ts: String,
        id: u32,
        name: Option<String>,
        #[serde(rename = "self")]
        self_: bool,
        data: Value,
    },
}

fn now_ts() -> String {
    humantime::format_rfc3339_millis(std::time::SystemTime::now()).to_string()
}

/// Handle to a session's journal. Cloneable; clones share the writer task.
#[derive(Clone)]
pub struct SessionJournal {
    tx: mpsc::Sender<JournalRecord>,
}

impl SessionJournal {
    /// Start a journal for a session. `account_desc` goes into the file name
    /// (sanitized) and the opening meta record. Returns None if the log
    /// directory is unusable — journaling must never break a session.
    pub fn start(account_desc: &str) -> Option<Self> {
        let dir = paths::log_dir().join("sessions");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!("Journal disabled: cannot create {:?}: {}", dir, e);
            return None;
        }
        let safe_desc: String = account_desc
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | ' ' => '_',
                _ => c,
            })
            .collect();
        let file_name = format!(
            "{}_{}.jsonl",
            humantime::format_rfc3339_seconds(std::time::SystemTime::now())
                .to_string()
                .replace(':', "-"),
            safe_desc
        );
        let path = dir.join(file_name);

        let (tx, mut rx) = mpsc::channel::<JournalRecord>(JOURNAL_CHANNEL);
        tokio::spawn(async move {
            let file = match tokio::fs::File::create(&path).await {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!("Journal disabled: cannot create {:?}: {}", path, e);
                    // Drain the channel so senders don't spam warnings.
                    while rx.recv().await.is_some() {}
                    return;
                }
            };
            let mut writer = tokio::io::BufWriter::new(file);
            use tokio::io::AsyncWriteExt;
            while let Some(record) = rx.recv().await {
                let line = match serde_json::to_string(&record) {
                    Ok(l) => l,
                    Err(_) => continue,
                };
                if writer.write_all(line.as_bytes()).await.is_err()
                    || writer.write_all(b"\n").await.is_err()
                {
                    tracing::warn!("Journal write failed for {:?}, stopping", path);
                    return;
                }
                // Flush per record: journals exist for near-realtime debugging.
                let _ = writer.flush().await;
            }
        });

        let journal = Self { tx };
        journal.meta("session_start", account_desc);
        Some(journal)
    }

    fn send(&self, record: JournalRecord) {
        if self.tx.try_send(record).is_err() {
            tracing::warn!("Journal channel full, dropping record");
        }
    }

    pub fn meta(&self, event: &str, detail: &str) {
        self.send(JournalRecord::Meta {
            ts: now_ts(),
            event: event.to_string(),
            detail: detail.to_string(),
        });
    }

    /// Journal a downstream packet, unless it's on the spam exclusion list.
    pub fn down(&self, id: u32, name: &Option<String>, data: &Value) {
        if let Some(n) = name {
            if EXCLUDED_DOWNSTREAM.contains(&n.as_str()) {
                return;
            }
        }
        self.send(JournalRecord::Down {
            ts: now_ts(),
            id,
            name: name.clone(),
            data: data.clone(),
        });
    }

    /// Journal an upstream send. `by_automation` distinguishes our sends
    /// (bridge `__wardenly.send`) from the game client's own.
    pub fn up(&self, id: u32, name: &Option<String>, by_automation: bool, data: &Value) {
        self.send(JournalRecord::Up {
            ts: now_ts(),
            id,
            name: name.clone(),
            self_: by_automation,
            data: data.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_serialization_shape() {
        let record = JournalRecord::Up {
            ts: "2026-09-05T00:00:00.000Z".to_string(),
            id: 890,
            name: Some("C_2_S_KNIGHT_TOWER_TEAM_NUM".to_string()),
            self_: true,
            data: serde_json::json!({"ident": 1}),
        };
        let line = serde_json::to_string(&record).unwrap();
        assert!(line.contains(r#""kind":"up""#));
        assert!(line.contains(r#""self":true"#));
        assert!(line.contains("C_2_S_KNIGHT_TOWER_TEAM_NUM"));
    }
}
