//! All client-supplied paths are rejected. Legacy relative paths are read-only migration data.
use crate::object_model::{ObjectError, ObjectResult};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

pub(super) fn filename(value: &str) -> ObjectResult<()> {
    if value.trim().is_empty()
        || value == "."
        || value == ".."
        || value.len() > 255
        || value
            .chars()
            .any(|ch| ch.is_control() || ch == '/' || ch == '\\')
    {
        return Err(ObjectError::invalid("file.filename_invalid"));
    }
    Ok(())
}

pub(super) fn sha256(value: &str) -> ObjectResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|ch| ch.is_ascii_digit() || (b'a'..=b'f').contains(&ch))
    {
        return Err(ObjectError::invalid("file.sha256_invalid"));
    }
    Ok(())
}

pub(super) fn id(value: &str) -> ObjectResult<()> {
    if !value.starts_with("a_")
        || value.len() < 3
        || value.len() > 128
        || !value
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'-')
    {
        return Err(ObjectError::invalid("file.id_invalid"));
    }
    Ok(())
}

pub(super) fn relative(value: &str) -> ObjectResult<()> {
    if value.is_empty()
        || value.len() > 1024
        || value.contains('\\')
        || value.contains(':')
        || value.chars().any(char::is_control)
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || Path::new(value)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(ObjectError::precondition("file.storage_path_invalid"));
    }
    Ok(())
}

pub(super) fn guarded(root: &Path, key: &str, create_parents: bool) -> ObjectResult<PathBuf> {
    relative(key)?;
    let metadata = fs::symlink_metadata(root).map_err(ObjectError::storage)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ObjectError::precondition("file.root_invalid"));
    }
    let root = fs::canonicalize(root).map_err(ObjectError::storage)?;
    let parts = key.split('/').collect::<Vec<_>>();
    let mut current = root.clone();
    for (index, component) in parts.iter().enumerate() {
        current.push(component);
        if index + 1 == parts.len() {
            match fs::symlink_metadata(&current) {
                Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
                    return Err(ObjectError::precondition("file.content_path_invalid"));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(ObjectError::storage(error)),
            }
        } else {
            if create_parents {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(ObjectError::storage(error)),
                }
            }
            let meta = fs::symlink_metadata(&current).map_err(ObjectError::storage)?;
            if meta.file_type().is_symlink()
                || !meta.is_dir()
                || !fs::canonicalize(&current)
                    .map_err(ObjectError::storage)?
                    .starts_with(&root)
            {
                return Err(ObjectError::precondition("file.storage_path_invalid"));
            }
        }
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_traversal_and_platform_aliases() {
        for path in ["../a", "/a", "a//b", "a/./b", "a\\b", "C:a", "a:b", "a\n"] {
            assert!(relative(path).is_err(), "{path:?}");
        }
        assert!(relative("blobs/0123456789abcdef").is_ok());
    }
    #[test]
    fn refuses_unsafe_names_and_invalid_hashes() {
        assert!(filename("设计资料.zip").is_ok());
        assert!(filename("../db").is_err());
        assert!(sha256(&"a".repeat(64)).is_ok());
        assert!(sha256(&"A".repeat(64)).is_err());
    }
}
