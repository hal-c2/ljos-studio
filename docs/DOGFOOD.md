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

| # | ID | Rank | Where | Title | Now |
|---|---|---|---|---|---|
| 1 | D1 | blocker | window | The canvas has no direct manipulation: a drag pans the frame, nothing moves or resizes | fixed, bf87faa, a561a39 |
| 2 | D2 | blocker | window | Typing in the code view replaces the whole selected node | fixed, 53783be |
| 3 | D3 | blocker | window | No "New document": an empty project cannot be started in the window | fixed, c94eeb7 |
| 4 | D4 | blocker | window | Play mode's canvas picture goes stale after the first navigation | fixed, d553611 |
| 5 | D5 | major | window | The palette offers only the project's blocks, no Rectangle, Text, Row or control | fixed, 7ade037 |
| 6 | D6 | major | window | The canvas clips the frame instead of fitting it: half of a phone, a third of a desktop | fixed, a561a39 |
| 7 | D7 | major | window | A state's `when` and its changes cannot be set in the States panel | fixed, 5569e87 |
| 8 | D8 | major | MCP | `tools/list` answers 62 KB, past the 40 KB cap | fixed, c2608d2 |
| 9 | D9 | major | both | A killed Studio leaves its preview process running | fixed, 575f69f |
| 10 | D10 | major | window | Undo in the code view is one keystroke at a time | fixed, 7ca16e4 |

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
- **Fixed** (bf87faa, a561a39): One tool, `node_arrange`, places, aligns and copies; the window draws an outline, eight handles, a ghost and guides, and drops one edit. In a Layout a move reorders and a resize writes `Layout.preferred*`; an `anchors.fill` object says it cannot move. Remains: canvas edits while recording a state write base values (D7), no rubber-band select, no group resize, no geometry on the web host.

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
- **Fixed** (53783be): A full-width band marks the selection; typing inserts at the caret.

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
- **Fixed** (c94eeb7): The Documents panel has New; a window on a directory with nothing in it opens with `Main.qml`. `--mcp` never creates one. Remains: a window on an agent's empty workspace writes that `Main.qml`, and the agent's own `document_new Main.qml` is then refused.

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
- **Fixed** (d553611): Cause: nothing asked for a new picture after a click, since a click makes no new generation. A click now looks again every 100 ms, up to 30 looks, stopping after 3 unchanged. A continuous animation then went stale after the bound; in Play the picture now follows what the app says when it sleeps (`app_watch` `rests`, `wake_in_ms`): looked at when it has woken, at most ten times a second, and an app that will not wake by itself is asked nothing. Not `paints`, which a preview without a window never counts, nor `frames`, which a screenshot moves. `features/ui/play_follow.feature`.

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
- **Fixed** (7ade037): The palette offers the toolkit's types by module, with a search. Remains: some non-visual types are offered too.

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
- **Fixed** (a561a39): The frame fits the panel until a zoom is picked: Fit, 100 %, Ctrl+wheel, Ctrl+0, Ctrl+1, Ctrl+=, Ctrl+-.

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
- **Fixed** (5569e87): `when` and presets already existed; the gap was recording and seeing a state. Pick a state and edits go into it, and `preview_state.show` shows it. A canvas drag, an arrow press and an align while recording wrote the base value, and the state's override then hid it; they now go into the picked state (`node_arrange` `record`), and a move that would reorder a layout says a state cannot.

#### D8. `tools/list` answers 62 KB

- **Where:** MCP.
- **Tried:** to start an agent session.
- **Happened:** the list of tools alone is 62 KB, over the 40 KB per answer
  cap in DESIGN.md section 6 (ARCHITECTURE section 19.6), before any work.
- **Fix:** shorter descriptions with the detail moved into the input
  schemas' own descriptions or a `help` resource; or split tools by
  capability so a session with `inspect` only lists those. Owner: Studio,
  `studio_mcp` tool table.
- **Fixed** (c2608d2): 37 585 bytes, under the 40 KB cap. Answer schemas list fields and types only.

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
- **Fixed** (575f69f): SIGTERM and SIGINT close the preview within 2 s. Remains: SIGKILL, which only the child can notice (Ljós request 4).

#### D10. Undo in the code view is one keystroke at a time

