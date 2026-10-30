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
  depends on it.
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
  and confined to the open project; a path outside it is refused.

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
  `palette` (the project's blocks: `module`, `name`, `summary`,
  `category`), `done` and `undone` (history labels), `preview` (`target`,
  `picture`, `width`, `height`, `generation`), `error`.
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

### S5: web

- Studio on ljos-web; the web `PreviewHost`; `Store` on browser storage.
- **Exit:** the S1–S4 scenarios pass in a headless browser.

### S6: polish and later

- Prototyping flows (state graph view), CMS-style model binding, export
  bundles, design review comments in the sidecar; multiplayer research.

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
  and build `ljos_blocks`; S5 depends on the ljos-web alignment track.
