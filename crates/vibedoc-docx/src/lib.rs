//! Open and save Word (.docx) files.
//!
//! Supported: paragraphs, Heading 1-3, bulleted and numbered lists,
//! bold, italic, underline, and tabs. Everything else is dropped on open.

mod reader;
mod writer;

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use vibedoc_core::Document;

pub use reader::read_from;
pub use writer::write_to;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub fn open(path: &Path) -> Result<Document, Error> {
    read_from(BufReader::new(File::open(path)?))
}

pub fn save(document: &Document, path: &Path) -> Result<(), Error> {
    write_to(document, File::create(path)?)
}
