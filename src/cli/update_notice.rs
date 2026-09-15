//! Best-effort update notice. Cached metadata is advisory, never installation authority.

#[cfg(unix)]
mod unix {
    use anyhow::{Context, Result, ensure};
    use semver::Version;
    use serde::{Deserialize, Serialize};
    use std::fs::File;
    use std::io::{IsTerminal, Read, Seek, Write};
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::process::CommandExt;
    use std::path::{Component, Path};
    use std::process::{Command, Stdio};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const DAY: u64 = 86_400;
    const MAX_CACHE: u64 = 512;

    #[derive(Default, Deserialize, Serialize)]
    struct Cache {
        checked: u64,
        latest: Option<String>,
        notified: u64,
    }

    const fn elapsed(now: u64, then: u64, interval: u64) -> bool {
        then == 0 || now < then || now - then >= interval
    }

    fn notice(cache: &Cache, current: &Version, now: u64) -> Option<Version> {
        if elapsed(now, cache.checked, DAY * 7) || !elapsed(now, cache.notified, DAY) {
            return None;
        }
        let latest = Version::parse(cache.latest.as_deref()?).ok()?;
        (latest > *current && latest.pre.is_empty() && latest.build.is_empty()).then_some(latest)
    }

    fn open_cache(path: &Path) -> Result<File> {
        use rustix::fs::{Mode, OFlags, mkdirat, open, openat};
        ensure!(path.is_absolute(), "Notice cache must be absolute");
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mut directory = open("/", flags, Mode::empty())?;
        for component in path.parent().context("Cache parent")?.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(name) => {
                    directory = match openat(&directory, name, flags, Mode::empty()) {
                        Ok(fd) => fd,
                        Err(rustix::io::Errno::NOENT) => {
                            match mkdirat(&directory, name, Mode::RWXU) {
                                Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                                Err(error) => return Err(error.into()),
                            }
                            openat(&directory, name, flags, Mode::empty())?
                        }
                        Err(error) => return Err(error.into()),
                    };
                }
                _ => anyhow::bail!("Unsafe cache path"),
            }
        }
        let directory = File::from(directory);
        let owner = nix::unistd::geteuid().as_raw();
        let metadata = directory.metadata()?;
        ensure!(
            metadata.uid() == owner && metadata.mode() & 0o022 == 0,
            "Shared cache directory"
        );
        let file = File::from(openat(
            &directory,
            path.file_name().context("Cache filename")?,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )?);
        let metadata = file.metadata()?;
        ensure!(
            metadata.is_file()
                && metadata.nlink() == 1
                && metadata.uid() == owner
                // The six group/other permission bits must all be clear.
                && metadata.mode().trailing_zeros() >= 6
                && metadata.len() <= MAX_CACHE,
            "Unsafe notice cache"
        );
        file.try_lock().context("Notice cache busy")?;
        Ok(file)
    }

    fn read_cache(file: &mut File) -> Result<Cache> {
        let mut body = String::new();
        file.take(MAX_CACHE + 1).read_to_string(&mut body)?;
        ensure!(body.len() as u64 <= MAX_CACHE, "Notice cache too large");
        if body.is_empty() {
            Ok(Cache::default())
        } else {
            Ok(serde_json::from_str(&body)?)
        }
    }

    fn save_cache(file: &mut File, cache: &Cache) -> Result<()> {
        let body = serde_json::to_vec(cache)?;
        ensure!(body.len() as u64 <= MAX_CACHE, "Notice cache too large");
        file.rewind()?;
        file.set_len(0)?;
        file.write_all(&body)?;
        Ok(())
    }

    pub(super) fn run(refresh: bool) -> Result<()> {
        if crate::core::is_root()
            || std::env::var_os("CI").is_some()
            || std::env::var_os("OMG_NO_UPDATE_CHECK").is_some()
            || (!refresh && !std::io::stderr().is_terminal())
        {
            return Ok(());
        }
        let path = crate::core::paths::cache_dir().join("update-notice.json");
        let mut file = open_cache(&path)?;
        let mut cache = read_cache(&mut file)?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        if refresh {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            cache.checked = now;
            cache.latest = runtime.block_on(async {
                tokio::time::timeout(
                    Duration::from_secs(3),
                    super::super::self_update::fetch_latest_version(),
                )
                .await
                .ok()?
                .ok()
                .map(|version| version.to_string())
            });
            save_cache(&mut file, &cache)?;
            return Ok(());
        }
        let current = Version::parse(env!("CARGO_PKG_VERSION"))?;
        if let Some(latest) = notice(&cache, &current, now) {
            eprintln!(
                "OMG {latest} is available (installed: {current}). Update with your package manager or `omg self-update`. Notes: https://github.com/omg-cli/omg/releases/tag/v{latest}"
            );
            cache.notified = now;
            save_cache(&mut file, &cache)?;
        }
        if elapsed(now, cache.checked, DAY) {
            // Reserve the attempt before spawning: simultaneous shells must not stampede.
            cache.checked = now;
            cache.latest = None;
            save_cache(&mut file, &cache)?;
            drop(file);
            // This private process exits immediately; its child is reparented.
            // Waiting here would block shell startup on the network.
            #[allow(clippy::zombie_processes)]
            let _child = Command::new(std::env::current_exe()?)
                .args(["__update-notice", "--refresh"])
                .current_dir("/")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn()?;
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn notices_only_new_stable_versions_with_fresh_daily_policy() {
            let current = Version::parse("0.1.220").unwrap();
            let now = DAY * 10;
            let mut cache = Cache {
                checked: now,
                latest: Some("0.1.221".into()),
                notified: 0,
            };
            assert!(notice(&cache, &current, now).is_some());
            for value in [
                "0.1.220",
                "0.1.219",
                "0.1.222-rc.1",
                "0.1.221+meta",
                "0.1.221\n\x1b[31m",
                "$(touch injected)",
            ] {
                cache.latest = Some(value.into());
                assert!(notice(&cache, &current, now).is_none());
            }
            cache.latest = Some("0.1.221".into());
            cache.notified = now;
            assert!(notice(&cache, &current, now).is_none());
            cache.notified = 0;
            cache.checked = now - DAY * 7;
            assert!(notice(&cache, &current, now).is_none());
            assert!(elapsed(now, now + DAY, DAY));
        }

        #[test]
        fn cache_is_bounded_locked_and_refuses_links() {
            use std::os::unix::fs::{PermissionsExt, symlink};
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().canonicalize().unwrap();
            let path = root.join("notice.json");
            let mut file = open_cache(&path).unwrap();
            assert!(open_cache(&path).is_err());
            let cache = Cache {
                checked: DAY,
                latest: Some("0.1.221".into()),
                notified: 0,
            };
            save_cache(&mut file, &cache).unwrap();
            file.rewind().unwrap();
            assert_eq!(read_cache(&mut file).unwrap().latest, cache.latest);
            drop(file);
            let link = root.join("link.json");
            symlink(&path, &link).unwrap();
            assert!(open_cache(&link).is_err());
            std::fs::hard_link(&path, root.join("hardlink")).unwrap();
            assert!(open_cache(&path).is_err());
            let big = root.join("big.json");
            std::fs::write(&big, vec![b'x'; 513]).unwrap();
            std::fs::set_permissions(&big, std::fs::Permissions::from_mode(0o600)).unwrap();
            assert!(open_cache(&big).is_err());
            let dir_link = root.join("linked-dir");
            symlink(&root, &dir_link).unwrap();
            assert!(open_cache(&dir_link.join("other.json")).is_err());
        }
    }
}

