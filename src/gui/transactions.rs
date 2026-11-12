//! The transaction list, split into income and expenses.

use std::cmp::Ordering;

use chrono::NaiveDate;
use eframe::egui::{self, RichText, Ui};
use egui_extras::{Column, TableBuilder};
use rust_decimal::Decimal;
use strum::{IntoStaticStr, VariantArray};

use super::range::RangeSelector;
use super::style::{amount_label, money, stat_tile};
use crate::analysis::{RangePreset, totals};
use crate::model::{Dataset, TransactionRecord};

/// Which side of the ledger is listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, VariantArray, IntoStaticStr)]
pub enum Side {
    /// Expense transactions.
    #[default]
    Expenses,
    /// Income transactions.
    Income,
}

impl Side {
    /// Whether this side lists income.
    #[must_use]
    pub fn is_income(self) -> bool {
        self == Self::Income
    }
}

/// Table columns that can be sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, VariantArray, IntoStaticStr)]
pub enum SortColumn {
    /// Posted date.
    Date,
    /// Account name.
    Account,
    /// Category.
    Category,
    /// Transaction name.
    Name,
    /// Amount.
    Amount,
}

impl SortColumn {
    /// Compare two transactions by this column.
    fn compare(self, a: &TransactionRecord, b: &TransactionRecord) -> Ordering {
        match self {
            Self::Date => a.posted_date.cmp(&b.posted_date),
            Self::Account => a.account.cmp(&b.account),
            Self::Category => a.category.cmp(&b.category),
            Self::Name => a.name.cmp(&b.name),
            Self::Amount => a.amount.cmp(&b.amount),
        }
        .then_with(|| a.id.cmp(&b.id))
    }
}

/// Filters applied to the transaction list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionFilter {
    /// Income or expenses.
    pub side: Side,
    /// Date range.
    pub range: RangeSelector,
    /// Case-insensitive text matched against name, memo, category and account.
    pub search: String,
    /// Only show this base category.
    pub category: Option<String>,
    /// Only show this account.
    pub account: Option<String>,
}

impl TransactionFilter {
    /// Indices into `transactions` that pass the filter.
    #[must_use]
    pub fn apply(
        &self,
        transactions: &[TransactionRecord],
        today: NaiveDate,
        bounds: Option<(NaiveDate, NaiveDate)>,
    ) -> Vec<usize> {
        let range = self.range.range(today, bounds);
        let needle = self.search.trim().to_lowercase();
        transactions
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                t.income == self.side.is_income()
                    && range.contains(t.posted_date)
                    && self.category.as_ref().is_none_or(|c| &t.base_category == c)
                    && self.account.as_ref().is_none_or(|a| &t.account == a)
                    && (needle.is_empty()
                        || [
                            Some(t.name.as_str()),
                            t.memo.as_deref(),
                            Some(t.category.as_str()),
                            Some(t.account.as_str()),
                        ]
                        .into_iter()
                        .flatten()
                        .any(|field| field.to_lowercase().contains(&needle)))
            })
            .map(|(idx, _)| idx)
            .collect()
    }
}

/// State of the transactions view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionsView {
    /// Current filters.
    pub filter: TransactionFilter,
    /// Sorted column.
    pub sort: SortColumn,
    /// Sort direction.
    pub ascending: bool,
}

impl TransactionsView {
    /// A view showing the last 12 months of expenses, newest first.
    #[must_use]
    pub fn new(today: NaiveDate) -> Self {
        Self {
            filter: TransactionFilter {
                side: Side::Expenses,
                range: RangeSelector::new(RangePreset::Last12Months, today),
                search: String::new(),
                category: None,
                account: None,
            },
            sort: SortColumn::Date,
            ascending: false,
        }
    }

    /// Filtered and sorted row indices.
    #[must_use]
    pub fn rows(&self, data: &Dataset, today: NaiveDate) -> Vec<usize> {
        let mut rows = self
            .filter
            .apply(&data.transactions, today, data.date_bounds());
        rows.sort_by(|&a, &b| {
            let ord = self
                .sort
                .compare(&data.transactions[a], &data.transactions[b]);
            if self.ascending { ord } else { ord.reverse() }
        });
        rows
    }

