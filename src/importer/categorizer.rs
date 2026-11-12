//! Assigns categories to raw transactions using the configured types and rules.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use color_eyre::Result;
use color_eyre::eyre::{OptionExt, bail};
use patricia_tree::GenericPatriciaMap;

use crate::config::{
    IncomeType, NameSource, TransactionRuleConfig, TransactionTypeConfig, TransactionTypeMode,
    UserTransactionType,
};
use crate::importer::TransactionType;

/// Decodes the transactions matched by one [`TransactionTypeConfig`].
#[derive(Debug, Clone)]
struct TransactionDecoder {
    /// Type assigned to matched transactions.
    transaction_type: UserTransactionType,
    /// Where the rule-matching name comes from.
    name_source: NameSource,
    /// Whether matched transactions are income.
    income: IncomeType,
    /// Rule pattern to category lookup for this type.
    categories: HashMap<String, PatternCategory>,
}

/// The result of successfully categorizing a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Categorization<'a> {
    /// Whether the transaction is income.
    pub income: IncomeType,
    /// Whether the transaction should be dropped.
    pub ignore: bool,
    /// The assigned category.
    pub category: &'a str,
    /// The decoded transaction type.
    pub transaction_type: UserTransactionType,
}

/// A transaction which could not be categorized.
#[derive(Debug, PartialEq, Eq)]
pub enum UncategorizedTransaction {
    /// No transaction type mapping matched.
    MissingType {
        /// Account name.
        account: String,
        /// Type reported by the source file.
        source_type: TransactionType,
        /// Raw transaction name.
        name: String,
    },
    /// A type was decoded but no rule matched.
    MissingRule {
        /// Account name.
        account: String,
        /// Decoded type.
        transaction_type: UserTransactionType,
        /// The name that was looked up in the rules.
        display: String,
    },
}

/// Outcome of [`Categorizer::categorize`].
#[derive(Debug, PartialEq, Eq)]
pub enum CategorizationStatus<'a> {
    /// The transaction was categorized.
    Categorized(Categorization<'a>),
    /// The transaction could not be categorized.
    Uncategorized(UncategorizedTransaction),
}

/// Lookup structure built from the config's transaction types and rules.
pub struct Categorizer {
    /// Mapping of account_name to a mapping between name prefixes and decoders
    /// `{account_name: {prefix: decoder}}`
    prefix_map: HashMap<String, GenericPatriciaMap<String, TransactionDecoder>>,
    /// Mapping of account_name to a mapping between transaction types and decoders
    /// `{account_name: {transaction_type: decoder}}`
    source_type_map: HashMap<String, HashMap<TransactionType, TransactionDecoder>>,
}

/// The category assigned by a single rule pattern.
#[derive(Debug, Clone)]
struct PatternCategory {
    /// Category name.
    category: String,
    /// Whether matching transactions are dropped.
    ignore: bool,
}

impl Categorizer {
    /// Build a categorizer from config.
    ///
    /// # Errors
    /// Fails if patterns or prefixes are duplicated, or a mapping is missing its prefix/type.
    pub fn build(
        transaction_types: &[TransactionTypeConfig],
        rules: &[TransactionRuleConfig],
    ) -> Result<Self> {
        let mut type_categories: HashMap<UserTransactionType, HashMap<String, PatternCategory>> =
            HashMap::new();
        for rule in rules {
            let entry = type_categories.entry(rule.transaction_type).or_default();

            for pattern_str in &rule.patterns {
                match entry.entry(pattern_str.trim().to_string()) {
                    Entry::Occupied(e) => {
                        bail!(
                            "Duplicate rule for pattern {:?}. Old category: {:?}, new category: {:?}",
                            e.key(),
                            e.get(),
                            &rule.category
                        );
                    }
                    Entry::Vacant(e) => {
                        e.insert(PatternCategory {
                            category: rule.category.clone(),
                            ignore: rule.ignore,
                        });
                    }
                }
            }
        }

        let mut prefix_map = HashMap::new();
        let mut source_type_map = HashMap::new();
        for type_config in transaction_types {
            let categories = type_categories
                .get(&type_config.transaction_type)
                .cloned()
                .unwrap_or_default();

            let decoder = TransactionDecoder {
                transaction_type: type_config.transaction_type,
                name_source: type_config.name_source,
                income: type_config.income,
                categories,
            };

            match type_config.mode {
                TransactionTypeMode::Prefix => {
                    let prefix = type_config
                        .prefix
                        .as_ref()
                        .ok_or_eyre("prefix required in Prefix mode")?;

                    for account in &type_config.accounts {
                        let account_prefixes: &mut GenericPatriciaMap<String, TransactionDecoder> =
                            prefix_map.entry(account.clone()).or_default();

                        if account_prefixes
                            .insert(prefix.clone(), decoder.clone())
                            .is_some()
                        {
                            bail!("Multiple transaction types use the prefix \"{}\"", prefix);
                        }
                    }
                }
                TransactionTypeMode::SourceType => {
                    let source_type = type_config
                        .source_type
                        .ok_or_eyre("source_type required in SourceType mode")?;

                    for account in &type_config.accounts {
                        let account_types: &mut HashMap<TransactionType, TransactionDecoder> =
                            source_type_map.entry(account.clone()).or_default();

                        if account_types.insert(source_type, decoder.clone()).is_some() {
                            bail!(
                                "Multiple transaction types use the source transaction type {:?}",
                                source_type
                            );
                        }
                    }
                }
            }
        }

        Ok(Self {
            prefix_map,
            source_type_map,
        })
    }

