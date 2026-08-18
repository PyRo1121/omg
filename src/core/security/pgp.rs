//! PGP signature verification using Sequoia-OpenPGP
//!
//! Verifies detached and inline PGP signatures for package integrity,
//! supports keyring management, and validates against trusted keys.

use std::io::{self, Cursor, Seek};
use std::path::Path;

use openpgp::Cert;
use openpgp::Packet;
use openpgp::cert::CertParser;
use openpgp::parse::Parse;
use openpgp::parse::{PacketParser, PacketParserResult};
use openpgp::policy::StandardPolicy;
use sequoia_openpgp as openpgp;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignatureFileError {
    #[error("Package file missing for '{package_name}': {path}")]
    PackageMissing { package_name: String, path: String },
    #[error("PGP signature missing for '{package_name}': {path}")]
    SignatureMissing { package_name: String, path: String },
}

/// Failures loading a keyring or verifying a detached signature.
#[derive(Debug, Error)]
pub enum PgpError {
    #[error("System keyring not found at {path}")]
    KeyringMissing { path: String },
    #[error("Failed to open keyring '{path}'")]
    KeyringOpen {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("Keyring '{path}' contains no certificates")]
    KeyringEmpty { path: String },
    #[error("Failed to parse keyring '{path}'")]
    KeyringParse {
        path: String,
        #[source]
        source: anyhow::Error,
    },
    #[error("Failed to open signature '{path}'")]
    SignatureOpen {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("Failed to parse signature '{path}'")]
    SignatureParse {
        path: String,
        #[source]
        source: anyhow::Error,
    },
    #[error("Failed to parse signature bytes")]
    SignatureBytesParse {
        #[source]
        source: anyhow::Error,
    },
    #[error("Failed to open package '{path}'")]
    PackageOpen {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("Failed to initialize signature hash")]
    HashContext {
        #[source]
        source: anyhow::Error,
    },
    #[error("No valid signature found")]
    NoValidSignature,
}

/// PGP verification engine using Sequoia
pub struct PgpVerifier {
    policy: StandardPolicy<'static>,
    certs: Vec<Cert>,
}

fn system_keyring_path() -> Option<&'static str> {
    match crate::core::env::distro::detect_distro() {
        crate::core::env::distro::Distro::Debian | crate::core::env::distro::Distro::Ubuntu => {
            Some("/usr/share/keyrings/debian-archive-keyring.gpg")
        }
        crate::core::env::distro::Distro::Fedora => Some("/etc/pki/rpm-gpg/RPM-GPG-KEY-fedora"),
        crate::core::env::distro::Distro::Arch | crate::core::env::distro::Distro::Unknown => {
            Some("/usr/share/pacman/keyrings/archlinux.gpg")
        }
        crate::core::env::distro::Distro::MacOS | crate::core::env::distro::Distro::Windows => None,
    }
}

impl PgpVerifier {
    /// Load the distro keyring. Platforms without a default keyring get an
    /// empty trusted set; verify then fails with [`PgpError::NoValidSignature`].
    pub fn new() -> Result<Self, PgpError> {
        match system_keyring_path() {
            None => Ok(Self::empty()),
            Some(path) => Self::from_keyring(path),
        }
    }

