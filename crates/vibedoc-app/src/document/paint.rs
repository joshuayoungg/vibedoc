//! Painting the laid-out pages: paper, text and its formatting, pictures,
//! selection, and highlights.

use std::collections::HashMap;
use std::ops::Range;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Color32, FontId, Painter, Pos2, Rect, Shadow, Shape, Stroke, TextureHandle, Ui, Vec2,
};
use vibedoc_core::layout::{Line, LIST_INDENT};
use vibedoc_core::{is_image_char, Color, Document, Editor, Layout, ParagraphKind, Position, TextStyle};

use super::fonts::font_id;
use super::geometry::PageGeometry;
use super::{image_key, ImageKey};

// See-through, so selected and found images stay visible underneath. Premultiplied
// (each channel already scaled by alpha), so they tint on light and dark pages alike.
const SELECTION_COLOR: Color32 = Color32::from_rgba_premultiplied(19, 30, 66, 70);
const FIND_COLOR: Color32 = Color32::from_rgba_premultiplied(99, 78, 0, 110);
const CHANGE_COLOR: Color32 = Color32::from_rgba_premultiplied(6, 24, 14, 40);
const CHANGE_BAR: Color32 = Color32::from_rgb(36, 150, 90);
/// Text on a highlight stays dark, so it reads on dark pages too.
const INK_ON_HIGHLIGHT: Color32 = Color32::from_rgb(30, 32, 40);

/// The page's look: paper color, default text color, and the accent for checkboxes.
#[derive(Clone, Copy)]
pub struct Paper {
    pub fill: Color32,
    pub ink: Color32,
    pub accent: Color32,
}

/// How the page looks and what to draw on it besides the text and selection.
pub struct PageLook<'a> {
    pub paper: Paper,
    /// Find matches.
    pub found: &'a [(Position, Position)],
    /// Paragraphs the assistant changed, tinted green with a bar in the margin.
    pub changed: &'a [Range<usize>],
}

pub fn paint(
    ui: &Ui,
    editor: &Editor,
    layout: &Layout,
    pages: &PageGeometry,
    look: &PageLook,
    textures: &HashMap<ImageKey, Option<TextureHandle>>,
    focused: bool,
) {
    let painter = ui.painter();
    let visible = ui.clip_rect();
    let paper = look.paper;
    let shadow = Shadow { offset: [0, 3], blur: 18, spread: 0, color: Color32::from_black_alpha(22) };

    for page in 0..layout.page_count {
        let rect = pages.page_rect(page);
        if rect.expand(24.0).intersects(visible) {
            painter.add(shadow.as_shape(rect, 4.0));
            painter.rect_filled(rect, 4.0, paper.fill);
        }
    }

    let selection = editor.selection();
    for line in &layout.lines {
        let top_left = pages.to_screen(line.page, 0.0, line.y);
        let content_width = layout.page_setup.content_width();
        let line_rect = Rect::from_min_size(top_left, Vec2::new(content_width, line.height) * pages.zoom);
        if !line_rect.intersects(visible) {
            continue;
        }
        paint_block_decoration(painter, line, line_rect, pages.zoom, paper);
        if look.changed.iter().any(|range| range.contains(&line.paragraph)) {
            paint_change_mark(painter, line_rect, pages.zoom);
        }
        paint_line_images(painter, line, pages, editor.document(), textures);
        paint_text_highlights(painter, line, pages);
        for &(start, end) in look.found {
            paint_range(painter, line, pages, start, end, FIND_COLOR);
        }
        if !selection.is_empty() {
            paint_range(painter, line, pages, selection.start(), selection.end(), SELECTION_COLOR);
        }
        paint_marker(ui, painter, line, pages, editor.document(), paper);
        paint_line_text(ui, painter, line, pages, paper);
    }

    if focused {
        let caret = layout.caret(selection.cursor);
        let top = pages.to_screen(caret.page, caret.x, caret.y);
        let bottom = top + Vec2::new(0.0, caret.height * pages.zoom);
        painter.line_segment([top, bottom], Stroke::new(1.5, paper.ink));
    }
}

fn to_color32(color: Color) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

