use color_eyre::Result;
use color_eyre::eyre::{Context, eyre};
use std::fs::ReadDir;
use std::path::{Path, PathBuf};

use crate::data::FileTransaction;
use crate::importer::qfx;

pub enum TransactionReader {
    QfxReader(qfx::QfxReader),
}

impl<'a> TransactionReader {
    pub fn transactions(&'a self) -> Result<impl Iterator<Item = Result<FileTransaction<'a>>>> {
        match self {
            Self::QfxReader(r) => r.read(),
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
                let reader = qfx::QfxReader::open(&file_path).wrap_err_with(|| {
                    format!("Failed to read file: {}", file_path.to_string_lossy())
                })?;

                Ok(Some((file_path, TransactionReader::QfxReader(reader))))
            }
            "csv" => {
                println!("CSV exit early");
                Ok(None)
            }
            ext => Err(eyre!("Unrecognized file type: {}", ext)),
        }
    }
}
