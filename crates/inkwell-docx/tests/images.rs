use std::io::Cursor;

use inkwell_core::{Document, Image, Paragraph, ParagraphKind, Position, TextStyle};

#[test]
fn images_survive_saving_and_opening() {
    let mut original = Document::from_paragraphs(vec![Paragraph::from_text(
        ParagraphKind::Normal,
        "before after",
        TextStyle::default(),
    )]);
    let png_bytes = vec![0x89, b'P', b'N', b'G', 1, 2, 3];
    original.insert_image(Position::new(0, 7), Image::fitted(png_bytes, 200.0, 100.0, 468.0, 648.0));

    let mut bytes = Cursor::new(Vec::new());
    inkwell_docx::write_to(&original, &mut bytes).unwrap();
    bytes.set_position(0);
    let reopened = inkwell_docx::read_from(bytes).unwrap();

    assert_eq!(reopened, original);
}
