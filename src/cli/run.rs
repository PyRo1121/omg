use crate::cli::{CliContext, LocalCommandRunner};
use crate::core::task_runner;
use anyhow::Result;

pub struct RunCommand {
    pub task: String,
    pub args: Vec<String>,
    pub watch: bool,
    pub parallel: bool,
    pub using: Option<String>,
    pub all: bool,
}

impl LocalCommandRunner for RunCommand {
    async fn execute(&self, _ctx: &CliContext) -> Result<()> {
        if self.watch {
            task_runner::run_task_watch(&self.task, &self.args)?;
        } else if self.parallel {
            task_runner::run_tasks_parallel(&self.task, &self.args).await?;
        } else {
            task_runner::run_task_advanced(
                &self.task,
                &self.args,
                self.using.as_deref(),
                self.all,
            )?;
        }
        Ok(())
    }
}
