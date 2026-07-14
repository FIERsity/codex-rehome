use crate::{discovery, plan::MigrationPlan};
use anyhow::{Result, bail};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct VerificationReport {
    pub verified: bool,
    pub old_root: Option<PathBuf>,
    pub new_root: PathBuf,
    pub old_references: usize,
    pub destination_threads: usize,
    pub discovery: discovery::Discovery,
}
pub fn migrated(plan: &MigrationPlan) -> Result<()> {
    let old_d = discovery::inspect(&plan.old_root)?;
    if !old_d.changes.is_empty() {
        bail!("structural references to old path remain")
    };
    let new_d = discovery::inspect(&plan.new_root)?;
    for thread_id in &plan.thread_ids {
        if !new_d.threads.iter().any(|t| &t.id == thread_id) {
            bail!("migrated thread missing at destination: {thread_id}")
        }
    }
    verify_change_counts(plan, &new_d, true)?;
    Ok(())
}
pub fn restored(plan: &MigrationPlan) -> Result<()> {
    let restored = discovery::inspect(&plan.old_root)?;
    verify_change_counts(plan, &restored, false)?;
    let destination = discovery::inspect(&plan.new_root)?;
    verify_exact_changes(&plan.destination_baseline, &destination)?;
    Ok(())
}
fn verify_exact_changes(
    expected_changes: &[crate::plan::Change],
    actual: &discovery::Discovery,
) -> Result<()> {
    for expected in expected_changes {
        let count: usize = actual
            .changes
            .iter()
            .filter(|c| {
                c.store == expected.store && c.file == expected.file && c.field == expected.field
            })
            .map(|c| c.expected)
            .sum();
        if count != expected.expected {
            bail!(
                "rollback destination baseline mismatch for {}:{}",
                expected.store,
                expected.field
            )
        }
    }
    let expected_total: usize = expected_changes.iter().map(|c| c.expected).sum();
    let actual_total: usize = actual.changes.iter().map(|c| c.expected).sum();
    if actual_total != expected_total {
        bail!("rollback left unexpected structural references to the destination path")
    }
    Ok(())
}
fn verify_change_counts(
    plan: &MigrationPlan,
    actual: &discovery::Discovery,
    include_destination_baseline: bool,
) -> Result<()> {
    for expected in &plan.changes {
        let count: usize = actual
            .changes
            .iter()
            .filter(|c| {
                c.store == expected.store && c.file == expected.file && c.field == expected.field
            })
            .map(|c| c.expected)
            .sum();
        let baseline: usize = if include_destination_baseline {
            plan.destination_baseline
                .iter()
                .filter(|c| {
                    c.store == expected.store
                        && c.file == expected.file
                        && c.field == expected.field
                })
                .map(|c| c.expected)
                .sum()
        } else {
            0
        };
        let total_expected = expected.expected + baseline;
        if count != total_expected {
            bail!(
                "verification count mismatch for {}:{}: expected {}, found {}",
                expected.store,
                expected.field,
                total_expected,
                count
            )
        }
    }
    Ok(())
}
pub fn report(new: &Path, old: Option<&Path>) -> Result<VerificationReport> {
    let old_references = old
        .map(discovery::inspect)
        .transpose()?
        .map(|result| result.changes.iter().map(|change| change.expected).sum())
        .unwrap_or(0);
    if old_references != 0 {
        bail!("structural references to old path remain")
    }
    let result = discovery::inspect(new)?;
    if old.is_some() && result.threads.is_empty() {
        bail!("no destination threads found")
    }
    Ok(VerificationReport {
        verified: true,
        old_root: old.map(Path::to_path_buf),
        new_root: new.to_path_buf(),
        old_references,
        destination_threads: result.threads.len(),
        discovery: result,
    })
}
