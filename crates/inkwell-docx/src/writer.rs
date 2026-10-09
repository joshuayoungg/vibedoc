//! Writes a document as a .docx (a zip of XML files Word understands).

use std::io::{Seek, Write};

use inkwell_core::{Document, Paragraph, ParagraphKind, TextStyle};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::Error;

const BULLET_NUM_ID: usize = 1;

pub fn write_to<W: Write + Seek>(document: &Document, output: W) -> Result<(), Error> {
    let (body, numbered_lists) = document_body(document);
    let mut zip = ZipWriter::new(output);
    let parts = [
        ("[Content_Types].xml", CONTENT_TYPES.to_string()),
        ("_rels/.rels", ROOT_RELS.to_string()),
        ("word/_rels/document.xml.rels", DOCUMENT_RELS.to_string()),
        ("word/document.xml", body),
        ("word/styles.xml", STYLES.to_string()),
        ("word/numbering.xml", numbering(numbered_lists)),
    ];
    for (name, content) in parts {
        zip.start_file(name, SimpleFileOptions::default())?;
        zip.write_all(content.as_bytes())?;
    }
    zip.finish()?;
    Ok(())
}

/// Builds word/document.xml. Also returns how many separate numbered lists
/// there are, because each needs its own numbering entry so it starts at 1.
fn document_body(document: &Document) -> (String, usize) {
    let mut xml = String::from(DOCUMENT_START);
    let mut numbered_lists = 0;
    let mut previous_kind = None;
    for paragraph in &document.paragraphs {
        if paragraph.kind == ParagraphKind::Numbered && previous_kind != Some(ParagraphKind::Numbered) {
            numbered_lists += 1;
        }
        // Numbered list N uses numbering id N + 1 (id 1 is shared by all bullets).
        xml.push_str(&paragraph_xml(paragraph, BULLET_NUM_ID + numbered_lists));
        previous_kind = Some(paragraph.kind);
    }
    xml.push_str(DOCUMENT_END);
    (xml, numbered_lists)
}

fn paragraph_xml(paragraph: &Paragraph, numbered_list_id: usize) -> String {
    let properties = match paragraph.kind {
        ParagraphKind::Normal => String::new(),
        ParagraphKind::Heading1 => style_property("Heading1"),
        ParagraphKind::Heading2 => style_property("Heading2"),
        ParagraphKind::Heading3 => style_property("Heading3"),
        ParagraphKind::Bullet => list_properties(BULLET_NUM_ID),
        ParagraphKind::Numbered => list_properties(numbered_list_id),
    };
    let runs: String = paragraph.runs().iter().map(|(style, text)| run_xml(*style, text)).collect();
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

fn run_xml(style: TextStyle, text: &str) -> String {
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
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#;

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

const DOCUMENT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/></Relationships>"#;

const DOCUMENT_START: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    "\n",
    r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#
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
        let (body, lists) = document_body(&document);
        assert_eq!(lists, 2);
        assert_eq!(body.matches(r#"w:numId w:val="2""#).count(), 2);
        assert_eq!(body.matches(r#"w:numId w:val="3""#).count(), 1);
    }

    #[test]
    fn escapes_special_characters() {
        assert_eq!(escape("a<b & \"c\">"), "a&lt;b &amp; &quot;c&quot;&gt;");
    }
}
