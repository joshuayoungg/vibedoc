//! Exports a document as a PDF. It draws from the same [`Layout`] the app
//! shows, so lines wrap and pages break exactly as they do on screen. The
//! fonts are embedded, so the PDF looks the same on any computer.

use std::collections::HashMap;

use krilla::color::rgb;
use krilla::geom::{Path, PathBuilder, Point, Rect, Size, Transform};
use krilla::image::Image as PdfImage;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::surface::Surface;
use krilla::text::{Font as PdfFont, TextDirection};
use vibedoc_core::layout::{Line, LIST_INDENT};
use vibedoc_core::{is_image_char, Color, Document, Font, Layout, ParagraphKind, TextStyle};

const INK: Color = Color::rgb(30, 32, 40);
const QUOTE_INK: Color = Color::rgb(80, 82, 92);
const QUOTE_BAR: Color = Color::rgb(200, 203, 212);
const CODE_BACKGROUND: Color = Color::rgb(243, 244, 246);
const CHECKBOX: Color = Color::rgb(140, 144, 154);
const CHECKED: Color = Color::rgb(91, 106, 240);
/// How far italic text leans, since the bundled fonts have no italic faces.
const ITALIC_SLANT: f32 = 0.2;

/// Writes `document` as a PDF, page by page as `layout` arranged it.
/// `font_file` returns the TrueType file for a typeface, regular or bold.
pub fn export(
    document: &Document,
    layout: &Layout,
    font_file: impl Fn(Font, bool) -> &'static [u8],
) -> Result<Vec<u8>, String> {
    let mut fonts = Fonts { file: &font_file, loaded: HashMap::new() };
    let setup = layout.page_setup;
    let settings = PageSettings::from_wh(setup.width, setup.height).ok_or("The page size is invalid")?;
    let mut pdf = krilla::Document::new();
    for page_index in 0..layout.page_count {
        let mut page = pdf.start_page_with(settings.clone());
        let mut surface = page.surface();
        // Everything is placed relative to the page's content area.
        surface.push_transform(&Transform::from_translate(setup.margin, setup.margin));
        for line in layout.lines.iter().filter(|l| l.page == page_index) {
            draw_line(&mut surface, document, line, setup.content_width(), &mut fonts)?;
        }
        surface.pop();
        surface.finish();
        page.finish();
    }
    pdf.finish().map_err(|error| format!("Couldn't write the PDF: {error:?}"))
}

/// Each typeface is loaded once and shared by every page.
struct Fonts<'a> {
    file: &'a dyn Fn(Font, bool) -> &'static [u8],
    loaded: HashMap<(Font, bool), PdfFont>,
}

impl Fonts<'_> {
    fn get(&mut self, style: TextStyle, kind: ParagraphKind) -> Result<PdfFont, String> {
        let face = style.face(kind);
        if let Some(font) = self.loaded.get(&face) {
            return Ok(font.clone());
        }
        let data = (self.file)(face.0, face.1);
        let font = PdfFont::new(data.into(), 0)
            .ok_or_else(|| format!("Couldn't read the {} font", face.0.name()))?;
        self.loaded.insert(face, font.clone());
        Ok(font)
    }
}

fn draw_line(
    surface: &mut Surface,
    document: &Document,
    line: &Line,
    content_width: f32,
    fonts: &mut Fonts,
) -> Result<(), String> {
    let baseline = line.y + line.baseline;
    match line.kind {
        ParagraphKind::Quote => fill_rect(surface, 3.0, line.y, 3.0, line.height, QUOTE_BAR),
        // Slightly taller than the line, so a block of code reads as one box.
        ParagraphKind::Code => {
            fill_rect(surface, 0.0, line.y - 2.5, content_width, line.height + 5.0, CODE_BACKGROUND)
        }
        _ => {}
    }
    for glyph in &line.glyphs {
        if let Some(color) = glyph.style.highlight {
            let size = glyph.style.points(line.kind);
            fill_rect(surface, glyph.x, baseline - size * 0.98, glyph.width, size * 1.28, color);
        }
    }
    draw_marker(surface, document, line, baseline, fonts)?;

    for glyph in &line.glyphs {
        let style = glyph.style;
        if let Some(image) = document.image(glyph.ch) {
            let height = image.height * glyph.width / image.width;
            draw_image(surface, &image.bytes, glyph.x, line.y + line.height - height, glyph.width, height);
            continue;
        }
        let color = style.color.unwrap_or(if line.kind == ParagraphKind::Quote { QUOTE_INK } else { INK });
        let glyph_baseline = baseline - style.rise(line.kind);
        if !is_image_char(glyph.ch) {
            let text = if glyph.ch == '\t' { " ".to_string() } else { glyph.ch.to_string() };
            let italic = style.italic || line.kind == ParagraphKind::Quote;
            let font = fonts.get(style, line.kind)?;
            draw_text(
                surface,
                &text,
                font,
                style.glyph_points(line.kind),
                glyph.x,
                glyph_baseline,
                italic,
                color,
            );
        }
        let size = style.points(line.kind);
        let thickness = (size * 0.055).max(0.5);
        if style.underline {
            fill_rect(surface, glyph.x, glyph_baseline + size * 0.13, glyph.width, thickness, color);
        }
        if style.strikethrough {
            fill_rect(surface, glyph.x, glyph_baseline - size * 0.3, glyph.width, thickness, color);
        }
    }
    Ok(())
}