    /// Draw the filter bar.
    fn filter_bar(&mut self, ui: &mut Ui, data: &Dataset, today: NaiveDate) {
        ui.horizontal_wrapped(|ui| {
            for &side in Side::VARIANTS {
                let label: &'static str = side.into();
                if ui
                    .selectable_label(self.filter.side == side, RichText::new(label).strong())
                    .clicked()
                {
                    self.filter.side = side;
                    self.filter.category = None;
                }
            }
            ui.separator();
            self.filter
                .range
                .ui(ui, "transactions-range", today, data.date_bounds());
            ui.separator();

            let income = self.filter.side.is_income();
            let mut categories: Vec<&str> = data
                .transactions
                .iter()
                .filter(|t| t.income == income)
                .map(|t| t.base_category.as_str())
                .collect();
            categories.sort_unstable();
            categories.dedup();
            option_combo(
                ui,
                "Category",
                "transactions-category",
                &mut self.filter.category,
                &categories,
            );

            let mut accounts: Vec<&str> = data
                .transactions
                .iter()
                .map(|t| t.account.as_str())
                .collect();
            accounts.sort_unstable();
            accounts.dedup();
            option_combo(
                ui,
                "Account",
                "transactions-account",
                &mut self.filter.account,
                &accounts,
            );

            ui.separator();
            let label = ui.label("Search:");
            ui.add(
                egui::TextEdit::singleline(&mut self.filter.search)
                    .hint_text("name, memo, category…")
                    .desired_width(180.0),
            )
            .labelled_by(label.id);
            if !self.filter.search.is_empty() && ui.small_button("Clear").clicked() {
                self.filter.search.clear();
            }
        });
    }

    /// Draw the view.
    pub fn ui(&mut self, ui: &mut Ui, data: &Dataset, today: NaiveDate) {
        self.filter_bar(ui, data, today);
        ui.add_space(4.0);

        let rows = self.rows(data, today);
        let summary = totals(rows.iter().map(|&i| &data.transactions[i]));
        let range = self.filter.range.range(today, data.date_bounds());
        // Same definition as the chart: the number of calendar months the range touches.
        #[allow(clippy::cast_precision_loss)]
        let months =
            crate::analysis::period_count(range, crate::analysis::Period::Month).max(1) as f64;
        let total = if self.filter.side.is_income() {
            summary.income
        } else {
            summary.expenses
        };
        ui.horizontal_wrapped(|ui| {
            stat_tile(ui, "Transactions", &summary.count.to_string());
            stat_tile(ui, "Total", &money(total));
            stat_tile(
                ui,
                "Monthly average",
                &money(
                    Decimal::try_from(crate::analysis::to_f64(total) / months).unwrap_or_default(),
                ),
            );
            let largest = rows
                .iter()
                .map(|&i| &data.transactions[i])
                .max_by_key(|t| if t.income { t.amount } else { -t.amount });
            if let Some(t) = largest {
                stat_tile(
                    ui,
                    "Largest",
                    &format!("{} · {}", money(t.amount.abs()), t.name),
                );
            }
        });
        ui.add_space(4.0);
        self.table(ui, data, &rows);
    }

    /// A sortable column header.
    fn header(&mut self, ui: &mut Ui, column: SortColumn) {
        let name: &'static str = column.into();
        let arrow = match (self.sort == column, self.ascending) {
            (false, _) => "",
            (true, true) => " ⏶",
            (true, false) => " ⏷",
        };
        if ui
            .add(egui::Button::new(RichText::new(format!("{name}{arrow}")).strong()).frame(false))
            .clicked()
        {
            if self.sort == column {
                self.ascending = !self.ascending;
            } else {
                self.sort = column;
                self.ascending = matches!(
                    column,
                    SortColumn::Account | SortColumn::Category | SortColumn::Name
                );
            }
        }
    }

    /// Draw the table of `rows`.
    fn table(&mut self, ui: &mut Ui, data: &Dataset, rows: &[usize]) {
        if rows.is_empty() {
            ui.centered_and_justified(|ui| ui.weak("No transactions match the current filters."));
            return;
        }
        let row_height = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
        TableBuilder::new(ui)
            .id_salt("transactions-table")
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(90.0))
            .column(Column::initial(110.0).at_least(60.0))
            .column(Column::initial(170.0).at_least(80.0))
            .column(Column::initial(260.0).at_least(80.0))
            .column(Column::initial(200.0).at_least(60.0).clip(true))
            .column(Column::remainder().at_least(100.0))
            .header(row_height + 4.0, |mut header| {
                header.col(|ui| self.header(ui, SortColumn::Date));
                header.col(|ui| self.header(ui, SortColumn::Account));
                header.col(|ui| self.header(ui, SortColumn::Category));
                header.col(|ui| self.header(ui, SortColumn::Name));
                header.col(|ui| {
                    ui.strong("Memo");
                });
                header.col(|ui| self.header(ui, SortColumn::Amount));
            })
            .body(|body| {
                body.rows(row_height, rows.len(), |mut row| {
                    let t = &data.transactions[rows[row.index()]];
                    row.col(|ui| {
                        ui.label(t.posted_date.to_string());
                    });
                    row.col(|ui| {
                        ui.label(&t.account);
                    });
                    row.col(|ui| {
                        ui.label(&t.category)
                            .on_hover_text(format!("Type: {}", t.transaction_type));
                    });
                    row.col(|ui| {
                        ui.label(&t.name);
                    });
                    row.col(|ui| {
                        ui.weak(t.memo.as_deref().unwrap_or(""));
                    });
                    row.col(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            amount_label(ui, t.amount);
                        });
                    });
                });
            });
    }
}

