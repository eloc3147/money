use chrono::{DateTime, FixedOffset, NaiveDateTime};
use color_eyre::{
    Result,
    eyre::{Context, eyre, bail},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rust_decimal::Decimal;
use std::path::Path;

use super::{config::AccountConfig, qfx};

pub fn load_accounts(accounts: &[AccountConfig], progress: &MultiProgress) -> Result<()> {
    let spinner = progress.add(ProgressBar::no_length());
    spinner.set_style(ProgressStyle::default_spinner());
    spinner.tick();

    for account in accounts.iter() {
        spinner.set_message(format!("Loading account {}", account.name));
        spinner.tick();
        load_dir(
            &account
                .source_path
                .parent()
                .ok_or_else(|| eyre!("Invalid source path: {:?}", &account.source_path))?,
            &account.source_path,
            progress,
        )
        .wrap_err_with(|| format!("Failed to load account: {:}", account.name))?;
    }

    progress.remove(&spinner);

    Ok(())
}

fn load_dir(base_path: &Path, path: &Path, progress: &MultiProgress) -> Result<()> {
    let spinner = progress.add(ProgressBar::no_length());
    spinner.set_style(ProgressStyle::default_spinner());
    spinner.tick();

    for entry in path.read_dir()? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry.file_type()?.is_dir() {
            load_dir(base_path, &entry_path, progress)
                .wrap_err_with(|| format!("Error loading directory: {:?}", &entry_path))?;
        } else {
            spinner.set_message(format!(
                "Loading {}",
                entry_path
                    .strip_prefix(base_path)
                    .wrap_err("Invalid search path")?
                    .to_string_lossy()
            ));
            spinner.tick();
            load_file(&entry_path).wrap_err_with(|| format!("Error loading file {:?}", &entry_path))?;
        }
    }

    progress.remove(&spinner);

    Ok(())
}

#[derive(Debug)]
pub enum FileTransactionType {
    Debit,
    Credit,
    Pos,
    Atm,
    Fee,
    Other,
}

#[derive(Debug)]
pub struct FileTransaction<'a> {
    pub transaction_type: FileTransactionType,
    pub date_posted: DateTime<FixedOffset>,
    pub amount: Decimal,
    pub transaction_id: &'a str,
    pub name: &'a str,
    pub memo: Option<&'a str>,
}

pub trait TransactionReader<'a> {
    fn read(&'a self) -> Result<impl Iterator<Item = Result<FileTransaction<'a>>>>;
}

fn load_file(path: &Path) -> Result<()> {
    let ext = path
        .extension()
        .ok_or_else(|| eyre!("File missing extension: {:?}", path))?
        .to_ascii_lowercase();

    let loader = match &*ext.to_string_lossy() {
        "qfx" => {
            Box::new(qfx::QfxReader::open(path)?)
        }
        "csv" => return Ok(()),
        ext => bail!("Unrecognized file type: {}", ext),
    };
    
    for transaction in loader.read()? {
        println!("{:?}", transaction?);
    }

    Ok(())
}
