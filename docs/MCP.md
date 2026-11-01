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
on one server. Studio offers 25 tools. Capabilities as in section 19.7:
`inspect` (read), `mutate` (edit documents), `fs` (write files outside
documents: export), `run` (start previews). They are granted at launch with
`--caps`: by default `inspect` on a running window's endpoint and
`inspect,mutate` for the client that started `studio --mcp` on stdio.

## Studio server (`studio --mcp`)

| # | Tool | Does | Needs |
|---|---|---|---|
| 1 | `project_open` | Open a project directory below the workspace, or describe the open one: documents, blocks, targets; `asset` lists, imports or removes its pictures and fonts | inspect; fs to import or remove |
| 2 | `document_tree` | A document's objects in order: handle, path, type, `id`, bindings; paginated | inspect |
| 3 | `document_text` | A document's text or a byte range of it, with its generation and whether it parses | inspect |
| 4 | `document_new` | Create a QML document, or one keeping a project JSON file's records (`data`): a `JsonListModel` of the file, or a `ListModel` copy; undo removes it | mutate |
| 5 | `document_edit` | Replace a byte range of a document's text, as the code view does | mutate |
| 6 | `node_find` | Find objects by `id`, type, binding name or text in a binding's value | inspect |
| 7 | `node_select` | Set Studio's selection (shown on the canvas and in the inspector) by addresses or by a point `at` in the preview; `hold` leases them to the caller | inspect (`hold`: mutate) |
| 8 | `node_inspect` | An object's bindings with byte ranges, its parent, its children, its type's members, its signals, its states and transitions, its flows and the data it sees | inspect |
| 9 | `blocks_list` | Registered blocks with contracts: state, actions, events, slots, mocks; per document, the toolkit types it can place, QtQuick3D among them | inspect |
| 10 | `node_insert` | Insert QML text or a block instance as a child at an index | mutate |
| 11 | `node_move` | Move a node to a parent and index (reparent or reorder) | mutate |
| 12 | `node_remove` | Remove a node with its comments | mutate |
| 13 | `property_set` | Set or remove a property's value (a literal) | mutate |
| 14 | `binding_set` | Set a binding expression or a handler, checked before it is written | mutate |
| 15 | `state_edit` | Add, rename or remove a `State`; set its changes, its `when` or a breakpoint; add or remove a `Transition` or an interaction | mutate |
| 16 | `slot_contribute` | Write a `Contribution` to a block's slot, or take one out | mutate |
| 17 | `history` | List, undo or redo edits (each with its inverse changes and its author); undo and redo take the caller's own, `all` the newest | inspect to list; mutate to undo or redo |
| 18 | `theme_set` | Read the tokens (`Ljos.Design` roles and the theme file, light and dark), or set one; the preview reloads live | mutate |
| 19 | `preview_open` | Start a preview of a shell on a target (desktop, tablet, phone) or size with live or mocked state; on the open one, change size or state in place | run |
| 20 | `preview_state` | Read an object's state and a block's published state in the preview; set it when the block is mocked; hold an object in a state of its own (`show`); click first | run |
| 21 | `preview_screenshot` | A PNG of the preview (≤ 1280 px), or its outline as text | inspect |
| 22 | `preview_events` | Since a cursor: opened, reloaded (with ms), reload failed, logs, ended; or wait until its app rests saying something new (`wait_ms`, past `rests`), answering when it wakes next (`wake_in_ms`) and asking the app nothing meanwhile | inspect |
| 23 | `export` | Write a bundle that `ljos run` starts (`native`) or that ljos-web builds a site from (`web`): the project's QML, theme, assets and own modules with a manifest, never mocks or `.studio/` | fs |
| 24 | `review` | Add, reply to, resolve, reopen or list design review comments on objects, kept in `.studio/` | inspect (writing: mutate) |
| 25 | `node_arrange` | What a canvas gesture does: `geometry` (boxes, what a drag does, siblings), `place` (`record` writes into a state), `align`, `copy`, `paste`, `duplicate`, `remove`, `component_extract` (below), `wrap`, `unwrap`, `relayout`, `anchor`; each edit is one undo step, all or none | inspect; mutate to edit |

