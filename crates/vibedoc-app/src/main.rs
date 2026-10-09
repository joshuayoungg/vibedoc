//! Inkwell: a small word processor with an AI assistant. This file is the
//! window chrome: toolbar, find bar, status bar, files, and keyboard shortcuts.

mod assistant;
mod document_view;
mod fonts;

use std::path::PathBuf;

use eframe::egui::{
    self, Button, Color32, ComboBox, Frame, Key, KeyboardShortcut, Modifiers, Panel, RichText, Ui,
    ViewportCommand,
};
use inkwell_core::find::find_all;
use inkwell_core::{Document, Editor, ParagraphKind, Position, Selection, StyleFlag};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult};

use assistant::Assistant;
use document_view::DocumentView;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]).with_title("Inkwell"),
        ..Default::default()
    };
    let mut app = App::default();
    // `inkwell report.docx` opens that file.
    if let Some(path) = std::env::args().nth(1).map(PathBuf::from) {
        app.open_path(path);
    }
    eframe::run_native("Inkwell", options, Box::new(|_| Ok(Box::new(app))))
}

#[derive(Default)]
struct App {
    editor: Editor,
    view: DocumentView,
    file: Option<PathBuf>,
    saved_revision: u64,
    find: FindBar,
    status: String,
    window_title: String,
    assistant: Assistant,
    /// A text box other than the document had the keyboard last frame.
    typing_elsewhere: bool,
}

#[derive(Default)]
struct FindBar {
    open: bool,
    query: String,
    matches: Vec<(Position, Position)>,
    current: usize,
    has_focus: bool,
    wants_focus: bool,
    /// The query and document revision `matches` was computed for.
    searched: Option<(String, u64)>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);
        self.confirm_close(&ctx);
        if let Some(finished) = self.assistant.poll() {
            // Everything the assistant did is one undo step.
            self.editor.replace_document(finished.document);
            self.editor.set_selection(finished.selection);
            self.view.reveal_caret();
        }

        Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        if self.find.open {
            Panel::top("find").show(ui, |ui| self.find_bar(ui));
        }
        Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        if self.assistant.panel_open {
            Panel::right("assistant")
                .resizable(true)
                .default_size(340.0)
                .show(ui, |ui| self.assistant.panel(ui, &self.editor));
        }
        self.assistant.command_bar(&ctx, &self.editor);
        self.assistant.settings_window(&ctx);

        self.typing_elsewhere = self.find.has_focus || self.assistant.wants_keyboard();
        // While the assistant works on its copy, the document is read-only.
        let editable = !self.assistant.is_running();
        let page_background = Frame::new().fill(Color32::from_gray(225));
        egui::CentralPanel::default().frame(page_background).show(ui, |ui| {
            let highlights: &[(Position, Position)] = if self.find.open { &self.find.matches } else { &[] };
            self.view.show(ui, &mut self.editor, highlights, !self.typing_elsewhere, editable);
        });

        self.update_title(ui.ctx());
    }
}

impl App {
    fn is_dirty(&self) -> bool {
        self.editor.revision() != self.saved_revision
    }

    // ---- Chrome ----

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.button("New").clicked() {
                self.new_document();
            }
            if ui.button("Open…").clicked() {
                self.open();
            }
            if ui.button("Save").clicked() {
                self.save();
            }
            ui.separator();

            let editable = !self.assistant.is_running();
            ui.add_enabled_ui(editable, |ui| self.editing_tools(ui));
            ui.separator();

