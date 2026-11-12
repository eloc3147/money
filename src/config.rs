//! Application configuration, stored as TOML.
//!
//! The config describes which accounts to import, how raw bank transactions are mapped to
//! [`UserTransactionType`]s, which rules assign categories, and any budgets. It can be loaded,
//! edited from the GUI and saved back to disk.

use std::fmt::Debug;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use color_eyre::Result;
use color_eyre::eyre::Context;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use strum::{EnumString, IntoStaticStr, VariantArray};

use crate::importer::TransactionType;

/// A user facing classification of a transaction, decoded from the raw bank data.
#[derive(
    Debug,
    Deserialize,
    Serialize,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    VariantArray,
    IntoStaticStr,
    EnumString,
)]
#[allow(missing_docs)]
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
    /// The name used for this type in the config file and database.
    #[must_use]
    pub fn name(self) -> &'static str {
        self.into()
    }

    /// Parse a name produced by [`Self::name`].
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        name.parse().ok()
    }
}

/// Which raw transaction field holds the name used to match rules.
#[derive(
    Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, VariantArray, IntoStaticStr,
)]
pub enum NameSource {
    /// Use the transaction memo.
    Memo,
    /// Use the full transaction name.
    Name,
    /// Use the transaction name with the matched prefix removed.
    NameSuffix,
}

/// How a [`TransactionTypeConfig`] selects the transactions it applies to.
#[derive(
    Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, Default, VariantArray, IntoStaticStr,
)]
pub enum TransactionTypeMode {
    /// Match on a prefix of the transaction name.
    #[default]
    Prefix,
    /// Match on the transaction type reported by the source file.
    SourceType,
}

/// A bank account whose statement files should be imported.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct AccountConfig {
    /// Display name, referenced from [`TransactionTypeConfig::accounts`].
    pub name: String,
    /// Directory that is searched recursively for `.qfx` and `.csv` files.
    pub source_path: PathBuf,
}

/// Whether transactions of a type count as income.
#[derive(
    Debug,
    Deserialize,
    Serialize,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    VariantArray,
    IntoStaticStr,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum IncomeType {
    /// Always income.
    Yes,
    /// Never income (an expense).
    #[default]
    No,
    /// Income when the amount is positive, expense otherwise.
    Auto,
}

/// Maps raw transactions of an account onto a [`UserTransactionType`].
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct TransactionTypeConfig {
    /// How transactions are matched.
    #[serde(default)]
    pub mode: TransactionTypeMode,
    /// Name prefix to match in [`TransactionTypeMode::Prefix`] mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Source type to match in [`TransactionTypeMode::SourceType`] mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<TransactionType>,
    /// The type assigned to matching transactions.
    pub transaction_type: UserTransactionType,
    /// Whether matching transactions are income.
    #[serde(default)]
    pub income: IncomeType,
    /// Where the rule-matching name comes from.
    pub name_source: NameSource,
    /// Names of the accounts this mapping applies to.
    pub accounts: Vec<String>,
}

/// Assigns a category to transactions of one type whose name matches a pattern exactly.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct TransactionRuleConfig {
    /// The type of transaction this rule applies to.
    pub transaction_type: UserTransactionType,
    /// Dot separated category, e.g. `Food.Groceries`. The first segment is the base category.
    pub category: String,
    /// Drop matching transactions entirely (e.g. transfers between own accounts).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ignore: bool,
    /// Exact (trimmed) names that this rule matches.
    pub patterns: Vec<String>,
}

/// Storage settings.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
pub struct DatabaseConfig {
    /// Path of the SQLite database. Defaults to `money.sqlite` in the data directory.
    ///
    /// Older configs containing Postgres connection settings are still accepted; the extra
    /// keys are ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

/// A spending limit for one category within a budget.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct BudgetRule {
    /// The category being limited.
    pub category: String,
    /// The limit per month.
    pub limit: Decimal,
}

/// A set of budget limits which apply from `start_date` until the next budget starts.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct BudgetConfig {
    /// Budget name.
    pub name: String,
    /// First day this budget applies.
    pub start_date: NaiveDate,
    /// Limits per category.
    pub rules: Vec<BudgetRule>,
}

