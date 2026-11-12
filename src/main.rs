//! Command line entry point: parses arguments and opens the GUI (or runs a headless import).

use std::path::PathBuf;

use clap::Parser;
use color_eyre::Result;
use color_eyre::eyre::{Context, eyre};
use money::gui::{self, Tab};
use money::loader::{LoadOptions, LoadStatus, load};
use money::mock::{MockOptions, write_mock_data};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
/// A simple expense tracking program
struct Args {
    /// Config file to use. Defaults to `config.toml` in the data directory.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Generate mock data into this directory and use it (overrides --config).
    #[arg(long, value_name = "DIR")]
    mock: Option<PathBuf>,

    /// Import and print a summary without opening the GUI.
    #[arg(long)]
    headless: bool,

    /// Tab to show on start.
    #[arg(long, value_enum, default_value = "transactions")]
    tab: Tab,

    /// Save a screenshot of the window to this PNG once loaded, then exit (for testing).
    #[arg(long, value_name = "PNG", hide = true)]
    screenshot: Option<PathBuf>,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let data_dir = dirs::data_dir()
        .ok_or_else(|| eyre!("OS user data directory missing"))?
        .join("money_app");

    let options = if let Some(dir) = &args.mock {
        let config_path =
            write_mock_data(dir, MockOptions::default()).wrap_err("Failed to write mock data")?;
        LoadOptions {
            config_path,
            default_db_path: Some(dir.join("money.sqlite")),
        }
    } else {
        LoadOptions {
            config_path: args.config.unwrap_or_else(|| data_dir.join("config.toml")),
            default_db_path: Some(data_dir.join("money.sqlite")),
        }
    };

    if args.headless {
        let runtime = tokio::runtime::Runtime::new()?;
        let status = LoadStatus::default();
        let loaded = runtime.block_on(load(&options, &status))?;
        println!(
            "Imported {} files: {} transactions, {} uncategorized",
            status.import.files_done(),
            loaded.dataset.transactions.len(),
            loaded.dataset.uncategorized.len()
        );
        return Ok(());
    }

    gui::run(options, args.screenshot, args.tab).map_err(|e| eyre!("GUI error: {e}"))
}
