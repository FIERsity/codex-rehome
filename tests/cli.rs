use assert_cmd::Command;
use predicates::prelude::*;
use rusqlite::Connection;
use serde_json::Value;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    home: PathBuf,
    old: PathBuf,
    new: PathBuf,
    path: PathBuf,
}

fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("codex");
    let old = temp.path().join("旧 project");
    let new = temp.path().join("new project");
    let bin = temp.path().join("bin");
    fs::create_dir_all(home.join("sessions/2026/07/14")).unwrap();
    fs::create_dir_all(&old).unwrap();
    fs::create_dir_all(&new).unwrap();
    fs::create_dir(&bin).unwrap();
    let pgrep = bin.join("pgrep");
    fs::write(&pgrep, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&pgrep, fs::Permissions::from_mode(0o755)).unwrap();

    let db = Connection::open(home.join("state_5.sqlite")).unwrap();
    create_schema(&db);
    db.execute(
        "INSERT INTO threads(id,rollout_path,created_at,updated_at,source,model_provider,cwd,title,sandbox_policy,approval_mode) VALUES(?1,?2,0,0,'cli','openai',?3,'synthetic','{}','never')",
        ("synthetic-thread", home.join("sessions/2026/07/14/rollout.jsonl").to_string_lossy(), old.join("sub").to_string_lossy()),
    ).unwrap();
    drop(db);
    let rollout = serde_json::json!({"type":"session_meta","payload":{"id":"synthetic-thread","cwd":old.join("sub"),"message":"do not alter this path"}});
    let prose = serde_json::json!({"type":"response_item","payload":{"content":format!("mentioned {} in prose",old.display())}});
    fs::write(
        home.join("sessions/2026/07/14/rollout.jsonl"),
        format!("{}\n{}\n", rollout, prose),
    )
    .unwrap();
    fs::write(
        home.join(".codex-global-state.json"),
        serde_json::to_vec(&serde_json::json!({
            "active-workspace-roots":[old],
            "thread-workspace-root-hints":{"synthetic-thread":old.join("sub")},
            "unrelated":format!("the path {} in prose",old.display())
        }))
        .unwrap(),
    )
    .unwrap();
    let path = format!("{}:/usr/bin:/bin", bin.display()).into();
    Fixture {
        _temp: temp,
        home,
        old,
        new,
        path,
    }
}

fn create_schema(db: &Connection) {
    db.execute_batch(include_str!("fixtures/codex-0.144.2/schema.sql"))
        .unwrap();
}

fn command(f: &Fixture) -> Command {
    let mut c = Command::cargo_bin("codex-rehome").unwrap();
    c.env("CODEX_HOME", &f.home).env("PATH", &f.path);
    c
}

