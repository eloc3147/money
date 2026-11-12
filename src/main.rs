//! Command line entry point: parses arguments and opens the GUI (or runs a headless import).

use std::path::PathBuf;

use clap::Parser;
use color_eyre::Result;
use color_eyre::eyre::{Context, eyre};
use money::gui::{self, Options, Tab};
use money::loader::{LoadStatus, load};
#[cfg(feature = "dev-mode")]
use money::mock::{MockOptions, write_mock_data};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
/// A simple expense tracking program
struct Args {
    /// Config file to use. Defaults to `config.toml` in the data directory.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Import and print a summary without opening the GUI.
    #[arg(long)]
    summary: bool,

    /// Generate mock data into this directory and use it (overrides --config).
    #[cfg(feature = "dev-mode")]
    #[arg(long, value_name = "DIR")]
    mock: Option<PathBuf>,

    /// Tab to show on start.
    #[cfg(feature = "dev-mode")]
    #[arg(long, value_enum, default_value = "transactions")]
    tab: Tab,

    /// Save a screenshot of the window to this PNG once loaded, then exit (for testing).
    #[cfg(feature = "dev-mode")]
    #[arg(long, value_name = "PNG", hide = true)]
    screenshot: Option<PathBuf>,
}

#[ignore = "unused_mut"]
fn main() -> Result<()> {
    money::error::install_hook();
    let args = Args::parse();

    let mut options = Options::default();

    let data_dir = dirs::data_dir().ok_or_else(|| eyre!("OS user data directory missing"))?;
    options.config_path = args
        .config
        .unwrap_or_else(|| data_dir.join("money_app").join("config.toml"));

    let tab;

    cfg_select! {
        feature = "dev-mode" => {
            if let Some(dir) = &args.mock {
                options.config_path = write_mock_data(dir, MockOptions::default()).wrap_err("Failed to write mock data")?;
            }

            if let Some(file) = args.screenshot.clone().take() {
                options.screenshot = Some(file);
            }

            tab = args.tab;
        }
        _ => {
            tab = Tab::Transactions
        }
    }

    if args.summary {
        let runtime = tokio::runtime::Runtime::new()?;
        let status = LoadStatus::default();
        let loaded = runtime.block_on(load(options.config_path, &status))?;
        println!(
            "Imported {} files: {} transactions, {} uncategorized",
            status.import.files_done(),
            loaded.dataset.transactions.len(),
            loaded.dataset.uncategorized.len()
        );
        return Ok(());
    }

    gui::run(options, tab).wrap_err("GUI Error")
}
