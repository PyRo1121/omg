use crate::cli::components::Components;
use crate::cli::tea::Cmd;
use crate::cli::{CliContext, EnvCommands, LocalCommandRunner};
use crate::core::env::fingerprint::{DriftReport, EnvironmentState};
use crate::core::http::shared_client;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

impl LocalCommandRunner for EnvCommands {
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
        Cmd::success("Environment state captured"),
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
            Cmd::warning("Environment drift detected"),
            Cmd::spacer(),
            Cmd::println("  The following differences were found:"),
        ]));
        report.print();
        anyhow::bail!("Environment drift detected");
    }

    execute_cmd(Cmd::batch([
        Cmd::success("Environment is in sync"),
        Cmd::spacer(),
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

fn parse_gist_id(input: &str) -> Result<String> {
    let gist_id = if input.starts_with("https://") {
        let url = reqwest::Url::parse(input).context("Invalid Gist URL")?;
        anyhow::ensure!(
            url.scheme() == "https" && url.host_str() == Some("gist.github.com"),
            "Gist URL must use https://gist.github.com"
        );
        anyhow::ensure!(
            url.query().is_none() && url.fragment().is_none(),
            "Gist URL must not contain a query or fragment"
        );
        url.path_segments()
            .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
            .context("Gist URL does not contain an ID")?
            .to_string()
    } else {
        input.to_string()
    };

    anyhow::ensure!(
        (7..=64).contains(&gist_id.len())
            && gist_id
                .chars()
                .all(|character| character.is_ascii_hexdigit()),
        "Gist ID must be 7 to 64 hexadecimal characters"
    );
    Ok(gist_id)
}

/// Share environment state to GitHub Gist
fn sanitize_remote_error_body(body: &str) -> String {
    crate::cli::style::sanitize_terminal_text(body)
        .chars()
        .take(200)
        .collect()
}

pub async fn share(description: String, public: bool) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    // SECURITY: Validate description
    if description.len() > 1000 {
        execute_cmd(Cmd::error("Description too long (max 1000 characters)"));
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
    let content =
        std::fs::read_to_string("omg.lock").context("Failed to read omg.lock for sharing")?;

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
        let body = response.text().await?;
        let safe_body = sanitize_remote_error_body(&body);
        tracing::debug!(status = %status, body_bytes = body.len(), "GitHub Gist request failed");
        execute_cmd(Cmd::error(format!(
            "Failed to create gist: {status} - {safe_body}"
        )));
        anyhow::bail!("Failed to create gist: {status} - {safe_body}");
    }

    let gist_resp: GistResponse = response.json().await?;

    execute_cmd(Cmd::batch([
        Cmd::success("Environment shared successfully!"),
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
        execute_cmd(Cmd::error("Invalid Gist URL or ID"));
        anyhow::bail!("Invalid Gist URL or ID");
    }

    execute_cmd(Components::loading("Syncing environment..."));

    let client = shared_client();

    let gist_id = parse_gist_id(&url_or_id)?;
    let api_url = format!("https://api.github.com/gists/{gist_id}");

    // Authorization is optional for reading public gists, but good if token exists
    let mut req = client.get(&api_url);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("token {token}"));
    }

    let response = req.send().await?;

    if !response.status().is_success() {
        let status = response.status();
        execute_cmd(Cmd::error(format!("Failed to fetch Gist: {status}")));
        anyhow::bail!("Failed to fetch Gist: {status}");
    }

    let gist_resp: GistResponse = response.json().await?;

    if let Some(file) = gist_resp.files.get("omg.lock") {
        let content = if let Some(c) = &file.content {
            c.clone()
        } else {
            // Fetch raw if content is truncated/missing in metadata
            client
                .get(&file.raw_url)
                .send()
                .await?
                .error_for_status()?
                .text()
                .await?
        };

        EnvironmentState::parse_lockfile(&content)
            .context("Downloaded Gist contains an invalid omg.lock")?;
        crate::core::safe_ops::atomic_write_file_sync("omg.lock", content)
            .context("Failed to write omg.lock from Gist")?;
        execute_cmd(Cmd::batch([
            Cmd::success("omg.lock updated from Gist"),
            Cmd::info("Running environment check..."),
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

#[cfg(test)]
mod tests {
    use super::{parse_gist_id, sanitize_remote_error_body};

    #[test]
    fn gist_ids_are_extracted_and_strictly_validated() {
        assert_eq!(
            parse_gist_id("https://gist.github.com/alice/0123abcdef").unwrap(),
            "0123abcdef"
        );
        assert_eq!(parse_gist_id("0123abcdef").unwrap(), "0123abcdef");

        for invalid in [
            "https://gist.github.com/",
            "https://gist.github.com/alice/0123abcdef?file=omg.lock",
            "https://example.com/alice/0123abcdef",
            "not-a-gist-id",
            "123",
        ] {
            assert!(parse_gist_id(invalid).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn remote_error_body_is_terminal_safe_and_bounded() {
        let body = format!("\u{1b}]52;c;secret\u{7}{}", "x".repeat(400));
        let safe = sanitize_remote_error_body(&body);
        assert!(!safe.contains('\u{1b}'));
        assert!(!safe.contains('\u{7}'));
        assert_eq!(safe.chars().count(), 200);
    }
}
