use crate::{
    discovery::{Discovery, Thread},
    path_map::belongs_to,
    plan::Change,
};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};
use std::path::Path;

const SUPPORTED_COLUMNS: &[&str] = &[
    "id",
    "rollout_path",
    "created_at",
    "updated_at",
    "source",
    "model_provider",
    "cwd",
    "title",
    "sandbox_policy",
    "approval_mode",
    "tokens_used",
    "has_user_event",
    "archived",
    "archived_at",
    "git_sha",
    "git_branch",
    "git_origin_url",
    "cli_version",
    "first_user_message",
    "agent_nickname",
    "agent_role",
    "memory_mode",
    "model",
    "reasoning_effort",
    "agent_path",
    "created_at_ms",
    "updated_at_ms",
    "thread_source",
    "preview",
    "recency_at",
    "recency_at_ms",
    "history_mode",
];
const SUPPORTED_MIGRATION_MAX: i64 = 40;
pub fn open_checked(path: &Path) -> Result<Connection> {
    open_checked_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
}
pub fn open_checked_read_only(path: &Path) -> Result<Connection> {
    open_checked_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
}
fn open_checked_with_flags(path: &Path, flags: OpenFlags) -> Result<Connection> {
    let db = Connection::open_with_flags(path, flags)?;
    validate_schema(&db)?;
    Ok(db)
}
fn validate_schema(db: &Connection) -> Result<()> {
    let cols: Vec<String> = {
        let mut stmt = db.prepare("PRAGMA table_info(threads)")?;
        stmt.query_map([], |r| r.get(1))?
            .collect::<rusqlite::Result<_>>()?
    };
    if cols != SUPPORTED_COLUMNS {
        bail!(
            "unknown Codex state schema: threads columns do not match the validated 0.144.x adapter"
        );
    }
    let migrations: (i64, Option<i64>, i64) = db.query_row(
        "SELECT count(*), max(version), sum(CASE WHEN success THEN 0 ELSE 1 END) FROM _sqlx_migrations",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).context("missing or invalid _sqlx_migrations table")?;
    if migrations != (SUPPORTED_MIGRATION_MAX, Some(SUPPORTED_MIGRATION_MAX), 0) {
        bail!(
            "unknown Codex state schema: expected successful migrations 1..={SUPPORTED_MIGRATION_MAX}"
        );
    }
    Ok(())
}
pub fn schema_fingerprint(db: &Connection) -> Result<String> {
    validate_schema(db)?;
    let mut rows = db.prepare(
        "SELECT version, description, hex(checksum) FROM _sqlx_migrations ORDER BY version",
    )?;
    let migrations: Vec<(i64, String, String)> = rows
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut h = Sha256::new();
    h.update(serde_json::to_vec(&(SUPPORTED_COLUMNS, migrations))?);
    Ok(format!(
        "codex-state-v40-sha256:{}",
        hex::encode(h.finalize())
    ))
}
pub fn discover(home: &Path, root: &Path, d: &mut Discovery) -> Result<()> {
    let path = home.join("state_5.sqlite");
    if !path.exists() {
        d.warnings.push("state_5.sqlite not found".into());
        return Ok(());
    }
    let db = open_checked_read_only(&path).context("opening state_5.sqlite read-only discovery")?;
    d.state_schema_fingerprint = Some(schema_fingerprint(&db)?);
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
