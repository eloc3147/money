use std::fmt::Debug;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use color_eyre::Result;
use color_eyre::eyre::Context;
use serde::Deserialize;

use crate::importer::TransactionType;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UserTransactionType {
    DebitPurchase,
    DebitRefund,
    CreditPurchase,
    CreditRefund,
    VisaDebitPurchase,
    VisaDebitRefund,
    SentEtransfer,
    ReceivedEtransfer,
    CancelledEtransfer,
    InterAccountTransfer,
    SentDirectDeposit,
    ReceivedDirectDeposit,
    AtmWithdrawal,
    AtmDeposit,
    Interest,
    BankFee,
    ChequeDeposit,
}

impl UserTransactionType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::DebitPurchase => "DebitPurchase",
            Self::DebitRefund => "DebitRefund",
            Self::CreditPurchase => "CreditPurchase",
            Self::CreditRefund => "CreditRefund",
            Self::VisaDebitPurchase => "VisaDebitPurchase",
            Self::VisaDebitRefund => "VisaDebitRefund",
            Self::SentEtransfer => "SentEtransfer",
            Self::ReceivedEtransfer => "ReceivedEtransfer",
            Self::CancelledEtransfer => "CancelledEtransfer",
            Self::InterAccountTransfer => "InterAccountTransfer",
            Self::SentDirectDeposit => "SentDirectDeposit",
            Self::ReceivedDirectDeposit => "ReceivedDirectDeposit",
            Self::AtmWithdrawal => "AtmWithdrawal",
            Self::AtmDeposit => "AtmDeposit",
            Self::Interest => "Interest",
            Self::BankFee => "BankFee",
            Self::ChequeDeposit => "ChequeDeposit",
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub enum NameSource {
    Memo,
    Name,
    NameSuffix,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransactionTypeMode {
    #[default]
    Prefix,
    SourceType,
}

#[derive(Debug, Deserialize)]
pub struct AccountConfig {
    pub name: String,
    pub source_path: PathBuf,
}

#[derive(Debug, Deserialize, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum IncomeType {
    Yes,
    #[default]
    No,
    Auto,
}

#[derive(Debug, Deserialize)]
pub struct TransactionTypeConfig {
    #[serde(default)]
    pub mode: TransactionTypeMode,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub source_type: Option<TransactionType>,
    pub transaction_type: UserTransactionType,
    #[serde(default)]
    pub income: IncomeType,
    pub name_source: NameSource,
    pub accounts: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct TransactionRuleConfig {
    pub transaction_type: UserTransactionType,
    pub category: String,
    #[serde(default)]
    pub ignore: bool,
    pub patterns: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub account: Vec<AccountConfig>,
    pub transaction_type: Vec<TransactionTypeConfig>,
    pub rule: Vec<TransactionRuleConfig>,
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let mut config_text = String::new();

        File::open(path)
            .and_then(|mut f| f.read_to_string(&mut config_text))
            .wrap_err_with(|| format!("Cannot read config file at {}", path.display()))?;

        toml::from_str(&config_text).wrap_err("Malformed config file")
    }
}
