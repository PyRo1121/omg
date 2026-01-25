use crate::cli::{CliContext, CommandRunner};
use crate::core::{RuntimeBackend, task_runner};
use anyhow::Result;
use async_trait::async_trait;

pub struct RunCommand {
    pub task: String,
    pub args: Vec<String>,
    pub runtime_backend: Option<String>,
    pub watch: bool,
    pub parallel: bool,
    pub using: Option<String>,
    pub all: bool,
}

#[async_trait]
impl CommandRunner for RunCommand {
    async fn execute(&self, _ctx: &CliContext) -> Result<()> {
        let backend = self.runtime_backend
            .as_deref()
            .map(str::parse::<RuntimeBackend>)
            .transpose()
            .map_err(|err| anyhow::anyhow!(err))?;

        if self.watch {
            task_runner::run_task_watch(&self.task, &self.args, backend).await?;
        } else if self.parallel {
            task_runner::run_tasks_parallel(&self.task, &self.args, backend).await?;
        } else {
            task_runner::run_task_advanced(&self.task, &self.args, backend, self.using.as_deref(), self.all)?;
        }
        Ok(())
    }
}
