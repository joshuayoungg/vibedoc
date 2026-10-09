//! How each kind of paragraph looks, and measuring text with egui's fonts.

use std::cell::RefCell;

use eframe::egui::FontId;
use eframe::epaint::FontsView;
use inkwell_core::{Measure, ParagraphKind, TextStyle};

/// egui's built-in font has no bold face, so bold text is drawn twice,
/// shifted right by this fraction of the font size.
pub const FAKE_BOLD_SHIFT: f32 = 0.04;

pub fn font_size(kind: ParagraphKind) -> f32 {
    match kind {
        ParagraphKind::Heading1 => 22.0,
        ParagraphKind::Heading2 => 17.0,
        ParagraphKind::Heading3 => 14.0,
        _ => 12.0,
    }
}

pub fn is_bold(style: TextStyle, kind: ParagraphKind) -> bool {
    style.bold || kind.is_heading()
}

pub struct EguiMeasure<'a, 'b> {
    // `glyph_width` needs `&mut`, but `Measure` only hands out `&self`.
    fonts: RefCell<&'a mut FontsView<'b>>,
}

impl<'a, 'b> EguiMeasure<'a, 'b> {
    pub fn new(fonts: &'a mut FontsView<'b>) -> Self {
        EguiMeasure { fonts: RefCell::new(fonts) }
    }
}

impl Measure for EguiMeasure<'_, '_> {
    fn char_width(&self, ch: char, style: TextStyle, kind: ParagraphKind) -> f32 {
        let size = font_size(kind);
        let font = FontId::proportional(size);
        let width = if ch == '\t' {
            4.0 * self.fonts.borrow_mut().glyph_width(&font, ' ')
        } else {
            self.fonts.borrow_mut().glyph_width(&font, ch)
        };
        let bold_extra = if is_bold(style, kind) { size * FAKE_BOLD_SHIFT } else { 0.0 };
        width + bold_extra
    }

    fn line_height(&self, kind: ParagraphKind) -> f32 {
        font_size(kind) * 1.4
    }
}
