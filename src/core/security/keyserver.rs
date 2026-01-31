use anyhow::{Context, Result};
use sequoia_net::KeyServer;
use sequoia_openpgp::{Cert, KeyHandle, parse::Parse};
use std::path::Path;
use std::time::Duration;

const DEFAULT_KEYSERVER: &str = "hkps://keyserver.ubuntu.com";
const KEYSERVER_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn fetch_key(key_id: &str) -> Result<Cert> {
    fetch_key_from(key_id, DEFAULT_KEYSERVER).await
}

pub async fn fetch_key_from(key_id: &str, keyserver_url: &str) -> Result<Cert> {
    let key_handle: KeyHandle = key_id
        .parse()
        .with_context(|| format!("Invalid key ID format: {key_id}"))?;

    let ks = KeyServer::new(keyserver_url)
        .with_context(|| format!("Failed to connect to keyserver: {keyserver_url}"))?;

    let certs = tokio::time::timeout(KEYSERVER_TIMEOUT, ks.get(key_handle))
        .await
        .with_context(|| format!("Timeout fetching key {key_id} from {keyserver_url}"))?
        .with_context(|| format!("Failed to fetch key {key_id} from {keyserver_url}"))?;

    certs
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No certificates found for {key_id}"))?
        .with_context(|| format!("Failed to parse certificate for {key_id}"))
}

pub async fn fetch_keys(key_ids: &[String]) -> Vec<(String, Result<Cert>)> {
    let handles: Vec<_> = key_ids
        .iter()
        .map(|kid| {
            let kid = kid.clone();
            tokio::spawn(async move {
                let result = fetch_key(&kid).await;
                (kid, result)
            })
        })
        .collect();

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }
    results
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
        if cert.keys().any(|k| k.key().key_handle().aliases(&key_handle)) {
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

    #[test]
    fn test_key_info_display() {
        let info = KeyInfo {
            fingerprint: "ABCD1234".to_string(),
            user_ids: vec!["Test User <test@example.com>".to_string()],
        };
        assert_eq!(
            format!("{info}"),
            "ABCD1234 (Test User <test@example.com>)"
        );
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
