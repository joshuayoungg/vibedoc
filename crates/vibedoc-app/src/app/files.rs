//! Opening, saving, exporting, copying, renaming, and closing documents.
//! Documents save themselves (see `autosave`), so closing just saves.

use std::path::PathBuf;

use eframe::egui::{self, ViewportCommand};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

use super::tabs::Tab;
use super::App;
use crate::library;

impl App {
    pub(super) fn open(&mut self) {
        let dialog =
            FileDialog::new().add_filter("Word document", &["docx"]).set_directory(self.sidebar.root());
        for path in dialog.pick_files().unwrap_or_default() {
            self.open_in_tab(path);
        }
    }

    /// Opens a file from the command line.
    pub fn open_path(&mut self, path: PathBuf) {
        self.open_in_tab(path);
    }

    pub(super) fn save(&mut self) {
        self.save_tab(self.active);
    }

    /// Saves a tab's document to its file. A new document is saved straight
    /// into the library under its name, so it shows up in the sidebar.
    /// Returns false if saving failed.
    pub(super) fn save_tab(&mut self, index: usize) -> bool {
        let tab = &self.tabs[index];
        let result = match &tab.file {
            Some(path) => vibedoc_docx::save(tab.editor.document(), path)
                .map(|()| path.clone())
                .map_err(|e| e.to_string()),
            None => library::save_new(tab.editor.document(), self.sidebar.root(), &tab.name),
        };
        match result {
            Ok(path) => {
                self.saved(index, path);
                true
            }
            Err(error) => {
                self.status = format!("Couldn't save “{}”: {error}", self.tabs[index].name);
                // Try again after the next pause instead of on every frame.
                self.tabs[index].changed.1 = std::time::Instant::now();
                false
            }
        }
    }

    pub(super) fn save_as(&mut self) {
        let dialog = FileDialog::new()
            .add_filter("Word document", &["docx"])
            .set_directory(self.current_folder())
            .set_file_name(format!("{}.docx", self.tab().name));
        let Some(path) = dialog.save_file() else { return };
        let path = path.with_extension("docx");
        match vibedoc_docx::save(self.tab().editor.document(), &path) {
            Ok(()) => self.saved(self.active, path),
            Err(error) => self.status = format!("Couldn't save: {error}"),
        }
    }

    fn saved(&mut self, index: usize, path: PathBuf) {
        let tab = &mut self.tabs[index];
        let is_new_file = tab.file.as_ref() != Some(&path);
        tab.saved_revision = tab.editor.revision();
        tab.name = library::display_name(&path);
        tab.file = Some(path);
        self.status.clear();
        if is_new_file {
            self.sidebar.rescan();
        }
    }

    /// The folder the document is in, or the library for a new one.
    fn current_folder(&self) -> PathBuf {
        let folder = self.tab().file.as_ref().and_then(|f| f.parent()).map(PathBuf::from);
        let folder = folder.unwrap_or_else(|| self.sidebar.root().to_path_buf());
        let _ = std::fs::create_dir_all(&folder);
        folder
    }

    /// Writes the document as a PDF, laid out exactly as it is on screen.
    pub(super) fn export_pdf(&mut self, ctx: &egui::Context) {
        let dialog = FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .set_directory(self.current_folder())
            .set_file_name(format!("{}.pdf", self.tab().name));
        let Some(path) = dialog.save_file() else { return };
        let path = path.with_extension("pdf");
        let tab = &mut self.tabs[self.active];
        let layout = tab.view.layout(ctx, &tab.editor);
        let written = vibedoc_pdf::export(tab.editor.document(), &layout, crate::ui::fonts::file)
            .and_then(|bytes| std::fs::write(&path, bytes).map_err(|e| e.to_string()));
        self.status = match written {
            Ok(()) => format!("Exported {}", path.file_name().unwrap_or_default().to_string_lossy()),
            Err(error) => format!("Couldn't export the PDF: {error}"),
        };
    }

    /// A copy in a new tab: saved next to the original, or untitled if the
    /// original was never saved.
    pub(super) fn make_copy(&mut self) {
        let document = self.tab().editor.document().clone();
        match self.tab().file.clone() {
            Some(original) => match library::save_copy(&document, &original) {
                Ok(path) => {
                    self.sidebar.rescan();
                    self.open_in_tab(path);
                }
                Err(error) => self.status = format!("Couldn't make a copy: {error}"),
            },
            None => {
                let name = format!("{} copy", self.tab().name);
                self.push_tab(Tab::new(name, document, None));
            }
        }
    }

    /// Renames the document, and its file if it has one.
    pub(super) fn rename_active(&mut self, name: &str) {
        let Some(file) = self.tab().file.clone() else {
            self.tab_mut().name = name.to_string();
            return;
        };
        match library::rename(&file, name) {
            Ok(path) => {
                let tab = self.tab_mut();
                tab.name = library::display_name(&path);
                tab.file = Some(path);
                self.sidebar.rescan();
            }
            Err(error) => self.status = format!("Couldn't rename: {error}"),
        }
    }

    /// Saves and closes a tab. Only asks if saving fails.
    pub(super) fn close_tab(&mut self, index: usize) {
        let tab = &self.tabs[index];
        if tab.is_dirty() && tab.has_content() && !self.save_tab(index) {
            let question = format!("{} Close it without saving?", self.status);
            if !ask(&question) {
                return;
            }
        }
        self.remove_tab(index);
    }

    /// Removes a tab without saving. There's always at least one tab.
    pub(super) fn remove_tab(&mut self, index: usize) {
        self.tabs[index].conversation.cancel();
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.tabs.push(Tab::untitled("Untitled".into()));
        }
        if self.active > index || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
        self.title_edit = None;
        self.find.searched = None;
    }

    /// Saves everything on the way out. Only asks if something couldn't be saved.
    pub(super) fn confirm_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        let mut failed = 0;
        for index in 0..self.tabs.len() {
            let tab = &self.tabs[index];
            if tab.is_dirty() && tab.has_content() && !self.save_tab(index) {
                failed += 1;
            }
        }
        if failed > 0 && !ask(&format!("{} Quit without saving?", self.status)) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        }
    }
}

fn ask(question: &str) -> bool {
    MessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title("Couldn't save")
        .set_description(question)
        .set_buttons(MessageButtons::YesNo)
        .show()
        == MessageDialogResult::Yes
}
