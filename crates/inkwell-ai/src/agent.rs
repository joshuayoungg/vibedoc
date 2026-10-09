//! The assistant loop: send the request, run the tools the model asks for,
//! send back the results, and repeat until the model is done.
//!
//! The AI edits its own copy of the document on a background thread. The app
//! applies the finished copy as a single undo step.

use std::sync::mpsc::{channel, Receiver};

use inkwell_core::{Document, Selection};

use crate::config::Config;
use crate::models::{chat_model, image_model, ChatModel, Message, ToolResult};
use crate::text::{describe_document, describe_selection};
use crate::tools::{tool_specs, Workspace};

/// Stops a confused model from looping forever.
const MAX_STEPS: usize = 16;

const SYSTEM_PROMPT: &str = "\
You are the assistant built into Inkwell, a word processor. The user asks you to write, edit, \
format, or illustrate their document.

- Change the document only by calling tools. Never say you changed something you didn't.
- Paragraphs are numbered from 0, shown in [brackets]. Inserting or deleting shifts the numbers \
of later paragraphs; call read_document if you're unsure.
- Inline formatting appears, and can be written, as **bold**, *italic*, and __underline__. \
Prefixes like `# `, `- `, and `1. ` show the paragraph kind; set kinds with the `kind` field, \
not by typing these prefixes.
- \"This\", \"it\", and \"here\" usually mean the user's selection, or the paragraph with the caret.
- For pictures, write a detailed prompt (subject, style, composition) for insert_image.
- When you're done, reply with one short sentence saying what you did. If the user only asks \
a question, answer briefly without editing.";

/// One finished exchange, kept so follow-ups like "make it shorter" work.
#[derive(Clone, Debug)]
pub struct Turn {
    pub request: String,
    pub reply: String,
}

pub enum AgentEvent {
    /// A short note on what the assistant is doing right now.
    Progress(String),
    Finished {
        document: Document,
        selection: Selection,
        reply: String,
    },
    Failed(String),
}

/// Starts the assistant on a background thread. `notify` is called after each
/// event so the app can wake up and redraw.
pub fn spawn(
    config: Config,
    request: String,
    history: Vec<Turn>,
    document: Document,
    selection: Selection,
    notify: impl Fn() + Send + 'static,
) -> Receiver<AgentEvent> {
    let (sender, receiver) = channel();
    std::thread::spawn(move || {
        let progress = |note: String| {
            let _ = sender.send(AgentEvent::Progress(note));
            notify();
        };
        let event = match run(&config, &request, &history, document, selection, &progress) {
            Ok((document, selection, reply)) => AgentEvent::Finished { document, selection, reply },
            Err(error) => AgentEvent::Failed(error),
        };
        let _ = sender.send(event);
        notify();
    });
    receiver
}

pub fn run(
    config: &Config,
    request: &str,
    history: &[Turn],
    document: Document,
    selection: Selection,
    progress: &dyn Fn(String),
) -> Result<(Document, Selection, String), String> {
    let chat = chat_model(&config.agent)?;
    let images = image_model(&config.images);
    let images = images.as_deref().map_err(|e| format!("Image generation isn't set up: {e}"));
    let workspace = Workspace { document, selection, images };
    run_with(chat.as_ref(), &config.agent.instructions, request, history, workspace, progress)
}

/// The loop itself, taking models directly so tests can pass in fakes.
pub fn run_with(
    chat: &dyn ChatModel,
    instructions: &str,
    request: &str,
    history: &[Turn],
    mut workspace: Workspace,
    progress: &dyn Fn(String),
) -> Result<(Document, Selection, String), String> {
    let system = match instructions.trim() {
        "" => SYSTEM_PROMPT.to_string(),
        extra => format!("{SYSTEM_PROMPT}\n\nThe user's own instructions for you:\n{extra}"),
    };
    let mut messages = Vec::new();
    for turn in history {
        messages.push(Message::User(turn.request.clone()));
        messages.push(Message::Assistant { text: turn.reply.clone(), tool_calls: Vec::new() });
    }
    messages.push(Message::User(format!(
        "{request}\n\n---\nThe document right now:\n{}\n{}",
        describe_document(&workspace.document),
        describe_selection(&workspace.document, workspace.selection),
    )));

    let tools = tool_specs();
    for _ in 0..MAX_STEPS {
        progress("Thinking…".into());
        let reply = chat.reply(&system, &messages, &tools)?;
        if reply.tool_calls.is_empty() {
            return Ok((workspace.document, workspace.selection, reply.text));
        }
        let mut results = Vec::new();
        for call in &reply.tool_calls {
            progress(describe_action(&call.name));
            let content = match workspace.run(call) {
                Ok(result) => result,
                Err(error) => format!("Error: {error}"),
            };
            results.push(ToolResult { call_id: call.id.clone(), content });
        }
        messages.push(Message::Assistant { text: reply.text, tool_calls: reply.tool_calls });
        messages.push(Message::ToolResults(results));
    }
    let reply = "I stopped after too many steps. Here's what I'd done so far.".to_string();
    Ok((workspace.document, workspace.selection, reply))
}

fn describe_action(tool: &str) -> String {
    match tool {
        "insert_image" => "Drawing a picture…",
        "read_document" => "Reading the document…",
        "format_text" | "format_selection" | "set_paragraph_kind" => "Formatting…",
        "delete_paragraphs" => "Deleting…",
        _ => "Writing…",
    }
    .to_string()
}
