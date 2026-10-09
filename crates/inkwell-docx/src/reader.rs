//! Reads a .docx into a document. Anything Inkwell doesn't support yet
//! (tables, images, fonts, colors) is skipped, keeping its text where possible.

use std::collections::HashMap;
use std::io::{Read, Seek};

use inkwell_core::{Document, Paragraph, ParagraphKind, StyledChar, TextStyle};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use zip::ZipArchive;

use crate::Error;

pub fn read_from<R: Read + Seek>(input: R) -> Result<Document, Error> {
    let mut zip = ZipArchive::new(input)?;
    let body = read_part(&mut zip, "word/document.xml")?
        .ok_or("not a Word document: word/document.xml is missing")?;
    let bullet_lists = match read_part(&mut zip, "word/numbering.xml")? {
        Some(numbering) => bullet_list_ids(&numbering)?,
        None => HashMap::new(),
    };
    parse_body(&body, &bullet_lists)
}

fn read_part<R: Read + Seek>(zip: &mut ZipArchive<R>, name: &str) -> Result<Option<String>, Error> {
    let mut file = match zip.by_name(name) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(Some(content))
}

/// What we know about the paragraph and run being read.
#[derive(Default)]
struct ParserState {
    paragraph: Option<Paragraph>,
    run_style: TextStyle,
    in_paragraph_properties: bool,
    in_run_properties: bool,
    in_text: bool,
    is_list: bool,
    list_id: Option<String>,
}

