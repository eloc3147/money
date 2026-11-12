//! A date range selector: preset drop-down plus date pickers for custom ranges.

use chrono::{Datelike, NaiveDate};
use eframe::egui::{self, Ui};
use egui_extras::DatePickerButton;
use strum::VariantArray;

use crate::analysis::{DateRange, RangePreset};

/// Convert a chrono date to the date type used by the date picker.
#[must_use]
pub fn to_jiff(date: NaiveDate) -> jiff::civil::Date {
    // Day and month always fit in i8, and chrono years fit jiff's supported range in practice.
    jiff::civil::Date::new(
        i16::try_from(date.year()).unwrap_or(i16::MAX),
        i8::try_from(date.month()).unwrap_or(1),
        i8::try_from(date.day()).unwrap_or(1),
    )
    .unwrap_or_default()
}

/// Convert a date picker date to chrono.
#[must_use]
pub fn from_jiff(date: jiff::civil::Date) -> NaiveDate {
    NaiveDate::from_ymd_opt(
        i32::from(date.year()),
        u32::try_from(date.month()).unwrap_or(1),
        u32::try_from(date.day()).unwrap_or(1),
    )
    .unwrap_or_default()
}

/// State of a range selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeSelector {
    /// Selected preset.
    pub preset: RangePreset,
    /// Range used for [`RangePreset::Custom`].
    pub custom: DateRange,
}

impl RangeSelector {
    /// A selector defaulting to `preset`, with the custom range initialised to `today`.
    #[must_use]
    pub fn new(preset: RangePreset, today: NaiveDate) -> Self {
        Self {
            preset,
            custom: DateRange {
                start: today,
                end: today,
            },
        }
    }

    /// The selected range.
    #[must_use]
    pub fn range(&self, today: NaiveDate, bounds: Option<(NaiveDate, NaiveDate)>) -> DateRange {
        self.preset.resolve(today, bounds, self.custom)
    }

    /// Show the selector. `id` must be unique per selector on screen.
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        id: &str,
        today: NaiveDate,
        bounds: Option<(NaiveDate, NaiveDate)>,
    ) {
        let label = ui.label("Range:");
        let before = self.preset;
        egui::ComboBox::from_id_salt((id, "preset"))
            .selected_text(self.preset.label())
            .show_ui(ui, |ui| {
                for &preset in RangePreset::VARIANTS {
                    ui.selectable_value(&mut self.preset, preset, preset.label());
                }
            })
            .response
            .labelled_by(label.id);
        if self.preset == RangePreset::Custom && before != RangePreset::Custom {
            // Start the custom range from whatever was shown before.
            self.custom = before.resolve(today, bounds, self.custom);
        }
        if self.preset == RangePreset::Custom {
            let mut start = to_jiff(self.custom.start);
            let mut end = to_jiff(self.custom.end);
            ui.add(DatePickerButton::new(&mut start).id_salt(&format!("{id}-start")));
            ui.label("to");
            ui.add(DatePickerButton::new(&mut end).id_salt(&format!("{id}-end")));
            self.custom.start = from_jiff(start);
            self.custom.end = from_jiff(end).max(self.custom.start);
        } else {
            let range = self.range(today, bounds);
            ui.weak(format!("{} – {}", range.start, range.end));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_dates() {
        let date = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
        assert_eq!(from_jiff(to_jiff(date)), date);
    }

    #[test]
    fn selector_resolves() {
        let today = NaiveDate::from_ymd_opt(2024, 5, 10).unwrap();
        let selector = RangeSelector::new(RangePreset::YearToDate, today);
        assert_eq!(
            selector.range(today, None).start,
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()
        );
    }
}
