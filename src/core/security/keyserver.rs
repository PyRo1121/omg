//! PGP keyserver integration for automatic key fetching
//!
//! Fetches PGP keys from keyservers (Ubuntu keyserver by default)
//! with timeout handling for signature verification workflows.

use anyhow::{Context, Result};
use futures::{StreamExt, stream};
use reqwest::Url;
use sequoia_openpgp::{Cert, KeyHandle, parse::Parse};
use std::path::Path;
use std::time::Duration;

use crate::core::http::shared_client;

const DEFAULT_KEYSERVER: &str = "hkps://keyserver.ubuntu.com";
const KEYSERVER_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_KEY_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_CONCURRENT_KEY_FETCHES: usize = 8;

pub async fn fetch_key(key_id: &str) -> Result<Cert> {
    fetch_key_from(key_id, DEFAULT_KEYSERVER).await
}

pub async fn fetch_key_from(key_id: &str, keyserver_url: &str) -> Result<Cert> {
    let key_handle: KeyHandle = key_id
        .parse()
        .with_context(|| format!("Invalid key ID format: {key_id}"))?;
    let lookup_url = keyserver_lookup_url(keyserver_url, &key_handle)?;

    let fetch = async {
        let response = shared_client()
            .get(lookup_url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch key {key_id} from {keyserver_url}"))?
            .error_for_status()
            .with_context(|| {
                format!("Keyserver returned an error for key {key_id} from {keyserver_url}")
            })?;

        if let Some(content_length) = response.content_length() {
            anyhow::ensure!(
                content_length <= MAX_KEY_RESPONSE_BYTES as u64,
                "Keyserver response for {key_id} exceeds {MAX_KEY_RESPONSE_BYTES} bytes"
            );
        }

        let mut body = Vec::new();
        let mut chunks = response.bytes_stream();
        while let Some(chunk) = chunks.next().await {
            let chunk = chunk
                .with_context(|| format!("Failed to read keyserver response for key {key_id}"))?;
            anyhow::ensure!(
                body.len().saturating_add(chunk.len()) <= MAX_KEY_RESPONSE_BYTES,
                "Keyserver response for {key_id} exceeds {MAX_KEY_RESPONSE_BYTES} bytes"
            );
            body.extend_from_slice(&chunk);
        }

        let mut certs = sequoia_openpgp::cert::CertParser::from_bytes(&body)
            .with_context(|| format!("Failed to parse certificate response for {key_id}"))?;
        let cert = certs
            .next()
            .ok_or_else(|| anyhow::anyhow!("No certificates found for {key_id}"))?
            .with_context(|| format!("Failed to parse certificate for {key_id}"))?;

        anyhow::ensure!(
            cert.keys()
                .any(|key| key.key().key_handle().aliases(&key_handle)),
            "Keyserver returned a certificate that does not match {key_id}"
        );
        Ok(cert)
    };

    tokio::time::timeout(KEYSERVER_TIMEOUT, fetch)
        .await
        .with_context(|| format!("Timeout fetching key {key_id} from {keyserver_url}"))?
}

fn keyserver_lookup_url(keyserver_url: &str, key_handle: &KeyHandle) -> Result<Url> {
    let normalized = if let Some(authority) = keyserver_url.strip_prefix("hkps://") {
        format!("https://{authority}")
    } else {
        keyserver_url.to_string()
    };
    let mut url = Url::parse(&normalized)
        .with_context(|| format!("Invalid keyserver URL: {keyserver_url}"))?;

    anyhow::ensure!(
        url.scheme() == "https",
        "Keyserver URL must use hkps or https: {keyserver_url}"
    );
    anyhow::ensure!(
        url.host_str().is_some(),
        "Keyserver URL must include a host: {keyserver_url}"
    );
    anyhow::ensure!(
        url.username().is_empty() && url.password().is_none(),
        "Keyserver URL must not include credentials"
    );

    url.set_path("/pks/lookup");
    url.set_query(None);
    url.set_fragment(None);
    url.query_pairs_mut()
        .append_pair("op", "get")
        .append_pair("options", "mr")
        .append_pair("search", &format!("0x{key_handle:X}"));
    Ok(url)
}

pub async fn fetch_keys(key_ids: &[String]) -> Vec<(String, Result<Cert>)> {
    stream::iter(key_ids.iter().cloned())
        .map(|key_id| async move {
            let result = fetch_key(&key_id).await;
            (key_id, result)
        })
        .buffer_unordered(MAX_CONCURRENT_KEY_FETCHES)
        .collect()
        .await
}

pub fn is_key_in_keyring(key_id: &str, keyring_path: &Path) -> Result<bool> {
    if !keyring_path.exists() {
        return Ok(false);
    }

    let key_handle: KeyHandle = key_id.parse().context("Invalid key ID")?;
    let mut file = std::fs::File::open(keyring_path)?;
    let certs = sequoia_openpgp::cert::CertParser::from_reader(&mut file)
        .context("Failed to parse keyring")?;

    for cert in certs.flatten() {
        if cert
            .keys()
            .any(|k| k.key().key_handle().aliases(&key_handle))
        {
            return Ok(true);
        }
    }

    Ok(false)
}

pub fn append_to_keyring(cert: &Cert, keyring_path: &Path) -> Result<()> {
    use sequoia_openpgp::serialize::Serialize;
    use std::fs::OpenOptions;
    use std::io::Write;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(keyring_path)
        .with_context(|| format!("Failed to open keyring: {}", keyring_path.display()))?;

    let mut buf = Vec::new();
    cert.serialize(&mut buf)?;
    file.write_all(&buf)?;

    Ok(())
}

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

#[derive(Debug, Clone)]
pub struct KeyInfo {
    pub fingerprint: String,
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
            assert!(error.to_string().contains("must use hkps or https"));
        }
    }

    #[test]
    fn keyserver_lookup_rejects_credentials() {
        let error = keyserver_lookup_url(
            "https://user:secret@keyserver.example.com",
            &test_key_handle(),
        )
        .expect_err("keyserver credentials must be rejected");

        assert!(error.to_string().contains("must not include credentials"));
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
}
