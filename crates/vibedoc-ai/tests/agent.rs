//! Runs the assistant loop against scripted fake models.

use std::sync::Mutex;

use inkwell_ai::agent::run_with;
use inkwell_ai::models::{ChatModel, ImageModel, Message, Reply, ToolCall, ToolSpec};
use inkwell_ai::tools::Workspace;
use inkwell_core::{is_image_char, Document, Paragraph, ParagraphKind, Position, Selection, TextStyle};
use serde_json::{json, Value};

/// Replies with each scripted batch of tool calls in turn, then "Done."
struct ScriptedModel {
    steps: Mutex<Vec<Vec<(&'static str, Value)>>>,
    tool_results_seen: Mutex<Vec<String>>,
}

impl ScriptedModel {
    fn new(steps: Vec<Vec<(&'static str, Value)>>) -> Self {
        ScriptedModel { steps: Mutex::new(steps), tool_results_seen: Mutex::new(Vec::new()) }
    }
}

impl ChatModel for ScriptedModel {
    fn reply(&self, _: &str, messages: &[Message], _: &[ToolSpec]) -> Result<Reply, String> {
        if let Some(Message::ToolResults(results)) = messages.last() {
            self.tool_results_seen.lock().unwrap().extend(results.iter().map(|r| r.content.clone()));
        }
        let mut steps = self.steps.lock().unwrap();
        if steps.is_empty() {
            return Ok(Reply { text: "Done.".into(), tool_calls: vec![] });
        }
        let tool_calls = steps
            .remove(0)
            .into_iter()
            .enumerate()
            .map(|(i, (name, input))| ToolCall { id: format!("call{i}"), name: name.into(), input })
            .collect();
        Ok(Reply { text: String::new(), tool_calls })
    }
}

/// Returns a 200x100 PNG header, which is all sizing needs.
struct FakeImages;

impl ImageModel for FakeImages {
    fn generate(&self, _: &str) -> Result<Vec<u8>, String> {
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13];
        png.extend(b"IHDR");
        png.extend(200u32.to_be_bytes());
        png.extend(100u32.to_be_bytes());
        png.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
        Ok(png)
    }
}

fn texts(document: &Document) -> Vec<String> {
    document.paragraphs.iter().map(|p| p.text()).collect()
}

fn run(model: &ScriptedModel, document: Document, selection: Selection) -> (Document, Selection, String) {
    let workspace = Workspace { document, selection, images: Ok(&FakeImages) };
    run_with(model, "", "do it", &[], workspace, &|_| {}).unwrap()
}

#[test]
fn puts_a_list_into_an_empty_document() {
    let model = ScriptedModel::new(vec![vec![(
        "insert_paragraphs",
        json!({ "after_paragraph": 0, "paragraphs": [
            { "kind": "heading1", "text": "Groceries" },
            { "kind": "bullet", "text": "**eggs**" },
            { "kind": "bullet", "text": "milk" },
        ]}),
    )]]);
    let (document, _, reply) = run(&model, Document::default(), Selection::default());
    assert_eq!(texts(&document), ["Groceries", "eggs", "milk"]);
    assert_eq!(document.paragraphs[1].kind, ParagraphKind::Bullet);
    assert!(document.paragraphs[1].chars[0].style.bold);
    assert_eq!(reply, "Done.");
}

#[test]
fn makes_the_selection_bold() {
    let document = Document::from_paragraphs(vec![Paragraph::from_text(
        ParagraphKind::Normal,
        "make this bold",
        TextStyle::default(),
    )]);
    let selection = Selection { anchor: Position::new(0, 5), cursor: Position::new(0, 9) };
    let model = ScriptedModel::new(vec![vec![("format_selection", json!({ "bold": true }))]]);
    let (document, _, _) = run(&model, document, selection);
    let bold: String = document.paragraphs[0].chars.iter().filter(|c| c.style.bold).map(|c| c.ch).collect();
    assert_eq!(bold, "this");
}

#[test]
fn generates_and_places_an_image() {
    let model =
        ScriptedModel::new(vec![vec![("insert_image", json!({ "after_paragraph": 0, "prompt": "a cat" }))]]);
    let document = Document::from_paragraphs(vec![Paragraph::from_text(
        ParagraphKind::Normal,
        "intro",
        TextStyle::default(),
    )]);
    let (document, _, _) = run(&model, document, Selection::default());
    let placeholder = document.paragraphs[1].chars[0].ch;
    assert!(is_image_char(placeholder));
    let image = document.image(placeholder).unwrap();
    assert_eq!((image.width, image.height), (150.0, 75.0));
}

#[test]
fn mistakes_are_reported_to_the_model_instead_of_crashing() {
    let model = ScriptedModel::new(vec![vec![
        ("delete_paragraphs", json!({ "first_paragraph": 7 })),
        ("format_selection", json!({ "bold": true })),
        ("no_such_tool", json!({})),
    ]]);
    let (document, _, _) = run(&model, Document::default(), Selection::default());
    assert_eq!(document, Document::default());
    let seen = model.tool_results_seen.lock().unwrap();
    assert_eq!(seen.len(), 3);
    assert!(seen.iter().all(|r| r.starts_with("Error:")));
}

#[test]
fn formats_matching_text_across_paragraphs() {
    let document = Document::from_paragraphs(vec![
        Paragraph::from_text(ParagraphKind::Normal, "Rust is fast", TextStyle::default()),
        Paragraph::from_text(ParagraphKind::Normal, "I like Rust", TextStyle::default()),
    ]);
    let model = ScriptedModel::new(vec![vec![(
        "format_text",
        json!({ "first_paragraph": 0, "last_paragraph": 1, "text": "Rust", "italic": true }),
    )]]);
    let (document, _, _) = run(&model, document, Selection::default());
    let italic_count = document.paragraphs.iter().flat_map(|p| &p.chars).filter(|c| c.style.italic).count();
    assert_eq!(italic_count, 8);
}

#[test]
fn the_selection_follows_its_text_when_paragraphs_are_inserted_above() {
    let document = Document::from_paragraphs(vec![
        Paragraph::from_text(ParagraphKind::Normal, "top", TextStyle::default()),
        Paragraph::from_text(ParagraphKind::Normal, "pick me", TextStyle::default()),
    ]);
    let selection = Selection { anchor: Position::new(1, 0), cursor: Position::new(1, 4) };
    let model = ScriptedModel::new(vec![vec![
        (
            "insert_paragraphs",
            json!({ "after_paragraph": 0, "paragraphs": [
            { "kind": "normal", "text": "new one" }, { "kind": "normal", "text": "new two" }] }),
        ),
        ("format_selection", json!({ "underline": true })),
    ]]);
    let (document, selection, _) = run(&model, document, selection);
    assert_eq!(texts(&document), ["top", "new one", "new two", "pick me"]);
    assert_eq!(selection.start(), Position::new(3, 0));
    let underlined: String =
        document.paragraphs[3].chars.iter().filter(|c| c.style.underline).map(|c| c.ch).collect();
    assert_eq!(underlined, "pick");
}

#[test]
fn deleting_paragraphs_above_moves_the_selection_up() {
    let document = Document::from_paragraphs(vec![
        Paragraph::from_text(ParagraphKind::Normal, "a", TextStyle::default()),
        Paragraph::from_text(ParagraphKind::Normal, "b", TextStyle::default()),
        Paragraph::from_text(ParagraphKind::Normal, "keep", TextStyle::default()),
    ]);
    let model = ScriptedModel::new(vec![vec![(
        "delete_paragraphs",
        json!({ "first_paragraph": 0, "last_paragraph": 1 }),
    )]]);
    let (_, selection, _) = run(&model, document, Selection::caret(Position::new(2, 3)));
    assert_eq!(selection.cursor, Position::new(0, 3));
}
