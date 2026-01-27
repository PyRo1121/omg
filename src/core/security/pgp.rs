use anyhow::{Context, Result};
use openpgp::Cert;
use openpgp::Packet;
use openpgp::parse::Parse;
use openpgp::parse::{PacketParser, PacketParserResult};
use openpgp::policy::StandardPolicy;
use sequoia_openpgp as openpgp;
use std::path::Path;

/// PGP verification engine using Sequoia
pub struct PgpVerifier {
    policy: StandardPolicy<'static>,
    certs: Vec<Cert>,
}

impl Default for PgpVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl PgpVerifier {
    #[must_use]
    pub fn new() -> Self {
        let distro = crate::core::env::distro::detect_distro();
        let system_keyring = match distro {
            crate::core::env::distro::Distro::Debian | crate::core::env::distro::Distro::Ubuntu => {
                "/usr/share/keyrings/debian-archive-keyring.gpg"
            }
            crate::core::env::distro::Distro::Arch | crate::core::env::distro::Distro::Unknown => {
                "/usr/share/pacman/keyrings/archlinux.gpg"
            }
        };

        let certs = if std::path::Path::new(system_keyring).exists() {
            std::fs::File::open(system_keyring)
                .ok()
                .map(|mut f| {
                    openpgp::cert::CertParser::from_reader(&mut f)
                        .map(|parser| parser.collect::<Result<Vec<_>, _>>().unwrap_or_default())
                        .unwrap_or_default()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        Self {
            policy: StandardPolicy::new(),
            certs,
        }
    }

    /// Verify a file against a detached signature
    pub fn verify_detached(
        &self,
        file_path: &Path,
        sig_path: &Path,
        _keyring_path: &Path,
    ) -> Result<()> {
        let mut sig_file = std::fs::File::open(sig_path)?;

        // Parse the signature packets
        let mut valid_signature_found = false;
        let mut ppr = PacketParser::from_reader(&mut sig_file)?;

        while let PacketParserResult::Some(pp) = ppr {
            if let Packet::Signature(sig) = &pp.packet {
                let algo = sig.hash_algo();
                let issuers = sig.get_issuers();

                // 1. Calculate the hash ONCE for this signature's algorithm
                let mut hasher = algo.context()?.for_signature(sig.version());
                let mut data_file = std::fs::File::open(file_path)?;
                std::io::copy(&mut data_file, &mut hasher)?;

                for cert in &self.certs {
                    // Check if this cert might be the issuer
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
                            // 2. Verify against the pre-calculated hasher (cloned)
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
            ppr = pp.next()?.1;
        }

        if valid_signature_found {
            Ok(())
        } else {
            anyhow::bail!("No valid signature found for package")
        }
    }

    /// Verify data against a detached signature (memory-based)
    pub fn verify_memory(&self, data: &[u8], signature: &[u8]) -> Result<()> {
        use std::io::Cursor;

        // Parse signature
        let mut ppr = PacketParser::from_reader(Cursor::new(signature))?;
        let mut valid_signature_found = false;

        while let PacketParserResult::Some(pp) = ppr {
            if let Packet::Signature(sig) = &pp.packet {
                let algo = sig.hash_algo();
                let issuers = sig.get_issuers();

                // Calculate hash of data
                let mut hasher = algo.context()?.for_signature(sig.version());
                hasher.update(data);

                for cert in &self.certs {
                    // Check if cert is relevant (optimization)
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
            ppr = pp.next()?.1;
        }

        if valid_signature_found {
            Ok(())
        } else {
            anyhow::bail!("No valid signature found")
        }
    }

    /// Verify an Arch Linux package signature (.sig)
    pub fn verify_package<P: AsRef<Path>>(&self, pkg_path: P, sig_path: P) -> Result<()> {
        let system_keyring = "/usr/share/pacman/keyrings/archlinux.gpg";
        if !std::path::Path::new(system_keyring).exists() {
            anyhow::bail!("System keyring not found at {system_keyring}");
        }

        self.verify_detached(
            pkg_path.as_ref(),
            sig_path.as_ref(),
            std::path::Path::new(system_keyring),
        )
        .context("Signature verification failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_pgp_verifier_new() {
        let verifier = PgpVerifier::new();
        // Should construct successfully even without system keyring
        assert!(verifier.policy.time().is_none());
    }

    #[test]
    fn test_pgp_verifier_default() {
        let verifier = PgpVerifier::default();
        // Default should be equivalent to new()
        assert!(verifier.policy.time().is_none());
    }

    #[test]
    fn test_verify_detached_missing_signature() {
        let verifier = PgpVerifier::new();

        // Create a test file
        let mut data_file = NamedTempFile::new().unwrap();
        writeln!(data_file, "test data").unwrap();
        data_file.flush().unwrap();

        // Non-existent signature file
        let sig_path = std::path::Path::new("/nonexistent.sig");
        let keyring_path = std::path::Path::new("/nonexistent.gpg");

        let result = verifier.verify_detached(data_file.path(), sig_path, keyring_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_detached_missing_data_file() {
        let verifier = PgpVerifier::new();

        // Create a temporary signature file (even if invalid)
        let mut sig_file = NamedTempFile::new().unwrap();
        writeln!(sig_file, "fake signature").unwrap();
        sig_file.flush().unwrap();

        let data_path = std::path::Path::new("/nonexistent.data");
        let keyring_path = std::path::Path::new("/nonexistent.gpg");

        let result = verifier.verify_detached(data_path, sig_file.path(), keyring_path);
        // Should fail when trying to read data file
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_memory_invalid_signature() {
        let verifier = PgpVerifier::new();
        let data = b"test data";
        let invalid_sig = b"not a real signature";

        let result = verifier.verify_memory(data, invalid_sig);
        // Should fail with invalid signature format
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_memory_empty_signature() {
        let verifier = PgpVerifier::new();
        let data = b"test data";
        let empty_sig = b"";

        let result = verifier.verify_memory(data, empty_sig);
        // Should fail with empty signature
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_package_missing_keyring() {
        let verifier = PgpVerifier::new();

        let mut pkg = NamedTempFile::new().unwrap();
        writeln!(pkg, "package data").unwrap();
        pkg.flush().unwrap();

        let mut sig = NamedTempFile::new().unwrap();
        writeln!(sig, "signature").unwrap();
        sig.flush().unwrap();

        // This test will check the keyring path validation
        // On non-Arch systems, the keyring won't exist
        let result = verifier.verify_package(pkg.path(), sig.path());

        // May succeed on Arch Linux, should fail on other systems
        // We're just testing that it doesn't panic
        let _ = result;
    }
}
