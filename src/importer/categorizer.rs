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

#[derive(Debug, Clone)]
struct TransactionDecoder {
    transaction_type: UserTransactionType,
    name_source: NameSource,
    income: IncomeType,
    categories: HashMap<&'static str, PatternCategory>,
}

#[derive(Debug, Clone, Copy)]
pub struct Categorization {
    pub income: IncomeType,
    pub ignore: bool,
    pub category: &'static str,
}

#[derive(Debug)]
pub enum UncategorizedTransaction {
    MissingType {
        account: String,
        source_type: TransactionType,
        name: String,
    },
    MissingRule {
        account: String,
        transaction_type: UserTransactionType,
        display: String,
    },
}

#[derive(Debug)]
pub enum CategorizationStatus {
    Categorized(Categorization),
    Uncategorized(UncategorizedTransaction),
}

pub struct Categorizer {
    /// Mapping of account_name to a mapping between name prefixes and decoders
    /// `{account_name: {prefix: decoder}}`
    prefix_map: HashMap<&'static str, GenericPatriciaMap<String, TransactionDecoder>>,
    /// Mapping of account_name to a mapping between transaction types and decoders
    /// `{account_name: {transaction_type: decoder}}`
    source_type_map: HashMap<&'static str, HashMap<TransactionType, TransactionDecoder>>,
}

#[derive(Debug, Clone)]
struct PatternCategory {
    category: &'static str,
    ignore: bool,
}

impl Categorizer {
    pub fn build(
        transaction_types: &'static [TransactionTypeConfig],
        rules: &'static [TransactionRuleConfig],
    ) -> Result<Self> {
        let mut type_categories: HashMap<
            UserTransactionType,
            HashMap<&'static str, PatternCategory>,
        > = HashMap::new();
        for rule in rules {
            let entry = type_categories.entry(rule.transaction_type).or_default();

            for pattern_str in &rule.patterns {
                match entry.entry(pattern_str.as_str()) {
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
                            category: rule.category.as_str(),
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
                            prefix_map.entry(account.as_str()).or_default();

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
                            source_type_map.entry(account.as_str()).or_default();

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

    pub fn categorize(
        &self,
        account: &str,
        name: &str,
        transaction_tye: TransactionType,
        memo: Option<&str>,
    ) -> Result<CategorizationStatus> {
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
            (Some(_), Some(_)) => bail!("todo"),
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
            category: category.category,
        }))
    }
}
