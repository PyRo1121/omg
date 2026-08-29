//! PGP keyserver integration for automatic key fetching
//!
//! Fetches PGP keys from keyservers (Ubuntu keyserver by default)
//! with timeout handling for signature verification workflows.

use std::io;
use std::path::Path;
use std::time::Duration;

use futures::{StreamExt, stream};
use reqwest::Url;
use sequoia_openpgp::{Cert, KeyHandle, parse::Parse};
use thiserror::Error;

use crate::core::http::shared_client;

use super::pgp::SequoiaSource;

const DEFAULT_KEYSERVER: &str = "hkps://keyserver.ubuntu.com";
const KEYSERVER_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_KEY_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_CONCURRENT_KEY_FETCHES: usize = 8;

/// Failures fetching keys or reading a local keyring.
#[derive(Debug, Error)]
pub enum KeyserverError {
    #[error("Invalid key ID format: {key_id}")]
    InvalidKeyId {
        key_id: String,
        #[source]
        source: SequoiaSource,
    },
    #[error("Invalid keyserver URL: {url}")]
    InvalidUrl {
        url: String,
        #[source]
        source: SequoiaSource,
    },
    #[error("Keyserver URL must use hkps or https: {url}")]
    InsecureTransport { url: String },
    #[error("Keyserver URL must include a host: {url}")]
    MissingHost { url: String },
    #[error("Keyserver URL must not include credentials")]
    CredentialsNotAllowed,
    #[error("Failed to fetch key {key_id} from {keyserver}")]
    Fetch {
        key_id: String,
        keyserver: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("Keyserver returned an error for key {key_id} from {keyserver}")]
    HttpStatus {
        key_id: String,
        keyserver: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("Keyserver response for {key_id} exceeds {max_bytes} bytes")]
    ResponseTooLarge { key_id: String, max_bytes: usize },
    #[error("Failed to read keyserver response for key {key_id}")]
    ReadBody {
        key_id: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("Failed to parse certificate response for {key_id}")]
    ParseResponse {
        key_id: String,
        #[source]
        source: SequoiaSource,
    },
    #[error("No certificates found for {key_id}")]
    NoCertificates { key_id: String },
    #[error("Failed to parse certificate for {key_id}")]
    ParseCertificate {
        key_id: String,
        #[source]
        source: SequoiaSource,
    },
    #[error("Keyserver returned a certificate that does not match {key_id}")]
    KeyMismatch { key_id: String },
    #[error("Timeout fetching key {key_id} from {keyserver}")]
    Timeout { key_id: String, keyserver: String },
    #[error("Invalid key ID")]
    InvalidLookupKeyId {
        #[source]
        source: SequoiaSource,
    },
    #[error("Failed to parse keyring")]
    KeyringParse {
        #[source]
        source: SequoiaSource,
    },
    #[error("Failed to parse certificate in keyring")]
    KeyringCertificate {
        #[source]
        source: SequoiaSource,
    },
    #[error("Failed to open keyring: {path}")]
    KeyringOpen {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("Failed to serialize certificate")]
    Serialize {
        #[source]
        source: SequoiaSource,
    },
    #[error("Failed to write keyring: {path}")]
    KeyringWrite {
        path: String,
        #[source]
        source: io::Error,
    },
}

/// Fetch a key from the default Ubuntu keyserver.
pub async fn fetch_key(key_id: &str) -> Result<Cert, KeyserverError> {
    fetch_key_from(key_id, DEFAULT_KEYSERVER).await
}

