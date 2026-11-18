//! Stacked bar chart of spending (or income) by category over time.

use chrono::NaiveDate;
use eframe::egui::{self, RichText, Stroke, Ui};
use egui_extras::{Column, TableBuilder};
use egui_plot::{Bar, BarChart, GridMark, Plot};
use strum::VariantArray;

use super::range::RangeSelector;
use super::style::{CategoryColors, OTHER_NAME, money_f64, money_whole};
use super::theme::{Palette, medium, regular};
use super::transactions::Side;
use super::widgets::{
    GAP, Stat, caption, card, field, page_header, section_title, segmented, share_bar, stat_row,
};
use crate::analysis::{
    Averaging, BinnedSeries, CategoryLevel, DateRange, Period, RangePreset, bin_by_category,
};
use crate::config::ChartPalette;
use crate::model::Dataset;

/// State of the chart view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartView {
    /// Expenses or income.
    pub side: Side,
    /// Date range.
    pub range: RangeSelector,
    /// Bin size.
    pub period: Period,
    /// Category grouping level.
    pub level: CategoryLevel,
    /// Smoothing.
    pub averaging: Averaging,
    /// Moving average window, in periods.
    pub window: usize,
}

impl ChartView {
    /// Monthly expenses for the last 12 months.
    #[must_use]
    pub fn new(today: NaiveDate) -> Self {
        Self {
            side: Side::Expenses,
            range: RangeSelector::new(RangePreset::Last12Months, today),
            period: Period::Month,
            level: CategoryLevel::Base,
            averaging: Averaging::None,
            window: 3,
        }
    }

    /// Stable colors for the current side and level, ranked over the whole dataset.
    #[must_use]
    pub fn colors(&self, data: &Dataset, palette: ChartPalette) -> CategoryColors {
        category_colors(data, self.side.is_income(), self.level, palette)
    }

    /// The series to plot, after folding and averaging.
    #[must_use]
    pub fn series(
        &self,
        data: &Dataset,
        today: NaiveDate,
        colors: &CategoryColors,
    ) -> BinnedSeries {
        let range = self.range.range(today, data.date_bounds());
        let mut binned = bin_by_category(
            &data.transactions,
            self.side.is_income(),
            range,
            self.period,
            self.level,
        );
        binned.fold_into_other(|name| colors.has_slot(name), OTHER_NAME);
        binned.apply_averaging(self.averaging, self.window);
        binned
    }

    /// Draw the controls toolbar.
    fn controls(&mut self, ui: &mut Ui, data: &Dataset, today: NaiveDate) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            self.range.ui(ui, "chart-range", today, data.date_bounds());

            field(ui, "Group by", |ui, _| {
                let options: Vec<(Period, &str)> =
                    Period::VARIANTS.iter().map(|&p| (p, p.label())).collect();
                segmented(ui, &mut self.period, &options);
            });

