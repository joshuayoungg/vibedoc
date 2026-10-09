//! The window: three cards side by side (library, document, assistant), each
//! with the same gap around it and headers on the same line.

mod file_menu;
mod files;
mod find;
mod preferences;
mod shortcuts;
mod status_bar;
mod tabs;
mod title;
mod toolbar;

use eframe::egui::{self, Align2, CentralPanel, Frame, Margin, Panel, Rect, RichText, Spinner, Ui};

use crate::assistant::{self, Assistant};
use crate::document::PageLook;
use crate::library::sidebar::{Sidebar, SidebarAction};
use crate::ui::theme::{self, palette, GAP};
use find::FindBar;
use preferences::Preferences;
use tabs::Tab;
use title::TitleEdit;

pub struct App {
    tabs: Vec<Tab>,
    active: usize,
    assistant: Assistant,
    sidebar: Sidebar,
    preferences: Preferences,
    find: FindBar,
    status: String,
    window_title: String,
    /// The document name being edited in place, if any.
    title_edit: Option<TitleEdit>,
    /// A text box other than the document had the keyboard last frame.
    typing_elsewhere: bool,
}

impl Default for App {
    fn default() -> Self {
        let preferences = Preferences::load();
        App {
            tabs: vec![Tab::untitled("Untitled".into())],
            active: 0,
            assistant: Assistant::default(),
            sidebar: Sidebar::new(preferences.library_root()),
            preferences,
            find: FindBar::default(),
            status: String::new(),
            window_title: String::new(),
            title_edit: None,
            typing_elsewhere: false,
        }
    }
}

impl eframe::App for App {
    /// The canvas the cards sit on.
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        theme::palette_for(visuals.dark_mode).window.to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.set_theme(self.preferences.theme.preference());
        self.handle_shortcuts(&ctx);
        self.confirm_close(&ctx);
        self.apply_finished_requests();
        self.autosave(&ctx);
        let p = palette(ui);

        // Side cards first, so the document card fills what's left between them.
        if self.sidebar.open {
            let frame = theme::card(p).outer_margin(Margin { left: GAP, right: 0, top: GAP, bottom: GAP });
            let (file, dirty) = (self.tab().file.clone(), self.tab().is_dirty());
            let actions = Panel::left("library")
                .frame(frame)
                .resizable(true)
                .default_size(264.0)
                .size_range(220.0..=420.0)
                .show_separator_line(false)
                .show(ui, |ui| self.sidebar.show(ui, file.as_deref(), dirty))
                .inner;
            for action in actions {
                self.library_action(action);
            }
        }
        if self.assistant.panel_open {
            let frame = theme::card(p).outer_margin(Margin { left: 0, right: GAP, top: GAP, bottom: GAP });
            let tab = &mut self.tabs[self.active];
            let action = Panel::right("assistant")
                .frame(frame)
                .resizable(true)
                .default_size(372.0)
                .size_range(320.0..=560.0)
                .show_separator_line(false)
                .show(ui, |ui| self.assistant.panel(ui, &mut tab.conversation, &tab.editor))
                .inner;
            match action {
                Some(assistant::Action::Apply(document)) => tab.editor.replace_document(document),
                Some(assistant::Action::Reveal(paragraph)) => tab.view.reveal_paragraph(paragraph),
                None => {}
            }
        }
        let tab = &mut self.tabs[self.active];
        self.assistant.command_bar(&ctx, &mut tab.conversation, &tab.editor);
        self.assistant.settings_window(&ctx);

