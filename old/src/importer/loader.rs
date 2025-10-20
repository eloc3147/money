use std::fs::ReadDir;
use std::path::{Path, PathBuf};

use color_eyre::Result;
use color_eyre::eyre::{Context, eyre};

use crate::importer::Transaction;
use crate::importer::csv_file::{CsvReader, CsvTransactionIter};
use crate::importer::qfx_file::{QfxReader, QfxTransactionIter};

pub enum TransactionReader {
    QfxReader(QfxReader),
    CsvReader(CsvReader),
}

impl<'a> TransactionReader {
    pub fn transactions(&'a mut self) -> Result<TransactionIter<'a>> {
        match self {
            Self::QfxReader(r) => Ok(TransactionIter::QfxIter(
                r.read().wrap_err("Failed to read from QfxReader")?,
            )),
            Self::CsvReader(r) => Ok(TransactionIter::CsvIter(
                r.read().wrap_err("Failed to read from CsvReader")?,
            )),
        }
    }
}

pub enum TransactionIter<'a> {
    QfxIter(QfxTransactionIter<'a>),
    CsvIter(CsvTransactionIter<'a>),
}

impl<'a> Iterator for TransactionIter<'a> {
    type Item = Result<Transaction<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::QfxIter(i) => i.next(),
            Self::CsvIter(i) => i.next(),
        }
    }
}

pub struct Loader {
    search_stack: Vec<ReadDir>,
}

impl Loader {
    pub fn new() -> Self {
        Self {
            search_stack: Vec::new(),
        }
    }

    pub fn add_dir(&mut self, dir: &Path) -> Result<()> {
        self.search_stack.push(dir.read_dir()?);

        Ok(())
    }

    pub fn open_next_file(&mut self) -> Result<Option<(PathBuf, TransactionReader)>> {
        let Some(file_path) = self.next_file()? else {
            return Ok(None);
        };

        let ext = file_path
            .extension()
            .ok_or_else(|| eyre!("File missing extension: {:?}", file_path))?
            .to_ascii_lowercase();

        match &*ext.to_string_lossy() {
            "qfx" => {
                let reader = QfxReader::open(&file_path).wrap_err_with(|| {
                    format!("Failed to read file: {}", file_path.to_string_lossy())
                })?;

                Ok(Some((file_path, TransactionReader::QfxReader(reader))))
            }
            "csv" => {
                let reader = CsvReader::open(&file_path).wrap_err_with(|| {
                    format!("Failed to read file: {}", file_path.to_string_lossy())
                })?;

                Ok(Some((file_path, TransactionReader::CsvReader(reader))))
            }
            ext => Err(eyre!("Unrecognized file type: {}", ext)),
        }
    }

    pub fn clear(&mut self) {
        self.search_stack.clear();
    }

    fn next_file(&mut self) -> Result<Option<PathBuf>> {
        loop {
            let Some(dir_iter) = self.search_stack.last_mut() else {
                return Ok(None);
            };

            match dir_iter.next() {
                Some(Ok(entry)) => {
                    let entry_type = entry.file_type()?;
                    if entry_type.is_file() {
                        return Ok(Some(entry.path()));
                    } else if entry_type.is_dir() {
                        self.add_dir(&entry.path())?;
                        // Continue loop
                    } else if entry_type.is_symlink() {
                        let new_path = std::fs::read_link(entry.path())?;
                        let new_meta = new_path.metadata()?;

                        if new_meta.is_file() {
                            return Ok(Some(new_path));
                        } else if new_meta.is_dir() {
                            self.add_dir(&new_path)?;
                            // Continue loop
                        }
                    }
                }
                Some(Err(e)) => return Err(e.into()),
                None => {
                    let _ = self.search_stack.pop();
                    // Continue loop
                }
            }
        }
    }
}
