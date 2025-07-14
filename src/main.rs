mod importer;
mod server;

use std::time::Duration;

use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use console::{Emoji, style};
use importer::categorizer::Categorizer;
use importer::config::AppConfig;
use importer::loader::Loader;
use indicatif::MultiProgress;
use tokio::runtime::Builder;
use warp;

fn load_data() -> Result<()> {
    color_eyre::install()?;

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
    let categorizer = Categorizer::build(config.transaction_type, config.rule)
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

async fn run_server() {
    let routes = server::build_routes();

    println!(
        "Starting server at {}",
        style("http://127.0.0.1:3030").bold().bright().blue()
    );
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

fn main() -> Result<()> {
    println!("{}", style("Money").white());

    load_data()?;

    let runtime = Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("money-web")
        .enable_all()
        .build()
        .wrap_err("Failed to launch tokio runtime")?;

    runtime.block_on(run_server());
    runtime.shutdown_timeout(Duration::from_secs(3));

    Ok(())
}
