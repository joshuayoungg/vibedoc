//! Turns a document into lines on pages.
//!
//! All numbers are in points (1/72 inch), measured from the top-left corner of
//! a page's *content area* (inside the margins). The UI only has to scale and
//! paint. Text measuring is left to the caller through the [`Measure`] trait,
//! which keeps this module free of any GUI code and easy to test.

use crate::document::{Alignment, Document, Paragraph, ParagraphKind, Position, TextStyle};

/// How wide list content is pushed in to make room for the bullet, number, or checkbox.
pub const LIST_INDENT: f32 = 24.0;
/// Quotes sit to the right of a bar; code sits inside a shaded box.
pub const QUOTE_INDENT: f32 = 18.0;
pub const CODE_INDENT: f32 = 10.0;

pub trait Measure {
    fn char_width(&self, ch: char, style: TextStyle, kind: ParagraphKind) -> f32;
    /// Height of a line of text in this style, with single spacing.
    fn line_height(&self, style: TextStyle, kind: ParagraphKind) -> f32;
    /// How much of `line_height` is below the baseline.
    fn descent(&self, style: TextStyle, kind: ParagraphKind) -> f32 {
        self.line_height(style, kind) * 0.25
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageSetup {
    pub width: f32,
    pub height: f32,
    pub margin: f32,
}

impl PageSetup {
    /// US Letter with one-inch margins, Word's default.
    pub const LETTER: PageSetup = PageSetup { width: 612.0, height: 792.0, margin: 72.0 };

    pub fn content_width(&self) -> f32 {
        self.width - 2.0 * self.margin
    }

    pub fn content_height(&self) -> f32 {
        self.height - 2.0 * self.margin
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub ch: char,
    pub style: TextStyle,
    pub x: f32,
    pub width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub page: usize,
    pub y: f32,
    /// Full height, including any image taller than the text.
    pub height: f32,
    /// Height of the text part, including line spacing; text sits at the bottom of the line.
    pub text_height: f32,
    /// Distance from the top of the line down to the text's baseline.
    pub baseline: f32,
    pub paragraph: usize,
    pub kind: ParagraphKind,
    /// Character offsets in the paragraph covered by this line (`end` is exclusive).
    pub start: usize,
    pub end: usize,
    pub indent: f32,
    /// Where an empty line's caret goes, so centered empty lines put it in the middle.
    pub empty_x: f32,
    pub glyphs: Vec<Glyph>,
    /// "•" or "3." on the first line of a list item.
    pub marker: Option<String>,
    pub last_in_paragraph: bool,
}

impl Line {
    /// Horizontal position of the caret before character `offset`.
    pub fn x_of(&self, offset: usize) -> f32 {
        let index = offset.saturating_sub(self.start);
        match (self.glyphs.get(index), self.glyphs.last()) {
            (Some(glyph), _) => glyph.x,
            (None, Some(last)) => last.x + last.width,
            (None, None) => self.empty_x,
        }
    }

    /// The caret offset closest to `x`.
    pub fn offset_at_x(&self, x: f32) -> usize {
        for (i, glyph) in self.glyphs.iter().enumerate() {
            if x < glyph.x + glyph.width / 2.0 {
                return self.start + i;
            }
        }
        self.visual_end()
    }

    /// Where End puts the caret. On a wrapped line that is before the trailing
    /// space, so the caret stays on this line instead of jumping to the next.
    pub fn visual_end(&self) -> usize {
        if self.last_in_paragraph || self.end == self.start {
            self.end
        } else {
            self.end - 1
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaretRect {
    pub page: usize,
    pub x: f32,
    pub y: f32,
    pub height: f32,
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub page_setup: PageSetup,
    pub lines: Vec<Line>,
    pub page_count: usize,
}

impl Layout {
    pub fn build(document: &Document, page_setup: PageSetup, measure: &impl Measure) -> Layout {
        let mut lines = Vec::new();
        let mut page = 0;
        let mut y = 0.0;
        let mut list_number = 0;

        for (index, paragraph) in document.paragraphs.iter().enumerate() {
            let kind = paragraph.kind;
            list_number = if kind == ParagraphKind::Numbered { list_number + 1 } else { 0 };
            let indent = indent_of(kind);
            let max_width = page_setup.content_width() - indent;
            let widths: Vec<f32> = paragraph
                .chars
                .iter()
                .map(|c| match document.image(c.ch) {
                    Some(image) => image.width.min(max_width),
                    None => measure.char_width(c.ch, c.style, kind),
                })
                .collect();
            let chars: Vec<char> = paragraph.chars.iter().map(|c| c.ch).collect();
            let ranges = wrap(&chars, &widths, max_width);
            let line_count = ranges.len();

            for (line_index, (start, end)) in ranges.into_iter().enumerate() {
                let last_in_paragraph = line_index + 1 == line_count;
                let (text_height, descent) = text_metrics(paragraph, start, end, measure);
                let tallest_image = chars[start..end]
                    .iter()
                    .filter_map(|&ch| document.image(ch))
                    .map(|image| image.height)
                    .fold(0.0, f32::max);
                let height = text_height.max(tallest_image);
                if y + height > page_setup.content_height() && y > 0.0 {
                    page += 1;
                    y = 0.0;
                }
                let mut x = indent;
                let mut glyphs: Vec<Glyph> = paragraph.chars[start..end]
                    .iter()
                    .zip(&widths[start..end])
                    .map(|(c, &width)| {
                        let glyph = Glyph { ch: c.ch, style: c.style, x, width };
                        x += width;
                        glyph
                    })
                    .collect();
                let justify = paragraph.alignment == Alignment::Justify && !last_in_paragraph;
                let empty_x = align(&mut glyphs, paragraph.alignment, justify, indent, max_width);
                let marker = match (line_index, kind) {
                    (0, ParagraphKind::Bullet) => Some("•".to_string()),
                    (0, ParagraphKind::Numbered) => Some(format!("{list_number}.")),
                    _ => None,
                };
                lines.push(Line {
                    page,
                    y,
                    height,
                    text_height,
                    baseline: height - descent,
                    paragraph: index,
                    kind,
                    start,
                    end,
                    indent,
                    empty_x,
                    glyphs,
                    marker,
                    last_in_paragraph,
                });
                y += height;
            }
            let next_kind = document.paragraphs.get(index + 1).map(|p| p.kind);
            y += paragraph_spacing(kind, next_kind, measure.line_height(TextStyle::default(), kind));
        }

        Layout { page_setup, lines, page_count: page + 1 }
    }

    /// Index of the line that shows the caret at `position`.
    pub fn line_index_of(&self, position: Position) -> usize {
        let first = self.lines.partition_point(|l| l.paragraph < position.paragraph);
        let mut index = first.min(self.lines.len() - 1);
        while index + 1 < self.lines.len()
            && self.lines[index + 1].paragraph == position.paragraph
            && position.offset >= self.lines[index].end
        {
            index += 1;
        }
        index
    }

    pub fn caret(&self, position: Position) -> CaretRect {
        let line = &self.lines[self.line_index_of(position)];
        CaretRect { page: line.page, x: line.x_of(position.offset), y: line.y, height: line.height }
    }

    /// The position closest to a point on a page (content-area coordinates).
    pub fn position_at(&self, page: usize, x: f32, y: f32) -> Position {
        let on_page = |l: &&Line| l.page == page;
        let line = self
            .lines
            .iter()
            .filter(on_page)
            .find(|l| y < l.y + l.height)
            .or_else(|| self.lines.iter().rev().find(on_page))
            .unwrap_or(&self.lines[self.lines.len() - 1]);
        Position::new(line.paragraph, line.offset_at_x(x))
    }

    /// Up/Down arrow. `goal_x` keeps the column steady across short lines.
    pub fn vertical_move(&self, from: Position, down: bool, goal_x: f32) -> Position {
        let index = self.line_index_of(from);
        let target = if down { index + 1 } else { index.wrapping_sub(1) };
        match self.lines.get(target) {
            Some(line) => Position::new(line.paragraph, line.offset_at_x(goal_x)),
            None if down => {
                let last = &self.lines[index];
                Position::new(last.paragraph, last.end)
            }
            None => Position::new(0, 0),
        }
    }

    pub fn line_start(&self, position: Position) -> Position {
        let line = &self.lines[self.line_index_of(position)];
        Position::new(line.paragraph, line.start)
    }

    pub fn line_end(&self, position: Position) -> Position {
        let line = &self.lines[self.line_index_of(position)];
        Position::new(line.paragraph, line.visual_end())
    }

    /// The checklist item whose checkbox is at this point, if any.
    pub fn checkbox_at(&self, page: usize, x: f32, y: f32) -> Option<usize> {
        let line = self.lines.iter().find(|l| l.page == page && y >= l.y && y < l.y + l.height)?;
        let in_box = x >= line.indent - LIST_INDENT && x < line.indent - 4.0;
        (line.kind == ParagraphKind::Checklist && line.start == 0 && in_box).then_some(line.paragraph)
    }
}

fn indent_of(kind: ParagraphKind) -> f32 {
    match kind {
        _ if kind.is_list() => LIST_INDENT,
        ParagraphKind::Quote => QUOTE_INDENT,
        ParagraphKind::Code => CODE_INDENT,
        _ => 0.0,
    }
}

/// The height of a line of text (with line spacing) and how much of it is
/// below the baseline. Mixed sizes share one baseline, like in Word.
fn text_metrics(paragraph: &Paragraph, start: usize, end: usize, measure: &impl Measure) -> (f32, f32) {
    let kind = paragraph.kind;
    let styles: Vec<TextStyle> = if start == end {
        // An empty line is as tall as the text that would be typed there.
        vec![paragraph.chars.get(start.saturating_sub(1)).map(|c| c.style).unwrap_or_default()]
    } else {
        paragraph.chars[start..end].iter().map(|c| c.style).collect()
    };
    let mut above: f32 = 0.0;
    let mut below: f32 = 0.0;
    for style in styles {
        let descent = measure.descent(style, kind);
        above = above.max(measure.line_height(style, kind) - descent);
        below = below.max(descent);
    }
    // Extra spacing goes above the text, keeping the baseline near the bottom.
    ((above + below) * paragraph.spacing.factor(), below)
}

/// Moves a line's glyphs for its alignment. Justified lines stretch their
/// spaces to fill the line. Returns where the caret sits if the line is empty.
fn align(glyphs: &mut [Glyph], alignment: Alignment, justify: bool, indent: f32, max_width: f32) -> f32 {
    // Trailing spaces hang past the edge, so they don't count.
    let visible = glyphs.iter().rposition(|g| g.ch != ' ').map_or(0, |i| i + 1);
    let used = glyphs[..visible].iter().map(|g| g.width).sum::<f32>();
    let free = (max_width - used).max(0.0);
    let offset = match alignment {
        Alignment::Center => free / 2.0,
        Alignment::Right => free,
        Alignment::Left | Alignment::Justify => 0.0,
    };
    let spaces = glyphs[..visible].iter().filter(|g| g.ch == ' ').count();
    let stretch = if justify && spaces > 0 { free / spaces as f32 } else { 0.0 };
    let mut x = indent + offset;
    for (i, glyph) in glyphs.iter_mut().enumerate() {
        glyph.x = x;
        if glyph.ch == ' ' && i < visible {
            glyph.width += stretch;
        }
        x += glyph.width;
    }
    indent + offset
}

fn paragraph_spacing(kind: ParagraphKind, next: Option<ParagraphKind>, line_height: f32) -> f32 {
    // List items and lines of code stay close together.
    if kind.is_list() || (kind == ParagraphKind::Code && next == Some(ParagraphKind::Code)) {
        line_height * 0.15
    } else {
        line_height * 0.5
    }
}

/// Splits one paragraph into lines no wider than `max_width`.
/// Breaks after spaces; a word longer than a whole line is broken anywhere.
/// Returns (start, end) character ranges. An empty paragraph still gets a line.
fn wrap(chars: &[char], widths: &[f32], max_width: f32) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let mut width = 0.0;
        let mut end = start;
        let mut last_break = None;
        while end < chars.len() {
            let is_space = chars[end] == ' ';
            // Spaces may hang past the margin; they are invisible anyway.
            if !is_space && end > start && width + widths[end] > max_width {
                break;
            }
            width += widths[end];
            end += 1;
            if is_space {
                last_break = Some(end);
            }
        }
        if end < chars.len() {
            if let Some(after_space) = last_break {
                end = after_space;
            }
        }
        lines.push((start, end));
        start = end;
    }
    if lines.is_empty() {
        lines.push((0, 0));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Paragraph;

    /// Every character is 10pt wide and every line 20pt tall.
    struct FixedMeasure;

    impl Measure for FixedMeasure {
        fn char_width(&self, _: char, _: TextStyle, _: ParagraphKind) -> f32 {
            10.0
        }
        fn line_height(&self, _: TextStyle, _: ParagraphKind) -> f32 {
            20.0
        }
    }

    /// A page with room for 10 characters per line and 3 lines per page.
    const SMALL_PAGE: PageSetup = PageSetup { width: 120.0, height: 80.0, margin: 10.0 };

    fn layout(texts: &[&str]) -> Layout {
        let paragraphs = texts
            .iter()
            .map(|t| Paragraph::from_text(ParagraphKind::Normal, t, TextStyle::default()))
            .collect();
        Layout::build(&Document::from_paragraphs(paragraphs), SMALL_PAGE, &FixedMeasure)
    }

    fn line_texts(layout: &Layout) -> Vec<String> {
        layout.lines.iter().map(|l| l.glyphs.iter().map(|g| g.ch).collect()).collect()
    }

    #[test]
    fn wraps_at_spaces() {
        let l = layout(&["hello big world"]);
        assert_eq!(line_texts(&l), ["hello big ", "world"]);
    }

    #[test]
    fn breaks_words_longer_than_a_line() {
        let l = layout(&["abcdefghijklmn"]);
        assert_eq!(line_texts(&l), ["abcdefghij", "klmn"]);
    }

    #[test]
    fn empty_paragraph_gets_a_line() {
        let l = layout(&[""]);
        assert_eq!(l.lines.len(), 1);
    }

    #[test]
    fn overflowing_lines_move_to_the_next_page() {
        let l = layout(&["a", "b", "c"]);
        // Each paragraph is 20pt + 10pt spacing, so the third one doesn't fit on 60pt.
        assert_eq!(l.lines.iter().map(|l| l.page).collect::<Vec<_>>(), [0, 0, 1]);
        assert_eq!(l.page_count, 2);
    }

    #[test]
    fn caret_at_wrap_point_sits_on_the_next_line() {
        let l = layout(&["hello big world"]);
        assert_eq!(l.line_index_of(Position::new(0, 10)), 1);
        assert_eq!(l.line_index_of(Position::new(0, 9)), 0);
    }

    #[test]
    fn clicking_maps_to_the_nearest_character() {
        let l = layout(&["hello big world"]);
        assert_eq!(l.position_at(0, 24.0, 5.0), Position::new(0, 2));
        assert_eq!(l.position_at(0, 999.0, 25.0), Position::new(0, 15));
        // Past the end of a wrapped line stays before the trailing space.
        assert_eq!(l.position_at(0, 999.0, 5.0), Position::new(0, 9));
    }

    #[test]
    fn vertical_move_keeps_the_column() {
        let l = layout(&["hello big world"]);
        assert_eq!(l.vertical_move(Position::new(0, 2), true, 20.0), Position::new(0, 12));
        assert_eq!(l.vertical_move(Position::new(0, 12), false, 20.0), Position::new(0, 2));
    }

    #[test]
    fn images_widen_and_heighten_their_line() {
        let mut document = Document::from_paragraphs(vec![Paragraph::from_text(
            ParagraphKind::Normal,
            "ab",
            TextStyle::default(),
        )]);
        let image = crate::document::Image::fitted(vec![], 30.0, 50.0, 100.0, 100.0);
        document.insert_image(Position::new(0, 1), image);
        let l = Layout::build(&document, SMALL_PAGE, &FixedMeasure);
        let line = &l.lines[0];
        assert_eq!(line.height, 50.0);
        assert_eq!(line.text_height, 20.0);
        assert_eq!(line.x_of(2), 40.0);
    }

    fn layout_paragraph(paragraph: Paragraph) -> Layout {
        Layout::build(&Document::from_paragraphs(vec![paragraph]), SMALL_PAGE, &FixedMeasure)
    }

    #[test]
    fn centered_and_right_aligned_lines_move_over() {
        let mut p = Paragraph::from_text(ParagraphKind::Normal, "abcd", TextStyle::default());
        p.alignment = Alignment::Center;
        assert_eq!(layout_paragraph(p.clone()).lines[0].glyphs[0].x, 30.0);
        p.alignment = Alignment::Right;
        assert_eq!(layout_paragraph(p).lines[0].glyphs[0].x, 60.0);
    }

    #[test]
    fn justified_lines_fill_the_width_except_the_last() {
        let mut p = Paragraph::from_text(ParagraphKind::Normal, "ab cd efgh ij", TextStyle::default());
        p.alignment = Alignment::Justify;
        let l = layout_paragraph(p);
        let first = &l.lines[0];
        let last_glyph = first.glyphs.iter().rfind(|g| g.ch != ' ').unwrap();
        assert_eq!(last_glyph.x + last_glyph.width, 100.0);
        assert_eq!(l.lines[1].glyphs[0].x, 0.0);
    }

    #[test]
    fn line_spacing_makes_lines_taller() {
        let mut p = Paragraph::from_text(ParagraphKind::Normal, "a", TextStyle::default());
        p.spacing = crate::document::LineSpacing::Double;
        let line = &layout_paragraph(p).lines[0];
        assert_eq!(line.height, 40.0);
        assert_eq!(line.baseline, 35.0);
    }

    #[test]
    fn clicking_a_checkbox_finds_its_item() {
        let p = Paragraph::from_text(ParagraphKind::Checklist, "task", TextStyle::default());
        let l = layout_paragraph(p);
        assert_eq!(l.checkbox_at(0, 8.0, 5.0), Some(0));
        assert_eq!(l.checkbox_at(0, 30.0, 5.0), None);
    }

    #[test]
    fn numbered_lists_count_and_restart() {
        let numbered = |t| Paragraph::from_text(ParagraphKind::Numbered, t, TextStyle::default());
        let normal = Paragraph::from_text(ParagraphKind::Normal, "x", TextStyle::default());
        let document = Document::from_paragraphs(vec![numbered("a"), numbered("b"), normal, numbered("c")]);
        let l = Layout::build(&document, PageSetup::LETTER, &FixedMeasure);
        let markers: Vec<_> = l.lines.iter().map(|l| l.marker.clone()).collect();
        assert_eq!(markers, [Some("1.".into()), Some("2.".into()), None, Some("1.".into())]);
    }
}
