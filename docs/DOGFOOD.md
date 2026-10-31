# Dogfooding Studio

Two real samples were built in Studio, each twice: once as a person in
Studio's window (off-screen, in the desktop:cua sandbox, by clicks and
keys), and once as an agent through Studio's MCP endpoint (one tool call at
a time, every call counted with its request and answer bytes).

- `samples/notes`: field notes, a four-screen phone app (StackView, a
  JSON-backed list, a validated form, states, light and dark).
- `samples/site`: Varða, a product site (nav, hero, features grid,
  testimonial, pricing, call to action, footer), responsive through
  breakpoint states, exported for the web.

What follows is what got in the way, ranked. **Blocker**: it cannot be done
in Studio at all, or work is lost. **Major**: it can be done, but only by
leaving the design surface (hand-written QML in the code view, or a second
tool), or it costs more than a person or agent will put up with.
**Minor**: friction. Within a rank, items are ordered by how often they bit.

DECISION: the code view counts as "in Studio". A thing a person can only do
by writing QML in the code view is major, not a blocker, unless the code
view itself loses work.

## Top ten

| # | ID | Rank | Where | Title |
|---|---|---|---|---|
| 1 | D1 | blocker | window | The canvas has no direct manipulation: a drag pans the frame, nothing moves or resizes |
| 2 | D2 | blocker | window | Typing in the code view replaces the whole selected node |
| 3 | D3 | blocker | window | No "New document": an empty project cannot be started in the window |
| 4 | D4 | blocker | window | Play mode's canvas picture goes stale after the first navigation |
| 5 | D5 | major | window | The palette offers only the project's blocks, no Rectangle, Text, Row or control |
| 6 | D6 | major | window | The canvas clips the frame instead of fitting it: half of a phone, a third of a desktop |
| 7 | D7 | major | window | A state's `when` and its changes cannot be set in the States panel |
| 8 | D8 | major | MCP | `tools/list` answers 62 KB, past the 40 KB cap |
| 9 | D9 | major | both | A killed Studio leaves its preview process running |
| 10 | D10 | major | window | Undo in the code view is one keystroke at a time |

## Items

### Blocker

#### D1. The canvas has no direct manipulation

- **Where:** window.
- **Tried:** to move the hero's button and to widen a pricing card by
  dragging on the canvas, as in any design tool.
- **Happened:** a press on the canvas selects the object under it (and its
  code range), but a drag pans the frame. There is no selection outline, no
  resize handle, no guide or snap, and nothing moves. Every geometry change
  went through the inspector or the code view. It bit on every screen of
  both samples.
- **Evidence:** `budgets` drives the drag (the drag line in Numbers): the
  frame moves, the object does not.
- **Fix:** a canvas overlay drawn by Studio (outline, eight handles, guides
  against siblings and the parent's edges), with the edit applied on drop as
  section 6 of DESIGN.md says. Owner: Studio, `studio_blocks` canvas
  (`Canvas.qml`) over the existing `node_with`/selection seam; the drop
  becomes a `binding_set` on `x`/`y`/`width`/`height` or an anchor margin.

#### D2. Typing in the code view replaces the whole selected node

- **Where:** window.
- **Tried:** to select `Hero` in Layers, then click into the code view and
  type a property.
- **Happened:** selecting a layer selects its code range in the code view,
  so the first keystroke replaced the whole object with one character. Undo
  brought it back, but only one keystroke at a time (D10).
- **Evidence:** `docs/dogfood/code-replaced.png`.
- **Fix:** show the selected range with a highlight, not a text selection,
  and put the caret at the range's start. Owner: Studio, the code view block
  (`studio_blocks`) and the selection seam that sets its cursor.

#### D3. No "New document"

- **Where:** window.
- **Tried:** to start `samples/notes` and `samples/site` from an empty
  folder in the window.
- **Happened:** there is no New, the code view is disabled with no document
  open, and the palette has nothing to drop into. `Main.qml` had to be made
  outside Studio, and Studio only listed it after the next tool call (D16).
- **Fix:** a New button in Documents that calls `document_new` (name, root
  type), and an empty project that opens with one. Owner: Studio, Documents
  block plus `studio_core` (the tool exists already).

#### D4. Play mode's canvas picture goes stale

- **Where:** window.
- **Tried:** to click through the notes app in Play: list → a note → back.
- **Happened:** the first click navigated (the preview's outline showed the
  detail screen) but the canvas kept showing the list. The cause was not
  traced; a picture taken before the push transition ended and never asked
  for again would explain it.
- **Evidence:** `docs/dogfood/play-stale.png`.
- **Fix:** in Play, ask for a picture again while the preview runs an
  animation or transition (the preview knows: it reports `at rest`), not
  only after an edit. Owner: Studio, the preview client in `studio_core`;
  Ljós may need to say when it is at rest after input, not only after a
  reload (request 3).

### Major

#### D5. The palette offers only the project's blocks

- **Where:** window.
- **Tried:** to add a Rectangle, a Text, a Row and a Button from the
  palette.
- **Happened:** the palette lists the project's own blocks (`NavBar`,
  `Hero` and so on once they existed) and nothing from QtQuick, Layouts or
  Controls. Every built-in object was typed or pasted in the code view.
  This bit on every object of both samples.
