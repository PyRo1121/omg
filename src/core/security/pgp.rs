use anyhow::{Context, Result};
use openpgp::parse::Parse;
use openpgp::parse::{PacketParser, PacketParserResult};
use openpgp::policy::StandardPolicy;
use openpgp::Cert;
use openpgp::Packet;
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
        let system_keyring = "/usr/share/pacman/keyrings/archlinux.gpg";
        let certs = if std::path::Path::new(system_keyring).exists() {
            let mut keyring_file = std::fs::File::open(system_keyring).unwrap();
            openpgp::cert::CertParser::from_reader(&mut keyring_file)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
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