/// The bar beside a quote and the shaded box behind code.
fn paint_block_decoration(painter: &Painter, line: &Line, line_rect: Rect, zoom: f32, paper: Paper) {
    match line.kind {
        ParagraphKind::Quote => {
            let x = line_rect.left() + 3.0 * zoom;
            let bar = Rect::from_x_y_ranges(x..=x + 3.0 * zoom, line_rect.y_range());
            painter.rect_filled(bar, 1.5 * zoom, paper.ink.gamma_multiply(0.22));
        }
        ParagraphKind::Code => {
            // Slightly taller than the line, so a block of code lines reads as one box.
            let block = line_rect.expand2(Vec2::new(0.0, 2.5 * zoom));
            painter.rect_filled(block, 3.0 * zoom, paper.ink.gamma_multiply(0.06));
        }
        _ => {}
    }
}

/// A soft tint behind a changed line, and a change bar in the left margin like Word's.
fn paint_change_mark(painter: &Painter, line_rect: Rect, zoom: f32) {
    let pad = 6.0 * zoom;
    painter.rect_filled(line_rect.expand2(Vec2::new(pad, 1.0)), 0.0, CHANGE_COLOR);
    let bar_x = line_rect.left() - 18.0 * zoom;
    let bar = Rect::from_x_y_ranges(bar_x..=bar_x + 3.0, line_rect.y_range());
    painter.rect_filled(bar, 1.5, CHANGE_BAR);
}

/// Where the line's baseline is on screen.
fn baseline_y(line: &Line, pages: &PageGeometry) -> f32 {
    pages.to_screen(line.page, 0.0, line.y + line.baseline).y
}

/// Highlighter color behind characters that have one.
fn paint_text_highlights(painter: &Painter, line: &Line, pages: &PageGeometry) {
    let baseline = baseline_y(line, pages);
    for glyph in &line.glyphs {
        let Some(color) = glyph.style.highlight else { continue };
        let size = glyph.style.points(line.kind) * pages.zoom;
        let left = pages.to_screen(line.page, glyph.x, line.y).x;
        let rect = Rect::from_x_y_ranges(
            left..=left + glyph.width * pages.zoom,
            baseline - size * 0.98..=baseline + size * 0.3,
        );
        painter.rect_filled(rect, 0.0, to_color32(color));
    }
}

/// Shades the part of `start..end` that falls on this line.
fn paint_range(
    painter: &Painter,
    line: &Line,
    pages: &PageGeometry,
    start: Position,
    end: Position,
    color: Color32,
) {
    if line.paragraph < start.paragraph || line.paragraph > end.paragraph {
        return;
    }
    let from = if line.paragraph == start.paragraph { start.offset.max(line.start) } else { line.start };
    let to = if line.paragraph == end.paragraph { end.offset.min(line.end) } else { line.end };
    let mut right = line.x_of(to);
    // Show the paragraph break as selected when the selection continues past it.
    if line.last_in_paragraph && line.paragraph < end.paragraph {
        right += 4.0;
    }
    let left = line.x_of(from);
    if from > to || right <= left {
        return;
    }
    let min = pages.to_screen(line.page, left, line.y);
    let max = pages.to_screen(line.page, right, line.y + line.height);
    painter.rect_filled(Rect::from_min_max(min, max), 2.0, color);
}

/// Images sit on the bottom of their line, like a very tall letter.
fn paint_line_images(
    painter: &Painter,
    line: &Line,
    pages: &PageGeometry,
    document: &Document,
    textures: &HashMap<ImageKey, Option<TextureHandle>>,
) {
    for glyph in &line.glyphs {
        let Some(image) = document.image(glyph.ch) else { continue };
        let height = image.height * glyph.width / image.width;
        let top_left = pages.to_screen(line.page, glyph.x, line.y + line.height - height);
        let rect = Rect::from_min_size(top_left, Vec2::new(glyph.width, height) * pages.zoom);
        match textures.get(&image_key(image)).and_then(Option::as_ref) {
            Some(texture) => {
                let whole = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                painter.image(texture.id(), rect, whole, Color32::WHITE);
            }
            None => {
                painter.rect_filled(rect, 4.0, Color32::from_gray(235));
                let font = FontId::proportional(14.0);
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, "image", font, Color32::GRAY);
            }
        }
    }
}