            if ui.button("Find").on_hover_text("Ctrl+F").clicked() {
                self.open_find();
            }
            if ui
                .add(Button::selectable(self.assistant.panel_open, "Assistant"))
                .on_hover_text("Ask with Ctrl+K")
                .clicked()
            {
                self.assistant.panel_open = !self.assistant.panel_open;
            }
        });
    }

    fn editing_tools(&mut self, ui: &mut Ui) {
        if ui.add_enabled(self.editor.can_undo(), Button::new("Undo")).clicked() {
            self.editor.undo();
        }
        if ui.add_enabled(self.editor.can_redo(), Button::new("Redo")).clicked() {
            self.editor.redo();
        }
        ui.separator();

        let style = self.editor.current_style();
        let buttons = [
            (style.bold, RichText::new(" B ").strong(), StyleFlag::Bold, "Bold (Ctrl+B)"),
            (style.italic, RichText::new(" I ").italics(), StyleFlag::Italic, "Italic (Ctrl+I)"),
            (style.underline, RichText::new(" U ").underline(), StyleFlag::Underline, "Underline (Ctrl+U)"),
        ];
        for (active, label, flag, tooltip) in buttons {
            if ui.add(Button::selectable(active, label)).on_hover_text(tooltip).clicked() {
                self.editor.toggle_style(flag);
            }
        }
        ui.separator();

        let current_kind = self.editor.current_paragraph_kind();
        let mut kind = current_kind;
        ComboBox::from_id_salt("paragraph kind").selected_text(kind.label()).width(130.0).show_ui(ui, |ui| {
            for option in ParagraphKind::ALL {
                ui.selectable_value(&mut kind, option, option.label());
            }
        });
        if kind != current_kind {
            self.editor.set_paragraph_kind(kind);
        }
    }

    fn find_bar(&mut self, ui: &mut Ui) {
        self.refresh_matches();
        ui.horizontal(|ui| {
            ui.label("Find:");
            let field = ui.text_edit_singleline(&mut self.find.query);
            if std::mem::take(&mut self.find.wants_focus) {
                field.request_focus();
            }
            self.find.has_focus = field.has_focus();
            let enter_pressed = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));

            let count = self.find.matches.len();
            match count {
                0 if self.find.query.is_empty() => {}
                0 => {
                    ui.label("No results");
                }
                _ => {
                    ui.label(format!("{} of {count}", self.find.current + 1));
                }
            }
            if ui.add_enabled(count > 0, Button::new("Previous")).clicked() {
                self.go_to_match(self.find.current + count - 1);
            }
            if ui.add_enabled(count > 0, Button::new("Next")).clicked() || (enter_pressed && count > 0) {
                self.go_to_match(self.find.current + 1);
                self.find.wants_focus = enter_pressed;
            }
            if ui.button("Close").clicked() {
                self.close_find();
            }
        });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let layout = self.view.layout(ui.ctx(), &self.editor);
            let page = layout.caret(self.editor.selection().cursor).page + 1;
            ui.label(format!("Page {page} of {}", layout.page_count));
            ui.separator();
            ui.label(format!("{} words", self.editor.document().word_count()));
            if !self.status.is_empty() {
                ui.separator();
                ui.label(&self.status);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::Slider::new(&mut self.view.zoom, 0.5..=3.0)
                        .text("Zoom")
                        .custom_formatter(|z, _| format!("{:.0}%", z * 100.0)),
                );
            });
        });
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self
            .file
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
        let unsaved = if self.is_dirty() { " •" } else { "" };
        let title = format!("{name}{unsaved} — Inkwell");
        if title != self.window_title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    // ---- Shortcuts ----

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let command = Modifiers::COMMAND;
        let command_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        let pressed =
            |modifiers, key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(modifiers, key)));

        if pressed(command_shift, Key::S) {
            self.save_as();
        } else if pressed(command, Key::S) {
            self.save();
        }
        if pressed(command, Key::O) {
            self.open();
        }
        if pressed(command, Key::N) {
            self.new_document();
        }
        if pressed(command, Key::F) {
            self.open_find();
        }
        if pressed(command, Key::K) {
            self.assistant.open_command_bar();
        }
        if self.find.open && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.close_find();
        }

        // The rest belong to the document, not to other text boxes, and wait
        // while the assistant is editing.
        if self.typing_elsewhere || self.assistant.is_running() {
            return;
        }
        if pressed(command_shift, Key::Z) || pressed(command, Key::Y) {
            self.editor.redo();
        } else if pressed(command, Key::Z) {
            self.editor.undo();
        }
        let formatting =
            [(Key::B, StyleFlag::Bold), (Key::I, StyleFlag::Italic), (Key::U, StyleFlag::Underline)];
        for (key, flag) in formatting {
            if pressed(command, key) {
                self.editor.toggle_style(flag);
            }
        }
        if pressed(command, Key::A) {
            self.editor.select_all();
        }
    }

    // ---- Find ----

    fn open_find(&mut self) {
        self.find.open = true;
        self.find.wants_focus = true;
        if !self.editor.selection().is_empty() {
            self.find.query = self.editor.selected_text();
        }
    }

    fn close_find(&mut self) {
        self.find.open = false;
        self.find.has_focus = false;
    }

    fn refresh_matches(&mut self) {
        let key = (self.find.query.clone(), self.editor.revision());
        if self.find.searched.as_ref() == Some(&key) {
            return;
        }
        let query_changed = self.find.searched.as_ref().is_none_or(|(query, _)| *query != key.0);
        self.find.matches = find_all(self.editor.document(), &self.find.query);
        self.find.current = self.find.current.min(self.find.matches.len().saturating_sub(1));
        self.find.searched = Some(key);
        // A new search jumps to the first match at or after the caret, like most editors.
        // (Editing the document only refreshes the highlights.)
        if query_changed {
            let caret = self.editor.selection().start();
            let index = self.find.matches.iter().position(|(start, _)| *start >= caret).unwrap_or(0);
            self.go_to_match(index);
        }
    }

    fn go_to_match(&mut self, index: usize) {
        let count = self.find.matches.len();
        if count == 0 {
            return;
        }
        self.find.current = index % count;
        let (start, end) = self.find.matches[self.find.current];
        self.editor.set_selection(Selection { anchor: start, cursor: end });
        self.view.reveal_caret();
    }

    // ---- Files ----

    fn new_document(&mut self) {
        if self.discard_changes_ok() {
            self.load(Document::default(), None);
        }
    }

    fn open(&mut self) {
        if !self.discard_changes_ok() {
            return;
        }
        if let Some(path) = FileDialog::new().add_filter("Word document", &["docx"]).pick_file() {
            self.open_path(path);
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        match inkwell_docx::open(&path) {
            Ok(document) => {
                self.status = format!("Opened {}", path.display());
                self.load(document, Some(path));
            }
            Err(error) => self.status = format!("Couldn't open {}: {error}", path.display()),
        }
    }

    fn load(&mut self, document: Document, file: Option<PathBuf>) {
        self.assistant.cancel();
        self.editor = Editor::new(document);
        self.saved_revision = self.editor.revision();
        self.file = file;
        self.view.reset();
        self.find.searched = None;
    }

    fn save(&mut self) {
        match self.file.clone() {
            Some(path) => self.save_to(path),
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) {
        let dialog = FileDialog::new().add_filter("Word document", &["docx"]).set_file_name("Untitled.docx");
        if let Some(path) = dialog.save_file() {
            self.save_to(path.with_extension("docx"));
        }
    }

    fn save_to(&mut self, path: PathBuf) {
        match inkwell_docx::save(self.editor.document(), &path) {
            Ok(()) => {
                self.status = format!("Saved {}", path.display());
                self.saved_revision = self.editor.revision();
                self.file = Some(path);
            }
            Err(error) => self.status = format!("Couldn't save: {error}"),
        }
    }

    /// Asks before throwing away unsaved work. Returns true if it's fine to continue.
    fn discard_changes_ok(&self) -> bool {
        !self.is_dirty()
            || MessageDialog::new()
                .set_title("Unsaved changes")
                .set_description("Discard your unsaved changes?")
                .set_buttons(MessageButtons::YesNo)
                .show()
                == MessageDialogResult::Yes
    }

    fn confirm_close(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.discard_changes_ok() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        }
    }
}
