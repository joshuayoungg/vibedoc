//! The document model: paragraphs made of styled characters.
//!
//! Each character carries its own style. That costs a little memory but makes
//! every edit (insert, delete, restyle) a plain `Vec` operation.

use std::collections::HashMap;
use std::sync::Arc;

/// How a run of text looks. Every character carries one of these.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub script: Script,
    pub font: Font,
    /// Size in half-points, like Word. `None` uses the paragraph kind's size.
    pub size: Option<u16>,
    /// `None` is automatic: dark on light pages, light on dark ones.
    pub color: Option<Color>,
    pub highlight: Option<Color>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleFlag {
    Bold,
    Italic,
    Underline,
    Strikethrough,
}

impl TextStyle {
    pub fn get(self, flag: StyleFlag) -> bool {
        match flag {
            StyleFlag::Bold => self.bold,
            StyleFlag::Italic => self.italic,
            StyleFlag::Underline => self.underline,
            StyleFlag::Strikethrough => self.strikethrough,
        }
    }

    pub fn set(&mut self, flag: StyleFlag, value: bool) {
        match flag {
            StyleFlag::Bold => self.bold = value,
            StyleFlag::Italic => self.italic = value,
            StyleFlag::Underline => self.underline = value,
            StyleFlag::Strikethrough => self.strikethrough = value,
        }
    }

    /// The size in points, falling back to the paragraph kind's size.
    pub fn points(self, kind: ParagraphKind) -> f32 {
        self.size.map_or(kind.font_size(), |half_points| f32::from(half_points) / 2.0)
    }

    /// The typeface and weight text is drawn in. Code is always monospaced,
    /// and headings are always bold.
    pub fn face(self, kind: ParagraphKind) -> (Font, bool) {
        let font = if kind == ParagraphKind::Code { Font::JetBrainsMono } else { self.font };
        (font, self.bold || kind.is_heading())
    }

    /// The size glyphs are drawn at: smaller for raised and lowered text.
    pub fn glyph_points(self, kind: ParagraphKind) -> f32 {
        const SCRIPT_SCALE: f32 = 0.65;
        let scale = if self.script == Script::Normal { 1.0 } else { SCRIPT_SCALE };
        self.points(kind) * scale
    }

    /// How far the baseline is raised for superscript (negative: lowered for subscript), in points.
    pub fn rise(self, kind: ParagraphKind) -> f32 {
        let size = self.points(kind);
        match self.script {
            Script::Normal => 0.0,
            Script::Superscript => size * 0.35,
            Script::Subscript => -size * 0.12,
        }
    }
}

/// Raised or lowered text, like x² or H₂O.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Script {
    #[default]
    Normal,
    Superscript,
    Subscript,
}

/// The typefaces VibeDoc ships with, so documents look the same everywhere.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Font {
    #[default]
    Inter,
    Poppins,
    Lora,
    Merriweather,
    PlayfairDisplay,
    JetBrainsMono,
}

impl Font {
    pub const ALL: [Font; 6] = [
        Font::Inter,
        Font::Poppins,
        Font::Lora,
        Font::Merriweather,
        Font::PlayfairDisplay,
        Font::JetBrainsMono,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Font::Inter => "Inter",
            Font::Poppins => "Poppins",
            Font::Lora => "Lora",
            Font::Merriweather => "Merriweather",
            Font::PlayfairDisplay => "Playfair Display",
            Font::JetBrainsMono => "JetBrains Mono",
        }
    }

    /// The closest bundled font for a font name found in a .docx file.
    pub fn closest_to(name: &str) -> Font {
        let name = name.to_lowercase();
        if let Some(font) = Font::ALL.into_iter().find(|f| f.name().to_lowercase() == name) {
            return font;
        }
        let any = |words: &[&str]| words.iter().any(|w| name.contains(w));
        if any(&["mono", "courier", "consolas", "menlo", "monaco", "code"]) {
            Font::JetBrainsMono
        } else if any(&["playfair", "didot", "bodoni"]) {
            Font::PlayfairDisplay
        } else if any(&["times", "georgia", "cambria", "garamond", "palatino", "book", "serif", "minion"]) {
            Font::Merriweather
        } else if any(&["poppins", "futura", "century gothic", "avenir", "montserrat"]) {
            Font::Poppins
        } else {
            Font::Inter
        }
    }
}

