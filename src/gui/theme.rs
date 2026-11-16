//! The visual theme: color tokens, the Inter typeface and egui style for light and dark mode.
//!
//! The look is calm and editorial: warm off-white (or near-black) canvas, white (or charcoal)
//! cards with hairline borders, near-black ink, pill-shaped controls and large numbers.
//! Views take colors from [`Palette::of`] rather than hard coding them, so both themes stay
//! consistent.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, Style, TextStyle, Theme, Ui,
};

/// Color tokens for one theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Window background behind cards.
    pub canvas: Color32,
    /// Card and sidebar background.
    pub surface: Color32,
    /// Quiet fill for controls, segmented tracks and hovered rows.
    pub surface_alt: Color32,
    /// Fill of a hovered control.
    pub hover: Color32,
    /// Hairline borders and dividers.
    pub hairline: Color32,
    /// Border of inputs and outlined buttons.
    pub outline: Color32,
    /// Primary text.
    pub text: Color32,
    /// Secondary text (captions, metadata).
    pub muted: Color32,
    /// Tertiary text (placeholders, axis labels).
    pub faint: Color32,
    /// Fill of primary buttons and the selected segment.
    pub primary: Color32,
    /// Text on [`Self::primary`].
    pub on_primary: Color32,
    /// Income and gains.
    pub positive: Color32,
    /// Errors and destructive actions.
    pub negative: Color32,
    /// Warnings such as unsaved changes.
    pub warning: Color32,
    /// Text selection and focus.
    pub selection: Color32,
}

/// Light theme tokens.
pub const LIGHT: Palette = Palette {
    canvas: Color32::from_rgb(0xf5, 0xf4, 0xf0),
    surface: Color32::from_rgb(0xff, 0xff, 0xff),
    surface_alt: Color32::from_rgb(0xef, 0xee, 0xe9),
    hover: Color32::from_rgb(0xe6, 0xe4, 0xde),
    hairline: Color32::from_rgb(0xe6, 0xe4, 0xde),
    outline: Color32::from_rgb(0xd6, 0xd3, 0xcb),
    text: Color32::from_rgb(0x19, 0x19, 0x17),
    muted: Color32::from_rgb(0x67, 0x65, 0x60),
    faint: Color32::from_rgb(0x9a, 0x97, 0x90),
    primary: Color32::from_rgb(0x19, 0x19, 0x17),
    on_primary: Color32::from_rgb(0xff, 0xff, 0xff),
    positive: Color32::from_rgb(0x0b, 0x7a, 0x47),
    negative: Color32::from_rgb(0xb4, 0x23, 0x18),
    warning: Color32::from_rgb(0xa1, 0x5c, 0x00),
    selection: Color32::from_rgb(0xd9, 0xe4, 0xf7),
};

/// Dark theme tokens.
pub const DARK: Palette = Palette {
    canvas: Color32::from_rgb(0x11, 0x11, 0x10),
    surface: Color32::from_rgb(0x1c, 0x1c, 0x1a),
    surface_alt: Color32::from_rgb(0x26, 0x26, 0x24),
    hover: Color32::from_rgb(0x30, 0x30, 0x2d),
    hairline: Color32::from_rgb(0x2c, 0x2c, 0x29),
    outline: Color32::from_rgb(0x3d, 0x3d, 0x39),
    text: Color32::from_rgb(0xf3, 0xf2, 0xee),
    muted: Color32::from_rgb(0xa8, 0xa6, 0x9f),
    faint: Color32::from_rgb(0x74, 0x72, 0x6c),
    primary: Color32::from_rgb(0xf3, 0xf2, 0xee),
    on_primary: Color32::from_rgb(0x11, 0x11, 0x10),
    positive: Color32::from_rgb(0x4c, 0xc3, 0x8a),
    negative: Color32::from_rgb(0xf0, 0x71, 0x67),
    warning: Color32::from_rgb(0xe0, 0xa4, 0x3a),
    selection: Color32::from_rgb(0x2b, 0x3b, 0x57),
};

impl Palette {
    /// The palette for the theme `ui` is drawn in.
    #[must_use]
    pub fn of(ui: &Ui) -> &'static Self {
        Self::for_dark_mode(ui.visuals().dark_mode)
    }

    /// The palette for light or dark mode.
    #[must_use]
    pub fn for_dark_mode(dark_mode: bool) -> &'static Self {
        if dark_mode { &DARK } else { &LIGHT }
    }
}

/// Corner radius of cards.
pub const CARD_RADIUS: u8 = 16;
/// Corner radius of inputs, combo boxes and menus.
pub const CONTROL_RADIUS: u8 = 10;
/// Height of buttons and inputs.
pub const CONTROL_HEIGHT: f32 = 34.0;

/// Font family name of Inter Medium.
const MEDIUM: &str = "Inter-Medium";
/// Font family name of Inter SemiBold.
const SEMIBOLD: &str = "Inter-SemiBold";

/// Inter Medium at `size`.
#[must_use]
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}

/// Inter SemiBold at `size`.
#[must_use]
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

