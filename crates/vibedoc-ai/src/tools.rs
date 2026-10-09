//! The only ways the AI can change a document. Each tool is a small, typed
//! edit, so the AI can't produce anything the editor doesn't understand.

use inkwell_core::{Document, Image, PageSetup, Paragraph, ParagraphKind, Position, Selection, StyleFlag};
use serde_json::{json, Value};

use crate::models::{ImageModel, ToolCall, ToolSpec};
use crate::text::{describe_document, parse_markdown};

/// The document the AI is editing, plus what it needs to edit it.
pub struct Workspace<'a> {
    pub document: Document,
    pub selection: Selection,
    /// Image generation, or why it isn't available.
    pub images: Result<&'a dyn ImageModel, String>,
}

pub fn tool_specs() -> Vec<ToolSpec> {
    let kind = json!({
        "type": "string",
        "enum": ["normal", "heading1", "heading2", "heading3", "bullet", "numbered"],
    });
    let paragraph_range = |what: &str| {
        json!({
            "first_paragraph": { "type": "integer", "description": format!("First paragraph {what}.") },
            "last_paragraph": { "type": "integer", "description": "Last paragraph, inclusive. Defaults to first_paragraph." },
        })
    };
    let style_flags = json!({
        "bold": { "type": "boolean" },
        "italic": { "type": "boolean" },
        "underline": { "type": "boolean" },
    });
    let after_paragraph = json!({
        "type": "integer",
        "description": "Insert after this paragraph. Use -1 for the very top. An empty paragraph here is replaced instead.",
    });

    vec![
        ToolSpec {
            name: "read_document",
            description: "Show the current document with paragraph numbers. Use after edits that shift numbering.",
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: "insert_paragraphs",
            description: "Insert new paragraphs (text, headings, or list items). Text may use **bold**, *italic*, __underline__.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "after_paragraph": after_paragraph,
                    "paragraphs": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": { "kind": kind, "text": { "type": "string" } },
                            "required": ["kind", "text"],
                        },
                    },
                },
                "required": ["after_paragraph", "paragraphs"],
            }),
        },
        ToolSpec {
            name: "replace_paragraph",
            description: "Rewrite one paragraph's text and/or change its kind. Text may use **bold**, *italic*, __underline__.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "paragraph": { "type": "integer" },
                    "text": { "type": "string" },
                    "kind": kind,
                },
                "required": ["paragraph"],
            }),
        },
        ToolSpec {
            name: "format_text",
            description: "Turn bold/italic/underline on or off for paragraphs, or only for occurrences of `text` within them.",
            input_schema: json!({
                "type": "object",
                "properties": merge(paragraph_range("to format"), merge(style_flags.clone(), json!({
                    "text": { "type": "string", "description": "Only format occurrences of this exact text (case-sensitive)." },
                }))),
                "required": ["first_paragraph"],
            }),
        },
        ToolSpec {
            name: "format_selection",
            description: "Turn bold/italic/underline on or off for exactly what the user has selected.",
            input_schema: json!({ "type": "object", "properties": style_flags }),
        },
        ToolSpec {
            name: "replace_selection",
            description: "Replace what the user has selected (or insert at the caret) with text. Newlines start new paragraphs.",
            input_schema: json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
            }),
        },
        ToolSpec {
            name: "set_paragraph_kind",
            description: "Make paragraphs normal text, headings, or list items.",
            input_schema: json!({
                "type": "object",
                "properties": merge(paragraph_range("to change"), json!({ "kind": kind })),
                "required": ["first_paragraph", "kind"],
            }),
        },
        ToolSpec {
            name: "delete_paragraphs",
            description: "Delete whole paragraphs.",
            input_schema: json!({
                "type": "object",
                "properties": paragraph_range("to delete"),
                "required": ["first_paragraph"],
            }),
        },
        ToolSpec {
            name: "insert_image",
            description: "Generate a picture from a detailed prompt and insert it as its own paragraph.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "after_paragraph": after_paragraph,
                    "prompt": { "type": "string", "description": "What to draw: subject, style, composition, colors." },
                },
                "required": ["after_paragraph", "prompt"],
            }),
        },
    ]
}

fn merge(mut a: Value, b: Value) -> Value {
    if let (Some(a), Value::Object(b)) = (a.as_object_mut(), b) {
        a.extend(b);
    }
    a
}