- **Fix:** list the imported modules' types (the type registry Studio
  already reads for the inspector), grouped by module, with a search field.
  Owner: Studio, Palette block; the list comes from `studio_core`'s types
  seam.

#### D6. The canvas clips the frame instead of fitting it

- **Where:** window.
- **Tried:** the site at Phone (390 × 844) and Desktop (1280 × 800).
- **Happened:** the frame is drawn at 1:1 and clipped by the canvas: the
  phone is cut at y ≈ 604, the desktop at x ≈ 844, so the hero heading, the
  nav links and the "Get the app" button are off the edge. There is no fit
  or zoom, and a drag pans (D1).
- **Evidence:** `docs/dogfood/site-clipped-desktop.png`.
- **Fix:** fit the frame to the canvas by default, with zoom (Ctrl+wheel,
  Ctrl+0 fit, Ctrl+1 100 %). Owner: Studio, `Canvas.qml` (a scale on the
  picture; hit testing divides by it).

#### D7. A state's `when` and its changes cannot be set in the States panel

- **Where:** window (MCP has it: `state_edit` with `breakpoint`).
- **Tried:** to add the site's `phone`, `tablet` and `desktop` states and
  the notes list's `empty` state in the window.
- **Happened:** the panel adds, renames, applies and removes a state on the
  root, but cannot set `when` or a change; clicking a state did not enter it
  ("No state"). All states were written in the code view.
- **Fix:** a state row that, when entered, records inspector edits as
  `PropertyChanges`, and a "when" field with the breakpoint presets the MCP
  tool already has. Owner: Studio, States block over `state_edit`.

#### D8. `tools/list` answers 62 KB

- **Where:** MCP.
- **Tried:** to start an agent session.
- **Happened:** the list of tools alone is 62 KB, over the 40 KB per answer
  cap in DESIGN.md section 6 (ARCHITECTURE section 19.6), before any work.
- **Fix:** shorter descriptions with the detail moved into the input
  schemas' own descriptions or a `help` resource; or split tools by
  capability so a session with `inspect` only lists those. Owner: Studio,
  `studio_mcp` tool table.

#### D9. A killed Studio leaves its preview process running

- **Where:** both.
- **Tried:** to stop Studio with SIGTERM between runs.
- **Happened:** the `ljos run --headless` preview child lived on. The
  machine had 58 such orphans (about 1.3 GB), up to nine hours old, from
  Studio's own test temporary directories.
- **Fix:** `NativeHost` closes the child on `Drop`, which a signal skips.
  Handle SIGTERM and SIGINT in the shell so the host is dropped, and give
  the child a way to notice its parent is gone: a pipe on its stdin (now
  `Stdio::null()`) that Ljós watches. Owner: Studio, `studio_preview`
  (`NativeHost::start`); Ljós, request 4.

#### D10. Undo in the code view is one keystroke at a time

- **Where:** window.
- **Tried:** to undo a 26-character mistake.
- **Happened:** 26 undos. A paste is one step, which is right.
- **Fix:** coalesce typing into one undo step per word or per pause (about
  a second), as text editors do. Owner: Studio, the undo stack in
  `studio_core` where code-view edits are recorded.

#### D11. The Documents list shows seven rows and no scroll bar

- **Where:** window.
- **Tried:** to open the eighth file of the site.
- **Happened:** only seven rows show and nothing says there are more; the
  wheel scrolls. A missed click left `Pricing.qml` open, and a paste meant
  for `Testimonial.qml` replaced it. One undo restored it.
- **Evidence:** `docs/dogfood/docs-clipped.png`.
- **Fix:** a scroll bar, and a panel that takes the height it is given.
  Owner: Studio, Documents block.

#### D12. The outline hides objects that are not accessible