/// Inter Regular at `size`.
#[must_use]
pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Font definitions with Inter as the proportional face (keeping egui's fonts as fallbacks for
/// symbols and emoji) and named families for the heavier weights.
#[must_use]
pub fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let faces: [(&str, &'static [u8]); 3] = [
        (
            "Inter-Regular",
            include_bytes!("../../assets/fonts/Inter-Regular.ttf"),
        ),
        (
            MEDIUM,
            include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
        ),
        (
            SEMIBOLD,
            include_bytes!("../../assets/fonts/Inter-SemiBold.ttf"),
        ),
    ];
    for (name, bytes) in faces {
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (family, face) in [
        (FontFamily::Proportional, "Inter-Regular"),
        (FontFamily::Name(MEDIUM.into()), MEDIUM),
        (FontFamily::Name(SEMIBOLD.into()), SEMIBOLD),
    ] {
        let mut chain = vec![face.to_string()];
        chain.extend(fallbacks.iter().cloned());
        fonts.families.insert(family, chain);
    }
    fonts
}

/// The egui style for one theme.
#[must_use]
pub fn style(theme: Theme) -> Style {
    let p = Palette::for_dark_mode(theme == Theme::Dark);
    let mut style = Style {
        visuals: theme.default_visuals(),
        ..Style::default()
    };

    style.text_styles = [
        (TextStyle::Small, regular(12.0)),
        (TextStyle::Body, regular(14.0)),
        (TextStyle::Button, medium(14.0)),
        (TextStyle::Heading, semibold(28.0)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into();

    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(8.0, 8.0);
    spacing.button_padding = egui::vec2(14.0, 7.0);
    spacing.interact_size = egui::vec2(40.0, CONTROL_HEIGHT);
    spacing.combo_height = 320.0;
    spacing.menu_margin = Margin::same(6);
    spacing.window_margin = Margin::same(14);
    spacing.icon_width = 16.0;
    spacing.icon_width_inner = 9.0;
    spacing.scroll = egui::style::ScrollStyle {
        // Draw handles in the text color (faded), since control fills match the surface.
        foreground_color: true,
        dormant_handle_opacity: 0.25,
        active_handle_opacity: 0.4,
        interact_handle_opacity: 0.55,
        dormant_background_opacity: 0.0,
        active_background_opacity: 0.0,
        interact_background_opacity: 0.0,
        ..egui::style::ScrollStyle::thin()
    };

    let v = &mut style.visuals;
    v.override_text_color = None;
    v.weak_text_color = Some(p.muted);
    v.hyperlink_color = p.text;
    v.panel_fill = p.canvas;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.hairline);
    v.window_corner_radius = CornerRadius::same(14);
    v.menu_corner_radius = CornerRadius::same(12);
    let shadow = Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if theme == Theme::Dark { 120 } else { 28 }),
    };
    v.window_shadow = shadow;
    v.popup_shadow = shadow;
    v.extreme_bg_color = p.surface;
    v.text_edit_bg_color = Some(p.surface);
    v.faint_bg_color = if theme == Theme::Dark {
        Color32::from_rgb(0x21, 0x21, 0x1f)
    } else {
        Color32::from_rgb(0xfa, 0xf9, 0xf6)
    };
    v.code_bg_color = p.surface_alt;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.negative;
    v.selection.bg_fill = p.selection;
    v.selection.stroke = Stroke::new(1.0, p.text);
    v.collapsing_header_frame = false;
    v.indent_has_left_vline = false;
    v.striped = false;
    v.slider_trailing_fill = true;

    let radius = CornerRadius::same(CONTROL_RADIUS);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.hairline);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = p.surface;
    w.inactive.weak_bg_fill = p.surface;
    w.inactive.bg_stroke = Stroke::new(1.0, p.outline);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = radius;
    w.inactive.expansion = 0.0;

    w.hovered.bg_fill = p.hover;
    w.hovered.weak_bg_fill = p.surface_alt;
    w.hovered.bg_stroke = Stroke::new(1.0, p.faint);
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = p.hover;
    w.active.weak_bg_fill = p.hover;
    w.active.bg_stroke = Stroke::new(1.0, p.text);
    w.active.fg_stroke = Stroke::new(1.5, p.text);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open.bg_fill = p.surface_alt;
    w.open.weak_bg_fill = p.surface_alt;
    w.open.bg_stroke = Stroke::new(1.0, p.outline);
    w.open.fg_stroke = Stroke::new(1.0, p.text);
    w.open.corner_radius = radius;

    style
}

/// Install the fonts and both theme styles into `ctx`.
///
/// Fonts take effect from the next frame, so callers should not draw with the named Inter
/// families until a frame after this call.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    for theme in [Theme::Light, Theme::Dark] {
        ctx.set_style_of(theme, style(theme));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_define_named_weights() {
        let fonts = fonts();
        for family in [MEDIUM, SEMIBOLD] {
            let chain = &fonts.families[&FontFamily::Name(family.into())];
            assert_eq!(chain[0], family);
            assert!(chain.len() > 1, "keeps fallbacks for symbols");
        }
        assert_eq!(
            fonts.families[&FontFamily::Proportional][0],
            "Inter-Regular"
        );
    }

    #[test]
    fn styles_follow_palette() {
        let light = style(Theme::Light);
        let dark = style(Theme::Dark);
        assert!(!light.visuals.dark_mode);
        assert!(dark.visuals.dark_mode);
        assert_eq!(light.visuals.panel_fill, LIGHT.canvas);
        assert_eq!(dark.visuals.panel_fill, DARK.canvas);
    }
}
