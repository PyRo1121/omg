//! Shared, ordered mise project configuration. Discovery never executes project code.

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct Document {
    pub path: PathBuf,
    /// Project root, including for grouped `.config/mise/config.toml` files.
    pub root: PathBuf,
    pub value: toml::Value,
}

fn selected_environments(env: &HashMap<String, String>) -> Result<Vec<String>> {
    let mut selected = Vec::new();
    for name in env
        .get("MISE_ENV")
        .into_iter()
        .flat_map(|value| value.split(','))
    {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        anyhow::ensure!(
            name.chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c)),
            "Invalid mise environment name; expected letters, digits, underscores or hyphens"
        );
        selected.retain(|existing| existing != name);
        selected.push(name.to_owned());
    }
    Ok(selected)
}

fn add_document(
    documents: &mut Vec<Document>,
    seen: &mut HashSet<PathBuf>,
    path: PathBuf,
    root: &Path,
) -> Result<()> {
    if seen.contains(&path) {
        return Ok(());
    }
    let Some(text) = super::mise_env::read_bounded_regular_file(&path)? else {
        return Ok(());
    };
    anyhow::ensure!(documents.len() < 256, "Too many mise configuration files");
    let value: toml::Value = toml::from_str(&text)
        .with_context(|| format!("Failed to parse mise configuration {}", path.display()))?;
    seen.insert(path.clone());
    documents.push(Document {
        path,
        root: root.to_owned(),
        value,
    });
    Ok(())
}

fn add_fragments(
    documents: &mut Vec<Document>,
    seen: &mut HashSet<PathBuf>,
    directory: &Path,
    root: &Path,
) -> Result<()> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error).with_context(|| format!("Read {}", directory.display())),
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "toml")
            && path
                .file_name()
                .is_some_and(|name| !name.to_string_lossy().starts_with('.'))
        {
            anyhow::ensure!(files.len() < 256, "Too many mise configuration fragments");
            files.push(path);
        }
    }
    files.sort();
    for path in files {
        add_document(documents, seen, path, root)?;
    }
    Ok(())
}

/// Return low-to-high precedence documents, preserving each declaring root.
/// `env` is supplied by the caller so configuration resolution never mutates process state.
pub(crate) fn load(start: &Path, env: &HashMap<String, String>) -> Result<Vec<Document>> {
    let selected = selected_environments(env)?;
    let mut roots: Vec<_> = start.ancestors().collect();
    roots.reverse();
    let mut documents = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        // Pinned upstream LOCAL_CONFIG_FILENAMES order, with overlays applied
        // at each directory rather than above the complete project hierarchy.
        for group in [".config/mise", ".mise", "mise"] {
            add_fragments(
                &mut documents,
                &mut seen,
                &root.join(group).join("conf.d"),
                root,
            )?;
            add_document(
                &mut documents,
                &mut seen,
                root.join(group).join("config.toml"),
                root,
            )?;
            if group == ".config/mise" {
                add_document(
                    &mut documents,
                    &mut seen,
                    root.join(".config/mise/mise.toml"),
                    root,
                )?;
                add_document(
                    &mut documents,
                    &mut seen,
                    root.join(".config/mise.toml"),
                    root,
                )?;
            }
        }
        for name in ["mise.toml", ".mise.toml"] {
            add_document(&mut documents, &mut seen, root.join(name), root)?;
        }
        let stems = [
            ".config/mise/config",
            ".config/mise",
            "mise/config",
            "mise",
            ".mise/config",
            ".mise",
        ];
        for selected_env in &selected {
            for stem in stems {
                add_document(
                    &mut documents,
                    &mut seen,
                    root.join(format!("{stem}.{selected_env}.toml")),
                    root,
                )?;
            }
        }
        for name in [
            ".config/mise/config.local.toml",
            ".config/mise/mise.local.toml",
            ".config/mise.local.toml",
            ".mise/config.local.toml",
            "mise/config.local.toml",
            "mise.local.toml",
            ".mise.local.toml",
        ] {
            add_document(&mut documents, &mut seen, root.join(name), root)?;
        }
        for selected_env in &selected {
            for stem in stems {
                add_document(
                    &mut documents,
                    &mut seen,
                    root.join(format!("{stem}.{selected_env}.local.toml")),
                    root,
                )?;
            }
        }
    }
    Ok(documents)
}
