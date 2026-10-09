//! Which font each character is drawn in, and measuring text with egui's fonts.

use std::cell::RefCell;
use std::collections::HashMap;

use eframe::egui::text::LayoutJob;
use eframe::egui::{Color32, FontId};
use eframe::epaint::FontsView;
use vibedoc_core::{Measure, ParagraphKind, Script, TextStyle};

use crate::ui::fonts::family;

/// Line height as a multiple of the font size, before line spacing.
const LINE_HEIGHT: f32 = 1.4;

/// The font for a character, `zoom` times its size on paper.
pub fn font_id(style: TextStyle, kind: ParagraphKind, zoom: f32) -> FontId {
    let (font, bold) = style.face(kind);
    FontId::new(style.glyph_points(kind) * zoom, family(font, bold))
}

pub struct EguiMeasure<'a, 'b> {
    // `glyph_width` needs `&mut`, but `Measure` only hands out `&self`.
    fonts: RefCell<&'a mut FontsView<'b>>,
    /// (font height, ascent) per font, both for a 1pt size.
    metrics: RefCell<HashMap<String, (f32, f32)>>,
}

impl<'a, 'b> EguiMeasure<'a, 'b> {
    pub fn new(fonts: &'a mut FontsView<'b>) -> Self {
        EguiMeasure { fonts: RefCell::new(fonts), metrics: RefCell::new(HashMap::new()) }
    }

    /// The font's natural line height and ascent, scaled to its size.
    fn metrics(&self, font: &FontId) -> (f32, f32) {
        let key = font.family.to_string();
        let unit = *self.metrics.borrow_mut().entry(key).or_insert_with(|| {
            let probe = FontId::new(100.0, font.family.clone());
            let job = LayoutJob::simple_singleline("x".into(), probe, Color32::WHITE);
            let galley = self.fonts.borrow_mut().layout_job(job);
            galley
                .rows
                .first()
                .and_then(|row| row.glyphs.first())
                .map_or((1.2, 0.95), |g| (g.font_height / 100.0, g.font_ascent / 100.0))
        });
        (unit.0 * font.size, unit.1 * font.size)
    }
}

impl Measure for EguiMeasure<'_, '_> {
    fn char_width(&self, ch: char, style: TextStyle, kind: ParagraphKind) -> f32 {
        let font = font_id(style, kind, 1.0);
        if ch == '\t' {
            4.0 * self.fonts.borrow_mut().glyph_width(&font, ' ')
        } else {
            self.fonts.borrow_mut().glyph_width(&font, ch)
        }
    }

    fn line_height(&self, style: TextStyle, kind: ParagraphKind) -> f32 {
        style.points(kind) * LINE_HEIGHT
    }

    /// Text is centered in its line height, so the descent is half the extra
    /// room plus the part of the font below the baseline.
    fn descent(&self, style: TextStyle, kind: ParagraphKind) -> f32 {
        let font = font_id(TextStyle { script: Script::Normal, ..style }, kind, 1.0);
        let (height, ascent) = self.metrics(&font);
        let line = self.line_height(style, kind);
        (line - height) / 2.0 + (height - ascent)
    }
}