/// The bullet, number, or checkbox in front of a list item's first line.
fn draw_marker(
    surface: &mut Surface,
    document: &Document,
    line: &Line,
    baseline: f32,
    fonts: &mut Fonts,
) -> Result<(), String> {
    let style = line.glyphs.first().map_or(TextStyle::default(), |g| g.style);
    let size = style.points(line.kind);
    let x = line.indent - LIST_INDENT * 0.75;
    if line.kind == ParagraphKind::Checklist && line.start == 0 {
        let side = size * 0.78;
        let top = baseline - size * 0.36 - side / 2.0;
        if document.paragraphs[line.paragraph].checked {
            fill_rect(surface, x, top, side, side, CHECKED);
            let mut check = PathBuilder::new();
            check.move_to(x + side * 0.24, top + side * 0.52);
            check.line_to(x + side * 0.43, top + side * 0.7);
            check.line_to(x + side * 0.77, top + side * 0.3);
            if let Some(path) = check.finish() {
                surface.set_fill(None);
                surface.set_stroke(Some(stroke(Color::rgb(255, 255, 255), side * 0.12)));
                surface.draw_path(&path);
                surface.set_stroke(None);
            }
        } else if let Some(path) = rect_path(x, top, side, side) {
            surface.set_fill(None);
            surface.set_stroke(Some(stroke(CHECKBOX, (side * 0.08).max(0.6))));
            surface.draw_path(&path);
            surface.set_stroke(None);
        }
        return Ok(());
    }
    if let Some(marker) = &line.marker {
        let style = TextStyle { script: Default::default(), ..style };
        let font = fonts.get(style, line.kind)?;
        draw_text(surface, marker, font, style.points(line.kind), x, baseline, false, INK);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_text(
    surface: &mut Surface,
    text: &str,
    font: PdfFont,
    size: f32,
    x: f32,
    baseline: f32,
    italic: bool,
    color: Color,
) {
    surface.set_stroke(None);
    surface.set_fill(Some(fill(color)));
    // Lean the text about its baseline for italics.
    let slant = if italic { -ITALIC_SLANT } else { 0.0 };
    surface.push_transform(&Transform::from_row(1.0, 0.0, slant, 1.0, x, baseline));
    surface.draw_text(Point::from_xy(0.0, 0.0), font, size, text, false, TextDirection::Auto);
    surface.pop();
}

fn draw_image(surface: &mut Surface, bytes: &[u8], x: f32, y: f32, width: f32, height: f32) {
    let data = bytes.to_vec().into();
    let image = if bytes.starts_with(&[0xFF, 0xD8]) {
        PdfImage::from_jpeg(data, true)
    } else {
        PdfImage::from_png(data, true)
    };
    // A picture that can't be decoded is left out rather than failing the whole export.
    let (Ok(image), Some(size)) = (image, Size::from_wh(width, height)) else { return };
    surface.push_transform(&Transform::from_translate(x, y));
    surface.draw_image(image, size);
    surface.pop();
}

fn fill_rect(surface: &mut Surface, x: f32, y: f32, width: f32, height: f32, color: Color) {
    let Some(path) = rect_path(x, y, width, height) else { return };
    surface.set_stroke(None);
    surface.set_fill(Some(fill(color)));
    surface.draw_path(&path);
}

fn rect_path(x: f32, y: f32, width: f32, height: f32) -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.push_rect(Rect::from_xywh(x, y, width, height)?);
    builder.finish()
}

fn fill(color: Color) -> Fill {
    Fill {
        paint: rgb::Color::new(color.r, color.g, color.b).into(),
        opacity: NormalizedF32::ONE,
        ..Default::default()
    }
}

fn stroke(color: Color, width: f32) -> Stroke {
    Stroke { paint: rgb::Color::new(color.r, color.g, color.b).into(), width, ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vibedoc_core::{Measure, PageSetup, Paragraph, StyledChar};

    /// Every character is 7pt wide and every line 16pt tall.
    struct FixedMeasure;

    impl Measure for FixedMeasure {
        fn char_width(&self, _: char, _: TextStyle, _: ParagraphKind) -> f32 {
            7.0
        }
        fn line_height(&self, _: TextStyle, _: ParagraphKind) -> f32 {
            16.0
        }
    }

    fn font_file(_: Font, bold: bool) -> &'static [u8] {
        if bold {
            include_bytes!("../../vibedoc-app/assets/fonts/Inter/Inter-SemiBold.ttf")
        } else {
            include_bytes!("../../vibedoc-app/assets/fonts/Inter/Inter-Regular.ttf")
        }
    }

    #[test]
    fn exports_every_page_with_formatting() {
        let fancy = TextStyle { bold: true, italic: true, underline: true, ..Default::default() };
        let mut mixed = Paragraph::from_text(ParagraphKind::Normal, "plain ", TextStyle::default());
        mixed.chars.extend("fancy".chars().map(|ch| StyledChar { ch, style: fancy }));
        let mut done = Paragraph::from_text(ParagraphKind::Checklist, "done", TextStyle::default());
        done.checked = true;
        let mut paragraphs = vec![
            Paragraph::from_text(ParagraphKind::Heading1, "Title", TextStyle::default()),
            mixed,
            done,
            Paragraph::from_text(ParagraphKind::Numbered, "first", TextStyle::default()),
            Paragraph::from_text(ParagraphKind::Code, "let x = 1;", TextStyle::default()),
        ];
        // Enough paragraphs to spill onto a second page.
        paragraphs.extend((0..60).map(|i| {
            Paragraph::from_text(ParagraphKind::Normal, &format!("line {i}"), TextStyle::default())
        }));
        let document = Document::from_paragraphs(paragraphs);
        let layout = Layout::build(&document, PageSetup::LETTER, &FixedMeasure);
        assert!(layout.page_count > 1);

        let pdf = export(&document, &layout, font_file).unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains(&format!("/Count {}", layout.page_count)));
    }
}
