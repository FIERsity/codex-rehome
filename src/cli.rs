use crate::{
    adapters, backup, discovery, path_map,
    plan::{MigrationPlan, Operation},
    rollback, verify,
};
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use fs2::FileExt;
use rusqlite::params;
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Parser)]
#[command(name = "codex-rehome", version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: CommandKind,
}
#[derive(Subcommand)]
enum CommandKind {
    Inspect {
        path: PathBuf,
    },
    Plan {
        old: PathBuf,
        new: PathBuf,
        #[arg(long)]
        move_directory: bool,
    },
    Remap {
        old: PathBuf,
        new: PathBuf,
        #[arg(long)]
        yes: bool,
    },
    Move {
        old: PathBuf,
        new: PathBuf,
        #[arg(long)]
        yes: bool,
    },
    Verify {
        new: PathBuf,
        #[arg(long)]
        old: Option<PathBuf>,
    },
    Rollback {
        migration_id: String,
        #[arg(long)]
        yes: bool,
    },
    Doctor,
}
pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        CommandKind::Inspect { path } => print_json(&discovery::inspect(&path)?)?,
        CommandKind::Plan {
            old,
            new,
            move_directory,
        } => print_json(&make_plan(
            &old,
            &new,
            if move_directory {
                Operation::Move
            } else {
                Operation::Remap
            },
        )?)?,
        CommandKind::Remap { old, new, yes } => execute(&old, &new, Operation::Remap, yes)?,
        CommandKind::Move { old, new, yes } => execute(&old, &new, Operation::Move, yes)?,
        CommandKind::Verify { new, old } => {
            if let Some(o) = old {
                verify::paths(&o, &new)?
            }
            print_json(&verify::report(&new)?)?
        }
        CommandKind::Rollback { migration_id, yes } => {
            require_yes(yes)?;
            ensure_stopped()?;
            rollback::run(&discovery::codex_home(), &migration_id)?
        }
        CommandKind::Doctor => doctor()?,
    }
    Ok(())
}
fn print_json<T: serde::Serialize>(v: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}
fn make_plan(old: &Path, new: &Path, op: Operation) -> Result<MigrationPlan> {
    let o = path_map::lexical_absolute(old)?;
    let n = path_map::lexical_absolute(new)?;
    MigrationPlan::build(
        &o,
        &n,
        op,
        &discovery::inspect(&o)?,
        &discovery::inspect(&n)?,
    )
}
fn require_yes(yes: bool) -> Result<()> {
    if !yes {
        bail!("dry-run only: review `plan`, then repeat the write command with --yes")
    }
    Ok(())
}
fn ensure_stopped() -> Result<()> {
    let out = Command::new("pgrep")
        .args(["-afil", "Codex|codex app-server|codex exec|codex resume"])
        .output();
    let Ok(o) = out else { return Ok(()) };
    if o.status.success() && !o.stdout.is_empty() {
        bail!("Codex appears to be running; close Codex Desktop and CLI before writing")
    }
    Ok(())
}
fn execute(old: &Path, new: &Path, op: Operation, yes: bool) -> Result<()> {
    require_yes(yes)?;
    ensure_stopped()?;
    reject_symlink_root(old, "source")?;
    reject_symlink_root(new, "destination")?;
    let plan = make_plan(old, new, op.clone())?;
    if plan.changes.is_empty() {
        bail!("nothing to migrate")
    };
    if op == Operation::Remap && !new.exists() {
        bail!("remap target does not exist")
    };
    if op == Operation::Move && (!old.exists() || new.exists()) {
        bail!("move requires existing source and absent target")
    }
    if op == Operation::Move {
        ensure_move_preflight(old, new)?;
    }
    let home = discovery::codex_home();
    let lock_path = home.join("codex-rehome.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)?;
    lock.try_lock_exclusive()
        .context("another migration holds the lock")?;
    let (dir, mut manifest) = backup::create(&home, &plan)?;
    if let Err(error) = backup::write_manifest_stage(&dir, &manifest, Some("prepared_manifest")) {
        let _ = fs::remove_dir_all(&dir);
        return Err(error);
    }
    let result = (|| -> Result<()> {
        if op == Operation::Move {
            fs::rename(old, new).context(
                "atomic directory move failed (cross-filesystem moves are not supported in v0.1)",
            )?;
            manifest.directory_moved = true;
            backup::write_manifest_stage(&dir, &manifest, Some("moved_manifest"))?;
            crate::fault::check("after_move")?;
        }
        mutate(&plan)?;
        crate::fault::check("before_verify")?;
        verify::migrated(&plan)?;
        backup::record_after_hashes(&mut manifest)?;
        manifest.status = "complete".into();
        backup::write_manifest_stage(&dir, &manifest, Some("complete_manifest"))?;
        Ok(())
    })();
    if let Err(e) = result {
        let mut recovery_errors = Vec::new();
        if let Err(error) = backup::restore(&manifest) {
            recovery_errors.push(format!("state restore: {error:#}"));
        }
        if manifest.directory_moved && !old.exists() && new.exists() {
            if let Err(error) = fs::rename(new, old) {
                recovery_errors.push(format!("directory restore: {error}"));
            }
        }
        if recovery_errors.is_empty() {
            recovery_errors.extend(
                verify::restored(&plan)
                    .err()
                    .map(|error| format!("restore verification: {error:#}")),
            );
        }
        manifest.status = if recovery_errors.is_empty() {
            "failed-rolled-back"
        } else {
            "failed-rollback-error"
        }
        .into();
        let _ = backup::write_manifest(&dir, &manifest);
        if !recovery_errors.is_empty() {
            bail!(
                "migration failed: {e:#}; automatic recovery also failed: {}",
                recovery_errors.join("; ")
            )
        }
        return Err(e);
    }
    println!(
        "migration {} completed (plan {})",
        manifest.migration_id, plan.migration_id
    );
    Ok(())
}
fn reject_symlink_root(path: &Path, label: &str) -> Result<()> {
    if path.exists() && fs::symlink_metadata(path)?.file_type().is_symlink() {
        bail!(
            "refusing {label} project root that is a symbolic link: {}",
            path.display()
        )
    }
    Ok(())
}
fn ensure_move_preflight(old: &Path, new: &Path) -> Result<()> {
    let source = fs::metadata(old)?;
    if !source.is_dir() {
        bail!("move source is not a directory: {}", old.display())
    }
    let mut destination_parent = new.parent().context("move destination has no parent")?;
    while !destination_parent.exists() {
        destination_parent = destination_parent
            .parent()
            .context("move destination has no existing ancestor")?;
    }
    ensure_same_device(source.dev(), fs::metadata(destination_parent)?.dev())
}
fn ensure_same_device(source_device: u64, destination_device: u64) -> Result<()> {
    if source_device != destination_device {
        bail!("cross-filesystem moves are not supported; use a manual move followed by remap")
    }
    Ok(())
}
fn mutate(plan: &MigrationPlan) -> Result<()> {
    for c in &plan.changes {
        match c.store.as_str() {
            "state_db" => mutate_db(&c.file, &plan.old_root, &plan.new_root)?,
            "rollout" => mutate_jsonl(&c.file, &plan.old_root, &plan.new_root)?,
            "desktop_state" => mutate_global(&c.file, &plan.old_root, &plan.new_root)?,
            _ => bail!("unknown adapter"),
        }
        crate::fault::check(&format!("after_{}", c.store))?;
    }
    Ok(())
}
fn mutate_db(path: &Path, old: &Path, new: &Path) -> Result<()> {
    let mut db = adapters::state_db::open_checked(path)?;
    let tx = db.transaction()?;
    let mut rows = vec![];
    {
        let mut s = tx.prepare("SELECT id,cwd FROM threads")?;
        for r in s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (id, cwd) = r?;
            if let Some(p) = path_map::remap(Path::new(&cwd), old, new)? {
                rows.push((id, p.to_string_lossy().into_owned()))
            }
        }
    }
    for (id, cwd) in rows {
        tx.execute("UPDATE threads SET cwd=?1 WHERE id=?2", params![cwd, id])?;
    }
    tx.commit()?;
    Ok(())
}
fn mutate_jsonl(path: &Path, old: &Path, new: &Path) -> Result<()> {
    let parent = path.parent().unwrap();
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    for line in BufReader::new(File::open(path)?).lines() {
        let mut v: Value = serde_json::from_str(&line?)?;
        adapters::rollout::rewrite(&mut v, old, new)?;
        serde_json::to_writer(&mut tmp, &v)?;
        tmp.write_all(b"\n")?
    }
    tmp.as_file().sync_all()?;
    crate::fault::check("before_rollout_persist")?;
    tmp.persist(path)?;
    backup::sync_directory(parent)?;
    crate::fault::check("after_rollout_persist")?;
    Ok(())
}
fn mutate_global(path: &Path, old: &Path, new: &Path) -> Result<()> {
    let mut v: Value = serde_json::from_slice(&fs::read(path)?)?;
    adapters::desktop_state::rewrite(&mut v, old, new, false)?;
    let parent = path.parent().unwrap();
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut tmp, &v)?;
    tmp.as_file().sync_all()?;
    crate::fault::check("before_desktop_persist")?;
    tmp.persist(path)?;
    backup::sync_directory(parent)?;
    crate::fault::check("after_desktop_persist")?;
    Ok(())
}
fn doctor() -> Result<()> {
    let home = discovery::codex_home();
    let db = home.join("state_5.sqlite");
    let mut issues = vec![];
    if db.exists() {
        let c = adapters::state_db::open_checked_read_only(&db)?;
        let ok: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if ok != "ok" {
            issues.push(ok)
        }
    } else {
        issues.push("state_5.sqlite missing".into())
    }
    print_json(
        &serde_json::json!({"codex_home":home,"compatible":issues.is_empty(),"issues":issues}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_devices_are_accepted() {
        ensure_same_device(7, 7).unwrap();
    }
    #[test]
    fn different_devices_are_rejected() {
        assert!(
            ensure_same_device(7, 8)
                .unwrap_err()
                .to_string()
                .contains("cross-filesystem")
        );
    }
}
