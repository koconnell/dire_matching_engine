//! Persistence: save and load engine state (+ market state) to a file.
//! Enables recovery after restart: instruments, resting orders, and next IDs are restored.

use crate::engine::EngineSnapshot;
use std::path::Path;

/// Full persisted state: engine snapshot and market state (Open/Halted/Closed).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistedState {
    pub engine: EngineSnapshot,
    pub market_state: String,
}

/// File-based persistence: one JSON file. Save after state changes; load on startup.
#[derive(Clone, Debug)]
pub struct FilePersistence {
    path: std::path::PathBuf,
}

impl FilePersistence {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Save state to file. Overwrites existing file.
    pub fn save(&self, state: &PersistedState) -> Result<(), String> {
        let json = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| e.to_string())
    }

    /// Load state from file. Returns None if file does not exist or is invalid.
    pub fn load(&self) -> Result<Option<PersistedState>, String> {
        let data = match std::fs::read_to_string(&self.path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let state: PersistedState = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        Ok(Some(state))
    }
}
