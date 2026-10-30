# Multiplayer: two writers on one QML document

Research for Studio's phase 3, milestone 4. No code: this says how two
people, or a person and an agent, could edit one QML document at once
without giving up what Studio already promises, compares four ways to do
it, recommends one, and lists the seams it needs in Studio and in Ljós.

## 1. What has to hold

Studio's design (DESIGN.md section 1) fixes the ground rules. Multiplayer
may not trade any of them away:

1. **The file is the document.** There is no scene format beside the
   `.qml` file, and an edit changes only the bytes it means to change
   (`qml_edit`'s promise). Comments, blank lines and the author's layout
   survive every merge.
2. **A file never breaks.** `qml_edit` refuses an edit whose result does
   not parse (`WouldBreak`). A merge of two writers' work must keep this,
   or say plainly when it can't.
3. **Addresses are stable.** A writer names objects by handle (`n7`), and
   the session keeps a handle on its object across edits (by the edit's
   effect on paths, else by `ObjectKey`).
4. **Undo gives back bytes**, and a person undoes their own work, not
   someone else's.
5. **Agents are first-class writers.** An agent works over MCP, reads,
   plans several steps and writes. It copes well with a refusal that says
   why. It copes badly with silent merges that change what it read.
6. **Sharing is git and files** (non-goal: a hosted service). Live
   multiplayer is for people working on a design together *now*; git
   stays the way work is shared over time.
7. **The code view is a writer too.** A person can type any text into the
   file, including text that does not parse while they type.

Two shapes of collaboration follow:

- **Live.** Two or more writers are attached to one session at the same
  time: a person and an agent on one machine, or two people on two
  machines over the loopback endpoint or a relay. Latency is milliseconds
  to a second.
- **Apart.** Writers edit copies and meet later (offline, branches).
  Today that is git's text merge.

## 2. What Studio has already

What exists makes some options much cheaper than others:

- **Edits are small splices.** `qml_edit::apply` gives `Applied { text,
  changes }`, with the changes sorted and not overlapping, and
  `Applied::inverse` for undo. A history entry is a list of changed
  ranges and their inverses.
- **One session orders every write.** The canvas, the code view and MCP
  all go through `Session`. Each document has a **generation**, and an
  edit names the generation it was made against. A stale one is refused
  with "read it again and redo the edit". That is already optimistic
  concurrency control with one writer winning.
- **Handles survive any change of the text.** After Studio's own edits, a
  handle follows the edit's path effect. After any other text change, it
  follows the object's key: its `id`, else its literal `objectName`, else
  its type and its place among unnamed siblings (`ljos_ir::ObjectKey`,
  the hot-reload rule). Each handle also carries its byte span through
  text changes. A handle whose object went away says it is gone.
- **Review comments already follow handles** and are reattached by key
  (S6), so per-object metadata that survives other writers' edits exists.
- **The guarded endpoint.** The running Studio serves the same MCP tools
  on loopback, with a per-start token, Origin/Host checks and
  capabilities. A second writer on the same machine is already possible;
  today the second writer just sees more "stale generation" refusals.

So Studio already *is* single-authority multiplayer with refusal on any
conflict. The question is how much more merging to do, and where.

## 3. The four candidates

### 3.1 Operational transformation over the text

Every writer sends text changes (`range`, `replacement`) against the
generation it saw. A server holds the order. Each incoming change is
transformed over the changes applied since its base generation, by
shifting its range past earlier insertions and deletions, and then
applied. This is the Jupiter / Google Docs shape (Nichols et al., 1995):
one central order, so only the "transform against what came since"
direction is needed, and none of the TP2 puzzles of peer-to-peer OT
(Ellis and Gibbs, 1989, and the later corrections).

- **For.** Studio's `Change` is exactly a text operation, and `Session` is
  already the central order. Transforming ranges over ranges is a few
  hundred lines and easy to test exhaustively. Bytes stay exact: the
  result is a splice of real text.
- **Against.** Text OT merges *characters*, not *intent*. Say writer A
  removes a `Rectangle` and writer B, concurrently, sets `color` inside
  it. OT shifts B's insert to the deletion point. The result is a stray
  `color: "red"` among the parent's members. It may parse (a binding the
  parent gets by accident) or not. Two inserts at one point interleave in
  arrival order, which is fine for text but can split an object's lines
  from its comment. Each result must be re-parsed, and a broken one
  refused. Then a "merge" is only a smarter version of today's refusal,
  for edits that touch the same object.
