use std::io::Cursor;

use vibedoc_core::{
    Alignment, Color, Document, Font, LineSpacing, Paragraph, ParagraphKind, Script, StyledChar, TextStyle,
};

#[test]
fn saving_then_opening_keeps_everything_vibedoc_supports() {
    let bold_italic = TextStyle { bold: true, italic: true, underline: false, ..Default::default() };
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
    let mut original = original;
    original.paragraphs.extend(formatted_paragraphs());

    let mut bytes = Cursor::new(Vec::new());
    vibedoc_docx::write_to(&original, &mut bytes).unwrap();
    bytes.set_position(0);
    let reopened = vibedoc_docx::read_from(bytes).unwrap();

    assert_eq!(reopened, original);
}

/// Fonts, sizes, colors, alignment, spacing, and the newer paragraph kinds.
fn formatted_paragraphs() -> Vec<Paragraph> {
    let fancy = TextStyle {
        strikethrough: true,
        font: Font::Lora,
        size: Some(36),
        color: Some(Color::rgb(0xC0, 0x30, 0x40)),
        highlight: Some(Color::rgb(0xFF, 0xF1, 0xA8)),
        ..Default::default()
    };
    let raised = TextStyle { script: Script::Superscript, font: Font::JetBrainsMono, ..Default::default() };
    let mut centered = Paragraph::from_text(ParagraphKind::Normal, "Centered ", TextStyle::default());
    centered.chars.extend("fancy".chars().map(|ch| StyledChar { ch, style: fancy }));
    centered.chars.extend("2".chars().map(|ch| StyledChar { ch, style: raised }));
    centered.alignment = Alignment::Center;
    centered.spacing = LineSpacing::OneAndHalf;
    let mut justified = Paragraph::from_text(ParagraphKind::Quote, "A justified quote", TextStyle::default());
    justified.alignment = Alignment::Justify;
    let mut done = Paragraph::from_text(ParagraphKind::Checklist, "done", TextStyle::default());
    done.checked = true;
    vec![
        centered,
        justified,
        Paragraph::from_text(ParagraphKind::Code, "let x = 1;", TextStyle::default()),
        done,
        Paragraph::from_text(ParagraphKind::Checklist, "to do", TextStyle::default()),
    ]
}
