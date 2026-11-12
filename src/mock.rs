//! Deterministic mock data: a config plus QFX and CSV statement files.
//!
//! The files go through the real importer, so mock data exercises the whole pipeline. The data
//! deliberately includes transactions with no type mapping and with no rule, so the
//! uncategorized view has something to show.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local, Months, NaiveDate};
use color_eyre::Result;
use color_eyre::eyre::Context;

use crate::config::{
    AccountConfig, AppConfig, IncomeType, NameSource, TransactionRuleConfig, TransactionTypeConfig,
    TransactionTypeMode, UserTransactionType,
};
use crate::importer::TransactionType;

/// Options for [`write_mock_data`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MockOptions {
    /// Last day of generated data.
    pub end: NaiveDate,
    /// Number of months of data, ending at `end`.
    pub months: u32,
    /// Random seed.
    pub seed: u64,
}

impl Default for MockOptions {
    fn default() -> Self {
        Self {
            end: Local::now().date_naive(),
            months: 24,
            seed: 0x5eed,
        }
    }
}

/// Small deterministic xorshift PRNG, so mock data needs no extra dependency.
struct Rng(u64);

impl Rng {
    /// Next raw value.
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform value in `lo..=hi`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = u64::try_from(hi - lo + 1).unwrap_or(1);
        lo + i64::try_from(self.next() % span).unwrap_or(0)
    }

    /// True with probability `percent`/100.
    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

/// One generated transaction.
struct MockTransaction {
    /// Posted date.
    date: NaiveDate,
    /// Signed amount in cents.
    cents: i64,
    /// Name field.
    name: String,
    /// Memo field.
    memo: Option<String>,
    /// Source type (QFX `TRNTYPE`; for CSV only the sign matters).
    kind: TransactionType,
}

/// Recurring purchase merchants on the chequing account: (name, typical cents, visits/month).
const CHEQUING_MERCHANTS: &[(&str, i64, i64)] = &[
    ("GROCERY MART", 8500, 4),
    ("FRESH FOODS", 4200, 2),
    ("COFFEE HOUSE", 550, 6),
    ("CITY TRANSIT", 1200, 3),
    ("BOOKSTORE", 2600, 1),
];

/// Card merchants on the Visa account.
const VISA_MERCHANTS: &[(&str, i64, i64)] = &[
    ("PIZZA PLACE", 3200, 2),
    ("GAS STATION", 6000, 3),
    ("STREAMFLIX", 1699, 1),
    ("ONLINE SHOP", 4500, 2),
    ("PHONE CO", 6500, 1),
    ("HARDWARE DEPOT", 7800, 1),
    ("NEW CAFE", 900, 2),
];

