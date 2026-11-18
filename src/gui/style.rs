//! Shared formatting and colors for the GUI.

use std::collections::HashMap;

use eframe::egui::Color32;
use rust_decimal::Decimal;

use crate::config::ChartPalette;

/// Number of colors in each category palette.
pub const SLOTS: usize = 20;

/// A color from a `0xRRGGBB` literal.
#[allow(clippy::cast_possible_truncation)]
const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// matplotlib `tab20`, reordered so stacked neighbours differ.
const TAB20: [Color32; SLOTS] = [
    rgb(0x17becf),
    rgb(0xdbdb8d),
    rgb(0x2ca02c),
    rgb(0xc49c94),
    rgb(0xd62728),
    rgb(0x98df8a),
    rgb(0xe377c2),
    rgb(0x9edae5),
    rgb(0xff7f0e),
    rgb(0xaec7e8),
    rgb(0x9467bd),
    rgb(0xc7c7c7),
    rgb(0x1f77b4),
    rgb(0xffbb78),
    rgb(0x8c564b),
    rgb(0xff9896),
    rgb(0x7f7f7f),
    rgb(0xf7b6d2),
    rgb(0xbcbd22),
    rgb(0xc5b0d5),
];

/// plotly `Dark24` (first 20), reordered so stacked neighbours differ.
const DARK24: [Color32; SLOTS] = [
    rgb(0x862a16),
    rgb(0x00a08b),
    rgb(0x750d86),
    rgb(0xb2828d),
    rgb(0x620042),
    rgb(0xfb00d1),
    rgb(0x6c7c32),
    rgb(0x222a2a),
    rgb(0xe15f99),
    rgb(0x1616a7),
    rgb(0xeb663b),
    rgb(0xda16ff),
    rgb(0xb68100),
    rgb(0x511cfb),
    rgb(0xfc0080),
    rgb(0x2e91e5),
    rgb(0x1ca71c),
    rgb(0xa777f1),
    rgb(0xfb0d0d),
    rgb(0x778aae),
];

/// colorcet `glasbey_bw_minc_20`, reordered so stacked neighbours differ.
const GLASBEY: [Color32; SLOTS] = [
    rgb(0x00acc7),
    rgb(0x6c004f),
    rgb(0x028800),
    rgb(0xbcb7ff),
    rgb(0x005759),
    rgb(0x00fdcf),
    rgb(0xd70000),
    rgb(0xfef590),
    rgb(0xc004b9),
    rgb(0x95b578),
    rgb(0x790000),
    rgb(0x8c3cff),
    rgb(0x583b00),
    rgb(0xff7fd1),
    rgb(0x0000dd),
    rgb(0xa1756a),
    rgb(0x98ff00),
    rgb(0x0774d8),
    rgb(0xffa530),
    rgb(0x645474),
];

/// Glasbey max-min selection limited to the light theme's lightness and chroma, starting
/// from the app's blue.
const THEME_GLASBEY_LIGHT: [Color32; SLOTS] = [
    rgb(0x2a78d6),
    rgb(0x79df35),
    rgb(0x86016b),
    rgb(0xfe7cfc),
    rgb(0x195d02),
    rgb(0x12d7fb),
    rgb(0xdb1d1d),
    rgb(0xfd9f07),
    rgb(0x4a1fbd),
    rgb(0xab4adc),
    rgb(0xa8ad08),
    rgb(0x015477),
    rgb(0x9a96fe),
    rgb(0x765592),
    rgb(0x0ec193),
    rgb(0xc77007),
    rgb(0x029b1f),
    rgb(0xd92d8e),
    rgb(0x7d3501),
    rgb(0xff6d87),
];

/// Glasbey max-min selection limited to the dark theme's lightness and chroma, starting
/// from the app's blue.
const THEME_GLASBEY_DARK: [Color32; SLOTS] = [
    rgb(0x3987e5),
    rgb(0x7fe63e),
    rgb(0xfc78fa),
    rgb(0x9f12a0),
    rgb(0x07dffd),
    rgb(0xd81618),
    rgb(0xfeb580),
    rgb(0x007916),
    rgb(0x05b56c),
    rgb(0x8b5505),
    rgb(0x8cd19e),
    rgb(0x705491),
    rgb(0xa664fe),
    rgb(0x036f83),
    rgb(0xccbbfe),
    rgb(0xdb6b04),
    rgb(0x5d3fd8),
    rgb(0xfc6983),
    rgb(0xbda000),
    rgb(0xba5586),
];

/// The colors of `palette` for light or dark mode, in slot order.
#[must_use]
pub fn palette_colors(palette: ChartPalette, dark_mode: bool) -> &'static [Color32; SLOTS] {
    match palette {
        ChartPalette::ThemeGlasbey if dark_mode => &THEME_GLASBEY_DARK,
        ChartPalette::ThemeGlasbey => &THEME_GLASBEY_LIGHT,
        ChartPalette::Glasbey => &GLASBEY,
        ChartPalette::Dark24 => &DARK24,
        ChartPalette::Tab20 => &TAB20,
    }
}

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
    /// The palette the slots index into.
    palette: ChartPalette,
}

impl CategoryColors {
    /// Number of distinct series colors.
    pub const SLOTS: usize = SLOTS;

    /// Assign slots of `palette` to `names`, which must be ordered largest first.
    #[must_use]
    pub fn new<'a>(names: impl IntoIterator<Item = &'a str>, palette: ChartPalette) -> Self {
        let slots = names
            .into_iter()
            .take(Self::SLOTS)
            .enumerate()
            .map(|(idx, name)| (name.to_string(), idx))
            .collect();
        Self { slots, palette }
    }

    /// Whether `name` has its own slot (otherwise it belongs to "Other").
    #[must_use]
    pub fn has_slot(&self, name: &str) -> bool {
        self.slots.contains_key(name)
    }

    /// The color for `name` in the given theme.
    #[must_use]
    pub fn color(&self, name: &str, dark_mode: bool) -> Color32 {
        let colors = palette_colors(self.palette, dark_mode);
        self.slots.get(name).map_or(OTHER, |&idx| colors[idx])
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
        let names: Vec<String> = (0..22).map(|i| format!("C{i}")).collect();
        let colors =
            CategoryColors::new(names.iter().map(String::as_str), ChartPalette::ThemeGlasbey);
        assert!(colors.has_slot("C0"));
        assert!(colors.has_slot("C19"));
        assert!(!colors.has_slot("C20"));
        assert_eq!(colors.color("C0", false), THEME_GLASBEY_LIGHT[0]);
        assert_eq!(colors.color("C1", true), THEME_GLASBEY_DARK[1]);
        assert_eq!(colors.color("C21", false), OTHER);

        let tab20 = CategoryColors::new(names.iter().map(String::as_str), ChartPalette::Tab20);
        assert_eq!(
            tab20.color("C2", false),
            tab20.color("C2", true),
            "one set for both themes"
        );
        assert_eq!(tab20.color("C2", false), TAB20[2]);
    }

    #[test]
    fn palettes_have_distinct_slots() {
        use strum::VariantArray;
        for &palette in ChartPalette::VARIANTS {
            for dark in [false, true] {
                let colors = palette_colors(palette, dark);
                for (i, a) in colors.iter().enumerate() {
                    assert!(!colors[i + 1..].contains(a), "{palette:?} repeats {a:?}");
                    assert_ne!(*a, OTHER);
                }
            }
        }
    }
}