- **Offline.** None. It needs the server.

### 3.2 A CRDT over the text

Each replica holds the file as a sequence CRDT, such as RGA, YATA (Yjs)
or Fugue (Weidner and Kleppmann, 2023, which avoids interleaving
anomalies). Replicas exchange operations in any order and converge. Rust
implementations exist that build for wasm (`yrs`, `automerge`, `diamond-types`).

- **For.** Peer-to-peer, offline-first, no server order needed; mature
  libraries; convergence is a theorem, not a test.
- **Against.**
  - A CRDT *must* accept every operation, so promise 2 is gone. The
    merged text may not parse, and no replica may refuse it, because
    refusing breaks convergence. The A-removes / B-sets-color case lands
    the same way as with OT, but now nobody can say no.
  - The document carries the CRDT's identity metadata (a tombstone or
    id per character, compressed but present). Either the `.qml` file
    stops being the document and a sidecar becomes the truth, or every
    load re-derives identities from scratch, which loses the history
    that makes merging work.
  - Studio's edits are structural (insert an object, move a subtree).
    In a text CRDT a move is delete plus insert, so a concurrent edit
    inside a moved object is lost or duplicated.
  - Git still merges the file as text anyway.
- **Offline.** Yes, which is its one real advantage, and Studio's
  non-goals mostly decline it.

### 3.3 A CRDT over the object tree, with `qml_edit` as the projection

Each replica holds the object tree:
- objects with stable ids (Studio's handles made global);
- per object, a map of bindings, each a last-writer-wins register (or a
  text CRDT for long values such as JS handlers);
- children as a list CRDT;
- moves through a replicated-tree move operation (Kleppmann et al.,
  "A highly-available move operation for replicated trees", 2021), which
  keeps the tree acyclic under concurrent moves.

The file is a *projection*: each tree operation that wins is applied to
the text through `qml_edit` (`SetBinding`, `InsertObject`, `MoveObject`,
`RemoveObject`), so the bytes change only where the tree did. This is
close to how Figma has publicly described its multiplayer: per-property
last-writer-wins on an object tree, with a server.

- **For.** Merges carry intent:
  - concurrent sets of different properties on one object both land;
  - concurrent moves resolve to a tree;
  - A-removes / B-sets-color resolves by a rule (remove wins, or the
    edit revives the object) instead of making stray text.
  - The operations are Studio's MCP verbs, and handles become the
    object ids.
- **Against.**
  - **The bytes don't converge.** `qml_edit` makes layout decisions from
    the text it is given (where a new binding goes, group or dotted, one
    line or many). Replicas that apply the same set of tree operations
    in different orders end up with equal trees but different bytes.
    Byte-equal files need one order of application. Then the tree CRDT
    is doing what a sequencer does, at a CRDT's cost.
  - **Text that isn't tree.** Comments, blank lines, JS bodies and
    member order are not in the tree. They have to be opaque leaves (a
    handler body as a text register) or be lost. Bindings written as
    expressions merge only last-writer-wins.
  - **The code view.** A person typing into the file produces text, not
    tree operations. Each pause has to be lifted into operations by
    diffing the parse trees by `ObjectKey`, which guesses at intent
    (rename versus remove plus insert) and fails outright while the text
    does not parse. That is most of the time a person is typing.
  - It is the most code by far: a tree CRDT, a lifter, a projector, and
    per-property merge rules for QML's grouped, attached and dotted
    names.
- **Offline.** In principle; in practice the byte problem pulls it back
  to a server.

### 3.4 Locks or leases per object

A writer leases an object (its subtree) before editing it, or the
session grants one implicitly on the first edit. While the lease lasts,
other writers' edits *inside that subtree* are refused with `leased_by`
(who holds it, and until when). Edits elsewhere go through. Leases expire
(a time to live, renewed while the writer is active) and are released on
disconnect. Presence, meaning who is where, comes with it.

- **For.** The simplest. No merge, so nothing to get wrong. Bytes stay
  exact because one writer at a time edits each subtree. It fits agents
  well: an agent leases the subtree its plan touches, does its steps, and
  releases. A person sees "the agent is editing Header" instead of their
  edits being merged into something neither meant. It extends today's
  generation refusal, and the error is already one agents handle.
- **Against.**
  - The granularity is coarse: two people can't change two properties of
    one object at once.
  - It needs one authority, which Studio has.
  - The code view needs a rule: typing takes a lease on the object whose
    span the caret is in, widened to the whole document when the edit
    spans objects or the text does not parse.
  - Lease expiry on a crashed writer needs a sensible time to live.
  - Starvation is possible, but presence makes it visible and social.
- **Offline.** No.

## 4. Side by side

| | Text OT | Text CRDT | Tree CRDT + `qml_edit` | Leases |
|---|---|---|---|---|
| Bytes exact, comments kept | yes | yes, but metadata beside the file | tree yes, bytes diverge without one order | yes |
| File always parses | only by refusing after the fact | no, can't refuse | mostly: projection is checked; lifted text may fail | yes |
| Same-object concurrent edits | merge by characters, often wrong | merge by characters, often wrong | merge by property, right | refused, one waits |
| Different-object concurrent edits | yes | yes | yes | yes |
| Code view as a writer | natural | natural | hard (lift text to ops) | natural, leased span |
| Agents (plan, then act) | silent shifts under the plan | silent shifts | good | very good |
| Undo own work only | transform inverses | library support (some) | per-op inverse | own entries, exact |
| Offline / apart | no | yes | in principle | no (git) |
| New code in Studio | small | library + glue + sidecar | large | small |
| Fits what exists | very well | poorly | partly (verbs, handles) | very well |

## 5. Recommendation

DECISION: **one authority, ordered operations addressed by handle, with
leases for the edits that need them.** In short: Studio's session stays
the single order, as today, and gets four additions:

1. **Rebase by handle instead of refusing on generation.** A structural
   edit (an MCP verb, a canvas action) names objects by handle and its
   base generation. When the base is stale, the session doesn't refuse
   at once. It re-resolves the handles in the current text (they already
   survive edits) and applies the same `qml_edit::Edit` there. It
   refuses only when a handle is gone, `qml_edit` refuses, or a lease
   covers the target. This is operational transformation where the
   transform is address resolution. Studio already has that resolution,
   and it merges intent ("set `color` on n7") rather than characters.
2. **Text changes from the code view are transformed as text OT**
   (section 3.1), against the changes since their base, but **only where
   they don't overlap** an intervening change. An overlapping hunk is
   refused back to that writer's view, which shows both. After the
   transform the result must parse, or must not newly fail to parse. A
   person mid-typing holds a lease on the object they are typing in, so
   structured edits there wait rather than collide.
3. **Leases** (section 3.4):
   - a writer can lease a handle's subtree explicitly (an agent's plan,
     a drag on the canvas);
   - typing in the code view takes one implicitly;
   - each lease has a time to live renewed by activity;
   - a refusal says who holds it and until when;
   - presence (who has what selected, who holds what) is broadcast on
     the change feed.
