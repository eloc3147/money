mod db;
mod importer;
mod server;

use std::path::PathBuf;

use color_eyre::Result;
use color_eyre::eyre::{Context, eyre};
use console::{Emoji, style};
use importer::categorizer::Categorizer;
use importer::config::AppConfig;
use indicatif::MultiProgress;

use crate::db::DbConnection;

fn print_uncategorized(categorizer: &Categorizer) {
    let (missing_prefix, missing_rule) = categorizer.get_missing_stats();

    if missing_prefix.len() > 0 {
        let mut items = Vec::from_iter(missing_prefix);
        items.sort_by(|a, b| a.1.cmp(&b.1).reverse());
        let count: usize = items.iter().map(|(_, c)| *c).sum();

        println!(
            "\n{} transactions missing types",
            style(count).bright().yellow()
        );

        println!("Most frequent transactions missing types:");
        for (info, count) in items.iter().take(20) {
            println!(
                "{:22} | {:33}: {}",
                style(&info.account).bright().white().bold(),
                style(&info.name).bright().white().bold(),
                style(count).bright().cyan()
            );
        }
        if items.len() > 20 {
            println!("{}", style("...").bright().white());
        }
    }

    if missing_rule.len() > 0 {
        let mut items = Vec::from_iter(missing_rule);
        items.sort_by(|(a, _), (b, _)| {
            (a.transaction_type, &a.display)
                .cmp(&(b.transaction_type, &b.display))
                .reverse()
        });
        let count: usize = items.iter().map(|(_, c)| *c).sum();

        println!(
            "\n{} transactions missing categories",
            style(count).bright().yellow()
        );

        println!("Most frequent transactions missing categories:");
        for (info, count) in items.iter().take(30) {
            println!(
                "{:22} | {:40}: {}",
                style(format!("{:?}", info.transaction_type))
                    .bright()
                    .white()
                    .bold(),
                style(&info.display).bright().white().bold(),
                style(count).bright().cyan()
            );
        }
        if items.len() > 30 {
            println!("{}", style("...").bright().white());
        }
    }
}

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
        "{} {}Loading config...",
        style("[1/4]").bold().dim(),
        Emoji("📄 ", "")
    );
    let config = load_config(config_path)
        .await
        .map(|c| Box::leak(Box::new(c)))?;

    println!(
        "{} {}Building rules...",
        style("[2/4]").bold().dim(),
        Emoji("⚙️ ", "")
    );
    let mut categorizer = Categorizer::build(&config.transaction_type, &config.rule)
        .wrap_err("Failed to load transaction rules")?;

    println!(
        "{} {}Loading transaction files...",
        style("[3/4]").bold().dim(),
        Emoji("🏦 ", ""),
    );
    let mut load_progress = MultiProgress::new();

    let db_pool = db::build().await.wrap_err("Failed to setup DB")?;
    let mut import_conn = db_pool
        .acquire()
        .await
        .map(|conn| DbConnection { conn })
        .wrap_err("Failed to connect to DB")?;

    importer::import_data(
        &mut categorizer,
        &mut import_conn,
        &config.account,
        &mut load_progress,
    )
    .await
    .wrap_err("Failed to load transactions")?;

    load_progress.clear()?;

    // TMP
    import_conn.dump_transactions().await?;

    println!(
        "{} {}Import complete",
        style("[4/4]").bold().dim(),
        Emoji("✅ ", ""),
    );

    print_uncategorized(&categorizer);

    server::run(db_pool).await?;

    Ok(())
}
