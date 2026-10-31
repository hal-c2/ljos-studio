# Ljós Studio: design

Status: phase 1, 2026-10-30. The block model it builds on is proposed to
Ljós as ADR 0003 ("Functional blocks, shells that skin them, and edits that
keep the file"); the lossless edit seam is spiked as Ljós's `qml_edit`
crate. Both are on the `studio-needs` branch of the lead's ljos-ui clone
until the Ljós lead integrates them.

Studio is a visual designer for Ljós applications and for sites built with
ljos-web. Its users are people and agents: every capability is an MCP tool
first and a panel second. It takes ideas from canvas design tools (an
infinite canvas of frames, layers, an inspector, components with variants,
tokens, prototyping, breakpoints) and does something those tools cannot:
the design *is* the application's QML, with its bindings, states and
behaviour, so nothing is handed off and nothing is exported back.

All of this is clean room. Framer, Figma, Penpot and open-pencil are
inspiration from their public product pages and docs only; none of their
code or file formats is read or copied.

## 1. Goals and non-goals

### Goals

1. **The QML is the document.** Studio opens a project's `.qml` files and
   saves the same files. There is no scene format, binary or JSON, beside
   them. Designer-only metadata (canvas position of a frame, a note) lives
   in a deletable sidecar (`.studio/`) or a QML comment; deleting it loses
   only that metadata.
2. **Lossless edits.** A canvas edit changes the members it touches and
   leaves every other byte as it was: comments, blank lines, ordering and
   the author's formatting. The code view and the canvas are two views of
   the same text and either can edit it.
3. **Blocks carry behaviour, shells carry looks.** Studio's main job is
   composing and skinning functional blocks (ADR 0003): list them with
   their contracts, drop them into a shell, bind their state to skins, wire
   actions to handlers, fill their slots, theme them with tokens, and
   preview with live or mocked state.
4. **Bindings and states are first-class.** Auto layout is Layouts and
   anchors; components with variants are components with `states`;
   prototyping is states, transitions and handlers; responsive breakpoints
   are bindings on the window's width; CMS-style content is models. Studio
   edits these as what they are rather than as frozen pixels.
5. **MCP first.** Every capability is a tool on Studio's MCP surface before
   or alongside its UI (see [MCP.md](MCP.md)). An agent can do anything a
   person can, and the person sees it on the canvas as it happens.
6. **One designer, every target.** Studio is itself a Ljós application
   (QML shell + Rust blocks). It runs natively on desktop and in the
   browser on ljos-web, and previews a design on desktop, mobile-sized and
   web targets.
7. **As flexible as the toolkit.** Studio's own UI is a shell over its own
   blocks, so a user can reskin or rearrange Studio with Studio.

### Non-goals (for now)

- A vector illustration tool. Shapes are `Rectangle`, `Shape`, `Image` and
  the Ljós `Canvas`; pen tools and boolean path editing are not planned.
- Importing other tools' files. A paste of SVG or an image is in scope;
  another tool's document format is not.
- Multiplayer. The edit model is built so it can come later (edits are
  small, ordered, invertible operations on text), but nothing in phase 2–4
  depends on it. [MULTIPLAYER.md](MULTIPLAYER.md) compares the ways to do
  it and recommends one authority with edits rebased by handle and leases.
- Code generation from pictures. Studio edits QML; it never produces QML
  from a screenshot or a drawing.
- A hosted service. Studio is an application; sharing is git and files.

## 2. Modules, seams and adapters

```
            people                         agents
              │                              │
   ┌──────────▼───────────┐       ┌──────────▼───────────┐
   │ Studio shell (QML)   │       │ studio MCP server    │  ← ljos_mcp_core
   │ canvas, layers,      │       │ (tools in MCP.md)    │
   │ inspector, palette,  │       └──────────┬───────────┘
   │ code view, tokens    │                  │
   └──────────┬───────────┘                  │
              │ actions / published state    │ same actions
   ┌──────────▼──────────────────────────────▼───────────┐
   │ studio_core (Rust): Session                          │
   │  Project ─ Documents ─ Selection ─ History ─ Preview │
   └──┬──────────┬───────────┬────────────┬───────────────┘
      │          │           │            │
  qml_edit   ljos_blocks  qml_front/   PreviewHost ──► ljos_dev inspector (app_*)
 (lossless   (registry,   ljos_compiler  (adapter:      in a child process (native)
  edits)     contracts)   (check, IR)    native | web)  or an iframe/worker (web)
```

### Studio-side modules (this repository)

| Module | Kind | Does |
|---|---|---|
| `studio_core` | Rust library | The session: open project, documents, selection, history (undo = inverse changes), preview targets. No UI. Depends on `qml_edit`, `ljos_blocks`, the compiler (Studio links it on purpose, as Ljós's ARCHITECTURE.md goal 9 lets an application do). |
| `studio_blocks` | Rust, Ljós blocks | Studio's own functional blocks (`Session`, `Selection`, `Canvas`, `Palette`, `Inspector`, `History`, `Tokens`) publishing state to the shell and taking actions. |
| `studio_mcp` | Rust | Studio's MCP server on `ljos_mcp_core`: each tool calls a `studio_core` action. |
| `shell/` | QML | The default Studio shell and its skins; users can replace it. |
| `studio` | binary | Native entry: `studio [project]`, `studio --mcp` (stdio, headless). |
| `web/` | ljos-web entry | The same shell and blocks compiled for the browser. |

### Seams and their adapters

| Seam | Interface | Adapters |
|---|---|---|
| **Edit** | `qml_edit::apply(text, Edit) -> Applied{text, changes}`, and `Applied::inverse(&self, source) -> Vec<Change>` | One. Studio's `History` keeps the inverse changes for undo. |
| **Document store** | `trait Store { read, write, watch }` | Filesystem (native); browser storage / File System Access (web); in-memory (tests). |
| **Preview host** | `trait PreviewHost { open(spec), reload, alias, resize, screenshot, outline, read, write, logs, close }` (`studio_core::preview`) | Native: a child Ljós process with the live inspector (`ljos_dev`, `app_*` tools) and hot reload. Web: an ljos-web runtime in an iframe/worker. Test: in-process `TestWindow`. |
| **Block source** | `BlockRegistry::from_type_dump` | `ljostypes.json` + qmldir `block` lines on disk; a running app's `Engine::type_dump()`. |
| **Agent surface** | MCP tools (MCP.md) | stdio (`studio --mcp`), and the same tools on the running Studio's loopback endpoint, guarded as below. |
| **Canvas renderer** | Studio's `Canvas` block draws the preview's frames | Native: textures from the preview host. Web: the preview iframe is the canvas frame. |

**Guarding the loopback endpoint.** The running Studio's endpoint can
rewrite the user's QML (`mutate`), start a preview that runs project code
(`run`) and write files (`fs`), so it is guarded as a Ljós inspector is
(ARCHITECTURE.md section 19.7, through `ljos_mcp_core`):

- it binds to loopback only;
- every call needs a 256-bit bearer token made for each start of Studio
  and written to a 0600 file in Studio's state directory, compared in
  constant time;
- `Origin` and `Host` are checked, so a web page that can reach localhost
  is refused;
- capabilities are granted at launch (`inspect` by default) and enforced
  in the server;
- every path a tool takes, `export`'s output among them, is canonicalised
  and confined to the open project; a path outside it is refused, and so
  is one a link leads out of. `export` leaves a link out of the project,
  or to anything it leaves out itself (mocks, the sidecar, build output),
  out of the bundle, and refuses a build directory that is a link. It
  leaves out what is not a regular file (a pipe would hold it forever),
  replaces only a directory that is empty or holds a manifest that is a
  file saying `ljos-bundle`, and asks all of that again just before it
  replaces one, and again of each file as it copies it.

`studio --mcp` on stdio needs no token: its client started it.

DECISION: the preview runs **out of Studio's process** (a child Ljós app
natively, a sandboxed frame on the web). A design under edit runs user
code; a hang or crash there must not take Studio down, and the preview then
behaves exactly like the shipped app. The cost is a texture hop per frame,
which section 6 budgets.

## 3. General-purpose Ljós versus Studio-specific

| Lives in ljos-ui (general, no app names) | Lives in ljos-studio |
|---|---|
| `qml_edit`: lossless structured edits | Session, selection, history UX |
| `ljos_blocks`: block contracts and registry | Studio's own blocks and default shell |
| `Ljos.Blocks`: `Contribution`, `Slot`, `ShellLoader` | Canvas, rulers, snapping, frames/artboards |
| qmldir `block` line, `#[qml(block)]` | Palette, inspector and layers panels |
| Theme file watching and live tokens on `Ljos.Design` | Token editor UI |
| Mock modules through module aliases | Mock authoring helpers |
| Inspector tools `app_blocks`, `app_block_invoke`, `app_preview_alias` | Studio MCP server (composes the above) |
| Authoring tools `qml_edit_apply`, `blocks_list` | Breakpoint and target presets |

The rule: if a second, unrelated application (an IDE plugin, a site
builder, an agent with no UI) would need it to edit or preview Ljós QML, it
belongs in ljos-ui. Studio composes those pieces and adds a UI.

## 4. Data flow

**A canvas edit (drag a child into another frame):**

1. The `Canvas` skin sends `Session.move(node, newParent, index)`.
2. `studio_core` resolves `node` (an `id` or a path; see risks) to a
   `qml_edit::NodePath` in the document's current text and calls
   `qml_edit::apply`.
3. On `Ok(Applied)`: the document text is replaced, `Applied::inverse` is
   pushed onto `History`, the `Store` writes the file (debounced with the
   hot-reload watcher's quiet time, ARCHITECTURE section 12.1).
4. The preview host's hot reload picks up the file, keeps state by
   `ObjectKey` and shows the new tree in under a frame after compile.
5. The `Session` publishes the new document generation; the layers panel,
   inspector and code view (bound to it) update. No panel polls.
6. On `Err(WouldBreak | Unsupported)`: nothing is written, the action
   returns the error and the shell shows it beside the selection.

**A code edit** goes the other way: the code view writes text; `Session`
parses it; on success panels update from the new tree; while the text does
not parse, structural canvas edits are disabled and the last good preview
stays (hot reload already keeps the old generation on error).

**An agent edit** is step 1 replaced by an MCP tool call into the same
`Session` action, so people watching see the change on the canvas and can
undo it.

**Block state in a preview** comes from the real app core (live) or from a
mock module selected by alias (mocked); the shell's text is the same either
way.

## 5. Testing

| What | How | Oracle |
|---|---|---|
| Lossless edits | `qml_edit` golden tests + the corpus round-trip test (all QML in ljos-ui) | Byte identity outside changed ranges; `Applied::inverse` restores bytes; `qml_fmt::same_tree` for opposite moves |
| Block contracts | `ljos_blocks` unit tests from fixture type dumps | Hand-written expected contracts; mock/real surface comparison |
| Studio behaviour | Gherkin in `features/` (Ljós's step library through `ljos_bdd`), one feature per capability: open, select, insert, move, restyle, bind, tokens, preview, export | The QML file text after the scenario (exact), plus the preview's accessibility tree |
| MCP surface | Golden JSON of every tool schema; in-process protocol tests; the scenario steps driven through MCP | Same as above: the file text and the preview |
| Preview fidelity | The preview of a shell compared with the shipped app run with `desktop:cua` / headless browser | qml6 for Qt-compatible QML (Ljós's existing oracle); the shipped Ljós app for Ljós-only modules |
| Studio's own UI | Off-screen drives (`desktop:cua`, headless Playwright for web), never the user's screen | Accessibility tree and screenshots |

DECISION: Studio's scenarios assert **file text**, not pixels: the file is
the product, and text is exact where screenshots are fuzzy. Pixels are
checked only for the preview, where qml6 is the oracle.

## 6. Performance budget

Studio must feel like a native design tool. Targets on the reference desktop:

| Path | Budget |
|---|---|
| `qml_edit::apply` on a 1 000-line file | ≤ 2 ms (the spike's corpus of 581 files × ~13 edits runs in ~3.5 s in debug, unoptimised) |
| Canvas edit → file written → preview updated | ≤ 50 ms p95 (hot reload is 17–31 ms today, section 12.1's drive) |
| Selection change → inspector shown | ≤ 1 frame (16 ms) |
| Drag on the canvas | 60 fps with the drag drawn by Studio; the edit is applied on drop, not per frame |
| Studio idle | 0 % CPU (rule 6): nothing polls; the preview host is woken by edits |
| Studio cold start with a 200-file project | ≤ 1 s to an interactive canvas |
| Studio memory | ≤ 150 MiB beyond the preview process |
| Web build | Studio's wasm + compiler lazily loaded; first canvas ≤ 2 s on a mid laptop |
| MCP | ARCHITECTURE section 19.6 caps (40 KB responses, ≤ 25 tools per server) |

## 7. Milestones

Each milestone ends with its exit criteria met and its scenarios green.

### S0: the seam (phase 1, done)

- Design doc, MCP list, block-model ADR.
- `qml_edit` spike in ljos-ui with golden and corpus tests.
- **Exit:** corpus round trips byte-identical except named, verified cases.
  Met: 7727 identical, 40 same-document moves, 29 refused, 0 failures.

### S1: headless Studio over MCP

- `ljos_blocks` registry in ljos-ui; `qml_edit` exposed as
  `qml_edit_apply` in `ljos mcp`.
- `studio_core` Session with Store (fs), History, Selection.
- `studio --mcp`: `project_open`, `document_tree`, `node_select`,
  `node_insert`, `node_move`, `node_remove`, `property_set`,
  `binding_set`, `blocks_list`, `history` (list, undo, redo).
- **Exit:** an agent builds a two-block shell from an empty project using
  only MCP; Gherkin scenarios assert the file text; undo returns the
  original bytes; tool schemas golden.

### S2: preview

- `PreviewHost` native adapter (child Ljós process, inspector, hot reload).
- Mock modules through aliases; `preview_open` (the target, desktop,
  tablet, phone or any size, is an argument, not a tool), `preview_state` (read
  or set mocked state), `preview_screenshot`, `preview_events` (MCP.md).
- **Exit:** a block edited through MCP shows in the preview within the
  latency budget; live and mocked state preview the same shell unchanged.

### S3: the canvas

- Studio's own shell: canvas of frames, layers panel, inspector, palette,
  code view, all bound to `studio_blocks`.
- Drag to move/insert, Layouts/anchors editing, selection sync with code.
- **Exit:** the S1 scenarios pass driven through the UI off-screen; idle
  CPU 0 %; budgets measured in CI.

#### As built: Studio's blocks and its shell

Studio's window is a Ljós app (`studio [project]`; `studio --mcp` stays
headless). Its shell is QML under `shell/`, and it binds one block,
`Studio.Workspace` (`studio_blocks`), as any shell binds an app's blocks:

- **State it publishes** (read-only properties, each a plain JS value so a
  shell needs no Rust model types): `documents`, `doc`, `generation`,
  `text`, `layers` (one row per object: `node`, `type`, `id`, `depth`,
  `label`), `selected`, `selection` (`start`, `end` in characters of
  `text`), `inspector` (the selected object's bindings: `name`, `value`),
  `states` (the selected object's states: `name`, `when`, `changes`),
  `palette` (the project's blocks: `module`, `name`, `summary`,
  `category`, `slots`), `done` and `undone` (history labels), `preview`
  (`target`, `picture`, `width`, `height`, `generation`, `variant`: the
  root's state the preview's width turns on), `error`.
- **Its one action**, `dispatch(tool, args)`, takes a Studio tool's name
  and its arguments. A gesture *is* a tool call: the shell has no path to
  the session that an agent lacks, which is how the UI scenarios
  (`features/ui/`) prove the same flows as the MCP ones with the same
  oracle, the file text.
- **Its events**: `answered(tool, answer)` and `refused(tool, error)`.

DECISION: one block with a `dispatch` action, not one native type per
panel. Panels are skins; splitting the state per panel would put layout
into Rust. DECISION: Studio's shell uses `node_select` like an agent, so a
selection made by either shows in the other.

The block republishes after every write, whoever made it: the session's
write callback posts to the UI thread through `UiHandle` (rule 7), so an
agent attached over MCP while a person works moves the panels too, and a
window nobody touches does no work (rule 6).

### S4: shells, skins and tokens

- Theme tokens editor over `Ljos.Design` + theme files; `ShellLoader` and
  user shells; slots and contributions in the palette.
- States/transitions as variants; breakpoints as width bindings.
- **Exit:** Studio reskins itself (its own shell edited in Studio) and a
  user shell with an error falls back with the error shown.

#### As built: variants, breakpoints and slots

This half of S4 makes a component's variants and a block's slots edits of
the file, made the same way by an agent (`state_edit`, `slot_contribute`,
MCP.md) and by a person in the window. The tokens editor, `ShellLoader`
and the self-reskin are the other half.

- **States are the variants.** A variant is a `State` in the object's
  `states`, with `PropertyChanges` for the values it changes and a `when`
  that turns it on; a `Transition` animates between them. There is no
  variant model beside the file: `studio_core::states` reads the lists
  from the parse and edits them in place, keeping every byte it does not
  change, and every edit is one undo step.
- **A breakpoint is a state's `when`.** `breakpoint: "phone"` writes
  `root.width < 600`; tablets are 600 up to 1024, desktops from 1024, so
  each preview target turns on its own state. A person sees the condition
  in the code view and may change it.
- **A slot contribution is a `Contribution` in the shell.** It names its
  slot, joins the slot by `mode` and `order`, and holds its delegate in a
  `Component`; the block is not changed (ADR 0003, section 4). Slots are
  checked against the block registry.
- **The window.** The inspector's *States* section lists the selected
  object's states as toggle buttons ("State compact"), with a field to
  rename each ("Name of compact") and a button to remove it; "New state"
  and "Add state" add one. Picking a state shows its "When" field, the
  buttons "Breakpoint phone", "Breakpoint tablet" and "Breakpoint
  desktop", a field for each value it changes (named `label.text`, empty
  to remove), and "Change target", "Change property", "Change value" and
  "Set change" for a new one. The canvas's frame says which state its
  width shows ("State compact" or "No state"). The palette lists each
  block's slots ("Slot controls of Player"), and when an instance of that
  block is selected, a field and a "Contribute to controls" button for
  each. Every control has an accessible role and name, and the scenarios
  (`features/ui/variants.feature`) find them by those.

DECISION: the frame names the root's state whose `when` is exactly a
breakpoint's condition for the frame's width. Studio does not evaluate
other conditions; the preview does, and shows what they turn on.
DECISION: a value typed in the inspector for a state goes in as written
(as `binding_set` takes it), so `"narrow"` with its quotes is a string and
`parent.width / 2` an expression.

Measured in a release build on the reference desktop:

| Path | p50 | p95 |
|---|---|---|
| `state_edit` on a 200-object document, 50 edits of each kind in turn (`studio_mcp` example `state_perf`) | 1.8 ms | 2.0 ms |
| A state's value edited in the inspector → new picture, one edit per 250 ms (`studio` example `budgets`) | 38 ms | 376 ms |
| For comparison, the same run's plain value edit → new picture, one per 250 ms | 22 ms | 627 ms |

Both picture paths missed section 6's 50 ms p95 in their tail, and the
edit is not the cause (the first row). The inspector's call rate was the
first cause: it answered 20 calls a second and had a refused call wait out
a one-second `Retry-After`, so the preview now grants Studio 200
(`ljos run --mcp-rate`).

The tail that is left is the file system's. `budgets` prints where the time
of each round's slowest edits went: the text written beside the document,
the rename into place (both read off the file's own times), the reload
asked and how it answered, the picture asked and fetched, the window at
rest and the frame, with the preview's own account of each reload from its
log. Under other lanes' builds on the same drive, one run of 150 edits of
each kind (p50 / p95 / max):

| Path | p50 | p95 | max |
|---|---|---|---|
| Inspector edit → new picture, back to back | 17.8 ms | 20.5 ms | 115 ms |
| The same, one per 250 ms | 19.6 ms | 71.8 ms | 524 ms |
| A state's value, one per 250 ms | 15.2 ms | 18.9 ms | 85 ms |

In every slow edit the time is in the rename: 506 ms of the 524 ms edit,
292 of 311, 252 of 282, 94 of 115, while the preview's reload took 1–2 ms
from asked to answered (its compile and apply about 1 ms) and the
picture's fetch 3–5 ms. Timed alone on that drive, each call of the same
save separately, `rename` over the document took up to 1 040 ms while the
`open` that creates the file beside it, the `write` and the `close` stayed
under 0.2 ms. The create needs the file system's journal as the rename
does, so it is not waiting on the journal: ext4 (mounted without
`noauto_da_alloc`) sends a replacing file's data towards the disk before
the rename replaces the old one, and that write waits behind the other
writing to the drive. On an idle drive the save is under 2 ms and no
edit of 600 went past 24 ms.

DECISION: Studio keeps its save written beside and renamed over. Writing
the document in place would skip ext4's flush, but a crash in the middle
would leave neither the old text nor the new, and the preview's watcher
would see the file half written. The tail is the price of a save that
survives a crash, paid on a busy drive only. Taking it off the edit's way
to the preview would need the preview to be given the new text by its
inspector as well as reading it from the file; Ljós's watcher can take a
save as text already, but no inspector tool hands it one.

#### As built: tokens, shells and the self-reskin

This half of S4 lets a person restyle any project, Studio included, from
inside Studio, and lets them replace Studio's whole shell with one of
their own.

- **Tokens.** `theme_set` (MCP.md, "Tokens") reads and sets the roles of
  `Ljos.Design` in a project's `theme.json`. The `Workspace` block
  publishes `theme` (the file of the document shown) and `tokens` (one
  row per role: `token`, `kind`, the module's value and the file's, and
  for a colour `light` and `dark`). The *Tokens* panel
  (`shell/Builtin/TokensPanel.qml`) lists them under a filter, a colour
  with a swatch and a field for light and for dark ("Colors.canvas
  light"), any other role with one field. Enter in a field is
  `dispatch("theme_set", …)`; an empty field gives the role back to the
  module. A refusal shows in the panel, with Ljós's own message (a
  contrast that fails names the pair and the ratio).
- **The window is a `ShellLoader`.** `shell/Window.qml` loads the
  person's `Shell.qml` (`userShell`, set by the host), with
  `shell/Builtin/Shell.qml`, the shell Studio ships, as its fallback. A
  banner above it (an accessible alert named "Shell error") shows the
  loader's `error` (`file:line: message`) when the person's shell
  failed and the built-in one is shown, or else why their last edit did
  not apply.
- **Where the person's shell is.** `studio --shell DIR`, else
  `ljos-studio/shell` in the configuration directory (`XDG_CONFIG_HOME`,
  else `~/.config`) when it holds a `Shell.qml`. A directory without one
  is given a copy of Studio's (`studio_blocks::shell::seed`), the place a
  person's shell starts. With no shell of the person's, the window is
  the built-in shell compiled ahead of time, as before.
- **Studio's own theme.** `--theme FILE`, else `ljos-studio/theme.json`
  in the configuration directory: the same kind of file `theme_set`
  writes, so a person restyles Studio by opening its configuration
  directory as a project.
- **The self-reskin.** With a shell of the person's, the window is a
  watched session, as any app under `ljos run --watch`: its roots are
  the person's directory and Studio's own shell, written to the state
  directory (`studio_blocks::shell::write`); its entry is `Window`; its
  theme file is Studio's. A save of the theme puts the new theme on the
  running window; a save of the shell reloads it in place, keeping what
  it can. The reload hook tells the window how the save went
  (`studio_blocks::reloaded`, which `Studio.ShellStatus` publishes as
  `reloadError`).

DECISION: Studio's window is the shell's `ShellLoader`, not a Rust
fallback: the rule for when a shell gives way is Ljós's (ADR 0003), and
the banner is a part of the window anyone may restyle.
DECISION: the watched session is used only for a person's shell. Studio's
own shell stays compiled ahead of time when there is none, so the
default start pays no compile; with one, both are compiled from the disk
so a reload sees them together.
DECISION: a shell made from a copy of Studio's, not an empty directory:
a person changes a shell that works, and a panel they do not touch is
Studio's as it was copied.
DECISION: why a save did not apply comes to QML from the host
(`Studio.ShellStatus`). A shell that is still running cannot know its
own reload failed, and Ljós does not yet tell QML (asked of Ljós below).
DECISION: Studio's own theme is put on once at start when there is no
person's shell (`ljos_design_qml::theme::apply`); it is live only in
the watched window, where Ljós already watches a theme file.
DECISION: an edit of a running shell keeps what it made: a reload that
does not compile keeps the shell and says why on the banner, and one
that compiles is applied in place, so a `Component.onCompleted` it adds
does not run on objects already made. The built-in shell takes over when
the person's shell fails while it is made, as when the window opens on
it; mending the file brings theirs back.

The scenarios (`features/ui/reskin.feature`) run the watched window
in-process over the test window: a token edit in the panel repaints the
canvas, a QML edit renames a pane, a shell that throws when the window
opens falls back with the banner and comes back when mended, an edit that
does not compile keeps the shell with the banner, and a watched window
nobody touches runs no frames.

Measured in a release build, off-screen (`studio` example `budgets`,
20 rounds, one edit per 250 ms; the preview is a release `ljos`):

| Path | p50 | p95 |
|---|---|---|
| `theme_set` in the tokens panel → the preview's new picture drawn | 14.2 ms | 17.0 ms |
| Studio's theme file saved → the window repainted in it | 15.6 ms | 15.8 ms |
| The window opened on a shell that throws → the built-in shell shown | 47.4 ms | 50.8 ms |
| For comparison, cold start on the built-in shell (5 rounds) | 44.7 ms | 52.0 ms |

The watched window left alone for 5 s used 0 ms of CPU, its watcher woke
nothing, and it drew no frames. The theme save's time is almost all the
watcher's quiet period (15 ms), which lets a save that is still being
written finish. A first run on a machine under heavy load (load average
about 35) measured `theme_set` → picture at 157 ms p50 and 422 ms p95;
the quiet run above is the one the budget is held to.

Asked of Ljós, each general: a reload's outcome readable from QML (a
`ShellLoader`'s `reloadError`, say), so no app needs its own
`ShellStatus`; an app's theme file on `ljos::App` outside watch mode,
so no app calls `ljos_design_qml` itself; `DevSession` on the facade, for
in-process tests of a watched window; a driver whose `aim_at_node` aims
at the part of a node its clips leave showing; and a watched window that
runs the shell compiled ahead of time until the first save, so a person's
shell does not cost a compile at every start.

### S5: web

- Studio on ljos-web; the web `PreviewHost`; `Store` on browser storage.
- **Exit:** the S1–S4 scenarios pass in a headless browser.

#### As built (first half)

The session, its tools and the shell run in a browser; the second half
(previews drawn by ljos-web, the S1–S4 scenarios in a browser) waits on
the ljos-web requests below.

- **wasm32.** `studio_core` builds without its `fs` feature (no `FsStore`,
  no `notify`), and `studio_mcp` without `native` (no rmcp server, no
  process previews, no wall-clock timer). Time comes through the `Timer`
  seam (`studio_mcp::timer`), so the reload budget runs on `setTimeout` in
  a page and on threads natively. Previews are made by a
  `StoreHostFactory` (`Studio::with_preview_hosts`), not by spawning.
  `mise run check:wasm` is clippy `-D warnings` for `wasm32-unknown-unknown`.
  `ljos_mcp_core` is linked without its `server` feature there (`native`
  turns it on), so the wasm has neither rmcp nor tokio and needs no
  `tokio_unstable`; `check:wasm` fails if either is linked.
- **`studio_web`** (crate): `WebStore`, the preview protocol, the
  `WebPreviewHost`, and `Face`, the JSON surface a page calls; `wasm.rs` is
  the only `wasm-bindgen` code, so everything else is tested natively.
- **`WebStore`** holds the project in memory, loaded whole on open, and
  queues writes for storage (OPFS, else IndexedDB, else memory:
  `web/store.js`). The page drains the queue after each call and answers
  the call only once storage has its files. Against `FsStore`: both
  replace whole files and neither leaves half of one; but a write reaches
  storage after `write` returns, so a page closed in between loses it, and
  a write storage refuses refuses its call after the edit is in the
  session (`details.kind = "not_stored"`, which files, and the files are
  queued again as the session holds them then: two calls store alongside,
  so the refused text may be older than one already stored).
- **Preview protocol**: `{"ljos-preview": 1, id, request | answer}` with
  `open`, `reload` (carrying the files), `alias`, `resize`, `screenshot`,
  `outline`, `read`, `write`, `logs`, `pick`, `close`: the `PreviewHost`
  surface, one request each. The session's `PreviewHost` is synchronous, a page is not:
  `web/worker.js` runs the session in a worker and blocks on
  `Atomics.wait` over a `SharedArrayBuffer` while the page answers
  (`web/preview.js`, `web/page.js`). That needs cross-origin isolation
  (COOP/COEP). The worker waits for the id of its own request in the
  buffer, so a late answer to one that timed out is never the next one's
  (`web/bridge.mjs` sets that race up in node). Tested against a fake page
  natively (`host.rs`) and in Chromium (`web/browser.mjs`).
- **JS face** (`web/studio.js`): every Studio a page opens shares one
  wasm instance; each keeps its own `ask` and watchers (`web/cores.mjs`).
  `close()` stores what is left and lets go of them and of the session (the
  page's `pagehide` does it, and whoever replaces a Studio): the instance
  holds a watcher that holds the Studio, so no finalizer ever would.
  `openStudio({backend, caps, ask})` gives
  `call(tool, args)`, answered as an MCP client's call is, plus `tools()`,
  `view(doc)`, `watch(stale)`, `text(doc)` and `replaceText` (offsets in
  UTF-16 units, as JavaScript counts). `web/features.mjs` runs the
  `features/` scenarios that need no preview and no window against it in
  node (`mise run features:web`); `@web` scenarios
  (`features/web/storage.feature`) run only there, and `@disk` ones
  (`features/project/export.feature`: a bundle is written to a directory,
  and the browser's storage is not one) only natively. It reads Scenario
  Outlines and data tables as cucumber does (one scenario per Examples
  row, `<name>`s written into step text, doc strings and tables) and
  refuses any Gherkin it does not read. Its steps match by kind (`Given`,
  `When`, `Then`) as cucumber's do, and it reads the native runner's
  `#[given]`/`#[when]`/`#[then]` patterns from
  `crates/studio_mcp/tests/features.rs`: a step one library has and the
  other lacks fails the run unless it is listed as the other's own (the
  preview's two steps native, browser storage's two web).
  A file a scenario gives after the tools connected reaches the session
  through `Studio.stored(path, text)` (`WebStore::stored`): storage got it
  from elsewhere, as another tab's write would be, and reads see it as
  `FsStore`'s see a file written into the directory.
- **The web preview's rate**: `WebPreviewHost` says it answers 150 calls a
  second (`studio_web::host::CALLS_PER_SECOND`), so a burst reloads a page
  60 times a second, once a display frame at 60 Hz. The bridge has no
  limiter to stay under; the number is Studio's own pace. It is above
  Ljós's inspector default (20, 8 reloads a second) and below the native
  preview's 200 because a page's preview draws on the shell's own thread,
  where a reload no frame shows costs the shell a frame.
- **The shell on ljos-web** (item 5, below): `Studio.Workspace` has a
  hand-written face (`web/Workspace.js`) that publishes
  `studio_mcp::view`, the projection the native block publishes, read
  through `Face::view`, so the two cannot say different things. The
  compiler sees QML stand-ins (`web/standins/Studio/`), checked against
  `crates/studio_blocks/ljostypes.json` by `studio_web`'s `standins` test.
  `mise run web:shell` compiles the built-in shell (`shell/Builtin/*.qml`)
  in a window of the page's own (`web/shell/Window.qml`) with ljos-web's
  `qmlc`, bundles it with ljos-web's vite plugin and drives it in headless
  Chromium (`web/shell.mjs`, `features/web/shell.feature`): the S3
  gestures, and a token set from the Tokens panel (9 of 9 checks).
  `features:web` runs `features/theme.feature` too: `theme_set` reads and
  writes the theme file in wasm with Ljós's reader and checker (83
  scenarios, 5 not run here).

DECISION: a page shows the built-in shell. The native window loads a
person's shell through `Ljos.Blocks`' `ShellLoader` (S4), which ljos-web
does not have; a request to ljos-web, not a second loader in JavaScript.
DECISION: QML stand-ins plus a hand-written face, the interim of
ljos-web's LJOS_WEB.md §3, until its generated `web` backend exists.
DECISION: the face reads `studio_mcp::view` through wasm rather than
restating it in JavaScript. DECISION: the shell's Workspace runs on the
page's thread without previews; previews need the worker bridge and a
picture ljos-web cannot take yet. DECISION: `Ljos.Design` is compiled from
its real QML (`--with`), not stood in for. DECISION: `web:shell` assembles
the app in the target dir with `node_modules` linked to an ljos-web
checkout; nothing is written into ljos-web. DECISION: `wasm-bindgen` is
pinned at 0.2.129, ahead of ljos-web's 0.2.114 (since bf49026 only an
example's; see the second half). DECISION: the browser storage order is
OPFS, then IndexedDB, then memory; a call resolves after storage.

**Compiling `shell/*.qml` with `qmlc`** (ljos-web 55ea53b, its
`types.txt`):

| Inputs | Result |
|---|---|
| stand-ins + `Ljos.Design` | 9 of 9 compile, no diagnostics |
| no Studio stand-ins | 7 of 9: ``Main.qml:18:5: `Workspace` is not a type of anything the file imports``, ``CanvasPanel.qml:66:25: `PreviewPicture` …`` |
| no `Ljos.Design` | 9 of 9 "compile", but import `{Colors, Spacing}` from `ljos-web/Ljos/Design`, which does not exist: fails at bundle time |
| `Ljos.Design` alone | 8 of 8 compile |
| `Workspace { nosuch: 3 }`, `Rectangle { colour: "red" }` | compile: members are not checked |

In Chromium the compiled shell lists documents, opens one, lists its
layers, selects one, sets `width` from the inspector, shows no refusal
and undoes, all through `dispatch`, with nothing logged (7 of 7 checks).
What it showed that the desktop does not:

- The window ignores its declared 1440×900 and fills its host element.
- Under ljos-web's Basic style the Inspector's new-property value field
  is 0 px wide (two 100 px buttons in a 300 px panel) and Layers'
  "Remove" is clipped: the shell's fixed panel widths assume Ljós
  metrics. A shell finding, left for S4's skins.
- Buttons get `aria-label` but no `role`; lists get `role=list`.
- `PreviewPicture` has no face (a visual native type, ljos-web W4), so
  `preview.picture` is always 0.

**Numbers** (this machine, release, `profile.web`):

| What | Value |
|---|---|
| `studio_web` wasm | 2,430,949 B raw, 527,275 B gzip |
| shell page JS (308 modules) | 773,618 B raw, 246,248 B gzip |
| `property_set`, 200 objects, node | p50 1.7 ms, p95 2.9 ms |
| same, OPFS in Chromium | p50 2.33 ms, p95 3.83 ms |
| OPFS write alone | p50 0.63 ms, p95 0.85 ms |
| memory store write | ~0.002 ms |
| `preview_open` over the bridge | 3.4 ms |

**Requests to ljos-web** (with evidence in the S5 report):

1. A QML compiler in the browser: `document_open` of a new file, reload
   and alias all need one; today `qmlc` is native and AOT.
2. A screenshot of a mounted root, for `preview_screenshot` and
   `PreviewPicture`.
3. An outline (object tree) of a mounted root.
4. Read and write a property by `id`; ids are private to `$context`.
5. Source locations on objects, so `pick` can name file, line, column.
6. File and line on logged messages.
7. Runtime module aliasing (the preview's import of a document under
   edit).
8. Resizing a mounted root is already feasible; noted, not requested.
9. Member checks for stand-ins and runtime types: unknown properties
   compile.
10. An unresolved module import as a compile error, not an import from
    `ljos-web/<Module>`.
11. `qmlc --types ljostypes.json` (W1), to retire the stand-ins.
12. The generated `web` face backend (§2) and visual faces (W4, for
    `PreviewPicture`).
13. Controls without an installed Qt (precompiled styles), and finding
    Qt when only Qt 5's `qtpaths` is on `PATH` (`qtpaths: Unknown option
    'query'`).
14. ARIA roles for controls (`button`, `textbox`).
15. Perhaps a Controls style with Ljós metrics on the web.
16. `Ljos.Blocks` (`Contribution`, `Slot`, `ShellLoader`), so a page can
    show a person's own shell as the native window does.

**Request to ljos-ui (done in 2582cba2):** gate rmcp and tokio in
`ljos_mcp_core` behind a feature, so a wasm build needs no `tokio_unstable`.

Left for the second half: requests 1–7 and 11–12, then previews drawn by
ljos-web behind the protocol, `PreviewPicture`, Playwright in CI, and the
S1–S4 scenarios in a headless browser.

#### As built (second half): previews through ljos-web

Against ljos-web main bf49026 (`web/ljos-web.rev`, the pin's one place).
In a browser the document under edit is compiled by ljos-web's `qmlc`
and mounted in the preview frame, which answers the preview protocol
(`web/page.js`) as the child `ljos run` answers natively.

**The Compiler** (`web/compiler.js`) is the one seam between a page and how
QML becomes JavaScript:

    compile({files, entry, aliases}) -> {component} | {diagnostics: [{file?, line?, column?, message}]}

`serverCompiler()` posts to the server that served the page (`serve.mjs`
`/compile`, which runs `qmlc` through ljos-web's Vite plugin, one
directory per compile, `--root` that directory) and imports what it
answers; qmlc's `path:line:col: message` lines become diagnostics.
`fakeCompiler()` is the tests' (`web/browser.mjs`). A compiler in the page
(request 1) is a drop-in: the page does not change.

What the page answers:

| Call | Answer |
|---|---|
| `open`, `reload` | compiled and mounted; a reload remounts (the app's state starts over, request 18); a reload that does not compile keeps the last good mount and answers `failed` with the diagnostics |
| `alias` | answered by the Compiler: the aliased module's files are the alias directory's (as `--alias`) |
| `resize` | the mount element's size, which the DOM gives honestly |
| `logs` | the frame's console lines, without file and line (request 6) |
| `screenshot` | `not_supported`, request 2 |
| `outline` | `not_supported`, request 3; the DOM's tree is not the QML object tree and is not given as one |
| `read`, `write` | `not_supported`, request 4 |
| `pick` | `not_supported`, request 5 |
| `click` (the host's, by id or point) | `not_supported`, request 4, without asking the page |

`Answer::NotSupported {message, ljos_web_request}` is the protocol's,
`PreviewError::NotSupported` the session's, and the agent sees
`not_actionable` with `details.kind: "not_supported"` and
`details.ljos_web_request` (MCP.md). The native host never gives it.

**Scenarios in Chromium** (`mise run features:browser`,
`web/features.browser.mjs`): every `@preview` scenario of
`features/preview/` runs with the session in a worker and its preview in a
frame that `serve.mjs` compiles, the same steps as `web/features.mjs`
plus the native runner's two preview steps (`the preview shows`, the PNG
step), which features.mjs lists as run there; a step of this runner the
native one lacks is a failure. 21 of 39 pass. The other 18 are in
`WAITING` with the request they wait on and the failure that says so
(a `not_supported` refusal naming it; for request 20 the drawn text; for
21 and 22 ljos-web's compile error for the type, or a frame that draws
nothing when that error comes after the open);
one that passes, or fails another way, fails the run:

- request 4: the six flow scenarios that read or click (`preview_state`),
  one of them a click at a point, a mocked block's state set from
  outside, a live block's state read;
- request 2: a picture of the preview;
- request 20: a token set shows in the preview, and its undo shows the
  module's value again. The undo's last step reads what a preview that
  never drew the token shows too, so its entry has a probe: once the
  token is set the preview must show it, and its absence is the wait.
- request 21: the three scenarios of a JSON list in the preview
  (`Ljos.Models`);
- request 22: four of the five scenarios of a project's type dumps
  (stand-ins from `ljostypes.json`; the mocked state's passes).

CI's `browser` job runs it after `web:serve:check`.

- DECISION: "the preview shows" is, in a browser, what the frame draws
  as text: an element whose own text is the text, as the native outline
  names a Text by its text. It is the test's eye; the agent is never given
  a DOM outline in place of the object tree.
- DECISION: two `qmldir`s declaring one module (`App/Media` and
  `mocks/App/Media`) are shown to qmlc as Qt resolves them: the one at the
  module's import path. qmlc takes either (request 17); without this, a
  live preview ran the mocks.
- DECISION: a Ljós block's `# block Name Version File` qmldir line is
  shown to qmlc as a type line too: Ljós declares the type by it, qmlc reads
  only Qt's lines (request 19). Both are `forQmlc` in `serve.mjs`, to go
  when ljos-web resolves modules as Ljós does.
- DECISION: in `web:serve` the preview frame is an element beside the
  shell's root, laid over the Canvas panel's box (`aria-label="Canvas"`)
  while the shell shows one, followed by a ResizeObserver and a
  MutationObserver (nothing polled), and resized with it; with no Canvas
  panel it is the column beside the shell. `PreviewPicture` has no web
  face: a native picture is a PNG the preview draws, and a frame draws
  itself (a picture would need request 2).
- DECISION: `web:serve`'s page still drives its frame itself (`open` after
  each write), now through the Compiler; the session's `preview_*` tools
  drive a frame through the worker bridge, which is what the scenarios run.
- DECISION (wasm-bindgen): neither side moves. At bf49026 ljos-web's
  workspace builds its kernel (`ljos_web_kernel`) with raw `u32` imports,
  no wasm-bindgen; only `examples/wasm-counter` pins `=0.2.114`. Studio
  links no ljos-web Rust crate, and its wasm is a module of its own with
  its own glue, so the two never share a wasm-bindgen runtime. Studio stays
  on 0.2.129; the risk below is retired.

**Numbers** (release `qmlc`, the `web` wasm profile, headless Chromium,
`node web/preview.bench.mjs <app> 20` after `mise run features:browser`
assembled the app; `samples/hello/Main.qml`):

| What | Measured |
|---|---|
| first preview after the project opens (worker, session, Qt Quick's first compile, mount), cold server | 472.8 ms |
| a reload (`property_set` answered after the remount), cold | 34.7 ms |
| `preview_open` of another document, warm | p50 28.7 ms, p95 33.1 ms (n 20) |
| a reload, warm | p50 29.1 ms, p95 31.5 ms (n 20) |
| Studio's wasm (`studio_web_bg.wasm`) | 3,257,162 B, 705,316 gzip |
| `web:serve`'s page JS (`dist/assets/index-*.js`) | 796,946 B, 252,606 gzip |

The frame's JS is Vite's dev server's, module by module; its size is not
a shipped size and is not given.

**Requests to ljos-web, at bf49026.** Nothing of 1–16 is done by ljos-web
since 55ea53b (the commits add the wasm property kernel only); Studio now
does 7 and 8 itself:

1. Open. A QML compiler in the browser. A9's live-shell compiler (tier-1
   IR, shell mode, no arbitrary JS) would not compile a general Studio
   document; request it general, or say which documents it takes.
2. Open. A screenshot of a mounted root.
3. Open. An outline (object tree) of a mounted root.
4. Open. Read and write a property by `id`, and a click by object or at
   a point (`app_click`'s `at`, folded in from phase 3 / M1).
5. Open. Source locations on objects (`pick`).
6. Open. File and line on logged messages.
7. Done on Studio's side (aliases are the Compiler's); not needed.
8. Done (resize through the DOM).
9. Open: `Rectangle { colour: "red" }` still compiles (exit 0).
10. Open: `import No.Such.Module` still compiles (exit 0).
11. Open: qmlc has no `--types`.
12. Open (W4 is not done).
13. Open.
14. Open: a compiled `Button` carries no role.
15. Open.
16. Open.
17. New. Module resolution: of two `qmldir`s declaring one module, qmlc
    takes either (here `mocks/App/Media` over `App/Media`). Resolve by
    import path, as Qt does, or refuse the ambiguity.
18. New. A reload of a mounted root that keeps its state (hot reload), as
    `ljos run --watch` keeps it; today a reload is a remount.
19. New. Read Ljós's `# block Name Version File` qmldir lines as type
    declarations (ADR 0003), or say Ljós's qmldir is not ljos-web's.
20. New. A theme file beside the entry (`theme.json`) applied to
    `Ljos.Design`'s tokens at mount and on change, as Ljós does.

21 (`Ljos.Models` in the web runtime) and 22 (stand-ins from a project's
type dumps) are phase 3 / M1's, under S6.

### S6: polish and later

- Prototyping flows (state graph view), CMS-style model binding, export
  bundles, design review comments in the sidecar; multiplayer research.

#### As built: prototyping flows and list data

- **A flow is the app's handlers.** A step of a prototype is what a tap
  does: go to a state of a component, or open a document in a `Loader` or
  on a `StackView`. Studio writes it as a signal handler on the object
  tapped (`onClicked` when the signal is named, otherwise `onTapped` in a
  `TapHandler`, which any item can hold), so the prototype is the app and
  runs anywhere QML runs. `state_edit` writes and takes out interactions;
  `node_inspect` lists them (MCP.md, "Flows and list data").
- **The state graph is read from the file.** The root's states are its
  nodes, the base state first; its transitions and its interactions are
  its edges, an interaction going from any state. There is no flow model
  beside the file.
- **The preview plays it.** `preview_state` clicks the running app, by an
  object's `id` or at a point, and reads the state the `node` it names
  reached; a click with no `node` plays and reads nothing, which is how
  the window plays a document whose root has no `id`. Setting a state
  needs the `node` it is set on.
- **Data is a model document.** A list's content is a model in its own
  document of a data module: a `JsonListModel` reading a JSON file of the
  project (phase 3), or a `ListModel` written as QML; a JSON file becomes
  either with `document_new`. The roles the data binds are offered in the
  inspector of a delegate as `model.<role>`. The mocked state swaps the
  module for one under `mocks/`, so a fixture is a file beside the other
  mocks. Only a view's delegate sees its roles, not its header or footer.
  A record's keys are its roles, so a key may not repeat within a record,
  and `id` is not a role: QML reads it as the object's id.
- **The window.** The *Flow* panel (`shell/FlowPanel.qml`) shows the
  states as nodes ("Flow state Base", "Flow state details"), each with a
  button that wires the selection to go to it ("Tap selection to go to
  details"), and the edges as rows ("Flow edge tapped card to details")
  with "Remove flow …" on interactions. The canvas's "Play" toggle turns a
  click in a frame into `preview_state` with `click_at`; the panel then
  marks the state the app is in ("Playing details"). The inspector's
  *Roles* section lists the roles the selection sees ("Role name"), each
  with a field ("Bind role name to") that binds a property to it.
  `features/ui/flows.feature` drives them.

DECISION: an edge's `from` is `*` for every interaction: a handler sets
its state from wherever the app is, and Studio does not evaluate which
states make the trigger visible. DECISION: the flow is the root's; an
interaction that sets another object's state is that object's and is not
drawn. DECISION: play mode does not select; leaving it, a click selects
again.
#### As built: export bundles and review comments

Two of S6's items are tools with their scenarios
(`features/project/export.feature`, `features/edit/review.feature`), and
review is in the window too (`features/ui/review.feature`). MCP.md lists
their arguments, answers and refusals.

- **An export is what `ljos run` starts.** `export` copies the project
  into `build/native`: its QML, its theme, its assets and its own blocks,
  never the mocks or `.studio/`, with a manifest (`ljos-bundle.json`) that
  says what it holds, what was left out and why, and how to start it. The
  scenario copies the bundle out of the project and runs it headless: its
  outline is the live preview's.
- **What cannot be exported is refused, saying why.** Types written in
  Rust (only the app's program registers them), a document that does not
  parse, and a module that only a mock provides each have their own
  refusal; nothing is written. A site for the web is the next section.
- **Comments sit on objects, in the sidecar.** `review` adds, answers,
  resolves and reopens comments, and lists them by document or object. A
  comment follows its object through anybody's edits as a handle does,
  and is written down with the object's key, so the next session finds it.
  Deleting `.studio/` loses the comments and nothing else.
- **A removed object orphans its comments.** They are kept with the
  place the object last had and can still be answered; undoing the
  removal gives them their object back, and so does redoing the creation
  of a document whose undo orphaned them.
- **The sidecar is never written over blindly.** Each review call reads
  `.studio/review.json` again when it changed on disk. A sidecar that
  does not read, or one a later Studio wrote (a higher `version`), is
  left alone and every call says so (`review_unreadable`) until it is
  mended or deleted. A sidecar changed by somebody else between Studio's
  read and its write is not overwritten: the call is refused with
  `review_changed` and the new file is what Studio holds from then on. A
  comment the sidecar did not take is not made. Re-reading merges: what
  the file says is kept, and a comment this session follows keeps its
  object, place and orphaned state, so an edit or an undo around the
  change still lands; that is then written down.
- **The window.** A layer with open comments says how many ("1 comment on
  Text title"). The inspector's *Comments* section lists the selected
  object's comments and the document's orphaned ones ("Open comment: Too
  small", "Resolved comment: …", "Orphaned comment: …"), each with a
  "Resolve …" or "Reopen …" button, and "New comment" with "Add comment"
  adds one to the selected object. The box keeps its text until the
  comment is taken, so a refused one is sent again rather than typed
  again. Every gesture is the `review` call an agent makes.

DECISION: the bundle is a directory, not Ljós's production bundle
(ARCHITECTURE.md 12.2) nor an AOT binary: both need a build Studio does
not make. Its manifest has no hashes or signature until Ljós defines them.
DECISION: a bundle is written below the project's `build/`, which the
store does not list, so it is never a document nor part of the next one.
DECISION: comments are not edits: they are not in the undo history, and
they change no document's generation. DECISION: while the sidecar does
not read, the window shows no comments rather than a refusal on every
repaint; the next review gesture shows the refusal. DECISION: a sidecar
changed on disk replaces Studio's comments as a whole, so comments made
in this session and not yet written are not merged into it; the refusal
says so. DECISION: export leaves a link out of the project out of the
bundle (named in `left_out`) rather than refusing the export, as it does
for the mocks; an entry that is left out is refused (`entry_left_out`).
DECISION: export works path by path, and checks again right before each
step that removes or reads, rather than through directory handles
(`openat`/`unlinkat`, or `cap-std`). Between the last check and the
removal a process of the same user could still swap `build/` for a link
and have a directory that holds a bundle's manifest removed. That process
can already remove the directory itself, so the window only matters for a
swap made by accident, such as a checkout. Directory handles would need a
dependency on each platform, and the browser build has no directory to
export. Handles are the next step if Studio ever exports for another user.
DECISION: export walks each real directory once, under its own name, and
never walks a link to a directory: links that fan out and meet again (a
diamond at each of 24 levels took 56 s and failed before) stay linear in
time and size. Every directory of the project is walked under its own
name or left out by the walk's rules, so a link to a directory leads to
one the bundle holds or to one it leaves out. The first goes into the
bundle as a relative link (the manifest's `links`), so an app that
imports a directory through the link's name (`import "alias"`) runs
from the bundle as from the project. Where the system makes no links (a
Windows user without the privilege), that directory is copied under the
link's name, with the links inside it copied the same way: each
`(path, to)` once, and a link back into a directory being copied around it
not followed again. A copy cannot share what a diamond of links shares, so
it doubles with every level; the links are made before any file is copied,
and where they cannot be, what their copies would write is counted first:
every link followed and every file placed, so a diamond of directories that
hold only links counts too. Past 10,000 entries the export is refused
(`unsupported`, `links_multiply`) with nothing written. The answer's `files` and `bytes` count the copies.
The second stays out with
its reason, as does a link to a directory it is in, which would make the
bundle hold itself. A file's check right before it is copied applies the walk's
exclusions again to where it now leads, so a file swapped for a link into
`.studio/`, the mocks or build output is not copied.

Measured in a release build on the reference desktop:

| Path | p50 | p95 |
|---|---|---|
| `export` of a project whose bundle holds 200 files (415 677 bytes), 20 exports (`studio_mcp` example `export_perf`, release) | 9.41 ms | 9.95 ms |
| the same since a document is parsed only, not outlined, to know it parses (M3) | 6.04 ms | 6.74 ms |

#### As built: a site for the web

`export` with `"target": "web"` writes `build/web`: the same walk of the
project, the same refusals, `left_out` and link rules, and a manifest for a
site builder instead of for `ljos run` (`features/project/export-web.feature`).
ljos-web compiles, bundles and runs; Studio chooses the files and refuses
what has no face on the web. `mise run export:web:build <bundle> [out]` builds
the site with an ljos-web checkout until ljos-web has a builder of its own,
and `mise run export:web:check` holds the result to
`features/web/site.feature` in headless Chromium.

**What ljos-web bf49026 can do** (evidence, M3): `qmlc --root DIR` compiles
QML to Solid components and finds a project's modules by their qmldirs below
`DIR` (without `--root` the root is the compiled file's directory, so a page
must pass it); its Vite plugin (`ljos-web/vite`) runs `qmlc` per import and
takes `args` and `standins`; a page imports `ljos-web/runtime.css` and calls
`mount(Entry, element)` from `ljos-web/object`. The runtime package's exports
(`packages/runtime/package.json`) are the modules a page can import.
`QtQuick.Controls` is compiled from an installed Qt's own QML; `Ljos.Design`
is plain QML and compiles from `ljos_design_qml/qml` as a stand-in. Built
from the fixture (`web/site/project`), the site is three files, Vite inlines
a small SVG into the script, the text takes Ljos.Design's colour, and the
mock never reaches the page. What it cannot do: build a site from a
directory on its own (request 30), apply `theme.json` (Studio's native
preview applies it through `ljos_design_qml::theme::apply`, which is Rust:
request 20), and run `Ljos.Blocks`, `Ljos.Models` or `Ljos.Observer`, which
are Rust types (requests 32, 33).

**The site contract.** `ljos-site.json`, at the bundle's root:

| Field | Promise |
|---|---|
| `format`, `version`, `target` | `"ljos-site"`, `1`, `"web"`; a builder refuses anything else |
| `entry` | the document the site mounts, a listed `.qml` |
| `theme` | the `theme.json` Ljós reads for the entry, the nearest from its directory up to the project's, when there is one (carried, not yet applied) |
| `files` | every file of the bundle with its size, and nothing else is in it; plain paths below the bundle, never `mocks/`, `.studio/` or `ljostypes.json`; no links, so a directory the project links to is listed again under the link's name |
| `modules` | each of the project's modules (`module`) and its `qmldir`, both listed, each module once, and every listed qmldir that declares a module among them; every component a qmldir names is listed |
| `imports` | every module a document imports that is not the project's, sorted: each one ljos-web at `ljos_web` provides |
| `blocks`, `left_out` | as in the native manifest, for a person reading it |
| `ljos_web` | the ljos-web commit the imports were checked against (`web/ljos-web.rev`) |

A builder that holds to it needs nothing but the bundle and ljos-web: no
Studio, no project around it. `crates/studio_mcp/tests/site/mod.rs` is such a
builder for tests: it reads the manifest by these words (unknown fields
refused), and `tests/site_contract.rs` tampers with an exported bundle every
way the contract rules out (an unlisted file, a missing one, other bytes, a
path out of the bundle, an import nobody provides or the manifest does not
say, a module without its qmldir, another version or target, Studio's files,
types in Rust, a commit that is not one, a link) and sees each refused.

**Refused for the web**, in the order the checks run, the native ones
among them:

1. The native `not_found`, for an entry that is no document of the
   project.
2. `unsupported` with `links_multiply`, right after the walk and before
   any document is read: links that lead into the same directories along
   so many ways that the site would copy more than 10,000 files and
   directories through them, even directories that hold only more links.
3. The native `entry_left_out`, `syntax` and `native_types` checks. For the
   web, `native_types` says that types written in Rust have no face there
   (request 33).
4. `unsupported` with `linked_module` (its `module`, its `qmldir` and the
   copy `through` the link): a link to a directory that declares a module.
   The site would hold the module twice, and its builder, which finds
   modules by their qmldirs, would pick one. Natively it stays a link.
5. The native `mocked_only`.
6. `unsupported` with `no_web_face`, its `modules` and the `docs` that
   import them: a module that is neither the project's own nor one ljos-web
   provides. That covers `Ljos.Blocks` shells, `Ljos.Models`, a module
   only an installed plugin gives, and a typo.

DECISION: the web export reuses the native one's walk and checks, and adds
to its manifest rather than forking it; `run` is native's alone, and
`modules`, `imports` and `ljos_web` are the web's alone. DECISION: the
manifest has its own name (`ljos-site.json`, format `ljos-site`), so a
native bundle and a site bundle sit side by side under `build/` and neither
is taken for the other; either may be replaced by the next export.
DECISION: what ljos-web provides is a list in Studio
(`export::WEB_MODULES`), its runtime package's exports at the pin plus
`QtQuick.Controls` and `Ljos.Design`; a test compares it with the package
when `LJOS_WEB` names a checkout, and the build task compares the manifest's
imports with the checkout it builds with. Request 34 would let Studio read
it instead. DECISION: a path import (`import "parts"`) is not checked: its
files are the project's and in the bundle, or outside it and missing from
the native bundle alike. DECISION: a site holds no links, as its
builder and the hosts it goes on keep none: a link to a directory the bundle
holds is a copy of that directory's files under the link's name, as a native
bundle has where the system makes no links, so `import "alias"` resolves on
the page too. Links that fan out and meet again would copy a directory once
per way down, which doubles with every level, so past 10,000 entries reached
through links (each link followed and each file placed) the export is
refused rather than written. DECISION: Studio's window has no export UI, so the
web target has none either. DECISION: Studio's interim builder is a task
(`export:web:build`) over ljos-web's own compiler and plugin, not code in a
crate, so it is gone when request 30 lands; it copies only the listed files,
asks for Qt only when the site imports the controls, and replaces only a
site it built (a `.ljos-site-build` stamp). DECISION: the pin moves to
bf49026, against which the module list and the site were checked; `mise run
web:shell` passes on it. DECISION: a document is parsed, not outlined, to
know it parses, which also gives its imports: one parse per document for
both targets.

Measured in a release build on the reference desktop:

| Path | p50 | p95 |
|---|---|---|
| `export` `web` of the same 200-file project (415 677 bytes of files, a 15 250-byte manifest), 20 exports | 5.90 ms | 6.01 ms |
| `export` `native` of it (a 14 941-byte manifest), same run | 6.04 ms | 6.74 ms |

The fixture's site (`mise run export:web:check`): 3 files, 443 160 bytes
(157 020 gzipped), almost all ljos-web's runtime and Solid; first
contentful paint 60–72 ms in headless Chromium, the entry's text shown by
70–87 ms, served from memory by Playwright.

**Requests to ljos-web** (M2's start at 17; these at 30):

30. A site builder: `ljos-web build-site <dir>` (or an API) that takes a
    directory with `ljos-site.json` and writes a static site, so Studio's
    interim task goes.
31. Is request 20 (`theme.json` applied to `Ljos.Design`, in a page as in
    a preview); today a site has the default palette. The number is kept
    so 32–36 keep theirs.
32. `Ljos.Blocks` on the web (request 16), so a shell over `ShellLoader`
    can be exported rather than refused.
33. Web faces for Ljós's Rust modules (`Ljos.Models`, `Ljos.Observer`) and
    for a project's `ljostypes.json` (requests 11, 12, 21, 22).
34. A machine-readable list of the modules a commit provides, including
    those it compiles from Qt or Ljós, so Studio need not keep one.
35. A `root` option on the Vite plugin, rather than `--root` through
    `args`, and a project root that does not depend on which file was
    compiled first.
36. Controls without an installed Qt (request 13): a site importing
    `QtQuick.Controls` cannot be built where Qt is not.

**Request to ljos-ui:** a way for `Ljos.Design` to take a `theme.json`
without Rust (its palette's seed and overrides as QML can be given them),
so a page and a native window read one theme the same way.

#### Phase 3 / M1: as built

Studio uses five pieces Ljós gained in 2582cba2, each with its scenarios.
MCP.md has the arguments, answers and refusals.

- **A JSON file stays JSON.** `document_new {doc, data}` makes a document
  that reads the file, `JsonListModel { source: Qt.resolvedUrl("…json") }`
  (relative to the new document); `model: "list"` makes the `ListModel` copy phase 2
  made. `node_inspect` reads a `JsonListModel`'s file when asked, so its
  `roles` are the records' keys in the order first seen and `rows` their
  count, with `source` naming the file; the inspector of a delegate offers
  them as it does a `ListModel`'s. A save of the file shows in the native
  preview without a reload of the QML, by whatever saved it, since
  `ljos run --watch` reads a watched `source` again; a mock module
  under `mocks/` reads a fixture beside it
  (`features/edit/models.feature`, `features/preview/json-models.feature`,
  `features/ui/flows.feature`).
- **A click lands at its point.** `preview_state`'s `click_at` and the
  canvas's Play mode call the preview's `app_click` with `at`, so whatever
  is on top there gets the click, as under a person's pointer; it used to
  click the middle of the topmost item drawn at the point
  (`features/preview/flows.feature`, `features/ui/flows.feature`).
- **Signals are the type's.** `node_inspect` lists an object's signals
  with their arguments, and the inspector's *Signals* section and the
  Flow panel's *Wire* list offer them; a native block's come from its
  dump. A flow on a signal the trigger lacks is refused, naming those it
  has (`features/edit/flows.feature`). Studio reads Ljós's own table of
  types (`qt_types`) for the toolkit's, the project's dumps and QML files
  for its own, in the order the document's imports name them.
  `studio_blocks`' `ljostypes.json` and the web stand-ins carry the
  workspace's `signals`, which the stand-ins test checks.
- **A project with types written in Rust previews.** The native preview
  starts `ljos run` with the project's type dumps (`--types`; several are
  merged in the state directory), so the types stand in with the members
  they declare (`features/preview/native-types.feature`). Export still
  refuses such a project: only the app's program registers them.
- **No tokio in the web build.** `ljos_mcp_core` is linked without its
  `server` feature in the wasm, so `.cargo/config.toml` no longer sets
  `tokio_unstable`, and `check:wasm` fails if rmcp or tokio is linked.

DECISION: JSON is the default and the `ListModel` copy the choice, because
a JSON file has an owner outside Studio and a copy forks it (MCP.md).
DECISION: a type Studio finds described nowhere has no signals listed and
none refused; Ljós names a missing one when it compiles. DECISION: the
signals offered leave out properties' change signals, which `node_inspect`
lists marked `change`. DECISION: a dump is read when the preview starts,
as `ljos run` reads it once; an edit of the dump shows when the preview is
opened again. DECISION: in the mocked state a mock module stands in for a
module a dump describes, as for one of QML. DECISION: the Flow panel
offers the signals in a `ComboBox`, not a row of buttons (a `MouseArea`
has ten, which pushed the graph out of the panel); the pick is kept by
name, so it holds across an edit while the selection still has the
signal, and an edit that changes the selection's signals with the box
open leaves the panel answering (`features/ui/flows.feature`).
DECISION: `document_new` writes the source as `Qt.resolvedUrl("…")`,
which resolves against the file it is written in wherever the model is
made. A plain relative `source` follows Ljós's rule (ljos-ui 9805b6ad,
as qml6 reads it): on a document's root object it resolves against the
file that uses the document, on any other object against its own file.
Studio reads a plain one on a root through the list that uses the
document, against that list's file; inspected alone it names no file,
since each user may read another (`features/edit/models.feature`). A
`source` an instance sets in the using file is not read.

Measured in a release build of Studio on the reference desktop, which
other builds kept at a load average near 7.5; only a debug `ljos` was
available to this lane, so the preview's side is a debug build's, and a
release `ljos` will read and lay out the file faster:

| Path | p50 | p95 |
|---|---|---|
| A 1,000-record JSON file (50,779 bytes) saved → the preview's outline shows the edit, 30 rounds (`LJOS=… cargo run --release -p studio_preview --example json_edit`) | 32.3 ms | 114.1 ms |
| `node_inspect` of a list reading 1,000 records (74,781 bytes), 200 calls (`cargo run --release -p studio_mcp --example inspect_json`) | 0.39 ms | 0.44 ms |

The JSON save's time includes the watcher noticing the file, `ljos run`
reading and parsing it, the list's delegates updating and the next
outline call; the outline is asked every 5 ms, so a sample is late by at
most that. MCP.md has each changed tool's time.

The wasm (`web/pkg/studio_web_bg.wasm`, `mise run web:build`, `web`
profile) is 3,650,408 bytes, 772,022 gzipped (`gzip -9`). It was
3,245,818 (697,691) before phase 3; without tokio and rmcp it is
3,224,088 (694,368), and Ljós's table of types (`qt_types`), which the
browser's `node_inspect` reads a toolkit type's signals from, adds
426,320 (77,654). DECISION: the table is linked in the web build too, so
an agent and a person see the same signals in either; a smaller table
for the browser is a Ljós request (the signals alone, not every member).

**Requests to ljos-web** (numbered in the list S5 began, at bf49026):

21. `JsonListModel` (`Ljos.Models`) in the web runtime, reading its
    `source` and again when the file changes, so a browser preview of a
    list that reads a JSON file shows its rows; bf49026 has no
    `Ljos.Models`.
22. Stand-ins made from the project's type dumps (`ljostypes.json`) in the
    web preview, as `ljos run --types` makes them natively, so a project
    with types written in Rust previews in a browser too.

A click at a point (`app_click`'s `at`) is folded into request 4, which
becomes read, write and click: by object or at a point.

#### As built: running it

`mise run studio` is the window; `mise run web:serve` is the same shell in a
browser, for a person. ljos-web compiles QML only outside a page, so
`web/serve.mjs` compiles and serves the preview, bound to the loopback
interface and refusing another origin: a page elsewhere must not be able to
reach it. The project stays in browser storage (OPFS, with the names in
`localStorage`). The preview pane speaks the same protocol as `web/page.js`.
What each task finds and what it says when something is missing is pinned by
`mise run check:tasks`; the pin of ljos-web is `web/ljos-web.rev`.

## 8. Risks

- **Addressing across edits.** Child-index paths shift. Studio gives an
  `id` only to an object it inserts itself (one not yet used in the
  document); it never adds an `id` to an object the author wrote, since
  that writes bytes nobody asked for (goal 2). Existing objects are
  followed by handle, `id` or `ObjectKey`. A path is true of one
  generation, so an edit that names an object by path must give that
  generation and is refused when the document has moved on, rather than
  applied to the wrong object.
- **Partial files.** A code view produces broken files; structural edits
  wait for a parse, which a person may find surprising.
- **Members sharing a line with a brace** cannot yet take a multi-line
  insert (29 corpus cases).
- **Preview fidelity versus speed.** An out-of-process preview costs a
  frame hop; if that misses the drag budget, the drag is drawn by Studio.
- **Purity of skins** is a convention plus a heuristic lint; authors can
  still put state in a skin.
- **Ljós dependencies.** S1 needs the Ljós lead to integrate `qml_edit`
  and build `ljos_blocks`; S5 depends on the ljos-web alignment track
  (its requests are listed under S5).
- **Web.** The wasm is 2.4 MB; previews need COOP/COEP; a page closed
  before storage has a write loses it; a browser preview needs a server's
  `qmlc` until ljos-web compiles in the page, remounts on reload, and
  `forQmlc` stands in for module resolution ljos-web lacks; `web:shell` needs an installed Qt's QML and an ljos-web
  checkout.