/// Fetch a key from an explicit keyserver URL (`hkps://` or `https://`).
///
/// The response is bounded in size, matched against the requested key
/// handle, and the whole fetch is bounded by a hard timeout.
pub async fn fetch_key_from(key_id: &str, keyserver_url: &str) -> Result<Cert, KeyserverError> {
    let key_handle: KeyHandle = key_id
        .parse()
        .map_err(|source| KeyserverError::InvalidKeyId {
            key_id: key_id.to_string(),
            source: SequoiaSource(source),
        })?;
    let lookup_url = keyserver_lookup_url(keyserver_url, &key_handle)?;
    let safe_keyserver = crate::core::http::redact_url(keyserver_url);

    let fetch = async {
        let response = shared_client()
            .get(lookup_url)
            .send()
            .await
            .map_err(|source| KeyserverError::Fetch {
                key_id: key_id.to_string(),
                keyserver: safe_keyserver.clone(),
                source,
            })?
            .error_for_status()
            .map_err(|source| KeyserverError::HttpStatus {
                key_id: key_id.to_string(),
                keyserver: safe_keyserver.clone(),
                source,
            })?;

        if let Some(content_length) = response.content_length()
            && content_length > MAX_KEY_RESPONSE_BYTES as u64
        {
            return Err(KeyserverError::ResponseTooLarge {
                key_id: key_id.to_string(),
                max_bytes: MAX_KEY_RESPONSE_BYTES,
            });
        }

        let mut body = Vec::new();
        let mut chunks = response.bytes_stream();
        while let Some(chunk) = chunks.next().await {
            let chunk = chunk.map_err(|source| KeyserverError::ReadBody {
                key_id: key_id.to_string(),
                source,
            })?;
            if body.len().saturating_add(chunk.len()) > MAX_KEY_RESPONSE_BYTES {
                return Err(KeyserverError::ResponseTooLarge {
                    key_id: key_id.to_string(),
                    max_bytes: MAX_KEY_RESPONSE_BYTES,
                });
            }
            body.extend_from_slice(&chunk);
        }

        let mut certs = sequoia_openpgp::cert::CertParser::from_bytes(&body).map_err(|source| {
            KeyserverError::ParseResponse {
                key_id: key_id.to_string(),
                source: SequoiaSource(source),
            }
        })?;
        let cert = certs.next().ok_or_else(|| KeyserverError::NoCertificates {
            key_id: key_id.to_string(),
        })?;
        let cert = cert.map_err(|source| KeyserverError::ParseCertificate {
            key_id: key_id.to_string(),
            source: SequoiaSource(source),
        })?;

        if !cert
            .keys()
            .any(|key| key.key().key_handle().aliases(&key_handle))
        {
            return Err(KeyserverError::KeyMismatch {
                key_id: key_id.to_string(),
            });
        }
        Ok(cert)
    };

    match tokio::time::timeout(KEYSERVER_TIMEOUT, fetch).await {
        Ok(result) => result,
        Err(_) => Err(KeyserverError::Timeout {
            key_id: key_id.to_string(),
            keyserver: safe_keyserver,
        }),
    }
}

fn keyserver_lookup_url(
    keyserver_url: &str,
    key_handle: &KeyHandle,
) -> Result<Url, KeyserverError> {
    let normalized = if let Some(authority) = keyserver_url.strip_prefix("hkps://") {
        format!("https://{authority}")
    } else {
        keyserver_url.to_string()
    };
    let safe_keyserver = crate::core::http::redact_url(keyserver_url);
    let mut url = Url::parse(&normalized).map_err(|source| KeyserverError::InvalidUrl {
        url: safe_keyserver.clone(),
        source: SequoiaSource(source.into()),
    })?;

    if url.scheme() != "https" {
        return Err(KeyserverError::InsecureTransport {
            url: safe_keyserver,
        });
    }
    if url.host_str().is_none() {
        return Err(KeyserverError::MissingHost {
            url: safe_keyserver,
        });
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(KeyserverError::CredentialsNotAllowed);
    }

    url.set_path("/pks/lookup");
    url.set_query(None);
    url.set_fragment(None);
    url.query_pairs_mut()
        .append_pair("op", "get")
        .append_pair("options", "mr")
        .append_pair("search", &format!("0x{key_handle:X}"));
    Ok(url)
}

/// Fetch many keys concurrently (bounded), keeping each key's result
/// separate. Discarding the returned results silently is almost always a
/// bug, hence `#[must_use]`.
#[must_use]
pub async fn fetch_keys(key_ids: &[String]) -> Vec<(String, Result<Cert, KeyserverError>)> {
    stream::iter(key_ids.iter().cloned())
        .map(|key_id| async move {
            let result = fetch_key(&key_id).await;
            (key_id, result)
        })
        .buffer_unordered(MAX_CONCURRENT_KEY_FETCHES)
        .collect()
        .await
}

