use crate::{
    discovery::{Discovery, Thread},
    path_map::belongs_to,
    plan::Change,
};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const ADAPTER_ID: &str = "codex-state-0.144-v40";
const DISCOVERY_COLUMNS: &[&str] = &["id", "rollout_path", "cwd"];
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaReport {
    pub adapter_id: String,
    pub fingerprint: String,
    pub columns: Vec<String>,
    pub migration_count: Option<i64>,
    pub migration_max: Option<i64>,
    pub failed_migrations: Option<i64>,
    pub write_compatible: bool,
    pub incompatibilities: Vec<String>,
}

pub fn open_checked(path: &Path) -> Result<Connection> {
    open_checked_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
}
fn open_checked_with_flags(path: &Path, flags: OpenFlags) -> Result<Connection> {
    let db = Connection::open_with_flags(path, flags)?;
    let report = inspect_schema(&db)?;
    if !report.write_compatible {
        bail!(
            "unsupported Codex state schema; refusing mutation: {}",
            report.incompatibilities.join("; ")
        )
    }
    Ok(db)
}

pub fn open_for_discovery(path: &Path) -> Result<(Connection, SchemaReport)> {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let report = inspect_schema(&db)?;
    if !DISCOVERY_COLUMNS
        .iter()
        .all(|required| report.columns.iter().any(|column| column == required))
    {
        bail!("unknown Codex state schema: threads lacks read-only discovery columns")
    }
    Ok((db, report))
}

pub fn inspect_schema(db: &Connection) -> Result<SchemaReport> {
    let columns: Vec<String> = {
        let mut stmt = db.prepare("PRAGMA table_info(threads)")?;
        stmt.query_map([], |r| r.get(1))?
            .collect::<rusqlite::Result<_>>()?
    };
    let has_migrations: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations')",
        [],
        |r| r.get(0),
    )?;
    let (migration_count, migration_max, failed_migrations, migrations) = if has_migrations {
        let summary: (i64, Option<i64>, i64) = db.query_row(
            "SELECT count(*), max(version), coalesce(sum(CASE WHEN success THEN 0 ELSE 1 END),0) FROM _sqlx_migrations",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let mut stmt = db.prepare(
            "SELECT version, description, hex(checksum) FROM _sqlx_migrations ORDER BY version",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        (Some(summary.0), summary.1, Some(summary.2), rows)
    } else {
        (None, None, None, Vec::new())
    };
    let mut incompatibilities = Vec::new();
    if columns != SUPPORTED_COLUMNS {
        incompatibilities.push("threads columns differ from the validated 0.144.x layout".into());
    }
    if (migration_count, migration_max, failed_migrations)
        != (
            Some(SUPPORTED_MIGRATION_MAX),
            Some(SUPPORTED_MIGRATION_MAX),
            Some(0),
        )
    {
        incompatibilities.push(format!(
            "expected 40 successful migrations through version 40; found count={migration_count:?}, max={migration_max:?}, failed={failed_migrations:?}"
        ));
    }
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(&(&columns, &migrations))?);
    Ok(SchemaReport {
        adapter_id: ADAPTER_ID.into(),
        fingerprint: format!("sha256:{}", hex::encode(hasher.finalize())),
        columns,
        migration_count,
        migration_max,
        failed_migrations,
        write_compatible: incompatibilities.is_empty(),
        incompatibilities,
    })
}

pub fn discover(home: &Path, root: &Path, d: &mut Discovery) -> Result<()> {
    let path = home.join("state_5.sqlite");
    if !path.exists() {
        d.warnings.push("state_5.sqlite not found".into());
        return Ok(());
    }
    let (db, report) =
        open_for_discovery(&path).context("opening state_5.sqlite read-only discovery")?;
    d.state_schema_fingerprint = Some(format!("{}-{}", report.adapter_id, report.fingerprint));
    d.state_write_compatible = Some(report.write_compatible);
    d.warnings.extend(
        report
            .incompatibilities
            .iter()
            .map(|issue| format!("read-only schema warning: {issue}")),
    );
    d.state_schema = Some(report);
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
