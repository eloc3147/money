// use money::importer::app::ImporterApp;

use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use console::{Emoji, style};
use indicatif::MultiProgress;
use money::importer::config::AppConfig;
use money::importer::loader;

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
        "{} {}Loading transaction files...",
        style("[2/4]").bold().dim(),
        Emoji("🏦 ", ""),
    );
    let load_progress = MultiProgress::new();
    loader::load_accounts(&config.accounts, &load_progress).wrap_err("Failed to load accounts")?;

    Ok(())
}
