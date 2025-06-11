use std::collections::HashMap;

use color_eyre::Result;
use color_eyre::eyre::{Context, eyre};
use regex::Regex;

use crate::importer::config::TransactionRule;

pub struct Categorizer {
    categories: HashMap<u8, Vec<(Regex, String)>>,
    test_mode: bool,
}

impl Categorizer {
    pub fn build(rules: Vec<TransactionRule>, test_mode: bool) -> Result<Self> {
        let mut categories = HashMap::new();
        for rule in rules {
            let priority_entry: &mut Vec<(Regex, String)> =
                categories.entry(rule.priority).or_default();

            for pattern_str in rule.patterns {
                let pattern = Regex::new(&pattern_str).wrap_err_with(|| {
                    format!(
                        "Cannot compile pattern \"{}\" for category {}, priority {}",
                        &pattern_str, rule.category, rule.priority
                    )
                })?;
                priority_entry.push((pattern, rule.category.clone()));
            }
        }

        Ok(Self {
            categories,
            test_mode,
        })
    }

    pub fn find_category(&self, name: &str) -> Result<Option<&str>> {
        // Duplicates some code but avoid any alloc in normal case
        if self.test_mode {
            let mut categories = Vec::new();

            for priority in self.categories.values() {
                for (pattern, category) in priority {
                    if pattern.is_match_at(name, 0) {
                        categories.push(category);
                    }
                }
            }

            if categories.len() > 1 {
                let mut categories_string = String::new();
                for (idx, category) in categories.iter().enumerate() {
                    if idx > 0 {
                        categories_string.push(',');
                        categories_string.push(' ');
                    }
                    categories_string.push_str(&category);
                }

                Err(eyre!(
                    "Transaction \"{}\" matches multiple categories: {}",
                    name,
                    categories_string
                ))
            } else if categories.len() == 1 {
                Ok(Some(categories[0]))
            } else {
                Ok(None)
            }
        } else {
            for priority in self.categories.values() {
                for (pattern, category) in priority {
                    if pattern.is_match_at(name, 0) {
                        return Ok(Some(category));
                    }
                }
            }

            Ok(None)
        }
    }
}
