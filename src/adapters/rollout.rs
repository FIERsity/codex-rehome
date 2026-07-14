use crate::{discovery::Discovery, path_map::belongs_to, plan::Change};
use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};
use walkdir::WalkDir;

const PATH_FIELDS: &[&str] = &["cwd", "working_directory", "workspace_root"];
pub fn discover(home: &Path, root: &Path, d: &mut Discovery) -> Result<()> {
    let mut count = 0;
    for dir in [home.join("sessions"), home.join("archived_sessions")] {
        if !dir.exists() {
            continue;
        }
        for ent in WalkDir::new(dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        {
            let mut file_count = 0;
            for line in BufReader::new(File::open(ent.path())?).lines() {
                let v: Value = serde_json::from_str(&line.context("reading JSONL")?)
                    .context("malformed rollout JSONL")?;
                file_count += count_paths(&v, root)?;
            }
            if file_count > 0 {
                d.changes.push(Change {
                    store: "rollout".into(),
                    file: ent.path().into(),
                    field: "structured path fields".into(),
                    expected: file_count,
                });
                count += file_count;
            }
        }
    }
    if count == 0 && !d.threads.is_empty() {
        d.warnings
            .push("no matching structured rollout paths found".into())
    }
    Ok(())
}
fn count_paths(v: &Value, root: &Path) -> Result<usize> {
    let mut n = 0;
    if let Value::Object(map) = v {
        for (k, x) in map {
            if PATH_FIELDS.contains(&k.as_str()) {
                let Some(s) = x.as_str() else { continue };
                if Path::new(s).is_absolute() && belongs_to(Path::new(s), root)? {
                    n += 1
                }
            } else if k == "payload" {
                n += count_paths(x, root)?;
            }
        }
    }
    Ok(n)
}
pub(crate) fn rewrite(v: &mut Value, old: &Path, new: &Path) -> Result<usize> {
    let mut n = 0;
    if let Value::Object(map) = v {
        for (k, x) in map {
            if PATH_FIELDS.contains(&k.as_str()) {
                let Some(s) = x.as_str() else { continue };
                if let Some(p) = crate::path_map::remap(Path::new(s), old, new)? {
                    *x = Value::String(p.to_string_lossy().into());
                    n += 1
                }
            } else if k == "payload" {
                n += rewrite(x, old, new)?
            }
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn payload_cwd_is_rewritten_but_message_is_not() {
        let mut v = serde_json::json!({"payload":{"cwd":"/old/p/sub","message":"run cd /old/p"}});
        assert_eq!(
            rewrite(&mut v, Path::new("/old/p"), Path::new("/new/p")).unwrap(),
            1
        );
        assert_eq!(v["payload"]["cwd"], "/new/p/sub");
        assert_eq!(v["payload"]["message"], "run cd /old/p");
    }
    #[test]
    fn similar_prefix_is_not_rewritten() {
        let mut v = serde_json::json!({"payload":{"cwd":"/old/project-copy"}});
        assert_eq!(
            rewrite(&mut v, Path::new("/old/project"), Path::new("/new/project")).unwrap(),
            0
        );
    }
}
