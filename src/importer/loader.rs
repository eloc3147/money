use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use std::path::Path;

use super::{config::AccountConfig, qfx};

pub fn load_accounts(accounts: &[AccountConfig]) -> Result<()> {
    for account in accounts.iter() {
        println!("TMP: Loading account {}", account.name);
        load_dir(&account.source_path)
            .wrap_err_with(|| format!("Failed to load account: {:}", account.name))?;
    }

    Ok(())
}

fn load_dir(path: &Path) -> Result<()> {
    for entry in path.read_dir()? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry.file_type()?.is_dir() {
            load_dir(&entry_path)
                .wrap_err_with(|| format!("Error loading directory: {:?}", &entry_path))?;
        } else {
            load_file(&entry_path)
                .wrap_err_with(|| format!("Error loading file: {:?}", &entry_path))?;

            return Ok(());
        }
    }

    Ok(())
}

fn load_file(path: &Path) -> Result<()> {
    let ext = path
        .extension()
        .ok_or_else(|| eyre!("File missing extension: {:?}", path))?
        .to_ascii_lowercase();

    match &*ext.to_string_lossy() {
        "qfx" => qfx::load_file(path),
        ext => Err(eyre!("Unrecognized file type: {}", ext)),
    }
}
