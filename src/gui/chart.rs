//! Stacked bar chart of spending (or income) by category over time.

use chrono::NaiveDate;
use eframe::egui::{self, RichText, Stroke, Ui};
use egui_extras::{Column, TableBuilder};
use egui_plot::{Bar, BarChart, GridMark, Legend, Plot};
use strum::VariantArray;

use super::range::RangeSelector;
use super::style::{CategoryColors, OTHER_NAME, money_f64, stat_tile};
use super::transactions::Side;
use crate::analysis::{
    Averaging, BinnedSeries, CategoryLevel, DateRange, Period, RangePreset, bin_by_category,
};
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
    pub fn colors(&self, data: &Dataset) -> CategoryColors {
        let all = data.date_bounds().map_or(
            BinnedSeries {
                period: Period::Year,
                periods: Vec::new(),
                series: Vec::new(),
            },
            |(start, end)| {
                bin_by_category(
                    &data.transactions,
                    self.side.is_income(),
                    DateRange { start, end },
                    Period::Year,
                    self.level,
                )
            },
        );
        CategoryColors::new(all.names())
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

    /// Draw the controls.
    fn controls(&mut self, ui: &mut Ui, data: &Dataset, today: NaiveDate) {
        ui.horizontal_wrapped(|ui| {
            for &side in Side::VARIANTS {
                let label: &'static str = side.into();
                if ui
                    .selectable_label(self.side == side, RichText::new(label).strong())
                    .clicked()
                {
                    self.side = side;
                }
            }
            ui.separator();
            self.range.ui(ui, "chart-range", today, data.date_bounds());
            ui.separator();

            let group_label = ui.label("Group by:");
            egui::ComboBox::from_id_salt("chart-period")
                .selected_text(self.period.label())
                .show_ui(ui, |ui| {
                    for &period in Period::VARIANTS {
                        ui.selectable_value(&mut self.period, period, period.label());
                    }
                })
                .response
                .labelled_by(group_label.id);
            egui::ComboBox::from_id_salt("chart-level")
                .selected_text(self.level.label())
                .show_ui(ui, |ui| {
                    for &level in CategoryLevel::VARIANTS {
                        ui.selectable_value(&mut self.level, level, level.label());
                    }
                })
                .response
                .labelled_by(group_label.id);
            ui.separator();

            let label = ui.label("Averaging:");
            egui::ComboBox::from_id_salt("chart-averaging")
                .selected_text(self.averaging.label())
                .show_ui(ui, |ui| {
                    for &averaging in Averaging::VARIANTS {
                        ui.selectable_value(&mut self.averaging, averaging, averaging.label());
                    }
                })
                .response
                .labelled_by(label.id);
            if self.averaging == Averaging::Moving {
                ui.add(
                    egui::DragValue::new(&mut self.window)
                        .range(2..=24)
                        .suffix(" periods"),
                )
                .labelled_by(label.id);
            }
        });
    }

    /// Draw the view.
    pub fn ui(&mut self, ui: &mut Ui, data: &Dataset, today: NaiveDate) {
        self.controls(ui, data, today);
        ui.add_space(4.0);

        let colors = self.colors(data);
        let binned = self.series(data, today, &colors);
        let totals = binned.period_totals();
        let grand_total: f64 = binned.series.iter().map(|s| s.total).sum();
        #[allow(clippy::cast_precision_loss)]
        let per_period = grand_total / binned.periods.len().max(1) as f64;

        ui.horizontal_wrapped(|ui| {
            stat_tile(ui, "Total", &money_f64(grand_total));
            stat_tile(
                ui,
                &format!("Average per {}", self.period.label().to_lowercase()),
                &money_f64(per_period),
            );
            if let Some((idx, max)) = totals.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)) {
                stat_tile(
                    ui,
                    "Highest period",
                    &format!(
                        "{} · {}",
                        money_f64(*max),
                        self.period.format(binned.periods[idx])
                    ),
                );
            }
            if let Some(top) = binned.series.first() {
                stat_tile(
                    ui,
                    "Top category",
                    &format!("{} · {}", top.name, money_f64(top.total)),
                );
            }
        });
        ui.add_space(4.0);

        let plot_height = (ui.available_height() * 0.62).max(220.0);
        self.plot(ui, &binned, &colors, plot_height);
        ui.add_space(6.0);
        Self::breakdown_table(ui, &binned, &colors, grand_total);
    }

    /// Draw the stacked bars.
    fn plot(&self, ui: &mut Ui, binned: &BinnedSeries, colors: &CategoryColors, height: f32) {
        let dark = ui.visuals().dark_mode;
        let surface = ui.visuals().extreme_bg_color;
        let period = binned.period;
        let periods = binned.periods.clone();
        let label_periods = periods.clone();
        let step = (periods.len() / 12).max(1);
        // Leave headroom above the tallest stack so the legend doesn't cover bars.
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
                        .width(0.72)
                        .fill(color)
                        // A thin ring in the surface color separates stacked segments.
                        .stroke(Stroke::new(1.0, surface))
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
            .legend(Legend::default().follow_insertion_order(true))
            .allow_zoom(false)
            .allow_drag(false)
            .allow_scroll(false)
            .allow_boxed_zoom(false)
            .show_grid([false, true])
            .include_y(0.0)
            .include_y(max_total * 1.3)
            .y_axis_min_width(64.0)
            .y_axis_formatter(|mark, _| money_f64(mark.value))
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
        #[allow(clippy::cast_precision_loss)]
        let periods = binned.periods.len().max(1) as f64;
        let row_height = ui.text_style_height(&egui::TextStyle::Body) + 4.0;
        TableBuilder::new(ui)
            .id_salt("chart-breakdown")
            .striped(true)
            .column(Column::exact(16.0))
            .column(Column::initial(220.0))
            .column(Column::initial(120.0))
            .column(Column::initial(140.0))
            .column(Column::remainder())
            .header(row_height, |mut header| {
                header.col(|_| {});
                header.col(|ui| {
                    ui.strong("Category");
                });
                header.col(|ui| {
                    ui.strong("Total");
                });
                header.col(|ui| {
                    ui.strong(format!("Per {}", binned.period.label().to_lowercase()));
                });
                header.col(|ui| {
                    ui.strong("Share");
                });
            })
            .body(|mut body| {
                for series in &binned.series {
                    body.row(row_height, |mut row| {
                        row.col(|ui| {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            ui.painter()
                                .rect_filled(rect, 3.0, colors.color(&series.name, dark));
                        });
                        row.col(|ui| {
                            ui.label(&series.name);
                        });
                        row.col(|ui| {
                            ui.monospace(money_f64(series.total));
                        });
                        row.col(|ui| {
                            ui.monospace(money_f64(series.total / periods));
                        });
                        row.col(|ui| {
                            let share = if total.abs() > f64::EPSILON {
                                (series.total / total) as f32
                            } else {
                                0.0
                            };
                            ui.add(
                                egui::ProgressBar::new(share.clamp(0.0, 1.0))
                                    .desired_width(160.0)
                                    .text(format!("{:.1}%", share * 100.0)),
                            );
                        });
                    });
                }
            });
    }
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
        let data = data(10);
        let today = NaiveDate::from_ymd_opt(2024, 2, 1).unwrap();
        let mut view = ChartView::new(today);
        view.range.preset = RangePreset::AllTime;
        let colors = view.colors(&data);
        let series = view.series(&data, today, &colors);
        assert_eq!(series.series.len(), CategoryColors::SLOTS + 1);
        assert_eq!(series.series.last().unwrap().name, OTHER_NAME);
        assert_eq!(series.series[0].name, "C09", "largest first");

        // Narrowing the range must not recolor the remaining categories.
        let before = colors.color("C05", false);
        view.range.preset = RangePreset::Custom;
        view.range.custom = DateRange {
            start: NaiveDate::from_ymd_opt(2024, 1, 6).unwrap(),
            end: NaiveDate::from_ymd_opt(2024, 1, 6).unwrap(),
        };
        assert_eq!(view.colors(&data).color("C05", false), before);
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
        let colors = view.colors(&data);
        assert_eq!(view.series(&data, today, &colors).names(), vec!["C00.x"]);
        assert!(ChartView::new(today).colors(&Dataset::default()) == CategoryColors::default());
    }
}