/// The complete application configuration.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
pub struct AppConfig {
    /// Storage settings.
    #[serde(default)]
    pub database: DatabaseConfig,
    /// Accounts to import.
    #[serde(default)]
    pub account: Vec<AccountConfig>,
    /// Transaction type mappings.
    #[serde(default)]
    pub transaction_type: Vec<TransactionTypeConfig>,
    /// Categorization rules.
    #[serde(default)]
    pub rule: Vec<TransactionRuleConfig>,
    /// Budgets, in chronological order.
    #[serde(default)]
    pub budget: Vec<BudgetConfig>,
}

impl AppConfig {
    /// Load and parse a config file.
    ///
    /// # Errors
    /// Fails if the file cannot be read or is not a valid config.
    pub fn load(path: &Path) -> Result<Self> {
        let mut config_text = String::new();

        File::open(path)
            .and_then(|mut f| f.read_to_string(&mut config_text))
            .wrap_err_with(|| format!("Cannot read config file at {}", path.display()))?;

        Self::parse(&config_text)
    }

    /// Parse config text.
    ///
    /// # Errors
    /// Fails if the text is not a valid config.
    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str(text).wrap_err("Malformed config file")
    }

    /// Serialize to TOML text.
    ///
    /// # Errors
    /// Fails if the config cannot be represented as TOML.
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).wrap_err("Failed to serialize config")
    }

    /// Write the config to `path`, first copying any existing file to `<path>.bak`.
    ///
    /// # Errors
    /// Fails if the backup or the new file cannot be written.
    pub fn save(&self, path: &Path) -> Result<()> {
        let text = self.to_toml()?;
        if path.exists() {
            let mut backup = path.as_os_str().to_owned();
            backup.push(".bak");
            std::fs::copy(path, &backup).wrap_err("Failed to back up existing config")?;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).wrap_err("Failed to create config directory")?;
        }
        std::fs::write(path, text)
            .wrap_err_with(|| format!("Failed to write config file at {}", path.display()))
    }

    /// Check the config for problems that would prevent an import.
    ///
    /// Returns a list of human readable problems; an empty list means the config is valid.
    #[must_use]
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for (idx, account) in self.account.iter().enumerate() {
            if account.name.trim().is_empty() {
                problems.push(format!("Account #{} has no name", idx + 1));
            }
        }
        for (idx, tt) in self.transaction_type.iter().enumerate() {
            match tt.mode {
                TransactionTypeMode::Prefix if tt.prefix.as_deref().unwrap_or("").is_empty() => {
                    problems.push(format!(
                        "Transaction type #{} ({}) uses Prefix mode without a prefix",
                        idx + 1,
                        tt.transaction_type.name()
                    ));
                }
                TransactionTypeMode::SourceType if tt.source_type.is_none() => {
                    problems.push(format!(
                        "Transaction type #{} ({}) uses SourceType mode without a source type",
                        idx + 1,
                        tt.transaction_type.name()
                    ));
                }
                _ => {}
            }
            for account in &tt.accounts {
                if !self.account.iter().any(|a| &a.name == account) {
                    problems.push(format!(
                        "Transaction type #{} references unknown account \"{account}\"",
                        idx + 1
                    ));
                }
            }
        }
        for (idx, rule) in self.rule.iter().enumerate() {
            if rule.category.trim().is_empty() {
                problems.push(format!("Rule #{} has no category", idx + 1));
            }
        }
        problems
    }

    /// All distinct categories used by rules, sorted.
    #[must_use]
    pub fn categories(&self) -> Vec<String> {
        let mut categories: Vec<String> = self.rule.iter().map(|r| r.category.clone()).collect();
        categories.sort();
        categories.dedup();
        categories
    }

    /// Add `pattern` to the rule for (`transaction_type`, `category`, `ignore`), creating the
    /// rule if none exists.
    ///
    /// Any existing rule of the same transaction type that already matches `pattern` has the
    /// pattern removed first, so the result never contains duplicate patterns (which would be
    /// rejected by the categorizer).
    pub fn add_rule_pattern(
        &mut self,
        transaction_type: UserTransactionType,
        category: &str,
        ignore: bool,
        pattern: &str,
    ) {
        let pattern = pattern.trim();
        for rule in &mut self.rule {
            if rule.transaction_type == transaction_type {
                rule.patterns.retain(|p| p != pattern);
            }
        }
        self.rule.retain(|r| !r.patterns.is_empty());

        if let Some(rule) = self.rule.iter_mut().find(|r| {
            r.transaction_type == transaction_type && r.category == category && r.ignore == ignore
        }) {
            rule.patterns.push(pattern.to_string());
        } else {
            self.rule.push(TransactionRuleConfig {
                transaction_type,
                category: category.to_string(),
                ignore,
                patterns: vec![pattern.to_string()],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[database]
host = "localhost"
port = 5432
username = "u"
password = "p"

[[account]]
name = "Chequing"
source_path = "/tmp/chequing"

[[transaction_type]]
prefix = "POS "
transaction_type = "DebitPurchase"
name_source = "NameSuffix"
accounts = ["Chequing"]

[[transaction_type]]
mode = "SourceType"
source_type = "Credit"
transaction_type = "CreditPurchase"
income = "auto"
name_source = "Name"
accounts = ["Chequing"]

[[rule]]
transaction_type = "DebitPurchase"
category = "Food.Groceries"
patterns = ["GROCER"]

[[budget]]
name = "2024"
start_date = "2024-01-01"
rules = [{ category = "Food", limit = "500.00" }]
"#;

    #[test]
    fn parses_legacy_config() {
        let config = AppConfig::parse(SAMPLE).unwrap();
        assert_eq!(config.database.path, None);
        assert_eq!(config.account.len(), 1);
        assert_eq!(config.transaction_type[0].mode, TransactionTypeMode::Prefix);
        assert_eq!(config.transaction_type[1].income, IncomeType::Auto);
        assert_eq!(config.rule[0].category, "Food.Groceries");
        assert!(!config.rule[0].ignore);
        assert_eq!(config.budget[0].rules[0].limit, Decimal::new(50000, 2));
        assert!(config.validate().is_empty());
    }

    #[test]
    fn round_trips_through_toml() {
        let config = AppConfig::parse(SAMPLE).unwrap();
        let text = config.to_toml().unwrap();
        let reparsed = AppConfig::parse(&text).unwrap();
        assert_eq!(config, reparsed);
    }

    #[test]
    fn empty_config_is_valid() {
        let config = AppConfig::parse("").unwrap();
        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn malformed_config_errors() {
        assert!(AppConfig::parse("account = 3").is_err());
    }

    #[test]
    fn save_writes_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, SAMPLE).unwrap();

        let mut config = AppConfig::load(&path).unwrap();
        config.account[0].name = "Renamed".into();
        config.save(&path).unwrap();

        let backup = std::fs::read_to_string(dir.path().join("config.toml.bak")).unwrap();
        assert_eq!(backup, SAMPLE);
        assert_eq!(AppConfig::load(&path).unwrap().account[0].name, "Renamed");
    }

    #[test]
    fn load_missing_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        assert!(AppConfig::load(&dir.path().join("nope.toml")).is_err());
    }

    #[test]
    fn validate_reports_problems() {
        let mut config = AppConfig::parse(SAMPLE).unwrap();
        config.account.push(AccountConfig {
            name: " ".into(),
            source_path: PathBuf::new(),
        });
        config.transaction_type[0].prefix = None;
        config.transaction_type[1].source_type = None;
        config.transaction_type[1].accounts.push("Missing".into());
        config.rule[0].category.clear();
        let problems = config.validate();
        assert_eq!(problems.len(), 5, "{problems:?}");
    }

    #[test]
    fn add_rule_pattern_merges_and_moves() {
        let mut config = AppConfig::parse(SAMPLE).unwrap();
        config.add_rule_pattern(
            UserTransactionType::DebitPurchase,
            "Food.Groceries",
            false,
            "MART ",
        );
        assert_eq!(config.rule.len(), 1);
        assert_eq!(config.rule[0].patterns, vec!["GROCER", "MART"]);

        // Moving a pattern to a new category removes it from the old rule.
        config.add_rule_pattern(
            UserTransactionType::DebitPurchase,
            "Shopping",
            false,
            "MART",
        );
        assert_eq!(config.rule.len(), 2);
        assert_eq!(config.rule[0].patterns, vec!["GROCER"]);
        assert_eq!(config.rule[1].category, "Shopping");

        // Moving the last pattern out of a rule removes the rule.
        config.add_rule_pattern(
            UserTransactionType::DebitPurchase,
            "Shopping",
            false,
            "GROCER",
        );
        assert_eq!(config.rule.len(), 1);
        assert_eq!(config.categories(), vec!["Shopping"]);
    }

    #[test]
    fn transaction_type_names_round_trip() {
        for &t in UserTransactionType::VARIANTS {
            assert_eq!(UserTransactionType::from_name(t.name()), Some(t));
        }
        assert_eq!(UserTransactionType::from_name("nope"), None);
    }
}
