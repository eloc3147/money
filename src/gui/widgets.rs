//! Reusable building blocks drawn in the app's theme: cards, page headers, segmented controls,
//! pill buttons, stat tiles, chips and avatars.

use eframe::egui::{
    self, Color32, CornerRadius, Frame, InnerResponse, Margin, Response, RichText, Sense, Shadow,
    Stroke, Ui, WidgetInfo, accesskit::Role,
};
use rust_decimal::Decimal;

use super::style::money;
use super::theme::{CARD_RADIUS, CONTROL_HEIGHT, Palette, medium, regular, semibold};

/// Space between cards and page sections.
pub const GAP: f32 = 16.0;

/// Space kept free at the right of a scrolling table so the floating scroll bar never covers
/// the last column.
pub const SCROLL_GUTTER: f32 = 12.0;

/// A single-line label that ends in "…" when it does not fit (egui shows the full text on
/// hover).
pub fn truncated(ui: &mut Ui, text: RichText) -> Response {
    ui.add(egui::Label::new(text).truncate())
}

/// The frame of a card: surface fill, hairline border, soft shadow.
pub fn card_frame(ui: &Ui) -> Frame {
    let p = Palette::of(ui);
    Frame::new()
        .fill(p.surface)
        .stroke(Stroke::new(1.0, p.hairline))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin::same(20))
        .shadow(Shadow {
            offset: [0, 1],
            blur: 3,
            spread: 0,
            color: Color32::from_black_alpha(if ui.visuals().dark_mode { 40 } else { 8 }),
        })
}

/// A card as wide as the available space.
pub fn card<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    card_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add_contents(ui)
    })
}

/// A card that fills all the remaining space (for tables and long content).
pub fn card_fill<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    // Margins and border are outside the content rect.
    let inset = 2.0 * 20.0 + 2.0;
    let size = ui.available_size() - egui::vec2(inset, inset);
    card_frame(ui).show(ui, |ui| {
        ui.set_min_size(size.max(egui::Vec2::ZERO));
        ui.set_max_width(size.x.max(0.0));
        add_contents(ui)
    })
}

/// A page title with an optional subtitle, and controls aligned to the right.
pub fn page_header(ui: &mut Ui, title: &str, subtitle: Option<&str>, right: impl FnOnce(&mut Ui)) {
    let p = Palette::of(ui);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).font(semibold(28.0)).color(p.text));
            if let Some(subtitle) = subtitle {
                ui.label(RichText::new(subtitle).font(regular(14.0)).color(p.muted));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    });
    ui.add_space(GAP);
}

/// A card section title.
pub fn section_title(ui: &mut Ui, text: &str) -> Response {
    let p = Palette::of(ui);
    ui.label(RichText::new(text).font(semibold(17.0)).color(p.text))
}

/// Small secondary text used for captions and field labels.
#[must_use]
pub fn caption(ui: &Ui, text: impl Into<String>) -> RichText {
    RichText::new(text)
        .font(medium(12.0))
        .color(Palette::of(ui).muted)
}

/// Body text in the secondary color.
#[must_use]
pub fn muted(ui: &Ui, text: impl Into<String>) -> RichText {
    RichText::new(text)
        .font(regular(13.0))
        .color(Palette::of(ui).muted)
}

/// A control with a caption above it; the caption labels the control for accessibility.
pub fn field<R>(ui: &mut Ui, label: &str, add_contents: impl FnOnce(&mut Ui, egui::Id) -> R) -> R {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let label = ui.label(caption(ui, label));
        add_contents(ui, label.id)
    })
    .inner
}

