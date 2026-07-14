use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, backup::Backup};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    pub store: String,
    pub original: PathBuf,
    pub backup: PathBuf,
    pub source_sha256: String,
    pub before_sha256: String,
    pub after_sha256: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub migration_id: String,
    pub created_at: DateTime<Utc>,
    pub plan: crate::plan::MigrationPlan,
    pub status: String,
    pub files: Vec<BackupFile>,
    pub directory_moved: bool,
}
pub fn sha(path: &Path) -> Result<String> {
    let mut h = Sha256::new();
    h.update(fs::read(path)?);
    Ok(hex::encode(h.finalize()))
}
pub fn create(home: &Path, plan: &crate::plan::MigrationPlan) -> Result<(PathBuf, Manifest)> {
    let migration_id = format!("migration-{}", uuid::Uuid::new_v4());
    let root = home.join("rehome-backups");
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let dir = root.join(&migration_id);
    fs::create_dir(&dir)?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    let mut files = vec![];
    let mut unique = plan
        .changes
        .iter()
        .map(|c| (c.file.clone(), c.store.clone()))
        .collect::<Vec<_>>();
    unique.sort();
    unique.dedup_by(|a, b| a.0 == b.0);
    for (src, store) in unique {
        let name = format!(
            "{:04}-{}",
            files.len(),
            src.file_name().unwrap_or_default().to_string_lossy()
        );
        let dst = dir.join(name);
        let source_digest = sha(&src)?;
        if store == "state_db" {
            sqlite_snapshot(&src, &dst)?;
        } else {
            fs::copy(&src, &dst).with_context(|| format!("backup {}", src.display()))?;
        }
        fs::set_permissions(&dst, fs::Permissions::from_mode(0o600))?;
        let digest = sha(&dst)?;
        files.push(BackupFile {
            store,
            original: src,
            backup: dst,
            source_sha256: source_digest,
            before_sha256: digest,
            after_sha256: None,
        });
    }
    Ok((
        dir,
        Manifest {
            migration_id,
            created_at: Utc::now(),
            plan: plan.clone(),
            status: "prepared".into(),
            files,
            directory_moved: false,
        },
    ))
}
fn sqlite_snapshot(src: &Path, dst: &Path) -> Result<()> {
    let from = Connection::open_with_flags(src, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut to = Connection::open(dst)?;
    let backup = Backup::new(&from, &mut to)?;
    backup.run_to_completion(128, std::time::Duration::from_millis(10), None)?;
    drop(backup);
    let integrity: String = to.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        bail!("SQLite backup integrity check failed: {integrity}")
    }
    Ok(())
}
pub fn write_manifest(dir: &Path, m: &Manifest) -> Result<()> {
    let p = dir.join("manifest.json");
    let tmp = dir.join(".manifest.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(m)?)?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    fs::rename(tmp, p)?;
    Ok(())
}
pub fn restore(m: &Manifest) -> Result<()> {
    for f in &m.files {
        if sha(&f.backup)? != f.before_sha256 {
            bail!("backup was modified: {}", f.backup.display())
        }
        if f.store == "state_db" {
            remove_sqlite_sidecars(&f.original)?;
        }
        fs::copy(&f.backup, &f.original)?;
        if sha(&f.original)? != f.before_sha256 {
            bail!("restore checksum mismatch: {}", f.original.display())
        }
    }
    Ok(())
}
pub fn record_after_hashes(m: &mut Manifest) -> Result<()> {
    for file in &mut m.files {
        file.after_sha256 = Some(sha(&file.original)?);
    }
    Ok(())
}
fn remove_sqlite_sidecars(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{}", path.display(), suffix));
        if sidecar.exists() {
            fs::remove_file(sidecar)?;
        }
    }
    Ok(())
}