- **Where:** window.
- **Tried:** to undo a 26-character mistake.
- **Happened:** 26 undos. A paste is one step, which is right.
- **Fix:** coalesce typing into one undo step per word or per pause (about
  a second), as text editors do. Owner: Studio, the undo stack in
  `studio_core` where code-view edits are recorded.
- **Fixed** (7ca16e4): Typing is one undo step per word or pause, through the new `typing` argument; an agent's edits stay one step each.

#### D11. The Documents list shows seven rows and no scroll bar

- **Where:** window.
- **Tried:** to open the eighth file of the site.
- **Happened:** only seven rows show and nothing says there are more; the
  wheel scrolls. A missed click left `Pricing.qml` open, and a paste meant
  for `Testimonial.qml` replaced it. One undo restored it.
- **Evidence:** `docs/dogfood/docs-clipped.png`.
- **Fix:** a scroll bar, and a panel that takes the height it is given.
  Owner: Studio, Documents block.
- **Fixed** (dff6a44, 1104057): A scroll bar shows when the list overflows and stays hidden when not.

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
- **Fixed**: `format: "outline"` lists every visible item of the project with its type, id, box, file and line, and a text's words, below the accessible snapshot. Scenario in `features/preview/preview.feature`.

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
- **Fixed**: dragging a row is `node_move`; a double-click names the object, what the document says of it following (`node_arrange` `rename`); Hide/Show and Lock/Unlock on each row are `node_arrange` actions that leave the file alone; a scroll bar is the cue. Hiding needs an id, since the preview finds an object by it.

#### D14. Basic controls do not follow `Colors.dark`

- **Where:** both (Ljós).
- **Tried:** the notes app's settings switch that sets `Colors.dark`.
- **Happened:** the page went dark but a `ToolButton`'s text stayed dark on
  dark; each control needed its `palette` set by hand.
- **Fix:** Ljós request 1: the Basic style reads its palette from
  `Ljos.Design` when the module is imported.
- **Fixed** in Ljós e318dad7; the notes sample no longer sets `palette.buttonText` by hand. Checked off-screen in light and dark.

### Minor

#### D15. The inspector shows colours as text and keeps old text

- **Where:** window.
- **Happened:** colours are plain text with no swatch, the value field is
  narrow, the new-property row keeps the old text after Set, and a property
  declared without a value (`model`, `stack`) has no row.
- **Evidence:** `docs/dogfood/inspector-text.png`.
- **Fix:** a swatch with a picker, a wider field, clear the row after Set,
  rows for declared properties. Owner: Studio, Inspector block.
- **Fixed**: a colour row has a swatch that opens a picker (hue, saturation, value, hex; one undo step per drag); the field is wider; a property declared without a value has a row. The swatch of a colour written as an expression paints nothing, since only what is written is read here.

#### D16. A file made outside Studio shows only after the next tool call

- **Where:** both.
- **Fix:** watch the project directory as the preview already watches
  files. Owner: Studio, `studio_core` project.
- **Fixed**: `studio_mcp` follows the project directory on kernel file events, one watch per directory a walk lists (so not `target` or `node_modules`). The session's own writes and files saved unchanged tell nobody, so the selection and edit budgets pay nothing. On no watch available it falls back to the next tool call, as before.

#### D17. The first picture after a target switch is squeezed

- **Where:** window.
- **Happened:** after choosing Phone or Tablet the first picture is the old
  size's, scaled into the new frame; the next is right.
- **Evidence:** `docs/dogfood/preview-squeezed.png`.
- **Fix:** drop pictures whose size is not the frame's. Owner: Studio,
  `Canvas.qml`.
- **Fixed** (3d0faae): The real cause was wider than the first picture: a picture was fetched once per generation and a resize is not one, so the old size stayed squeezed until the next edit.

#### D18. `node_insert` writes default ids and puts states first

- **Where:** MCP.
- **Happened:** inserted objects get `id: column1` and the like, there is
  no blank line between inserted siblings, and `state_edit` puts `states`
  at the top of the object, not the bottom where people write it.
- **Fix:** an id only when the caller names one or a binding needs it;
  keep the file's spacing; put `states` and `transitions` last. Owner:
  Studio, `studio_core` edit formatting.
- **Not changed**: DECISION: default ids are specified by existing scenarios and `preview_click` and state recording need them; agents can pass `id: false`. Where states go is what the `state_edit` scenarios specify.

#### D19. `node_insert` cannot add a function or a signal handler

