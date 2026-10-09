//! The few custom controls the interface is built from, so every button,
//! dropdown, and menu row shares one size and one look.

use eframe::egui::{
    vec2, Color32, CursorIcon, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
};

use super::icons::{paint_icon, Icon};
use super::theme::{self, palette, ROW_HEIGHT};

/// Side of a square icon button.
pub const ICON_BUTTON: f32 = 30.0;

/// A square, borderless button showing just an icon. Add a tooltip with `on_hover_text`.
pub fn icon_button(ui: &mut Ui, icon: Icon, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(ICON_BUTTON), Sense::click());
    if ui.is_rect_visible(rect) {
        let (fill, color) = button_colors(ui, &response, selected);
        ui.painter().rect_filled(rect, 8.0, fill);
        paint_icon(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(16.0)), icon, color);
    }
    pointer(ui, response)
}

/// Background and foreground for a borderless button in its current state.
fn button_colors(ui: &Ui, response: &Response, selected: bool) -> (Color32, Color32) {
    let p = palette(ui);
    let enabled = ui.is_enabled();
    let fill = if selected {
        p.accent_soft
    } else if enabled && response.is_pointer_button_down_on() {
        p.pressed
    } else if enabled && response.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    let color = if !enabled {
        p.text_faint
    } else if selected {
        p.accent
    } else if response.hovered() {
        p.text
    } else {
        p.text_muted
    };
    (fill, color)
}

fn pointer(ui: &Ui, response: Response) -> Response {
    if ui.is_enabled() {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Filled with the accent color: the main action.
    Primary,
    /// Soft fill.
    Secondary,
    /// A thin border, no fill.
    Outline,
}

/// A rounded text button, optionally with a leading icon.
pub fn button(ui: &mut Ui, kind: ButtonKind, icon: Option<Icon>, label: &str) -> Response {
    let p = palette(ui);
    let galley = ui.painter().layout_no_wrap(label.to_string(), theme::semibold(13.0), p.text);
    let icon_width = if icon.is_some() { 14.0 + 6.0 } else { 0.0 };
    let size = vec2(galley.size().x + icon_width + 24.0, ROW_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let enabled = ui.is_enabled();
        let active = enabled && (response.hovered() || response.is_pointer_button_down_on());
        let (fill, stroke, text) = match kind {
            ButtonKind::Primary => {
                let fill = match (enabled, active) {
                    (false, _) => p.accent.gamma_multiply(0.4),
                    (true, true) => p.accent_hover,
                    (true, false) => p.accent,
                };
                (fill, Stroke::NONE, Color32::WHITE)
            }
            ButtonKind::Secondary => {
                let fill = if active { p.pressed } else { p.field };
                (fill, Stroke::NONE, if enabled { p.text } else { p.text_faint })
            }
            ButtonKind::Outline => {
                let fill = if active { p.hover } else { Color32::TRANSPARENT };
                (fill, Stroke::new(1.0, p.border), if enabled { p.text } else { p.text_faint })
            }
        };
        let painter = ui.painter();
        painter.rect(rect, 8.0, fill, stroke, StrokeKind::Inside);
        let mut x = rect.left() + 12.0;
        if let Some(icon) = icon {
            let icon_rect = Rect::from_min_size(Pos2::new(x, rect.center().y - 7.0), Vec2::splat(14.0));
            paint_icon(painter, icon_rect, icon, text);
            x += icon_width;
        }
        painter.galley(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, text);
    }
    pointer(ui, response)
}

/// A button that opens a menu: the current value, then a small arrow.
/// `font` lets a font picker show its value in that font.
pub fn dropdown(ui: &mut Ui, text: &str, font: FontId, width: f32) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::click());
    if ui.is_rect_visible(rect) {
        let enabled = ui.is_enabled();
        let fill = if enabled && response.hovered() { p.hover } else { p.field };
        let painter = ui.painter();
        painter.rect_filled(rect, 8.0, fill);
        let color = if enabled { p.text } else { p.text_faint };
        let mut job = eframe::egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
        job.wrap = eframe::egui::text::TextWrapping::truncate_at_width(width - 34.0);
        let galley = painter.layout_job(job);
        painter.galley(Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0), galley, color);
        let chevron =
            Rect::from_center_size(Pos2::new(rect.right() - 13.0, rect.center().y), Vec2::splat(12.0));
        paint_icon(painter, chevron, Icon::ChevronDown, p.text_muted);
    }
    pointer(ui, response)
}

/// One choice in a menu, highlighted when it's the current one.
pub fn menu_choice(ui: &mut Ui, text: &str, font: FontId, selected: bool) -> Response {
    let p = palette(ui);
    let width = ui.available_width().max(140.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::click());
    let fill = if selected {
        p.accent_soft
    } else if response.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    let color = if selected { p.accent } else { p.text };
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    ui.painter().galley(
        Pos2::new(rect.left() + 10.0, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    pointer(ui, response)
}

/// Paints `icon` inline at the current cursor, `size` points square.
pub fn icon(ui: &mut Ui, icon: Icon, size: f32, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_icon(ui.painter(), rect, icon, color);
    response
}

/// A thin vertical line between groups of controls.
pub fn vertical_divider(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(1.0, height), Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range(), Stroke::new(1.0, palette(ui).border));
}
