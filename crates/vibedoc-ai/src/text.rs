//! How the AI sees the document, and how its text becomes styled characters.
//!
//! Both directions use a tiny Markdown subset: `**bold**`, `*italic*`, and
//! `__underline__`. Models already write this naturally.

use inkwell_core::{is_image_char, Document, ParagraphKind, Selection, StyledChar, TextStyle};

/// The document as numbered lines, e.g. `[2] - a **bold** bullet`.
pub fn describe_document(document: &Document) -> String {
    let mut text = String::new();
    for (index, paragraph) in document.paragraphs.iter().enumerate() {
        let prefix = match paragraph.kind {
            ParagraphKind::Normal => "",
            ParagraphKind::Heading1 => "# ",
            ParagraphKind::Heading2 => "## ",
            ParagraphKind::Heading3 => "### ",
            ParagraphKind::Bullet => "- ",
            ParagraphKind::Numbered => "1. ",
        };
        let content = to_markdown(&paragraph.chars);
        let content = if content.is_empty() { "(empty)".to_string() } else { content };
        text.push_str(&format!("[{index}] {prefix}{content}\n"));
    }
    text
}

/// Where the user's caret or selection is, in words the AI can act on.
pub fn describe_selection(document: &Document, selection: Selection) -> String {
    let (start, end) = (selection.start(), selection.end());
    if selection.is_empty() {
        return format!(
            "Nothing is selected. The caret is in paragraph {} at character {}.",
            start.paragraph, start.offset
        );
    }
    let selected = document.text_in(start, end);
    format!(
        "The user has selected from paragraph {} character {} to paragraph {} character {}: \"{}\"",
        start.paragraph, start.offset, end.paragraph, end.offset, selected
    )
}

fn to_markdown(chars: &[StyledChar]) -> String {
    let mut text = String::new();
    let mut current = TextStyle::default();
    for c in chars {
        if c.style != current {
            text.push_str(&markers_between(current, c.style));
            current = c.style;
        }
        if is_image_char(c.ch) {
            text.push_str("[image]");
        } else {
            text.push(c.ch);
        }
    }
    text.push_str(&markers_between(current, TextStyle::default()));
    text
}

/// The markers that switch formatting from one style to the next.
fn markers_between(from: TextStyle, to: TextStyle) -> String {
    let mut markers = String::new();
    if from.underline != to.underline {
        markers.push_str("__");
    }
    if from.bold != to.bold {
        markers.push_str("**");
    }
    if from.italic != to.italic {
        markers.push('*');
    }
    markers
}

/// Turns `plain **bold** *italic* __underline__` into styled characters.
pub fn parse_markdown(text: &str) -> Vec<StyledChar> {
    let mut chars = Vec::new();
    let mut style = TextStyle::default();
    let mut rest = text;
    while let Some(ch) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix("**") {
            style.bold = !style.bold;
            rest = after;
        } else if let Some(after) = rest.strip_prefix("__") {
            style.underline = !style.underline;
            rest = after;
        } else if let Some(after) = rest.strip_prefix('*') {
            style.italic = !style.italic;
            rest = after;
        } else {
            chars.push(StyledChar { ch, style });
            rest = &rest[ch.len_utf8()..];
        }
    }
    chars
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell_core::{Paragraph, Position};

    #[test]
    fn markdown_round_trips() {
        let source = "plain **bold** *italic* __under__ ***both***";
        let chars = parse_markdown(source);
        assert!(chars.iter().find(|c| c.ch == 'b').unwrap().style.bold);
        assert_eq!(to_markdown(&chars), source);
    }

    #[test]
    fn describes_paragraphs_with_numbers_and_kinds() {
        let mut document = Document::from_paragraphs(vec![
            Paragraph::from_text(ParagraphKind::Heading1, "Title", TextStyle::default()),
            Paragraph::new(ParagraphKind::Normal),
            Paragraph::from_text(ParagraphKind::Bullet, "item", TextStyle::default()),
        ]);
        let image = inkwell_core::Image::fitted(vec![], 10.0, 10.0, 10.0, 10.0);
        document.insert_image(Position::new(1, 0), image);
        assert_eq!(describe_document(&document), "[0] # Title\n[1] [image]\n[2] - item\n");
    }

    #[test]
    fn describes_the_selection() {
        let document = Document::from_paragraphs(vec![Paragraph::from_text(
            ParagraphKind::Normal,
            "hello world",
            TextStyle::default(),
        )]);
        let selection = Selection { anchor: Position::new(0, 6), cursor: Position::new(0, 11) };
        assert!(describe_selection(&document, selection).ends_with("\"world\""));
    }
}