fn parse_body(xml: &str, bullet_lists: &HashMap<String, bool>) -> Result<Document, Error> {
    let mut reader = Reader::from_str(xml);
    let mut state = ParserState::default();
    let mut paragraphs = Vec::new();

    loop {
        match reader.read_event()? {
            Event::Start(tag) => state.open(&tag, false),
            Event::Empty(tag) => {
                state.open(&tag, true);
                if tag.name().as_ref() == "w:p" {
                    paragraphs.push(state.finish_paragraph(bullet_lists));
                }
            }
            Event::End(tag) => match tag.name().as_ref() {
                "w:p" => paragraphs.push(state.finish_paragraph(bullet_lists)),
                "w:pPr" => state.in_paragraph_properties = false,
                "w:rPr" => state.in_run_properties = false,
                "w:t" => state.in_text = false,
                _ => {}
            },
            Event::Text(text) if state.in_text => state.push_text(&text.xml10_content()),
            Event::GeneralRef(reference) if state.in_text => {
                let name = reference.xml10_content();
                let resolved = match reference.resolve_char_ref()? {
                    Some(ch) => ch.to_string(),
                    None => {
                        quick_xml::escape::resolve_predefined_entity(&name).unwrap_or_default().to_string()
                    }
                };
                state.push_text(&resolved);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(Document::from_paragraphs(paragraphs))
}

impl ParserState {
    fn open(&mut self, tag: &BytesStart, is_empty: bool) {
        let in_run = !self.in_paragraph_properties;
        match tag.name().as_ref() {
            "w:p" => {
                self.paragraph = Some(Paragraph::default());
                self.is_list = false;
                self.list_id = None;
            }
            "w:pPr" => self.in_paragraph_properties = !is_empty,
            "w:pStyle" if self.in_paragraph_properties => {
                let kind = kind_for_style(&attribute(tag, "w:val").unwrap_or_default());
                if let (Some(paragraph), Some(kind)) = (self.paragraph.as_mut(), kind) {
                    paragraph.kind = kind;
                }
            }
            "w:numPr" if self.in_paragraph_properties => self.is_list = true,
            "w:numId" if self.in_paragraph_properties => self.list_id = attribute(tag, "w:val"),
            "w:r" => self.run_style = TextStyle::default(),
            "w:rPr" if in_run => self.in_run_properties = !is_empty,
            "w:b" if self.in_run_properties => self.run_style.bold = is_on(tag),
            "w:i" if self.in_run_properties => self.run_style.italic = is_on(tag),
            "w:u" if self.in_run_properties => {
                self.run_style.underline = attribute(tag, "w:val").as_deref() != Some("none") && is_on(tag)
            }
            "w:t" => self.in_text = !is_empty,
            "w:tab" if in_run => self.push_text("\t"),
            "w:br" | "w:cr" if in_run => self.push_text(" "),
            _ => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        let style = self.run_style;
        if let Some(paragraph) = self.paragraph.as_mut() {
            paragraph.chars.extend(text.chars().map(|ch| StyledChar { ch, style }));
        }
    }

    fn finish_paragraph(&mut self, bullet_lists: &HashMap<String, bool>) -> Paragraph {
        let mut paragraph = self.paragraph.take().unwrap_or_default();
        // A numId of "0" means "no list" in Word.
        let list_id = self.list_id.take().filter(|id| id != "0");
        if self.is_list && list_id.is_some() {
            let is_bullet = list_id.and_then(|id| bullet_lists.get(&id).copied()).unwrap_or(true);
            paragraph.kind = if is_bullet { ParagraphKind::Bullet } else { ParagraphKind::Numbered };
        }
        self.in_paragraph_properties = false;
        paragraph
    }
}

/// Maps Word's built-in style ids to paragraph kinds. Deeper headings become level 3.
fn kind_for_style(style_id: &str) -> Option<ParagraphKind> {
    let id = style_id.to_lowercase();
    match id.as_str() {
        "title" | "heading1" => Some(ParagraphKind::Heading1),
        "subtitle" | "heading2" => Some(ParagraphKind::Heading2),
        _ if id.starts_with("listbullet") => Some(ParagraphKind::Bullet),
        _ if id.starts_with("listnumber") => Some(ParagraphKind::Numbered),
        _ if id.starts_with("heading") => Some(ParagraphKind::Heading3),
        _ => None,
    }
}

/// `<w:b/>` means bold, but `<w:b w:val="0"/>` means explicitly not bold.
fn is_on(tag: &BytesStart) -> bool {
    !matches!(attribute(tag, "w:val").as_deref(), Some("0" | "false" | "off"))
}

fn attribute(tag: &BytesStart, name: &str) -> Option<String> {
    tag.try_get_attribute(name).ok().flatten().map(|a| a.value.into_owned())
}

/// Reads word/numbering.xml to learn which list ids are bullets (vs numbers).
fn bullet_list_ids(xml: &str) -> Result<HashMap<String, bool>, Error> {
    let mut reader = Reader::from_str(xml);
    let mut abstract_is_bullet: HashMap<String, bool> = HashMap::new();
    let mut list_to_abstract: Vec<(String, String)> = Vec::new();
    let mut current_abstract: Option<String> = None;
    let mut current_list: Option<String> = None;
    let mut in_first_level = false;

    loop {
        match reader.read_event()? {
            Event::Start(tag) | Event::Empty(tag) => match tag.name().as_ref() {
                "w:abstractNum" => current_abstract = attribute(&tag, "w:abstractNumId"),
                "w:lvl" => in_first_level = attribute(&tag, "w:ilvl").as_deref() == Some("0"),
                "w:numFmt" if in_first_level => {
                    if let Some(id) = current_abstract.clone() {
                        let is_bullet = attribute(&tag, "w:val").as_deref() == Some("bullet");
                        abstract_is_bullet.insert(id, is_bullet);
                    }
                }
                "w:num" => current_list = attribute(&tag, "w:numId"),
                "w:abstractNumId" => {
                    if let (Some(list), Some(target)) = (current_list.clone(), attribute(&tag, "w:val")) {
                        list_to_abstract.push((list, target));
                    }
                }
                _ => {}
            },
            Event::End(tag) if tag.name().as_ref() == "w:lvl" => in_first_level = false,
            Event::Eof => break,
            _ => {}
        }
    }

    Ok(list_to_abstract
        .into_iter()
        .map(|(list, target)| {
            let is_bullet = abstract_is_bullet.get(&target).copied().unwrap_or(true);
            (list, is_bullet)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(body: &str) -> Document {
        let xml = format!(r#"<w:document xmlns:w="x"><w:body>{body}</w:body></w:document>"#);
        parse_body(&xml, &HashMap::new()).unwrap()
    }

    #[test]
    fn reads_runs_with_styles_and_entities() {
        let d = parse(
            r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>A &amp; B</w:t></w:r><w:r><w:rPr><w:b w:val="0"/><w:i/></w:rPr><w:t xml:space="preserve"> c</w:t></w:r></w:p>"#,
        );
        let p = &d.paragraphs[0];
        assert_eq!(p.text(), "A & B c");
        assert!(p.chars[0].style.bold);
        assert!(!p.chars[5].style.bold && p.chars[5].style.italic);
    }

    #[test]
    fn paragraph_mark_formatting_does_not_leak_into_text() {
        let d = parse(r#"<w:p><w:pPr><w:rPr><w:b/></w:rPr></w:pPr><w:r><w:t>plain</w:t></w:r></w:p>"#);
        assert!(!d.paragraphs[0].chars[0].style.bold);
    }

    #[test]
    fn reads_headings_and_empty_paragraphs() {
        let d =
            parse(r#"<w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>H</w:t></w:r></w:p><w:p/>"#);
        assert_eq!(d.paragraphs[0].kind, ParagraphKind::Heading2);
        assert_eq!(d.paragraphs.len(), 2);
    }

    #[test]
    fn tells_bullets_from_numbers_using_numbering_xml() {
        let numbering = r#"<w:numbering xmlns:w="x"><w:abstractNum w:abstractNumId="5"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum><w:num w:numId="9"><w:abstractNumId w:val="5"/></w:num></w:numbering>"#;
        let lists = bullet_list_ids(numbering).unwrap();
        assert_eq!(lists.get("9"), Some(&false));
    }
}
