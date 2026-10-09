//! Settings that belong to this computer, not to a document: theme, page
//! color, and the library folder. Saved as TOML next to the assistant's config.

use std::path::PathBuf;

use eframe::egui::{Color32, ThemePreference};
use serde::{Deserialize, Serialize};

use crate::document::Paper;
use crate::library;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub theme: Theme,
    pub page: PageColor,
    /// `None` uses `~/Documents/VibeDoc`.
    pub library_folder: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Light, Theme::Dark, Theme::System];

    pub fn label(self) -> &'static str {
        match self {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
    }

    pub fn preference(self) -> ThemePreference {
        match self {
            Theme::System => ThemePreference::System,
            Theme::Light => ThemePreference::Light,
            Theme::Dark => ThemePreference::Dark,
        }
    }
}

/// The color of the paper the document is shown on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PageColor {
    #[default]
    White,
    Ivory,
    Sepia,
    Mint,
    Slate,
    Night,
}

impl PageColor {
    pub const ALL: [PageColor; 6] = [
        PageColor::White,
        PageColor::Ivory,
        PageColor::Sepia,
        PageColor::Mint,
        PageColor::Slate,
        PageColor::Night,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PageColor::White => "White",
            PageColor::Ivory => "Ivory",
            PageColor::Sepia => "Sepia",
            PageColor::Mint => "Mint",
            PageColor::Slate => "Slate",
            PageColor::Night => "Night",
        }
    }

    pub fn fill(self) -> Color32 {
        match self {
            PageColor::White => Color32::WHITE,
            PageColor::Ivory => Color32::from_rgb(252, 249, 242),
            PageColor::Sepia => Color32::from_rgb(244, 236, 216),
            PageColor::Mint => Color32::from_rgb(237, 245, 239),
            PageColor::Slate => Color32::from_rgb(44, 47, 56),
            PageColor::Night => Color32::from_rgb(24, 25, 30),
        }
    }

    /// Text color when a character doesn't set its own.
    pub fn ink(self) -> Color32 {
        match self {
            PageColor::Slate | PageColor::Night => Color32::from_rgb(226, 228, 233),
            PageColor::Sepia => Color32::from_rgb(58, 46, 30),
            _ => Color32::from_rgb(30, 32, 40),
        }
    }

    pub fn paper(self, accent: Color32) -> Paper {
        Paper { fill: self.fill(), ink: self.ink(), accent }
    }
}

impl Preferences {
    fn path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("vibedoc").join("preferences.toml"))
    }

    /// The saved preferences, or the defaults if there are none (or they can't be read).
    pub fn load() -> Preferences {
        Preferences::path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Preferences::path().ok_or("Couldn't find a config folder on this computer")?;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn library_root(&self) -> PathBuf {
        self.library_folder.clone().unwrap_or_else(library::default_root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_round_trip_through_toml() {
        let preferences = Preferences {
            theme: Theme::Dark,
            page: PageColor::Sepia,
            library_folder: Some("/tmp/docs".into()),
        };
        let text = toml::to_string_pretty(&preferences).unwrap();
        assert_eq!(toml::from_str::<Preferences>(&text).unwrap(), preferences);
        assert_eq!(toml::from_str::<Preferences>("").unwrap(), Preferences::default());
    }
}
