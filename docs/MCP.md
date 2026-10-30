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
on one server. Studio offers 21 tools today; with `theme_set` and `export`,
still to come, it offers 23. Capabilities as in section 19.7:
`inspect` (read), `mutate` (edit documents), `fs` (write files outside
documents: export), `run` (start previews). They are granted at launch with
`--caps`: by default `inspect` on a running window's endpoint and
`inspect,mutate` for the client that started `studio --mcp` on stdio.

## Studio server (`studio --mcp`)

| # | Tool | Does | Needs |
|---|---|---|---|
| 1 | `project_open` | Open a project directory below the workspace, or describe the open one: documents, blocks, targets | inspect |
| 2 | `document_tree` | A document's objects in order: handle, path, type, `id`, bindings; paginated | inspect |
| 3 | `document_text` | A document's text or a byte range of it, with its generation and whether it parses | inspect |
| 4 | `document_new` | Create a QML document; undo removes it | mutate |
| 5 | `document_edit` | Replace a byte range of a document's text, as the code view does | mutate |
| 6 | `node_find` | Find objects by `id`, type, binding name or text in a binding's value | inspect |
| 7 | `node_select` | Set Studio's selection (shown on the canvas and in the inspector) by addresses or by a point `at` in the preview | inspect |
| 8 | `node_inspect` | An object's bindings with byte ranges, its parent, its children, its type's members, its states and transitions | inspect |
| 9 | `blocks_list` | Registered blocks with contracts: state, actions, events, slots, mocks | inspect |
| 10 | `node_insert` | Insert QML text or a block instance as a child at an index | mutate |
| 11 | `node_move` | Move a node to a parent and index (reparent or reorder) | mutate |
| 12 | `node_remove` | Remove a node with its comments | mutate |
| 13 | `property_set` | Set or remove a property's value (a literal) | mutate |
| 14 | `binding_set` | Set a binding expression or a handler, checked before it is written | mutate |
| 15 | `state_edit` | Add, rename or remove a `State`; set its changes, its `when` or a breakpoint; add or remove a `Transition` | mutate |
| 16 | `slot_contribute` | Write a `Contribution` to a block's slot, or take one out | mutate |
| 17 | `history` | List, undo or redo edits (each with its inverse changes) | inspect to list; mutate to undo or redo |
| 18 | `preview_open` | Start a preview of a shell on a target (desktop, tablet, phone) or size with live or mocked state; on the open one, change size or state in place | run |
| 19 | `preview_state` | Read a block's published state in the preview; set it when the block is mocked | run |
| 20 | `preview_screenshot` | A PNG of the preview (≤ 1280 px), or its outline as text | inspect |
| 21 | `preview_events` | Since a cursor: opened, reloaded (with ms), reload failed, logs, ended | inspect |

A document is a `.qml` file of the project outside the directories a walk
leaves out, judged where its links lead; anything else is `path_denied`.
A file reached by two names is one document: answers name it by where it
really is, and it has one generation and one history whichever name an
edit uses.

Still to come, in the order they will be listed after `history`:

| # | Tool | Does | Needs |
|---|---|---|---|
| 22 | `theme_set` | Read the tokens (`Ljos.Design` roles and the theme file, light and dark), or set one; the preview reloads live | mutate |
| 23 | `export` | Write a build bundle or a site for a target | fs |

- DECISION: `theme_tokens` folds into `theme_set`, which reads the tokens
  when it is given no value, as `history` lists when given no action.
  Reading the tokens then needs `mutate`, but the same tokens are in the
  theme file, and `document_text` reads that with `inspect`.
- DECISION: `project_info` and `node_wrap` from the first plan are not
  tools. `project_open` with no path describes the open project, and a
  wrap is `node_insert` of the container followed by `node_move` of each
  node, two undo steps.

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

- DECISION: Studio launches the child with `--mcp-rate 200`: Studio is
  its only client, and Ljós's default of 20 calls a second made a brisk
  person wait on its one-second `Retry-After`. Studio asks for at most two
  fifths of the host's rate in reloads a second (80 for a native preview,
  8 for a host that states no rate). A write past the budget is left to the child's watcher; the
  reload it missed is asked for when the budget next allows (one timer per
  burst, which tells a window as any change does) or before the next look
  at the preview (`preview_state`, `preview_screenshot`, `preview_events`),
  whichever comes first. What an agent reads is never older than what it
  wrote, and a burst's last state is what shows.

- DECISION: targets are a parameter of `preview_open`, not a tool:
  `desktop` 1280×800, `tablet` 820×1180, `phone` 390×844, or any `size`
  of 1 to 8192 pixels on each side (others are `invalid_argument`).
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

