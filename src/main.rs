use std::time::Duration;

use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use console::{Emoji, style};
use indicatif::MultiProgress;
use money::importer::categorizer::Categorizer;
use money::importer::config::AppConfig;
use money::importer::loader::Loader;
use serde::Serialize;
use tokio::runtime::Builder;
use warp::Filter;
use warp::http::StatusCode;

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

#[derive(Debug, Serialize)]
struct TestData {
    keys: &'static [&'static str],
    rows: &'static [&'static [u32]],
}

fn build_data() -> TestData {
    TestData {
        keys: &["A", "B", "C"],
        rows: &[&[1, 2, 3], &[3, 3, 2], &[2, 3, 0]],
    }
}

async fn run_server() {
    let test_data = warp::path("test_data")
        .and(warp::path::end())
        .and(warp::get())
        .map(|| {
            let data = build_data();
            warp::reply::json(&data)
        });
    let api = warp::path("api").and(test_data);

    let assets = warp::path("assets").and(warp::fs::dir("web/assets"));
    let home = warp::path::end().and(warp::fs::file("web/index.html"));
    let missing = warp::any()
        .map(warp::reply)
        .map(|r| warp::reply::with_status(r, StatusCode::NOT_FOUND));

    let routes = home.or(assets).or(api).or(missing);

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
