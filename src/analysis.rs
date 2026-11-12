//! Pure aggregation helpers used by the GUI views: date ranges, period binning, averaging,
//! totals and grouping of uncategorized transactions.

use std::collections::HashMap;

use chrono::{Datelike, Days, Months, NaiveDate};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use strum::{IntoStaticStr, VariantArray};

use crate::model::{TransactionRecord, UncategorizedKind, UncategorizedRecord};

/// The size of the bins transactions are grouped into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, VariantArray, IntoStaticStr)]
pub enum Period {
    /// Weeks starting on Monday.
    Week,
    /// Calendar months.
    #[default]
    Month,
    /// Calendar quarters.
    Quarter,
    /// Calendar years.
    Year,
}

impl Period {
    /// Display name.
    #[must_use]
    pub fn label(self) -> &'static str {
        self.into()
    }

    /// The first day of the period containing `date`.
    #[must_use]
    pub fn start_of(self, date: NaiveDate) -> NaiveDate {
        match self {
            Self::Week => date - Days::new(u64::from(date.weekday().num_days_from_monday())),
            Self::Month => date.with_day(1).unwrap_or(date),
            Self::Quarter => {
                let month = (date.month0() / 3) * 3 + 1;
                NaiveDate::from_ymd_opt(date.year(), month, 1).unwrap_or(date)
            }
            Self::Year => NaiveDate::from_ymd_opt(date.year(), 1, 1).unwrap_or(date),
        }
    }

    /// The first day of the period after the one starting at `start`.
    #[must_use]
    pub fn next(self, start: NaiveDate) -> NaiveDate {
        match self {
            Self::Week => start + Days::new(7),
            Self::Month => start + Months::new(1),
            Self::Quarter => start + Months::new(3),
            Self::Year => start + Months::new(12),
        }
    }

    /// Short axis label for the period starting at `start`.
    #[must_use]
    pub fn format(self, start: NaiveDate) -> String {
        match self {
            Self::Week => start.format("%Y-%m-%d").to_string(),
            Self::Month => start.format("%b %Y").to_string(),
            Self::Quarter => format!("Q{} {}", start.month0() / 3 + 1, start.year()),
            Self::Year => start.year().to_string(),
        }
    }

    /// Approximate number of days in a period, used to choose a sensible default.
    #[must_use]
    pub fn approx_days(self) -> i64 {
        match self {
            Self::Week => 7,
            Self::Month => 30,
            Self::Quarter => 91,
            Self::Year => 365,
        }
    }
}

/// An inclusive range of dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateRange {
    /// First included day.
    pub start: NaiveDate,
    /// Last included day.
    pub end: NaiveDate,
}

impl DateRange {
    /// Whether `date` falls within the range.
    #[must_use]
    pub fn contains(&self, date: NaiveDate) -> bool {
        self.start <= date && date <= self.end
    }
}

/// Number of `period`s that overlap `range` (at least 1 for a non-empty range).
#[must_use]
pub fn period_count(range: DateRange, period: Period) -> usize {
    let mut count = 0;
    let mut start = period.start_of(range.start);
    while start <= range.end {
        count += 1;
        start = period.next(start);
    }
    count
}

/// Common date range choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, VariantArray, IntoStaticStr)]
pub enum RangePreset {
    /// The last 3 months, including the current one.
    #[strum(serialize = "Last 3 months")]
    Last3Months,
    /// The last 6 months, including the current one.
    #[strum(serialize = "Last 6 months")]
    Last6Months,
    /// The last 12 months, including the current one.
    #[default]
    #[strum(serialize = "Last 12 months")]
    Last12Months,
    /// January 1st of this year until today.
    #[strum(serialize = "Year to date")]
    YearToDate,
    /// The previous calendar year.
    #[strum(serialize = "Last year")]
    LastYear,
    /// Everything in the dataset.
    #[strum(serialize = "All time")]
    AllTime,
    /// A user chosen range.
    Custom,
}

