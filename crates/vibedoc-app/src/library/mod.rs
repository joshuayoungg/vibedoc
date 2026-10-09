//! The document library: a folder on disk whose subfolders and `.docx` files
//! the sidebar shows as a tree. It's plain files and folders, so Finder and
//! Explorer see the same organization. No GUI code here; see `sidebar`.

pub mod sidebar;

use std::fs;
use std::path::{Path, PathBuf};

use vibedoc_core::Document;

/// A folder and everything in it, sorted by name. Hidden files are skipped.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Folder {
    pub path: PathBuf,
    pub name: String,
    pub folders: Vec<Folder>,
    pub documents: Vec<PathBuf>,
}

impl Folder {
    /// Reads the folder tree under `path`. A missing folder is just empty.
    pub fn scan(path: &Path) -> Folder {
        let mut folder = Folder { path: path.to_path_buf(), name: display_name(path), ..Default::default() };
        let Ok(entries) = fs::read_dir(path) else { return folder };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            // Word leaves "~$name.docx" lock files next to open documents.
            if name.starts_with('.') || name.starts_with("~$") {
                continue;
            }
            if path.is_dir() {
                folder.folders.push(Folder::scan(&path));
            } else if is_document(&path) {
                folder.documents.push(path);
            }
        }
        folder.folders.sort_by_key(|f| f.name.to_lowercase());
        folder.documents.sort_by_key(|d| display_name(d).to_lowercase());
        folder
    }

    /// Documents in this folder and all its subfolders.
    pub fn document_count(&self) -> usize {
        self.documents.len() + self.folders.iter().map(Folder::document_count).sum::<usize>()
    }

    /// This folder and every folder inside it, depth first, each with its depth.
    pub fn all_folders(&self) -> Vec<(usize, &Folder)> {
        let mut all = vec![(0, self)];
        for child in &self.folders {
            all.extend(child.all_folders().into_iter().map(|(depth, f)| (depth + 1, f)));
        }
        all
    }

    /// Every document in the tree, depth first.
    pub fn all_documents(&self) -> Vec<&Path> {
        let mut all: Vec<&Path> = self.documents.iter().map(PathBuf::as_path).collect();
        for child in &self.folders {
            all.extend(child.all_documents());
        }
        all
    }
}

pub fn is_document(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("docx"))
}