/// A pill shaped segmented control. Returns true if the value changed.
pub fn segmented<T: Copy + PartialEq>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let p = Palette::of(ui);
    let mut changed = false;
    // Measure the buttons so the row can be allocated at its true width: a right-to-left
    // parent places the allocation from the right edge.
    let padding = ui.spacing().button_padding.x;
    #[allow(clippy::cast_precision_loss)]
    let width = options
        .iter()
        .map(|(_, label)| {
            ui.painter()
                .layout_no_wrap((*label).to_string(), medium(13.0), p.text)
                .size()
                .x
                + 2.0 * padding
        })
        .sum::<f32>()
        + 2.0 * options.len().saturating_sub(1) as f32;
    Frame::new()
        .fill(p.surface_alt)
        .corner_radius(CornerRadius::same(u8::MAX))
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            // Always left to right (even inside right-to-left headers), one control high.
            ui.allocate_ui_with_layout(
                egui::vec2(width, CONTROL_HEIGHT - 6.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for &(option, label) in options {
                        let selected = *value == option;
                        let text = RichText::new(label).font(medium(13.0)).color(if selected {
                            p.on_primary
                        } else {
                            p.muted
                        });
                        let button = egui::Button::new(text)
                            .selected(selected)
                            .corner_radius(CornerRadius::same(u8::MAX))
                            .min_size(egui::vec2(0.0, CONTROL_HEIGHT - 6.0))
                            .stroke(Stroke::NONE)
                            .fill(if selected {
                                p.primary
                            } else {
                                Color32::TRANSPARENT
                            });
                        if ui.add(button).clicked() && !selected {
                            *value = option;
                            changed = true;
                        }
                    }
                },
            );
        });
    changed
}

/// A filled, pill shaped call to action.
pub fn primary_button(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    let p = Palette::of(ui);
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).font(medium(14.0)).color(p.on_primary))
            .fill(p.primary)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(u8::MAX))
            .min_size(egui::vec2(0.0, CONTROL_HEIGHT)),
    )
}

/// An outlined, pill shaped secondary action.
pub fn secondary_button(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    let p = Palette::of(ui);
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).font(medium(14.0)).color(p.text))
            .fill(p.surface)
            .stroke(Stroke::new(1.0, p.outline))
            .corner_radius(CornerRadius::same(u8::MAX))
            .min_size(egui::vec2(0.0, CONTROL_HEIGHT)),
    )
}

/// A borderless text button for low emphasis actions such as "Remove".
pub fn text_button(ui: &mut Ui, text: &str, color: Color32) -> Response {
    ui.add(egui::Button::new(RichText::new(text).font(medium(13.0)).color(color)).frame(false))
}

/// A row of equally wide stat cards.
pub fn stat_row(ui: &mut Ui, stats: &[Stat]) {
    if stats.is_empty() {
        return;
    }
    ui.columns(stats.len(), |columns| {
        for (ui, stat) in columns.iter_mut().zip(stats) {
            stat.ui(ui);
        }
    });
    ui.add_space(GAP - ui.spacing().item_spacing.y);
}

/// A headline number with a caption above it and optional detail below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stat {
    /// What the number is.
    pub caption: String,
    /// The number.
    pub value: String,
    /// Context, such as the period or category the number belongs to.
    pub detail: Option<String>,
}

impl Stat {
    /// A stat without detail.
    #[must_use]
    pub fn new(caption: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            caption: caption.into(),
            value: value.into(),
            detail: None,
        }
    }

    /// Add a detail line.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Draw the stat as a card.
    fn ui(&self, ui: &mut Ui) {
        let p = Palette::of(ui);
        card(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.label(caption(ui, &self.caption));
            ui.add(
                egui::Label::new(
                    RichText::new(&self.value)
                        .font(semibold(26.0))
                        .color(p.text),
                )
                .truncate(),
            );
            // Keep cards in a row the same height whether or not they have detail.
            let detail = self.detail.as_deref().unwrap_or(" ");
            ui.add(egui::Label::new(muted(ui, detail)).truncate());
        });
    }
}

/// Paint a pill of `text` with an optional dot, sized to its content (so table cells and
/// justified layouts cannot stretch it). Reported to accessibility as a label.
fn pill(
    ui: &mut Ui,
    text: &str,
    font: egui::FontId,
    colors: (Color32, Color32),
    dot: Option<Color32>,
    padding: egui::Vec2,
) -> Response {
    let (fill, text_color) = colors;
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font, text_color);
    let dot_width = if dot.is_some() { 14.0 } else { 0.0 };
    let size = galley.size() + padding * 2.0 + egui::vec2(dot_width, 0.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(u8::MAX), fill);
        let mut x = rect.left() + padding.x;
        if let Some(color) = dot {
            painter.circle_filled(egui::pos2(x + 4.0, rect.center().y), 4.0, color);
            x += dot_width;
        }
        painter.galley(
            egui::pos2(x, rect.center().y - galley.size().y / 2.0),
            galley,
            text_color,
        );
    }
    response
}

