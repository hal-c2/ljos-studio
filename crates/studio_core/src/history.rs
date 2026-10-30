//! Undo and redo as changed ranges and their inverses.

use qml_edit::Change;
use serde::Serialize;

use crate::address::Tracked;

/// One edit as history keeps it.
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u64,
    pub doc: String,
    pub label: String,
    /// On the text before the edit.
    pub forward: Vec<Change>,
    /// On the text after it: gives the bytes before back.
    pub inverse: Vec<Change>,
    /// The edit made the document; undoing it removes the file.
    pub created: bool,
    pub(crate) handles_before: Vec<Tracked>,
    pub(crate) handles_after: Vec<Tracked>,
}

/// What a list of the history shows of an entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EntrySummary {
    pub id: u64,
    pub doc: String,
    pub label: String,
    /// Bytes replaced and bytes put in.
    pub removed: usize,
    pub added: usize,
}

impl Entry {
    pub fn summary(&self) -> EntrySummary {
        EntrySummary {
            id: self.id,
            doc: self.doc.clone(),
            label: self.label.clone(),
            removed: self.forward.iter().map(|c| c.range.len()).sum(),
            added: self.forward.iter().map(|c| c.replacement.len()).sum(),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct History {
    done: Vec<Entry>,
    undone: Vec<Entry>,
    next_id: u64,
}

impl History {
    /// Records an edit; what was undone can no longer be redone.
    pub fn push(&mut self, mut entry: Entry) -> u64 {
        self.next_id += 1;
        entry.id = self.next_id;
        self.done.push(entry);
        self.undone.clear();
        self.next_id
    }

    pub fn peek_undo(&self) -> Option<&Entry> {
        self.done.last()
    }

    pub fn peek_redo(&self) -> Option<&Entry> {
        self.undone.last()
    }

    /// Drops every entry of a document whose file went.
    pub fn forget(&mut self, doc: &str) {
        self.done.retain(|e| e.doc != doc);
        self.undone.retain(|e| e.doc != doc);
    }

    /// The entry [`Self::peek_undo`] showed has been replayed.
    pub fn commit_undo(&mut self) {
        if let Some(entry) = self.done.pop() {
            self.undone.push(entry);
        }
    }

    /// The entry [`Self::peek_redo`] showed has been replayed.
    pub fn commit_redo(&mut self) {
        if let Some(entry) = self.undone.pop() {
            self.done.push(entry);
        }
    }

    /// Done (oldest first) and undone (next to redo first).
    pub fn list(&self) -> (Vec<EntrySummary>, Vec<EntrySummary>) {
        (
            self.done.iter().map(Entry::summary).collect(),
            self.undone.iter().rev().map(Entry::summary).collect(),
        )
    }
}
