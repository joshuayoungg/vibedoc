//! Keyboard shortcuts for the whole window, following Word where it has one.

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};
use vibedoc_core::{Alignment, StyleFlag};

use super::App;

impl App {
    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
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
        if pressed(command, Key::P) {
            self.export_pdf(ctx);
        }
        if pressed(command, Key::N) || pressed(command, Key::T) {
            self.new_tab();
        }
        if pressed(command, Key::W) {
            self.close_tab(self.active);
        }
        if pressed(Modifiers::CTRL | Modifiers::SHIFT, Key::Tab) {
            self.next_tab(-1);
        } else if pressed(Modifiers::CTRL, Key::Tab) {
            self.next_tab(1);
        }
        if pressed(command, Key::F) {
            self.open_find();
        }
        if pressed(command, Key::K) {
            self.assistant.open_command_bar();
        }
        if pressed(command, Key::Backslash) {
            self.sidebar.open = !self.sidebar.open;
        }
        if self.find.open && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.close_find();
        }

        // The rest belong to the document, not to other text boxes, and wait
        // while the assistant is editing.
        if self.typing_elsewhere || self.tab().conversation.is_running() {
            return;
        }
        let editor = &mut self.tabs[self.active].editor;
        if pressed(command_shift, Key::Z) || pressed(command, Key::Y) {
            editor.redo();
        } else if pressed(command, Key::Z) {
            editor.undo();
        }
        if pressed(command_shift, Key::X) {
            editor.toggle_style(StyleFlag::Strikethrough);
        }
        let formatting =
            [(Key::B, StyleFlag::Bold), (Key::I, StyleFlag::Italic), (Key::U, StyleFlag::Underline)];
        for (key, flag) in formatting {
            if pressed(command, key) {
                editor.toggle_style(flag);
            }
        }
        let alignments = [
            (Key::L, Alignment::Left),
            (Key::E, Alignment::Center),
            (Key::R, Alignment::Right),
            (Key::J, Alignment::Justify),
        ];
        for (key, alignment) in alignments {
            if pressed(command, key) {
                editor.format_paragraphs(|p| p.alignment = alignment);
            }
        }
        if pressed(command, Key::CloseBracket) {
            editor.step_font_size(1);
        }
        if pressed(command, Key::OpenBracket) {
            editor.step_font_size(-1);
        }
        if pressed(command, Key::A) {
            editor.select_all();
        }
    }
}