/// Lays out one character so it can be placed by its baseline.
fn glyph_galley(ui: &Ui, text: String, format: TextFormat) -> (std::sync::Arc<egui::Galley>, f32) {
    let galley = ui.ctx().fonts_mut(|f| f.layout_job(LayoutJob::single_section(text, format)));
    let baseline =
        galley.rows.first().and_then(|r| r.glyphs.first()).map_or(galley.size().y * 0.8, |g| g.pos.y);
    (galley, baseline)
}

/// The bullet, number, or checkbox in front of a list item's first line.
fn paint_marker(
    ui: &Ui,
    painter: &Painter,
    line: &Line,
    pages: &PageGeometry,
    document: &Document,
    paper: Paper,
) {
    let first_style = line.glyphs.first().map_or(TextStyle::default(), |g| g.style);
    let size = first_style.points(line.kind) * pages.zoom;
    let baseline = baseline_y(line, pages);
    let x = pages.to_screen(line.page, line.indent - LIST_INDENT * 0.75, line.y).x;
    if line.kind == ParagraphKind::Checklist && line.start == 0 {
        let checked = document.paragraphs[line.paragraph].checked;
        let side = size * 0.78;
        let rect = Rect::from_min_size(Pos2::new(x, baseline - size * 0.36 - side / 2.0), Vec2::splat(side));
        if checked {
            painter.rect_filled(rect, side * 0.25, paper.accent);
            let p = |fx: f32, fy: f32| rect.min + Vec2::new(fx, fy) * side;
            let check = vec![p(0.24, 0.52), p(0.43, 0.7), p(0.77, 0.3)];
            painter.add(Shape::line(check, Stroke::new((side * 0.12).max(1.2), Color32::WHITE)));
        } else {
            let stroke = Stroke::new((side * 0.08).max(1.0), paper.ink.gamma_multiply(0.55));
            painter.rect_stroke(rect, side * 0.25, stroke, egui::StrokeKind::Inside);
        }
        return;
    }
    let Some(marker) = &line.marker else { return };
    let format = TextFormat {
        font_id: font_id(TextStyle { script: Default::default(), ..first_style }, line.kind, pages.zoom),
        color: paper.ink,
        ..Default::default()
    };
    let (galley, galley_baseline) = glyph_galley(ui, marker.clone(), format);
    painter.galley(Pos2::new(x, baseline - galley_baseline), galley, paper.ink);
}

fn paint_line_text(ui: &Ui, painter: &Painter, line: &Line, pages: &PageGeometry, paper: Paper) {
    let baseline = baseline_y(line, pages);
    let quote = line.kind == ParagraphKind::Quote;
    for glyph in &line.glyphs {
        let style = glyph.style;
        let size = style.points(line.kind) * pages.zoom;
        let x = pages.to_screen(line.page, glyph.x, line.y).x;
        let default_ink = if style.highlight.is_some() {
            INK_ON_HIGHLIGHT
        } else if quote {
            paper.ink.gamma_multiply(0.78)
        } else {
            paper.ink
        };
        let color = style.color.map_or(default_ink, to_color32);
        let glyph_baseline = baseline - style.rise(line.kind) * pages.zoom;
        if !glyph.ch.is_whitespace() && !is_image_char(glyph.ch) {
            let format = TextFormat {
                font_id: font_id(style, line.kind, pages.zoom),
                color,
                italics: style.italic || quote,
                ..Default::default()
            };
            let (galley, galley_baseline) = glyph_galley(ui, glyph.ch.to_string(), format);
            painter.galley(Pos2::new(x, glyph_baseline - galley_baseline), galley, color);
        }
        let right = x + glyph.width * pages.zoom;
        let thickness = (size * 0.055).max(1.0);
        if style.underline {
            let y = glyph_baseline + size * 0.13;
            painter.line_segment([Pos2::new(x, y), Pos2::new(right, y)], Stroke::new(thickness, color));
        }
        if style.strikethrough {
            let y = glyph_baseline - size * 0.3;
            painter.line_segment([Pos2::new(x, y), Pos2::new(right, y)], Stroke::new(thickness, color));
        }
    }
}