- **Where:** MCP.
- **Happened:** a `function` or `onClicked` in the inserted text is
  refused; the agent wrote those in `document_new`'s text instead.
- **Fix:** accept members in `node_insert`, or a `member_insert` tool.
  Owner: Studio, `studio_core`.
- **Fixed**: `node_insert` takes one member as its `text`; a name the object already has is refused (`features/edit/members.feature`).

#### D20. `JsonListModel` is read-only

- **Where:** both (Ljós).
- **Happened:** to edit the notes, `Main.qml` copies the rows into a
  `ListModel`. A copy made synchronously in `onCountChanged` seemed to hang
  once under load; `Qt.callLater` did not.
- **Fix:** Ljós request 5.
- **Fixed** in Ljós e318dad7; the notes sample edits its `JsonListModel` directly, no `ListModel` copy. Checked off-screen by adding a note.

#### D21. The window sometimes never paints

- **Where:** Ljós (both samples, `ljos run` in the sandbox).
- **Happened:** about half the runs under heavy build load, the app's
  window maps but is never drawn: the process sits in `epoll_wait` at 0 %
  CPU with a black window. Idle, six of six runs drew.
- **Fix:** Ljós request 6.
- **Open**: Ljós request 6.

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
- **Fixed** (9a5a3a6): Cause: the check for leaving the workspace looked at the path's parts, and a root is not a plain part, so any absolute path was called outside. An agent that carried on wrote into the project still open; the refusal now names it. It now takes an absolute path below the workspace. The agent's site was built again in a new directory: 40 calls, no failures.

#### D24. A site's page did not size the entry's root on the web

- **Where:** web export, found while checking D23's site.
- **Happened:** the root kept its declared size, so a phone saw the corner
  of a desktop page and no breakpoint state turned on.
- **Fixed** (8b641c5): the page sizes the entry's root, as a window does.

#### D25. `--shell` refused to start on a first run

- **Where:** window, found while measuring.
- **Happened:** with no Studio config directory yet, a window given `--shell`
  stopped with "cannot watch": it watches the directory the theme file would
  be in.
- **Fixed** (e77f895): that directory is made first. Checked in the sandbox with
  the directory removed. No scenario: it needs a window started as a process.

#### D22. Studio's window is black with the default painter in the sandbox

- **Where:** window (environment).
- **Happened:** with wgpu in the headless sway sandbox the window stays
  black; `LJOS_PAINTER=cpu` draws.
- **Fixed** by the pin at Ljós e318dad: the default painter draws in the
  sandbox with no `LJOS_PAINTER`, driven and looked at on 2026-10-31.

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
| Resize handles | yes | D1 fixed |
| Snapping and guides | yes; Ctrl turns snapping off | D1 fixed |
| Multi-select | yes: shift-click; no rubber-band, no group resize | D1 fixed |
| Align and distribute | yes | D1 fixed |
| Nudge with arrow keys | yes, once the canvas has focus; one undo step a press | D1 fixed |
| Copy, paste, duplicate | yes, with Studio's own clipboard, not the system's | D1 fixed |
| Undo a person trusts | yes: a drop is one step, typing one per word or pause | D10 fixed |
| Layers: drag to reorder, rename, hide, lock | yes, driven in a real window off-screen | D13 fixed |
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

Studio, `budgets` and the preview's `ljos` are all release builds, taken
2026-10-31 at about 16:20 UTC. Before N2 the preview was a debug `ljos`, so the
"was" figures are not a like-for-like comparison.

| Path | Budget | Stock project | `samples/site` | `samples/notes` | Verdict |
|---|---|---|---|---|---|
| Drag in the real window, frame interval p50 / p95 / max | 16.7 ms (60 fps) | 17 / 18 / 24 ms | | | about 59 fps, see below |
| Edit to canvas picture, one per 250 ms, p50 / p95 / max | 50 ms p95 | 27.3 / 30.2 / 30.4 ms (was 158.1 / 234.5) | 35.7 / 40.8 / 45.0 ms (was 258.7 / 264.9) | 23.3 to 24.9 / 25.4 to 29.4 ms over three runs; max 31.6, 53.5 and 341.1, the two high ones a rename on the data drive (was 32.4 / 52.8 / 72.8) | within |
| Edit to picture, back to back, p50 / p95 | | 25.1 / 1028.8 ms | | | the preview's call rate limit, see below |
| Cold start, p50 / p95 | 1 s | 52.3 / 65.2 ms (200 files) | 59.4 / 60.2 ms | 53.6 / 57.9 ms | within |
| Selection to inspector, p50 / p95 / max | 16 ms | 8.4 to 10.6 / 10.2 to 12.6 / 14.3 to 17.2 ms over four runs (was 18.7 / 22.9 / 24.8) | | | within |
| Idle for 5 s | 0 % CPU | 0 ms CPU, 0 wakes, 0 frames | | | within |
| Drag, engine work per pointer move, p50 / p95 / max | | 34 / 45 / 418 us | | | no paint in it |