    /// Categorize one transaction.
    ///
    /// # Errors
    /// Fails if the matched mapping cannot produce a name (e.g. a missing memo), or if both a
    /// prefix and a source type mapping match.
    pub fn categorize(
        &self,
        account: &str,
        name: &str,
        transaction_tye: TransactionType,
        memo: Option<&str>,
    ) -> Result<CategorizationStatus<'_>> {
        let prefix_match = self
            .prefix_map
            .get(account)
            .and_then(|prefixes| prefixes.get_longest_common_prefix(name));

        let type_match = self
            .source_type_map
            .get(account)
            .and_then(|types| types.get(&transaction_tye));

        let mut matched_prefix = None;
        let decoder = match (prefix_match, type_match) {
            (Some((p, d)), None) => {
                matched_prefix = Some(p);
                d
            }
            (None, Some(d)) => d,
            (Some((p, _)), Some(_)) => bail!(
                "Transaction \"{name}\" matches both prefix \"{p}\" and source type {transaction_tye:?}"
            ),
            (None, None) => {
                return Ok(CategorizationStatus::Uncategorized(
                    UncategorizedTransaction::MissingType {
                        account: account.to_string(),
                        source_type: transaction_tye,
                        name: name.to_string(),
                    },
                ));
            }
        };

        let mut display_name = match decoder.name_source {
            NameSource::Memo => {
                memo.ok_or_eyre("Missing memo for transaction using memo as the name source")?
            }
            NameSource::Name => name,
            NameSource::NameSuffix => name
                .strip_prefix(
                    matched_prefix
                        .ok_or_eyre("NameSuffix name source cannot be used in SourceType mode")?,
                )
                .ok_or_eyre("Name does not contain selected prefix")?,
        };
        display_name = display_name.trim();

        let Some(category) = decoder.categories.get(display_name) else {
            return Ok(CategorizationStatus::Uncategorized(
                UncategorizedTransaction::MissingRule {
                    account: account.to_string(),
                    transaction_type: decoder.transaction_type,
                    display: display_name.to_string(),
                },
            ));
        };

