use thiserror::Error;

use crate::db::TransactionType;

#[derive(Error, Debug)]
pub enum CategoryRuleError {
    #[error("duplicate rule for pattern '{pattern}' (existing: '{existing}', new: '{new}')")]
    DuplicateRule {
        pattern: String,
        existing: String,
        new: String,
    },
    #[error("prefix required in Prefix mode")]
    MissingPrefix,
    #[error("multiple transaction types use the prefix '{0}'")]
    DuplicatePrefix(String),
    #[error("source_type required in SourceType mode")]
    MissingSourceType,
    #[error("multiple transaction types use the source transaction type {0:?}")]
    DuplicateSourceType(TransactionType),
}

#[derive(Error, Debug)]
pub enum CategorizationError {
    #[error(
        "transaction from account '{account}' matched both prefix '{prefix}' and type {transaction_type:?} for name '{name}'"
    )]
    MatchedTypeAndPrefix {
        account: String,
        prefix: String,
        transaction_type: TransactionType,
        name: String,
    },
    #[error("missing memo for transaction using memo as the name source")]
    MissingMemo,
    #[error("NameSuffix name source cannot be used in SourceType mode")]
    NameSuffixInSourceType,
    #[error("name does not contain selected prefix")]
    PrefixNotContained,
}