A document is a `.qml` file or a `theme.json` of the project outside the
directories a walk leaves out, judged where its links lead; anything else is
`path_denied`. A file reached by two names is one document: answers name it
by where it really is, and it has one generation and one history whichever
name an edit uses.

- DECISION: `theme_tokens` folds into `theme_set`, which reads the tokens
  when it is given no value, as `history` lists when given no action.
  Reading the tokens then needs `mutate`, but the same tokens are in the
  theme file, and `document_text` reads that with `inspect`.
- DECISION: `project_info` and `node_wrap` from the first plan are not
  tools. `project_open` with no path describes the open project, and a
  wrap is `node_arrange`'s `wrap`, one undo step.

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
  Studio's list of 25 stays within 25 tools.
- DECISION (phase 3): the native preview runs with the project's type
  dumps (every `ljostypes.json` in it, `--types`), so the types an app
  registers from Rust stand in with the members they declare and a shell
  that uses them previews. Several dumps run as one, merged into the
  state directory; a dump that does not read stops the start, naming it.
  A dump is read when the preview starts: an edit of it shows when the
  preview is opened again. Export still refuses a project whose types
  are written in Rust.
- DECISION: a preview in a browser (ljos-web, DESIGN.md S5) answers every
  preview tool through `ljos-web/inspect`. A page built without it refuses
  `not_actionable` with `details.kind: "not_supported"`, never with a
  made-up answer.
  One whose app failed to start refuses `not_actionable` with
  `details.kind: "refused"` and `details.code: "not_running"`: no ljos-web
  request will change that, and a save that mends it starts it again.
- DECISION: a preview in a browser keeps the app's state over a reload, as
  `--watch` does; a file read as data is told to what reads it, nothing
  compiled; switching a block between live and mocked starts the app again,
  since another module's code is another program.
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

A point on a `View3D` selects the `Model` under it when the host can read
the scene (the native preview): Studio casts a ray through the camera's
view against each model's box (`#Cube`, `#Rectangle`, `#Cylinder`, `#Cone`)
or sphere, and the nearest wins. A file mesh, an orthographic camera or a
host that reads no scene leave the `View3D` selected.

`node_arrange` `geometry` on a spatial object (a `Model`, a light or a
camera) has no box; it answers `scene` instead: the view's window box, the
camera (`eye`), the object's `origin`, `axes` (the scene's vector for one
unit of `x`, `y`, `z`), `local` as written and `refused` per axis (empty
where a drag may write). `place` takes `z` as well; it refuses with kind
`bound` an axis an expression sets, and a `position` set whole.
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

## Tokens (S4)

`theme_set {doc?, token?, value?, variant?, generation?, dry_run?}` works
on the theme file of `doc`: the one Ljós reads for it, the nearest
`theme.json` from its directory up to the project's, the document itself
when it is a `theme.json`, the open preview's when `doc` is left out. When
there is none it is the project's own, the file Ljós watches then, so a new
theme is made there. It is listed with the edit tools, after
`history`, so the tools a reader or an editor is offered come before the
preview tools, as they did.

- **Reading** (no `value`): `file`, its `generation` (0 while it does
  not exist), `exists`, `problem` (why Ljós would not load the file, with
  its values then left out), and `tokens`: one row per role of
  `Ljos.Design` (or the one `token` names, or a singleton's roles) with
  its `kind` (`color`, `int`, `real`, `curve`), the module's value and
  the theme file's. A colour's values are `light`, `dark`, `lightHigh`
  and `darkHigh`.
- **Setting** (`token` and `value`): `value` is a colour string, a
  number or a curve's JSON; `null` gives the role back to the module. A
  colour string sets every variant; `variant` sets one. The answer is
  an edit's (`doc`, `changes`, `generation`, `history`, `unchanged`,
  which every write's answer has and a read's has not), the rest of the
  file is kept byte for byte, and `history` undoes it.