impl RangePreset {
    /// Display name.
    #[must_use]
    pub fn label(self) -> &'static str {
        self.into()
    }

    /// Resolve to concrete dates.
    ///
    /// `today` anchors relative presets, `bounds` (earliest, latest) is used by
    /// [`Self::AllTime`] and `custom` is returned for [`Self::Custom`].
    #[must_use]
    pub fn resolve(
        self,
        today: NaiveDate,
        bounds: Option<(NaiveDate, NaiveDate)>,
        custom: DateRange,
    ) -> DateRange {
        let months_back = |n: u32| DateRange {
            start: Period::Month.start_of(today) - Months::new(n - 1),
            end: today,
        };
        let year_start = Period::Year.start_of(today);
        match self {
            Self::Last3Months => months_back(3),
            Self::Last6Months => months_back(6),
            Self::Last12Months => months_back(12),
            Self::YearToDate => DateRange {
                start: year_start,
                end: today,
            },
            Self::LastYear => DateRange {
                start: year_start - Months::new(12),
                end: year_start.pred_opt().unwrap_or(year_start),
            },
            Self::AllTime => bounds.map_or(
                DateRange {
                    start: today,
                    end: today,
                },
                |(start, end)| DateRange { start, end },
            ),
            Self::Custom => custom,
        }
    }
}

/// Which part of the category hierarchy to group by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, VariantArray, IntoStaticStr)]
pub enum CategoryLevel {
    /// The first segment, e.g. `Food`.
    #[default]
    #[strum(serialize = "Base category")]
    Base,
    /// The full category, e.g. `Food.Groceries`.
    #[strum(serialize = "Full category")]
    Full,
}

impl CategoryLevel {
    /// Display name.
    #[must_use]
    pub fn label(self) -> &'static str {
        self.into()
    }

    /// The grouping key of a transaction at this level.
    #[must_use]
    pub fn key(self, transaction: &TransactionRecord) -> &str {
        match self {
            Self::Base => &transaction.base_category,
            Self::Full => &transaction.category,
        }
    }
}

/// How bar values are smoothed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, VariantArray, IntoStaticStr)]
pub enum Averaging {
    /// Show each period's total.
    #[default]
    None,
    /// Show the trailing mean over a window of periods.
    #[strum(serialize = "Moving average")]
    Moving,
    /// Show the mean over the whole range in every period.
    #[strum(serialize = "Range average")]
    Range,
}

impl Averaging {
    /// Display name.
    #[must_use]
    pub fn label(self) -> &'static str {
        self.into()
    }
}

/// One category's value per period.
#[derive(Debug, Clone, PartialEq)]
pub struct CategorySeries {
    /// Category name.
    pub name: String,
    /// Value per period, aligned with [`BinnedSeries::periods`].
    pub values: Vec<f64>,
    /// Sum of the un-averaged values.
    pub total: f64,
}

/// Values per category per period.
#[derive(Debug, Clone, PartialEq)]
pub struct BinnedSeries {
    /// Period size.
    pub period: Period,
    /// Start date of each period.
    pub periods: Vec<NaiveDate>,
    /// Series, largest total first.
    pub series: Vec<CategorySeries>,
}

impl BinnedSeries {
    /// Sum over all series of each period.
    #[must_use]
    pub fn period_totals(&self) -> Vec<f64> {
        let mut totals = vec![0.0; self.periods.len()];
        for series in &self.series {
            for (total, value) in totals.iter_mut().zip(&series.values) {
                *total += value;
            }
        }
        totals
    }

    /// Merge every series for which `keep` returns false into one series named `other`, placed
    /// last. Nothing changes if all series are kept.
    pub fn fold_into_other(&mut self, keep: impl Fn(&str) -> bool, other: &str) {
        let (kept, folded): (Vec<_>, Vec<_>) = std::mem::take(&mut self.series)
            .into_iter()
            .partition(|s| keep(&s.name));
        self.series = kept;
        if folded.is_empty() {
            return;
        }
        let mut merged = CategorySeries {
            name: other.to_string(),
            values: vec![0.0; self.periods.len()],
            total: 0.0,
        };
        for series in folded {
            merged.total += series.total;
            for (sum, value) in merged.values.iter_mut().zip(series.values) {
                *sum += value;
            }
        }
        self.series.push(merged);
    }

    /// Category names ordered by total, largest first.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.series.iter().map(|s| s.name.as_str()).collect()
    }

    /// Smooth every series in place.
    pub fn apply_averaging(&mut self, averaging: Averaging, window: usize) {
        let window = window.max(1);
        for series in &mut self.series {
            match averaging {
                Averaging::None => {}
                Averaging::Moving => {
                    let raw = series.values.clone();
                    for (idx, value) in series.values.iter_mut().enumerate() {
                        let lo = (idx + 1).saturating_sub(window);
                        let slice = &raw[lo..=idx];
                        #[allow(clippy::cast_precision_loss)]
                        let len = slice.len() as f64;
                        *value = slice.iter().sum::<f64>() / len;
                    }
                }
                Averaging::Range => {
                    if !series.values.is_empty() {
                        #[allow(clippy::cast_precision_loss)]
                        let mean = series.total / series.values.len() as f64;
                        series.values.fill(mean);
                    }
                }
            }
        }
    }
}

