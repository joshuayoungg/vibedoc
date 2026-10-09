//! Try the assistant without the app:
//!
//!     cargo run -p vibedoc-ai --example ask -- report.docx "make the title bold"
//!
//! Prints what the assistant did and saves the result next to the original.

use vibedoc_core::Selection;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (Some(path), Some(request)) = (args.next(), args.next()) else {
        return Err("usage: ask <file.docx> \"<request>\"".into());
    };
    let document = vibedoc_docx::open(path.as_ref()).map_err(|e| e.to_string())?;
    let config = vibedoc_ai::Config::load()?;
    let progress = |note: String| eprintln!("{note}");
    let (document, _, reply) =
        vibedoc_ai::agent::run(&config, &request, &[], document, Selection::default(), &progress)?;
    let output = path.replace(".docx", ".edited.docx");
    vibedoc_docx::save(&document, output.as_ref()).map_err(|e| e.to_string())?;
    println!("{reply}\nSaved {output}");
    Ok(())
}