- **Refusals**: `invalid_argument` with `details.kind` `theme` and Ljós's
  own `errors` (`file`, `line`, `column`, `message`) for a theme Ljós
  would not load, a colour pair below its contrast among them, so a theme
  Studio writes always loads; `not_found` (`no_token`) for a role the
  module does not have; `invalid_argument` (`invalid`) for a value or
  variant that does not fit the role; `not_actionable` (`nothing_to_do`)
  for `null` on a role the file does not set; `stale_ref` for a stale
  `generation`.

- DECISION: Studio checks a theme with Ljós's reader (`ljos_design`)
  before it writes, rather than writing and letting the preview say why
  it did not load: an agent learns at once, and the file on disk always
  loads.
- DECISION: a write is renamed over the file, as document writes are, so
  a watcher (the preview's, or Studio's own window's) never reads half
  of one.

Studio's window shows the same tokens in its *Tokens* panel (DESIGN.md,
S4 "As built: tokens, shells and the self-reskin"): its `Workspace`
publishes `theme` (the file) and `tokens` (the rows `theme_set` reads),
and an edit there is `dispatch("theme_set", …)`.
## Flows and list data (S6)

A prototype's flow is the app's own QML: a tap goes to one of a
component's states or opens another document, written as a signal
handler on the object tapped. `state_edit` carries it:

| `action` | Takes | Does |
|---|---|---|
| `interaction` | `trigger`, `signal?`, and `state` (`""` is the base state) or `open` with `navigator` | Write `onTapped` in a new `TapHandler` of `trigger`, or the signal's own handler (`onClicked`) when `signal` is named; its body is `owner.state = "S"`, `navigator.source = "D"` (a `Loader`) or `navigator.push(Qt.resolvedUrl("D"))` (a `StackView`) |
| `remove_interaction` | `trigger`, `signal?` | Take it out: the whole `TapHandler` when it holds nothing else |

`node` names the owner of the state (the root by default). The history
labels are `tapped card to details` and `remove tapped interaction`.
From an older generation, a tap flow lands past another writer's new child
of the trigger, but conflicts with another writer adding or taking out a
`TapHandler` that is a direct child of it (`details.name` is `<taps>`).
Taking a tap flow out also conflicts with any change to the handler it is in.
Refusals: `not_found` with `details.kind` `no_state`, `no_document` (the
document must be beside this one) or `no_interaction`; `invalid_argument`
for `exists`, an owner or navigator with no `id` (`no_id`), a navigator
that is not a `Loader` or `StackView` (`not_a_navigator`), `state` and
`open` together, and a missing argument. With `signal`, the trigger must
have it: `not_found` with `details.kind` `no_signal`, `type`, and
`signals`, the names it has other than its properties' change signals,
which the message names too. `node_inspect` lists the object's `signals`,
each with `name`, `arguments` (`name` and `type`; an argument the type's
description leaves unnamed has an empty name) and `change` for a
property's change signal: those the object declares and its properties'
first, then its type's and its bases', a derived type's before its base's.
A type is looked up as the document imports it: a dump of the project
(`ljostypes.json`, a type written in Rust), a project module's `qmldir`, a
QML file beside the document, then Ljós's own table of types. A type
described nowhere has no `signals`, and a flow on it is written as asked.
`node_inspect` lists `interactions` (`trigger`, `trigger_id`, `signal`, and `owner` and `state`
or `navigator` and `open`) of the object and everything in it, so the
root's is the document's whole flow; a handler whose body is not one of
the three forms is the app's own logic and is not listed.