    /// Empty trusted set. Every verify fails closed until certs are loaded.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            policy: StandardPolicy::new(),
            certs: Vec::new(),
        }
    }

    /// Load certificates from a keyring file. Missing, empty, and unreadable
    /// keyrings are errors — they are not treated as an empty trusted set.
    pub fn from_keyring(path: impl AsRef<Path>) -> Result<Self, PgpError> {
        let path = path.as_ref();
        let path_str = path.display().to_string();
        if !path.exists() {
            return Err(PgpError::KeyringMissing { path: path_str });
        }

        let mut file = std::fs::File::open(path).map_err(|source| PgpError::KeyringOpen {
            path: path_str.clone(),
            source,
        })?;
        let parser =
            CertParser::from_reader(&mut file).map_err(|source| PgpError::KeyringParse {
                path: path_str.clone(),
                source,
            })?;
        let certs =
            parser
                .collect::<Result<Vec<_>, _>>()
                .map_err(|source| PgpError::KeyringParse {
                    path: path_str.clone(),
                    source,
                })?;
        if certs.is_empty() {
            return Err(PgpError::KeyringEmpty { path: path_str });
        }

        Ok(Self {
            policy: StandardPolicy::new(),
            certs,
        })
    }

    /// Verify a file against a detached signature using the loaded keyring.
    pub fn verify_detached(&self, file_path: &Path, sig_path: &Path) -> Result<(), PgpError> {
        let mut data_file =
            std::fs::File::open(file_path).map_err(|source| PgpError::PackageOpen {
                path: file_path.display().to_string(),
                source,
            })?;
        let mut sig_file =
            std::fs::File::open(sig_path).map_err(|source| PgpError::SignatureOpen {
                path: sig_path.display().to_string(),
                source,
            })?;

        let mut valid_signature_found = false;
        let mut ppr = PacketParser::from_reader(&mut sig_file).map_err(|source| {
            PgpError::SignatureParse {
                path: sig_path.display().to_string(),
                source,
            }
        })?;

        while let PacketParserResult::Some(pp) = ppr {
            if let Packet::Signature(sig) = &pp.packet {
                let algo = sig.hash_algo();
                let issuers = sig.get_issuers();

                let mut hasher = algo
                    .context()
                    .map_err(|source| PgpError::HashContext { source })?
                    .for_signature(sig.version());
                data_file
                    .seek(io::SeekFrom::Start(0))
                    .map_err(|source| PgpError::PackageOpen {
                        path: file_path.display().to_string(),
                        source,
                    })?;
                std::io::copy(&mut data_file, &mut hasher).map_err(|source| {
                    PgpError::PackageOpen {
                        path: file_path.display().to_string(),
                        source,
                    }
                })?;

                for cert in &self.certs {
                    let mut relevant_cert = issuers.is_empty();
                    if !relevant_cert {
                        for issuer_id in &issuers {
                            if cert.keys().any(|k| k.key().key_handle().aliases(issuer_id)) {
                                relevant_cert = true;
                                break;
                            }
                        }
                    }

                    if relevant_cert {
                        for key in cert
                            .keys()
                            .with_policy(&self.policy, None)
                            .alive()
                            .revoked(false)
                            .for_signing()
                        {
                            if sig.verify_hash(key.key(), hasher.clone()).is_ok() {
                                valid_signature_found = true;
                                break;
                            }
                        }
                    }
                    if valid_signature_found {
                        break;
                    }
                }
            }
            if valid_signature_found {
                break;
            }
            ppr = pp
                .next()
                .map_err(|source| PgpError::SignatureParse {
                    path: sig_path.display().to_string(),
                    source,
                })?
                .1;
        }

        if valid_signature_found {
            Ok(())
        } else {
            Err(PgpError::NoValidSignature)
        }
    }

    /// Verify data against a detached signature (memory-based)
    pub fn verify_memory(&self, data: &[u8], signature: &[u8]) -> Result<(), PgpError> {
        let mut ppr = PacketParser::from_reader(Cursor::new(signature))
            .map_err(|source| PgpError::SignatureBytesParse { source })?;
        let mut valid_signature_found = false;

        while let PacketParserResult::Some(pp) = ppr {
            if let Packet::Signature(sig) = &pp.packet {
                let algo = sig.hash_algo();
                let issuers = sig.get_issuers();

                let mut hasher = algo
                    .context()
                    .map_err(|source| PgpError::HashContext { source })?
                    .for_signature(sig.version());
                hasher.update(data);

                for cert in &self.certs {
                    let mut relevant_cert = issuers.is_empty();
                    if !relevant_cert {
                        for issuer_id in &issuers {
                            if cert.keys().any(|k| k.key().key_handle().aliases(issuer_id)) {
                                relevant_cert = true;
                                break;
                            }
                        }
                    }

                    if relevant_cert {
                        for key in cert
                            .keys()
                            .with_policy(&self.policy, None)
                            .alive()
                            .revoked(false)
                            .for_signing()
                        {
                            if sig.verify_hash(key.key(), hasher.clone()).is_ok() {
                                valid_signature_found = true;
                                break;
                            }
                        }
                    }
                    if valid_signature_found {
                        break;
                    }
                }
            }
            if valid_signature_found {
                break;
            }
            ppr = pp
                .next()
                .map_err(|source| PgpError::SignatureBytesParse { source })?
                .1;
        }

        if valid_signature_found {
            Ok(())
        } else {
            Err(PgpError::NoValidSignature)
        }
    }

    /// Verify an Arch Linux package signature (`.sig`)
    pub fn verify_package<P: AsRef<Path>>(&self, pkg_path: P, sig_path: P) -> Result<(), PgpError> {
        self.verify_detached(pkg_path.as_ref(), sig_path.as_ref())
    }
}

