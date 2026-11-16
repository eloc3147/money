//! Shared formatting and colors for the GUI.

use std::collections::HashMap;

use eframe::egui::Color32;
use rust_decimal::Decimal;

/// Categorical series colors for light mode, in fixed order (validated for colour-vision
/// deficiency separation between adjacent slots).
const LIGHT_SERIES: [Color32; 8] = [
    Color32::from_rgb(0x2a, 0x78, 0xd6),
    Color32::from_rgb(0xeb, 0x68, 0x34),
    Color32::from_rgb(0x1b, 0xaf, 0x7a),
    Color32::from_rgb(0xed, 0xa1, 0x00),
    Color32::from_rgb(0xe8, 0x7b, 0xa4),
    Color32::from_rgb(0x00, 0x83, 0x00),
    Color32::from_rgb(0x62, 0x50, 0xd6),
    Color32::from_rgb(0xe3, 0x49, 0x48),
];

/// The same hues stepped for a dark surface.
const DARK_SERIES: [Color32; 8] = [
    Color32::from_rgb(0x39, 0x87, 0xe5),
    Color32::from_rgb(0xd9, 0x59, 0x26),
    Color32::from_rgb(0x19, 0x9e, 0x70),
    Color32::from_rgb(0xc9, 0x85, 0x00),
    Color32::from_rgb(0xd5, 0x51, 0x81),
    Color32::from_rgb(0x00, 0x83, 0x00),
    Color32::from_rgb(0x90, 0x85, 0xe9),
    Color32::from_rgb(0xe6, 0x67, 0x67),
];

/// Neutral color for the folded "Other" series.
const OTHER: Color32 = Color32::from_rgb(0x8a, 0x89, 0x84);

/// Name of the series that collects categories beyond the palette size.
pub const OTHER_NAME: &str = "Other";

/// Maps category names to stable palette slots.
///
/// Slots are assigned once per dataset (by overall size), so a category keeps its color when
/// the selected range changes. Categories beyond the palette fold into [`OTHER_NAME`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CategoryColors {
    /// Slot index per category.
    slots: HashMap<String, usize>,
}

impl CategoryColors {
    /// Number of distinct series colors.
    pub const SLOTS: usize = LIGHT_SERIES.len();

    /// Assign slots to `names`, which must be ordered largest first.
    #[must_use]
    pub fn new<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        let slots = names
            .into_iter()
            .take(Self::SLOTS)
            .enumerate()
            .map(|(idx, name)| (name.to_string(), idx))
            .collect();
        Self { slots }
    }

    /// Whether `name` has its own slot (otherwise it belongs to "Other").
    #[must_use]
    pub fn has_slot(&self, name: &str) -> bool {
        self.slots.contains_key(name)
    }

    /// The color for `name` in the given theme.
    #[must_use]
    pub fn color(&self, name: &str, dark_mode: bool) -> Color32 {
        let palette = if dark_mode { DARK_SERIES } else { LIGHT_SERIES };
        self.slots.get(name).map_or(OTHER, |&idx| palette[idx])
    }
}

/// Format an amount as currency with thousands separators, e.g. `-$1,234.50`.
#[must_use]
pub fn money(amount: Decimal) -> String {
    let rounded = amount.round_dp(2);
    let sign = if rounded.is_sign_negative() && !rounded.is_zero() {
        "-"
    } else {
        ""
    };
    let text = format!("{:.2}", rounded.abs());
    let (whole, frac) = text.split_once('.').unwrap_or((&text, "00"));
    let mut grouped = String::new();
    for (idx, ch) in whole.chars().enumerate() {
        if idx > 0 && (whole.len() - idx).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{sign}${grouped}.{frac}")
}

/// Format an `f64` amount as currency.
#[must_use]
pub fn money_f64(amount: f64) -> String {
    money(Decimal::try_from(amount).unwrap_or_default())
}

/// Format an `f64` amount as whole dollars, e.g. `$4,000` (for axis labels).
#[must_use]
pub fn money_whole(amount: f64) -> String {
    let text = money(Decimal::try_from(amount.round()).unwrap_or_default());
    text.strip_suffix(".00")
        .map_or(text.clone(), str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_money() {
        assert_eq!(money(Decimal::new(123_456_789, 2)), "$1,234,567.89");
        assert_eq!(money(Decimal::new(-50, 2)), "-$0.50");
        assert_eq!(money(Decimal::new(100_000, 2)), "$1,000.00");
        assert_eq!(money(Decimal::new(-1, 3)), "$0.00");
        assert_eq!(money(Decimal::new(99_999, 2)), "$999.99");
        assert_eq!(money_f64(12.5), "$12.50");
        assert_eq!(money_whole(4000.4), "$4,000");
        assert_eq!(money_whole(-1234.6), "-$1,235");
    }

    #[test]
    fn category_colors_are_stable_and_fold() {
        let names: Vec<String> = (0..10).map(|i| format!("C{i}")).collect();
        let colors = CategoryColors::new(names.iter().map(String::as_str));
        assert!(colors.has_slot("C0"));
        assert!(colors.has_slot("C7"));
        assert!(!colors.has_slot("C8"));
        assert_eq!(colors.color("C0", false), LIGHT_SERIES[0]);
        assert_eq!(colors.color("C1", true), DARK_SERIES[1]);
        assert_eq!(colors.color("C9", false), OTHER);
    }
}