4. **Per-writer history.** History entries carry their author. Undo
   takes the writer's own last entry, rebases its inverse over what came
   after (by handle for structural entries, by range for text), and is
   refused, not guessed, when that fails.

Why this one and not the others:
- **It keeps every promise in section 1.** Bytes come only from
  `qml_edit` splices or real typed text, a file never newly breaks,
  handles are the merge key, and undo is still bytes.
- **The merge quality where it matters is the tree CRDT's.** Edits by
  handle merge by intent, and two writers on different properties of one
  object both land through rebase. It costs no new data structure,
  because the session's one order makes the bytes converge trivially.
- **It is incremental.** Each addition is useful alone and testable as a
  scenario. Rebase-by-handle helps today's one-person-one-agent case
  immediately: the agent's call no longer bounces because the person
  nudged something else.
- **Apart stays git.** The structural merge that a tree CRDT would give
  for offline work is better spent on a git merge driver. It would diff
  base, ours and theirs by `ObjectKey` and replay one side's edits onto
  the other through rebase-by-handle (seam 8). That reuses the same
  machinery without any replica protocol.

Not recommended now: the text CRDT, because it can't refuse and so
breaks promise 2, and the tree CRDT, the largest cost for a convergence
property that one authority gives for free. Neither choice is closed
off. The operation log below is a sequence of addressed operations, the
input a tree CRDT would take if Studio ever needs peer-to-peer.

## 6. Seams it needs

In Studio (`studio_core`, `studio_mcp`):