`preview_state {node?, click?, click_at?: {x, y}, set?}` plays the flow:
after `set`, it clicks the object `click` names (by its `id`, at its
middle) or the point `click_at`, through the preview's inspector, then
reads `node`. Without `node` it only clicks and answers nothing (`id` and
`variant` are left out), so a document whose root has no `id` can be
played; with neither `node` nor a click, or `set` without `node`, it is
refused (`invalid_argument`). `variant` is the state it is in now (empty for the base
state); `block` and `state` are left out for an object that is not a
block, and `set` on one is refused (`not_a_block`).

A list's data is a model, in the document or in a document of its own
the list makes (`model: Contacts {}`). `document_new {doc, data,
model?}` makes one from a project JSON file. By default (`model:
"json"`) it is a `JsonListModel` whose `source` is the file, relative to
the new document and written `Qt.resolvedUrl("…")` so that it resolves
against that file wherever the model is used (`import Ljos.Models`); the file must be an array of
records (`invalid_argument` with `not_a_list`), whose values may be
anything JSON holds. With `model: "list"` it is a `ListModel` copy of
flat records, one `ListElement` each with the records' keys in order
(`invalid_argument` with `not_a_list`, `not_a_role` (also for a key
named `id`, which QML reads as the object's id), `repeated_role` for a
key a record has twice, or `not_flat` otherwise); `text` and `data`
together are refused either way. `node_inspect` gives the `model` the
object sees (a view's delegate only, not its header or footer): `roles`
(in the order the rows first bind them; a `JsonListModel`'s are its
records' keys in the order first seen, read from the file when asked),
`rows`, `doc`, `source` (the JSON file, for a `JsonListModel`), and
`view`, the list it is a delegate of; a delegate binds a role as
`model.name` with `binding_set`. A `JsonListModel` whose `source` is not
a project file (a URL, a binding) or whose file is not an array of
records sees no roles Studio knows. In the native preview a save of the
JSON, by anything, shows without a reload of the QML: `ljos run --watch`
reads a watched `source` again. The mocked state swaps the data for a
fixture: a module under `mocks/` of the same name (`mocks/App/Data`)
replaces the app's (`App/Data`), its document reading a JSON file of its
own beside it, and the list's text is unchanged.

- DECISION: no new tool. Flows are `state_edit` actions beside states and
  transitions, which they move between; playing one is `preview_state`
  with a click, which reads the state the app reached; data is
  `document_new` and `node_inspect`. The surface stays at 21 tools.
- DECISION: an interaction is written in one of three forms and read back
  only in those, so Studio never claims to understand a handler it did
  not write.
- DECISION: `click_at` clicks at the point itself, through the preview
  inspector's `app_click` with `at` (phase 3): whatever is on top there
  gets it, as under a person's pointer, so a thin trigger under a larger
  item's edge is reached where it shows. It used to click the middle of
  the topmost item drawn there, which missed such a trigger. A point
  outside the preview's window is refused (`not_actionable`, `details.kind`
  `refused`) and the preview runs on.
- DECISION (phase 3): JSON stays JSON by default. `document_new` with
  `data` makes a `JsonListModel` reading the file, because the file has
  an owner outside Studio (an export, a script, a person's editor, the
  server it mirrors): a copy would fork the data the first time either
  side changed, and a `JsonListModel` takes nested values a `ListModel`
  cannot hold. `model: "list"` keeps the phase 2 path, a `ListModel` copy,
  for data the design itself owns and edits row by row as QML. It used to
  be the only path: Ljós had no model that read JSON.
- DECISION: a flow's handler is checked against the trigger's signals,
  and `binding_set` of an `on…` handler is not: `binding_set` already
  compiles what it writes, and Ljós names a handler of nothing.
- DECISION: the signals a person is offered, and a refusal names, leave
  out properties' change signals: every property has one, and they hide
  the few a trigger is for. `node_inspect` lists them, marked `change`.
## Export and review (S6)

`export {entry, target?, out?, dry_run?}` (`fs`) writes a bundle of the
project below its `build/` (`build/native` by default) and answers `out`,
`path` (the same, absolute),
`files` and `bytes` (totals that stay true when a long manifest is cut
short in the answer), the `manifest`, `written` (false for `dry_run`),
`replaces` (whether `out` held a bundle before the call, which a real export
swaps out) and `ms`. The bundle holds every file
of the project except `.studio/`, the top-level `mocks/`, hidden files,
`build/`, `target/`, `node_modules/` and unfinished writes, and beside them
`ljos-bundle.json`:

| Field | Holds |
|---|---|
| `format`, `version` | `ljos-bundle`, 1 |
| `target`, `entry` | `native`; the document the app starts with |
| `run` | `["ljos", "run", <entry>, "--project", "."]`, run in the bundle |
| `theme` | the nearest `theme.json` from the entry up to the project, when there is one |
| `blocks` | The project's own blocks (`App.Media.Player`) and their files |
| `files` | Every file, `path` and `bytes`, sorted |
| `left_out` | What of the project is not in it, `path` and `why` |

`target: "web"` writes the same files to `build/web` with `ljos-site.json`
instead (DESIGN.md S6, the site contract), which a site builder reads:

| Field | Holds |
|---|---|
| `format`, `version` | `ljos-site`, 1 |
| `target`, `entry` | `web`; the document the site mounts |
| `theme`, `blocks`, `files`, `left_out` | As above |
| `modules` | The project's own modules, `module` and `qmldir` |
| `imports` | The modules the documents import that are not the project's, sorted |
| `ljos_web` | The ljos-web commit they were checked against |

It has no `run`. `mise run export:web:build build/web` builds the site
with an ljos-web checkout.

A bundle is written beside its place and swapped in, so it is never half
of two exports. Refusals: `capability_denied` without `fs`; `not_found`
for an entry that is not a document; `unsupported` for types written in
Rust (`native_types`, with `details.files`) and, for `web`, for a module
neither the project nor ljos-web provides (`no_web_face`, with
`details.modules` and `details.docs`: `Ljos.Blocks`, `Ljos.Models`, a
plugin's); `syntax` for a document that does not parse (`broken`,
`details.doc`); `not_actionable` for a module only a mock provides
(`mocked_only`, `details.modules`) and for a project not in a directory
(`no_directory`) and for an entry that is not in the bundle
(`entry_left_out`, `details.why`); `path_denied` for an `out` not below
`build/` or one a link leads elsewhere; a link out of the project is left
out (`left_out`, "a link out of the project");
`invalid_argument` (`exists`) for an `out` that holds something other
than a bundle of either kind (a site built from one, say).

- DECISION: the native bundle is the project directory as `ljos run`
  starts it. Ljós's production bundles (ARCHITECTURE.md 12.2: hashed,
  signed, A/B) and AOT binaries need a build Studio does not make, so the
  manifest has no hashes or signature yet.
- DECISION: a web export is a directory and its manifest, not a site:
  compiling and bundling are ljos-web's, choosing the files and refusing
  what has no face on the web are Studio's. `export:web:build` uses ljos-web's
  compiler and Vite plugin until `ljos-web build-site` handles a themed bundle.
- DECISION: `export` writes no history entry; it changes no document.

`review {action, doc?, node?, comment?, text?, author?}` keeps design
review comments in `.studio/review.json`, never in the QML:

| `action` | Takes | Does |
|---|---|---|
| `add` | `doc`, `node`, `text`, `author?` | Comment on an object (`c1`, `c2`, …) |
| `reply` | `comment`, `text`, `author?` | Answer it |
| `resolve`, `reopen` | `comment` | Mark it resolved, or open again |
| `list` | `doc?`, `node?` (with `doc`) | The comments, in the order they were made |

A comment answers `id`, `doc`, `node` (the object's handle, absent once
orphaned), `anchor` (`path`, `line`, `type`, `id?`: where the object is or
last was), `orphaned`, `resolved` and `thread` (`author?`, `text`, `at`,
milliseconds since the epoch). Listing needs `inspect`; the rest `mutate`.
Refusals: `not_found` (`no_comment`), `invalid_argument` for a comment
with no text, `not_actionable` (`review_unreadable`) for any call while
the sidecar does not read or a later Studio wrote it (it is read again on
each call, so mending it is enough), `stale_ref` (`review_changed`) for a
write when somebody else changed the sidecar since Studio read it: the
comment is not made and the sidecar on disk is what Studio holds now.

- DECISION: a comment follows its object as a handle does, through
  anybody's edits; on disk it carries the object's key, so the next
  session finds it again.
- DECISION: a comment whose object is removed is kept, orphaned, at the
  place it last had, and can still be answered and resolved. Undo gives it
  its object back in the session; a new session does not match an
  orphaned comment to an object again.
- DECISION: comments are not in the undo history. `reopen` takes back
  `resolve`; a comment is not deleted, it is resolved.

## Phase 3 / M1: as built

What changed, for an agent: no tool was added (24 of the 25 budgeted).

- `document_new` with `block: true` also declares the document in the
  `qmldir` beside it, `# block Name 1.0 File.qml`, making that `qmldir` with the
  directory as its module if there is none. One undo takes back the file and
  that line, leaving what else was written to the `qmldir`.
- `document_new` with `data` writes a `JsonListModel` by default; `model:
  "list"` writes the `ListModel` copy it wrote before.
- `node_inspect` answers `signals`, and a model's `source` for a
  `JsonListModel`, whose `roles` are the JSON's keys.
- `node_inspect` answers `takes`, the properties the object's type declares
  in the project's files and type dumps, written on it or not. The toolkit's
  own are left out: there are too many to list on every object, and
  `type_lookup` says them.
- `state_edit`'s `interaction` with `signal` is refused (`no_signal`) when
  the trigger lacks the signal, naming those it has.
- `preview_state`'s `click_at` clicks at the point itself (`app_click`
  with `at`).
