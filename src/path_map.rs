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
    let destination = lexical_absolute(new)?;
    let suffix = p.strip_prefix(o)?;
    Ok(Some(if suffix.as_os_str().is_empty() {
        destination
    } else {
        destination.join(suffix)
    }))
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
    #[test]
    fn exact_root_maps() {
        let mapped = remap(Path::new("/a/p"), Path::new("/a/p/"), Path::new("/b/q"))
            .unwrap()
            .unwrap();
        assert_eq!(mapped, Path::new("/b/q"));
        assert_eq!(mapped.to_string_lossy(), "/b/q");
    }
    #[test]
    fn relative_paths_are_rejected() {
        assert!(belongs_to(Path::new("a/p"), Path::new("/a")).is_err());
    }
    #[test]
    fn parent_cannot_escape_root() {
        assert!(lexical_absolute(Path::new("/../../x")).is_err());
    }
    #[test]
    fn unicode_nfc_and_nfd_are_equivalent() {
        assert!(belongs_to(Path::new("/tmp/café/x"), Path::new("/tmp/cafe\u{301}")).unwrap());
    }
    #[test]
    fn spaces_are_preserved() {
        assert_eq!(
            remap(
                Path::new("/old root/a b"),
                Path::new("/old root"),
                Path::new("/new root")
            )
            .unwrap()
            .unwrap(),
            Path::new("/new root/a b")
        );
    }
}
