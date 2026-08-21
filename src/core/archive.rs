//! Shared archive path validation.

use anyhow::Result;
use std::path::{Path, PathBuf};

pub(crate) fn stripped_archive_path(
    path: &Path,
    strip_components: usize,
) -> Result<Option<PathBuf>> {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(value) => components.push(value.to_owned()),
            std::path::Component::CurDir => {}
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::ParentDir => {
                anyhow::bail!("Unsafe path in archive: {}", path.display());
            }
        }
    }

    let stripped = components
        .into_iter()
        .skip(strip_components)
        .collect::<PathBuf>();
    Ok((!stripped.as_os_str().is_empty()).then_some(stripped))
}

#[cfg(test)]
mod tests {
    use super::stripped_archive_path;
    use std::path::{Path, PathBuf};

    #[test]
    fn archive_paths_are_stripped_without_changing_containment() -> anyhow::Result<()> {
        assert_eq!(
            stripped_archive_path(Path::new("runtime/bin/tool"), 1)?,
            Some(PathBuf::from("bin/tool"))
        );
        assert_eq!(stripped_archive_path(Path::new("runtime"), 1)?, None);
        Ok(())
    }

    #[test]
    fn archive_paths_reject_absolute_and_parent_components() {
        for path in ["../escape", "runtime/../../escape", "/absolute/path"] {
            assert!(stripped_archive_path(Path::new(path), 1).is_err(), "{path}");
        }
    }
}
