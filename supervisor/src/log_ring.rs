//! Bounded ring buffer for structured log entries from each worker.
//!
//! All workers stream their stdout/stderr to the supervisor; each line is
//! parsed (best-effort JSON, otherwise wrapped raw) and pushed into this ring.
//! When the ring is full, the oldest entry is dropped (FIFO) — the buffer
//! never blocks the producer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;

/// One log line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Wall-clock timestamp when the supervisor received the line.
    pub timestamp: DateTime<Utc>,
    /// Source child name.
    pub child: String,
    /// Severity (parsed from JSON `level` field if present, else `Info`).
    pub level: LogLevel,
    /// Raw message body.
    pub message: String,
    /// Optional structured fields (echoed from the worker's JSON log line).
    pub fields: Option<serde_json::Value>,
}

/// Log severity.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Verbose tracing output.
    Trace,
    /// Debug information.
    Debug,
    /// Normal informational messages.
    #[default]
    Info,
    /// Recoverable warnings.
    Warn,
    /// Errors that did not stop the worker.
    Error,
    /// Fatal events; child usually exits afterwards.
    Fatal,
}

impl LogLevel {
    /// Parse a level string from a worker JSON log line.
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "trace" => LogLevel::Trace,
            "debug" => LogLevel::Debug,
            "info" => LogLevel::Info,
            "warn" | "warning" => LogLevel::Warn,
            "error" | "err" => LogLevel::Error,
            "fatal" | "critical" => LogLevel::Fatal,
            _ => LogLevel::Info,
        }
    }
}

/// Bounded ring buffer protected by a `Mutex`.
#[derive(Debug)]
pub struct LogRing {
    capacity: usize,
    inner: Mutex<VecDeque<LogEntry>>,
}

impl LogRing {
    /// Construct a ring with the given fixed capacity.
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(16);
        Self {
            capacity: cap,
            inner: Mutex::new(VecDeque::with_capacity(cap)),
        }
    }

    /// Push a new entry. Drops the oldest entry if the ring is full.
    pub fn push(&self, entry: LogEntry) {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if guard.len() == self.capacity {
            guard.pop_front();
        }
        guard.push_back(entry);
    }

    /// Push a raw line received from a worker. The line is best-effort JSON.
    pub fn push_raw(&self, child: &str, line: &str) {
        let entry = parse_log_line(child, line);
        self.push(entry);
    }

    /// Snapshot the most recent `n` entries for a child (or all children when
    /// `child` is `None`). Returns oldest-first.
    pub fn tail(&self, child: Option<&str>, n: usize) -> Vec<LogEntry> {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let it = guard.iter().filter(|e| match child {
            Some(c) => e.child == c,
            None => true,
        });
        let collected: Vec<LogEntry> = it.cloned().collect();
        let start = collected.len().saturating_sub(n);
        collected[start..].to_vec()
    }

    /// Total entries currently in the ring (across all children).
    pub fn len(&self) -> usize {
        let guard = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        guard.len()
    }

    /// True when no entries have been pushed yet.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Maximum capacity of the ring.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

fn parse_log_line(child: &str, line: &str) -> LogEntry {
    // I-19: strip ANSI escape sequences before any further parsing. A
    // worker that uses a TTY-aware logger (Bun, tracing-subscriber with
    // ansi-on-by-default, Python rich, ...) will embed ESC[..] colour
    // codes that otherwise (a) break the JSON-or-raw branch below and
    // (b) bleed into log dumps shown to the user. We strip first so the
    // rest of this function operates on clean text.
    let stripped = strip_ansi_escapes::strip_str(line);
    let trimmed = stripped.trim();
    if trimmed.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let level = v
                .get("level")
                .and_then(|x| x.as_str())
                .map(LogLevel::parse)
                .unwrap_or_default();
            let message = v
                .get("message")
                .or_else(|| v.get("msg"))
                .and_then(|x| x.as_str())
                .unwrap_or(trimmed)
                .to_string();
            return LogEntry {
                timestamp: Utc::now(),
                child: child.to_string(),
                level,
                message,
                fields: Some(v),
            };
        }
    }
    LogEntry {
        timestamp: Utc::now(),
        child: child.to_string(),
        level: LogLevel::Info,
        message: trimmed.to_string(),
        fields: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_drops_oldest_when_full() {
        let r = LogRing::new(16); // capacity floor is 16
        for i in 0..32 {
            r.push_raw("c", &format!("msg-{i}"));
        }
        let tail = r.tail(Some("c"), 100);
        assert_eq!(tail.len(), 16);
        assert!(tail.first().unwrap().message.ends_with("16"));
        assert!(tail.last().unwrap().message.ends_with("31"));
    }

    #[test]
    fn ring_strips_ansi_escapes() {
        // I-19 regression: red-painted message must end up clean.
        let r = LogRing::new(16);
        r.push_raw("c", "\x1b[31mHELLO\x1b[0m world");
        let tail = r.tail(None, 1);
        assert_eq!(tail[0].message, "HELLO world");
    }

    #[test]
    fn ring_strips_ansi_then_parses_json() {
        // I-19: ANSI-wrapped JSON payload must still parse.
        let r = LogRing::new(16);
        r.push_raw(
            "c",
            "\x1b[33m{\"level\":\"warn\",\"message\":\"hi\"}\x1b[0m",
        );
        let tail = r.tail(None, 1);
        assert_eq!(tail[0].level, LogLevel::Warn);
        assert_eq!(tail[0].message, "hi");
    }

    #[test]
    fn ring_parses_json_level() {
        let r = LogRing::new(16);
        r.push_raw("c", r#"{"level":"warn","message":"hi"}"#);
        let tail = r.tail(None, 10);
        assert_eq!(tail[0].level, LogLevel::Warn);
        assert_eq!(tail[0].message, "hi");
    }
}
