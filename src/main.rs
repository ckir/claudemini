use claudemini::cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Cli::run().await?;
    Ok(())
}