## Selection from the canvas (S3)

`node_select {doc, at: {x, y}}` selects by a point of the open preview of
`doc`, in its logical pixels, instead of by `nodes`. Studio asks the
preview which object is topmost there and where it is declared (the
inspector's `app_inspect` with the target `at=X,Y`, whose answer has
`source {file, line, column}`, the place of the object's type name, as
the engine names an object's declaration in its warnings), then selects
the object whose type name starts there: the object itself, not one
enclosing it. An object of a block's own file answers that file; the root
of a component file and an inline component's instance answer where they
are used.

- DECISION: no new tool. A point is another way to name what `node_select`
  selects, and the list stays within 25.
- DECISION: the selected object may be in another document (a block's own
  file); the answer's `doc` says which. A place outside the project's
  documents is refused (`not_found`, `details.kind: "not_a_document"`).
- DECISION: refused (`not_actionable`) with no preview of `doc`
  (`no_preview`) and while the preview's last reload failed
  (`preview_behind`): it then runs older text, whose lines name other
  objects. A point that names no object of a document (the window's own
  content, under every stack) clears the selection.
- Giving both `nodes` and `at` is `invalid_argument`.

## Variants and slots (S4)

`state_edit {doc, generation, node?, action, …}` edits one object's
`states` and `transitions` (the root's when `node` is not given). Each
action is one undo step, labelled as the history shows it
(`add state compact`):

| `action` | Takes | Does |
|---|---|---|
| `add` | `state`, `when` or `breakpoint` | Append a `State` (creating `states: [ … ]`) |
| `rename` | `state`, `new_name` | Rename it, and the transitions' `from` and `to` that name it |
| `remove` | `state` | Take it out (and `states` with the last one) |
| `when` | `state`, `when` or `breakpoint`, or neither | Set or remove the condition that turns it on |
| `change` | `state`, `target`, `name`, `value` or `binding`, or neither | Set or remove one value in its `PropertyChanges` for `target` |
| `add_transition` | `from?`, `to?`, `animations?` | Append a `Transition` (`"*"` when not given) |
| `remove_transition` | `from?`, `to?` | Take it out |

`breakpoint` (`phone`, `tablet`, `desktop`) writes the root's width bound
between the preview targets: `root.width < 600`,
`root.width >= 600 && root.width < 1024`, `root.width >= 1024`. Refusals:
`not_found` with `details.kind` `no_state`, `no_transition` or
`no_change`; `invalid_argument` for a state or transition that exists
already (`exists`), `when` and `breakpoint` together, a
missing argument, a target or a root with no `id` (`no_id`), or a value or
animation that would not parse. `node_inspect` lists the object's
`states` (`name`, `when`, `changes`: `target`, `name`, `value`) and
`transitions` (`from`, `to`).

- DECISION: states are named, transitions by `from` and `to`; neither has
  a path of its own, since `qml_edit`'s node paths do not step into list
  elements. An edit splices inside one element's text.
- DECISION: a breakpoint names the root by its `id`, so it is refused on a
  root without one rather than giving it one unasked.
- DECISION: the bounds are those of the preview targets (`phone` 390,
  `tablet` 820, `desktop` 1280 wide), so each target's preview shows its
  own state. A person who wants another bound edits `when`.

`slot_contribute {doc, generation, slot, text | block, parent?, mode?,
order?}` writes a `Contribution` into `parent` (the root by default) with
the next free `contributionN` id, its `slot`, `mode` and `order` when they
are given, and a `delegate: Component { … }` holding `text` or an instance
of `block`. It imports `Ljos.Blocks`, and the block's module for a
`block`, when the document does not. `{doc, generation, remove}` takes one
out. Refusals: `not_found` (`no_slot`) for a slot no registered block
offers, `no_block`; `invalid_argument` for a delegate that is not one
object that parses, and (`not_a_contribution`) for removing anything else.

- DECISION: slot names are one namespace: a slot is valid when any block
  offers it, since a contribution names the slot and not the block
  (Ljos.Blocks' `Contribution`). Studio's palette contributes beside the
  selected instance, into its parent.
- DECISION: both tools are annotated destructive, since they can take
  things out.

## Prompts

- **compose-shell:** `blocks_list` → `node_insert` per block →
  `binding_set` for skins → `preview_open` → `preview_screenshot`.
- **restyle:** `theme_set` (read) → `theme_set` → `preview_screenshot`.
- **fix-preview:** `preview_events` → `document_text` → `binding_set` /
  `property_set` → `preview_events`.