**The drag.** Dragging `card0` 150 moves across a release window, in the
desktop:cua sandbox with software GL (llvmpipe, a headless sway output). The
figure is the wall-clock time between frames, read by a `FrameAnimation` in a
private copy of the shell, in whole milliseconds. Over 613 frames (a median of 17 ms is about 59 fps)
the slowest was 24 ms, so at least that one frame missed 16.7 ms. It is a
cadence, not paint time: a frame's own paint shows only by pushing the next
interval out. No real GPU was measured, and keeping a `FrameAnimation` running
is itself continuous frames, which an ordinary drag does not have.

**Selection to inspector was over budget** for three reasons, none of them the
inspector. A selection's answer was followed by a second, redundant read of the
session (the write's notice reached the UI thread ahead of the answer of the
job that wrote); the overlay's geometry read published the whole view again;
and every publish set all seventeen lists, so panels made their rows anew. With
no preview it was already 11.2 / 13.8 ms. Now one read, no publish for geometry,
and only lists that changed are set; the overlay reads siblings only on a press.

**Back to back edits:** the p95 is about a second of waiting for a picture,
because the preview limits how often it is called. One edit per 250 ms does not
hit that limit.

| MCP, as an agent | Calls | Request bytes | Answer bytes |
|---|---|---|---|
| `samples/notes`, new directory, 2026-10-31 about 16:25 UTC | 32 (was 34), none failed | 17 261 (was 17 409) | 18 109 (was 37 421); images not counted |
| `samples/site`, new directory | 40, none failed (was 2, D23) | 18 246 | 19 195 (screenshot images not counted) |
| `tools/list`, once per session | 1, 25 tools | | 37 585 (was 62 218; cap 40 KB) |

#### D26. Objects of a 3D scene were told they are not shown

- **Where:** window, found placing a `View3D` on the native preview.
- **Happened:** selecting a `Model` said "The preview does not show this
  object" with the cube in plain sight, the align buttons stayed and answered
  "the preview does not show `n6`". Editing `eulerRotation` and pressing Enter
  was refused as "not a literal", though the row held an expression.
- **Fixed:** the canvas says the object has no box and offers no alignment; a
  row holding an expression stays one when edited; a literal row still refuses
  (`features/ui/quick3d.feature`). Driven in a real window off-screen.
- **Open:** a click on the picture selects the `View3D`, not the model under
  the pointer, and there are no 3D handles or orbit: a 3D object is chosen in
  Layers and edited in the inspector.

**Phase 5, measured before the review fixes** (machine load 10 to 17 from other
builds). Selection to inspector, driver's clock advanced between clicks: p50 /
p95 / max 10.1 / 11.3 / 15.9 ms, and 14.8 / 16.7 / 18.0 at load 17. Edit to picture, one per 250 ms: stock
22.4 ms p50 on the data drive, whose p95 of 904.1 is a stalled rename; with the
project on tmpfs 23.0 / 27.3 / 32.1. `samples/notes` on tmpfs 26.8 / 43.0 / 46.5;
on the drive 35.8 / 1121.2 / 3262.2. `samples/site` on the drive 34.2 / 41.9 /
47.1; on tmpfs 41.0 / 57.7 / 130.8. Idle 0 ms CPU, 0 wakes, 0 frames. `tools/list` is 40 324 bytes
against the scenario's 40 960: little room left.

## Not done

- Real frame time on a real GPU. The drag above is software GL in the sandbox.
- The window findings other than the items fixed here were not walked again.
- A standalone `ljos run` at 390 x 844 stayed black (D21). On the web, `export:web:check`
  passed with 3 files, 443 282 bytes (157 080 gzipped), first contentful paint
  144.0 ms, and headless Chromium showed no errors at 390 x 844, 800 x 1100 and
  1280 x 800.
