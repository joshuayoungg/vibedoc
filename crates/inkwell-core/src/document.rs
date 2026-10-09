//! The document model: paragraphs made of styled characters.
//!
//! Each character carries its own style. That costs a little memory but makes
//! every edit (insert, delete, restyle) a plain `Vec` operation.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleFlag {
    Bold,
    Italic,
    Underline,
}

impl TextStyle {
    pub fn get(self, flag: StyleFlag) -> bool {
        match flag {
            StyleFlag::Bold => self.bold,
            StyleFlag::Italic => self.italic,
            StyleFlag::Underline => self.underline,
        }
    }

    pub fn set(&mut self, flag: StyleFlag, value: bool) {
        match flag {
            StyleFlag::Bold => self.bold = value,
            StyleFlag::Italic => self.italic = value,
            StyleFlag::Underline => self.underline = value,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ParagraphKind {
    #[default]
    Normal,
    Heading1,
    Heading2,
    Heading3,
    Bullet,
    Numbered,
}

impl ParagraphKind {
    pub const ALL: [ParagraphKind; 6] = [
        ParagraphKind::Normal,
        ParagraphKind::Heading1,
        ParagraphKind::Heading2,
        ParagraphKind::Heading3,
        ParagraphKind::Bullet,
        ParagraphKind::Numbered,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ParagraphKind::Normal => "Normal",
            ParagraphKind::Heading1 => "Heading 1",
            ParagraphKind::Heading2 => "Heading 2",
            ParagraphKind::Heading3 => "Heading 3",
            ParagraphKind::Bullet => "Bulleted list",
            ParagraphKind::Numbered => "Numbered list",
        }
    }

    pub fn is_list(self) -> bool {
        matches!(self, ParagraphKind::Bullet | ParagraphKind::Numbered)
    }

    pub fn is_heading(self) -> bool {
        matches!(self, ParagraphKind::Heading1 | ParagraphKind::Heading2 | ParagraphKind::Heading3)
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
}

impl Paragraph {
    pub fn new(kind: ParagraphKind) -> Self {
        Paragraph { kind, chars: Vec::new() }
    }

    pub fn from_text(kind: ParagraphKind, text: &str, style: TextStyle) -> Self {
        let chars = text.chars().map(|ch| StyledChar { ch, style }).collect();
        Paragraph { kind, chars }
    }

    pub fn len(&self) -> usize {
        self.chars.len()
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub paragraphs: Vec<Paragraph>,
}

impl Default for Document {
    fn default() -> Self {
        Document { paragraphs: vec![Paragraph::default()] }
    }
}

impl Document {
    /// A document always has at least one paragraph, so the caret has a home.
    pub fn from_paragraphs(paragraphs: Vec<Paragraph>) -> Self {
        if paragraphs.is_empty() {
            Document::default()
        } else {
            Document { paragraphs }
        }
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
        // Like Word: a heading is followed by normal text, but lists continue.
        let kind = if paragraph.kind.is_heading() { ParagraphKind::Normal } else { paragraph.kind };
        self.paragraphs.insert(at.paragraph + 1, Paragraph { kind, chars: tail });
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
            text.extend(chars.iter().map(|c| c.ch));
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
    fn word_at_finds_word_boundaries() {
        let d = doc(&["hello big world"]);
        assert_eq!(d.word_at(Position::new(0, 7)), (Position::new(0, 6), Position::new(0, 9)));
    }
}
