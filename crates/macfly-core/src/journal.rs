use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::data_dir;
use crate::error::MacflyError;
use crate::plan::{ensure_private_dir, now_unix};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub timestamp_unix: u64,
    pub plan_id: String,
    pub profile: String,
    pub plan_hash: String,
    pub status: String,
    pub operations_count: usize,
    pub details: Vec<String>,
}

pub struct AuditJournal {
    path: PathBuf,
}

impl AuditJournal {
    pub fn default_journal() -> Self {
        Self {
            path: data_dir().join("journal.jsonl"),
        }
    }

    pub fn append(&self, entry: &JournalEntry) -> Result<(), MacflyError> {
        if let Some(parent) = self.path.parent() {
            ensure_private_dir(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(entry)?;
        writeln!(file, "{line}")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<JournalEntry>, MacflyError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&self.path)?;
        let mut entries: Vec<JournalEntry> = raw
            .lines()
            .filter_map(|l| serde_json::from_str::<JournalEntry>(l).ok())
            .collect();
        entries.reverse();
        entries.truncate(limit);
        Ok(entries)
    }

    pub fn record_apply(
        &self,
        plan_id: &str,
        profile: &str,
        plan_hash: &str,
        ok: bool,
        details: Vec<String>,
    ) -> Result<(), MacflyError> {
        self.append(&JournalEntry {
            timestamp_unix: now_unix(),
            plan_id: plan_id.to_string(),
            profile: profile.to_string(),
            plan_hash: plan_hash.to_string(),
            status: if ok {
                "applied".to_string()
            } else {
                "failed".to_string()
            },
            operations_count: details.len(),
            details,
        })
    }
}
