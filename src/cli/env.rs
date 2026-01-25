use crate::cli::components::Components;
use crate::cli::tea::Cmd;
use crate::cli::{CliContext, CommandRunner, EnvCommands};
use crate::core::env::fingerprint::{DriftReport, EnvironmentState};
use crate::core::http::shared_client;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[async_trait]
impl CommandRunner for EnvCommands {
    async fn execute(&self, _ctx: &CliContext) -> Result<()> {
        match self {
            EnvCommands::Capture => capture().await,
            EnvCommands::Check => check().await,
            EnvCommands::Share {
                description,
                public,
            } => share(description.clone(), *public).await,
            EnvCommands::Sync { url } => sync(url.clone()).await,
        }
    }
}

/// Capture environment state
pub async fn capture() -> Result<()> {
    use crate::cli::packages::execute_cmd;

    execute_cmd(Components::loading("Capturing environment state..."));

    let state = EnvironmentState::capture().await?;
    state.save("omg.lock")?;

    execute_cmd(Cmd::batch([
        Components::success("Environment state captured"),
        Components::kv_list(
            Some("Capture Details"),
            vec![
                ("File", "omg.lock"),
                ("Hash", &state.hash[..16]),
                ("Packages", &state.packages.len().to_string()),
            ],
        ),
        Components::complete("Environment state saved to omg.lock"),
    ]));

    Ok(())
}

/// Check for environment drift
pub async fn check() -> Result<()> {
    use crate::cli::packages::execute_cmd;

    if !std::path::Path::new("omg.lock").exists() {
        execute_cmd(Components::error_with_suggestion(
            "No omg.lock file found",
            "Run 'omg env capture' to create an environment lockfile",
        ));
        anyhow::bail!("No omg.lock file found");
    }

    execute_cmd(Components::loading("Checking for environment drift..."));

    let expected = EnvironmentState::load("omg.lock")?;
    let current = EnvironmentState::capture().await?;

    let report = DriftReport::compare(&expected, &current);

    if report.has_drift {
        execute_cmd(Cmd::batch([
            Components::warning("Environment drift detected"),
            Components::spacer(),
            Cmd::println("  The following differences were found:"),
        ]));
        report.print();
        anyhow::bail!("Environment drift detected");
    }

    execute_cmd(Cmd::batch([
        Components::success("Environment is in sync"),
        Components::spacer(),
        Components::kv_list(
            Some("Environment Status"),
            vec![("Lockfile", "omg.lock"), ("Status", "No drift detected")],
        ),
    ]));

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
    use crate::cli::packages::execute_cmd;

    // SECURITY: Validate description
    if description.len() > 1000 {
        execute_cmd(Components::error(
            "Description too long (max 1000 characters)",
        ));
        anyhow::bail!("Description too long");
    }

    if !std::path::Path::new("omg.lock").exists() {
        execute_cmd(Components::error_with_suggestion(
            "No omg.lock file found",
            "Run 'omg env capture' to create an environment lockfile",
        ));
        anyhow::bail!("No omg.lock file found");
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

    execute_cmd(Components::loading("Uploading to GitHub Gist..."));

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
        execute_cmd(Components::error(&format!(
            "Failed to create gist: {} - {}",
            status, text
        )));
        anyhow::bail!("Failed to create gist: {status} - {text}");
    }

    let gist_resp: GistResponse = response.json().await?;

    execute_cmd(Cmd::batch([
        Components::success("Environment shared successfully!"),
        Components::kv_list(
            Some("Gist Details"),
            vec![
                ("URL", &gist_resp.html_url),
                (
                    "Visibility",
                    &(if public {
                        "Public".to_string()
                    } else {
                        "Private".to_string()
                    }),
                ),
            ],
        ),
    ]));

    Ok(())
}

/// Sync environment from Gist
pub async fn sync(url_or_id: String) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    // SECURITY: Basic validation for input
    if url_or_id.len() > 255 || url_or_id.chars().any(char::is_control) {
        execute_cmd(Components::error("Invalid Gist URL or ID"));
        anyhow::bail!("Invalid Gist URL or ID");
    }

    execute_cmd(Components::loading("Syncing environment..."));

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
        let status = response.status();
        execute_cmd(Components::error(&format!(
            "Failed to fetch Gist: {}",
            status
        )));
        anyhow::bail!("Failed to fetch Gist: {}", status);
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
        execute_cmd(Cmd::batch([
            Components::success("omg.lock updated from Gist"),
            Components::info("Running environment check..."),
        ]));

        // Auto-check
        check().await?;
    } else {
        execute_cmd(Components::error_with_suggestion(
            "Gist does not contain omg.lock",
            "Ensure the Gist was created with 'omg env share'",
        ));
        anyhow::bail!("Gist does not contain omg.lock");
    }

    Ok(())
}