/// First day of the month containing `date`.
fn month_start(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

/// A random day within the month starting at `start`, never after `end`.
fn day_in_month(rng: &mut Rng, start: NaiveDate, end: NaiveDate) -> NaiveDate {
    let last = (start + Months::new(1))
        .pred_opt()
        .unwrap_or(start)
        .min(end);
    let span = (last - start).num_days();
    start + chrono::Days::new(u64::try_from(rng.range(0, span)).unwrap_or(0))
}

/// Generate chequing and visa transactions.
fn generate(options: MockOptions) -> (Vec<MockTransaction>, Vec<MockTransaction>) {
    let mut rng = Rng(options.seed.max(1));
    let mut chequing = Vec::new();
    let mut visa = Vec::new();
    let first = month_start(options.end) - Months::new(options.months.saturating_sub(1));

    let mut month = first;
    let mut serial = 0;
    while month <= options.end {
        // Seasonal multiplier: spend more in December, less in February.
        let season = match month.month() {
            12 => 140,
            2 => 85,
            7 | 8 => 115,
            _ => 100,
        };

        let push_merchants = |rng: &mut Rng,
                              list: &[(&str, i64, i64)],
                              out: &mut Vec<MockTransaction>,
                              prefix: &str| {
            for &(name, cents, visits) in list {
                for _ in 0..rng.range((visits - 1).max(0), visits + 1) {
                    let amount = cents * season / 100 * rng.range(70, 130) / 100;
                    out.push(MockTransaction {
                        date: day_in_month(rng, month, options.end),
                        cents: -amount,
                        name: format!("{prefix}{name}"),
                        memo: None,
                        kind: if prefix.is_empty() {
                            TransactionType::Debit
                        } else {
                            TransactionType::Pos
                        },
                    });
                }
            }
        };
        push_merchants(&mut rng, CHEQUING_MERCHANTS, &mut chequing, "POS ");
        push_merchants(&mut rng, VISA_MERCHANTS, &mut visa, "");

        let on_day = |day: u32| month.with_day(day).filter(|d| *d <= options.end);
        if let Some(date) = on_day(1) {
            chequing.push(MockTransaction {
                date,
                cents: -185_000,
                name: "EFT LANDLORD PROPERTIES".into(),
                memo: None,
                kind: TransactionType::Debit,
            });
        }
        for day in [15, 28] {
            if let Some(date) = on_day(day) {
                chequing.push(MockTransaction {
                    date,
                    cents: 265_000 + rng.range(-2000, 2000),
                    name: "PAYROLL ACME CORP".into(),
                    memo: None,
                    kind: TransactionType::Credit,
                });
            }
        }
        if let Some(date) = on_day(27) {
            chequing.push(MockTransaction {
                date,
                cents: rng.range(150, 900),
                name: "INTEREST PAID".into(),
                memo: Some("Monthly interest".into()),
                kind: TransactionType::Credit,
            });
            // Visa bill payment, ignored by a rule on the visa side.
            let total: i64 = visa
                .iter()
                .filter(|t| t.date >= month && t.cents < 0)
                .map(|t| -t.cents)
                .sum();
            visa.push(MockTransaction {
                date,
                cents: total,
                name: "PAYMENT THANK YOU".into(),
                memo: None,
                kind: TransactionType::Credit,
            });
        }
        if rng.chance(40) {
            // No type mapping exists for ATM transactions: shows up as uncategorized.
            chequing.push(MockTransaction {
                date: day_in_month(&mut rng, month, options.end),
                cents: -rng.range(2, 10) * 2000,
                name: "ATM WITHDRAWAL".into(),
                memo: None,
                kind: TransactionType::Atm,
            });
        }
        if rng.chance(20) {
            visa.push(MockTransaction {
                date: day_in_month(&mut rng, month, options.end),
                cents: rng.range(1000, 5000),
                name: "ONLINE SHOP".into(),
                memo: None,
                kind: TransactionType::Credit,
            });
        }

        serial += 1;
        month = first + Months::new(serial);
    }

    chequing.sort_by_key(|t| t.date);
    visa.sort_by_key(|t| t.date);
    (chequing, visa)
}

/// Format cents as a plain decimal string, e.g. `-12.30`.
fn format_cents(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", cents.abs() / 100, cents.abs() % 100)
}

/// Render transactions as an OFX 2 (XML) bank statement.
fn render_qfx(transactions: &[&MockTransaction]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <?OFX OFXHEADER=\"200\" VERSION=\"202\" SECURITY=\"NONE\" OLDFILEUID=\"NONE\" NEWFILEUID=\"NONE\"?>\n\
         <OFX><BANKMSGSRSV1><STMTTRNRS><TRNUID>1</TRNUID><STMTRS><CURDEF>CAD</CURDEF><BANKTRANLIST>\n",
    );
    for (idx, t) in transactions.iter().enumerate() {
        // QFX TRNTYPE values are the upper-cased type names.
        let kind = t.kind.name().to_uppercase();
        let _ = write!(
            out,
            "<STMTTRN><TRNTYPE>{kind}</TRNTYPE><DTPOSTED>{}120000.000[-5:EST]</DTPOSTED>\
             <DTUSER>{}120000.000</DTUSER><TRNAMT>{}</TRNAMT><FITID>{}{idx:05}</FITID>\
             <NAME>{}</NAME><CCACCTTO><ACCTID>0000</ACCTID></CCACCTTO>",
            t.date.format("%Y%m%d"),
            t.date.format("%Y%m%d"),
            format_cents(t.cents),
            t.date.format("%Y%m%d"),
            t.name,
        );
        if let Some(memo) = &t.memo {
            let _ = write!(out, "<MEMO>{memo}</MEMO>");
        }
        out.push_str("</STMTTRN>\n");
    }
    out.push_str("</BANKTRANLIST></STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>\n");
    out
}

