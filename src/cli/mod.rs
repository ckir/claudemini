use crate::tui::TuiManager;
use anyhow::Result;

pub struct Cli;

impl Cli {
    pub async fn run() -> Result<()> {
        TuiManager::run().await
    }
}