- **Where:** MCP.
- **Tried:** as the agent, to check the notes list's cards in the preview's
  outline (`preview_screenshot` with `format: outline`).
- **Happened:** cards made of a `Rectangle` with `Text` in it do not show;
  only controls and texts with accessible roles do, so the agent could not
  see the list's rows.
- **Fix:** an outline that includes every visible item with its type, id
  and geometry, not only the accessible ones. Owner: Ljós (request 2),
  since the outline comes from the preview.

#### D13. The Layers panel cannot reorder, rename, hide or lock

- **Where:** window.
- **Tried:** to move the testimonial above the features, and to hide the
  footer while working on pricing.
- **Happened:** only Move up / Move down / Remove buttons. No drag to
  reorder, no rename (setting `id`), no hide or lock. The tree shows a part
  of the document and scrolls without a cue.
- **Fix:** drag-to-reorder (a `node_move`), a double-click to rename the
  id, eye and lock toggles that are Studio's own (not written into the
  file). Owner: Studio, Layers block.

#### D14. Basic controls do not follow `Colors.dark`

- **Where:** both (Ljós).
- **Tried:** the notes app's settings switch that sets `Colors.dark`.
- **Happened:** the page went dark but a `ToolButton`'s text stayed dark on
  dark; each control needed its `palette` set by hand.
- **Fix:** Ljós request 1: the Basic style reads its palette from
  `Ljos.Design` when the module is imported.

### Minor

#### D15. The inspector shows colours as text and keeps old text

- **Where:** window.
- **Happened:** colours are plain text with no swatch, the value field is
  narrow, the new-property row keeps the old text after Set, and a property
  declared without a value (`model`, `stack`) has no row.
- **Evidence:** `docs/dogfood/inspector-text.png`.
- **Fix:** a swatch with a picker, a wider field, clear the row after Set,
  rows for declared properties. Owner: Studio, Inspector block.

#### D16. A file made outside Studio shows only after the next tool call

- **Where:** both.
- **Fix:** watch the project directory as the preview already watches
  files. Owner: Studio, `studio_core` project.

#### D17. The first picture after a target switch is squeezed

- **Where:** window.
- **Happened:** after choosing Phone or Tablet the first picture is the old
  size's, scaled into the new frame; the next is right.
- **Evidence:** `docs/dogfood/preview-squeezed.png`.
- **Fix:** drop pictures whose size is not the frame's. Owner: Studio,
  `Canvas.qml`.

#### D18. `node_insert` writes default ids and puts states first

- **Where:** MCP.
- **Happened:** inserted objects get `id: column1` and the like, there is
  no blank line between inserted siblings, and `state_edit` puts `states`
  at the top of the object, not the bottom where people write it.
- **Fix:** an id only when the caller names one or a binding needs it;
  keep the file's spacing; put `states` and `transitions` last. Owner:
  Studio, `studio_core` edit formatting.

#### D19. `node_insert` cannot add a function or a signal handler

- **Where:** MCP.
- **Happened:** a `function` or `onClicked` in the inserted text is
  refused; the agent wrote those in `document_new`'s text instead.
- **Fix:** accept members in `node_insert`, or a `member_insert` tool.
  Owner: Studio, `studio_core`.

#### D20. `JsonListModel` is read-only

- **Where:** both (Ljós).
- **Happened:** to edit the notes, `Main.qml` copies the rows into a
  `ListModel`. A copy made synchronously in `onCountChanged` seemed to hang
  once under load; `Qt.callLater` did not.
- **Fix:** Ljós request 5.

#### D21. The window sometimes never paints

- **Where:** Ljós (both samples, `ljos run` in the sandbox).
- **Happened:** about half the runs under heavy build load, the app's
  window maps but is never drawn: the process sits in `epoll_wait` at 0 %
  CPU with a black window. Idle, six of six runs drew.
- **Fix:** Ljós request 6.

#### D23. `project_open` refused a path inside the workspace, and the build carried on

- **Where:** MCP.
- **Tried:** `project_open` with the absolute path of
  `dog-agent/site`, with Studio started on `dog-agent`.
- **Happened:** `path_denied`, "outside the workspace". Why was not
  investigated. The later calls still succeeded and wrote the site into the
  workspace's root, beside `notes/`. `export` then refused with `no_web_face`
  because of `notes/EntryModel.qml`, a document of the other sample.
- **Fix:** to be decided after finding why the path was refused. Owner:
  Studio, `studio_mcp` `project_open` and `export`'s choice of documents.

#### D22. Studio's window is black with the default painter in the sandbox

