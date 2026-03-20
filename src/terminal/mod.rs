use anyhow::{Context, Result};
use std::process::{Child, Command};

use crate::config::{Config, TerminalBackend};

pub struct Terminal {
    process: Option<Child>,
    backend: TerminalBackend,
}

impl Terminal {
    pub fn new(config: &Config) -> Self {
        Self {
            process: None,
            backend: config.terminal.clone(),
        }
    }

    pub fn spawn(&mut self) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }
        let child = self.build_command().spawn().context("failed to spawn terminal")?;
        self.process = Some(child);
        Ok(())
    }

    pub fn kill(&mut self) {
        if let Some(ref mut child) = self.process {
            let _ = child.kill();
        }
        self.process = None;
    }

    pub fn is_running(&mut self) -> bool {
        match self.process {
            None => false,
            Some(ref mut child) => child.try_wait().map(|s| s.is_none()).unwrap_or(false),
        }
    }

    fn build_command(&self) -> Command {
        match self.backend {
            TerminalBackend::Foot => {
                let mut c = Command::new("foot");
                c.args(["--app-id", "termix", "--", "zellij"]);
                c
            }
            TerminalBackend::Kitty => {
                let mut c = Command::new("kitty");
                c.args(["--class", "termix", "--", "zellij"]);
                c
            }
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.kill();
    }
}
