# Studio's MCP tool surface

Every Studio capability is one of these tools before or alongside its
panel. People and agents act through the same `studio_core` actions, so an
agent's change shows on the canvas and is undone like a person's.

Studio composes three servers rather than duplicating them:

- **`ljos mcp`** (Ljós's authoring server, `ljos_mcp_author`): checking,
  linting, formatting, types, docs, scenarios. Studio forwards to it and
  adds `qml_edit_apply` and `blocks_list` *to Ljós* (general tools).
  Studio's own `blocks_list` is the same registry, built by the
  same `ljos_blocks` code, for the project Studio has open: an agent
  talking only to Studio should not need a second server to see the
  blocks it composes. The two answer alike; neither adds to the other.
- **The live inspector** (`ljos_dev`, `app_*` tools) of the preview
  process: snapshot, inspect, click, screenshot, reload.
- **Studio's own server** (`studio_mcp`), below: project, document, canvas
  and preview state that only a designer has.

Budgets follow ARCHITECTURE.md section 19.6: tools listed in a fixed order,
descriptions ≤ 300 characters, responses ≤ 40 KB with cursors, ≤ 25 tools
on one server. Studio's list is 24. Capabilities as in section 19.7:
`inspect` (read), `mutate` (edit documents), `fs` (write files outside
documents: export), `run` (start previews).

## Studio server (`studio --mcp`)

| # | Tool | Does | Needs |
|---|---|---|---|
| 1 | `project_open` | Open a project directory; returns documents, blocks count, targets | inspect |
| 2 | `project_info` | Documents, modules, shells, themes, mock modules, preview targets | inspect |
| 3 | `document_tree` | A document's object tree: path, `id`, type, bindings summary; paginated | inspect |
| 4 | `document_text` | A document's text, or a range of it, with the current generation | inspect |
| 5 | `document_new` | Create a QML document; undo removes it | mutate |
| 6 | `document_edit` | Replace a byte range of a document's text, as the code view does | mutate |
| 7 | `node_find` | Find nodes by `id`, type, property value or text; returns addresses | inspect |
| 8 | `node_select` | Set Studio's selection (shown on the canvas and in the inspector) | inspect |
| 9 | `node_inspect` | A node's bindings and properties with source spans, its type's members, its states | inspect |
| 10 | `node_insert` | Insert QML text or a block instance as a child at an index | mutate |
| 11 | `node_move` | Move a node to a parent and index (reparent or reorder) | mutate |
| 12 | `node_remove` | Remove a node with its comments | mutate |
| 13 | `node_wrap` | Wrap nodes in a container (`Row`, `ColumnLayout`, `Item`…) | mutate |
| 14 | `property_set` | Set or remove a property's value (a literal) | mutate |
| 15 | `binding_set` | Set a binding expression or a handler, checked before it is written | mutate |
| 16 | `state_edit` | Add, rename or remove a `State`; set a property change in it; add a transition | mutate |
| 17 | `blocks_list` | Registered blocks with contracts: state, actions, events, slots, mocks | inspect |
| 18 | `slot_contribute` | Add a `Contribution` to a block's slot | mutate |
| 19 | `theme_tokens` | Read tokens (`Ljos.Design` roles and the theme file), light and dark | inspect |
| 20 | `theme_set` | Set a token's value; preview reloads live | mutate |
| 21 | `history` | List, undo or redo edits (each with its inverse changes) | inspect to list; mutate to undo or redo |
| 22 | `preview_open` | Start a preview of a shell on a target (desktop, tablet, phone) or size with live or mocked state; on the open one, change size or state in place | run |
| 23 | `preview_state` | Read a block's published state in the preview; set it when the block is mocked | run |
| 24 | `preview_screenshot` | A PNG of the preview (≤ 1280 px), or its outline as text | inspect |
| 25 | `preview_events` | Since a cursor: opened, reloaded (with ms), reload failed, logs, ended | inspect |
| 26 | `export` | Write a build bundle or a site for a target | fs |

A document is a `.qml` file of the project outside the directories a walk
leaves out, judged where its links lead; anything else is `path_denied`.
A file reached by two names is one document: answers name it by where it
really is, and it has one generation and one history whichever name an
edit uses.

Every `mutate` tool:

- takes the document's `generation` and refuses a stale one, so two
  writers do not silently overwrite each other. A file changed by another
  program takes a new generation and an entry in the history when it is next
  read or edited. An edit that names an object by `path` must give the
  generation (`invalid_argument` otherwise): a path is true of one text and
  would name another object without notice. A handle or an `id` follows
  its object, so the generation stays optional with them;
- takes `dry_run` and then returns the changed ranges without writing;
- returns the changed ranges, the new generation and a `history` id; an
  edit that would write the bytes already there writes nothing, keeps the
  generation, makes no entry and says `unchanged: true`. `unchanged` is in
  every answer (`false` on a write), as the output schema says;
- refuses an edit that would not parse (`would_break`) and changes nothing.

`property_set` and `binding_set` remove a binding only when `value` is
`null`; a call that leaves `value` out is refused (`invalid_argument`), so
an argument forgotten by mistake removes nothing.

Node addresses are `{"node": "n12"}` (a handle from an earlier answer, which
follows its object through edits), `{"id": "header"}` or
`{"path": "root/0/2"}` with an optional `{"value": "background"}` step; paths
are re-resolved each call.

## Changes in review

The contract changes s1's review made, for a client written against the
first version:

- An edit that names an object by `path` must give `generation`
  (`invalid_argument`, kind `invalid`).
- `value` is required for `property_set` and `binding_set`; only `null`
  removes the binding.
- A no-op edit answers `unchanged: true`; `unchanged` is now always
  present, `false` on a write.
- A file reached through a link is one document, named by its resolved
  path.
- `history` `list` needs only `inspect`.

## Added to Ljós's servers (general)

| Server | Tool | Does |
|---|---|---|
| `ljos mcp` | `qml_edit_apply` | Apply a structured edit to a file, lossless; dry-run by default (`fs` to write) |
| `ljos mcp` | `blocks_list` | The block registry from the project's type dump and qmldir `block` lines |
| inspector | `app_blocks` | The live app's blocks with their current published state |
| inspector | `app_block_invoke` | Call a block action by name with arguments (`mutate`) |
| inspector | `app_alias` | Import a module from another directory (its mock module) and reload, keeping state (`mutate`, `--watch`) |
| `ljos run` | `--headless`, `--alias MODULE=DIR` | Run an application with no window (every `app_*` tool still works) and with a module imported from elsewhere |

## Previews (S2)

One preview runs at a time, of one document, in a child `ljos run
--headless --watch --mcp=inspect,input,mutate` process that Studio drives
through the child's inspector. After every write of the session (an
agent's edit, a person's typing, undo) Studio asks the child to reload at
once (`app_reload`), so the edit's tool call answers after the preview
shows it; the watcher's quiet time is not on that path.

