use crate::core::env::fingerprint::{DriftReport, EnvironmentState};
use crate::core::http::shared_client;
use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Capture environment state
pub async fn capture() -> Result<()> {
    println!("{} Capturing environment state...", "OMG".cyan().bold());

    let state = EnvironmentState::capture().await?;
    state.save("omg.lock")?;

    println!("{} Environment state saved to omg.lock", "✓".green());
    println!("  Hash: {}", state.hash.dimmed());
    Ok(())
}

/// Check for environment drift
pub async fn check() -> Result<()> {
    if !std::path::Path::new("omg.lock").exists() {
        anyhow::bail!("No omg.lock file found. Run 'omg env capture' first.");
    }

    println!("{} Checking for environment drift...", "OMG".cyan().bold());

    let expected = EnvironmentState::load("omg.lock")?;
    let current = EnvironmentState::capture().await?;

    let report = DriftReport::compare(&expected, &current);
    report.print();

    if report.has_drift {
        std::process::exit(1);
    }

    Ok(())
}

#[derive(Serialize)]
struct CreateGist {
    description: String,
    public: bool,
    files: HashMap<String, GistFile>,
}

#[derive(Serialize)]
struct GistFile {
    content: String,
}

#[derive(Deserialize)]
struct GistResponse {
    html_url: String,
    files: HashMap<String, GistFileResponse>,
}

#[derive(Deserialize)]
struct GistFileResponse {
    raw_url: String,
    content: Option<String>,
}

/// Share environment state to GitHub Gist
pub async fn share(description: String, public: bool) -> Result<()> {
    if !std::path::Path::new("omg.lock").exists() {
        anyhow::bail!("No omg.lock file found. Run 'omg env capture' first.");
    }

    let token =
        std::env::var("GITHUB_TOKEN").context("GITHUB_TOKEN environment variable not set")?;
    let content = std::fs::read_to_string("omg.lock")?;

    let mut files = HashMap::new();
    files.insert("omg.lock".to_string(), GistFile { content });

    let gist = CreateGist {
        description,
        public,
        files,
    };

    println!("{} Uploading to GitHub Gist...", "OMG".cyan().bold());

    let client = shared_client();

    let response = client
        .post("https://api.github.com/gists")
        .header("Authorization", format!("token {token}"))
        .json(&gist)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await?;
        anyhow::bail!("Failed to create gist: {status} - {text}");
    }

    let gist_resp: GistResponse = response.json().await?;
    println!("{} Environment shared successfully!", "✓".green());
    println!("  URL: {}", gist_resp.html_url.underline());

    Ok(())
}

/// Sync environment from Gist
pub async fn sync(url_or_id: String) -> Result<()> {
    println!("{} Syncing environment...", "OMG".cyan().bold());

    let client = shared_client();

    // Determine if it's a URL or ID
    let gist_id = if url_or_id.starts_with("https://gist.github.com/") {
        url_or_id
            .split('/')
            .next_back()
            .context("Invalid Gist URL")?
    } else {
        &url_or_id
    };

    let api_url = format!("https://api.github.com/gists/{gist_id}");

    // Authorization is optional for reading public gists, but good if token exists
    let mut req = client.get(&api_url);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("token {token}"));
    }

    let response = req.send().await?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to fetch Gist: {}", response.status());
    }

    let gist_resp: GistResponse = response.json().await?;

    if let Some(file) = gist_resp.files.get("omg.lock") {
        let content = if let Some(c) = &file.content {
            c.clone()
        } else {
            // Fetch raw if content is truncated/missing in metadata
            client.get(&file.raw_url).send().await?.text().await?
        };

        std::fs::write("omg.lock", content)?;
        println!("{} omg.lock updated from Gist", "✓".green());

        // Auto-check
        check().await?;
    } else {
        anyhow::bail!("Gist does not contain omg.lock");
    }

    Ok(())
}
