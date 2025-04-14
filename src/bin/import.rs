// use money::importer::app::ImporterApp;

use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use console::{Emoji, style};
use money::importer::config::AppConfig;
use money::importer::loader;

fn main() -> Result<()> {
    color_eyre::install()?;

    println!("{}", style("Money Importer").white());

    println!(
        "{} {}Loading config...",
        style("[1/4]").bold().dim(),
        Emoji("📄 ", ""),
    );

    let data_dir = dirs::data_dir()
        .ok_or_else(|| eyre!("OS user data directory missing"))?
        .join("money_app");

    let config =
        AppConfig::load(&data_dir.join("config.toml")).wrap_err("Failed to load config")?;
    println!("\n{:#?}", config);

    loader::load_accounts(&config.accounts).wrap_err("Failed to load accounts")?;

    // let mut terminal = ratatui::init();
    // let result = ImporterApp::new()?.run(&mut terminal);
    // ratatui::restore();

    Ok(())
}
