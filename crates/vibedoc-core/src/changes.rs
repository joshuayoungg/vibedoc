//! What changed between two versions of a document, paragraph by paragraph,
//! and taking a change back out later, even after other edits.
//!
//! The assistant panel uses this to show each request's edits and to undo
//! them one request at a time, the way code review tools revert a patch.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::ops::Range;

use crate::document::{is_image_char, Document, Paragraph};

/// A run of changed paragraphs: `before` in the old document became `after` in the new one.
/// An empty `before` is an insertion; an empty `after` is a deletion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub before: Range<usize>,
    pub after: Range<usize>,
}

/// Past this many paragraph pairs, the changed middle is reported as one hunk
/// instead of being matched up paragraph by paragraph.
const MAX_COMPARISONS: usize = 4_000_000;

/// The paragraphs that differ between `before` and `after`, in document order.
pub fn diff(before: &Document, after: &Document) -> Vec<Hunk> {
    let old: Vec<u64> = (0..before.paragraphs.len()).map(|i| paragraph_hash(before, i)).collect();
    let new: Vec<u64> = (0..after.paragraphs.len()).map(|i| paragraph_hash(after, i)).collect();
    // Hashes rule out almost every pair cheaply; equal hashes are double-checked.
    let same = |i: usize, j: usize| old[i] == new[j] && same_paragraph(before, i, after, j);

    let (n, m) = (old.len(), new.len());
    let mut prefix = 0;
    while prefix < n.min(m) && same(prefix, prefix) {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < (n - prefix).min(m - prefix) && same(n - 1 - suffix, m - 1 - suffix) {
        suffix += 1;
    }
    let (old_middle, new_middle) = (prefix..n - suffix, prefix..m - suffix);
    if old_middle.is_empty() && new_middle.is_empty() {
        return Vec::new();
    }
    if old_middle.is_empty() || new_middle.is_empty() || old_middle.len() * new_middle.len() > MAX_COMPARISONS
    {
        return vec![Hunk { before: old_middle, after: new_middle }];
    }

    // Longest common subsequence of the middles: `common[i][j]` counts the
    // paragraphs that can stay put in old[i..] and new[j..].
    let (rows, cols) = (old_middle.len(), new_middle.len());
    let at = |i: usize, j: usize| i * (cols + 1) + j;
    let mut common = vec![0u32; (rows + 1) * (cols + 1)];
    for i in (0..rows).rev() {
        for j in (0..cols).rev() {
            common[at(i, j)] = if same(prefix + i, prefix + j) {
                common[at(i + 1, j + 1)] + 1
            } else {
                common[at(i + 1, j)].max(common[at(i, j + 1)])
            };
        }
    }

    let mut hunks = Vec::new();
    let mut open: Option<(usize, usize)> = None;
    let (mut i, mut j) = (0, 0);
    let mut close = |open: &mut Option<(usize, usize)>, i: usize, j: usize| {
        if let Some((from_i, from_j)) = open.take() {
            hunks.push(Hunk { before: prefix + from_i..prefix + i, after: prefix + from_j..prefix + j });
        }
    };
    while i < rows || j < cols {
        if i < rows && j < cols && same(prefix + i, prefix + j) {
            close(&mut open, i, j);
            i += 1;
            j += 1;
        } else {
            open.get_or_insert((i, j));
            if j < cols && (i == rows || common[at(i, j + 1)] >= common[at(i + 1, j)]) {
                j += 1;
            } else {
                i += 1;
            }
        }
    }
    close(&mut open, i, j);
    hunks
}

/// Where each hunk's `after` paragraphs are now in `current`, which may have
/// been edited since. `None` if any of those paragraphs was edited again.
pub fn locate(hunks: &[Hunk], after: &Document, current: &Document) -> Option<Vec<Range<usize>>> {
    let later = diff(after, current);
    hunks
        .iter()
        .map(|hunk| {
            let mut shift = 0isize;
            for edit in &later {
                if edit.before.end <= hunk.after.start {
                    shift += edit.after.len() as isize - edit.before.len() as isize;
                } else if edit.before.start < hunk.after.end {
                    return None;
                }
            }
            let start = hunk.after.start.checked_add_signed(shift)?;
            Some(start..start + hunk.after.len())
        })
        .collect()
}

/// Takes the change from `before` to `after` back out of `current`, keeping
/// edits made since to other paragraphs. Swap `before` and `after` to put it
/// back again. `None` if the changed paragraphs were edited again.
pub fn revert(before: &Document, after: &Document, current: &Document) -> Option<Document> {
    let hunks = diff(before, after);
    let places = locate(&hunks, after, current)?;
    let mut result = current.clone();
    for (hunk, place) in hunks.iter().zip(places).rev() {
        let restored: Vec<Paragraph> = before.paragraphs[hunk.before.clone()]
            .iter()
            .map(|p| carry_images(p, before, &mut result))
            .collect();
        result.paragraphs.splice(place, restored);
    }
    if result.paragraphs.is_empty() {
        result.paragraphs.push(Paragraph::default());
    }
    Some(result)
}

/// A copy of `paragraph` (from `from`) whose pictures point at the same images in `into`.
fn carry_images(paragraph: &Paragraph, from: &Document, into: &mut Document) -> Paragraph {
    let mut paragraph = paragraph.clone();
    for c in &mut paragraph.chars {
        let Some(image) = from.image(c.ch) else { continue };
        if into.image(c.ch) != Some(image) {
            c.ch = into.add_image(image.clone());
        }
    }
    paragraph
}

/// Pictures compare by their image, not their placeholder character, which
/// can differ between copies of a document.
fn same_paragraph(a: &Document, i: usize, b: &Document, j: usize) -> bool {
    let (p, q) = (&a.paragraphs[i], &b.paragraphs[j]);
    p.kind == q.kind
        && p.chars.len() == q.chars.len()
        && p.chars.iter().zip(&q.chars).all(|(x, y)| {
            x.style == y.style
                && match (is_image_char(x.ch), is_image_char(y.ch)) {
                    (true, true) => a.image(x.ch) == b.image(y.ch),
                    (false, false) => x.ch == y.ch,
                    _ => false,
                }
        })
}

fn paragraph_hash(document: &Document, index: usize) -> u64 {
    let paragraph = &document.paragraphs[index];
    let mut hasher = DefaultHasher::new();
    paragraph.kind.hash(&mut hasher);
    for c in &paragraph.chars {
        c.style.hash(&mut hasher);
        match document.image(c.ch) {
            Some(image) => image.bytes.len().hash(&mut hasher),
            None => c.ch.hash(&mut hasher),
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Image, ParagraphKind, Position, TextStyle};

    fn doc(lines: &[&str]) -> Document {
        let paragraphs = lines
            .iter()
            .map(|l| Paragraph::from_text(ParagraphKind::Normal, l, TextStyle::default()))
            .collect();
        Document::from_paragraphs(paragraphs)
    }

    fn texts(document: &Document) -> Vec<String> {
        document.paragraphs.iter().map(|p| p.text()).collect()
    }

    fn hunk(before: Range<usize>, after: Range<usize>) -> Hunk {
        Hunk { before, after }
    }

    #[test]
    fn identical_documents_have_no_changes() {
        assert_eq!(diff(&doc(&["a", "b"]), &doc(&["a", "b"])), []);
    }

    #[test]
    fn insertions_rewrites_and_deletions_become_separate_hunks() {
        let before = doc(&["a", "b", "c", "d", "e"]);
        let after = doc(&["a", "new", "b", "C", "d"]);
        assert_eq!(diff(&before, &after), [hunk(1..1, 1..2), hunk(2..3, 3..4), hunk(4..5, 5..5)]);
    }

    #[test]
    fn formatting_and_kind_changes_count_as_changes() {
        let before = doc(&["a", "b"]);
        let mut after = before.clone();
        after.paragraphs[0].kind = ParagraphKind::Heading1;
        after.restyle(Position::new(1, 0), Position::new(1, 1), |s| s.bold = true);
        assert_eq!(diff(&before, &after), [hunk(0..2, 0..2)]);
    }

    #[test]
    fn revert_keeps_later_edits_to_other_paragraphs() {
        let before = doc(&["title", "body", "end"]);
        let after = doc(&["title", "body", "added by the assistant", "end"]);
        let mut current = after.clone();
        current
            .paragraphs
            .insert(0, Paragraph::from_text(ParagraphKind::Normal, "typed later", Default::default()));
        let reverted = revert(&before, &after, &current).unwrap();
        assert_eq!(texts(&reverted), ["typed later", "title", "body", "end"]);
    }

    #[test]
    fn revert_refuses_when_the_changed_paragraph_was_edited_again() {
        let before = doc(&["a", "b"]);
        let after = doc(&["a", "B"]);
        let current = doc(&["a", "B, then edited by hand"]);
        assert_eq!(revert(&before, &after, &current), None);
        assert_eq!(locate(&diff(&before, &after), &after, &current), None);
    }

    #[test]
    fn locate_follows_paragraphs_that_moved() {
        let before = doc(&["a", "b"]);
        let after = doc(&["a", "b", "c"]);
        let current = doc(&["x", "y", "a", "b", "c"]);
        let places = locate(&diff(&before, &after), &after, &current).unwrap();
        assert_eq!(places.len(), 1);
        assert_eq!(places[0], 4..5);
    }

    #[test]
    fn reverting_twice_puts_the_change_back() {
        let before = doc(&["a", "b", "c"]);
        let after = doc(&["a", "c", "d"]);
        let undone = revert(&before, &after, &after).unwrap();
        assert_eq!(undone, before);
        assert_eq!(revert(&after, &before, &undone).unwrap(), after);
    }

    #[test]
    fn reverting_a_deleted_picture_brings_its_image_back() {
        let mut before = doc(&["a", ""]);
        before.insert_image(Position::new(1, 0), Image::fitted(vec![1, 2, 3], 10.0, 10.0, 100.0, 100.0));
        let after = doc(&["a"]);
        let reverted = revert(&before, &after, &after).unwrap();
        assert_eq!(diff(&before, &reverted), []);
        let ch = reverted.paragraphs[1].chars[0].ch;
        assert_eq!(reverted.image(ch).map(|i| i.bytes.len()), Some(3));
    }
}
