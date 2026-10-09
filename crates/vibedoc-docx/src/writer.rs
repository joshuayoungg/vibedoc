//! Writes a document as a .docx (a zip of XML files Word understands).

use std::collections::HashMap;
use std::io::{Seek, Write};

use inkwell_core::{Document, Image, Paragraph, ParagraphKind, TextStyle};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::Error;

const BULLET_NUM_ID: usize = 1;
/// Word measures drawings in EMUs (English Metric Units).
const EMU_PER_POINT: f32 = 12700.0;

pub fn write_to<W: Write + Seek>(document: &Document, output: W) -> Result<(), Error> {
    let media = Media::collect(document);
    let (body, numbered_lists) = document_body(document, &media);
    let mut zip = ZipWriter::new(output);
    let parts = [
        ("[Content_Types].xml".to_string(), CONTENT_TYPES.as_bytes().to_vec()),
        ("_rels/.rels".to_string(), ROOT_RELS.as_bytes().to_vec()),
        ("word/_rels/document.xml.rels".to_string(), media.relationships().into_bytes()),
        ("word/document.xml".to_string(), body.into_bytes()),
        ("word/styles.xml".to_string(), STYLES.as_bytes().to_vec()),
        ("word/numbering.xml".to_string(), numbering(numbered_lists).into_bytes()),
    ];
    let image_files =
        media.images.iter().enumerate().map(|(i, image)| (media.path(i, image), image.bytes.to_vec()));
    for (name, content) in parts.into_iter().chain(image_files) {
        zip.start_file(name, SimpleFileOptions::default())?;
        zip.write_all(&content)?;
    }
    zip.finish()?;
    Ok(())
}

/// The images a document actually uses, numbered in order of appearance.
struct Media<'a> {
    images: Vec<&'a Image>,
    number_of: HashMap<char, usize>,
}

impl<'a> Media<'a> {
    fn collect(document: &'a Document) -> Self {
        let mut media = Media { images: Vec::new(), number_of: HashMap::new() };
        for c in document.paragraphs.iter().flat_map(|p| &p.chars) {
            if let Some(image) = document.image(c.ch) {
                if !media.number_of.contains_key(&c.ch) {
                    media.number_of.insert(c.ch, media.images.len());
                    media.images.push(image);
                }
            }
        }
        media
    }

    fn path(&self, index: usize, image: &Image) -> String {
        format!("word/media/image{}.{}", index + 1, file_extension(image))
    }

    fn relationships(&self) -> String {
        let mut xml = String::from(DOCUMENT_RELS_START);
        for (index, image) in self.images.iter().enumerate() {
            let target = self.path(index, image).replacen("word/", "", 1);
            xml.push_str(&format!(
                r#"<Relationship Id="rIdImage{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="{target}"/>"#,
                index + 1
            ));
        }
        xml.push_str("</Relationships>");
        xml
    }

    /// A `<w:drawing>` run for an image character, or `None` for ordinary text.
    fn drawing_run(&self, ch: char) -> Option<String> {
        let index = *self.number_of.get(&ch)?;
        let image = self.images[index];
        let (cx, cy) = ((image.width * EMU_PER_POINT) as i64, (image.height * EMU_PER_POINT) as i64);
        let id = index + 1;
        Some(format!(
            r#"<w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{id}" name="Picture {id}"/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="{id}" name="image{id}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rIdImage{id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>"#
        ))
    }
}

fn file_extension(image: &Image) -> &'static str {
    if image.bytes.starts_with(&[0xFF, 0xD8]) {
        "jpeg"
    } else {
        "png"
    }
}

/// Builds word/document.xml. Also returns how many separate numbered lists
/// there are, because each needs its own numbering entry so it starts at 1.
fn document_body(document: &Document, media: &Media) -> (String, usize) {
    let mut xml = String::from(DOCUMENT_START);
    let mut numbered_lists = 0;
    let mut previous_kind = None;
    for paragraph in &document.paragraphs {
        if paragraph.kind == ParagraphKind::Numbered && previous_kind != Some(ParagraphKind::Numbered) {
            numbered_lists += 1;
        }
        // Numbered list N uses numbering id N + 1 (id 1 is shared by all bullets).
        xml.push_str(&paragraph_xml(paragraph, BULLET_NUM_ID + numbered_lists, media));
        previous_kind = Some(paragraph.kind);
    }
    xml.push_str(DOCUMENT_END);
    (xml, numbered_lists)
}

