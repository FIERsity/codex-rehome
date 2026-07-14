use crate::{discovery::Discovery, path_map::belongs_to, plan::Change};
use anyhow::Result;
use serde_json::Value;
use std::{fs, path::Path};
const KEYS: &[&str] = &[
    "active-workspace-roots",
    "electron-saved-workspace-roots",
    "project-order",
    "thread-workspace-root-hints",
    "thread-writable-roots",
];
pub fn discover(home: &Path, root: &Path, d: &mut Discovery) -> Result<()> {
    let p = home.join(".codex-global-state.json");
    if !p.exists() {
        return Ok(());
    }
    let v: Value = serde_json::from_slice(&fs::read(&p)?)?;
    let n = count(&v, root, false)?;
    if n > 0 {
        d.changes.push(Change {
            store: "desktop_state".into(),
            file: p,
            field: "allowlisted workspace fields".into(),
            expected: n,
        })
    }
    Ok(())
}
fn count(v: &Value, root: &Path, active: bool) -> Result<usize> {
    match v {
        Value::String(s) if active && Path::new(s).is_absolute() => {
            Ok(usize::from(belongs_to(Path::new(s), root)?))
        }
        Value::Array(a) => a.iter().map(|x| count(x, root, active)).sum(),
        Value::Object(m) => m
            .iter()
            .map(|(k, x)| count(x, root, active || KEYS.contains(&k.as_str())))
            .sum(),
        _ => Ok(0),
    }
}
pub(crate) fn rewrite(v: &mut Value, old: &Path, new: &Path, active: bool) -> Result<usize> {
    let mut n = 0;
    match v {
        Value::String(s) if active => {
            if !Path::new(s).is_absolute() {
                return Ok(0);
            }
            if let Some(p) = crate::path_map::remap(Path::new(s), old, new)? {
                *s = p.to_string_lossy().into();
                n = 1
            }
        }
        Value::Array(a) => {
            for x in a {
                n += rewrite(x, old, new, active)?
            }
        }
        Value::Object(m) => {
            for (k, x) in m {
                n += rewrite(x, old, new, active || KEYS.contains(&k.as_str()))?
            }
        }
        _ => {}
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_hint_is_rewritten() {
        let mut v = serde_json::json!({"thread-workspace-root-hints":{"id":"/old/project/sub"}});
        assert_eq!(
            rewrite(
                &mut v,
                Path::new("/old/project"),
                Path::new("/new/project"),
                false
            )
            .unwrap(),
            1
        );
        assert_eq!(v["thread-workspace-root-hints"]["id"], "/new/project/sub");
    }
    #[test]
    fn unrelated_prose_is_untouched() {
        let mut v = serde_json::json!({"note":"/old/project appears in prose"});
        assert_eq!(
            rewrite(
                &mut v,
                Path::new("/old/project"),
                Path::new("/new/project"),
                false
            )
            .unwrap(),
            0
        );
        assert_eq!(v["note"], "/old/project appears in prose");
    }
}
