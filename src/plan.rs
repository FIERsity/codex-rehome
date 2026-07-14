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
    pub destination_baseline: Vec<Change>,
    pub warnings: Vec<String>,
    pub state_schema_fingerprint: Option<String>,
    pub state_write_compatible: Option<bool>,
}

impl MigrationPlan {
    pub fn build(
        old: &Path,
        new: &Path,
        operation: Operation,
        source: &Discovery,
        destination: &Discovery,
    ) -> Result<Self> {
        let mut plan = Self {
            format_version: 2,
            migration_id: String::new(),
            tool_version: env!("CARGO_PKG_VERSION").into(),
            codex_version: source.codex_version.clone(),
            operation,
            old_root: old.into(),
            new_root: new.into(),
            old_real: old.canonicalize().ok(),
            new_real: new.canonicalize().ok(),
            thread_ids: source.threads.iter().map(|t| t.id.clone()).collect(),
            changes: source.changes.clone(),
            destination_baseline: destination.changes.clone(),
            warnings: source.warnings.clone(),
            state_schema_fingerprint: source.state_schema_fingerprint.clone(),
            state_write_compatible: source.state_write_compatible,
        };
        plan.thread_ids.sort();
        plan.changes
            .sort_by(|a, b| (&a.store, &a.file, &a.field).cmp(&(&b.store, &b.file, &b.field)));
        plan.destination_baseline
            .sort_by(|a, b| (&a.store, &a.file, &a.field).cmp(&(&b.store, &b.file, &b.field)));
        let bytes = serde_json::to_vec(&plan)?;
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        plan.migration_id = format!("plan-{}", &hex::encode(hasher.finalize())[..20]);
        Ok(plan)
    }
}
