//! ALPM Test Harness
//!
//! Provides a fully isolated pacman/alpm environment for testing.

use alpm::Alpm;
use anyhow::Result;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::{Builder, EntryType, Header};
use tempfile::{TempDir, tempdir};

pub struct HarnessPkg {
    pub name: String,
    pub version: String,
    pub desc: String,
}

impl HarnessPkg {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            desc: generate_desc(name, version),
        }
    }
}

pub struct AlpmHarness {
    _temp_dir: TempDir,
    root_path: PathBuf,
    db_path: PathBuf,
}

impl AlpmHarness {
    pub fn new() -> Result<Self> {
        let temp_dir = tempdir()?;
        let root = temp_dir.path();

        let harness = Self {
            root_path: root.to_path_buf(),
            db_path: root.join("var/lib/pacman"),
            _temp_dir: temp_dir,
        };

        harness.create_fs_layout()?;
        Ok(harness)
    }

    fn create_fs_layout(&self) -> Result<()> {
        fs::create_dir_all(&self.db_path)?;
        fs::create_dir_all(self.root_path.join("var/cache/pacman/pkg"))?;
        fs::create_dir_all(self.root_path.join("etc"))?;
        fs::create_dir_all(self.db_path.join("local"))?;
        fs::create_dir_all(self.db_path.join("sync"))?;
        Ok(())
    }

    pub fn alpm(&self) -> Result<Alpm> {
        let alpm = Alpm::new(
            self.root_path.to_str().unwrap(),
            self.db_path.to_str().unwrap(),
        )?;
        Ok(alpm)
    }

    pub fn add_sync_pkg(&self, db_name: &str, pkg: &HarnessPkg) -> Result<()> {
        let db_path = self.db_path.join("sync").join(format!("{}.db", db_name));
        let file = File::create(&db_path)?;
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = Builder::new(encoder);

        let pkg_dir = format!("{}-{}/", pkg.name, pkg.version);

        let mut dir_header = Header::new_gnu();
        dir_header.set_path(&pkg_dir)?;
        dir_header.set_size(0);
        dir_header.set_entry_type(EntryType::Directory);
        dir_header.set_mode(0o755);
        dir_header.set_mtime(0);
        dir_header.set_uid(0);
        dir_header.set_gid(0);
        dir_header.set_cksum();
        builder.append(&dir_header, &mut std::io::empty())?;

        let mut file_header = Header::new_gnu();
        file_header.set_path(format!("{}desc", pkg_dir))?;
        file_header.set_size(pkg.desc.len() as u64);
        file_header.set_entry_type(EntryType::Regular);
        file_header.set_mode(0o644);
        file_header.set_mtime(0);
        file_header.set_uid(0);
        file_header.set_gid(0);
        file_header.set_cksum();
        builder.append(&file_header, pkg.desc.as_bytes())?;

        builder.into_inner()?.finish()?;
        Ok(())
    }

    pub fn root(&self) -> &Path {
        &self.root_path
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

pub fn generate_desc(name: &str, version: &str) -> String {
    format!(
        "%NAME%\n{}\n\n%VERSION%\n{}\n\n%DESC%\nA test package\n\n%ARCH%\nx86_64\n\n",
        name, version
    )
}