/// A file's name without ".docx", or a folder's name.
pub fn display_name(path: &Path) -> String {
    let name = if is_document(path) { path.file_stem() } else { path.file_name() };
    name.map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `~/Documents/VibeDoc`, used until the user picks another folder.
pub fn default_root() -> PathBuf {
    dirs::document_dir().or_else(dirs::home_dir).unwrap_or_default().join("VibeDoc")
}

/// Creates a blank document named `name` (or "name 2", ... if that's taken).
pub fn create_document(folder: &Path, name: &str) -> Result<PathBuf, String> {
    save_new(&Document::default(), folder, name)
}

/// Saves `document` as "name copy.docx" next to `original`. Returns the copy's path.
pub fn save_copy(document: &Document, original: &Path) -> Result<PathBuf, String> {
    let folder = original.parent().ok_or("The original has no folder")?;
    let path = unused_path(folder, &format!("{} copy", display_name(original)), ".docx");
    vibedoc_docx::save(document, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Saves `document` into `folder` as "name.docx" (or "name 2.docx", ...).
pub fn save_new(document: &Document, folder: &Path, name: &str) -> Result<PathBuf, String> {
    let name = valid_name(name)?;
    fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let path = unused_path(folder, name, ".docx");
    vibedoc_docx::save(document, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn create_folder(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let name = valid_name(name)?;
    let path = unused_path(parent, name, "");
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Renames a document (keeping ".docx") or a folder. Returns the new path.
pub fn rename(path: &Path, name: &str) -> Result<PathBuf, String> {
    let name = valid_name(name)?;
    let file_name = if is_document(path) { format!("{name}.docx") } else { name.to_string() };
    let target = path.with_file_name(file_name);
    move_to(path, target)
}

/// Moves a document or folder into `folder`. Returns the new path.
pub fn move_into(path: &Path, folder: &Path) -> Result<PathBuf, String> {
    if folder.starts_with(path) {
        return Err("A folder can't go inside itself.".into());
    }
    let file_name = path.file_name().ok_or("Nothing to move")?;
    move_to(path, folder.join(file_name))
}

fn move_to(from: &Path, to: PathBuf) -> Result<PathBuf, String> {
    if to == from {
        return Ok(to);
    }
    // Case-only renames ("notes" -> "Notes") look taken on case-insensitive disks.
    let same_file = to.to_string_lossy().to_lowercase() == from.to_string_lossy().to_lowercase();
    if to.exists() && !same_file {
        return Err(format!("“{}” already exists there.", display_name(&to)));
    }
    fs::rename(from, &to).map_err(|e| e.to_string())?;
    Ok(to)
}

/// Deletes a document, or a folder and everything in it.
pub fn delete(path: &Path) -> Result<(), String> {
    let result = if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) };
    result.map_err(|e| e.to_string())
}

fn valid_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() {
        Err("Names can't be empty.".into())
    } else if name.starts_with('.') || name.contains(['/', '\\', ':']) {
        Err("Names can't start with a dot or contain / \\ or :".into())
    } else {
        Ok(name)
    }
}

fn unused_path(folder: &Path, name: &str, extension: &str) -> PathBuf {
    (1..)
        .map(|n| if n == 1 { format!("{name}{extension}") } else { format!("{name} {n}{extension}") })
        .map(|file_name| folder.join(file_name))
        .find(|path| !path.exists())
        .expect("some numbered name is free")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, empty folder for one test.
    fn scratch(test: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("vibedoc-library-{}-{test}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn scan_builds_a_sorted_tree_of_documents() {
        let root = scratch("scan");
        let work = create_folder(&root, "Work").unwrap();
        let plans = create_folder(&work, "Plans").unwrap();
        create_document(&plans, "Roadmap").unwrap();
        create_document(&work, "b notes").unwrap();
        create_document(&work, "A memo").unwrap();
        create_document(&root, "Inbox").unwrap();
        fs::write(root.join("picture.png"), b"not a document").unwrap();
        fs::write(work.join("~$A memo.docx"), b"lock file").unwrap();

        let tree = Folder::scan(&root);
        assert_eq!(tree.documents, [root.join("Inbox.docx")]);
        assert_eq!(tree.folders[0].name, "Work");
        assert_eq!(tree.folders[0].documents, [work.join("A memo.docx"), work.join("b notes.docx")]);
        assert_eq!(tree.folders[0].folders[0].documents, [plans.join("Roadmap.docx")]);
        assert_eq!(tree.document_count(), 4);
        assert_eq!(tree.all_folders().iter().map(|(depth, _)| *depth).collect::<Vec<_>>(), [0, 1, 2]);
    }

    #[test]
    fn new_items_get_numbered_names_instead_of_overwriting() {
        let root = scratch("numbering");
        assert_eq!(create_document(&root, "Untitled").unwrap(), root.join("Untitled.docx"));
        assert_eq!(create_document(&root, "Untitled").unwrap(), root.join("Untitled 2.docx"));
        assert_eq!(create_folder(&root, " Notes ").unwrap(), root.join("Notes"));
        assert!(create_folder(&root, "a/b").is_err());
        assert!(create_document(&root, "  ").is_err());
    }

    #[test]
    fn rename_and_move_keep_documents_openable() {
        let root = scratch("moving");
        let folder = create_folder(&root, "Archive").unwrap();
        let document = create_document(&root, "Draft").unwrap();
        let renamed = rename(&document, "Final").unwrap();
        assert_eq!(renamed, root.join("Final.docx"));
        let moved = move_into(&renamed, &folder).unwrap();
        assert_eq!(moved, folder.join("Final.docx"));
        assert!(vibedoc_docx::open(&moved).is_ok());
        assert!(move_into(&folder, &folder.join("inside")).is_err());
        create_document(&root, "Final").unwrap();
        assert!(move_into(&root.join("Final.docx"), &folder).is_err());
    }

    #[test]
    fn copies_land_next_to_the_original() {
        let root = scratch("copy");
        let original = create_document(&root, "Plan").unwrap();
        assert_eq!(save_copy(&Document::default(), &original).unwrap(), root.join("Plan copy.docx"));
        assert_eq!(save_copy(&Document::default(), &original).unwrap(), root.join("Plan copy 2.docx"));
    }

    #[test]
    fn delete_removes_folders_with_their_contents() {
        let root = scratch("delete");
        let folder = create_folder(&root, "Old").unwrap();
        create_document(&folder, "Stale").unwrap();
        delete(&folder).unwrap();
        assert_eq!(Folder::scan(&root).document_count(), 0);
        assert_eq!(Folder::scan(&root.join("missing")).document_count(), 0);
    }
}
