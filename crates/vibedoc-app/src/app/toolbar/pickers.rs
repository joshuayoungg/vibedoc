//! The ribbon's dropdowns and color palettes: paragraph style, font, size,
//! text color, highlight, and line spacing.

use eframe::egui::{vec2, Color32, CursorIcon, FontId, Popup, Rect, Response, Sense, Stroke, Ui, Vec2};
use vibedoc_core::editor::FONT_SIZES;
use vibedoc_core::{Color, Editor, Font, LineSpacing, ParagraphKind};

use crate::ui::fonts::family;
use crate::ui::icons::{paint_icon, Icon};
use crate::ui::theme::{self, palette};
use crate::ui::widgets::{dropdown, icon_button, menu_choice, ICON_BUTTON};

/// Colors for text, chosen to read well on light pages.
const TEXT_COLORS: [(Color, &str); 10] = [
    (Color::rgb(0x37, 0x41, 0x51), "Graphite"),
    (Color::rgb(0xDC, 0x26, 0x26), "Red"),
    (Color::rgb(0xEA, 0x58, 0x0C), "Orange"),
    (Color::rgb(0xCA, 0x8A, 0x04), "Amber"),
    (Color::rgb(0x16, 0xA3, 0x4A), "Green"),
    (Color::rgb(0x0D, 0x94, 0x88), "Teal"),
    (Color::rgb(0x25, 0x63, 0xEB), "Blue"),
    (Color::rgb(0x4F, 0x46, 0xE5), "Indigo"),
    (Color::rgb(0x93, 0x33, 0xEA), "Purple"),
    (Color::rgb(0xDB, 0x27, 0x77), "Pink"),
];

/// Soft highlighter colors that keep text readable.
const HIGHLIGHTS: [(Color, &str); 8] = [
    (Color::rgb(0xFE, 0xF0, 0x8A), "Yellow"),
    (Color::rgb(0xBB, 0xF7, 0xD0), "Green"),
    (Color::rgb(0xBF, 0xDB, 0xFE), "Blue"),
    (Color::rgb(0xFB, 0xCF, 0xE8), "Pink"),
    (Color::rgb(0xE9, 0xD5, 0xFF), "Purple"),
    (Color::rgb(0xFE, 0xD7, 0xAA), "Orange"),
    (Color::rgb(0xA5, 0xF3, 0xFC), "Cyan"),
    (Color::rgb(0xE5, 0xE7, 0xEB), "Gray"),
];

/// The styles offered in the style menu; lists have their own buttons.
const STYLES: [ParagraphKind; 6] = [
    ParagraphKind::Normal,
    ParagraphKind::Heading1,
    ParagraphKind::Heading2,
    ParagraphKind::Heading3,
    ParagraphKind::Quote,
    ParagraphKind::Code,
];

fn to_color32(color: Color) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

pub fn paragraph_style(ui: &mut Ui, editor: &mut Editor) {
    let current = editor.current_paragraph_kind();
    let response =
        dropdown(ui, current.label(), FontId::proportional(13.0), 112.0).on_hover_text("Paragraph style");
    Popup::menu(&response).show(|ui| {
        ui.set_min_width(180.0);
        for kind in STYLES {
            // Each style previews itself.
            let font = match kind {
                ParagraphKind::Heading1 => theme::semibold(17.0),
                ParagraphKind::Heading2 => theme::semibold(15.0),
                ParagraphKind::Heading3 => theme::semibold(13.5),
                ParagraphKind::Code => FontId::new(12.5, family(Font::JetBrainsMono, false)),
                _ => FontId::proportional(13.5),
            };
            if menu_choice(ui, kind.label(), font, kind == current).clicked() {
                editor.set_paragraph_kind(kind);
                ui.close();
            }
        }
    });
}

pub fn font_family(ui: &mut Ui, editor: &mut Editor) {
    let current = editor.current_style().font;
    let font = FontId::new(13.0, family(current, false));
    let response = dropdown(ui, current.name(), font, 128.0).on_hover_text("Font");
    Popup::menu(&response).show(|ui| {
        ui.set_min_width(190.0);
        for choice in Font::ALL {
            let preview = FontId::new(14.0, family(choice, false));
            if menu_choice(ui, choice.name(), preview, choice == current).clicked() {
                editor.apply_style(|s| s.font = choice);
                ui.close();
            }
        }
    });
}

pub fn font_size(ui: &mut Ui, editor: &mut Editor) {
    let points = editor.current_style().points(editor.current_paragraph_kind());
    let response = dropdown(ui, &format_points(points), FontId::proportional(13.0), 58.0)
        .on_hover_text("Font size (Ctrl+] and Ctrl+[ to step)");
    Popup::menu(&response).show(|ui| {
        ui.set_min_width(80.0);
        eframe::egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
            for size in FONT_SIZES {
                let selected = (size - points).abs() < 0.01;
                if menu_choice(ui, &format_points(size), FontId::proportional(13.5), selected).clicked() {
                    editor.apply_style(|s| s.size = Some((size * 2.0) as u16));
                    ui.close();
                }
            }
        });
    });
}