/// Convert an amount to `f64` for plotting.
#[must_use]
pub fn to_f64(amount: Decimal) -> f64 {
    amount.to_f64().unwrap_or(0.0)
}

/// Bin income (`income == true`) or expense transactions within `range` by category and
/// period.
///
/// Expenses are negated so that spending is positive. Every period overlapping the range is
/// present, even when empty.
#[must_use]
pub fn bin_by_category<'a>(
    transactions: impl IntoIterator<Item = &'a TransactionRecord>,
    income: bool,
    range: DateRange,
    period: Period,
    level: CategoryLevel,
) -> BinnedSeries {
    let mut periods = Vec::new();
    let mut index = HashMap::new();
    let mut start = period.start_of(range.start);
    while start <= range.end {
        index.insert(start, periods.len());
        periods.push(start);
        start = period.next(start);
    }

    let mut by_category: HashMap<&str, Vec<f64>> = HashMap::new();
    for t in transactions {
        if t.income != income || !range.contains(t.posted_date) {
            continue;
        }
        let Some(&idx) = index.get(&period.start_of(t.posted_date)) else {
            continue;
        };
        let value = if income {
            to_f64(t.amount)
        } else {
            -to_f64(t.amount)
        };
        by_category
            .entry(level.key(t))
            .or_insert_with(|| vec![0.0; periods.len()])[idx] += value;
    }

    let mut series: Vec<CategorySeries> = by_category
        .into_iter()
        .map(|(name, values)| CategorySeries {
            name: name.to_string(),
            total: values.iter().sum(),
            values,
        })
        .collect();
    series.sort_by(|a, b| {
        b.total
            .total_cmp(&a.total)
            .then_with(|| a.name.cmp(&b.name))
    });

    BinnedSeries {
        period,
        periods,
        series,
    }
}

/// Summary of a set of transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Totals {
    /// Sum of income amounts.
    pub income: Decimal,
    /// Sum of expense amounts, negated so spending is positive.
    pub expenses: Decimal,
    /// Number of transactions.
    pub count: usize,
}

impl Totals {
    /// Income minus expenses.
    #[must_use]
    pub fn net(&self) -> Decimal {
        self.income - self.expenses
    }
}

/// Sum income and expenses.
#[must_use]
pub fn totals<'a>(transactions: impl IntoIterator<Item = &'a TransactionRecord>) -> Totals {
    let mut totals = Totals::default();
    for t in transactions {
        totals.count += 1;
        if t.income {
            totals.income += t.amount;
        } else {
            totals.expenses -= t.amount;
        }
    }
    totals
}

/// Uncategorized transactions sharing the same kind, account and name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UncategorizedGroup {
    /// Why they are uncategorized.
    pub kind: UncategorizedKind,
    /// Account.
    pub account: String,
    /// Shared name.
    pub message: String,
    /// Number of transactions.
    pub count: usize,
    /// Sum of amounts.
    pub total: Decimal,
    /// Earliest date.
    pub first: NaiveDate,
    /// Latest date.
    pub last: NaiveDate,
}

