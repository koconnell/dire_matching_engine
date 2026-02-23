//! Phase 3 §3: Structured audit trail for material actions.
//!
//! Events: order submit/cancel/modify, config changes, market state changes, emergency halt.
//! Format: JSON with timestamp, actor, action, resource, outcome. Sink: stdout or pluggable (e.g. test mock).

use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// Single audit record: one line of JSON per event.
#[derive(Clone, Debug, Serialize)]
pub struct AuditEvent {
    /// Unix timestamp (seconds since epoch). Log aggregators can convert to ISO8601.
    pub timestamp_secs: u64,
    /// Who performed the action (e.g. API key id, "fix", "anonymous").
    pub actor: String,
    /// Action type: order_submit, order_cancel, order_modify, config_change, market_state_change, emergency_halt.
    pub action: String,
    /// Resource identifiers (e.g. order_id, instrument_id). Flexible for different action types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<serde_json::Value>,
    /// Outcome: success, rejected, error.
    pub outcome: String,
}

impl AuditEvent {
    pub fn now(actor: impl Into<String>, action: impl Into<String>, resource: Option<serde_json::Value>, outcome: impl Into<String>) -> Self {
        let timestamp_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            timestamp_secs,
            actor: actor.into(),
            action: action.into(),
            resource,
            outcome: outcome.into(),
        }
    }
}

/// Sink for audit events. Implementations write to stdout, file, or in-memory (tests).
pub trait AuditSink: Send + Sync {
    fn emit(&self, event: &AuditEvent);
}

/// Writes one JSON line per event to stdout. Safe to use from multiple threads.
pub struct StdoutAuditSink;

impl AuditSink for StdoutAuditSink {
    fn emit(&self, event: &AuditEvent) {
        match serde_json::to_string(event) {
            Ok(line) => println!("{}", line),
            Err(_) => {}
        }
    }
}

/// In-memory sink that stores events for tests. Clone shares the same backing buffer.
#[derive(Clone)]
pub struct InMemoryAuditSink {
    events: std::sync::Arc<std::sync::Mutex<Vec<AuditEvent>>>,
}

impl InMemoryAuditSink {
    pub fn new() -> Self {
        Self {
            events: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn events(&self) -> Vec<AuditEvent> {
        self.events.lock().expect("lock").clone()
    }

    pub fn clear(&self) {
        self.events.lock().expect("lock").clear();
    }
}

impl Default for InMemoryAuditSink {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditSink for InMemoryAuditSink {
    fn emit(&self, event: &AuditEvent) {
        self.events.lock().expect("lock").push(event.clone());
    }
}
