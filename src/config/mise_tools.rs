//! Supported native runtime pins from ordered mise configuration documents.

use std::collections::HashMap;

use super::mise_config::Document;

pub(crate) fn normalize_runtime_name(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "nodejs" | "node" => "node".to_string(),
        "bun" | "bunjs" => "bun".to_string(),
        "python3" | "python" => "python".to_string(),
        "golang" | "go" => "go".to_string(),
        "rustlang" | "rust" => "rust".to_string(),
        other => other.to_string(),
    }
}

/// Later documents replace earlier pins, including aliases for the same runtime.
/// Backend-qualified entries and multi-version requests retain their existing
/// unsupported boundary; they must not become unmanaged installer requests.
pub(crate) fn native_pins(documents: &[Document]) -> HashMap<String, String> {
    let mut pins = HashMap::new();
    for document in documents {
        let Some(tools) = document.value.get("tools").and_then(toml::Value::as_table) else {
            continue;
        };
        for (name, spec) in tools {
            if name.contains(':') {
                continue;
            }
            let version = match spec {
                toml::Value::String(version) => Some(version.as_str()),
                toml::Value::Table(table) => table.get("version").and_then(toml::Value::as_str),
                _ => None,
            };
            if let Some(version) = version.map(str::trim).filter(|version| !version.is_empty()) {
                pins.insert(normalize_runtime_name(name), version.to_owned());
            }
        }
    }
    pins
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    #[test]
    fn selected_local_pins_override_base_and_normalize_aliases() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("mise.toml"),
            "[tools]\nnodejs='20'\nripgrep={version='14'}\n",
        )
        .unwrap();
        fs::write(
            root.path().join("mise.test.local.toml"),
            "[tools]\nnode='22'\n",
        )
        .unwrap();
        let env = HashMap::from([("MISE_ENV".into(), "test".into())]);
        let documents = super::super::mise_config::load_directory(root.path(), &env).unwrap();
        let pins = native_pins(&documents);
        assert_eq!(pins.get("node").map(String::as_str), Some("22"));
        assert_eq!(pins.get("ripgrep").map(String::as_str), Some("14"));
        assert!(!pins.contains_key("nodejs"));
    }

    #[test]
    fn pin_discovery_does_not_resolve_environment_directives_or_backends() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("mise.toml"), "[tools]\npython3='3.13'\n'cargo:example'='1'\nempty=''\n[env._]\nsource='missing.sh'\nfile='missing.env'\n").unwrap();
        let documents =
            super::super::mise_config::load_directory(root.path(), &HashMap::new()).unwrap();
        let pins = native_pins(&documents);
        assert_eq!(pins, HashMap::from([("python".into(), "3.13".into())]));
    }
}
