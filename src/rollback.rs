use crate::backup::{self, Manifest};
use anyhow::{Context, Result, bail};
use std::{fs, path::Path};
pub fn run(home: &Path, id: &str) -> Result<()> {
    let dir = home.join("rehome-backups").join(id);
    let m: Manifest = serde_json::from_slice(
        &fs::read(dir.join("manifest.json")).context("migration manifest not found")?,
    )?;
    backup::restore(&m)?;
    if m.directory_moved {
        if m.plan.old_root.exists() {
            bail!("refusing to overwrite conflicting old directory")
        };
        if m.plan.new_root.exists() {
            fs::rename(&m.plan.new_root, &m.plan.old_root)?
        }
    }
    Ok(())
}
