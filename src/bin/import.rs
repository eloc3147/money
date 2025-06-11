use std::collections::HashMap;

use color_eyre::{
    Result,
    eyre::{Context, eyre},
    owo_colors::OwoColorize,
};
use console::{Emoji, style};
use indicatif::MultiProgress;
use money::importer::categorizer::Categorizer;
use money::importer::config::AppConfig;
use money::importer::loader::Loader;

fn main() -> Result<()> {
    color_eyre::install()?;

    println!("{}", style("Money Importer").white());

    let data_dir = dirs::data_dir()
        .ok_or_else(|| eyre!("OS user data directory missing"))?
        .join("money_app");
    let config_path = data_dir.join("config.toml");

    println!(
        "{} {}Loading config ({})...",
        style("[1/4]").bold().dim(),
        Emoji("📄 ", ""),
        config_path.to_string_lossy()
    );

    let config = AppConfig::load(&config_path).wrap_err("Failed to load config")?;

    println!(
        "{} {}Building rules...",
        style("[2/4]").bold().dim(),
        Emoji("⚙️ ", "")
    );
    let categorizer =
        Categorizer::build(config.rule, true).wrap_err("Failed to load transaction rules")?;

    println!(
        "{} {}Loading transaction files...",
        style("[3/4]").bold().dim(),
        Emoji("🏦 ", ""),
    );
    let load_progress = MultiProgress::new();

    let loader = Loader::new(config.account, categorizer);

    let mut uncategorized = HashMap::new();
    loader
        .load_accounts(&load_progress, &mut uncategorized)
        .wrap_err("Failed to load accounts")?;

    drop(load_progress);

    println!("Most common uncategorized transactions:");
    if uncategorized.len() > 0 {
        let mut items = Vec::from_iter(uncategorized);
        items.sort_by(|a, b| a.1.cmp(&b.1).reverse());

        for (name, count) in items.iter().take(20) {
            println!(
                "{:50}: {}",
                style(name).bright_yellow().bold(),
                style(count).bright_cyan()
            );
        }
    }

    Ok(())
}
