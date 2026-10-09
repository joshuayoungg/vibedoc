//! The assistant's UI: a chat side panel, the Ctrl+K command bar, and settings.
//! The actual work happens in the `inkwell-ai` crate, on a background thread.

use std::sync::mpsc::Receiver;

use eframe::egui::{
    self, Align, Button, Color32, ComboBox, Key, KeyboardShortcut, Layout, Modifiers, RichText, ScrollArea,
    TextEdit, Ui,
};
use inkwell_ai::config::{AgentConfig, AgentProvider, ImageConfig, ImageProvider};
use inkwell_ai::{AgentEvent, Config, Turn};
use inkwell_core::{Document, Editor, Selection};

pub struct Assistant {
    pub panel_open: bool,
    config: Config,
    entries: Vec<Entry>,
    input: String,
    running: Option<Receiver<AgentEvent>>,
    progress: String,
    command_bar: Option<String>,
    settings: Option<Settings>,
    input_has_focus: bool,
}

struct Entry {
    request: String,
    reply: String,
    failed: bool,
}

struct Settings {
    draft: Config,
    error: Option<String>,
}

/// What the assistant finished, for the app to apply to the editor.
pub struct Finished {
    pub document: Document,
    pub selection: Selection,
}

const EXAMPLES: [&str; 4] = [
    "Turn my notes into a bulleted list",
    "Make the selected text bold",
    "Draw a watercolor fox and put it here",
    "Add a title and a short intro",
];

impl Default for Assistant {
    fn default() -> Self {
        let (config, load_error) = match Config::load() {
            Ok(config) => (config, None),
            Err(error) => (Config::default(), Some(error)),
        };
        let mut assistant = Assistant {
            panel_open: true,
            config,
            entries: Vec::new(),
            input: String::new(),
            running: None,
            progress: String::new(),
            command_bar: None,
            settings: None,
            input_has_focus: false,
        };
        if let Some(error) = load_error {
            assistant.entries.push(Entry {
                request: "(loading settings)".into(),
                reply: format!("Your settings file couldn't be read, so defaults are in use: {error}"),
                failed: true,
            });
        }
        assistant
    }
}

impl Assistant {
    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// True while one of the assistant's text boxes or windows wants the keyboard.
    pub fn wants_keyboard(&self) -> bool {
        self.input_has_focus || self.command_bar.is_some() || self.settings.is_some()
    }

    pub fn open_command_bar(&mut self) {
        if self.command_bar.is_none() {
            self.command_bar = Some(String::new());
        }
    }

    pub fn cancel(&mut self) {
        // The background thread finishes on its own; its result is ignored.
        self.running = None;
    }

    pub fn submit(&mut self, request: String, editor: &Editor, ctx: &egui::Context) {
        let request = request.trim().to_string();
        if request.is_empty() || self.is_running() {
            return;
        }
        let history: Vec<Turn> = self
            .entries
            .iter()
            .filter(|e| !e.failed)
            .map(|e| Turn { request: e.request.clone(), reply: e.reply.clone() })
            .collect();
        self.entries.push(Entry { request: request.clone(), reply: String::new(), failed: false });
        let ctx = ctx.clone();
        self.running = Some(inkwell_ai::spawn(
            self.config.clone(),
            request,
            history,
            editor.document().clone(),
            editor.selection(),
            move || ctx.request_repaint(),
        ));
        self.progress = "Starting…".into();
        self.panel_open = true;
    }

    /// Checks on the background work. Returns the result once it's done.
    pub fn poll(&mut self) -> Option<Finished> {
        let receiver = self.running.as_ref()?;
        let mut finished = None;
        while let Ok(event) = receiver.try_recv() {
            match event {
                AgentEvent::Progress(note) => self.progress = note,
                AgentEvent::Finished { document, selection, reply } => {
                    self.set_last_reply(reply, false);
                    finished = Some(Finished { document, selection });
                    self.running = None;
                    break;
                }
                AgentEvent::Failed(error) => {
                    self.set_last_reply(error, true);
                    self.running = None;
                    break;
                }
            }
        }
        finished
    }

