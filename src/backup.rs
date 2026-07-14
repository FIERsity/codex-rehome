use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    pub original: PathBuf,
    pub backup: PathBuf,
    pub before_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
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
    let dir = home.join("rehome-backups").join(&plan.migration_id);
    fs::create_dir_all(&dir)?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    let mut files = vec![];
    let mut unique = plan
        .changes
        .iter()
        .map(|c| c.file.clone())
        .collect::<Vec<_>>();
    unique.sort();
    unique.dedup();
    for src in unique {
        let name = format!(
            "{:04}-{}",
            files.len(),
            src.file_name().unwrap_or_default().to_string_lossy()
        );
        let dst = dir.join(name);
        fs::copy(&src, &dst).with_context(|| format!("backup {}", src.display()))?;
        fs::set_permissions(&dst, fs::Permissions::from_mode(0o600))?;
        let digest = sha(&src)?;
        if sha(&dst)? != digest {
            bail!("backup checksum mismatch")
        };
        files.push(BackupFile {
            original: src,
            backup: dst,
            before_sha256: digest,
        });
    }
    Ok((
        dir,
        Manifest {
            plan: plan.clone(),
            status: "prepared".into(),
            files,
            directory_moved: false,
        },
    ))
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
        fs::copy(&f.backup, &f.original)?;
        if sha(&f.original)? != f.before_sha256 {
            bail!("restore checksum mismatch: {}", f.original.display())
        }
    }
    Ok(())
}