/// Check whether a keyring file contains a certificate matching `key_id`.
///
/// A missing keyring is a miss (`Ok(false)`); a corrupt keyring fails
/// closed with an error instead of looking like a miss.
pub fn is_key_in_keyring(key_id: &str, keyring_path: &Path) -> Result<bool, KeyserverError> {
    if !keyring_path.exists() {
        return Ok(false);
    }

    let key_handle: KeyHandle =
        key_id
            .parse()
            .map_err(|source| KeyserverError::InvalidLookupKeyId {
                source: SequoiaSource(source),
            })?;
    let mut file =
        std::fs::File::open(keyring_path).map_err(|source| KeyserverError::KeyringOpen {
            path: keyring_path.display().to_string(),
            source,
        })?;
    let certs = sequoia_openpgp::cert::CertParser::from_reader(&mut file).map_err(|source| {
        KeyserverError::KeyringParse {
            source: SequoiaSource(source),
        }
    })?;

    for cert in certs {
        let cert = cert.map_err(|source| KeyserverError::KeyringCertificate {
            source: SequoiaSource(source),
        })?;
        if cert
            .keys()
            .any(|k| k.key().key_handle().aliases(&key_handle))
        {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Append a certificate to a local keyring file atomically and durably.
///
/// The complete prior keyring plus the new certificate is published through a
/// same-directory temporary file. A crash can therefore expose either the old
/// or new keyring, never a truncated append that bricks every later lookup.
pub fn append_to_keyring(cert: &Cert, keyring_path: &Path) -> Result<(), KeyserverError> {
    use sequoia_openpgp::serialize::Serialize;

    let path_str = keyring_path.display().to_string();
    let mut contents = match std::fs::read(keyring_path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(source) => {
            return Err(KeyserverError::KeyringOpen {
                path: path_str,
                source,
            });
        }
    };

    cert.serialize(&mut contents)
        .map_err(|source| KeyserverError::Serialize {
            source: SequoiaSource(source),
        })?;
    crate::core::safe_ops::atomic_write_file_sync(keyring_path, contents).map_err(|error| {
        KeyserverError::KeyringWrite {
            path: path_str,
            source: io::Error::other(error),
        }
    })
}

/// Extract display information (fingerprint, user IDs) from a certificate.
#[must_use]
pub fn get_key_info(cert: &Cert) -> KeyInfo {
    let fingerprint = cert.fingerprint().to_hex();
    let user_ids: Vec<String> = cert
        .userids()
        .map(|uid| String::from_utf8_lossy(uid.userid().value()).to_string())
        .collect();

    KeyInfo {
        fingerprint,
        user_ids,
    }
}

/// Display information about an OpenPGP certificate.
#[derive(Debug, Clone)]
pub struct KeyInfo {
    /// Hex-encoded certificate fingerprint.
    pub fingerprint: String,
    /// User IDs attached to the certificate, lossily decoded as UTF-8.
    pub user_ids: Vec<String>,
}

impl std::fmt::Display for KeyInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.fingerprint)?;
        if !self.user_ids.is_empty() {
            write!(f, " ({})", self.user_ids.join(", "))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key_handle() -> KeyHandle {
        KeyHandle::from(sequoia_openpgp::KeyID::from(0x0123_4567_89AB_CDEF))
    }

    #[test]
    fn keyserver_lookup_normalizes_hkps_and_replaces_untrusted_path() {
        let url = keyserver_lookup_url(
            "hkps://keyserver.ubuntu.com/untrusted?old=value#fragment",
            &test_key_handle(),
        )
        .expect("valid HKPS URL should produce a lookup URL");

        assert_eq!(
            url.as_str(),
            "https://keyserver.ubuntu.com/pks/lookup?op=get&options=mr&search=0x0123456789ABCDEF"
        );
    }

    #[test]
    fn keyserver_lookup_rejects_insecure_transport() {
        let key_handle = test_key_handle();
        for url in [
            "hkp://keyserver.example.com",
            "http://keyserver.example.com",
        ] {
            let error = keyserver_lookup_url(url, &key_handle)
                .expect_err("insecure keyserver transport must be rejected");
            assert!(
                matches!(error, KeyserverError::InsecureTransport { .. }),
                "got: {error}"
            );
        }
    }

    #[test]
    fn keyserver_lookup_rejects_credentials() {
        let error = keyserver_lookup_url(
            "https://user:secret@keyserver.example.com",
            &test_key_handle(),
        )
        .expect_err("keyserver credentials must be rejected");

        assert!(
            matches!(error, KeyserverError::CredentialsNotAllowed),
            "got: {error}"
        );
    }

    #[test]
    fn test_key_info_display() {
        let info = KeyInfo {
            fingerprint: "ABCD1234".to_string(),
            user_ids: vec!["Test User <test@example.com>".to_string()],
        };
        assert_eq!(format!("{info}"), "ABCD1234 (Test User <test@example.com>)");
    }

    #[test]
    fn test_key_info_display_no_uid() {
        let info = KeyInfo {
            fingerprint: "ABCD1234".to_string(),
            user_ids: vec![],
        };
        assert_eq!(format!("{info}"), "ABCD1234");
    }

    #[test]
    fn append_to_keyring_atomically_preserves_existing_certificates() {
        let temp = tempfile::tempdir().unwrap();
        let keyring = temp.path().join("pubring.pgp");
        let (first, _) = sequoia_openpgp::cert::prelude::CertBuilder::new()
            .add_userid("first@example.test")
            .generate()
            .unwrap();
        let (second, _) = sequoia_openpgp::cert::prelude::CertBuilder::new()
            .add_userid("second@example.test")
            .generate()
            .unwrap();

        append_to_keyring(&first, &keyring).unwrap();
        append_to_keyring(&second, &keyring).unwrap();

        assert!(is_key_in_keyring(&first.fingerprint().to_hex(), &keyring).unwrap());
        assert!(is_key_in_keyring(&second.fingerprint().to_hex(), &keyring).unwrap());
    }

    #[test]
    fn is_key_in_keyring_fails_closed_on_corrupt_data() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), "this is not an OpenPGP certificate\n").unwrap();
        let error = is_key_in_keyring("0123456789ABCDEF", temp.path())
            .expect_err("corrupt keyring data must not look like a miss");
        assert!(
            matches!(
                error,
                KeyserverError::KeyringParse { .. } | KeyserverError::KeyringCertificate { .. }
            ),
            "got: {error}"
        );
    }
}
