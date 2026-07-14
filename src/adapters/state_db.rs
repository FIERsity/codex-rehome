use crate::{
    discovery::{Discovery, Thread},
    path_map::belongs_to,
    plan::Change,
};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

const REQUIRED: &[&str] = &["id", "rollout_path", "cwd"];
pub fn open_checked(path: &Path) -> Result<Connection> {
    open_checked_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
}
pub fn open_checked_read_only(path: &Path) -> Result<Connection> {
    open_checked_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
}
fn open_checked_with_flags(path: &Path, flags: OpenFlags) -> Result<Connection> {
    let db = Connection::open_with_flags(path, flags)?;
    let cols: Vec<String> = {
        let mut stmt = db.prepare("PRAGMA table_info(threads)")?;
        stmt.query_map([], |r| r.get(1))?
            .collect::<rusqlite::Result<_>>()?
    };
    if !REQUIRED.iter().all(|c| cols.iter().any(|x| x == c)) {
        bail!("unknown Codex state schema: threads lacks required columns");
    }
    Ok(db)
}
pub fn discover(home: &Path, root: &Path, d: &mut Discovery) -> Result<()> {
    let path = home.join("state_5.sqlite");
    if !path.exists() {
        d.warnings.push("state_5.sqlite not found".into());
        return Ok(());
    }
    let db = open_checked_read_only(&path).context("opening state_5.sqlite read-only discovery")?;
    let mut stmt = db.prepare("SELECT id,cwd,rollout_path FROM threads")?;
    for row in stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })? {
        let (id, cwd, rollout) = row?;
        if Path::new(&cwd).is_absolute() && belongs_to(Path::new(&cwd), root)? {
            d.threads.push(Thread {
                id,
                cwd: cwd.into(),
                rollout_path: rollout.into(),
            });
        }
    }
    if !d.threads.is_empty() {
        d.changes.push(Change {
            store: "state_db".into(),
            file: path,
            field: "threads.cwd".into(),
            expected: d.threads.len(),
        });
    }
    Ok(())
}