/// "12" or "10.5".
fn format_points(points: f32) -> String {
    if points.fract() == 0.0 {
        format!("{points:.0}")
    } else {
        format!("{points:.1}")
    }
}

pub fn text_color(ui: &mut Ui, editor: &mut Editor) {
    let p = palette(ui);
    let current = editor.current_style().color;
    let bar = current.map_or(p.text, to_color32);
    let response = color_button(ui, Icon::TextColor, bar).on_hover_text("Text color");
    Popup::menu(&response).show(|ui| {
        if swatch_row_label(ui, "Automatic", current.is_none()).clicked() {
            editor.apply_style(|s| s.color = None);
            ui.close();
        }
        if let Some(color) = swatch_grid(ui, &TEXT_COLORS, current) {
            editor.apply_style(|s| s.color = Some(color));
            ui.close();
        }
    });
}

pub fn highlight(ui: &mut Ui, editor: &mut Editor) {
    let current = editor.current_style().highlight;
    let bar = current.map_or(Color32::TRANSPARENT, to_color32);
    let response = color_button(ui, Icon::Highlighter, bar).on_hover_text("Highlight");
    Popup::menu(&response).show(|ui| {
        if swatch_row_label(ui, "No highlight", current.is_none()).clicked() {
            editor.apply_style(|s| s.highlight = None);
            ui.close();
        }
        if let Some(color) = swatch_grid(ui, &HIGHLIGHTS, current) {
            editor.apply_style(|s| s.highlight = Some(color));
            ui.close();
        }
    });
}

/// One icon that opens a single row of choices: 1.0, 1.15, 1.5, 2.0.
pub fn line_spacing(ui: &mut Ui, editor: &mut Editor) {
    let current = editor.current_paragraph().spacing;
    let tip = format!("Line spacing ({})", current.label());
    let response = icon_button(ui, Icon::LineSpacing, false).on_hover_text(tip);
    Popup::menu(&response).show(|ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for spacing in LineSpacing::ALL {
                if spacing_choice(ui, spacing.label(), spacing == current).clicked() {
                    editor.format_paragraphs(|p| p.spacing = spacing);
                    ui.close();
                }
            }
        });
    });
}

fn spacing_choice(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(44.0, 28.0), Sense::click());
    let fill = if selected {
        p.accent_soft
    } else if response.hovered() {
        p.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    let color = if selected { p.accent } else { p.text };
    let font = theme::semibold(12.5);
    ui.painter().text(rect.center(), eframe::egui::Align2::CENTER_CENTER, label, font, color);
    response.on_hover_cursor(CursorIcon::PointingHand)
}

/// An icon with a color bar under it and a small arrow: opens a palette.
fn color_button(ui: &mut Ui, icon: Icon, bar: Color32) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(ICON_BUTTON + 14.0, ICON_BUTTON), Sense::click());
    let enabled = ui.is_enabled();
    let fill = if enabled && response.hovered() { p.hover } else { Color32::TRANSPARENT };
    ui.painter().rect_filled(rect, 8.0, fill);
    let color = if enabled { p.text_muted } else { p.text_faint };
    let icon_rect = Rect::from_center_size(rect.left_center() + vec2(15.0, -1.5), Vec2::splat(15.0));
    paint_icon(ui.painter(), icon_rect, icon, color);
    if bar != Color32::TRANSPARENT {
        let bar_rect = Rect::from_center_size(rect.left_center() + vec2(15.0, 9.0), vec2(14.0, 3.0));
        ui.painter().rect_filled(bar_rect, 1.5, bar);
    }
    let chevron = Rect::from_center_size(rect.right_center() - vec2(9.0, 0.0), Vec2::splat(10.0));
    paint_icon(ui.painter(), chevron, Icon::ChevronDown, color);
    if enabled {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    }
}

/// A full-width choice above the swatches, like "Automatic".
fn swatch_row_label(ui: &mut Ui, label: &str, selected: bool) -> Response {
    ui.set_min_width(5.0 * 30.0);
    menu_choice(ui, label, FontId::proportional(13.0), selected)
}

/// Round swatches, five to a row. Returns the color clicked, if any.
fn swatch_grid(ui: &mut Ui, colors: &[(Color, &str)], current: Option<Color>) -> Option<Color> {
    let p = palette(ui);
    let mut picked = None;
    for row in colors.chunks(5) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for &(color, name) in row {
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                if current == Some(color) {
                    ui.painter().circle_stroke(rect.center(), 12.0, Stroke::new(2.0, p.accent));
                }
                ui.painter().circle(rect.center(), 9.0, to_color32(color), Stroke::new(1.0, p.border));
                if response.on_hover_text(name).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    picked = Some(color);
                }
            }
        });
    }
    picked
}
