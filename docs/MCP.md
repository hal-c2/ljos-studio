# Studio's MCP tool surface

Every Studio capability is one of these tools before or alongside its
panel. People and agents act through the same `studio_core` actions, so an
agent's change shows on the canvas and is undone like a person's.

Studio composes three servers rather than duplicating them:

- **`ljos mcp`** (Ljós's authoring server, `ljos_mcp_author`): checking,
  linting, formatting, types, docs, scenarios. Studio forwards to it and
  adds `qml_edit_apply` and `blocks_list` *to Ljós* (general tools).
  Studio's own `blocks_list` (#15) is the same registry, built by the
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
| 5 | `node_find` | Find nodes by `id`, type, property value or text; returns addresses | inspect |
| 6 | `node_select` | Set Studio's selection (shown on the canvas and in the inspector) | inspect |
| 7 | `node_inspect` | A node's bindings and properties with source spans, its type's members, its states | inspect |
| 8 | `node_insert` | Insert QML text or a block instance as a child at an index | mutate |
| 9 | `node_move` | Move a node to a parent and index (reparent or reorder) | mutate |
| 10 | `node_remove` | Remove a node with its comments | mutate |
| 11 | `node_wrap` | Wrap nodes in a container (`Row`, `ColumnLayout`, `Item`…) | mutate |
| 12 | `property_set` | Set or remove a property's value (a literal) | mutate |
| 13 | `binding_set` | Set a binding expression or a handler, checked before it is written | mutate |
| 14 | `state_edit` | Add, rename or remove a `State`; set a property change in it; add a transition | mutate |
| 15 | `blocks_list` | Registered blocks with contracts: state, actions, events, slots, mocks | inspect |
| 16 | `slot_contribute` | Add a `Contribution` to a block's slot | mutate |
| 17 | `theme_tokens` | Read tokens (`Ljos.Design` roles and the theme file), light and dark | inspect |
| 18 | `theme_set` | Set a token's value; preview reloads live | mutate |
| 19 | `history` | List, undo or redo edits (each with its inverse changes) | mutate |
| 20 | `preview_open` | Start a preview of a shell on a target (desktop, phone-sized, web) with live or mocked state | run |
| 21 | `preview_state` | Read or set mocked block state in the preview | run |
| 22 | `preview_screenshot` | Screenshot a preview or a frame (≤ 1280 px, ≤ 1 MB) | inspect |
| 23 | `preview_events` | Since a cursor: reloads, errors, block events, logs from the preview | inspect |
| 24 | `export` | Write a build bundle or a site for a target | fs |

Every `mutate` tool:

- takes the document's `generation` and refuses a stale one, so two
  writers do not silently overwrite each other;
- takes `dry_run` and then returns the changed ranges without writing;
- returns the changed ranges, the new generation and a `history` id;
- refuses an edit that would not parse (`would_break`) and changes nothing.

Node addresses are `{"id": "header"}` or `{"path": "root/0/2"}` with an
optional `{"value": "background"}` step; paths are re-resolved each call.

## Added to Ljós's servers (general)

| Server | Tool | Does |
|---|---|---|
| `ljos mcp` | `qml_edit_apply` | Apply a structured edit to a file, lossless; dry-run by default (`fs` to write) |
| `ljos mcp` | `blocks_list` | The block registry from the project's type dump and qmldir `block` lines |
| inspector | `app_blocks` | The live app's blocks with their current published state |
| inspector | `app_block_invoke` | Call a block action by name with arguments (`mutate`) |
| inspector | `app_preview_alias` | Map a module to its mock module and reload (`mutate`, dev builds) |

## Prompts

- **compose-shell:** `blocks_list` → `node_insert` per block →
  `binding_set` for skins → `preview_open` → `preview_screenshot`.
- **restyle:** `theme_tokens` → `theme_set` → `preview_screenshot`.
- **fix-preview:** `preview_events` → `document_text` → `binding_set` /
  `property_set` → `preview_events`.