            field(ui, "Categories", |ui, label| {
                egui::ComboBox::from_id_salt("chart-level")
                    .selected_text(self.level.label())
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        for &level in CategoryLevel::VARIANTS {
                            ui.selectable_value(&mut self.level, level, level.label());
                        }
                    })
                    .response
                    .labelled_by(label);
            });

            field(ui, "Averaging", |ui, label| {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("chart-averaging")
                        .selected_text(self.averaging.label())
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            for &averaging in Averaging::VARIANTS {
                                ui.selectable_value(
                                    &mut self.averaging,
                                    averaging,
                                    averaging.label(),
                                );
                            }
                        })
                        .response
                        .labelled_by(label);
                    if self.averaging == Averaging::Moving {
                        ui.add(
                            egui::DragValue::new(&mut self.window)
                                .range(2..=24)
                                .suffix(" periods"),
                        )
                        .labelled_by(label);
                    }
                });
            });
        });
        ui.add_space(GAP);
    }

    /// Draw the view.
    pub fn ui(&mut self, ui: &mut Ui, data: &Dataset, today: NaiveDate, palette: ChartPalette) {
        let title = if self.side.is_income() {
            "Income by category"
        } else {
            "Spending by category"
        };
        let subtitle = self.range.describe(today, data.date_bounds());
        let mut side = self.side;
        page_header(ui, title, Some(&subtitle), |ui| {
            let options: Vec<(Side, &str)> =
                Side::VARIANTS.iter().map(|&s| (s, s.into())).collect();
            segmented(ui, &mut side, &options);
        });
        self.side = side;
        self.controls(ui, data, today);

        let colors = self.colors(data, palette);
        let binned = self.series(data, today, &colors);
        let totals = binned.period_totals();
        let grand_total: f64 = binned.series.iter().map(|s| s.total).sum();
        #[allow(clippy::cast_precision_loss)]
        let per_period = grand_total / binned.periods.len().max(1) as f64;
        let period_name = self.period.label().to_lowercase();

        egui::ScrollArea::vertical()
            .id_salt("chart-scroll")
            .auto_shrink(false)
            .show(ui, |ui| {
                let mut stats = vec![
                    Stat::new("Total", money_f64(grand_total))
                        .detail(format!("{} {period_name}s", binned.periods.len())),
                    Stat::new(format!("Average per {period_name}"), money_f64(per_period)),
                ];
                if let Some((idx, max)) =
                    totals.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))
                {
                    stats.push(
                        Stat::new(format!("Highest {period_name}"), money_f64(*max))
                            .detail(self.period.format(binned.periods[idx])),
                    );
                }
                if let Some(top) = binned.series.first() {
                    stats.push(
                        Stat::new("Top category", money_f64(top.total)).detail(top.name.clone()),
                    );
                }
                stat_row(ui, &stats);

                card(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        section_title(
                            ui,
                            &format!(
                                "{} by {period_name}",
                                if self.side.is_income() {
                                    "Income"
                                } else {
                                    "Spending"
                                }
                            ),
                        );
                        ui.add_space(GAP);
                        legend(ui, &binned, &colors);
                    });
                    ui.add_space(8.0);
                    self.plot(ui, &binned, &colors, 340.0);
                });
                ui.add_space(GAP);
                card(ui, |ui| {
                    section_title(ui, "Breakdown");
                    ui.add_space(4.0);
                    Self::breakdown_table(ui, &binned, &colors, grand_total);
                });
            });
    }

    /// Draw the stacked bars.
    fn plot(&self, ui: &mut Ui, binned: &BinnedSeries, colors: &CategoryColors, height: f32) {
        let dark = ui.visuals().dark_mode;
        let surface = Palette::of(ui).surface;
        let period = binned.period;
        let periods = binned.periods.clone();
        let label_periods = periods.clone();
        // Space x labels so they never overlap (about 84px per label).
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let max_labels = ((ui.available_width() - 72.0) / 84.0).floor().max(1.0) as usize;
        let step = periods.len().div_ceil(max_labels).max(1);
        // A little headroom above the tallest stack.
        let max_total = binned.period_totals().into_iter().fold(0.0, f64::max);

        let mut charts: Vec<BarChart> = Vec::new();
        for series in &binned.series {
            let color = colors.color(&series.name, dark);
            let bars = series
                .values
                .iter()
                .enumerate()
                .map(|(idx, &value)| {
                    #[allow(clippy::cast_precision_loss)]
                    Bar::new(idx as f64, value)
                        .name(period.format(periods[idx]))
                        .width(0.62)
                        .fill(color)
                        // A gap in the surface color separates stacked segments.
                        .stroke(Stroke::new(1.5, surface))
                })
                .collect();
            let name = series.name.clone();
            let mut chart = BarChart::new(series.name.clone(), bars)
                .color(color)
                .element_formatter(Box::new(move |bar, _| {
                    format!("{name}\n{}\n{}", bar.name, money_f64(bar.value))
                }));
            let below: Vec<&BarChart> = charts.iter().collect();
            chart = chart.stack_on(&below);
            charts.push(chart);
        }

        Plot::new("category-chart")
            .height(height)
            .show_background(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .show_grid([false, true])
            .include_y(0.0)
            .include_y(max_total * 1.08)
            .grid_spacing(40.0..=240.0)
            .y_axis_min_width(72.0)
            .y_axis_formatter(|mark, _| money_whole(mark.value))
            .x_grid_spacer(move |input| {
                let lo = input.bounds.0.ceil().max(0.0);
                let hi = input.bounds.1.floor();
                let mut marks = Vec::new();
                let mut x = lo;
                while x <= hi {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    if (x as usize).is_multiple_of(step) {
                        marks.push(GridMark {
                            value: x,
                            #[allow(clippy::cast_precision_loss)]
                            step_size: step as f64,
                        });
                    }
                    x += 1.0;
                }
                marks
            })
            .x_axis_formatter(move |mark, _| {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let idx = mark.value.round() as usize;
                label_periods
                    .get(idx)
                    .filter(|_| (mark.value - mark.value.round()).abs() < 1e-6)
                    .map(|p| period.format(*p))
                    .unwrap_or_default()
            })
            .label_formatter(|_| None)
            .show(ui, |plot| {
                for chart in charts {
                    plot.bar_chart(chart);
                }
            });
    }

    /// A table of per-category totals, so values are readable without hovering.
    fn breakdown_table(ui: &mut Ui, binned: &BinnedSeries, colors: &CategoryColors, total: f64) {
        let dark = ui.visuals().dark_mode;
        let p = Palette::of(ui);
        #[allow(clippy::cast_precision_loss)]
        let periods = binned.periods.len().max(1) as f64;
        let header = |ui: &mut Ui, text: &str| {
            ui.label(caption(ui, text));
        };
        let right = egui::Layout::right_to_left(egui::Align::Center);
        TableBuilder::new(ui)
            .id_salt("chart-breakdown")
            .striped(false)
            .vscroll(false)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::initial(240.0).at_least(160.0).clip(true))
            .column(Column::initial(130.0))
            .column(Column::initial(130.0))
            .column(Column::remainder().at_least(200.0))
            .header(28.0, |mut row| {
                row.col(|ui| header(ui, "Category"));
                row.col(|ui| {
                    ui.with_layout(right, |ui| header(ui, "Total"));
                });
                row.col(|ui| {
                    ui.with_layout(right, |ui| {
                        header(ui, &format!("Per {}", binned.period.label().to_lowercase()));
                    });
                });
                row.col(|ui| {
                    ui.add_space(24.0);
                    header(ui, "Share");
                });
            })
            .body(|mut body| {
                for series in &binned.series {
                    body.row(40.0, |mut row| {
                        let color = colors.color(&series.name, dark);
                        row.col(|ui| {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 5.0, color);
                            ui.label(RichText::new(&series.name).font(medium(14.0)).color(p.text));
                        });
                        row.col(|ui| {
                            ui.with_layout(right, |ui| {
                                ui.label(
                                    RichText::new(money_f64(series.total))
                                        .font(medium(14.0))
                                        .color(p.text),
                                );
                            });
                        });
                        row.col(|ui| {
                            ui.with_layout(right, |ui| {
                                ui.label(
                                    RichText::new(money_f64(series.total / periods))
                                        .font(regular(14.0))
                                        .color(p.muted),
                                );
                            });
                        });
                        row.col(|ui| {
                            #[allow(clippy::cast_possible_truncation)]
                            let share = if total.abs() > f64::EPSILON {
                                (series.total / total) as f32
                            } else {
                                0.0
                            };
                            ui.add_space(24.0);
                            share_bar(ui, share, color, 180.0);
                            ui.label(
                                RichText::new(format!("{:.1}%", share * 100.0))
                                    .font(regular(13.0))
                                    .color(p.muted),
                            );
                        });
                    });
                }
            });
    }
}

