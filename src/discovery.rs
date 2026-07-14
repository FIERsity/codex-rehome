use crate::{adapters, plan::Change};
use anyhow::Result;
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Serialize)]
pub struct Thread {
    pub id: String,
    pub cwd: PathBuf,
    pub rollout_path: PathBuf,
}
#[derive(Debug, Clone, Serialize)]
pub struct Discovery {
    pub codex_home: PathBuf,
    pub codex_version: Option<String>,
    pub state_schema_fingerprint: Option<String>,
    pub state_write_compatible: Option<bool>,
    pub state_schema: Option<crate::adapters::state_db::SchemaReport>,
    pub threads: Vec<Thread>,
    pub changes: Vec<Change>,
    pub warnings: Vec<String>,
}

pub fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".codex")
        })
}
pub fn codex_version() -> Option<String> {
    Command::new("codex")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub fn inspect(root: &Path) -> Result<Discovery> {
    let home = codex_home();
    let version = codex_version();
    let mut d = Discovery {
        codex_home: home.clone(),
        codex_version: version,
        state_schema_fingerprint: None,
        state_write_compatible: None,
        state_schema: None,
        threads: vec![],
        changes: vec![],
        warnings: vec![],
    };
    adapters::state_db::discover(&home, root, &mut d)?;
    adapters::rollout::discover(&home, root, &mut d)?;
    adapters::desktop_state::discover(&home, root, &mut d)?;
    Ok(d)
}
