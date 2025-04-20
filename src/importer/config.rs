use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use color_eyre::{Result, eyre::Context};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AccountConfig {
    pub name: String,
    pub source_path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub accounts: Vec<AccountConfig>,
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let mut config_text = String::new();

        File::open(path)
            .and_then(|mut f| f.read_to_string(&mut config_text))
            .wrap_err_with(|| format!("Cannot read config file at {:?}", path))?;

        toml::from_str(&config_text).wrap_err("Malformed config file")
    }
}