/// A color as red, green, and blue bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b }
    }

    /// "1F2937", as Word writes colors.
    pub fn hex(self) -> String {
        format!("{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn from_hex(hex: &str) -> Option<Color> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
        Some(Color { r: byte(0)?, g: byte(2)?, b: byte(4)? })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ParagraphKind {
    #[default]
    Normal,
    Heading1,
    Heading2,
    Heading3,
    Quote,
    Code,
    Bullet,
    Numbered,
    Checklist,
}

impl ParagraphKind {
    pub const ALL: [ParagraphKind; 9] = [
        ParagraphKind::Normal,
        ParagraphKind::Heading1,
        ParagraphKind::Heading2,
        ParagraphKind::Heading3,
        ParagraphKind::Quote,
        ParagraphKind::Code,
        ParagraphKind::Bullet,
        ParagraphKind::Numbered,
        ParagraphKind::Checklist,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ParagraphKind::Normal => "Normal",
            ParagraphKind::Heading1 => "Heading 1",
            ParagraphKind::Heading2 => "Heading 2",
            ParagraphKind::Heading3 => "Heading 3",
            ParagraphKind::Quote => "Quote",
            ParagraphKind::Code => "Code",
            ParagraphKind::Bullet => "Bulleted list",
            ParagraphKind::Numbered => "Numbered list",
            ParagraphKind::Checklist => "Checklist",
        }
    }

    /// Lists continue when you press Enter, and leave the list on an empty item.
    pub fn is_list(self) -> bool {
        matches!(self, ParagraphKind::Bullet | ParagraphKind::Numbered | ParagraphKind::Checklist)
    }

    pub fn is_heading(self) -> bool {
        matches!(self, ParagraphKind::Heading1 | ParagraphKind::Heading2 | ParagraphKind::Heading3)
    }

    /// Text size in points when a character doesn't set its own.
    pub fn font_size(self) -> f32 {
        match self {
            ParagraphKind::Heading1 => 22.0,
            ParagraphKind::Heading2 => 17.0,
            ParagraphKind::Heading3 => 14.0,
            ParagraphKind::Code => 11.0,
            _ => 12.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Space between lines, as a multiple of single spacing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LineSpacing {
    #[default]
    Single,
    Relaxed,
    OneAndHalf,
    Double,
}

impl LineSpacing {
    pub const ALL: [LineSpacing; 4] =
        [LineSpacing::Single, LineSpacing::Relaxed, LineSpacing::OneAndHalf, LineSpacing::Double];

    pub fn factor(self) -> f32 {
        match self {
            LineSpacing::Single => 1.0,
            LineSpacing::Relaxed => 1.15,
            LineSpacing::OneAndHalf => 1.5,
            LineSpacing::Double => 2.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LineSpacing::Single => "1.0",
            LineSpacing::Relaxed => "1.15",
            LineSpacing::OneAndHalf => "1.5",
            LineSpacing::Double => "2.0",
        }
    }

    /// The closest choice to a factor, e.g. from a .docx file.
    pub fn closest_to(factor: f32) -> LineSpacing {
        LineSpacing::ALL
            .into_iter()
            .min_by(|a, b| (a.factor() - factor).abs().total_cmp(&(b.factor() - factor).abs()))
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyledChar {
    pub ch: char,
    pub style: TextStyle,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Paragraph {
    pub kind: ParagraphKind,
    pub chars: Vec<StyledChar>,
    pub alignment: Alignment,
    pub spacing: LineSpacing,
    /// Whether a checklist item is ticked.
    pub checked: bool,
}

impl Paragraph {
    pub fn new(kind: ParagraphKind) -> Self {
        Paragraph { kind, ..Default::default() }
    }

    pub fn from_text(kind: ParagraphKind, text: &str, style: TextStyle) -> Self {
        let chars = text.chars().map(|ch| StyledChar { ch, style }).collect();
        Paragraph { kind, chars, ..Default::default() }
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    /// The paragraph's formatting with no text.
    fn clone_format(&self) -> Paragraph {
        Paragraph { chars: Vec::new(), ..*self }
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn text(&self) -> String {
        self.chars.iter().map(|c| c.ch).collect()
    }

    /// Groups neighbouring characters that share a style, e.g. for saving.
    pub fn runs(&self) -> Vec<(TextStyle, String)> {
        let mut runs: Vec<(TextStyle, String)> = Vec::new();
        for c in &self.chars {
            match runs.last_mut() {
                Some((style, text)) if *style == c.style => text.push(c.ch),
                _ => runs.push((c.style, c.ch.to_string())),
            }
        }
        runs
    }
}

/// A spot between two characters. `offset` counts characters, not bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub paragraph: usize,
    pub offset: usize,
}

impl Position {
    pub fn new(paragraph: usize, offset: usize) -> Self {
        Position { paragraph, offset }
    }
}

/// `anchor` is where the selection started, `cursor` is where the caret is now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Position,
    pub cursor: Position,
}

impl Selection {
    pub fn caret(position: Position) -> Self {
        Selection { anchor: position, cursor: position }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }

    pub fn start(&self) -> Position {
        self.anchor.min(self.cursor)
    }

    pub fn end(&self) -> Position {
        self.anchor.max(self.cursor)
    }
}

/// A picture in the document, shown at `width` x `height` points.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// The encoded file (PNG or JPEG). Shared, so undo snapshots stay cheap.
    pub bytes: Arc<Vec<u8>>,
    pub width: f32,
    pub height: f32,
}

impl Image {
    /// Shrinks the natural size (in points) to fit the box, keeping its shape.
    pub fn fitted(bytes: Vec<u8>, width: f32, height: f32, max_width: f32, max_height: f32) -> Image {
        let scale = (max_width / width).min(max_height / height).min(1.0);
        Image { bytes: Arc::new(bytes), width: width * scale, height: height * scale }
    }
}

/// Images sit in the text as one placeholder character from Unicode's private
/// use area, so editing, undo, and line wrapping treat them like letters.
/// The character is the key into `Document::images`.
const FIRST_IMAGE_CHAR: u32 = 0xF0000;
const LAST_IMAGE_CHAR: u32 = 0xFFFFD;

pub fn is_image_char(ch: char) -> bool {
    (FIRST_IMAGE_CHAR..=LAST_IMAGE_CHAR).contains(&(ch as u32))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub paragraphs: Vec<Paragraph>,
    pub images: HashMap<char, Image>,
}

impl Default for Document {
    fn default() -> Self {
        Document { paragraphs: vec![Paragraph::default()], images: HashMap::new() }
    }
}

impl Document {
    /// A document always has at least one paragraph, so the caret has a home.
    pub fn from_paragraphs(paragraphs: Vec<Paragraph>) -> Self {
        if paragraphs.is_empty() {
            Document::default()
        } else {
            Document { paragraphs, images: HashMap::new() }
        }
    }

    pub fn image(&self, ch: char) -> Option<&Image> {
        self.images.get(&ch)
    }

    /// Adds an image to the image table and returns its placeholder character.
    pub fn add_image(&mut self, image: Image) -> char {
        let next = self.images.keys().map(|&ch| ch as u32 + 1).max().unwrap_or(FIRST_IMAGE_CHAR);
        let ch = char::from_u32(next.min(LAST_IMAGE_CHAR)).expect("private use area is valid");
        self.images.insert(ch, image);
        ch
    }

    /// Inserts an image at `at`. Returns the position just after it.
    pub fn insert_image(&mut self, at: Position, image: Image) -> Position {
        let ch = self.add_image(image);
        self.paragraphs[at.paragraph].chars.insert(at.offset, StyledChar { ch, style: TextStyle::default() });
        Position::new(at.paragraph, at.offset + 1)
    }

    pub fn start(&self) -> Position {
        Position::new(0, 0)
    }

    pub fn end(&self) -> Position {
        let last = self.paragraphs.len() - 1;
        Position::new(last, self.paragraphs[last].len())
    }

    pub fn clamp(&self, position: Position) -> Position {
        let paragraph = position.paragraph.min(self.paragraphs.len() - 1);
        let offset = position.offset.min(self.paragraphs[paragraph].len());
        Position::new(paragraph, offset)
    }

    pub fn paragraph_len(&self, paragraph: usize) -> usize {
        self.paragraphs[paragraph].len()
    }

    /// Inserts text (newlines start new paragraphs). Returns the position after it.
    pub fn insert_text(&mut self, at: Position, text: &str, style: TextStyle) -> Position {
        let mut position = at;
        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                position = self.split_paragraph(position);
            }
            let line = line.trim_end_matches('\r');
            let chars = line.chars().map(|ch| StyledChar { ch, style });
            let paragraph = &mut self.paragraphs[position.paragraph];
            let count = line.chars().count();
            paragraph.chars.splice(position.offset..position.offset, chars);
            position.offset += count;
        }
        position
    }

    /// Splits a paragraph in two at `at` (what Enter does). Returns the start of the new one.
    pub fn split_paragraph(&mut self, at: Position) -> Position {
        let paragraph = &mut self.paragraphs[at.paragraph];
        let tail = paragraph.chars.split_off(at.offset);
        // Like Word: a heading is followed by normal text, but lists continue
        // (unticked) and the alignment and spacing carry over.
        let kind = if paragraph.kind.is_heading() { ParagraphKind::Normal } else { paragraph.kind };
        let next = Paragraph { kind, chars: tail, checked: false, ..paragraph.clone_format() };
        self.paragraphs.insert(at.paragraph + 1, next);
        Position::new(at.paragraph + 1, 0)
    }

    /// Deletes everything between `start` and `end`, merging paragraphs if needed.
    pub fn delete(&mut self, start: Position, end: Position) {
        let (start, end) = (start.min(end), start.max(end));
        if start.paragraph == end.paragraph {
            self.paragraphs[start.paragraph].chars.drain(start.offset..end.offset);
            return;
        }
        let tail = self.paragraphs[end.paragraph].chars.split_off(end.offset);
        self.paragraphs.drain(start.paragraph + 1..=end.paragraph);
        let first = &mut self.paragraphs[start.paragraph];
        first.chars.truncate(start.offset);
        first.chars.extend(tail);
    }

    /// Plain text of a range, with paragraphs joined by newlines.
    pub fn text_in(&self, start: Position, end: Position) -> String {
        let mut text = String::new();
        for paragraph in start.paragraph..=end.paragraph {
            if paragraph > start.paragraph {
                text.push('\n');
            }
            let chars = self.chars_of(paragraph, start, end);
            text.extend(chars.iter().map(|c| c.ch).filter(|&ch| !is_image_char(ch)));
        }
        text
    }

    /// The style a newly typed character should get at `position`:
    /// the style of the character just before it, like most editors.
    pub fn style_at(&self, position: Position) -> TextStyle {
        let chars = &self.paragraphs[position.paragraph].chars;
        if position.offset > 0 {
            chars[position.offset - 1].style
        } else {
            chars.first().map(|c| c.style).unwrap_or_default()
        }
    }

    pub fn all_chars_in(&self, start: Position, end: Position, check: impl Fn(TextStyle) -> bool) -> bool {
        self.chars_in(start, end).all(|c| check(c.style))
    }

    pub fn restyle(&mut self, start: Position, end: Position, change: impl Fn(&mut TextStyle)) {
        for paragraph in start.paragraph..=end.paragraph {
            let (from, to) = self.offsets_in(paragraph, start, end);
            for c in &mut self.paragraphs[paragraph].chars[from..to] {
                change(&mut c.style);
            }
        }
    }

    pub fn chars_in(&self, start: Position, end: Position) -> impl Iterator<Item = &StyledChar> {
        (start.paragraph..=end.paragraph).flat_map(move |paragraph| self.chars_of(paragraph, start, end))
    }

    /// The part of one paragraph that falls inside the range `start..end`.
    fn chars_of(&self, paragraph: usize, start: Position, end: Position) -> &[StyledChar] {
        let (from, to) = self.offsets_in(paragraph, start, end);
        &self.paragraphs[paragraph].chars[from..to]
    }

    fn offsets_in(&self, paragraph: usize, start: Position, end: Position) -> (usize, usize) {
        let from = if paragraph == start.paragraph { start.offset } else { 0 };
        let to = if paragraph == end.paragraph { end.offset } else { self.paragraph_len(paragraph) };
        (from, to)
    }

    pub fn word_count(&self) -> usize {
        self.paragraphs.iter().map(|p| p.text().split_whitespace().count()).sum()
    }

    /// The word around `position`, as a (start, end) pair within one paragraph.
    pub fn word_at(&self, position: Position) -> (Position, Position) {
        let chars = &self.paragraphs[position.paragraph].chars;
        let is_word = |i: usize| chars[i].ch.is_alphanumeric() || chars[i].ch == '_';
        let mut start = position.offset;
        while start > 0 && is_word(start - 1) {
            start -= 1;
        }
        let mut end = position.offset;
        while end < chars.len() && is_word(end) {
            end += 1;
        }
        (Position::new(position.paragraph, start), Position::new(position.paragraph, end))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(lines: &[&str]) -> Document {
        let paragraphs = lines
            .iter()
            .map(|l| Paragraph::from_text(ParagraphKind::Normal, l, TextStyle::default()))
            .collect();
        Document::from_paragraphs(paragraphs)
    }

    fn texts(document: &Document) -> Vec<String> {
        document.paragraphs.iter().map(|p| p.text()).collect()
    }

    #[test]
    fn insert_text_with_newlines_splits_paragraphs() {
        let mut d = doc(&["hello world"]);
        let end = d.insert_text(Position::new(0, 5), " there\nnew", TextStyle::default());
        assert_eq!(texts(&d), ["hello there", "new world"]);
        assert_eq!(end, Position::new(1, 3));
    }

    #[test]
    fn delete_across_paragraphs_merges_them() {
        let mut d = doc(&["abc", "def", "ghi"]);
        d.delete(Position::new(0, 1), Position::new(2, 2));
        assert_eq!(texts(&d), ["ai"]);
    }

    #[test]
    fn text_in_joins_paragraphs_with_newlines() {
        let d = doc(&["abc", "def"]);
        assert_eq!(d.text_in(Position::new(0, 1), Position::new(1, 2)), "bc\nde");
    }

    #[test]
    fn splitting_a_heading_makes_a_normal_paragraph() {
        let mut d = Document::from_paragraphs(vec![Paragraph::from_text(
            ParagraphKind::Heading1,
            "Title",
            TextStyle::default(),
        )]);
        d.split_paragraph(Position::new(0, 5));
        assert_eq!(d.paragraphs[1].kind, ParagraphKind::Normal);
    }

    #[test]
    fn runs_group_matching_styles() {
        let bold = TextStyle { bold: true, ..Default::default() };
        let mut p = Paragraph::from_text(ParagraphKind::Normal, "ab", TextStyle::default());
        p.chars.push(StyledChar { ch: 'c', style: bold });
        assert_eq!(p.runs(), vec![(TextStyle::default(), "ab".into()), (bold, "c".into())]);
    }

    #[test]
    fn images_are_single_characters_left_out_of_plain_text() {
        let mut d = doc(&["ab"]);
        let image = Image::fitted(vec![1, 2, 3], 800.0, 400.0, 400.0, 600.0);
        assert_eq!((image.width, image.height), (400.0, 200.0));
        let after = d.insert_image(Position::new(0, 1), image.clone());
        assert_eq!(after, Position::new(0, 2));
        let ch = d.paragraphs[0].chars[1].ch;
        assert!(is_image_char(ch));
        assert_eq!(d.image(ch), Some(&image));
        assert_eq!(d.text_in(d.start(), d.end()), "ab");
        assert_ne!(d.add_image(image), ch);
    }

    #[test]
    fn word_at_finds_word_boundaries() {
        let d = doc(&["hello big world"]);
        assert_eq!(d.word_at(Position::new(0, 7)), (Position::new(0, 6), Position::new(0, 9)));
    }
}
