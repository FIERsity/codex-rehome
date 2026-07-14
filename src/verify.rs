use crate::discovery;
use anyhow::{Result, bail};
use std::path::Path;
pub fn migrated(old: &Path, new: &Path, expected: usize) -> Result<()> {
    let old_d = discovery::inspect(old)?;
    if !old_d.changes.is_empty() {
        bail!("structural references to old path remain")
    };
    let new_d = discovery::inspect(new)?;
    if new_d.threads.len() < expected {
        bail!(
            "expected at least {expected} migrated threads, found {}",
            new_d.threads.len()
        )
    }
    Ok(())
}
pub fn report(root: &Path) -> Result<discovery::Discovery> {
    discovery::inspect(root)
}
