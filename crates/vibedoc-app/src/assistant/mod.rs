//! The assistant's UI: a side panel of review cards (one per request, each
//! with its changes and an Undo), the Ctrl+K command bar, and settings.
//! The actual work happens in the `vibedoc-ai` crate, on a background thread.
//!
//! Each open document has its own [`Conversation`], so a request keeps
//! working on the document it was asked about even after switching tabs.

mod command_bar;
mod diff_view;
mod panel;
mod settings;

use std::ops::Range;
use std::sync::mpsc::Receiver;

use eframe::egui;
use vibedoc_ai::{AgentEvent, Config, Turn};
use vibedoc_core::changes::{diff, locate, revert, Hunk};
use vibedoc_core::{Document, Editor, Selection};

/// What's shared by every document: the configuration and the panel's own state.
pub struct Assistant {
    pub panel_open: bool,
    config: Config,
    input: String,
    command_bar: Option<String>,
    settings: Option<settings::Settings>,
    input_has_focus: bool,
    /// Shown above the cards, e.g. when the settings file couldn't be read.
    notice: Option<String>,
}

/// One document's requests and their changes.
#[derive(Default)]
pub struct Conversation {
    entries: Vec<Entry>,
    running: Option<Running>,
    progress: String,
    /// Paragraphs to tint on the page: the changes of the card being looked at.
    highlights: Vec<Range<usize>>,
    /// The document revision right after the latest change was applied. Until
    /// the next edit, that change stays highlighted so it's easy to spot.
    fresh_change: Option<u64>,
    just_finished: bool,
}

struct Running {
    receiver: Receiver<AgentEvent>,
    /// The document as it was when the request was sent.
    before: Document,
}

/// One request and what came of it.
struct Entry {
    request: String,
    reply: String,
    outcome: Outcome,
    details_open: bool,
}

enum Outcome {
    Working,
    Failed,
    Cancelled,
    /// Finished without touching the document, e.g. a question.
    Answered,
    Changed(Change),
}

/// What one request did to the document, kept so it can be shown and undone.
struct Change {
    before: Document,
    after: Document,
    hunks: Vec<Hunk>,
    /// `state` for the document revision it was worked out for.
    cached: Option<(u64, ChangeState)>,
}

#[derive(Clone, Debug, PartialEq)]
enum ChangeState {
    /// In the document. These paragraphs are what the assistant wrote.
    Applied(Vec<Range<usize>>),
    /// Taken back out.
    Undone,
    /// The changed paragraphs were edited again, so it can't be undone here.
    EditedSince,
}

/// What the assistant finished, for the app to apply to the editor.
pub struct Finished {
    pub document: Document,
    pub selection: Selection,
}

/// Something the panel asks the app to do.
pub enum Action {
    /// Swap in this document as one undo step (undoing or redoing a change).
    Apply(Document),
    /// Scroll the page to this paragraph.
    Reveal(usize),
}

impl Default for Assistant {
    fn default() -> Self {
        let (config, notice) = match Config::load() {
            Ok(config) => (config, None),
            Err(error) => (
                Config::default(),
                Some(format!("Your settings file couldn't be read, so defaults are in use: {error}")),
            ),
        };
        Assistant {
            panel_open: true,
            config,
            input: String::new(),
            command_bar: None,
            settings: None,
            input_has_focus: false,
            notice,
        }
    }
}

impl Change {
    fn new(before: Document, after: Document) -> Change {
        let hunks = diff(&before, &after);
        Change { before, after, hunks, cached: None }
    }

    /// Where this change stands in the editor's document right now.
    fn state(&mut self, editor: &Editor) -> &ChangeState {
        let revision = editor.revision();
        if self.cached.as_ref().is_none_or(|(at, _)| *at != revision) {
            let current = editor.document();
            let state = if let Some(ranges) = locate(&self.hunks, &self.after, current) {
                ChangeState::Applied(ranges)
            } else if locate(&self.undo_hunks(), &self.before, current).is_some() {
                ChangeState::Undone
            } else {
                ChangeState::EditedSince
            };
            self.cached = Some((revision, state));
        }
        &self.cached.as_ref().expect("just computed").1
    }

