//! Plain data records read back from the database and shown in the GUI.

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::config::UserTransactionType;
use crate::importer::TransactionType;

/// A categorized transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRecord {
    /// Database row id.
    pub id: i64,
    /// Account the transaction was imported from.
    pub account: String,
    /// First segment of [`Self::category`].
    pub base_category: String,
    /// Full dot separated category.
    pub category: String,
    /// Category reported by the bank, if any.
    pub source_category: Option<String>,
    /// Whether this transaction is income (otherwise an expense).
    pub income: bool,
    /// Decoded transaction type name.
    pub transaction_type: String,
    /// Date the transaction posted.
    pub posted_date: NaiveDate,
    /// Signed amount. Expenses are normally negative.
    pub amount: Decimal,
    /// Bank provided transaction id.
    pub transaction_id: Option<String>,
    /// Transaction name.
    pub name: String,
    /// Transaction memo.
    pub memo: Option<String>,
}

/// Why a transaction could not be categorized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UncategorizedKind {
    /// No [`crate::config::TransactionTypeConfig`] matched the raw transaction.
    MissingType(TransactionType),
    /// The type was decoded, but no rule matched the name.
    MissingRule(UserTransactionType),
}

impl UncategorizedKind {
    /// Short label for display.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::MissingType(t) => format!("No type mapping ({})", t.name()),
            Self::MissingRule(t) => format!("No rule ({})", t.name()),
        }
    }
}

/// A transaction that could not be categorized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncategorizedRecord {
    /// Database row id.
    pub id: i64,
    /// Why categorization failed.
    pub kind: UncategorizedKind,
    /// Account the transaction was imported from.
    pub account: String,
    /// The name used for matching (the raw name for [`UncategorizedKind::MissingType`]).
    pub message: String,
    /// Date the transaction posted.
    pub posted_date: NaiveDate,
    /// Signed amount.
    pub amount: Decimal,
}

/// Everything loaded from the database for display.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dataset {
    /// Categorized transactions, ordered by date.
    pub transactions: Vec<TransactionRecord>,
    /// Uncategorized transactions, ordered by date.
    pub uncategorized: Vec<UncategorizedRecord>,
}

impl Dataset {
    /// Earliest and latest posted dates over all transactions, if any exist.
    #[must_use]
    pub fn date_bounds(&self) -> Option<(NaiveDate, NaiveDate)> {
        let dates = self
            .transactions
            .iter()
            .map(|t| t.posted_date)
            .chain(self.uncategorized.iter().map(|t| t.posted_date));
        dates.fold(None, |acc, d| match acc {
            None => Some((d, d)),
            Some((lo, hi)) => Some((lo.min(d), hi.max(d))),
        })
    }
}

/// Convert an amount to integer cents, rounding half away from zero.
#[must_use]
pub fn to_cents(amount: Decimal) -> i64 {
    use rust_decimal::prelude::ToPrimitive;
    (amount * Decimal::ONE_HUNDRED)
        .round_dp_with_strategy(0, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .unwrap_or(i64::MAX)
}

/// Convert integer cents back to an amount.
#[must_use]
pub fn from_cents(cents: i64) -> Decimal {
    Decimal::new(cents, 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cents_round_trip() {
        assert_eq!(to_cents(Decimal::new(-1234, 2)), -1234);
        assert_eq!(to_cents(Decimal::new(12345, 3)), 1235);
        assert_eq!(to_cents(Decimal::new(-12345, 3)), -1235);
        assert_eq!(from_cents(-1234), Decimal::new(-1234, 2));
    }

    #[test]
    fn date_bounds() {
        let mut data = Dataset::default();
        assert_eq!(data.date_bounds(), None);
        let d = |m, d| NaiveDate::from_ymd_opt(2024, m, d).unwrap();
        data.uncategorized.push(UncategorizedRecord {
            id: 1,
            kind: UncategorizedKind::MissingType(TransactionType::Debit),
            account: "A".into(),
            message: "X".into(),
            posted_date: d(3, 1),
            amount: Decimal::ONE,
        });
        data.transactions.push(TransactionRecord {
            id: 1,
            account: "A".into(),
            base_category: "Food".into(),
            category: "Food".into(),
            source_category: None,
            income: false,
            transaction_type: "DebitPurchase".into(),
            posted_date: d(1, 5),
            amount: Decimal::ONE,
            transaction_id: None,
            name: "N".into(),
            memo: None,
        });
        assert_eq!(data.date_bounds(), Some((d(1, 5), d(3, 1))));
    }

    #[test]
    fn kind_labels() {
        assert_eq!(
            UncategorizedKind::MissingRule(UserTransactionType::Interest).label(),
            "No rule (Interest)"
        );
        assert_eq!(
            UncategorizedKind::MissingType(TransactionType::Atm).label(),
            "No type mapping (Atm)"
        );
    }
}
