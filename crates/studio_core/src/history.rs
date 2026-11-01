//! Undo and redo as changed ranges and their inverses.

use qml_edit::Change;
use serde::Serialize;

use crate::address::{PathChange, Tracked};

/// One edit as history keeps it.
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u64,
    pub doc: String,
    pub label: String,
    /// The writer who made it: an MCP client's name, `agent` for one that
    /// gave none, `person` for the window's own gestures.
    pub author: String,
    /// A change found in the file, which nobody made through Studio: the
    /// diff that reads it is a guess, so nothing is rebased across it.
    pub outside: bool,
    /// The generation of the text the entry's side applies to: after the
    /// edit while it is done, after its undo while it is undone. A later
    /// generation means others wrote since, and replaying it is rebased.
    pub(crate) generation: u64,
    /// A hash of that text: when the document has it again, the entry
    /// applies as it is, whatever was written and taken back between.
    pub(crate) side: u64,
    /// On the text before the edit.
    pub forward: Vec<Change>,
    /// On the text after it: gives the bytes before back.
    pub inverse: Vec<Change>,
    /// The edit made the document; undoing it removes the file.
    pub created: bool,
    /// The entry this one is one step with, in another file: undo and redo
    /// take both, and the lists show the one that is not the new file.
    pub group: Option<u64>,
    /// The new document is a block, and this is its module's `qmldir` line.
    pub(crate) declares: Option<crate::Declared>,
    pub(crate) handles_before: Vec<Tracked>,
    /// Also every handle given out on the text after it while that was the
    /// document's text: redo puts each back where it was.
    pub(crate) handles_after: Vec<Tracked>,
    /// What a structural edit did to paths; `None` for a change of the text.
    pub(crate) change: Option<PathChange>,
    /// Its handle snapshots are of the texts on either side of it. Once it
    /// was replayed past other writers' edits they are not, and handles
    /// are found again by bytes and key.
    pub(crate) exact: bool,
}

/// What a list of the history shows of an entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EntrySummary {
    pub id: u64,
    pub doc: String,
    pub label: String,
    pub author: String,
    /// Bytes replaced and bytes put in.
    pub removed: usize,
    pub added: usize,
}

/// What a replay past other writers' edits leaves the entry as: the
/// changes that make the edit and those that take it back, on the texts on
/// either side of it now. They are not the ones it was made with.
pub(crate) struct Replayed {
    pub forward: Vec<Change>,
    pub inverse: Vec<Change>,
}

impl Entry {
    fn is_partner(&self) -> bool {
        self.group.is_some() && self.created
    }

    fn moved(&mut self, generation: u64, side: u64, rebased: Option<Replayed>) {
        self.generation = generation;
        self.side = side;
        if let Some(r) = rebased {
            self.forward = r.forward;
            self.inverse = r.inverse;
            self.exact = false;
        }
    }

