use anyhow::{Result, bail};
use std::path::{Component, Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

pub fn lexical_absolute(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("path must be absolute: {}", path.display());
    }
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => out.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    bail!("path escapes filesystem root");
                }
            }
        }
    }
    Ok(out)
}

fn normalized_components(path: &Path) -> Result<Vec<String>> {
    Ok(lexical_absolute(path)?
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().nfc().collect()),
            _ => None,
        })
        .collect())
}

pub fn belongs_to(path: &Path, root: &Path) -> Result<bool> {
    let p = normalized_components(path)?;
    let r = normalized_components(root)?;
    Ok(p.len() >= r.len() && p.iter().zip(r.iter()).all(|(a, b)| a == b))
}

pub fn remap(path: &Path, old: &Path, new: &Path) -> Result<Option<PathBuf>> {
    if !belongs_to(path, old)? {
        return Ok(None);
    }
    let p = lexical_absolute(path)?;
    let o = lexical_absolute(old)?;
    Ok(Some(lexical_absolute(new)?.join(p.strip_prefix(o)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_and_child_match() {
        assert!(belongs_to(Path::new("/a/项目/x"), Path::new("/a/项目")).unwrap());
    }
    #[test]
    fn similar_prefix_does_not() {
        assert!(!belongs_to(Path::new("/a/project-copy"), Path::new("/a/project")).unwrap());
    }
    #[test]
    fn dot_segments_are_cleaned() {
        assert_eq!(
            remap(
                Path::new("/a/p/../p/x"),
                Path::new("/a/p"),
                Path::new("/b/q")
            )
            .unwrap()
            .unwrap(),
            Path::new("/b/q/x")
        );
    }
}
