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
    let n = count(&v, root, None)?;
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
fn count(v: &Value, root: &Path, key: Option<&str>) -> Result<usize> {
    match v {
        Value::String(s)
            if key.is_some_and(|k| KEYS.contains(&k)) && Path::new(s).is_absolute() =>
        {
            Ok(usize::from(belongs_to(Path::new(s), root)?))
        }
        Value::Array(a) => a.iter().map(|x| count(x, root, key)).sum(),
        Value::Object(m) => m.iter().map(|(k, x)| count(x, root, Some(k))).sum(),
        _ => Ok(0),
    }
}
pub(crate) fn rewrite(v: &mut Value, old: &Path, new: &Path, key: Option<&str>) -> Result<usize> {
    let mut n = 0;
    match v {
        Value::String(s) if key.is_some_and(|k| KEYS.contains(&k)) => {
            if Path::new(s).is_absolute() {
                if let Some(p) = crate::path_map::remap(Path::new(s), old, new)? {
                    *s = p.to_string_lossy().into();
                    n = 1
                }
            }
        }
        Value::Array(a) => {
            for x in a {
                n += rewrite(x, old, new, key)?
            }
        }
        Value::Object(m) => {
            for (k, x) in m {
                n += rewrite(x, old, new, Some(k))?
            }
        }
        _ => {}
    }
    Ok(n)
}