/// Render transactions in the Capital One CSV format.
fn render_csv(transactions: &[&MockTransaction]) -> String {
    let mut out =
        String::from("Transaction Date,Posted Date,Card No.,Description,Category,Debit,Credit\n");
    for t in transactions {
        let (debit, credit) = if t.cents < 0 {
            (format_cents(-t.cents), String::new())
        } else {
            (String::new(), format_cents(t.cents))
        };
        let date = t.date.format("%Y-%m-%d");
        let _ = writeln!(out, "{date},{date},1234,{},Other,{debit},{credit}", t.name);
    }
    out
}

/// The config matching the generated files.
fn mock_config(chequing_dir: PathBuf, visa_dir: PathBuf) -> AppConfig {
    let prefix = |prefix: &str, transaction_type, income, name_source| TransactionTypeConfig {
        mode: TransactionTypeMode::Prefix,
        prefix: Some(prefix.into()),
        source_type: None,
        transaction_type,
        income,
        name_source,
        accounts: vec!["Chequing".into()],
    };
    let source = |source_type, transaction_type, income| TransactionTypeConfig {
        mode: TransactionTypeMode::SourceType,
        prefix: None,
        source_type: Some(source_type),
        transaction_type,
        income,
        name_source: NameSource::Name,
        accounts: vec!["Visa".into()],
    };
    let rule =
        |transaction_type, category: &str, ignore, patterns: &[&str]| TransactionRuleConfig {
            transaction_type,
            category: category.into(),
            ignore,
            patterns: patterns.iter().map(|p| (*p).to_string()).collect(),
        };
    use UserTransactionType as T;

    AppConfig {
        database: crate::config::DatabaseConfig::default(),
        account: vec![
            AccountConfig {
                name: "Chequing".into(),
                source_path: chequing_dir,
            },
            AccountConfig {
                name: "Visa".into(),
                source_path: visa_dir,
            },
        ],
        transaction_type: vec![
            prefix(
                "POS ",
                T::DebitPurchase,
                IncomeType::No,
                NameSource::NameSuffix,
            ),
            prefix(
                "EFT ",
                T::SentDirectDeposit,
                IncomeType::No,
                NameSource::NameSuffix,
            ),
            prefix(
                "PAYROLL ",
                T::ReceivedDirectDeposit,
                IncomeType::Yes,
                NameSource::NameSuffix,
            ),
            prefix("INTEREST", T::Interest, IncomeType::Yes, NameSource::Memo),
            source(TransactionType::Debit, T::CreditPurchase, IncomeType::No),
            source(TransactionType::Credit, T::CreditRefund, IncomeType::Auto),
        ],
        rule: vec![
            rule(
                T::DebitPurchase,
                "Food.Groceries",
                false,
                &["GROCERY MART", "FRESH FOODS"],
            ),
            rule(T::DebitPurchase, "Food.Coffee", false, &["COFFEE HOUSE"]),
            rule(
                T::DebitPurchase,
                "Transport.Transit",
                false,
                &["CITY TRANSIT"],
            ),
            rule(
                T::SentDirectDeposit,
                "Housing.Rent",
                false,
                &["LANDLORD PROPERTIES"],
            ),
            rule(T::ReceivedDirectDeposit, "Salary", false, &["ACME CORP"]),
            rule(T::Interest, "Interest", false, &["Monthly interest"]),
            rule(
                T::CreditPurchase,
                "Food.Restaurants",
                false,
                &["PIZZA PLACE"],
            ),
            rule(T::CreditPurchase, "Transport.Gas", false, &["GAS STATION"]),
            rule(
                T::CreditPurchase,
                "Entertainment.Streaming",
                false,
                &["STREAMFLIX"],
            ),
            rule(
                T::CreditPurchase,
                "Shopping.Online",
                false,
                &["ONLINE SHOP"],
            ),
            rule(T::CreditPurchase, "Utilities.Phone", false, &["PHONE CO"]),
            rule(T::CreditRefund, "Shopping.Online", false, &["ONLINE SHOP"]),
            rule(T::CreditRefund, "Transfer", true, &["PAYMENT THANK YOU"]),
        ],
        budget: Vec::new(),
    }
}