/// Group uncategorized transactions, most frequent first.
#[must_use]
pub fn group_uncategorized(records: &[UncategorizedRecord]) -> Vec<UncategorizedGroup> {
    let mut groups: HashMap<(UncategorizedKind, &str, &str), UncategorizedGroup> = HashMap::new();
    for r in records {
        groups
            .entry((r.kind, &r.account, &r.message))
            .and_modify(|g| {
                g.count += 1;
                g.total += r.amount;
                g.first = g.first.min(r.posted_date);
                g.last = g.last.max(r.posted_date);
            })
            .or_insert_with(|| UncategorizedGroup {
                kind: r.kind,
                account: r.account.clone(),
                message: r.message.clone(),
                count: 1,
                total: r.amount,
                first: r.posted_date,
                last: r.posted_date,
            });
    }
    let mut groups: Vec<_> = groups.into_values().collect();
    groups.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.message.cmp(&b.message))
            .then_with(|| a.account.cmp(&b.account))
            .then_with(|| a.kind.cmp(&b.kind))
    });
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::UserTransactionType;
    use crate::importer::TransactionType;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn tx(date: NaiveDate, cents: i64, category: &str, income: bool) -> TransactionRecord {
        TransactionRecord {
            id: 0,
            account: "A".into(),
            base_category: category.split('.').next().unwrap().into(),
            category: category.into(),
            source_category: None,
            income,
            transaction_type: "DebitPurchase".into(),
            posted_date: date,
            amount: Decimal::new(cents, 2),
            transaction_id: None,
            name: "N".into(),
            memo: None,
        }
    }

    #[test]
    fn period_start_and_next() {
        let date = d(2024, 8, 15); // Thursday
        assert_eq!(Period::Week.start_of(date), d(2024, 8, 12));
        assert_eq!(Period::Month.start_of(date), d(2024, 8, 1));
        assert_eq!(Period::Quarter.start_of(date), d(2024, 7, 1));
        assert_eq!(Period::Year.start_of(date), d(2024, 1, 1));
        assert_eq!(Period::Week.next(d(2024, 8, 12)), d(2024, 8, 19));
        assert_eq!(Period::Month.next(d(2024, 12, 1)), d(2025, 1, 1));
        assert_eq!(Period::Quarter.next(d(2024, 10, 1)), d(2025, 1, 1));
        assert_eq!(Period::Year.next(d(2024, 1, 1)), d(2025, 1, 1));
    }

    #[test]
    fn period_labels() {
        assert_eq!(Period::Month.format(d(2024, 3, 1)), "Mar 2024");
        assert_eq!(Period::Quarter.format(d(2024, 4, 1)), "Q2 2024");
        assert_eq!(Period::Year.format(d(2024, 1, 1)), "2024");
        assert_eq!(Period::Week.format(d(2024, 1, 1)), "2024-01-01");
        assert_eq!(Period::Quarter.label(), "Quarter");
        assert!(
            Period::VARIANTS
                .windows(2)
                .all(|w| w[0].approx_days() < w[1].approx_days())
        );
    }

    #[test]
    fn counts_periods() {
        let range = DateRange {
            start: d(2024, 1, 15),
            end: d(2024, 3, 1),
        };
        assert_eq!(period_count(range, Period::Month), 3);
        assert_eq!(period_count(range, Period::Year), 1);
        assert_eq!(period_count(range, Period::Week), 7);
    }

    #[test]
    fn range_presets() {
        let today = d(2024, 8, 15);
        let custom = DateRange {
            start: d(2020, 1, 1),
            end: d(2020, 2, 1),
        };
        let bounds = Some((d(2019, 5, 5), d(2024, 8, 1)));
        let r = |p: RangePreset| p.resolve(today, bounds, custom);
        assert_eq!(r(RangePreset::Last3Months).start, d(2024, 6, 1));
        assert_eq!(r(RangePreset::Last6Months).start, d(2024, 3, 1));
        assert_eq!(r(RangePreset::Last12Months).start, d(2023, 9, 1));
        assert_eq!(r(RangePreset::Last12Months).end, today);
        assert_eq!(r(RangePreset::YearToDate).start, d(2024, 1, 1));
        assert_eq!(
            r(RangePreset::LastYear),
            DateRange {
                start: d(2023, 1, 1),
                end: d(2023, 12, 31)
            }
        );
        assert_eq!(r(RangePreset::AllTime).start, d(2019, 5, 5));
        assert_eq!(r(RangePreset::Custom), custom);
        assert_eq!(
            RangePreset::AllTime.resolve(today, None, custom).start,
            today
        );
        assert_eq!(RangePreset::YearToDate.label(), "Year to date");
    }

    #[test]
    fn bins_expenses_by_category() {
        let data = vec![
            tx(d(2024, 1, 5), -1000, "Food.Groceries", false),
            tx(d(2024, 1, 20), -500, "Food.Coffee", false),
            tx(d(2024, 1, 21), 200, "Food.Coffee", false), // refund
            tx(d(2024, 3, 2), -3000, "Rent", false),
            tx(d(2024, 3, 2), 9999, "Salary", true),
            tx(d(2023, 12, 31), -7777, "Rent", false), // out of range
        ];
        let range = DateRange {
            start: d(2024, 1, 1),
            end: d(2024, 3, 31),
        };
        let binned = bin_by_category(&data, false, range, Period::Month, CategoryLevel::Base);
        assert_eq!(
            binned.periods,
            vec![d(2024, 1, 1), d(2024, 2, 1), d(2024, 3, 1)]
        );
        assert_eq!(binned.series.len(), 2);
        assert_eq!(binned.series[0].name, "Rent");
        assert_eq!(binned.series[0].values, vec![0.0, 0.0, 30.0]);
        assert_eq!(binned.series[1].name, "Food");
        assert!((binned.series[1].values[0] - 13.0).abs() < 1e-9);
        assert_eq!(binned.period_totals()[2], 30.0);

        let full = bin_by_category(&data, false, range, Period::Quarter, CategoryLevel::Full);
        assert_eq!(full.periods.len(), 1);
        assert_eq!(full.series.len(), 3);

        let income = bin_by_category(&data, true, range, Period::Year, CategoryLevel::Base);
        assert_eq!(income.series[0].name, "Salary");
        assert!((income.series[0].total - 99.99).abs() < 1e-9);
    }

    #[test]
    fn averaging() {
        let mut binned = BinnedSeries {
            period: Period::Month,
            periods: vec![d(2024, 1, 1); 4],
            series: vec![CategorySeries {
                name: "X".into(),
                values: vec![4.0, 8.0, 0.0, 4.0],
                total: 16.0,
            }],
        };
        let mut moving = binned.clone();
        moving.apply_averaging(Averaging::Moving, 2);
        assert_eq!(moving.series[0].values, vec![4.0, 6.0, 4.0, 2.0]);

        let mut none = binned.clone();
        none.apply_averaging(Averaging::None, 3);
        assert_eq!(none, binned);

        binned.apply_averaging(Averaging::Range, 0);
        assert_eq!(binned.series[0].values, vec![4.0; 4]);
        assert_eq!(Averaging::Moving.label(), "Moving average");
    }

    #[test]
    fn folds_small_series() {
        let series = |name: &str, total| CategorySeries {
            name: name.into(),
            values: vec![total / 2.0; 2],
            total,
        };
        let mut binned = BinnedSeries {
            period: Period::Year,
            periods: vec![d(2023, 1, 1), d(2024, 1, 1)],
            series: vec![series("A", 10.0), series("B", 4.0), series("C", 2.0)],
        };
        let mut unchanged = binned.clone();
        unchanged.fold_into_other(|_| true, "Other");
        assert_eq!(unchanged.names(), vec!["A", "B", "C"]);

        binned.fold_into_other(|n| n == "A", "Other");
        assert_eq!(binned.names(), vec!["A", "Other"]);
        assert_eq!(binned.series[1].values, vec![3.0, 3.0]);
        assert_eq!(binned.series[1].total, 6.0);
    }

    #[test]
    fn computes_totals() {
        let data = vec![
            tx(d(2024, 1, 1), -1000, "Food", false),
            tx(d(2024, 1, 1), 250, "Food", false),
            tx(d(2024, 1, 1), 5000, "Salary", true),
        ];
        let t = totals(&data);
        assert_eq!(t.count, 3);
        assert_eq!(t.income, Decimal::new(5000, 2));
        assert_eq!(t.expenses, Decimal::new(750, 2));
        assert_eq!(t.net(), Decimal::new(4250, 2));
    }

    #[test]
    fn groups_uncategorized() {
        let rec = |date, cents, message: &str, kind| UncategorizedRecord {
            id: 0,
            kind,
            account: "A".into(),
            message: message.into(),
            posted_date: date,
            amount: Decimal::new(cents, 2),
        };
        let rule = UncategorizedKind::MissingRule(UserTransactionType::DebitPurchase);
        let missing = UncategorizedKind::MissingType(TransactionType::Atm);
        let records = vec![
            rec(d(2024, 2, 1), -100, "CAFE", rule),
            rec(d(2024, 1, 1), -300, "CAFE", rule),
            rec(d(2024, 3, 1), -2000, "ATM", missing),
        ];
        let groups = group_uncategorized(&records);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].message, "CAFE");
        assert_eq!(groups[0].count, 2);
        assert_eq!(groups[0].total, Decimal::new(-400, 2));
        assert_eq!(groups[0].first, d(2024, 1, 1));
        assert_eq!(groups[0].last, d(2024, 2, 1));
        assert_eq!(groups[1].kind, missing);
    }

    #[test]
    fn category_level_labels() {
        let t = tx(d(2024, 1, 1), 0, "Food.Coffee", false);
        assert_eq!(CategoryLevel::Base.key(&t), "Food");
        assert_eq!(CategoryLevel::Full.key(&t), "Food.Coffee");
        assert_eq!(CategoryLevel::Full.label(), "Full category");
    }
}