/// A wrapping legend of color dots and category names.
fn legend(ui: &mut Ui, binned: &BinnedSeries, colors: &CategoryColors) {
    let dark = ui.visuals().dark_mode;
    let p = Palette::of(ui);
    for series in &binned.series {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 5.0, colors.color(&series.name, dark));
            ui.label(
                RichText::new(&series.name)
                    .font(regular(13.0))
                    .color(p.muted),
            );
        });
    }
}

/// Colors for the categories of one side at one level, ranked by size over the whole dataset
/// so a category keeps its color whatever range or view shows it.
#[must_use]
pub fn category_colors(
    data: &Dataset,
    income: bool,
    level: CategoryLevel,
    palette: ChartPalette,
) -> CategoryColors {
    let all = data.date_bounds().map_or(
        BinnedSeries {
            period: Period::Year,
            periods: Vec::new(),
            series: Vec::new(),
        },
        |(start, end)| {
            bin_by_category(
                &data.transactions,
                income,
                DateRange { start, end },
                Period::Year,
                level,
            )
        },
    );
    CategoryColors::new(all.names(), palette)
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;
    use crate::model::TransactionRecord;

    fn data(categories: usize) -> Dataset {
        let transactions = (0..categories)
            .map(|i| TransactionRecord {
                id: i64::try_from(i).unwrap(),
                account: "A".into(),
                base_category: format!("C{i:02}"),
                category: format!("C{i:02}.x"),
                source_category: None,
                income: false,
                transaction_type: "DebitPurchase".into(),
                posted_date: NaiveDate::from_ymd_opt(2024, 1, 1 + u32::try_from(i).unwrap())
                    .unwrap(),
                amount: Decimal::new(-1000 * (i64::try_from(i).unwrap() + 1), 2),
                transaction_id: None,
                name: "N".into(),
                memo: None,
            })
            .collect();
        Dataset {
            transactions,
            uncategorized: vec![],
        }
    }

    #[test]
    fn folds_beyond_palette_and_keeps_colors() {
        let data = data(CategoryColors::SLOTS + 2);
        let today = NaiveDate::from_ymd_opt(2024, 2, 1).unwrap();
        let mut view = ChartView::new(today);
        view.range.preset = RangePreset::AllTime;
        let colors = view.colors(&data, ChartPalette::default());
        let series = view.series(&data, today, &colors);
        assert_eq!(series.series.len(), CategoryColors::SLOTS + 1);
        assert_eq!(series.series.last().unwrap().name, OTHER_NAME);
        assert_eq!(series.series[0].name, "C21", "largest first");

        // Narrowing the range must not recolor the remaining categories.
        let before = colors.color("C05", false);
        view.range.preset = RangePreset::Custom;
        view.range.custom = DateRange {
            start: NaiveDate::from_ymd_opt(2024, 1, 6).unwrap(),
            end: NaiveDate::from_ymd_opt(2024, 1, 6).unwrap(),
        };
        assert_eq!(
            view.colors(&data, ChartPalette::default())
                .color("C05", false),
            before
        );
        assert_eq!(view.series(&data, today, &colors).names(), vec!["C05"]);
    }

    #[test]
    fn income_side_and_full_level() {
        let mut data = data(2);
        data.transactions[0].income = true;
        let today = NaiveDate::from_ymd_opt(2024, 2, 1).unwrap();
        let mut view = ChartView::new(today);
        view.side = Side::Income;
        view.level = CategoryLevel::Full;
        view.range.preset = RangePreset::AllTime;
        let colors = view.colors(&data, ChartPalette::default());
        assert_eq!(view.series(&data, today, &colors).names(), vec!["C00.x"]);
        assert!(
            ChartView::new(today).colors(&Dataset::default(), ChartPalette::default())
                == CategoryColors::default()
        );
    }
}
