//! VibeDoc's look: a calm palette in light and dark, shared spacing, and egui's style.

use eframe::egui::{
    self, Color32, CornerRadius, FontId, Frame, Margin, Shadow, Stroke, TextStyle, Theme, Ui,
};
use vibedoc_core::Font;

use super::fonts;

/// The gap around and between the three cards.
pub const GAP: i8 = 12;
/// Height of the top row in every card, so their headers line up.
pub const HEADER_HEIGHT: f32 = 40.0;
/// Height of the second row (search, tabs, status), also shared.
pub const SUBHEADER_HEIGHT: f32 = 32.0;
pub const ROW_HEIGHT: f32 = 30.0;

/// Every color the interface uses, for one theme.
pub struct Palette {
    pub dark: bool,
    pub window: Color32,
    pub surface: Color32,
    /// Behind the pages.
    pub canvas: Color32,
    /// Text boxes and soft fills.
    pub field: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_soft: Color32,
    pub success: Color32,
    pub success_soft: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub warning: Color32,
    pub shadow: Color32,
    /// Folder tiles and tab markers cycle through these.
    pub tiles: [Color32; 6],
}

pub const LIGHT: Palette = Palette {
    dark: false,
    window: Color32::from_rgb(238, 240, 245),
    surface: Color32::WHITE,
    canvas: Color32::from_rgb(244, 245, 248),
    field: Color32::from_rgb(244, 245, 248),
    hover: Color32::from_rgb(239, 241, 245),
    pressed: Color32::from_rgb(228, 231, 238),
    border: Color32::from_rgb(228, 231, 236),
    text: Color32::from_rgb(28, 31, 41),
    text_muted: Color32::from_rgb(102, 110, 128),
    text_faint: Color32::from_rgb(152, 160, 176),
    accent: Color32::from_rgb(91, 106, 240),
    accent_hover: Color32::from_rgb(78, 93, 226),
    accent_soft: Color32::from_rgb(236, 238, 254),
    success: Color32::from_rgb(36, 133, 84),
    success_soft: Color32::from_rgb(230, 245, 236),
    danger: Color32::from_rgb(208, 72, 66),
    danger_soft: Color32::from_rgb(252, 236, 235),
    warning: Color32::from_rgb(196, 128, 20),
    shadow: Color32::from_black_alpha(12),
    tiles: [
        Color32::from_rgb(91, 106, 240),
        Color32::from_rgb(226, 88, 120),
        Color32::from_rgb(232, 145, 40),
        Color32::from_rgb(38, 160, 110),
        Color32::from_rgb(40, 150, 210),
        Color32::from_rgb(150, 92, 220),
    ],
};

pub const DARK: Palette = Palette {
    dark: true,
    window: Color32::from_rgb(15, 16, 19),
    surface: Color32::from_rgb(24, 25, 29),
    canvas: Color32::from_rgb(17, 18, 21),
    field: Color32::from_rgb(33, 34, 39),
    hover: Color32::from_rgb(36, 38, 43),
    pressed: Color32::from_rgb(44, 46, 52),
    border: Color32::from_rgb(42, 44, 50),
    text: Color32::from_rgb(231, 232, 236),
    text_muted: Color32::from_rgb(161, 166, 178),
    text_faint: Color32::from_rgb(108, 113, 124),
    accent: Color32::from_rgb(129, 144, 255),
    accent_hover: Color32::from_rgb(147, 160, 255),
    accent_soft: Color32::from_rgb(38, 42, 72),
    success: Color32::from_rgb(76, 195, 138),
    success_soft: Color32::from_rgb(26, 51, 38),
    danger: Color32::from_rgb(240, 113, 107),
    danger_soft: Color32::from_rgb(58, 32, 32),
    warning: Color32::from_rgb(226, 166, 59),
    shadow: Color32::from_black_alpha(70),
    tiles: [
        Color32::from_rgb(129, 144, 255),
        Color32::from_rgb(236, 112, 140),
        Color32::from_rgb(240, 165, 70),
        Color32::from_rgb(70, 190, 135),
        Color32::from_rgb(80, 175, 230),
        Color32::from_rgb(175, 125, 240),
    ],
};

pub fn palette(ui: &Ui) -> &'static Palette {
    palette_for(ui.visuals().dark_mode)
}

pub fn palette_for(dark: bool) -> &'static Palette {
    if dark {
        &DARK
    } else {
        &LIGHT
    }
}

/// Inter SemiBold, for headings and emphasis in the interface.
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, fonts::family(Font::Inter, true))
}

/// The rounded card each column sits in.
pub fn card(palette: &Palette) -> Frame {
    Frame::new()
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.border))
        .corner_radius(14)
        .inner_margin(Margin::same(12))
        .shadow(Shadow { offset: [0, 2], blur: 16, spread: 0, color: palette.shadow })
}

pub fn install(ctx: &egui::Context) {
    fonts::install(ctx);
    for (theme, palette) in [(Theme::Light, &LIGHT), (Theme::Dark, &DARK)] {
        ctx.style_mut_of(theme, |style| configure(style, palette));
    }
}

fn configure(style: &mut egui::Style, p: &Palette) {
    style.text_styles = [
        (TextStyle::Small, FontId::proportional(11.5)),
        (TextStyle::Body, FontId::proportional(13.5)),
        (TextStyle::Button, FontId::proportional(13.5)),
        (TextStyle::Heading, semibold(17.0)),
        (TextStyle::Monospace, FontId::monospace(12.5)),
    ]
    .into();

    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(8.0, 6.0);
    spacing.button_padding = egui::vec2(10.0, 5.0);
    spacing.interact_size.y = 28.0;
    spacing.menu_margin = Margin::same(6);
    spacing.window_margin = Margin::same(16);
    spacing.combo_height = 320.0;
    spacing.scroll = egui::style::ScrollStyle::floating();
    spacing.scroll.bar_width = 6.0;
    style.interaction.tooltip_delay = 0.4;

    let v = &mut style.visuals;
    v.dark_mode = p.dark;
    v.override_text_color = None;
    v.panel_fill = p.window;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(14);
    v.window_shadow = Shadow { offset: [0, 10], blur: 32, spread: 0, color: p.shadow.gamma_multiply(2.2) };
    v.popup_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: p.shadow.gamma_multiply(1.8) };
    v.menu_corner_radius = CornerRadius::same(10);
    v.extreme_bg_color = p.field;
    v.text_edit_bg_color = Some(p.field);
    v.faint_bg_color = p.field;
    v.code_bg_color = p.field;
    v.hyperlink_color = p.accent;
    v.selection.bg_fill = p.accent.gamma_multiply(0.28);
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.text_cursor.stroke = Stroke::new(1.5, p.accent);
    v.error_fg_color = p.danger;
    v.warn_fg_color = p.warning;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = CornerRadius::same(8);
    for (state, fill) in [
        (&mut w.inactive, p.field),
        (&mut w.hovered, p.hover),
        (&mut w.active, p.pressed),
        (&mut w.open, p.hover),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::NONE;
        state.fg_stroke = Stroke::new(1.0, p.text);
        state.corner_radius = CornerRadius::same(8);
        state.expansion = 0.0;
    }
    // Menu items and buttons in popups read better without a resting fill.
    w.inactive.weak_bg_fill = Color32::TRANSPARENT;
}
