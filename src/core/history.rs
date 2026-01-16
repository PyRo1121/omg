use anyhow::{Context, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Install,
    Remove,
    Update,
    Sync,
}

impl std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Install => write!(f, "Install"),
            Self::Remove => write!(f, "Remove"),
            Self::Update => write!(f, "Update"),
            Self::Sync => write!(f, "Sync"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageChange {
    pub name: String,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    pub source: String, // "official" or "aur"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Transaction {
    pub id: String,
    pub timestamp: Timestamp,
    pub transaction_type: TransactionType,
    pub changes: Vec<PackageChange>,
    pub success: bool,
}

pub struct HistoryManager {
    log_path: PathBuf,
}

impl HistoryManager {
    pub fn new() -> Result<Self> {
        let log_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("omg");

        if !log_dir.exists() {
            fs::create_dir_all(&log_dir)?;
        }

        Ok(Self {
            log_path: log_dir.join("history.json"),
        })
    }

    pub fn load(&self) -> Result<Vec<Transaction>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.log_path).context("Failed to read history file")?;

        let history: Vec<Transaction> = serde_json::from_str(&content).unwrap_or_default(); // Gracefully handle corruption or empty file

        Ok(history)
    }

    pub fn save(&self, history: &[Transaction]) -> Result<()> {
        let content =
            serde_json::to_string_pretty(history).context("Failed to serialize history")?;

        fs::write(&self.log_path, content).context("Failed to write history file")?;

        Ok(())
    }

    pub fn add_transaction(
        &self,
        transaction_type: TransactionType,
        changes: Vec<PackageChange>,
        success: bool,
    ) -> Result<()> {
        let mut history = self.load()?;

        let transaction = Transaction {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Timestamp::now(),
            transaction_type,
            changes,
            success,
        };

        history.push(transaction);

        // Keep only last 1000 transactions to prevent file bloat
        if history.len() > 1000 {
            history.drain(0..history.len() - 1000);
        }

        self.save(&history)
    }
}