fn run_remap(f: &Fixture) -> String {
    let output = command(f)
        .args([
            "remap",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_owned()
}

#[test]
fn inspect_is_read_only() {
    let f = fixture();
    let before = fs::read(f.home.join("state_5.sqlite")).unwrap();
    command(&f)
        .args(["inspect", f.old.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("synthetic-thread"));
    assert_eq!(before, fs::read(f.home.join("state_5.sqlite")).unwrap());
}

#[test]
fn plan_is_deterministic_and_finds_nested_desktop_paths() {
    let f = fixture();
    let first = command(&f)
        .args(["plan", f.old.to_str().unwrap(), f.new.to_str().unwrap()])
        .output()
        .unwrap()
        .stdout;
    let second = command(&f)
        .args(["plan", f.old.to_str().unwrap(), f.new.to_str().unwrap()])
        .output()
        .unwrap()
        .stdout;
    assert_eq!(first, second);
    let value: Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(value["format_version"], 2);
    assert_eq!(
        value["changes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["store"] == "desktop_state")
            .unwrap()["expected"],
        2
    );
}

#[test]
fn write_requires_yes() {
    let f = fixture();
    command(&f)
        .args(["remap", f.old.to_str().unwrap(), f.new.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));
}

#[test]
fn remap_and_rollback_round_trip_without_touching_prose() {
    let f = fixture();
    let before_global = fs::read(f.home.join(".codex-global-state.json")).unwrap();
    let migration_id = run_remap(&f);
    let db = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    let cwd: String = db
        .query_row(
            "SELECT cwd FROM threads WHERE id='synthetic-thread'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(cwd.starts_with(f.new.to_str().unwrap()));
    let global = fs::read_to_string(f.home.join(".codex-global-state.json")).unwrap();
    assert!(global.contains(&format!("the path {} in prose", f.old.display())));
    command(&f)
        .args(["rollback", &migration_id, "--yes"])
        .assert()
        .success();
    assert_eq!(
        before_global,
        fs::read(f.home.join(".codex-global-state.json")).unwrap()
    );
}

#[test]
fn repeated_remap_is_idempotent() {
    let f = fixture();
    run_remap(&f);
    command(&f)
        .args([
            "remap",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("nothing to migrate"));
}

#[test]
fn backup_tampering_blocks_rollback() {
    let f = fixture();
    let id = run_remap(&f);
    let manifest_path = f
        .home
        .join("rehome-backups")
        .join(&id)
        .join("manifest.json");
    let manifest: Value = serde_json::from_slice(&fs::read(manifest_path).unwrap()).unwrap();
    let backup = manifest["files"][0]["backup"].as_str().unwrap();
    fs::write(backup, b"tampered").unwrap();
    command(&f)
        .args(["rollback", &id, "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("backup was modified"));
}

#[test]
fn backup_permissions_and_after_hashes_are_recorded() {
    let f = fixture();
    let id = run_remap(&f);
    let dir = f.home.join("rehome-backups").join(&id);
    assert_eq!(
        fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    for file in manifest["files"].as_array().unwrap() {
        assert!(file["after_sha256"].is_string());
        assert_eq!(
            fs::metadata(file["backup"].as_str().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}

#[test]
fn malformed_rollout_fails_closed_before_writing() {
    let f = fixture();
    let db_before = fs::read(f.home.join("state_5.sqlite")).unwrap();
    fs::write(
        f.home.join("sessions/2026/07/14/rollout.jsonl"),
        b"{bad json\n",
    )
    .unwrap();
    command(&f)
        .args([
            "remap",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("malformed rollout JSONL"));
    assert_eq!(db_before, fs::read(f.home.join("state_5.sqlite")).unwrap());
}

#[test]
fn process_detection_refuses_writes() {
    let f = fixture();
    let pgrep = f._temp.path().join("bin/pgrep");
    fs::write(&pgrep, "#!/bin/sh\necho '123 Codex'\nexit 0\n").unwrap();
    command(&f)
        .args([
            "remap",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Codex appears to be running"));
}

#[test]
fn move_and_rollback_restore_the_directory() {
    let f = fixture();
    fs::remove_dir(&f.new).unwrap();
    let output = command(&f)
        .args([
            "move",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let id = String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_owned();
    assert!(!f.old.exists() && f.new.exists());
    command(&f)
        .args(["rollback", &id, "--yes"])
        .assert()
        .success();
    assert!(f.old.exists() && !f.new.exists());
}

#[test]
fn rollback_will_not_overwrite_a_conflicting_directory() {
    let f = fixture();
    fs::remove_dir(&f.new).unwrap();
    let output = command(&f)
        .args([
            "move",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .output()
        .unwrap();
    let id = String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_owned();
    fs::create_dir(&f.old).unwrap();
    command(&f)
        .args(["rollback", &id, "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("refusing to overwrite"));
    assert!(f.old.exists() && f.new.exists());
}

#[test]
fn sqlite_snapshot_includes_committed_wal_rows() {
    let f = fixture();
    let writer = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    writer.pragma_update(None, "journal_mode", "WAL").unwrap();
    writer.execute(
        "INSERT INTO threads(id,rollout_path,created_at,updated_at,source,model_provider,cwd,title,sandbox_policy,approval_mode) VALUES('wal-thread','/tmp/wal.jsonl',0,0,'cli','openai',?1,'wal','{}','never')",
        [f.old.join("wal-child").to_string_lossy()],
    ).unwrap();
    let id = run_remap(&f);
    drop(writer);
    command(&f)
        .args(["rollback", &id, "--yes"])
        .assert()
        .success();
    let restored = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    let cwd: String = restored
        .query_row("SELECT cwd FROM threads WHERE id='wal-thread'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(cwd, f.old.join("wal-child").to_string_lossy());
}

#[test]
fn partially_migrated_state_preserves_destination_baseline() {
    let f = fixture();
    let db = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    db.execute(
        "INSERT INTO threads(id,rollout_path,created_at,updated_at,source,model_provider,cwd,title,sandbox_policy,approval_mode) VALUES('already-new','/tmp/already.jsonl',0,0,'cli','openai',?1,'existing','{}','never')",
        [f.new.join("existing").to_string_lossy()],
    ).unwrap();
    drop(db);
    let id = run_remap(&f);
    let db = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM threads WHERE cwd LIKE ?1",
            [format!("{}%", f.new.display())],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
    drop(db);
    command(&f)
        .args(["rollback", &id, "--yes"])
        .assert()
        .success();
    let db = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    let old_count: i64 = db
        .query_row(
            "SELECT count(*) FROM threads WHERE cwd LIKE ?1",
            [format!("{}%", f.old.display())],
            |r| r.get(0),
        )
        .unwrap();
    let new_count: i64 = db
        .query_row(
            "SELECT count(*) FROM threads WHERE cwd LIKE ?1",
            [format!("{}%", f.new.display())],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!((old_count, new_count), (1, 1));
}

#[test]
fn injected_failure_after_sqlite_update_restores_every_store() {
    let f = fixture();
    let before_global = fs::read(f.home.join(".codex-global-state.json")).unwrap();
    let before_rollout = fs::read(f.home.join("sessions/2026/07/14/rollout.jsonl")).unwrap();
    command(&f)
        .env("CODEX_REHOME_FAULT", "after_state_db")
        .args([
            "remap",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("injected fault"));
    let db = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    let cwd: String = db
        .query_row(
            "SELECT cwd FROM threads WHERE id='synthetic-thread'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cwd, f.old.join("sub").to_string_lossy());
    assert_eq!(
        before_global,
        fs::read(f.home.join(".codex-global-state.json")).unwrap()
    );
    assert_eq!(
        before_rollout,
        fs::read(f.home.join("sessions/2026/07/14/rollout.jsonl")).unwrap()
    );
}

#[test]
fn symlink_project_root_is_rejected() {
    let f = fixture();
    let link = f._temp.path().join("linked destination");
    std::os::unix::fs::symlink(&f.new, &link).unwrap();
    command(&f)
        .args([
            "remap",
            f.old.to_str().unwrap(),
            link.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("symbolic link"));
}

#[test]
fn hard_linked_state_file_is_rejected() {
    let f = fixture();
    fs::hard_link(
        f.home.join(".codex-global-state.json"),
        f.home.join("global-state-hardlink.json"),
    )
    .unwrap();
    command(&f)
        .args([
            "remap",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("hard-linked state file"));
}

#[test]
fn injected_failure_after_move_restores_directory_and_state() {
    let f = fixture();
    fs::remove_dir(&f.new).unwrap();
    command(&f)
        .env("CODEX_REHOME_FAULT", "after_move")
        .args([
            "move",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("injected fault"));
    assert!(f.old.exists());
    assert!(!f.new.exists());
    let db = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    let cwd: String = db
        .query_row(
            "SELECT cwd FROM threads WHERE id='synthetic-thread'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cwd, f.old.join("sub").to_string_lossy());
}

#[test]
fn remap_succeeds_when_old_directory_is_already_missing() {
    let f = fixture();
    fs::remove_dir(&f.old).unwrap();
    let id = run_remap(&f);
    let db = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    let cwd: String = db
        .query_row(
            "SELECT cwd FROM threads WHERE id='synthetic-thread'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(cwd.starts_with(f.new.to_str().unwrap()));
    drop(db);
    command(&f)
        .args(["rollback", &id, "--yes"])
        .assert()
        .success();
}

#[test]
fn move_refuses_an_existing_destination() {
    let f = fixture();
    command(&f)
        .args([
            "move",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "existing source and absent target",
        ));
}

#[test]
fn migration_version_drift_fails_closed() {
    let f = fixture();
    let db = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    db.execute("DELETE FROM _sqlx_migrations WHERE version=40", [])
        .unwrap();
    drop(db);
    command(&f)
        .args(["inspect", f.old.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("expected successful migrations"));
}

#[test]
fn rollout_atomic_replace_fault_restores_all_state() {
    let f = fixture();
    let before_global = fs::read(f.home.join(".codex-global-state.json")).unwrap();
    let before_rollout = fs::read(f.home.join("sessions/2026/07/14/rollout.jsonl")).unwrap();
    command(&f)
        .env("CODEX_REHOME_FAULT", "after_rollout_persist")
        .args([
            "remap",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("injected fault"));
    let db = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    let cwd: String = db
        .query_row(
            "SELECT cwd FROM threads WHERE id='synthetic-thread'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cwd, f.old.join("sub").to_string_lossy());
    assert_eq!(
        before_global,
        fs::read(f.home.join(".codex-global-state.json")).unwrap()
    );
    assert_eq!(
        before_rollout,
        fs::read(f.home.join("sessions/2026/07/14/rollout.jsonl")).unwrap()
    );
}

#[test]
fn prepared_manifest_fault_leaves_no_orphan_backup() {
    let f = fixture();
    command(&f)
        .env("CODEX_REHOME_FAULT", "before_prepared_manifest_persist")
        .args([
            "remap",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("injected fault"));
    let backup_root = f.home.join("rehome-backups");
    assert_eq!(fs::read_dir(backup_root).unwrap().count(), 0);
}

#[test]
fn complete_manifest_fault_rolls_back_and_records_failure() {
    let f = fixture();
    command(&f)
        .env("CODEX_REHOME_FAULT", "before_complete_manifest_persist")
        .args([
            "remap",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("injected fault"));
    let dir = fs::read_dir(f.home.join("rehome-backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let manifest: Value =
        serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["status"], "failed-rolled-back");
    let db = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    let cwd: String = db
        .query_row(
            "SELECT cwd FROM threads WHERE id='synthetic-thread'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cwd, f.old.join("sub").to_string_lossy());
}

#[test]
fn interrupted_rollback_can_be_retried() {
    let f = fixture();
    let id = run_remap(&f);
    command(&f)
        .env("CODEX_REHOME_FAULT", "after_restore_file_0")
        .args(["rollback", &id, "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("injected fault"));
    command(&f)
        .args(["rollback", &id, "--yes"])
        .assert()
        .success();
    let db = Connection::open(f.home.join("state_5.sqlite")).unwrap();
    let cwd: String = db
        .query_row(
            "SELECT cwd FROM threads WHERE id='synthetic-thread'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(cwd, f.old.join("sub").to_string_lossy());
}

#[test]
fn rollback_manifest_interruption_is_idempotent() {
    let f = fixture();
    let id = run_remap(&f);
    command(&f)
        .env("CODEX_REHOME_FAULT", "before_rolledback_manifest_persist")
        .args(["rollback", &id, "--yes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("injected fault"));
    command(&f)
        .args(["rollback", &id, "--yes"])
        .assert()
        .success();
    let manifest: Value = serde_json::from_slice(
        &fs::read(
            f.home
                .join("rehome-backups")
                .join(&id)
                .join("manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["status"], "rolled-back");
}

#[test]
fn automatic_recovery_failure_is_never_reported_as_rolled_back() {
    let f = fixture();
    command(&f)
        .env("CODEX_REHOME_FAULT", "after_state_db,after_restore_file_0")
        .args([
            "remap",
            f.old.to_str().unwrap(),
            f.new.to_str().unwrap(),
            "--yes",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("automatic recovery also failed"));
    let dir = fs::read_dir(f.home.join("rehome-backups"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let manifest: Value =
        serde_json::from_slice(&fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["status"], "failed-rollback-error");
}

#[test]
fn unknown_schema_fails_closed() {
    let t = tempfile::tempdir().unwrap();
    let db = Connection::open(t.path().join("state_5.sqlite")).unwrap();
    db.execute("CREATE TABLE threads(id TEXT)", []).unwrap();
    Command::cargo_bin("codex-rehome")
        .unwrap()
        .env("CODEX_HOME", t.path())
        .args(["inspect", "/tmp/x"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown Codex state schema"));
}