    fn set_last_reply(&mut self, reply: String, failed: bool) {
        if let Some(entry) = self.entries.last_mut() {
            entry.reply = if reply.trim().is_empty() { "Done.".into() } else { reply };
            entry.failed = failed;
        }
    }

    // ---- Side panel ----

    pub fn panel(&mut self, ui: &mut Ui, editor: &Editor) {
        ui.horizontal(|ui| {
            ui.heading("Assistant");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Settings").clicked() {
                    self.open_settings();
                }
            });
        });
        ui.label(RichText::new(self.model_summary()).small().weak());
        ui.separator();

        let input_height = 90.0;
        ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink(false)
            .max_height(ui.available_height() - input_height)
            .show(ui, |ui| self.conversation(ui, editor));

        ui.separator();
        let running = self.is_running();
        let field = TextEdit::multiline(&mut self.input)
            .hint_text("Ask for an edit… (Enter to send, Shift+Enter for a new line)")
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .return_key(KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter));
        let response = ui.add_enabled(!running, field);
        self.input_has_focus = response.has_focus();
        let enter = response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift);
        let send = ui.add_enabled(!running && !self.input.trim().is_empty(), Button::new("Send"));
        if enter || send.clicked() {
            let request = std::mem::take(&mut self.input);
            self.submit(request, editor, ui.ctx());
        }
    }

    fn conversation(&mut self, ui: &mut Ui, editor: &Editor) {
        if self.entries.is_empty() {
            ui.label("Tell me what to do with your document. For example:");
            let mut picked = None;
            for example in EXAMPLES {
                if ui.link(format!("“{example}”")).clicked() {
                    picked = Some(example.to_string());
                }
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Press Ctrl+K anywhere to ask without the panel.").weak());
            if let Some(example) = picked {
                self.submit(example, editor, ui.ctx());
            }
            return;
        }
        let last = self.entries.len() - 1;
        for (index, entry) in self.entries.iter().enumerate() {
            ui.label(RichText::new(&entry.request).strong());
            if index == last && self.running.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(&self.progress);
                });
            } else if entry.failed {
                ui.label(RichText::new(&entry.reply).color(Color32::from_rgb(220, 90, 80)));
            } else {
                ui.label(&entry.reply);
            }
            ui.add_space(10.0);
        }
        if self.running.is_some() && ui.button("Cancel").clicked() {
            self.cancel();
            self.set_last_reply("Cancelled.".into(), true);
        }
    }

    fn model_summary(&self) -> String {
        format!("{} · images: {}", self.config.agent.model, self.config.images.model)
    }

    // ---- Ctrl+K command bar ----

    pub fn command_bar(&mut self, ctx: &egui::Context, editor: &Editor) {
        let Some(text) = self.command_bar.as_mut() else { return };
        let mut submit = false;
        let mut close = ctx.input(|i| i.key_pressed(Key::Escape));
        egui::Window::new("Ask the assistant")
            .title_bar(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .fixed_size([520.0, 0.0])
            .show(ctx, |ui| {
                let field = TextEdit::singleline(text)
                    .hint_text("What should I do? e.g. “make this a numbered list”")
                    .desired_width(f32::INFINITY);
                let response = ui.add(field);
                submit = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if !submit {
                    response.request_focus();
                }
                ui.label(RichText::new("Enter to send · Esc to close").small().weak());
            });
        if submit {
            let request = std::mem::take(text);
            self.submit(request, editor, ctx);
            close = true;
        }
        if close {
            self.command_bar = None;
        }
    }

    // ---- Settings ----

    fn open_settings(&mut self) {
        self.settings = Some(Settings { draft: self.config.clone(), error: None });
    }

    pub fn settings_window(&mut self, ctx: &egui::Context) {
        let Some(settings) = self.settings.as_mut() else { return };
        let mut open = true;
        let mut save = false;
        egui::Window::new("Assistant settings").open(&mut open).collapsible(false).default_width(460.0).show(
            ctx,
            |ui| {
                ui.heading("Writing assistant");
                agent_settings(ui, &mut settings.draft.agent);
                ui.add_space(8.0);
                ui.heading("Pictures");
                image_settings(ui, &mut settings.draft.images);
                ui.add_space(8.0);
                if let Some(path) = Config::path() {
                    ui.label(
                        RichText::new(format!(
                            "Saved to {}. API keys are stored there as plain text.",
                            path.display()
                        ))
                        .small()
                        .weak(),
                    );
                }
                if let Some(error) = &settings.error {
                    ui.colored_label(Color32::from_rgb(220, 90, 80), error);
                }
                ui.horizontal(|ui| {
                    save = ui.button("Save").clicked();
                });
            },
        );
        if save {
            match settings.draft.save() {
                Ok(()) => {
                    self.config = settings.draft.clone();
                    self.settings = None;
                }
                Err(error) => settings.error = Some(error),
            }
        } else if !open {
            self.settings = None;
        }
    }
}