        self.typing_elsewhere = self.find.has_focus
            || self.assistant.wants_keyboard()
            || self.sidebar.wants_keyboard()
            || self.title_edit.is_some();
        CentralPanel::default().frame(Frame::NONE.inner_margin(Margin::same(GAP))).show(ui, |ui| {
            theme::card(p).inner_margin(Margin::same(14)).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                let bar = |top: i8, bottom: i8| {
                    Frame::NONE.inner_margin(Margin { top, bottom, ..Default::default() })
                };
                Panel::top("title")
                    .frame(bar(0, 0))
                    .show_separator_line(false)
                    .show(ui, |ui| self.title_row(ui));
                Panel::top("tabs")
                    .frame(bar(0, 0))
                    .show_separator_line(false)
                    .show(ui, |ui| self.tab_bar(ui));
                Panel::top("toolbar")
                    .frame(bar(10, 10))
                    .show_separator_line(false)
                    .show(ui, |ui| self.toolbar(ui));
                Panel::bottom("status")
                    .frame(bar(8, 0))
                    .show_separator_line(false)
                    .show(ui, |ui| self.status_bar(ui));
                CentralPanel::no_frame().show(ui, |ui| self.pages(ui));
            });
        });

        self.update_title(ui.ctx());
    }
}

impl App {
    fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    /// Requests run in the background; each lands in the tab it was asked in.
    fn apply_finished_requests(&mut self) {
        for tab in &mut self.tabs {
            if let Some(finished) = tab.conversation.poll() {
                // Everything the assistant did is one undo step.
                if finished.document != *tab.editor.document() {
                    tab.editor.replace_document(finished.document);
                }
                tab.editor.set_selection(finished.selection);
                tab.view.reveal_caret();
            }
        }
    }

    /// The pages, on their own softly tinted canvas.
    fn pages(&mut self, ui: &mut Ui) {
        let p = palette(ui);
        let paper = self.preferences.page.paper(p.accent);
        let editable = !self.tab().conversation.is_running();
        let keep_focus = !self.typing_elsewhere;
        let canvas = Frame::new().fill(p.canvas).corner_radius(10).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            let tab = &mut self.tabs[self.active];
            let look = PageLook {
                paper,
                found: if self.find.open { &self.find.matches } else { &[] },
                changed: if self.assistant.panel_open { tab.conversation.highlights() } else { &[] },
            };
            tab.view.show(ui, &mut tab.editor, &look, keep_focus, editable);
        });
        let canvas = canvas.response.rect;
        if self.find.open {
            self.find_overlay(ui, canvas);
        }
        if !editable {
            working_notice(ui, canvas);
        }
    }

    fn library_action(&mut self, action: SidebarAction) {
        match action {
            SidebarAction::Open(path) => self.open_in_tab(path),
            SidebarAction::Moved { from, to } => {
                for tab in &mut self.tabs {
                    let Some(file) = &tab.file else { continue };
                    let Ok(inside) = file.strip_prefix(&from) else { continue };
                    let moved = if inside.as_os_str().is_empty() { to.clone() } else { to.join(inside) };
                    tab.name = crate::library::display_name(&moved);
                    tab.file = Some(moved);
                }
            }
            SidebarAction::Deleted(path) => {
                // The user deleted these on purpose, so their tabs close without saving.
                let deleted = |tab: &Tab| tab.file.as_ref().is_some_and(|file| file.starts_with(&path));
                while let Some(index) = self.tabs.iter().position(deleted) {
                    self.remove_tab(index);
                }
            }
        }
    }

    fn save_preferences(&mut self) {
        if let Err(error) = self.preferences.save() {
            self.status = format!("Couldn't save preferences: {error}");
        }
    }
}

/// A floating note over the pages while the assistant edits them.
fn working_notice(ui: &Ui, canvas: Rect) {
    let color = palette(ui).text_muted;
    let accent = palette(ui).accent;
    egui::Area::new(egui::Id::new("assistant working"))
        .order(egui::Order::Foreground)
        .pivot(Align2::CENTER_TOP)
        .fixed_pos(canvas.center_top() + egui::vec2(0.0, 14.0))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            Frame::popup(ui.style()).corner_radius(20).inner_margin(Margin::symmetric(14, 8)).show(
                ui,
                |ui| {
                    ui.horizontal(|ui| {
                        ui.add(Spinner::new().size(14.0).color(accent));
                        let note = "The assistant is editing. The page is read-only until it's done.";
                        ui.label(RichText::new(note).color(color));
                    });
                },
            );
        });
}