/// Fail closed when a package blob or its detached `.sig` is missing.
/// Skipping unsigned packages is not verification.
pub fn require_detached_signature_files(
    package_name: &str,
    package_path: &Path,
    signature_path: &Path,
) -> Result<(), SignatureFileError> {
    if !package_path.exists() {
        return Err(SignatureFileError::PackageMissing {
            package_name: package_name.to_string(),
            path: package_path.display().to_string(),
        });
    }
    if !signature_path.exists() {
        return Err(SignatureFileError::SignatureMissing {
            package_name: package_name.to_string(),
            path: signature_path.display().to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_verify_detached_missing_signature() {
        let verifier = PgpVerifier::empty();

        let mut data_file = NamedTempFile::new().unwrap();
        writeln!(data_file, "test data").unwrap();
        data_file.flush().unwrap();

        let err = verifier
            .verify_detached(data_file.path(), std::path::Path::new("/nonexistent.sig"))
            .expect_err("missing signature must fail");
        assert!(matches!(err, PgpError::SignatureOpen { .. }), "got: {err}");
    }

    #[test]
    fn test_verify_detached_missing_data_file() {
        let verifier = PgpVerifier::empty();

        let mut sig_file = NamedTempFile::new().unwrap();
        writeln!(sig_file, "fake signature").unwrap();
        sig_file.flush().unwrap();

        let err = verifier
            .verify_detached(std::path::Path::new("/nonexistent.data"), sig_file.path())
            .expect_err("missing package blob must fail");
        assert!(matches!(err, PgpError::PackageOpen { .. }), "got: {err}");
    }

    #[test]
    fn test_verify_memory_invalid_signature() {
        let verifier = PgpVerifier::empty();
        let err = verifier
            .verify_memory(b"test data", b"not a real signature")
            .expect_err("garbage signature bytes must fail");
        assert!(
            matches!(err, PgpError::SignatureBytesParse { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn test_verify_memory_empty_signature() {
        let verifier = PgpVerifier::empty();
        let err = verifier
            .verify_memory(b"test data", b"")
            .expect_err("empty signature must fail");
        assert!(matches!(err, PgpError::NoValidSignature), "got: {err}");
    }

    fn expect_pgp_err(result: Result<PgpVerifier, PgpError>, why: &str) -> PgpError {
        match result {
            Err(err) => err,
            Ok(_) => panic!("{why}"),
        }
    }

    #[test]
    fn missing_keyring_is_an_error() {
        let err = expect_pgp_err(
            PgpVerifier::from_keyring("/nonexistent-omg-keyring.gpg"),
            "missing keyring must fail closed",
        );
        assert!(matches!(err, PgpError::KeyringMissing { .. }), "got: {err}");
    }

    #[test]
    fn empty_keyring_file_is_an_error() {
        let empty = NamedTempFile::new().unwrap();
        let err = expect_pgp_err(
            PgpVerifier::from_keyring(empty.path()),
            "empty keyring must not become an empty trusted set",
        );
        assert!(matches!(err, PgpError::KeyringEmpty { .. }), "got: {err}");
    }

    #[test]
    fn corrupt_keyring_is_an_error() {
        let mut keyring = NamedTempFile::new().unwrap();
        writeln!(keyring, "this is not an OpenPGP keyring").unwrap();
        keyring.flush().unwrap();
        let err = expect_pgp_err(
            PgpVerifier::from_keyring(keyring.path()),
            "corrupt keyring must fail closed",
        );
        assert!(matches!(err, PgpError::KeyringParse { .. }), "got: {err}");
    }

    #[test]
    fn empty_verifier_rejects_garbage_package_signature() {
        let verifier = PgpVerifier::empty();

        let mut pkg = NamedTempFile::new().unwrap();
        writeln!(pkg, "package data").unwrap();
        pkg.flush().unwrap();

        let mut sig = NamedTempFile::new().unwrap();
        writeln!(sig, "signature").unwrap();
        sig.flush().unwrap();

        let err = verifier
            .verify_package(pkg.path(), sig.path())
            .expect_err("garbage signature must fail verification");
        assert!(matches!(err, PgpError::SignatureParse { .. }), "got: {err}");
    }

    #[test]
    fn missing_signature_file_is_not_skipped() {
        let mut pkg = NamedTempFile::new().unwrap();
        writeln!(pkg, "package data").unwrap();
        pkg.flush().unwrap();
        let err = require_detached_signature_files(
            "vim",
            pkg.path(),
            std::path::Path::new("/nonexistent.sig"),
        )
        .expect_err("unsigned packages must not proceed");
        assert!(
            matches!(err, SignatureFileError::SignatureMissing { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn missing_package_file_is_an_error() {
        let mut sig = NamedTempFile::new().unwrap();
        writeln!(sig, "sig").unwrap();
        sig.flush().unwrap();
        let err = require_detached_signature_files(
            "vim",
            std::path::Path::new("/nonexistent.pkg"),
            sig.path(),
        )
        .expect_err("missing package blob must fail");
        assert!(
            matches!(err, SignatureFileError::PackageMissing { .. }),
            "got: {err}"
        );
    }

    #[test]
    fn present_package_and_signature_paths_are_accepted() {
        let mut pkg = NamedTempFile::new().unwrap();
        writeln!(pkg, "package data").unwrap();
        pkg.flush().unwrap();
        let mut sig = NamedTempFile::new().unwrap();
        writeln!(sig, "sig").unwrap();
        sig.flush().unwrap();
        assert!(require_detached_signature_files("vim", pkg.path(), sig.path()).is_ok());
    }
}
