//! What was written to each document since the oldest base a writer can
//! still rebase from (MULTIPLAYER.md seam 1), and the leases writers hold
//! on objects (seam 4).
//!
//! An edit made against an older generation is moved forward over these
//! entries: by handle, when no other writer touched what it touches; by byte
//! ranges, for a change of the text.

use std::collections::VecDeque;

use qml_edit::Change;

/// How many entries a document keeps. A base older than the oldest is
/// refused as stale: the writer reads again. A writer that read, thought
/// and wrote while 256 others landed on the same document has read
/// something no longer worth rebasing from, and the bound keeps the log's
/// memory a small multiple of the document's (DESIGN.md "Phase 4: rebase by
/// handle, as built" measures an entry).
pub(crate) const LOG_LEN: usize = 256;

/// What an entry touched: an object's binding (`Some(name)`), or the object
/// itself and all it holds (`None`).
pub(crate) type Touch = (u64, Option<String>);

/// The name an insert touches on its parent: two inserts into one parent
/// both land, but a remove of the parent conflicts with either.
pub(crate) const CHILDREN: &str = "<children>";
/// The `TapHandler`s directly among a parent's children, which hold its one
/// tap flow: a change that leaves them different, however it was made, also
/// touches this.
pub(crate) const TAPS: &str = "<taps>";

/// What a state edit touches on the state's owner.
pub(crate) const STATES: &str = "states";

/// One applied edit.
#[derive(Clone, Debug)]
pub(crate) struct Logged {
    /// The generation it made.
    pub generation: u64,
    pub author: String,
    pub label: String,
    /// Ranges of the text before it, sorted and not overlapping.
    pub changes: Vec<Change>,
    /// `None` when it is not known: a change of a text that did not parse
    /// names no objects, so a rebase by handle cannot pass it.
    pub touched: Option<Vec<Touch>>,
    /// A change found in the file: its ranges are a diff's reading of a
    /// whole new text, so nothing is rebased across it.
    pub outside: bool,
    /// The generation whose edit this gave back exactly: an undo or redo
    /// made on the very text that edit left.
    pub reverts: Option<u64>,
}

#[derive(Debug, Default)]
pub(crate) struct Log {
    entries: VecDeque<Logged>,
}

impl Log {
    pub fn push(&mut self, entry: Logged) {
        if self.entries.len() == LOG_LEN {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// The entries made since `base`, oldest first, when the log reaches
    /// back that far and the document is at `current`.
    pub fn since(&self, base: u64, current: u64) -> Option<Vec<&Logged>> {
        if base > current {
            return None;
        }
        if base == current {
            return Some(Vec::new());
        }
        let first = self.entries.iter().position(|e| e.generation == base + 1)?;
        let since: Vec<&Logged> = self.entries.iter().skip(first).collect();
        // Every generation between has its entry: one made without (a
        // document read anew) would leave a gap no transform can cross.
        let whole = since.iter().zip(base + 1..).all(|(e, g)| e.generation == g)
            && since.last().map(|e| e.generation) == Some(current);
        whole.then(|| net(since))
    }

    /// Bytes the log holds, for DESIGN.md's measure of an entry.
    pub fn bytes(&self) -> usize {
        self.entries
            .iter()
            .map(|e| {
                std::mem::size_of::<Logged>()
                    + e.author.capacity()
                    + e.label.capacity()
                    + e.changes
                        .iter()
                        .map(|c| std::mem::size_of::<Change>() + c.replacement.capacity())
                        .sum::<usize>()
                    + e.touched.as_ref().map_or(0, |t| {
                        t.iter()
                            .map(|(_, n)| {
                                std::mem::size_of::<Touch>()
                                    + n.as_ref().map_or(0, String::capacity)
                            })
                            .sum()
                    })
            })
            .sum()
    }

    pub fn latest(&self) -> Option<&Logged> {
        self.entries.back()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// The entries without an edit and the undo that came straight after it:
/// together they are no change, and moving past both would refuse what
/// overlaps the edit that is no longer there. An edit with others between it
/// and its undo stays, as their ranges are of the text that has it.
fn net(since: Vec<&Logged>) -> Vec<&Logged> {
    let mut out: Vec<&Logged> = Vec::with_capacity(since.len());
    for entry in since {
        if entry.reverts.is_some() && out.last().map(|e| e.generation) == entry.reverts {
            out.pop();
        } else {
            out.push(entry);
        }
    }
    out
}

/// How long a lease lasts after the holder's last call. Long enough for an
/// agent's next call in a burst of edits (a model's turn between tool calls
/// takes a few seconds), short enough that a holder that stopped, crashed
/// or forgot frees the objects before a person waiting on them gives up.
/// A first value: DESIGN.md "Phase 4" says to measure the gap between an
/// agent's calls and set it from that.
pub const LEASE_TTL_MS: u64 = 5_000;

/// The longest a lease lives, renewed or not: a holder that keeps calling
/// never lets go otherwise, and a person would wait on an agent's loop.
/// Five minutes is a long burst of edits to one part of a design; past it
/// the holder is told to release and plan again.
pub const LEASE_LIFE_MS: u64 = 300_000;

/// One object a writer holds, with all it holds.
#[derive(Clone, Debug)]
pub(crate) struct Lease {
    pub doc: String,
    pub handle: u64,
    pub holder: String,
    pub since_ms: u64,
    pub until_ms: u64,
}

impl Lease {
    pub fn renew(&mut self, now_ms: u64) {
        self.until_ms = (now_ms + LEASE_TTL_MS).min(self.since_ms + LEASE_LIFE_MS);
    }

    /// It ran its whole life rather than lapsing.
    pub fn spent(&self) -> bool {
        self.until_ms == self.since_ms + LEASE_LIFE_MS
    }
}

/// A lease as a window shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseView {
    pub doc: String,
    pub handle: String,
    pub holder: String,
    /// On the session's clock ([`crate::Session::act_as`]), in milliseconds.
    pub until_ms: u64,
}

/// Whether `range` reaches into the bytes `span`: overlapping them, or an
/// insertion strictly inside.
pub(crate) fn reaches(range: &std::ops::Range<usize>, span: (usize, usize)) -> bool {
    let (start, end) = span;
    if range.is_empty() {
        start < range.start && range.start < end
    } else {
        range.start < end && start < range.end
    }
}
