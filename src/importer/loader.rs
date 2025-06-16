use chrono::{DateTime, FixedOffset};
use color_eyre::{
    Result,
    eyre::{Context, bail, eyre},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rust_decimal::Decimal;
use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use crate::importer::categorizer::{Categorizer, MissingRuleInfo};

use super::{config::AccountConfig, qfx};

/// Transaction type as defined by the input file
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

pub struct Loader {
    categorizer: Categorizer,
}

impl Loader {
    pub fn new(categorizer: Categorizer) -> Self {
        Self { categorizer }
    }

    pub fn load(&mut self, accounts: Vec<AccountConfig>, progress: &MultiProgress) -> Result<()> {
        let spinner = progress.add(ProgressBar::no_length());
        spinner.set_style(ProgressStyle::default_spinner());
        spinner.tick();

        for account in accounts {
            spinner.set_message(format!("Loading account {}", account.name));
            spinner.tick();
            self.load_dir(
                &account
                    .source_path
                    .parent()
                    .ok_or_else(|| eyre!("Invalid source path: {:?}", &account.source_path))?,
                &account.source_path,
                account.name.as_str(),
                progress,
            )
            .wrap_err_with(|| format!("Failed to load account: {:}", account.name))?;
        }

        progress.remove(&spinner);

        Ok(())
    }

    fn load_dir(
        &mut self,
        base_path: &Path,
        path: &Path,
        account: &str,
        progress: &MultiProgress,
    ) -> Result<()> {
        let spinner = progress.add(ProgressBar::no_length());
        spinner.set_style(ProgressStyle::default_spinner());
        spinner.tick();

        for entry in path.read_dir()? {
            let entry = entry?;
            let entry_path = entry.path();
            if entry.file_type()?.is_dir() {
                self.load_dir(base_path, &entry_path, account, progress)
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
                self.load_file(&entry_path, account)
                    .wrap_err_with(|| format!("Error loading file {:?}", &entry_path))?;
            }
        }

        progress.remove(&spinner);

        Ok(())
    }

    fn load_file(&mut self, path: &Path, account: &str) -> Result<()> {
        let ext = path
            .extension()
            .ok_or_else(|| eyre!("File missing extension: {:?}", path))?
            .to_ascii_lowercase();

        let loader = match &*ext.to_string_lossy() {
            "qfx" => Box::new(qfx::QfxReader::open(path)?),
            "csv" => return Ok(()),
            ext => bail!("Unrecognized file type: {}", ext),
        };

        for transaction in loader.read()? {
            let transaction = transaction?;

            if transaction.amount == Decimal::ZERO && transaction.transaction_id.contains('.') {
                // Weird multiline transaction. Extra lines don't contain much useful information
                continue;
            }

            let category =
                self.categorizer
                    .categorize(account, &transaction.name, transaction.memo)?;
        }

        Ok(())
    }

    pub fn get_missing_stats(
        &self,
    ) -> (
        &HashMap<MissingRuleInfo, usize>,
        &HashMap<MissingRuleInfo, usize>,
    ) {
        self.categorizer.get_missing_stats()
    }
}
