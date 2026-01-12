#[deny(clippy::all, clippy::pedantic)]
mod config;
mod db;
mod importer;

use std::path::PathBuf;

use color_eyre::Result;
use color_eyre::eyre::{Context, eyre};
use config::AppConfig;
use console::{Emoji, style};
use importer::categorizer::Categorizer;

async fn load_config(config_path: PathBuf) -> Result<AppConfig> {
    tokio::task::spawn_blocking(move || AppConfig::load(&config_path))
        .await?
        .wrap_err("Failed to load config")
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    println!(
        "{}",
        style(concat!("Money v", env!("CARGO_PKG_VERSION"))).white()
    );

    let data_dir = dirs::data_dir()
        .ok_or_else(|| eyre!("OS user data directory missing"))?
        .join("money_app");

    println!("Data directory: {}\n", data_dir.to_string_lossy());

    let config_path = data_dir.join("config.toml");
    println!(
        "[{}] {}Loading config...",
        style("1/4").bold().white(),
        Emoji("📄 ", "")
    );
    let config = load_config(config_path)
        .await
        .map(|c| Box::leak(Box::new(c)))?;

    println!(
        "[{}] {}Building rules...",
        style("2/4").bold().white(),
        Emoji("⚙️ ", "")
    );
    let categorizer = Categorizer::build(&config.transaction_type, &config.rule)
        .wrap_err("Failed to load transaction rules")?;

    println!(
        "[{}] {}Loading transaction files...",
        style("3/4").bold().white(),
        Emoji("🏦 ", ""),
    );
    let db_pool = db::build(&config.database)
        .await
        .wrap_err("Failed to setup DB")?;

    importer::import_files(&db_pool, &categorizer, &config.account).await?;

    println!(
        "[{}] {}Import complete",
        style("4/4").bold().white(),
        Emoji("✅ ", ""),
    );

    Ok(())
}