fn paragraph_xml(paragraph: &Paragraph, numbered_list_id: usize, media: &Media) -> String {
    let properties = match paragraph.kind {
        ParagraphKind::Normal => String::new(),
        ParagraphKind::Heading1 => style_property("Heading1"),
        ParagraphKind::Heading2 => style_property("Heading2"),
        ParagraphKind::Heading3 => style_property("Heading3"),
        ParagraphKind::Bullet => list_properties(BULLET_NUM_ID),
        ParagraphKind::Numbered => list_properties(numbered_list_id),
    };
    let runs: String = paragraph.runs().iter().map(|(style, text)| runs_xml(*style, text, media)).collect();
    format!("<w:p>{properties}{runs}</w:p>")
}

fn style_property(style_id: &str) -> String {
    format!(r#"<w:pPr><w:pStyle w:val="{style_id}"/></w:pPr>"#)
}

fn list_properties(num_id: usize) -> String {
    format!(
        r#"<w:pPr><w:pStyle w:val="ListParagraph"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="{num_id}"/></w:numPr></w:pPr>"#
    )
}

/// Text runs, with each image split out into a drawing run of its own.
fn runs_xml(style: TextStyle, text: &str, media: &Media) -> String {
    let mut xml = String::new();
    let mut pending_text = String::new();
    for ch in text.chars() {
        match media.drawing_run(ch) {
            Some(drawing) => {
                xml.push_str(&text_run_xml(style, &std::mem::take(&mut pending_text)));
                xml.push_str(&drawing);
            }
            None => pending_text.push(ch),
        }
    }
    xml.push_str(&text_run_xml(style, &pending_text));
    xml
}

fn text_run_xml(style: TextStyle, text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut properties = String::new();
    if style.bold {
        properties.push_str("<w:b/>");
    }
    if style.italic {
        properties.push_str("<w:i/>");
    }
    if style.underline {
        properties.push_str(r#"<w:u w:val="single"/>"#);
    }
    if !properties.is_empty() {
        properties = format!("<w:rPr>{properties}</w:rPr>");
    }
    // Tabs are their own element in Word, not a character inside <w:t>.
    let content: Vec<String> =
        text.split('\t').map(|part| format!(r#"<w:t xml:space="preserve">{}</w:t>"#, escape(part))).collect();
    format!("<w:r>{properties}{}</w:r>", content.join("<w:tab/>"))
}

fn numbering(numbered_lists: usize) -> String {
    let mut xml = String::from(NUMBERING_START);
    xml.push_str(&format!(r#"<w:num w:numId="{BULLET_NUM_ID}"><w:abstractNumId w:val="0"/></w:num>"#));
    for list in 1..=numbered_lists {
        let num_id = BULLET_NUM_ID + list;
        xml.push_str(&format!(
            r#"<w:num w:numId="{num_id}"><w:abstractNumId w:val="1"/><w:lvlOverride w:ilvl="0"><w:startOverride w:val="1"/></w:lvlOverride></w:num>"#
        ));
    }
    xml.push_str("</w:numbering>");
    xml
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="jpeg" ContentType="image/jpeg"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

const DOCUMENT_RELS_START: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>"#;

const DOCUMENT_START: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    "\n",
    r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"><w:body>"#
);

// US Letter, one-inch margins (Word measures in twentieths of a point).
const DOCUMENT_END: &str = r#"<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#;

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/><w:sz w:val="24"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="120"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:keepNext/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="44"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:keepNext/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="34"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:keepNext/><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:sz w:val="28"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="ListParagraph"><w:name w:val="List Paragraph"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="40"/><w:ind w:left="720"/></w:pPr></w:style></w:styles>"#;

const NUMBERING_START: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:lvlJc w:val="left"/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separate_numbered_lists_get_separate_ids() {
        let numbered = |t| Paragraph::from_text(ParagraphKind::Numbered, t, TextStyle::default());
        let normal = Paragraph::from_text(ParagraphKind::Normal, "x", TextStyle::default());
        let document = Document::from_paragraphs(vec![numbered("a"), numbered("b"), normal, numbered("c")]);
        let (body, lists) = document_body(&document, &Media::collect(&document));
        assert_eq!(lists, 2);
        assert_eq!(body.matches(r#"w:numId w:val="2""#).count(), 2);
        assert_eq!(body.matches(r#"w:numId w:val="3""#).count(), 1);
    }

    #[test]
    fn escapes_special_characters() {
        assert_eq!(escape("a<b & \"c\">"), "a&lt;b &amp; &quot;c&quot;&gt;");
    }
}
