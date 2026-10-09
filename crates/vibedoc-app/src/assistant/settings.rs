//! The assistant settings window: which models to use, keys, and instructions.

use eframe::egui::{self, ComboBox, RichText, TextEdit, Ui};
use vibedoc_ai::config::{AgentConfig, AgentProvider, ImageConfig, ImageProvider};
use vibedoc_ai::Config;

use super::Assistant;
use crate::ui::theme::{self, palette};
use crate::ui::widgets::{button, ButtonKind};

pub struct Settings {
    draft: Config,
    error: Option<String>,
}

impl Assistant {
    pub fn open_settings(&mut self) {
        self.settings = Some(Settings { draft: self.config.clone(), error: None });
    }

    pub fn settings_window(&mut self, ctx: &egui::Context) {
        let Some(settings) = self.settings.as_mut() else { return };
        let mut open = true;
        let mut save = false;
        let mut close = false;
        egui::Window::new(RichText::new("Assistant settings").font(theme::semibold(15.0)))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(480.0)
            .show(ctx, |ui| {
                let p = palette(ui);
                section_title(ui, "Writing assistant", "Reads your requests and edits the document.");
                agent_settings(ui, &mut settings.draft.agent);
                ui.add_space(14.0);
                section_title(ui, "Pictures", "Draws images the assistant adds.");
                image_settings(ui, &mut settings.draft.images);
                ui.add_space(12.0);
                if let Some(path) = Config::path() {
                    let note =
                        format!("Saved to {}. API keys are stored there as plain text.", path.display());
                    ui.label(RichText::new(note).small().color(p.text_muted));
                }
                if let Some(error) = &settings.error {
                    ui.colored_label(p.danger, error);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    save = button(ui, ButtonKind::Primary, None, "Save").clicked();
                    close = button(ui, ButtonKind::Outline, None, "Cancel").clicked();
                });
            });
        if save {
            match settings.draft.save() {
                Ok(()) => {
                    self.config = settings.draft.clone();
                    self.settings = None;
                }
                Err(error) => settings.error = Some(error),
            }
        } else if !open || close {
            self.settings = None;
        }
    }
}

fn section_title(ui: &mut Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).font(theme::semibold(14.0)));
    ui.label(RichText::new(subtitle).small().color(palette(ui).text_muted));
    ui.add_space(4.0);
}

fn agent_settings(ui: &mut Ui, agent: &mut AgentConfig) {
    let mut provider = agent.provider;
    let label = |p: AgentProvider| match p {
        AgentProvider::Anthropic => "Claude (Anthropic)",
        AgentProvider::OpenaiCompatible => "OpenAI-compatible (OpenAI, Gemini, Ollama, …)",
    };
    ComboBox::from_id_salt("agent provider").selected_text(label(provider)).width(320.0).show_ui(ui, |ui| {
        for option in [AgentProvider::Anthropic, AgentProvider::OpenaiCompatible] {
            ui.selectable_value(&mut provider, option, label(option));
        }
    });
    if provider != agent.provider {
        let instructions = std::mem::take(&mut agent.instructions);
        *agent = AgentConfig { instructions, ..AgentConfig::for_provider(provider) };
    }
    egui::Grid::new("agent settings").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        connection_fields(ui, &mut agent.model, &mut agent.api_key, &agent.api_key_env, &mut agent.base_url);
    });
    ui.add_space(4.0);
    ui.label("Your instructions for the assistant (optional)");
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
    ComboBox::from_id_salt("image provider").selected_text(label(provider)).width(320.0).show_ui(ui, |ui| {
        for option in [ImageProvider::Gemini, ImageProvider::Openai] {
            ui.selectable_value(&mut provider, option, label(option));
        }
    });
    if provider != images.provider {
        *images = ImageConfig::for_provider(provider);
    }
    egui::Grid::new("image settings").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
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
    let width = 320.0;
    ui.label("Model");
    ui.add(TextEdit::singleline(model).desired_width(width));
    ui.end_row();
    ui.label("API key");
    ui.add(
        TextEdit::singleline(api_key)
            .password(true)
            .hint_text(format!("or set ${api_key_env}"))
            .desired_width(width),
    );
    ui.end_row();
    ui.label("Base URL");
    ui.add(TextEdit::singleline(base_url).desired_width(width));
    ui.end_row();
}