- `preview_open` starts the native preview with the project's type dumps.

The DECISIONs are with the tools above: JSON stays JSON, `click_at` at
the point, which handlers are checked, which signals are offered, the
preview's type dumps; DESIGN.md has the rest and the numbers.

Each changed tool, called as an agent does in a release build
(`cargo run --release -p studio_mcp --example inspect_json`, 200 calls
each, on a project whose list reads 1,000 JSON records and that keeps a
type dump):

| Tool call | p50 | p95 |
|---|---|---|
| `node_inspect` of a list reading 1,000 JSON records (roles, rows, source) | 0.39 ms | 0.44 ms |
| `node_inspect` of a `Button` (signals from Ljós's table of types) | 0.10 ms | 0.11 ms |
| `node_inspect` of a type a dump describes (signals from `ljostypes.json`) | 0.10 ms | 0.10 ms |
| `state_edit` `interaction` with `signal`, the signal checked | 0.13 ms | 0.18 ms |
| `document_new` with `data`, a `JsonListModel` of 1,000 records | 0.27 ms | 0.32 ms |

`preview_open` and `preview_state` wait on the preview; DESIGN.md has a
JSON save's time to show.

## Phase 4: writers side by side, as built

Several writers edit one project at once: agents over MCP and the person
at the window. Still 24 tools; MULTIPLAYER.md section 6 has the design,
DESIGN.md "Phase 4: rebase by handle, as built" the DECISIONs and numbers.

**Who writes.** A call's writer is its MCP client's name, or `agent` when
the client gave none; the window is `person`. Two clients giving one name
are one writer: they share an undo stack and leases.

**Rebase.** An edit made at an older `generation` is moved onto the
current text rather than refused, and its answer says `rebased_from`
(the generation it was made at; absent when it was current):

- an edit naming its object by handle or `id` passes the entries since
  its generation when none of them, by another writer, touched the same
  binding of the same object, or touched the object or an object holding
  it as a whole (a remove, a move, a replace, or a write of the binding
  whose value it is written in). An edit giving a child `index` also
  conflicts with another writer's change of that parent's children, as the
  index counts the children it read; one with no `index` appends. Otherwise it is refused
  `stale_ref`, kind `conflict`, with `doc`, `handle` (`null` for a change of
  the text that is no one object's), `name`, the other
  writer (`author`), its edit's `label` and `generation`, and the value
  `now` (or `null`);
- an edit by `path` is never rebased: a path is true of one text, so an
  old generation stays `stale_ref`, kind `stale_generation`;
- `document_edit` moves its ranges over the changes since
  (`qml_edit::transform`, the earlier change first). A range that overlaps
  one of them is a `conflict`; a moved edit that newly breaks the parse is
  refused `invalid_argument`;
- the log keeps 256 entries a document. A base older than those, or an
  entry from a change found in the file, is refused as before.

A writer's own entries never conflict with it: an agent making a burst of
edits from one read does not have to read again between them.

**Leases.** `node_select` with `hold: true` (needs `mutate`) leases the
selected objects and all they hold to the caller; the answer's `held`
lists the handles the caller holds in that document. A lease lasts 5 s
past the holder's last call (any call renews it) and 5 min at most; then
it ends, and the holder's next `hold` is refused `not_actionable`, kind
`lease_spent`, saying to release and plan again, until they release.
Leases end when their time is up; nothing watches the clock. `node_select` with no nodes and `hold:
false` lets go. While a lease stands, another writer's `property_set`,
`binding_set`, `node_*` edits, `state_edit`, `slot_contribute`,
`document_edit`, `theme_set` and `history` undo or redo changes reaching
into its bytes, and
`review` writes on a held object are refused `busy`, kind `leased`, with
`holder` and `until_ms`. Reading is never refused. The window shows the
holder in the layers panel and on the canvas ("held by Ada").

**History by author.** Every `history` entry has an `author`. `undo` and
`redo` take the caller's newest own entry, moved past what others wrote
since (refused `conflict` when someone changed the same thing); `all:
true` takes the newest anyone made, as before. The window's Undo takes the
person's own. `list` lists every writer's entries.

## Layout tools: wrap, unwrap, anchor

`node_arrange` `wrap` {nodes, in: Row|Column|Grid|Item, spacing?, columns?}
puts siblings in a new object at their joint box, in reading order. A layout's
children lose their written x/y; an Item's are counted from its corner and keep
document order, which is what is drawn over what. `spacing` defaults to the
gap they had, `columns` to the widest row. Refused: `not_siblings`, `root`,
`anchored`, `reads_parent`, `locked`.

`unwrap` {nodes: [one]} takes a Row, Column, Grid, Flow, Item or layout away
from around its children: they get their place on the preview, or for an Item
its numbers added to theirs, or none. Refused: `not_a_layout`, `holds_more`
(its own property, function or handler), `referenced` (something reads its id),
`reads_parent`, `root`.

`relayout` {nodes: [one], name} turns a Row, Column, Grid or Flow into another
of them, or a `*Layout` into another `*Layout`, writing only the type's name.
Refused: `not_a_layout`, `other_family`, `says_other` (it says something the new
kind has no member for, such as a Grid's `columns`).

`anchor` {nodes, fill?, centerIn?, sides?: {left, right, top, bottom}, target?}
writes `anchors.*` and takes away the x, y, width and height that now disagree.
A side's value is its margin, null takes it off; `fill` and `centerIn` take
true or false. An axis let go of keeps its place from the preview when one is
running. `target` is `parent` (the default) or a sibling's id. Refused:
`arranged` in a Row, Column, Grid, Flow or `*Layout`, saying what to use
instead; `not_a_sibling`, `root`, `locked`.

Each is one undo step. The bar above the canvas has wrap and unwrap; the
inspector's Layout section has the pin pad, a layout's spacing and columns,
turning it into another kind, and wrap.

## Components from a selection

`node_arrange` `component_extract` {nodes: [one], name} writes `Name.qml` beside
the document, holding what the document imports and the object's text with its
own id, and leaves `Name { }` in its place with the id and the x, y and anchors,
which say where it sits rather than what it is. Both files are one undo step:
undo and redo take both, and an undo that would take away what another writer
put in the new file is refused with both left as they are. If the new file was
deleted outside Studio, undo gives back the document alone.

`inner.prop` read from outside becomes `property alias innerProp: inner.prop` on
the component and `instance.innerProp` where it was read, giving the instance an
id if it had none; the answer's `aliases` names them. `id` of the object itself
goes on working as it did. Refused, naming the line and word:

- `reference_out`: an inner id used as a whole (`target: inner`), called or gone
  through (`inner.f()`, `inner.font.bold`), or read by the place that stays.
- `reference_in`: the object reads an id or a root property left in the document,
  an inline component of the document, or its own `parent`.
- `root`, `exists` (the file is there), `in_component` (inside an inline
  component), `locked`, and `invalid_argument` for a name that is no type.

Words are told apart by how they are written, without types: a property named
like an inner id reads as a use and is refused, never rewritten silently. The
Layout section of the inspector has a name field and the button; the palette's
project list then offers the new file.

## Assets

`project_open` with `asset` works on the project's `assets/`, by the same call
the Insert tab's Assets section makes. An action here and not a tool of its
own because the list stays at 25 tools.

| `asset` | Takes | Answers | Needs |
|---|---|---|---|
| `list` | | `assets`: `name`, `url` (from the project's root), `kind` (`image`, `font`), `bytes`, and `width`, `height` of a picture that says them | inspect |
| `import` | `file` or `data` (base64) with `name`; `name` is optional with `file` | `asset`, as above | fs |
| `remove` | `name` | `removed`, the url | fs |

- `import` takes png, jpg, webp, svg, gif, ttf, otf and woff2, by the name's
  extension and by what the file begins with; anything else is refused
  `not_actionable`, kind `asset_kind`, listing them. 64 MiB at most (kind
  `asset_size`).
- The name becomes letters, digits, `.`, `_` and `-`, leads nowhere, and takes
  a number if a name differing at most by case is there: `LOGO-1.svg`.
- `file` is judged where it really is, relative to the project: below the
  workspace and a regular file, or `path_denied` (what is missing answers the
  same, so a refusal does not tell what exists outside). A file elsewhere goes
  as `data`.
- `remove` is refused `not_actionable`, kind `asset_in_use`, with `documents`,
  while a document says the asset's url. A url a document builds at run time
  from parts is not seen.
- DECISION: assets have no undo. They are not documents; an import writes a
  name nothing uses and a removal happens only when nothing does.
- A picture is placed by `node_insert` with `text: "Image { source:
  \"assets/x.png\" }"`, and `export` carries `assets/` in both bundles, bytes
  as they are.
- On the web a file that is not text is held in browser storage as a data
  URL's text; `list`, `import` and `remove` work the same.

## Prompts

- **compose-shell:** `blocks_list` → `node_insert` per block →
  `binding_set` for skins → `preview_open` → `preview_screenshot`.
- **restyle:** `theme_set` (read: the tokens and the file's `problem`) →
  `theme_set` per role, a refusal's `errors` saying which pair fails its
  contrast → `preview_events` (the reload) → `preview_screenshot`.
- **fix-preview:** `preview_events` → `document_text` → `binding_set` /
  `property_set` → `preview_events`.
- **prototype:** `state_edit` (`add`, a state per screen) → `state_edit`
  (`interaction`, a trigger per step) → `preview_open` → `preview_state`
  (`click`) per step, checking `variant` → `preview_screenshot`.
- **review:** `review` (`list`, by doc) → `document_tree` →
  `node_inspect` of each object commented on → `preview_screenshot` →
  `review` (`add` or `reply`, one per finding, on the object it is about)
  → `review` (`resolve`) once an edit answers one.
- **ship:** `export` with `dry_run` → mend what it refuses → `export` →
  start the bundle with its manifest's `run`; for the web, `export` with
  `target: "web"` and build the site from `build/web`.
