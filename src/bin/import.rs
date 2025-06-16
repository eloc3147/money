use color_eyre::{
    Result,
    eyre::{Context, eyre},
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
    let categorizer = Categorizer::build(config.transaction_type, config.rule, true)
        .wrap_err("Failed to load transaction rules")?;

    println!(
        "{} {}Loading transaction files...",
        style("[3/4]").bold().dim(),
        Emoji("🏦 ", ""),
    );
    let load_progress = MultiProgress::new();

    let mut loader = Loader::new(categorizer);
    loader
        .load(config.account, &load_progress)
        .wrap_err("Failed to load accounts")?;

    load_progress.clear()?;

    println!(
        "{} {}Import complete",
        style("[4/4]").bold().dim(),
        Emoji("✅ ", ""),
    );

    let (missing_prefix, missing_rule) = loader.get_missing_stats();

    if missing_prefix.len() > 0 {
        let mut items = Vec::from_iter(missing_prefix);
        items.sort_by(|a, b| a.1.cmp(&b.1).reverse());
        let count: usize = items.iter().map(|(_, c)| *c).sum();

        println!(
            "\n{} transactions missing prefixes",
            style(count).bright().yellow()
        );

        println!("Most frequent transactions missing prefixes:");
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
        items.sort_by(|a, b| a.1.cmp(&b.1).reverse());
        let count: usize = items.iter().map(|(_, c)| *c).sum();

        println!(
            "\n{} transactions missing category rules",
            style(count).bright().yellow()
        );

        println!("Most frequent transactions missing category rules:");
        for (info, count) in items.iter().take(20) {
            println!(
                "{:28} | {:40}: {}",
                style(&format!("{:?}", info.transaction_type))
                    .bright()
                    .white()
                    .bold(),
                style(&info.display).bright().white().bold(),
                style(count).bright().cyan()
            );
        }
        if items.len() > 20 {
            println!("{}", style("...").bright().white());
        }
    }

    Ok(())
}
