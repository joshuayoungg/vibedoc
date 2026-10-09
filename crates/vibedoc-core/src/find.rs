//! Case-insensitive find. Matches never cross paragraph boundaries.

use crate::document::{Document, Position};

pub fn find_all(document: &Document, query: &str) -> Vec<(Position, Position)> {
    let needle: Vec<char> = query.chars().map(fold_case).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for (index, paragraph) in document.paragraphs.iter().enumerate() {
        let haystack: Vec<char> = paragraph.chars.iter().map(|c| fold_case(c.ch)).collect();
        let mut offset = 0;
        while offset + needle.len() <= haystack.len() {
            if haystack[offset..offset + needle.len()] == needle[..] {
                matches.push((Position::new(index, offset), Position::new(index, offset + needle.len())));
                offset += needle.len();
            } else {
                offset += 1;
            }
        }
    }
    matches
}

/// Lowercases one character, keeping it one character so offsets stay valid.
fn fold_case(ch: char) -> char {
    ch.to_lowercase().next().unwrap_or(ch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Paragraph, ParagraphKind, TextStyle};

    #[test]
    fn finds_matches_ignoring_case() {
        let document = Document::from_paragraphs(vec![
            Paragraph::from_text(ParagraphKind::Normal, "Rust is rusty", TextStyle::default()),
            Paragraph::from_text(ParagraphKind::Normal, "no match", TextStyle::default()),
            Paragraph::from_text(ParagraphKind::Normal, "RUST", TextStyle::default()),
        ]);
        let found = find_all(&document, "rust");
        assert_eq!(
            found,
            vec![
                (Position::new(0, 0), Position::new(0, 4)),
                (Position::new(0, 8), Position::new(0, 12)),
                (Position::new(2, 0), Position::new(2, 4)),
            ]
        );
    }

    #[test]
    fn empty_query_finds_nothing() {
        assert!(find_all(&Document::default(), "").is_empty());
    }
}
