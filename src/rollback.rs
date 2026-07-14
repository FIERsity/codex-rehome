use crate::backup::{self, Manifest};
use anyhow::{Context, Result, bail};
use fs2::FileExt;
use std::{fs, fs::OpenOptions, path::Path};
pub fn run(home: &Path, id: &str) -> Result<()> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(home.join("codex-rehome.lock"))?;
    lock.try_lock_exclusive()
        .context("another migration holds the lock")?;
    let dir = home.join("rehome-backups").join(id);
    let mut m: Manifest = serde_json::from_slice(
        &fs::read(dir.join("manifest.json")).context("migration manifest not found")?,
    )?;
    if m.directory_moved && m.plan.old_root.exists() {
        bail!("refusing to overwrite conflicting old directory")
    }
    backup::restore(&m)?;
    if m.directory_moved && m.plan.new_root.exists() {
        fs::rename(&m.plan.new_root, &m.plan.old_root)?
    }
    crate::verify::restored(&m.plan)?;
    m.status = "rolled-back".into();
    backup::write_manifest(&dir, &m)?;
    Ok(())
}