- DECISION: Studio asks for at most 8 reloads a second (the inspector
  answers at most 20 calls a second, and a refused call waits a second to
  retry). A write past the budget is left to the child's watcher, and the
  next look at the preview (`preview_state`, `preview_screenshot`,
  `preview_events`) asks for the reload first, so what an agent reads is
  never older than what it wrote. A burst of edits costs no stalls; its
  last state is what shows.

- DECISION: targets are a parameter of `preview_open`, not a tool:
  `desktop` 1280×800, `tablet` 820×1180, `phone` 390×844, or any `size`.
- DECISION: re-opening the open document changes its size and its aliases
  in place (`started: false`) and keeps the app's state; another document
  starts another process. A re-open first asks the host whether the
  process still runs: one that ended (an `ended` event) is started again.
- DECISION: mock modules are the project's `mocks/` directory, one
  `qmldir` per module, found by its `module` line. `state: "mocked"`
  imports each from there (`--alias`, then `app_alias` to switch); the
  shell's text is the same either way.
- DECISION: `preview_state` names a block's object by node and needs its
  `id`; state names come from the block's contract. Setting is refused
  (`not_actionable`) unless the block is mocked: a live block's state is the
  app's own.
- DECISION: `preview_screenshot {format: "outline"}` gives the preview's
  items as text (the inspector's outline) instead of a separate tool:
  Studio's list stays within 25 tools with the ones still to come.
- DECISION: starting a preview needs `run` (it runs the project's code);
  looking at one that runs (`preview_screenshot`, `preview_events`) needs
  only `inspect`.

## Prompts

- **compose-shell:** `blocks_list` → `node_insert` per block →
  `binding_set` for skins → `preview_open` → `preview_screenshot`.
- **restyle:** `theme_tokens` → `theme_set` → `preview_screenshot`.
- **fix-preview:** `preview_events` → `document_text` → `binding_set` /
  `property_set` → `preview_events`.