impl Workspace<'_> {
    /// Runs one tool call. The result text goes back to the AI either way.
    pub fn run(&mut self, call: &ToolCall) -> Result<String, String> {
        let input = &call.input;
        match call.name.as_str() {
            "read_document" => Ok(describe_document(&self.document)),
            "insert_paragraphs" => {
                let items = input["paragraphs"].as_array().ok_or("`paragraphs` must be a list")?;
                let paragraphs = items
                    .iter()
                    .map(|item| {
                        let kind = parse_kind(&item["kind"])?;
                        Ok(Paragraph {
                            kind,
                            chars: parse_markdown(item["text"].as_str().unwrap_or_default()),
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let count = paragraphs.len();
                let first = self.insert_after(&input["after_paragraph"], paragraphs)?;
                Ok(format!("Inserted as paragraphs {first}-{}.", first + count - 1))
            }
            "replace_paragraph" => {
                let index = self.paragraph_index(&input["paragraph"])?;
                let paragraph = &mut self.document.paragraphs[index];
                if let Some(text) = input["text"].as_str() {
                    paragraph.chars = parse_markdown(text);
                }
                if !input["kind"].is_null() {
                    paragraph.kind = parse_kind(&input["kind"])?;
                }
                self.move_selection(|p| p);
                Ok(format!("Paragraph {index} updated."))
            }
            "format_text" => {
                let (first, last) = self.paragraph_range(input)?;
                let ranges = match input["text"].as_str() {
                    Some(text) => self.occurrences(first, last, text),
                    None => vec![(
                        Position::new(first, 0),
                        Position::new(last, self.document.paragraph_len(last)),
                    )],
                };
                if ranges.is_empty() {
                    return Err("That text isn't in those paragraphs.".into());
                }
                for &(start, end) in &ranges {
                    self.apply_style_flags(input, start, end);
                }
                Ok(format!("Formatted {} range(s).", ranges.len()))
            }
            "format_selection" => {
                if self.selection.is_empty() {
                    return Err("Nothing is selected. Use format_text instead.".into());
                }
                self.apply_style_flags(input, self.selection.start(), self.selection.end());
                Ok("Formatted the selection.".into())
            }
            "replace_selection" => {
                let text = input["text"].as_str().ok_or("`text` is required")?;
                let (start, end) = (self.selection.start(), self.selection.end());
                self.document.delete(start, end);
                let after = self.insert_markdown(start, text);
                self.selection = Selection::caret(after);
                Ok("Replaced the selection.".into())
            }
            "set_paragraph_kind" => {
                let (first, last) = self.paragraph_range(input)?;
                let kind = parse_kind(&input["kind"])?;
                for paragraph in &mut self.document.paragraphs[first..=last] {
                    paragraph.kind = kind;
                }
                Ok(format!("Paragraphs {first}-{last} are now {kind:?}."))
            }
            "delete_paragraphs" => {
                let (first, last) = self.paragraph_range(input)?;
                self.document.paragraphs.drain(first..=last);
                if self.document.paragraphs.is_empty() {
                    self.document.paragraphs.push(Paragraph::default());
                }
                let removed = last - first + 1;
                self.move_selection(|p| match p.paragraph {
                    index if index < first => p,
                    index if index <= last => Position::new(first, 0),
                    index => Position::new(index - removed, p.offset),
                });
                Ok(format!("Deleted paragraphs {first}-{last}. Later paragraphs moved up."))
            }
            "insert_image" => {
                let prompt = input["prompt"].as_str().ok_or("`prompt` is required")?;
                let bytes = self.images.clone()?.generate(prompt)?;
                let image = fit_to_page(bytes)?;
                let mut paragraph = Paragraph::new(ParagraphKind::Normal);
                let placeholder = self.document.add_image(image);
                paragraph.chars.push(inkwell_core::StyledChar { ch: placeholder, style: Default::default() });
                let index = self.insert_after(&input["after_paragraph"], vec![paragraph])?;
                Ok(format!("Inserted the image as paragraph {index}."))
            }
            other => Err(format!("There's no tool called {other}.")),
        }
    }

    /// Inserts paragraphs after `after` (-1 = top). If that paragraph is empty
    /// normal text, it's replaced, so "put a list here" doesn't leave a blank line.
    /// Returns the index of the first inserted paragraph.
    fn insert_after(&mut self, after: &Value, paragraphs: Vec<Paragraph>) -> Result<usize, String> {
        let after = after.as_i64().ok_or("`after_paragraph` must be a number")?;
        let paragraph_count = self.document.paragraphs.len() as i64;
        if after < -1 || after >= paragraph_count {
            return Err(format!("after_paragraph must be between -1 and {}.", paragraph_count - 1));
        }
        let count = paragraphs.len();
        let target = after.max(0) as usize;
        let existing = &self.document.paragraphs[target];
        let replace_empty = after >= 0 && existing.is_empty() && existing.kind == ParagraphKind::Normal;
        let first = if replace_empty { target } else { (after + 1) as usize };
        let removed = if replace_empty { first..first + 1 } else { first..first };
        let removed_count = removed.len();
        self.document.paragraphs.splice(removed, paragraphs);
        let last_inserted = first + count.max(1) - 1;
        self.move_selection(|p| match p.paragraph {
            index if index < first => p,
            // The caret was in the empty paragraph that got replaced.
            index if index < first + removed_count => Position::new(last_inserted, usize::MAX),
            index => Position::new(index + count - removed_count, p.offset),
        });
        Ok(first)
    }

    /// Keeps the user's selection on the same text after paragraphs move.
    fn move_selection(&mut self, update: impl Fn(Position) -> Position) {
        let anchor = self.document.clamp(update(self.selection.anchor));
        let cursor = self.document.clamp(update(self.selection.cursor));
        self.selection = Selection { anchor, cursor };
    }

    fn insert_markdown(&mut self, at: Position, text: &str) -> Position {
        let mut position = at;
        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                position = self.document.split_paragraph(position);
            }
            let chars = parse_markdown(line);
            let count = chars.len();
            self.document.paragraphs[position.paragraph]
                .chars
                .splice(position.offset..position.offset, chars);
            position.offset += count;
        }
        position
    }

    fn apply_style_flags(&mut self, input: &Value, start: Position, end: Position) {
        for (name, flag) in
            [("bold", StyleFlag::Bold), ("italic", StyleFlag::Italic), ("underline", StyleFlag::Underline)]
        {
            if let Some(on) = input[name].as_bool() {
                self.document.restyle(start, end, |style| style.set(flag, on));
            }
        }
    }

    /// Every place `needle` appears in paragraphs `first..=last`.
    fn occurrences(&self, first: usize, last: usize, needle: &str) -> Vec<(Position, Position)> {
        let needle: Vec<char> = needle.chars().collect();
        let mut found = Vec::new();
        if needle.is_empty() {
            return found;
        }
        for index in first..=last {
            let chars: Vec<char> = self.document.paragraphs[index].chars.iter().map(|c| c.ch).collect();
            for offset in 0..chars.len().saturating_sub(needle.len() - 1) {
                if chars[offset..offset + needle.len()] == needle[..] {
                    found.push((Position::new(index, offset), Position::new(index, offset + needle.len())));
                }
            }
        }
        found
    }

    fn paragraph_index(&self, value: &Value) -> Result<usize, String> {
        let last = self.document.paragraphs.len() - 1;
        match value.as_u64() {
            Some(index) if index as usize <= last => Ok(index as usize),
            _ => Err(format!("Paragraph numbers go from 0 to {last}.")),
        }
    }

    fn paragraph_range(&self, input: &Value) -> Result<(usize, usize), String> {
        let first = self.paragraph_index(&input["first_paragraph"])?;
        let last = match input["last_paragraph"].is_null() {
            true => first,
            false => self.paragraph_index(&input["last_paragraph"])?,
        };
        if last < first {
            return Err("last_paragraph comes before first_paragraph.".into());
        }
        Ok((first, last))
    }
}

fn parse_kind(value: &Value) -> Result<ParagraphKind, String> {
    Ok(match value.as_str() {
        Some("normal") => ParagraphKind::Normal,
        Some("heading1") => ParagraphKind::Heading1,
        Some("heading2") => ParagraphKind::Heading2,
        Some("heading3") => ParagraphKind::Heading3,
        Some("bullet") => ParagraphKind::Bullet,
        Some("numbered") => ParagraphKind::Numbered,
        _ => return Err(format!("Unknown paragraph kind {value}.")),
    })
}

/// Sizes a generated picture to sit comfortably on the page.
fn fit_to_page(bytes: Vec<u8>) -> Result<Image, String> {
    let size =
        imagesize::blob_size(&bytes).map_err(|e| format!("The image model sent an unreadable image: {e}"))?;
    // Screen pixels are 3/4 of a point.
    let (width, height) = (size.width as f32 * 0.75, size.height as f32 * 0.75);
    let page = PageSetup::LETTER;
    Ok(Image::fitted(bytes, width, height, page.content_width() * 0.75, page.content_height() * 0.5))
}
