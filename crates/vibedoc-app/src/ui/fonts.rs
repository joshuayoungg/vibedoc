//! The bundled typefaces: Inter for the interface, plus every font a
//! document can use. Each has a regular and a bold face.

use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use vibedoc_core::Font;

/// The font file for a typeface, regular or bold.
pub fn file(font: Font, bold: bool) -> &'static [u8] {
    let (regular, bold_face) = files(font);
    if bold {
        bold_face
    } else {
        regular
    }
}

/// The font files for one typeface: (regular, bold).
fn files(font: Font) -> (&'static [u8], &'static [u8]) {
    macro_rules! faces {
        ($dir:literal, $regular:literal, $bold:literal) => {
            (
                include_bytes!(concat!("../../assets/fonts/", $dir, "/", $regular)).as_slice(),
                include_bytes!(concat!("../../assets/fonts/", $dir, "/", $bold)).as_slice(),
            )
        };
    }
    match font {
        Font::Inter => faces!("Inter", "Inter-Regular.ttf", "Inter-SemiBold.ttf"),
        Font::Poppins => faces!("Poppins", "Poppins-Regular.ttf", "Poppins-Bold.ttf"),
        Font::Lora => faces!("Lora", "Lora-Regular.ttf", "Lora-Bold.ttf"),
        Font::Merriweather => faces!("Merriweather", "Merriweather-Regular.ttf", "Merriweather-Bold.ttf"),
        Font::PlayfairDisplay => {
            faces!("PlayfairDisplay", "PlayfairDisplay-Regular.ttf", "PlayfairDisplay-Bold.ttf")
        }
        Font::JetBrainsMono => faces!("JetBrainsMono", "JetBrainsMono-Regular.ttf", "JetBrainsMono-Bold.ttf"),
    }
}

/// The egui font family for a typeface and weight.
pub fn family(font: Font, bold: bool) -> FontFamily {
    let name = if bold { format!("{} Bold", font.name()) } else { font.name().to_string() };
    FontFamily::Name(name.into())
}

pub fn install(ctx: &egui::Context) {
    let mut definitions = FontDefinitions::default();
    // Symbols the bundled fonts lack fall back to egui's built-in fonts.
    let fallbacks = definitions.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    for font in Font::ALL {
        let (regular, bold) = files(font);
        for (is_bold, bytes) in [(false, regular), (true, bold)] {
            let FontFamily::Name(name) = family(font, is_bold) else { unreachable!() };
            definitions.font_data.insert(name.to_string(), Arc::new(FontData::from_static(bytes)));
            let mut chain = vec![name.to_string()];
            chain.extend(fallbacks.iter().cloned());
            definitions.families.insert(FontFamily::Name(name), chain);
        }
    }
    // The interface itself is set in Inter.
    let proportional = definitions.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, Font::Inter.name().to_string());
    ctx.set_fonts(definitions);
}