    /// The same change seen backwards, from `after` to `before`.
    fn undo_hunks(&self) -> Vec<Hunk> {
        self.hunks.iter().map(|h| Hunk { before: h.after.clone(), after: h.before.clone() }).collect()
    }

    /// Paragraphs added (or rewritten) and removed.
    fn counts(&self) -> (usize, usize) {
        let added = self.hunks.iter().map(|h| h.after.len()).sum();
        let removed = self.hunks.iter().map(|h| h.before.len()).sum();
        (added, removed)
    }

    /// The document with this change taken out, or put back in.
    fn toggled(&self, current: &Document, undo: bool) -> Option<Document> {
        if undo {
            revert(&self.before, &self.after, current)
        } else {
            revert(&self.after, &self.before, current)
        }
    }
}

impl Conversation {
    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// Paragraphs to tint on the page, worked out when the panel was drawn.
    pub fn highlights(&self) -> &[Range<usize>] {
        &self.highlights
    }

    pub fn cancel(&mut self) {
        // The background thread finishes on its own; its result is ignored.
        if self.running.take().is_some() {
            self.finish_last("Cancelled.".into(), Outcome::Cancelled);
        }
    }

    /// Earlier exchanges, so follow-ups like "make it shorter" work.
    fn history(&mut self, editor: &Editor) -> Vec<Turn> {
        let mut turns = Vec::new();
        for entry in &mut self.entries {
            let reply = match &mut entry.outcome {
                Outcome::Answered => entry.reply.clone(),
                Outcome::Changed(change) => match change.state(editor) {
                    ChangeState::Undone => {
                        format!("{} (The user has since undone this change.)", entry.reply)
                    }
                    _ => entry.reply.clone(),
                },
                Outcome::Working | Outcome::Failed | Outcome::Cancelled => continue,
            };
            turns.push(Turn { request: entry.request.clone(), reply });
        }
        turns
    }

    /// Checks on the background work. Returns the result once it's done.
    pub fn poll(&mut self) -> Option<Finished> {
        let running = self.running.as_ref()?;
        while let Ok(event) = running.receiver.try_recv() {
            match event {
                AgentEvent::Progress(note) => self.progress = note,
                AgentEvent::Finished { document, selection, reply } => {
                    let before = self.running.take().expect("running").before;
                    let outcome = if document == before {
                        Outcome::Answered
                    } else {
                        self.just_finished = true;
                        Outcome::Changed(Change::new(before, document.clone()))
                    };
                    self.finish_last(reply, outcome);
                    return Some(Finished { document, selection });
                }
                AgentEvent::Failed(error) => {
                    self.running = None;
                    self.finish_last(error, Outcome::Failed);
                    return None;
                }
            }
        }
        None
    }

    fn finish_last(&mut self, reply: String, outcome: Outcome) {
        if let Some(entry) = self.entries.last_mut() {
            entry.reply = if reply.trim().is_empty() { "Done.".into() } else { reply };
            entry.outcome = outcome;
        }
    }
}

impl Assistant {
    /// True while one of the assistant's text boxes or windows wants the keyboard.
    pub fn wants_keyboard(&self) -> bool {
        self.input_has_focus || self.command_bar.is_some() || self.settings.is_some()
    }

    pub fn submit(
        &mut self,
        conversation: &mut Conversation,
        request: String,
        editor: &Editor,
        ctx: &egui::Context,
    ) {
        let request = request.trim().to_string();
        if request.is_empty() || conversation.is_running() {
            return;
        }
        let history = conversation.history(editor);
        conversation.entries.push(Entry {
            request: request.clone(),
            reply: String::new(),
            outcome: Outcome::Working,
            details_open: false,
        });
        let ctx = ctx.clone();
        let receiver = vibedoc_ai::spawn(
            self.config.clone(),
            request,
            history,
            editor.document().clone(),
            editor.selection(),
            move || ctx.request_repaint(),
        );
        conversation.running = Some(Running { receiver, before: editor.document().clone() });
        conversation.progress = "Starting…".into();
        self.panel_open = true;
    }
}
