# Ljós Studio

A visual designer for Ljós applications and ljos-web sites, where the
design is the application's own QML. People and agents drive it alike:
every capability is an MCP tool.

- [docs/DESIGN.md](docs/DESIGN.md): goals, modules and seams, data flow,
  testing, performance budget and milestones.
- [docs/MCP.md](docs/MCP.md): the MCP tool surface.

Status: design (milestone S0). The lossless edit engine (`qml_edit`) and
the block model live in Ljós itself, as general pieces any tool can use.

## Building

A checkout builds on its own: `cargo test --workspace`. Ljós comes from
[ljos-ui](https://github.com/hal-c2/ljos-ui) as a git dependency pinned to
one commit of its main branch (`rev` in `Cargo.toml`); cargo fetches it with
the git command line (`.cargo/config.toml`). `mise.toml` only names tool
versions, so mise runs here without `mise trust`; `mise run check`,
`mise run test` and `mise run features` are the gates.

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
