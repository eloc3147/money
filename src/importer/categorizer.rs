use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use color_eyre::Result;
use color_eyre::eyre::{OptionExt, bail};
use patricia_tree::GenericPatriciaMap;

use crate::importer::config::{
    NameSource, TransactionRuleConfig, TransactionTypeConfig, UserTransactionType,
};

#[derive(Debug, Clone)]
struct TransactionDecoder {
    transaction_type: UserTransactionType,
    name_source: NameSource,
    categories: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct MissingRuleInfo {
    pub account: String,
    pub prefix: String,
    pub transaction_type: Option<UserTransactionType>,
    pub display: String,
    pub name: String,
    pub memo: String,
}

pub struct Categorizer {
    test_mode: bool,
    dbg_uc_log: File,
    dbg_m_log: File,
    dbg_c_log: File,

    accounts: HashMap<String, GenericPatriciaMap<String, TransactionDecoder>>,
    missing_prefix: HashMap<MissingRuleInfo, usize>,
    missing_rule: HashMap<MissingRuleInfo, usize>,
}

impl Categorizer {
    pub fn build(
        transaction_types: Vec<TransactionTypeConfig>,
        rules: Vec<TransactionRuleConfig>,
        test_mode: bool,
    ) -> Result<Self> {
        // 1st LUT: dict[str<account_name>, list[(prefix, parser)]]
        // 2nd LUT: list[(pattern, category)]

        let mut type_categories: HashMap<UserTransactionType, HashMap<String, String>> =
            HashMap::new();
        for rule in rules {
            let entry = type_categories.entry(rule.transaction_type).or_default();

            for pattern_str in rule.patterns {
                match entry.entry(pattern_str) {
                    Entry::Occupied(e) => {
                        bail!(
                            "Duplicate rule for pattern {:?}. Old category: {:?}, new category: {:?}",
                            e.key(),
                            e.get(),
                            &rule.category
                        );
                    }
                    Entry::Vacant(e) => {
                        e.insert(rule.category.clone());
                    }
                }
            }
        }

        let mut accounts = HashMap::new();
        for type_config in transaction_types {
            let categories = type_categories
                .get(&type_config.transaction_type)
                .map(Clone::clone)
                .unwrap_or_default();

            let decoder = TransactionDecoder {
                transaction_type: type_config.transaction_type,
                name_source: type_config.name_source,
                categories,
            };

            for account in type_config.accounts {
                let entry: &mut GenericPatriciaMap<String, TransactionDecoder> =
                    accounts.entry(account).or_default();

                let existing = entry.insert(type_config.prefix.clone(), decoder.clone());
                if existing.is_some() {
                    bail!(
                        "Multiple transaction types use the prefix \"{}\"",
                        type_config.prefix
                    );
                }
            }
        }

        Ok(Self {
            test_mode,
            dbg_uc_log: File::create(&PathBuf::from("uncategorized.log")).unwrap(),
            dbg_m_log: File::create(&PathBuf::from("med_categorized.log")).unwrap(),
            dbg_c_log: File::create(&PathBuf::from("categorized.log")).unwrap(),
            accounts,
            missing_prefix: HashMap::new(),
            missing_rule: HashMap::new(),
        })
    }

    pub fn categorize(
        &mut self,
        account: &str,
        name: &str,
        memo: Option<&str>,
    ) -> Result<Option<&str>> {
        let Some(prefixes) = self.accounts.get(account) else {
            return Ok(None);
        };

        let Some((prefix, decoder)) = prefixes.get_longest_common_prefix(name) else {
            let count = self
                .missing_prefix
                .entry(MissingRuleInfo {
                    account: account.to_string(),
                    prefix: String::new(),
                    display: String::new(),
                    transaction_type: None,
                    name: name.to_string(),
                    memo: memo.unwrap_or_default().to_string(),
                })
                .or_default();

            *count += 1;

            write!(
                &mut self.dbg_uc_log,
                "{:40} | {:50}\n",
                name,
                memo.unwrap_or_default()
            )
            .unwrap();

            return Ok(None);
        };

        let display_name = match decoder.name_source {
            NameSource::Memo => match memo {
                Some(m) => m,
                None => bail!("Missing memo for transaction using memo as the name source"),
            },
            NameSource::Name => name,
            NameSource::NameSuffix => name
                .strip_prefix(prefix)
                .ok_or_eyre("Name does not contain selected prefix")?,
        };

        let tn = format!("{:?}", decoder.transaction_type);
        write!(
            &mut self.dbg_m_log,
            "{:20} | {:40} | {:40} | {:50}\n",
            tn,
            display_name,
            name,
            memo.unwrap_or_default()
        )
        .unwrap();

        let Some(category) = decoder.categories.get(display_name) else {
            let count = self
                .missing_rule
                .entry(MissingRuleInfo {
                    account: account.to_string(),
                    prefix: prefix.to_string(),
                    transaction_type: Some(decoder.transaction_type),
                    display: display_name.to_string(),
                    name: name.to_string(),
                    memo: memo.unwrap_or_default().to_string(),
                })
                .or_default();

            *count += 1;
            return Ok(None);
        };

        write!(
            &mut self.dbg_c_log,
            "{:20} | {:40} | {:40} | {:50}\n",
            category,
            display_name,
            name,
            memo.unwrap_or_default()
        )
        .unwrap();

        Ok(Some(category))
    }

    pub fn get_missing_stats(
        &self,
    ) -> (
        &HashMap<MissingRuleInfo, usize>,
        &HashMap<MissingRuleInfo, usize>,
    ) {
        (&self.missing_prefix, &self.missing_rule)
    }
}
