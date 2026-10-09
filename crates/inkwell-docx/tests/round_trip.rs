use std::io::Cursor;

use inkwell_core::{Document, Paragraph, ParagraphKind, StyledChar, TextStyle};

#[test]
fn saving_then_opening_keeps_everything_inkwell_supports() {
    let bold_italic = TextStyle { bold: true, italic: true, underline: false };
    let underline = TextStyle { underline: true, ..Default::default() };
    let mut mixed = Paragraph::from_text(ParagraphKind::Normal, "Plain <&> then\ttab ", TextStyle::default());
    mixed.chars.extend("bold".chars().map(|ch| StyledChar { ch, style: bold_italic }));
    mixed.chars.extend(" under".chars().map(|ch| StyledChar { ch, style: underline }));

    let original = Document::from_paragraphs(vec![
        Paragraph::from_text(ParagraphKind::Heading1, "Title", TextStyle::default()),
        Paragraph::from_text(ParagraphKind::Heading2, "Section", TextStyle::default()),
        Paragraph::from_text(ParagraphKind::Heading3, "Subsection", TextStyle::default()),
        mixed,
        Paragraph::new(ParagraphKind::Normal),
        Paragraph::from_text(ParagraphKind::Bullet, "bullet", TextStyle::default()),
        Paragraph::from_text(ParagraphKind::Numbered, "first", TextStyle::default()),
        Paragraph::from_text(ParagraphKind::Numbered, "second", TextStyle::default()),
        Paragraph::from_text(ParagraphKind::Normal, "émoji 🦀 ok", TextStyle::default()),
    ]);

    let mut bytes = Cursor::new(Vec::new());
    inkwell_docx::write_to(&original, &mut bytes).unwrap();
    bytes.set_position(0);
    let reopened = inkwell_docx::read_from(bytes).unwrap();

    assert_eq!(reopened, original);
}