1. **`Op` and the log.** One type for what a writer sends: `{ author,
   base: generation, body: Edit-by-handle | TextChanges | Lease |
   Release }`. Its answer is applied (new generation, changes, the
   handles touched), rebased (the same, with a note), or refused (stale
   with a reason, `leased_by`, `gone`, `would_break`). The session keeps
   the applied ops per document since the oldest base still in use.
2. **`Session::rebase`.** Resolve handles at the current generation and
   apply. The handle resolution and `ObjectKey` fallback already exist;
   the change is to stop refusing on generation alone.
3. **Text transform.** `transform(change, since: &[Change]) ->
   Result<Change, Overlap>`, pure and exhaustively tested. It could live
   in `qml_edit` beside `splice`, since it is general, so it would be an
   ljos-ui request.
4. **Leases and presence.** A lease table in the session (handle →
   holder, expiry), checked on every write that resolves into a leased
   subtree. To stay within the 25-tool budget, presence and leases go as
   arguments of existing tools: `node_select` gains `hold` (lease what
   is selected) and the change feed carries presence. A lease's time
   comes from the `Timer` seam, so the web build and tests use the same
   clock.
5. **Author on history.** `Entry` gains an author. Undo and redo are per
   author by default. `history` lists everyone's entries, each with its
   author.
6. **A change feed for several writers.** The session's watcher already
   tells a page about writes. It needs to carry ops (author, changes,
   new generation) to every attached writer, over MCP notifications on
   stdio and the loopback endpoint, and over the page bridge on the web.
7. **Transport and trust for other machines.** Loopback covers one
   machine. Two machines need a relay that keeps the endpoint's model (a
   token per session, capabilities per writer; `inspect` for watchers,
   `mutate` for editors). This is the only part that touches the non-goal
   "a hosted service". A self-hosted relay or a direct connection keeps
   it out of scope.
8. **A git merge driver (apart).** `studio merge base ours theirs`
   lifts theirs into handle-addressed edits (a parse-tree diff by
   `ObjectKey`), replays them onto ours through rebase-by-handle, and
   leaves conflict markers only for what fails. It can be built last and
   alone.

Requests to ljos-ui:

- **`qml_edit::transform`** (seam 3), a range-over-range transform of
  `Change`s, with the same promises as `splice`.
- **A tree diff by `ObjectKey`** (seam 8): given two parsed files, the
  objects added, removed, moved, and the bindings changed, as `Edit`s.
  It is general (hot reload compares by key already) and is what lifts
  text into operations for the merge driver.

## 7. Risks and open questions

- **Key collisions.** Rebase relies on `ObjectKey` when the edit's own
  path effect is unknown: after a code-view edit, or another writer's.
  Unnamed siblings of one type are keyed by place, so a concurrent
  insert of a same-typed sibling can shift the key. The session's
  carried byte spans resolve most of these; the rest are refused as
  `gone`, never guessed. Measure how often this happens on the corpus
  before relying on it.
- **Lease time to live.** It is a trade-off between a crashed writer
  blocking others and an active writer losing their lease mid-edit.
  Start at a few seconds, renewed by any call. Measure with people.
- **Agents holding leases for long plans.** Cap a lease's total
  duration, and have the refusal tell the agent to release and re-plan.
- **Code view lag.** Transforming typed text over a stream of structured
  edits must stay within the 1-frame budget. Range transforms are cheap,
  but each one is followed by a re-parse (≤ 2 ms per 1 000 lines,
  section 6).
- **Undo across writers** is the classic hard case. "Refuse when the
  inverse no longer applies" keeps it honest. Whether people find the
  refusals acceptable is a question for testing with users, not for
  this document.

## References (descriptions of designs, not code)

- D. Nichols et al., "High-latency, low-bandwidth windowing in the
  Jupiter collaboration system", UIST 1995.
- C. Ellis and S. Gibbs, "Concurrency control in groupware systems",
  SIGMOD 1989.
- M. Shapiro et al., "Conflict-free replicated data types", SSS 2011.
- M. Weidner and M. Kleppmann, "The Art of the Fugue: minimizing
  interleaving in collaborative text editing", 2023.
- M. Kleppmann et al., "A highly-available move operation for
  replicated trees", IEEE TPDS 2021.
- G. Litt et al., "Peritext: a CRDT for collaborative rich text
  editing", CSCW 2022.
- Figma's public engineering posts on its multiplayer design
  (inspiration only; no code read).
