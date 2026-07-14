use assert_cmd::Command;
use predicates::prelude::*;
use rusqlite::Connection;
use std::fs;
use tempfile::TempDir;

fn fixture() -> (TempDir, std::path::PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("codex");
    fs::create_dir_all(&home).unwrap();
    let db = Connection::open(home.join("state_5.sqlite")).unwrap();
    db.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY, rollout_path TEXT NOT NULL, cwd TEXT NOT NULL); INSERT INTO threads VALUES('synthetic-thread','/tmp/rollout.jsonl','/tmp/old project/sub');").unwrap();
    fs::write(home.join(".codex-global-state.json"), r#"{"active-workspace-roots":["/tmp/old project"],"unrelated":"the path /tmp/old project in prose"}"#).unwrap();
    (t, home)
}

#[test]
fn inspect_is_read_only() {
    let (_t, home) = fixture();
    let before = fs::read(home.join("state_5.sqlite")).unwrap();
    Command::cargo_bin("codex-rehome")
        .unwrap()
        .env("CODEX_HOME", &home)
        .args(["inspect", "/tmp/old project"])
        .assert()
        .success()
        .stdout(predicate::str::contains("synthetic-thread"));
    assert_eq!(before, fs::read(home.join("state_5.sqlite")).unwrap());
}

#[test]
fn write_requires_yes() {
    let (_t, home) = fixture();
    fs::create_dir_all("/tmp/new project").ok();
    Command::cargo_bin("codex-rehome")
        .unwrap()
        .env("CODEX_HOME", home)
        .args(["remap", "/tmp/old project", "/tmp/new project"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));
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
