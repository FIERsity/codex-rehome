use crate::discovery::Discovery;
use anyhow::Result;
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
    /// Stable digest of the semantic plan. Execution IDs live in the manifest.
    pub migration_id: String,
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
    pub state_schema_fingerprint: Option<String>,
}

impl MigrationPlan {
    pub fn build(old: &Path, new: &Path, operation: Operation, d: &Discovery) -> Result<Self> {
        let mut plan = Self {
            format_version: 2,
            migration_id: String::new(),
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
            state_schema_fingerprint: d.state_schema_fingerprint.clone(),
        };
        plan.thread_ids.sort();
        plan.changes
            .sort_by(|a, b| (&a.store, &a.file, &a.field).cmp(&(&b.store, &b.file, &b.field)));
        let bytes = serde_json::to_vec(&plan)?;
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        plan.migration_id = format!("plan-{}", &hex::encode(hasher.finalize())[..20]);
        Ok(plan)
    }
}
