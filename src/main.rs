#[deny(clippy::all, clippy::pedantic)]
mod config;
mod db;
mod importer;

use std::path::PathBuf;

use clap::Parser;
use color_eyre::Result;
use color_eyre::eyre::{Context, eyre};
use config::AppConfig;
use console::{Emoji, style};
use importer::categorizer::Categorizer;

use crate::config::BudgetConfig;
use crate::db::Db;

async fn load_config(config_path: PathBuf) -> Result<AppConfig> {
    tokio::task::spawn_blocking(move || AppConfig::load(&config_path))
        .await?
        .wrap_err("Failed to load config")
}

async fn build_budgets(db: &Db, configs: &[BudgetConfig]) -> Result<()> {
    let mut iter = configs.iter();
    let Some(mut previous) = iter.next() else {
        return Ok(());
    };

    let mut handle = db.open_handle().await?;
    while let Some(config) = iter.next() {
        for rule in &previous.rules {
            handle
                .add_budget(
                    &previous.name,
                    previous.start_date,
                    Some(config.start_date),
                    &rule.category,
                    rule.limit,
                )
                .await?;
        }

        previous = config;
    }

    for rule in &previous.rules {
        handle
            .add_budget(
                &previous.name,
                previous.start_date,
                None,
                &rule.category,
                rule.limit,
            )
            .await?;
    }

    Ok(())
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
/// A simple expense tracking program
struct Args {
    /// Clear the database and re-import all transactions
    #[arg(long)]
    clean: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    println!(
        "{}",
        style(concat!("Money v", env!("CARGO_PKG_VERSION"))).white()
    );

    let args = Args::parse();

    let data_dir = dirs::data_dir()
        .ok_or_else(|| eyre!("OS user data directory missing"))?
        .join("money_app");

    println!("Data directory: {}\n", data_dir.to_string_lossy());

    let config_path = data_dir.join("config.toml");
    println!(
        "[{}] {}Loading config...",
        style("1/5").bold().white(),
        Emoji("📄 ", "")
    );
    let config = load_config(config_path)
        .await
        .map(|c| Box::leak(Box::new(c)))?;

    println!(
        "[{}] {}Building rules...",
        style("2/5").bold().white(),
        Emoji("⚙️ ", "")
    );
    let categorizer = Categorizer::build(&config.transaction_type, &config.rule)
        .wrap_err("Failed to load transaction rules")?;

    println!(
        "[{}] {}Updating Budgets...",
        style("3/5").bold().white(),
        Emoji("📊 ", "")
    );
    let db_pool = db::build(&config.database, args.clean)
        .await
        .wrap_err("Failed to setup DB")?;

    build_budgets(&db_pool, &config.budget).await?;

    println!(
        "[{}] {}Loading transaction files...",
        style("4/5").bold().white(),
        Emoji("🏦 ", ""),
    );

    importer::import_files(&db_pool, &categorizer, &config.account).await?;

    println!(
        "[{}] {}Import complete",
        style("5/5").bold().white(),
        Emoji("✅ ", ""),
    );

    Ok(())
}