        Ok(CategorizationStatus::Categorized(Categorization {
            income: decoder.income,
            ignore: category.ignore,
            category: &category.category,
            transaction_type: decoder.transaction_type,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    fn config() -> AppConfig {
        AppConfig::parse(
            r#"
[[transaction_type]]
prefix = "POS "
transaction_type = "DebitPurchase"
name_source = "NameSuffix"
accounts = ["Chequing"]

[[transaction_type]]
prefix = "INTEREST"
transaction_type = "Interest"
income = "yes"
name_source = "Memo"
accounts = ["Chequing"]

[[transaction_type]]
mode = "SourceType"
source_type = "Credit"
transaction_type = "CreditPurchase"
income = "auto"
name_source = "Name"
accounts = ["Visa"]

[[rule]]
transaction_type = "DebitPurchase"
category = "Food.Groceries"
patterns = ["GROCER"]

[[rule]]
transaction_type = "CreditPurchase"
category = "Transfer"
ignore = true
patterns = ["PAYMENT"]

[[rule]]
transaction_type = "Interest"
category = "Interest"
patterns = ["Monthly interest"]
"#,
        )
        .unwrap()
    }

    fn build() -> Categorizer {
        let config = config();
        Categorizer::build(&config.transaction_type, &config.rule).unwrap()
    }

    #[test]
    fn categorizes_by_prefix_suffix() {
        let c = build();
        let result = c
            .categorize("Chequing", "POS GROCER ", TransactionType::Pos, None)
            .unwrap();
        assert_eq!(
            result,
            CategorizationStatus::Categorized(Categorization {
                income: IncomeType::No,
                ignore: false,
                category: "Food.Groceries",
                transaction_type: UserTransactionType::DebitPurchase,
            })
        );
    }

    #[test]
    fn categorizes_by_memo() {
        let c = build();
        let result = c
            .categorize(
                "Chequing",
                "INTEREST PAID",
                TransactionType::Credit,
                Some("Monthly interest"),
            )
            .unwrap();
        let CategorizationStatus::Categorized(cat) = result else {
            panic!("{result:?}")
        };
        assert_eq!(cat.income, IncomeType::Yes);
        assert_eq!(cat.category, "Interest");

        assert!(
            c.categorize("Chequing", "INTEREST PAID", TransactionType::Credit, None)
                .is_err(),
            "memo name source requires a memo"
        );
    }

    #[test]
    fn categorizes_by_source_type() {
        let c = build();
        let result = c
            .categorize("Visa", "PAYMENT", TransactionType::Credit, None)
            .unwrap();
        let CategorizationStatus::Categorized(cat) = result else {
            panic!("{result:?}")
        };
        assert!(cat.ignore);
        assert_eq!(cat.income, IncomeType::Auto);
    }

    #[test]
    fn reports_missing_type_and_rule() {
        let c = build();
        assert_eq!(
            c.categorize("Visa", "SHOP", TransactionType::Debit, None)
                .unwrap(),
            CategorizationStatus::Uncategorized(UncategorizedTransaction::MissingType {
                account: "Visa".into(),
                source_type: TransactionType::Debit,
                name: "SHOP".into(),
            })
        );
        assert_eq!(
            c.categorize("Chequing", "POS  CAFE ", TransactionType::Pos, None)
                .unwrap(),
            CategorizationStatus::Uncategorized(UncategorizedTransaction::MissingRule {
                account: "Chequing".into(),
                transaction_type: UserTransactionType::DebitPurchase,
                display: "CAFE".into(),
            })
        );
    }

    #[test]
    fn rejects_duplicates() {
        let mut config = config();
        config.rule[1].transaction_type = UserTransactionType::DebitPurchase;
        config.rule[1].patterns = vec!["GROCER".into()];
        assert!(Categorizer::build(&config.transaction_type, &config.rule).is_err());

        let mut config = self::config();
        let dup = config.transaction_type[0].clone();
        config.transaction_type.push(dup);
        assert!(Categorizer::build(&config.transaction_type, &config.rule).is_err());

        let mut config = self::config();
        let dup = config.transaction_type[2].clone();
        config.transaction_type.push(dup);
        assert!(Categorizer::build(&config.transaction_type, &config.rule).is_err());
    }

    #[test]
    fn rejects_incomplete_mappings() {
        let mut config = config();
        config.transaction_type[0].prefix = None;
        assert!(Categorizer::build(&config.transaction_type, &config.rule).is_err());

        let mut config = self::config();
        config.transaction_type[2].source_type = None;
        assert!(Categorizer::build(&config.transaction_type, &config.rule).is_err());
    }

    #[test]
    fn ambiguous_match_errors() {
        let mut config = config();
        config.transaction_type[2].accounts.push("Chequing".into());
        let c = Categorizer::build(&config.transaction_type, &config.rule).unwrap();
        assert!(
            c.categorize("Chequing", "POS GROCER", TransactionType::Credit, None)
                .is_err()
        );
    }

    #[test]
    fn name_suffix_requires_prefix_mode() {
        let mut config = config();
        config.transaction_type[2].name_source = NameSource::NameSuffix;
        let c = Categorizer::build(&config.transaction_type, &config.rule).unwrap();
        assert!(
            c.categorize("Visa", "PAYMENT", TransactionType::Credit, None)
                .is_err()
        );
    }
}
