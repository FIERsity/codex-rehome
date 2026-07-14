use crate::discovery::Discovery;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Operation {
    Move,
    Remap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub store: String,
    pub file: PathBuf,
    pub field: String,
    pub expected: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPlan {
    pub format_version: u32,
    pub migration_id: String,
    pub created_at: DateTime<Utc>,
    pub tool_version: String,
    pub codex_version: Option<String>,
    pub operation: Operation,
    pub old_root: PathBuf,
    pub new_root: PathBuf,
    pub old_real: Option<PathBuf>,
    pub new_real: Option<PathBuf>,
    pub thread_ids: Vec<String>,
    pub changes: Vec<Change>,
    pub warnings: Vec<String>,
}

impl MigrationPlan {
    pub fn build(old: &Path, new: &Path, operation: Operation, d: &Discovery) -> Result<Self> {
        let mut hasher = Sha256::new();
        hasher.update(old.as_os_str().as_encoded_bytes());
        hasher.update([0]);
        hasher.update(new.as_os_str().as_encoded_bytes());
        hasher.update(match operation {
            Operation::Move => &b"move"[..],
            Operation::Remap => &b"remap"[..],
        });
        let id = format!(
            "{}-{}",
            Utc::now().format("%Y%m%dT%H%M%SZ"),
            &hex::encode(hasher.finalize())[..12]
        );
        Ok(Self {
            format_version: 1,
            migration_id: id,
            created_at: Utc::now(),
            tool_version: env!("CARGO_PKG_VERSION").into(),
            codex_version: d.codex_version.clone(),
            operation,
            old_root: old.into(),
            new_root: new.into(),
            old_real: old.canonicalize().ok(),
            new_real: new.canonicalize().ok(),
            thread_ids: d.threads.iter().map(|t| t.id.clone()).collect(),
            changes: d.changes.clone(),
            warnings: d.warnings.clone(),
        })
    }
}