#[cfg(any(unix, test))]
fn command_notice_allowed(args: &[String]) -> bool {
    args.get(1).is_some_and(|command| {
        !command.starts_with('-')
            && !command.starts_with("__")
            && !matches!(
                command.as_str(),
                "self-update" | "help" | "version" | "hook" | "env" | "completions" | "daemon"
            )
    }) && !args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "--json" | "--quiet" | "-q" | "--help" | "-h" | "--version" | "-V"
        ) || arg.starts_with("--format")
    })
}

/// Print a cached advisory after successful interactive commands, before exit.
/// Background refresh never blocks the completed command or installs an update.
pub fn after_command() {
    #[cfg(unix)]
    {
        use std::io::IsTerminal;
        if !std::io::stdout().is_terminal() || !std::io::stderr().is_terminal() {
            return;
        }
        let args: Vec<String> = std::env::args().collect();
        if command_notice_allowed(&args)
            && let Err(error) = unix::run(false)
        {
            tracing::debug!("Update notice unavailable: {error:#}");
        }
    }
}

/// Handle the private shell protocol before telemetry, logging or runtime startup.
pub fn try_handle(args: &[String]) -> bool {
    if !(args.len() == 2 || (args.len() == 3 && args[2] == "--refresh"))
        || args.get(1).map(String::as_str) != Some("__update-notice")
    {
        return false;
    }
    #[cfg(unix)]
    if let Err(error) = unix::run(args.len() == 3) {
        tracing::debug!("Update notice unavailable: {error:#}");
    }
    true
}

#[cfg(test)]
mod command_tests {
    use super::command_notice_allowed;

    #[test]
    fn notices_target_interactive_commands_not_machine_or_update_protocols() {
        for command in ["status", "install", "search", "update"] {
            assert!(command_notice_allowed(&["omg".into(), command.into()]));
        }
        for args in [
            vec!["omg"],
            vec!["omg", "__update-notice"],
            vec!["omg", "self-update"],
            vec!["omg", "hook", "bash"],
            vec!["omg", "completions", "zsh"],
            vec!["omg", "env"],
            vec!["omg", "status", "--json"],
            vec!["omg", "status", "--quiet"],
            vec!["omg", "status", "-q"],
            vec!["omg", "status", "--help"],
            vec!["omg", "status", "--format=json"],
        ] {
            let args = args.into_iter().map(String::from).collect::<Vec<_>>();
            assert!(!command_notice_allowed(&args), "{args:?}");
        }
    }
}