fn agent_settings(ui: &mut Ui, agent: &mut AgentConfig) {
    let mut provider = agent.provider;
    let label = |p: AgentProvider| match p {
        AgentProvider::Anthropic => "Claude (Anthropic)",
        AgentProvider::OpenaiCompatible => "OpenAI-compatible (OpenAI, Gemini, Ollama, …)",
    };
    ComboBox::from_id_salt("agent provider").selected_text(label(provider)).width(300.0).show_ui(ui, |ui| {
        for option in [AgentProvider::Anthropic, AgentProvider::OpenaiCompatible] {
            ui.selectable_value(&mut provider, option, label(option));
        }
    });
    if provider != agent.provider {
        let instructions = std::mem::take(&mut agent.instructions);
        *agent = AgentConfig { instructions, ..AgentConfig::for_provider(provider) };
    }
    egui::Grid::new("agent settings").num_columns(2).show(ui, |ui| {
        connection_fields(ui, &mut agent.model, &mut agent.api_key, &agent.api_key_env, &mut agent.base_url);
    });
    ui.label("Your instructions for the assistant (optional):");
    ui.add(
        TextEdit::multiline(&mut agent.instructions)
            .hint_text("e.g. Write in British English. Keep a friendly tone.")
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );
}

fn image_settings(ui: &mut Ui, images: &mut ImageConfig) {
    let mut provider = images.provider;
    let label = |p: ImageProvider| match p {
        ImageProvider::Gemini => "Gemini",
        ImageProvider::Openai => "OpenAI",
    };
    ComboBox::from_id_salt("image provider").selected_text(label(provider)).width(300.0).show_ui(ui, |ui| {
        for option in [ImageProvider::Gemini, ImageProvider::Openai] {
            ui.selectable_value(&mut provider, option, label(option));
        }
    });
    if provider != images.provider {
        *images = ImageConfig::for_provider(provider);
    }
    egui::Grid::new("image settings").num_columns(2).show(ui, |ui| {
        connection_fields(
            ui,
            &mut images.model,
            &mut images.api_key,
            &images.api_key_env,
            &mut images.base_url,
        );
    });
}

/// Model, key, and URL fields, shared by both sections.
fn connection_fields(
    ui: &mut Ui,
    model: &mut String,
    api_key: &mut String,
    api_key_env: &str,
    base_url: &mut String,
) {
    ui.label("Model");
    ui.text_edit_singleline(model);
    ui.end_row();
    ui.label("API key");
    ui.add(TextEdit::singleline(api_key).password(true).hint_text(format!("or set ${api_key_env}")));
    ui.end_row();
    ui.label("Base URL");
    ui.text_edit_singleline(base_url);
    ui.end_row();
}
