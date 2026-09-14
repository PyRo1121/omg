//! Compiles the production mise modules independently of platform package backends.
#![allow(dead_code)]

#[path = "../../../src/config/mise_config.rs"]
mod mise_config;
#[path = "../../../src/config/mise_env.rs"]
mod mise_env;

#[cfg(test)]
mod compatibility {
    use super::mise_env::{Strictness, load_mise_env_chain};
    use std::collections::HashMap;
    use std::fs;

    #[test]
    fn local_environment_overrides_project_configuration() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("mise.toml"), "[env]\nMODE='base'\n").unwrap();
        fs::write(root.path().join("mise.local.toml"), "[env]\nMODE='local'\n").unwrap();
        let env = load_mise_env_chain(root.path(), &HashMap::new(), Strictness::Strict).unwrap();
        assert!(env.set.contains(&("MODE".into(), "local".into())));
    }

    #[test]
    fn selected_environment_applies_before_its_local_override() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("mise.toml"), "[env]\nMODE='base'\n").unwrap();
        fs::write(
            root.path().join("mise.test.toml"),
            "[env]\nMODE='test'\nTEST_ONLY='yes'\n",
        )
        .unwrap();
        fs::write(
            root.path().join("mise.test.local.toml"),
            "[env]\nMODE='local-test'\n",
        )
        .unwrap();
        let base = HashMap::from([("MISE_ENV".into(), "test".into())]);
        let env = load_mise_env_chain(root.path(), &base, Strictness::Strict).unwrap();
        assert!(env.set.contains(&("MODE".into(), "local-test".into())));
        assert!(env.set.contains(&("TEST_ONLY".into(), "yes".into())));
    }

    #[test]
    fn child_base_overrides_parent_selected_environment() {
        let root = tempfile::tempdir().unwrap();
        let child = root.path().join("child");
        fs::create_dir(&child).unwrap();
        fs::write(
            root.path().join("mise.test.toml"),
            "[env]\nMODE='parent'\nINHERITED='yes'\n",
        )
        .unwrap();
        fs::write(child.join("mise.toml"), "[env]\nMODE='child'\n").unwrap();
        let base = HashMap::from([("MISE_ENV".into(), "test".into())]);
        let env = load_mise_env_chain(&child, &base, Strictness::Strict).unwrap();
        assert!(env.set.contains(&("MODE".into(), "child".into())));
        assert!(env.set.contains(&("INHERITED".into(), "yes".into())));
    }
}