/// A rounded chip with an optional color dot, e.g. a category.
pub fn chip(ui: &mut Ui, text: &str, dot: Option<Color32>) -> Response {
    let p = Palette::of(ui);
    pill(
        ui,
        text,
        medium(12.0),
        (p.surface_alt, p.text),
        dot,
        egui::vec2(10.0, 4.0),
    )
}

/// A small count badge.
pub fn badge(ui: &mut Ui, count: usize, strong: bool) -> Response {
    let p = Palette::of(ui);
    let colors = if strong {
        (p.primary, p.on_primary)
    } else {
        (p.surface_alt, p.muted)
    };
    pill(
        ui,
        &count.to_string(),
        medium(11.0),
        colors,
        None,
        egui::vec2(7.0, 2.0),
    )
}

/// A round avatar with the first letter of `name`, tinted with `color`.
pub fn avatar(ui: &mut Ui, name: &str, color: Color32, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), Sense::hover());
    let initial = name
        .chars()
        .find(|c| c.is_alphanumeric())
        .map_or_else(String::new, |c| c.to_uppercase().collect());
    let painter = ui.painter();
    painter.circle_filled(rect.center(), size / 2.0, color.gamma_multiply(0.18));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initial,
        semibold(size * 0.42),
        color,
    );
    response
}

/// The text color for an amount: income green, expenses in the normal ink.
#[must_use]
pub fn amount_color(ui: &Ui, amount: Decimal) -> Color32 {
    let p = Palette::of(ui);
    if amount.is_sign_positive() && !amount.is_zero() {
        p.positive
    } else {
        p.text
    }
}

/// Format an amount with an explicit sign: `+$1,000.00` or `−$38.40`.
#[must_use]
pub fn signed_money(amount: Decimal) -> String {
    let text = money(amount);
    if let Some(rest) = text.strip_prefix('-') {
        format!("\u{2212}{rest}")
    } else if amount.is_zero() {
        text
    } else {
        format!("+{text}")
    }
}

/// A signed, colored amount label.
pub fn amount_label(ui: &mut Ui, amount: Decimal) -> Response {
    let color = amount_color(ui, amount);
    ui.label(
        RichText::new(signed_money(amount))
            .font(medium(14.0))
            .color(color),
    )
}

/// A thin horizontal bar showing `fraction` of the width in `color`.
pub fn share_bar(ui: &mut Ui, fraction: f32, color: Color32, width: f32) -> Response {
    let p = Palette::of(ui);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 6.0), Sense::hover());
    let radius = CornerRadius::same(3);
    ui.painter().rect_filled(rect, radius, p.surface_alt);
    let mut filled = rect;
    filled.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    if filled.width() > 0.0 {
        ui.painter().rect_filled(filled, radius, color);
    }
    response
}

/// A sidebar navigation entry: full width, rounded highlight when selected or hovered, with an
/// optional count badge. Reported to accessibility as a button named `text`.
pub fn nav_item(ui: &mut Ui, selected: bool, text: &str, badge_count: Option<usize>) -> Response {
    let p = Palette::of(ui);
    let size = egui::vec2(ui.available_width(), 38.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, text));
    if ui.is_rect_visible(rect) {
        let fill = if selected {
            p.surface_alt
        } else if response.hovered() {
            p.surface_alt.gamma_multiply(0.6)
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, CornerRadius::same(10), fill);
        let font = if selected {
            semibold(14.0)
        } else {
            medium(14.0)
        };
        let color = if selected { p.text } else { p.muted };
        ui.painter().text(
            rect.left_center() + egui::vec2(12.0, 0.0),
            egui::Align2::LEFT_CENTER,
            text,
            font,
            color,
        );
        if let Some(count) = badge_count.filter(|&c| c > 0) {
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(rect.shrink2(egui::vec2(10.0, 9.0)))
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            badge(&mut child, count, selected);
        }
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_amounts() {
        assert_eq!(signed_money(Decimal::new(-3840, 2)), "\u{2212}$38.40");
        assert_eq!(signed_money(Decimal::new(100_000, 2)), "+$1,000.00");
        assert_eq!(signed_money(Decimal::ZERO), "$0.00");
    }
}