- **Where:** window (environment).
- **Happened:** with wgpu in the headless sway sandbox the window stays
  black; `LJOS_PAINTER=cpu` draws.
- **Fix:** none in Studio; noted for anyone driving it off-screen.

## What worked

- Inspector Set, Undo and Redo; a paste in the code view is one undo step.
- Tokens: the seed hue field and `theme_set` re-theme the preview at once.
- `state_edit` with `breakpoint` writes `when` from the preview targets'
  own bounds (600 and 1024), and the canvas says which state is shown
  ("State desktop").
- The agent built the notes app with no failed call. The site's objects,
  states and theme also went in without a failure, but see D23.

## Direct-manipulation checklist

"Not tried" means the samples never called for it, so it says nothing about
whether the feature exists.

| Feature | Answer | Items |
|---|---|---|
| Resize handles | no | D1 |
| Snapping and guides | no | D1 |
| Multi-select | no: never seen, not tried on purpose | D1 |
| Align and distribute | no: never seen, not tried on purpose | D1 |
| Nudge with arrow keys | not tried | |
| Copy, paste, duplicate | partly: text in the code view; objects not tried | D2 |
| Undo a person trusts | partly: inspector and paste yes, typing one key at a time | D10 |
| Layers: drag to reorder, rename, hide, lock | no: Move up and Move down only | D13 |
| Inspector fields for Layouts, anchors, padding, margins | partly: plain text rows; no dedicated fields seen | D15 |
| Image and asset import | not tried: neither sample has an image | |
| Zoom and pan feel | partly: drag pans, no zoom or fit | D1, D6 |
| Keyboard shortcuts | partly: Ctrl+A and Ctrl+V in the code view; not surveyed | |

## Numbers

Taken 2026-10-31 at 13:27-13:28 on scratch copies of the samples:

```text
LJOS=$S/studio/target-ui/debug/ljos target-dog/release/examples/budgets
LJOS=... budgets sample <copy>/site Main.qml "Rectangle root" width 40
LJOS=... budgets sample <copy>/notes Main.qml "Rectangle root" width 40
```

Studio and `budgets` are release builds. **The preview is a debug `ljos`**,
the only one built, and other builds kept the load average at 11-20. Both
push the picture numbers up by an amount not measured.

| Path | Budget | Stock project | `samples/site` | `samples/notes` | Verdict |
|---|---|---|---|---|---|
| Drag, per pointer move, p50 / p95 / max | 16.7 ms (60 fps) | 17 / 29 / 147 us | 18 / 33 / 121 us | 18 / 30 / 139 us | not shown, see below |
| Edit to canvas picture, p50 / p95 | 50 ms p95 | 158.1 / 234.5 ms | 258.7 / 264.9 ms | 230.8 / 236.6 ms | over |
| Cold start, p50 / p95 | 1 s | 53.2 / 67.3 ms (200 files) | 52.7 / 60.7 ms | 49.3 / 60.1 ms | within |
| Selection to inspector, p50 / p95 / max | 16 ms | 14.1 / 19.0 / 21.2 ms | | | p95 over |
| Idle for 5 s | 0 % CPU | 0 ms CPU, 0 wakes | | | within |

**The drag number does not show that the 60 fps budget is met.** It is the
engine's work for a pointer move, on a stage with no painter, so egui's
tessellation and the GPU are not in it. It also times the only drag there is,
a pan: a node cannot be dragged (D1). Real frame time on a real window was not
measured.

**Edit to picture:** the three slowest edits of each sample spent 190-219 ms
fetching the picture (46-81 KB) from the debug preview. The preview itself
showed the change 23-32 ms after the save in those edits, so the fetch is the
first thing to remeasure with a release `ljos`. One site edit took 563 ms, 303
of it in the file rename, which is probably load.

| MCP, as an agent | Calls | Request bytes | Answer bytes |
|---|---|---|---|
| `samples/notes` | 34 | 17 409 | 37 421 (19 006 of it one raw screenshot) |
| `samples/site` | 40, 2 failed (D23) | 18 246 | 18 964 (screenshot images not counted) |
| `tools/list`, once per session | 1 | | 62 218 |

## Not done

- The site's export for the web and what `mise run export:web:check` runs:
  the only `export` call was refused (D23), so no site was built or checked.
- The agent's site build was not repeated in a clean project.
- The site at phone and tablet was not confirmed. A standalone `ljos run` at
  390 x 844 stayed black (D21), and an earlier phone frame in the canvas showed
  nav links that `compact` should hide; whether that was the preview or the QML
  is still open.
