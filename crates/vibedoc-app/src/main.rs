//! VibeDoc: a small word processor with an AI assistant.
//!
//! - `app`: the window: tabs, title, File menu, ribbon, find, status bar, files, shortcuts, preferences
//! - `document`: the pages, and turning input into editor commands
//! - `library`: the folders-and-documents sidebar
//! - `assistant`: the assistant panel, its review cards, and settings
//! - `ui`: colors, fonts, icons, and shared widgets

mod app;
mod assistant;
mod document;
mod library;
mod ui;

use std::path::PathBuf;

use eframe::egui;

use app::App;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([900.0, 560.0])
            .with_title("VibeDoc"),
        ..Default::default()
    };
    let mut app = App::default();
    // `vibedoc report.docx notes.docx` opens each file in its own tab.
    for path in std::env::args().skip(1).map(PathBuf::from) {
        app.open_path(path);
    }
    eframe::run_native(
        "VibeDoc",
        options,
        Box::new(|creation| {
            ui::theme::install(&creation.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}
