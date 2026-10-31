# Ljós Studio

A visual designer for Ljós applications and ljos-web sites, where the
design is the application's own QML. People and agents drive it alike:
every capability is an MCP tool.

## Run it

| | Command | Needs |
|---|---|---|
| Native window | `mise run studio [DIR]` | Rust only (mise installs the pinned one) |
| In a browser | `mise run web:serve`, then open the URL it prints | ljos-web beside this checkout, with its packages; Qt 6 for Controls; Node (mise installs it) |

`mise run studio` opens `DIR`, or with none a copy of `samples/hello` in the
target directory, so your edits leave the sample alone. It builds release:
opening a shell and following a selection are several times faster than in a
debug build, and the first build is the only cost.

While the window is open, agents reach Studio's tools on a loopback endpoint;
it prints the URL and the file that holds the token. That endpoint only reads
(`inspect`) unless you grant more at launch: `--caps` takes a comma-separated
list of `inspect`, `mutate`, `fs` and `run`, so
`mise run studio DIR --caps inspect,mutate` lets an agent edit. `--no-agents`
serves no endpoint. `studio --mcp` serves the tools on stdin and stdout
instead, with no window, and there the default is `inspect,mutate`.

`mise run web:serve` serves on `localhost` only (`PORT`, default 5199).
Projects live in your browser's storage; *New* starts one and *Import* reads a
folder of text files. Your edits compile on that local server, which draws the
preview, because ljos-web has no QML compiler that runs in a page.

For the browser, once:

```sh
git clone https://github.com/hal-c2/ljos-web ../ljos-web
git -C ../ljos-web checkout "$(cat web/ljos-web.rev)"
(cd ../ljos-web && mise trust && mise install && pnpm install --frozen-lockfile)
```

and Qt 6's QML, which the page reads Controls from. It is found with
`qtpaths6`, then `qtpaths`, then Homebrew's `qt` and `~/Qt/6.x/`; if yours is
elsewhere, set `QT_INSTALL_QML`. qmlc is built in ljos-web's own target the
first time it is wanted. A task that lacks something says the exact command.

Everything found has an override: `LJOS_WEB`, `PLAYWRIGHT`, `QMLC`,
`QT_INSTALL_QML`, `LJOS_UI`, `LJOS`, `WASM_BINDGEN`. None is needed in this
layout.

Nothing downloads on its own except `mise run setup`, which installs the
two programs cargo does not build here, into `target/tools`, printing each
command: the `ljos` program at the ljos-ui commit Cargo.toml pins (`mise run
features` and `mise run studio` run previews with it; skipped when `LJOS` is
set or a `../ljos-ui` checkout is beside this one, which they build from), and
the `wasm-bindgen` CLI at the version Cargo.toml pins (`mise run web:build`
and every web task after it). A task that needs one and lacks it says to run
`mise run setup` and the command it would run. `mise run web:browser`,
`web:shell`, `web:serve:check` and `features:browser` drive headless Chromium, which also needs
`(cd ../ljos-web && pnpm --filter ljos-web-example-web exec playwright install chromium)`.

## Status

[docs/DESIGN.md](docs/DESIGN.md) has what each milestone built.

- **S0–S4 built.** The lossless edit seam; headless Studio over MCP; previews
  in a child Ljós process; the canvas, layers, inspector, palette and code
  view; variants, breakpoints, slots, tokens, user shells and the self-reskin.
- **S5 built, as far as ljos-web goes.** The session, its tools and the
  shell run in a browser, with projects in browser storage. The document
  under edit is compiled by ljos-web's `qmlc` and mounted in a frame laid
  over the Canvas panel. 21 of the 39 `@preview` scenarios run in Chromium
  (`mise run features:browser`). The other 18 wait on numbered ljos-web
  requests (screenshots, reading by id, themes, `Ljos.Models`, type
  dumps); the preview refuses the calls it cannot answer as
  `not_supported` (DESIGN.md, S5).
- **S6 built, but for multiplayer.** Prototyping flows, list data, export
  bundles and review comments. Multiplayer research has not started.

The lossless edit engine (`qml_edit`) and the block model live in Ljós itself,
as general pieces any tool can use.

- [docs/DESIGN.md](docs/DESIGN.md): goals, modules and seams, data flow,
  testing, performance budget and milestones.
- [docs/MCP.md](docs/MCP.md): the MCP tool surface.

## Building

A checkout builds on its own: `cargo test --workspace`. Ljós comes from
[ljos-ui](https://github.com/hal-c2/ljos-ui) as a git dependency pinned to
one commit of its main branch (`rev` in `Cargo.toml`); cargo fetches it with
the git command line (`.cargo/config.toml`). `mise.toml` only names tool
versions, so mise runs here without `mise trust`; `mise run setup` adds
Rust's components and the wasm target with rustup; `mise run check` and
`mise run test` (the unit tests and the whole Gherkin ledger, whose previews
run `ljos`) are the gates, and `mise run features` runs the feature files it
is given.

### Building against a local ljos-ui

To change Ljós and Studio together, point cargo at a ljos-ui checkout with
a `[patch]` in a cargo config *outside* this checkout, for example
`../.cargo/config.toml` beside both checkouts (cargo reads configs in
parent directories), so it never gets committed:

```toml
[patch."https://github.com/hal-c2/ljos-ui"]
qml_syntax = { path = "ljos-ui/crates/qml_syntax" }
qml_edit = { path = "ljos-ui/crates/qml_edit" }
ljos_ir = { path = "ljos-ui/crates/ljos_ir" }
ljos_mcp_core = { path = "ljos-ui/crates/ljos_mcp_core" }
ljos_blocks = { path = "ljos-ui/crates/ljos_blocks" }
```

Paths are relative to the directory holding `.cargo/`. List every ljos-ui
crate `Cargo.toml` names. Once the Ljós change is on ljos-ui main, move
every `rev` in `Cargo.toml` to that commit and drop the patch.