/// Write mock statements and a matching `config.toml` under `dir`, returning the config path.
///
/// Statements are split into one file per account per calendar year.
///
/// # Errors
/// Fails if any file cannot be written.
pub fn write_mock_data(dir: &Path, options: MockOptions) -> Result<PathBuf> {
    let chequing_dir = dir.join("statements").join("chequing");
    let visa_dir = dir.join("statements").join("visa");
    std::fs::create_dir_all(&chequing_dir).wrap_err("Failed to create mock directory")?;
    std::fs::create_dir_all(&visa_dir).wrap_err("Failed to create mock directory")?;

    let (chequing, visa) = generate(options);
    let mut years: Vec<i32> = chequing.iter().map(|t| t.date.year()).collect();
    years.dedup();
    for year in years {
        fn in_year(list: &[MockTransaction], year: i32) -> Vec<&MockTransaction> {
            list.iter().filter(|t| t.date.year() == year).collect()
        }
        std::fs::write(
            chequing_dir.join(format!("chequing-{year}.qfx")),
            render_qfx(&in_year(&chequing, year)),
        )
        .wrap_err("Failed to write mock QFX")?;
        std::fs::write(
            visa_dir.join(format!("visa-{year}.csv")),
            render_csv(&in_year(&visa, year)),
        )
        .wrap_err("Failed to write mock CSV")?;
    }

    let config_path = dir.join("config.toml");
    mock_config(chequing_dir, visa_dir).save(&config_path)?;
    Ok(config_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{LoadStatus, load_with_config};
    use crate::model::UncategorizedKind;

    fn options() -> MockOptions {
        MockOptions {
            end: NaiveDate::from_ymd_opt(2025, 6, 30).unwrap(),
            months: 18,
            seed: 42,
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let (a, _) = generate(options());
        let (b, _) = generate(options());
        assert_eq!(a.len(), b.len());
        assert!(
            a.iter()
                .zip(&b)
                .all(|(x, y)| x.cents == y.cents && x.date == y.date)
        );
        assert!(a.iter().all(|t| t.date <= options().end));
    }

    #[test]
    fn format_cents_signs() {
        assert_eq!(format_cents(-1230), "-12.30");
        assert_eq!(format_cents(5), "0.05");
    }

    #[tokio::test]
    async fn mock_data_imports_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = write_mock_data(dir.path(), options()).unwrap();
        assert!(
            dir.path()
                .join("statements/chequing/chequing-2024.qfx")
                .exists()
        );
        assert!(dir.path().join("statements/visa/visa-2025.csv").exists());

        let config = AppConfig::load(&config_path).unwrap();
        assert!(config.validate().is_empty());
        let loaded = load_with_config(config, None, &LoadStatus::default())
            .await
            .unwrap();
        let data = loaded.dataset;

        assert!(
            data.transactions
                .iter()
                .any(|t| t.income && t.category == "Salary")
        );
        assert!(data.transactions.iter().any(|t| t.base_category == "Food"));
        assert!(
            !data.transactions.iter().any(|t| t.category == "Transfer"),
            "ignored rule must drop transactions"
        );
        assert!(
            data.uncategorized
                .iter()
                .any(|u| u.kind == UncategorizedKind::MissingType(TransactionType::Atm))
        );
        assert!(data.uncategorized.iter().any(|u| u.message == "BOOKSTORE"));
        assert!(
            data.uncategorized
                .iter()
                .any(|u| u.message == "HARDWARE DEPOT")
        );
        let bounds = data.date_bounds().unwrap();
        assert_eq!(bounds.0, NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
        assert!(bounds.1 <= options().end);
    }
}