/// A combo box choosing one of `options` or "All".
pub fn option_combo(
    ui: &mut Ui,
    label: &str,
    id: &str,
    value: &mut Option<String>,
    options: &[&str],
) {
    let label = ui.label(format!("{label}:"));
    egui::ComboBox::from_id_salt(id)
        .selected_text(value.as_deref().unwrap_or("All"))
        .show_ui(ui, |ui| {
            ui.selectable_value(value, None, "All");
            for &option in options {
                ui.selectable_value(value, Some(option.to_string()), option);
            }
        })
        .response
        .labelled_by(label.id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::DateRange;

    fn d(m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, m, day).unwrap()
    }

    fn data() -> Dataset {
        let tx =
            |id, date, cents, cat: &str, income, name: &str, account: &str| TransactionRecord {
                id,
                account: account.into(),
                base_category: cat.split('.').next().unwrap().into(),
                category: cat.into(),
                source_category: None,
                income,
                transaction_type: "DebitPurchase".into(),
                posted_date: date,
                amount: Decimal::new(cents, 2),
                transaction_id: None,
                name: name.into(),
                memo: Some("memo text".into()),
            };
        Dataset {
            transactions: vec![
                tx(1, d(1, 1), -500, "Food.Coffee", false, "CAFE", "Visa"),
                tx(
                    2,
                    d(2, 1),
                    -9000,
                    "Food.Groceries",
                    false,
                    "MART",
                    "Chequing",
                ),
                tx(3, d(3, 1), -1500, "Rent", false, "LANDLORD", "Chequing"),
                tx(4, d(3, 1), 100_000, "Salary", true, "ACME", "Chequing"),
            ],
            uncategorized: vec![],
        }
    }

    fn view() -> TransactionsView {
        let mut view = TransactionsView::new(d(6, 1));
        view.filter.range.preset = RangePreset::AllTime;
        view
    }

    #[test]
    fn filters_by_side_category_account_and_search() {
        let data = data();
        let today = d(6, 1);
        let mut view = view();
        assert_eq!(view.rows(&data, today), vec![2, 1, 0], "newest first");

        view.filter.side = Side::Income;
        assert_eq!(view.rows(&data, today), vec![3]);

        view.filter.side = Side::Expenses;
        view.filter.category = Some("Food".into());
        assert_eq!(view.rows(&data, today).len(), 2);
        view.filter.account = Some("Visa".into());
        assert_eq!(view.rows(&data, today), vec![0]);

        view.filter.category = None;
        view.filter.account = None;
        view.filter.search = "  mart ".into();
        assert_eq!(view.rows(&data, today), vec![1]);
        view.filter.search = "MEMO".into();
        assert_eq!(view.rows(&data, today).len(), 3);
    }

    #[test]
    fn filters_by_range() {
        let data = data();
        let mut view = view();
        view.filter.range.preset = RangePreset::Custom;
        view.filter.range.custom = DateRange {
            start: d(1, 15),
            end: d(2, 15),
        };
        assert_eq!(view.rows(&data, d(6, 1)), vec![1]);
    }

    #[test]
    fn sorts_by_columns() {
        let data = data();
        let today = d(6, 1);
        let mut view = view();
        view.sort = SortColumn::Amount;
        view.ascending = true;
        assert_eq!(view.rows(&data, today), vec![1, 2, 0]);
        view.sort = SortColumn::Name;
        assert_eq!(view.rows(&data, today), vec![0, 2, 1]);
        view.sort = SortColumn::Account;
        assert_eq!(view.rows(&data, today), vec![1, 2, 0]);
        view.sort = SortColumn::Category;
        view.ascending = false;
        assert_eq!(view.rows(&data, today), vec![2, 1, 0]);
    }
}
