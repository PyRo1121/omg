use crate::core::history::Transaction;
use crate::daemon::protocol::StatusResult;
use anyhow::Result;

pub struct App {
    pub status: Option<StatusResult>,
    pub history: Vec<Transaction>,
    pub last_tick: std::time::Instant,
}

impl App {
    pub async fn new() -> Result<Self> {
        let mut app = Self {
            status: None,
            history: Vec::new(),
            last_tick: std::time::Instant::now(),
        };
        app.refresh().await?;
        Ok(app)
    }

    pub async fn refresh(&mut self) -> Result<()> {
        // 1. Fetch status from daemon
        if let Ok(mut client) = crate::core::client::DaemonClient::connect().await
            && let Ok(crate::daemon::protocol::ResponseResult::Status(status)) = client
                .call(crate::daemon::protocol::Request::Status { id: 0 })
                .await
        {
            self.status = Some(status);
        }

        // 2. Fetch history
        if let Ok(history_mgr) = crate::core::history::HistoryManager::new()
            && let Ok(entries) = history_mgr.load()
        {
            self.history = entries.into_iter().rev().take(10).collect();
        }

        Ok(())
    }

    pub async fn tick(&mut self) -> Result<()> {
        if self.last_tick.elapsed() >= std::time::Duration::from_secs(10) {
            self.refresh().await?;
            self.last_tick = std::time::Instant::now();
        }
        Ok(())
    }

    pub const fn handle_key(_key: crossterm::event::KeyCode) {
        // Handle navigation etc.
    }
}
