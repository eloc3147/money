use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use patricia_tree::GenericPatriciaMap;

use crate::db::UncategorizedTransaction;
use crate::error::{CategorizationError, CategoryRuleError};
use crate::loader::TransactionType;
use crate::loader::config::{
    NameSource, TransactionRuleConfig, TransactionTypeConfig, TransactionTypeMode,
    UserTransactionType,
};

#[derive(Debug, Clone)]
struct TransactionDecoder {
    transaction_type: UserTransactionType,
    name_source: NameSource,
    income: bool,
    categories: HashMap<String, PatternCategory>,
}

#[derive(Debug, Clone)]
pub struct Categorization {
    pub income: bool,
    pub ignore: bool,
    pub category: String,
}

#[derive(Debug)]
pub enum CategorizationStatus {
    Categorized(Categorization),
    Uncategorized(UncategorizedTransaction),
}

pub struct Categorizer {
    /// Mapping of account_name to a mapping between name prefixes and decoders
    /// `{account_name: {prefix: decoder}}`
    prefix_map: HashMap<String, GenericPatriciaMap<String, TransactionDecoder>>,
    /// Mapping of account_name to a mapping between transaction types and decoders
    /// `{account_name: {transaction_type: decoder}}`
    source_type_map: HashMap<String, HashMap<TransactionType, TransactionDecoder>>,
    /// All categories, and whether they are income (true) or expenses (false)
    categories: HashSet<(String, bool)>,
}

#[derive(Debug, Clone)]
struct PatternCategory {
    category: String,
    ignore: bool,
}

impl Categorizer {
    pub fn build(
        transaction_types: Vec<TransactionTypeConfig>,
        rules: Vec<TransactionRuleConfig>,
    ) -> Result<Self, CategoryRuleError> {
        let mut type_categories: HashMap<UserTransactionType, HashMap<String, PatternCategory>> =
            HashMap::new();
        for rule in rules {
            let entry = type_categories.entry(rule.transaction_type).or_default();

            for pattern_str in rule.patterns {
                match entry.entry(pattern_str) {
                    Entry::Occupied(e) => {
                        return Err(CategoryRuleError::DuplicateRule {
                            pattern: e.key().clone(),
                            existing: e.get().category.clone(),
                            new: rule.category,
                        });
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

        let mut used_categories = HashSet::new();
        let mut prefix_map = HashMap::new();
        let mut source_type_map = HashMap::new();
        for type_config in transaction_types {
            let categories = type_categories
                .get(&type_config.transaction_type)
                .cloned()
                .unwrap_or_default();

            for category in categories.values() {
                if !category.ignore {
                    used_categories.insert((category.category.clone(), type_config.income));
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
                    let prefix = type_config.prefix.ok_or(CategoryRuleError::MissingPrefix)?;

                    for account in type_config.accounts {
                        let account_prefixes: &mut GenericPatriciaMap<String, TransactionDecoder> =
                            prefix_map.entry(account).or_default();

                        if account_prefixes
                            .insert(prefix.clone(), decoder.clone())
                            .is_some()
                        {
                            return Err(CategoryRuleError::DuplicatePrefix(prefix));
                        }
                    }
                }
                TransactionTypeMode::SourceType => {
                    let source_type = type_config
                        .source_type
                        .ok_or(CategoryRuleError::MissingSourceType)?;

                    for account in type_config.accounts {
                        let account_types: &mut HashMap<TransactionType, TransactionDecoder> =
                            source_type_map.entry(account).or_default();

                        if account_types.insert(source_type, decoder.clone()).is_some() {
                            return Err(CategoryRuleError::DuplicateSourceType(source_type));
                        }
                    }
                }
            }
        }

        Ok(Self {
            prefix_map,
            source_type_map,
            categories: used_categories,
        })
    }

    pub fn categories(&self) -> &HashSet<(String, bool)> {
        &self.categories
    }

    pub fn categorize(
        &self,
        account: &str,
        name: &str,
        transaction_tye: TransactionType,
        memo: Option<&str>,
    ) -> Result<CategorizationStatus, CategorizationError> {
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
            (Some((prefix, _)), Some(_)) => {
                return Err(CategorizationError::MatchedTypeAndPrefix {
                    account: account.to_owned(),
                    prefix: prefix.to_owned(),
                    transaction_type: transaction_tye,
                    name: name.to_owned(),
                });
            }
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
            NameSource::Memo => memo.ok_or(CategorizationError::MissingMemo)?,
            NameSource::Name => name,
            NameSource::NameSuffix => name
                .strip_prefix(matched_prefix.ok_or(CategorizationError::NameSuffixInSourceType)?)
                .ok_or(CategorizationError::PrefixNotContained)?,
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
            category: category.category.clone(),
        }))
    }
}
