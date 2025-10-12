use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use color_eyre::Result;
use color_eyre::eyre::{OptionExt, bail};
use patricia_tree::GenericPatriciaMap;

use crate::importer::TransactionType;
use crate::importer::config::{
    NameSource, TransactionRuleConfig, TransactionTypeConfig, TransactionTypeMode,
    UserTransactionType,
};

#[derive(Debug, Clone)]
struct TransactionDecoder {
    transaction_type: UserTransactionType,
    name_source: NameSource,
    income: bool,
    categories: HashMap<&'static str, PatternCategory>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct MissingTypeInfo {
    pub account: String,
    pub source_type: TransactionType,
    pub name: String,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct MissingRuleInfo {
    pub account: String,
    pub transaction_type: UserTransactionType,
    pub display: String,
}

#[derive(Debug, Clone, Copy)]
pub struct CategorizationResult {
    pub income: bool,
    pub ignore: bool,
    pub category: &'static str,
}

pub struct Categorizer {
    /// Mapping of account_name to a mapping between name prefixes and decoders
    /// `{account_name: {prefix: decoder}}`
    prefix_map: HashMap<&'static str, GenericPatriciaMap<String, TransactionDecoder>>,
    /// Mapping of account_name to a mapping between transaction types and decoders
    /// `{account_name: {transaction_type: decoder}}`
    source_type_map: HashMap<&'static str, HashMap<TransactionType, TransactionDecoder>>,
    /// All categories, and whether they are income (true) or expenses (false)
    categories: HashSet<(&'static str, bool)>,
    /// Count of transactions that could not find a transaction type
    unknown_type_counts: HashMap<MissingTypeInfo, usize>,
    /// Count of transactions that could not find a category
    unknown_category_counts: HashMap<MissingRuleInfo, usize>,
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

        let mut used_categories = HashSet::new();
        let mut prefix_map = HashMap::new();
        let mut source_type_map = HashMap::new();
        for type_config in transaction_types {
            let categories = type_categories
                .get(&type_config.transaction_type).cloned()
                .unwrap_or_default();

            for category in categories.values() {
                if !category.ignore {
                    used_categories.insert((category.category, type_config.income));
                }
            }

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
            categories: used_categories,
            unknown_type_counts: HashMap::new(),
            unknown_category_counts: HashMap::new(),
        })
    }

    pub fn categories(&self) -> &HashSet<(&'static str, bool)> {
        &self.categories
    }

    pub fn categorize(
        &mut self,
        account: &str,
        name: &str,
        transaction_tye: TransactionType,
        memo: Option<&str>,
    ) -> Result<Option<CategorizationResult>> {
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
                let count = self
                    .unknown_type_counts
                    .entry(MissingTypeInfo {
                        account: account.to_string(),
                        source_type: transaction_tye,
                        name: name.to_string(),
                    })
                    .or_default();

                *count += 1;
                return Ok(None);
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
            let count = self
                .unknown_category_counts
                .entry(MissingRuleInfo {
                    account: account.to_string(),
                    transaction_type: decoder.transaction_type,
                    display: display_name.to_string(),
                })
                .or_default();

            *count += 1;

            return Ok(None);
        };

        Ok(Some(CategorizationResult {
            income: decoder.income,
            ignore: category.ignore,
            category: category.category,
        }))
    }

    pub fn get_missing_stats(
        &self,
    ) -> (
        &HashMap<MissingTypeInfo, usize>,
        &HashMap<MissingRuleInfo, usize>,
    ) {
        (&self.unknown_type_counts, &self.unknown_category_counts)
    }
}