    pub fn summary(&self) -> EntrySummary {
        EntrySummary {
            id: self.id,
            doc: self.doc.clone(),
            label: self.label.clone(),
            author: self.author.clone(),
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
    /// Records an edit. What its author undid can no longer be redone, and
    /// a change found in the file ends what anybody could redo of that
    /// document: no redo is rebased across a guess, and an undone change
    /// found in the file is itself one, with no author to lose it otherwise.
    /// Another writer's undone entries stay: their redo is rebased past this
    /// edit.
    pub fn push(&mut self, mut entry: Entry) -> u64 {
        self.next_id += 1;
        entry.id = self.next_id;
        if entry.outside {
            self.undone.retain(|e| e.doc != entry.doc);
        } else {
            self.undone
                .retain(|e| e.author != entry.author && !(e.outside && e.doc == entry.doc));
        }
        self.done.push(entry);
        self.next_id
    }

    /// The last done entry of `doc`: the document's text is the one it
    /// left.
    pub fn latest_mut(&mut self, doc: &str) -> Option<&mut Entry> {
        self.done.iter_mut().rev().find(|e| e.doc == doc)
    }

    /// Makes the two newest done entries one step.
    pub(crate) fn group_newest(&mut self) {
        if let [.., first, second] = self.done.as_mut_slice() {
            first.group = Some(second.id);
            second.group = Some(first.id);
        }
    }

    /// The done entry `id`.
    pub(crate) fn done_entry(&self, id: u64) -> Option<&Entry> {
        self.done.iter().find(|e| e.id == id)
    }

    /// The undone entry `id`.
    pub(crate) fn undone_entry(&self, id: u64) -> Option<&Entry> {
        self.undone.iter().find(|e| e.id == id)
    }

    /// The last done entry, taken off, and the one it leaves on top.
    pub fn pop_and_top(&mut self) -> (Option<Entry>, Option<&mut Entry>) {
        let last = self.done.pop();
        (last, self.done.last_mut())
    }

    /// The entry an undo by `author` takes: the newest that is theirs or a
    /// change found in the file, which belongs to nobody; with `None`, the
    /// newest whoever made it.
    pub fn peek_undo(&self, author: Option<&str>) -> Option<&Entry> {
        self.done.iter().rev().find(|e| takes(e, author))
    }

    pub fn peek_redo(&self, author: Option<&str>) -> Option<&Entry> {
        self.undone.iter().rev().find(|e| takes(e, author))
    }

    /// Drops every entry of a document whose file went.
    pub fn forget(&mut self, doc: &str) {
        self.done.retain(|e| e.doc != doc);
        self.undone.retain(|e| e.doc != doc);
        // What was one step with a file that is gone is a step of its own.
        let kept: Vec<u64> = self.done.iter().chain(&self.undone).map(|e| e.id).collect();
        for entry in self.done.iter_mut().chain(&mut self.undone) {
            entry.group = entry.group.filter(|id| kept.contains(id));
        }
    }

    /// The entry `id` [`Self::peek_undo`] showed has been replayed, leaving
    /// its document at `generation`, whose text hashes to `side`. One replayed past others' edits did not
    /// give back the text it was made on: `rebased` is what it did, forward
    /// and inverse, on the text it left.
    pub fn commit_undo(&mut self, id: u64, generation: u64, side: u64, rebased: Option<Replayed>) {
        if let Some(at) = self.done.iter().position(|e| e.id == id) {
            let mut entry = self.done.remove(at);
            entry.moved(generation, side, rebased);
            self.undone.push(entry);
        }
    }

    /// The entry `id` [`Self::peek_redo`] showed has been replayed.
    pub fn commit_redo(&mut self, id: u64, generation: u64, side: u64, rebased: Option<Replayed>) {
        if let Some(at) = self.undone.iter().position(|e| e.id == id) {
            let mut entry = self.undone.remove(at);
            entry.moved(generation, side, rebased);
            self.done.push(entry);
        }
    }

    /// The done entries whose author is `author`, oldest first, and the
    /// undone ones, next to redo first: what that writer's undo and redo
    /// would take.
    pub fn list_by(&self, author: &str) -> (Vec<EntrySummary>, Vec<EntrySummary>) {
        let by = |e: &&Entry| takes(e, Some(author)) && !e.is_partner();
        (
            self.done.iter().filter(by).map(Entry::summary).collect(),
            self.undone
                .iter()
                .rev()
                .filter(by)
                .map(Entry::summary)
                .collect(),
        )
    }

    /// Done (oldest first) and undone (next to redo first).
    pub fn list(&self) -> (Vec<EntrySummary>, Vec<EntrySummary>) {
        (
            self.done
                .iter()
                .filter(|e| !e.is_partner())
                .map(Entry::summary)
                .collect(),
            self.undone
                .iter()
                .rev()
                .filter(|e| !e.is_partner())
                .map(Entry::summary)
                .collect(),
        )
    }
}

fn takes(entry: &Entry, author: Option<&str>) -> bool {
    author.is_none_or(|a| entry.outside || entry.author == a)
}
