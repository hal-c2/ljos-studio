# shellcheck shell=bash
# What the tasks find on a person's machine, with nothing set. Sourced, not a
# task (not executable, so mise does not list it). Each environment variable
# named here stays an override. Nothing is downloaded or installed unasked:
# what is missing is an error that says the exact command.
#
# Runs on the bash 3.2 macOS ships and BSD tools: no `readlink -f`, `realpath`,
# `sed -i`, `mapfile`, associative arrays, or an empty array under `set -u`.
# `mise run check:tasks` holds the tasks to that.

studio_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# The ljos-web commit Studio is built against. CI checks out the same one.
ljos_web_pin="$(cat "$studio_root/web/ljos-web.rev")"

# A shell that activated mise and then put a system Rust (a distro's
# /usr/bin/cargo) ahead of it keeps that order under `mise run`: mise does
# not move paths already on PATH. So rustup's proxies, which build with
# rust-toolchain.toml's toolchain, go in just ahead of that other Rust's
# directory, and whatever came before it still does. Directories are
# compared resolved, so `/usr/bin/`, `/usr//bin` or a link to it is the
# same one. mise's own way to do this (`activate_aggressive` in
# `[settings]`) would make mise refuse an untrusted checkout, shims and
# `mise env` included.
rust_first() {
  local proxies other rest entry path="" moved=false
  proxies="$(command -v rustup 2>/dev/null)" || return 0
  other="$(command -v rustc 2>/dev/null)" || return 0
  proxies="$(cd "$(dirname "$proxies")" 2>/dev/null && pwd -P)" || return 0
  other="$(cd "$(dirname "$other")" 2>/dev/null && pwd -P)" || return 0
  [ "$other" != "$proxies" ] || return 0
  rest="$PATH:"
  while [ -n "$rest" ]; do
    entry="${rest%%:*}"
    rest="${rest#*:}"
    if [ "$moved" = false ] && [ -n "$entry" ] && [ "$(cd "$entry" 2>/dev/null && pwd -P)" = "$other" ]; then
      path="$path:$proxies"
      moved=true
    fi
    path="$path:$entry"
  done
  PATH="${path#:}"
  export PATH
}
rust_first

die() {
  printf '%s\n' "$*" >&2
  exit 1
}

# A directory with its links resolved.
physical() {
  (cd "$1" && pwd -P)
}

# A file or directory as an absolute path, from where the task was asked:
# mise runs a task in the project's root, not where the person typed.
absolute() {
  local path="$1"
  case "$path" in
    /*) ;;
    *) path="${MISE_ORIGINAL_CWD:-$PWD}/$path" ;;
  esac
  if [ -d "$path" ]; then
    physical "$path"
  elif [ -e "$path" ]; then
    printf '%s/%s\n' "$(physical "$(dirname "$path")")" "$(basename "$path")"
  else
    die "no such file or directory: $path"
  fi
}

# CARGO_TARGET_DIR as an absolute path, empty when it is unset. Relative,
# it is from here, as cargo takes it for the tasks' own builds; absolute, it
# still names the same place after a build changes directory.
target_root() {
  case "${CARGO_TARGET_DIR:-}" in
    "") ;;
    /*) printf '%s\n' "$CARGO_TARGET_DIR" ;;
    *) printf '%s\n' "$PWD/$CARGO_TARGET_DIR" ;;
  esac
}

own_target() {
  local root
  root="$(target_root)"
  printf '%s\n' "${root:-$studio_root/target}"
}

# The `rev` every ljos-ui crate in Cargo.toml names.
ljos_ui_rev() {
  sed -n 's/.*hal-c2\/ljos-ui", rev = "\([0-9a-f]*\)".*/\1/p' "$studio_root/Cargo.toml" | head -n 1
}

# Sets LJOS_WEB; `ljos_web_shown` is how a message spells it.
find_ljos_web() {
  ljos_web_shown="${LJOS_WEB:-../ljos-web}"
  local dir="${LJOS_WEB:-$studio_root/../ljos-web}"
  if [ ! -d "$dir" ]; then
    die "no ljos-web checkout at $ljos_web_shown. Run, beside this checkout:
  git clone https://github.com/hal-c2/ljos-web ../ljos-web
  (cd ../ljos-web && git checkout $ljos_web_pin && pnpm install --frozen-lockfile)
or name one with LJOS_WEB=/path/to/ljos-web."
  fi
  LJOS_WEB="$(physical "$dir")"
  export LJOS_WEB
  local module
  for module in ljos-web solid-js vite @solidjs/web @playwright/test; do
    if [ ! -e "$LJOS_WEB/examples/web/node_modules/$module/package.json" ]; then
      die "ljos-web has no packages installed: run (cd $ljos_web_shown && pnpm install --frozen-lockfile)"
    fi
  done
  local at
  at="$(git -C "$LJOS_WEB" rev-parse HEAD 2>/dev/null || true)"
  case "$at" in
    "" | "$ljos_web_pin"*) ;;
    *) echo "note: $ljos_web_shown is at ${at%"${at#???????}"}; Studio is built against $ljos_web_pin (web/ljos-web.rev)" >&2 ;;
  esac
}

# Sets PLAYWRIGHT to the module of ljos-web's own Playwright, which is not a
# direct dependency of anything: only pnpm knows where it put it, so node's
# resolution from the package that depends on it is what finds it.
find_playwright() {
  if [ -z "${PLAYWRIGHT:-}" ]; then
    find_ljos_web
    PLAYWRIGHT="$(node -e '
      const { createRequire } = require("node:module");
      const { realpathSync } = require("node:fs");
      const { dirname, join } = require("node:path");
      const from = createRequire(realpathSync(process.argv[1]));
      console.log(join(dirname(from.resolve("playwright/package.json")), "index.mjs"));
    ' "$LJOS_WEB/examples/web/node_modules/@playwright/test/package.json" 2>/dev/null)" ||
      die "ljos-web has no Playwright: run (cd $ljos_web_shown && pnpm install --frozen-lockfile)"
  fi
  export PLAYWRIGHT
  # The only way to know the browser Playwright wants is there is to ask it to
  # start: which of its builds that is depends on its version.
  local said
  if said="$(node --input-type=module -e '
    const { chromium } = await import(process.env.PLAYWRIGHT);
    await (await chromium.launch({ headless: true })).close();
  ' 2>&1)"; then
    return 0
  fi
  case "$said" in
    *"Executable doesn't exist"*)
      die "Playwright's Chromium is not installed: run (cd ${ljos_web_shown:-../ljos-web} && pnpm --filter ljos-web-example-web exec playwright install chromium)"
      ;;
    *) die "Playwright's Chromium does not start: $said" ;;
  esac
}

# Sets QMLC. Built when there is none: in ljos-web's own target, or in a
# directory of its own under CARGO_TARGET_DIR when that is set, so the two
# workspaces never share one target and a checkout gains no target/ of ours.
qmlc_target() {
  if [ -n "${CARGO_TARGET_DIR:-}" ]; then
    printf '%s\n' "$(target_root)/ljos-web"
  else
    printf '%s\n' "$LJOS_WEB/target"
  fi
}

find_qmlc() {
  local built
  built="$(qmlc_target)"
  if [ -z "${QMLC:-}" ]; then
    local profile dir
    # A qmlc the person built in the checkout counts too.
    for dir in "$built" "$LJOS_WEB/target"; do
      for profile in release debug; do
        if [ -z "${QMLC:-}" ] && [ -x "$dir/$profile/qmlc" ]; then
          QMLC="$dir/$profile/qmlc"
        fi
      done
    done
  fi
  if [ -z "${QMLC:-}" ]; then
    echo "ljos-web has no qmlc yet; building it into $built, which takes minutes the first time: (cd $ljos_web_shown && cargo build --bin qmlc)" >&2
    (
      export CARGO_TARGET_DIR="$built"
      cd "$LJOS_WEB" || exit
      LJOS_QT_TYPES="$LJOS_WEB/crates/ljos_web/types.txt" cargo build --bin qmlc
    ) || die "building qmlc failed: run (cd $ljos_web_shown && cargo build --bin qmlc) and see what it says, or name one with QMLC=/path/to/qmlc"
    QMLC="$built/debug/qmlc"
  fi
  [ -x "$QMLC" ] || die "QMLC names $QMLC, which is not a program"
  export QMLC
}

# Qt 6's QML has Controls' Basic style; Qt 5's does not, and Qt 5's `qtpaths`
# is often what is on the path (it does not know `--query` either).
is_qt6_qml() {
  [ -d "$1/QtQuick/Controls/Basic" ]
}

# Where distributions and the installer put it, for when no tool on the path
# says. Globs that match nothing stay as they are, and fail the test.
qt_known_dirs() {
  local dir
  if command -v brew >/dev/null 2>&1; then
    local prefix
    prefix="$(brew --prefix qt 2>/dev/null || true)"
    if [ -n "$prefix" ]; then
      printf '%s\n' "$prefix/share/qt/qml" "$prefix/qml"
    fi
  fi
  for dir in "$HOME"/Qt/6.*/macos/qml "$HOME"/Qt/6.*/gcc_64/qml; do
    printf '%s\n' "$dir"
  done
  qt_system_dirs
}

qt_system_dirs() {
  printf '%s\n' /usr/lib/qt6/qml /usr/lib/x86_64-linux-gnu/qt6/qml /usr/lib/aarch64-linux-gnu/qt6/qml
}

# Sets QT_INSTALL_QML, and puts Qt's tools on the path: ljos-web asks them for
# the pictures a style keeps inside its plugin.
find_qt() {
  local dir tool
  if [ -n "${QT_INSTALL_QML:-}" ]; then
    is_qt6_qml "$QT_INSTALL_QML" || die "QT_INSTALL_QML is $QT_INSTALL_QML, which has no Qt 6 QtQuick/Controls/Basic"
    dir="$QT_INSTALL_QML"
  else
    dir=""
    for tool in qtpaths6 qtpaths; do
      local asked
      asked="$("$tool" --query QT_INSTALL_QML 2>/dev/null || true)"
      if [ -n "$asked" ] && is_qt6_qml "$asked"; then
        dir="$asked"
        break
      fi
    done
    if [ -z "$dir" ]; then
      local known
      while IFS= read -r known; do
        if is_qt6_qml "$known"; then
          dir="$known"
          break
        fi
      done <<EOF
$(qt_known_dirs)
EOF
    fi
  fi
  if [ -z "$dir" ]; then
    die "Qt 6's QML not found: neither qtpaths6 nor qtpaths --query QT_INSTALL_QML names one with QtQuick/Controls/Basic.
Point QT_INSTALL_QML at it:
  Homebrew (brew install qt): export QT_INSTALL_QML=\"\$(brew --prefix qt)/share/qt/qml\"
    (or \"\$(brew --prefix qt)/qml\", whichever exists)
  Qt online installer: export QT_INSTALL_QML=~/Qt/6.x/macos/qml (gcc_64 on Linux)
  Debian, Ubuntu: apt install qml6-module-qtquick-controls qt6-declarative-dev, then /usr/lib/x86_64-linux-gnu/qt6/qml"
  fi
  QT_INSTALL_QML="$dir"
  export QT_INSTALL_QML
  if [ -x "$dir/../bin/qtpaths6" ] || [ -x "$dir/../bin/qml" ]; then
    PATH="$(physical "$dir/../bin"):$PATH"
    export PATH
  fi
}

# Where Ljós's design module is: in the checkout LJOS_UI names, else wherever
# cargo has the ljos-ui that Cargo.toml pins (or a [patch] points at).
find_design_qml() {
  if [ -n "${LJOS_UI:-}" ]; then
    LJOS_DESIGN="$LJOS_UI/crates/ljos_design_qml/qml"
  else
    # Only this platform's: the rest of the graph need not be downloaded.
    local host
    host="$(rustc -vV | sed -n 's/^host: //p')"
    LJOS_DESIGN="$(cd "$studio_root" && cargo metadata --format-version 1 --filter-platform "$host" | node -e '
      const { dirname, join } = require("node:path");
      let text = "";
      process.stdin.on("data", (d) => (text += d)).on("end", () => {
        const found = JSON.parse(text).packages.find((p) => p.name === "ljos_design_qml");
        if (!found) throw new Error("no package ljos_design_qml in the graph");
        console.log(join(dirname(found.manifest_path), "qml"));
      });
    ')" || die "cargo metadata did not say where ljos_design_qml is; name a ljos-ui checkout with LJOS_UI=/path/to/ljos-ui"
  fi
  [ -d "$LJOS_DESIGN" ] || die "no design module at $LJOS_DESIGN"
  export LJOS_DESIGN
}

# The `cargo install` that puts the ljos program of the pinned commit in our
# target's tools, and the file that says which commit is there.
ljos_install_command() {
  printf 'cargo install --git https://github.com/hal-c2/ljos-ui --rev %s --locked --force ljos_cli --root %s\n' "$(ljos_ui_rev)" "$(own_target)/tools"
}

# Sets LJOS, the `ljos` previews run: as named; else built from a ljos-ui
# checkout beside this one; else the one `mise run setup` installed from the
# commit Cargo.toml pins. Nothing is downloaded here: a missing program is an
# error that names `mise run setup` and the command it runs.
find_ljos() {
  if [ -n "${LJOS:-}" ]; then
    export LJOS
    return 0
  fi
  local sibling="${LJOS_UI:-$studio_root/../ljos-ui}"
  if [ -f "$sibling/Cargo.toml" ]; then
    # Ljós's own workspace, so its own target: under CARGO_TARGET_DIR, a
    # directory of its own, or the two workspaces would rebuild each other's
    # dependencies with different features on every switch.
    local built="$sibling/target"
    [ -z "${CARGO_TARGET_DIR:-}" ] || built="$(target_root)/ljos-ui"
    echo "building ljos (ljos-ui's ljos_cli, debug) from ${LJOS_UI:-../ljos-ui} into $built: the first build compiles all of Ljós and takes many minutes; later ones only what changed. LJOS=/path/to/ljos skips it" >&2
    CARGO_TARGET_DIR="$built" cargo build --manifest-path "$sibling/Cargo.toml" -p ljos_cli --bin ljos
    LJOS="$built/debug/ljos"
    export LJOS
    return 0
  fi
  local rev tools
  rev="$(ljos_ui_rev)"
  [ -n "$rev" ] || die "Cargo.toml names no ljos-ui rev, and no ljos-ui checkout is beside this one; name a program with LJOS=/path/to/ljos"
  tools="$(own_target)/tools"
  # Stamped with the commit, so a moved pin asks for setup again.
  if [ ! -x "$tools/bin/ljos" ] || [ "$(cat "$tools/ljos.rev" 2>/dev/null || true)" != "$rev" ]; then
    die "no ljos program for ljos-ui $rev (no LJOS, no ../ljos-ui): run mise run setup, which runs
  $(ljos_install_command)
or name one with LJOS=/path/to/ljos."
  fi
  LJOS="$tools/bin/ljos"
  export LJOS
}

# What `mise run setup` does for ljos: install it unless the pin's is there.
install_ljos() {
  local rev tools
  rev="$(ljos_ui_rev)"
  [ -n "$rev" ] || die "Cargo.toml names no ljos-ui rev"
  tools="$(own_target)/tools"
  if [ -x "$tools/bin/ljos" ] && [ "$(cat "$tools/ljos.rev" 2>/dev/null || true)" = "$rev" ]; then
    echo "ljos for ljos-ui $rev is already in $tools"
    return 0
  fi
  echo "$(ljos_install_command)"
  cargo install --git https://github.com/hal-c2/ljos-ui --rev "$rev" --locked --force ljos_cli --root "$tools"
  printf '%s\n' "$rev" >"$tools/ljos.rev"
}

# The wasm-bindgen version Cargo.toml pins; the CLI must be the same one.
bindgen_version() {
  sed -n 's/^wasm-bindgen = "=\(.*\)"$/\1/p' "$studio_root/Cargo.toml"
}

bindgen_install_command() {
  printf 'cargo install wasm-bindgen-cli --version =%s --locked --root %s\n' "$(bindgen_version)" "$(own_target)/tools"
}

# Sets WASM_BINDGEN: as named, else on PATH, else in our tools, if it is the
# pinned version; otherwise an error naming `mise run setup`.
find_bindgen() {
  local want candidate
  want="$(bindgen_version)"
  for candidate in "${WASM_BINDGEN:-}" wasm-bindgen "$(own_target)/tools/bin/wasm-bindgen"; do
    [ -n "$candidate" ] || continue
    if [ "$("$candidate" --version 2>/dev/null || true)" = "wasm-bindgen $want" ]; then
      WASM_BINDGEN="$candidate"
      export WASM_BINDGEN
      return 0
    fi
    # A named one that is not the pinned version is not passed over.
    if [ "$candidate" = "${WASM_BINDGEN:-}" ]; then
      die "WASM_BINDGEN=$candidate is not wasm-bindgen $want"
    fi
  done
  die "no wasm-bindgen $want: run mise run setup, which runs
  $(bindgen_install_command)
or name one with WASM_BINDGEN=/path/to/wasm-bindgen."
}

install_bindgen() {
  local want
  want="$(bindgen_version)"
  if [ "$("$(own_target)/tools/bin/wasm-bindgen" --version 2>/dev/null || true)" = "wasm-bindgen $want" ]; then
    echo "wasm-bindgen $want is already in $(own_target)/tools"
    return 0
  fi
  echo "$(bindgen_install_command)"
  cargo install wasm-bindgen-cli --version "=$want" --locked --root "$(own_target)/tools"
}

# One of rust-toolchain.toml's lists (`components`, `targets`), spaced.
toolchain_list() {
  sed -n "s/^$1 *= *\\[\\(.*\\)\\]/\\1/p" "$studio_root/rust-toolchain.toml" | tr -d '"' | tr ',' ' '
}

# Empty when the rustc on PATH is rust-toolchain.toml's channel; otherwise
# why not. rustup's parts land in its toolchain, so a distro Rust ahead of
# rustup's proxies on PATH (mise puts its rust's bin nowhere: it leans on
# those proxies) builds with none of them, whatever setup added.
rust_channel() {
  sed -n 's/^channel *= *"\(.*\)"/\1/p' "$studio_root/rust-toolchain.toml"
}

wrong_rustc() {
  local channel have at
  channel="$(rust_channel)"
  have="$(rustc --version 2>/dev/null || true)"
  case "$have" in
    "rustc $channel" | "rustc $channel "*) return 0 ;;
  esac
  at="$(command -v rustc || true)"
  if [ -z "$at" ]; then
    echo "no rustc is on PATH: rustup's proxies (~/.cargo/bin) must be on it"
    return 0
  fi
  echo "the rustc on PATH ($at) is ${have:-not working}, not $channel: rustup's proxies (~/.cargo/bin) must come before $(dirname "$at") on PATH"
}

no_rustup() {
  echo "no rustup on PATH: Studio's Rust (rust-toolchain.toml) and its wasm target come through rustup. Install it (https://rustup.rs: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh), open a new shell, then run mise run setup again"
}

# rust-toolchain.toml's `$1` (components or targets) that the toolchain in
# use lacks, spaced; empty when all are there. Fails when rustup cannot
# list them, so that is never taken for nothing missing.
missing_rust_parts() {
  local installed part missing=""
  if [ "$1" = components ]; then
    installed="$(rustup component list --installed 2>/dev/null)" || return 1
  else
    installed="$(rustup target list --installed 2>/dev/null)" || return 1
  fi
  for part in $(toolchain_list "$1"); do
    case "
$installed
" in
      *"
$part
"* | *"
$part-"*) ;;
      *) missing="$missing $part" ;;
    esac
  done
  printf '%s\n' "${missing# }"
}

# rust-toolchain.toml's components and targets for the toolchain in use:
# under mise, RUSTUP_TOOLCHAIN overrides that file, so rustup installs none
# of them. rustup skips what is there.
install_rust_parts() {
  command -v rustup >/dev/null || die "$(no_rustup)"
  local components targets
  components="$(toolchain_list components)"
  targets="$(toolchain_list targets)"
  echo "rustup component add" $components
  rustup component add $components
  echo "rustup target add" $targets
  rustup target add $targets
}

# Dies naming setup when the toolchain in use lacks one of
# rust-toolchain.toml's `$1` (components or targets). Without rustup there
# is nothing to ask, and cargo says what is missing.
need_rust_parts() {
  command -v rustup >/dev/null || return 0
  local installed part
  if [ "$1" = components ]; then
    installed="$(rustup component list --installed 2>/dev/null)" ||
      die "rustup cannot list this Rust's ${1}: run mise run setup"
  else
    installed="$(rustup target list --installed 2>/dev/null)" ||
      die "rustup cannot list this Rust's ${1}: run mise run setup"
  fi
  for part in $(toolchain_list "$1"); do
    case "
$installed
" in
      *"
$part
"* | *"
$part-"*) ;;
      *) die "no $part for this Rust: run mise run setup (it runs rustup $([ "$1" = components ] && echo component || echo target) add $part)" ;;
    esac
  done
}

# The shell as an ljos-web page, assembled in the target (nothing is written
# into ljos-web): `web/shell`'s files, or `$2`'s over them, with
# `node_modules` linked to the checkout's. Sets `app`.
assemble_page() {
  app="$(own_target)/$1"
  rm -rf "$app"
  mkdir -p "$app/qml" "$app/node_modules/@solidjs"
  cp "$studio_root"/web/shell/{index.html,main.js,vite.config.js} "$app/"
  if [ -n "${2:-}" ]; then
    cp "$studio_root/web/$2"/* "$app/"
  fi
  # (Icon.qml imports icons.js, so the scripts go with the QML.)
  # The built-in shell, in a window of the page's own: the native window
  # loads a person's shell through `Ljos.Blocks`, which ljos-web lacks.
  cp "$studio_root"/shell/Builtin/*.{qml,js} "$studio_root"/web/shell/Window.qml "$app/qml/"
  cp -r "$studio_root/web/standins" "$studio_root/web/pkg" "$app/"
  cp "$studio_root"/web/{store,studio,Workspace,page,preview,compiler}.js "$app/"
  cp "$studio_root/samples/hello/Main.qml" "$app/sample.txt"
  # What compiled QML and the page import, from the checkout: its runtime,
  # and the Solid and Vite the runtime is built with.
  local modules="$LJOS_WEB/examples/web/node_modules" module
  for module in ljos-web solid-js vite @solidjs/web; do
    ln -s "$(physical "$modules/$module")" "$app/node_modules/$module"
  done
  export LJOS_QT_TYPES="${LJOS_QT_TYPES:-$LJOS_WEB/crates/ljos_web/types.txt}"
}

# The page `web:serve` serves, built. Sets `app`.
build_served_page() {
  find_ljos_web
  find_qmlc
  find_qt
  find_design_qml
  assemble_page web-serve serve
  (cd "$app" && VITE_CONFIG_NATIVE_IGNORE_WARNING=true node_modules/vite/bin/vite.js build --logLevel warn)
}

# A path the person typed, absolute from where they asked; unlike `absolute`
# it need not exist yet.
from_asked() {
  case "$1" in
    /*) printf '%s\n' "$1" ;;
    *) printf '%s/%s\n' "${MISE_ORIGINAL_CWD:-$PWD}" "$1" ;;
  esac
}

# A program as a person names it: with a slash it is a path, from where they
# asked; a bare name like `ljos` is looked up on PATH when it runs, as a shell would.
program_asked() {
  case "$1" in
    */*) from_asked "$1" ;;
    *) printf '%s\n' "$1" ;;
  esac
}

# mise strips `--` and hands the first word after it to [dir]: in
# `mise run dev -- --mcp` that is studio's flag, not a project. Moves a `dir`
# that starts with `-` to the front of `decoded`, the arguments for studio.
# A project typed among them would reach studio as a second DIR, which it
# refuses only after a release build; said here instead. Any word that is
# neither a flag nor the value of one of studio's flags that take a value
# (crates/studio/src/main.rs) is that project.
flag_not_dir() {
  case "$dir" in
    -*) ;;
    *) return 0 ;;
  esac
  decoded=("$dir" ${decoded[@]+"${decoded[@]}"})
  dir=""
  local word value=false project="" rest=""
  for word in "${decoded[@]}"; do
    if [ "$value" = true ]; then
      value=false
    else
      case "$word" in
        --caps | --ljos | --shell | --theme) value=true ;;
        -*) ;;
        *)
          if [ -z "$project" ]; then
            project="$word"
            continue
          fi
          ;;
      esac
    fi
    rest="$rest $word"
  done
  [ -z "$project" ] ||
    die "$project looks like the project: DIR goes before --, as in mise run ${MISE_TASK_NAME:-studio} $project --$rest"
}

# mise hands a variadic argument over as one shell-quoted string; this sets
# `decoded` to its words, quoting undone, so a path with spaces stays one.
# The string is mise's quoting of what the person typed, so eval only undoes
# that quoting. mise leaves braces unquoted, so brace expansion is off for it.
decode_args() {
  decoded=()
  [ -z "$1" ] || {
    set +B
    eval "decoded=($1)"
    set -B
  }
}

# What `setup` and `dev` share with `studio`: the window on DIR (a copy of
# samples/hello when empty), the rest of the arguments going to studio.
run_studio() {
  local dir="$1"
  shift
  case "$dir" in
    -*) die "$dir is a flag, not a DIR: studio's flags go after DIR and --, or name the flag the task's --help lists" ;;
  esac
  find_ljos
  if [ -n "$dir" ]; then
    dir="$(absolute "$dir")"
  else
    # A copy: a person trying Studio edits it, and the sample stays as it is.
    dir="$(own_target)/studio-sample"
    if [ ! -d "$dir" ]; then
      mkdir -p "$(dirname "$dir")"
      cp -R "$studio_root/samples/hello" "$dir"
    fi
    echo "No DIR given: opening a copy of samples/hello in $dir" >&2
  fi
  exec cargo run --release -q -p studio --bin studio -- "$@" "$dir"
}

# --- ljos-web, for `mise run setup` -------------------------------------------
# Setup clones ljos-web only when asked (`--web`) and only when nothing is
# there: a checkout that exists is somebody's, so it is read and reported on,
# never changed.

ljos_web_dir() {
  printf '%s\n' "${LJOS_WEB:-$studio_root/../ljos-web}"
}

# Clones ljos-web at the pin beside this checkout, and installs its packages
# and Playwright's Chromium with its own pinned node and pnpm. ljos-web's
# mise.toml sets tool options, so mise wants it trusted: trusted for these
# commands only, through the environment, so nothing in mise's state changes.
clone_ljos_web() {
  local dir partial owned
  dir="$(ljos_web_dir)"
  [ ! -e "$dir" ] || die "$dir is already there: setup does not change a checkout it did not make"
  # Made under another name and moved into place only when whole, so a run
  # that fails leaves no half checkout that the next run would take as
  # somebody's. The marker beside it, written before the clone starts, is how
  # a later run knows a partial one is setup's own to remove; one without it
  # is somebody's, and stays.
  partial="$dir.setup-partial"
  owned="$partial.by-setup"
  if [ -e "$partial" ]; then
    [ -f "$owned" ] || die "$partial is there and setup did not make it: move it away, then run setup --web again"
    echo "removing the partial clone an earlier setup left: $partial"
    rm -rf "$partial"
  fi
  : >"$owned"
  echo "git clone https://github.com/hal-c2/ljos-web $dir"
  git clone -q https://github.com/hal-c2/ljos-web "$partial"
  # The pin need not be on a branch a clone fetches (CI's checkout fetches it
  # by its hash too), so it is fetched by name.
  echo "git -C $dir fetch origin $ljos_web_pin"
  git -C "$partial" fetch -q origin "$ljos_web_pin"
  echo "git -C $dir checkout $ljos_web_pin"
  git -C "$partial" -c advice.detachedHead=false checkout -q "$ljos_web_pin"
  setup_ljos_web_packages "$partial" "$dir"
  mv "$partial" "$dir"
  rm -f "$owned"
}

# $1: the checkout to install into; $2: how messages name it.
setup_ljos_web_packages() {
  local dir shown="$2"
  dir="$(physical "$1")"
  echo "(cd $shown && mise install && pnpm install --frozen-lockfile)"
  (
    cd "$dir" || exit
    export MISE_TRUSTED_CONFIG_PATHS="$dir"
    mise install
    mise exec -- pnpm install --frozen-lockfile
    echo "(cd $shown && pnpm --filter ljos-web-example-web exec playwright install chromium)"
    mise exec -- pnpm --filter ljos-web-example-web exec playwright install chromium
  )
}

# One line per thing the web tasks need from ljos-web, each with the command
# that gets it; prints to stdout, returns 1 when something is missing.
report_ljos_web() {
  local dir missing=0 said
  dir="$(ljos_web_dir)"
  if [ ! -d "$dir" ]; then
    echo "SKIPPED ljos-web: no checkout at ${LJOS_WEB:-../ljos-web}; the native window does not need it. For the browser tasks run mise run setup --web (it clones it at $ljos_web_pin and installs its packages), or name one with LJOS_WEB=/path/to/ljos-web"
    return 1
  fi
  if said="$(find_ljos_web 2>&1)"; then
    echo "found ljos-web at $(physical "$dir")${said:+ ($said)}"
  else
    echo "SKIPPED ljos-web's packages: $said"
    return 1
  fi
  if said="$( (find_playwright) 2>&1)"; then
    echo "found Playwright's Chromium"
  else
    echo "SKIPPED Playwright: $said (web:browser, web:shell, web:serve:check and features:browser need it)"
    missing=1
  fi
  return $missing
}

# The Qt CI installs (install-qt-action's version in
# .github/workflows/ci.yml), the one place it is written down. Its minor is
# the pin: the shell's Controls differ from one minor to the next.
qt_pin() {
  awk '/install-qt-action/ { on = 1 }
    on && $1 == "version:" { gsub(/"/, "", $2); print $2; exit }' \
    "$studio_root/.github/workflows/ci.yml" 2>/dev/null || true
}

# `qt_version <qml dir>`: "<version> <tool>" for the Qt that owns <qml dir>,
# or nothing. The tools in its own bin directory are asked first. A tool on
# PATH counts only when it names <qml dir> as its QT_INSTALL_QML: a QML tree
# with no bin beside it (a QML-only copy, a share/qt/qml) says nothing of
# whichever Qt PATH holds. Qt 5's unsuffixed tools answer too, so an answer
# counts only when it is 6.
qt_version() {
  local qml bin tool out owner tools=()
  qml="$(physical "$1" 2>/dev/null)" || return 0
  if bin="$(physical "$qml/../bin" 2>/dev/null)"; then
    tools=("$bin/qtpaths6" "$bin/qtpaths" "$bin/qmake6" "$bin/qmake" "$bin/qml6" "$bin/qml")
  fi
  tools+=(qtpaths6 qtpaths qmake6 qmake)
  for tool in "${tools[@]}"; do
    command -v "$tool" >/dev/null 2>&1 || continue
    case "$tool" in
      */*) ;;
      qtpaths*)
        owner="$("$tool" --query QT_INSTALL_QML 2>/dev/null)" || continue
        [ "$(physical "$owner" 2>/dev/null)" = "$qml" ] || continue
        ;;
      *)
        owner="$("$tool" -query QT_INSTALL_QML 2>/dev/null)" || continue
        [ "$(physical "$owner" 2>/dev/null)" = "$qml" ] || continue
        ;;
    esac
    case "${tool##*/}" in
      qtpaths*) out="$("$tool" --qt-version 2>/dev/null)" || continue ;;
      qmake*) out="$("$tool" -query QT_VERSION 2>/dev/null)" || continue ;;
      # qml makes its GUI application before it reads --version.
      *) out="$(QT_QPA_PLATFORM=offscreen "$tool" --version 2>/dev/null)" || continue ;;
    esac
    [[ "$out" =~ ([0-9]+\.[0-9]+\.[0-9]+) ]] || continue
    case "${BASH_REMATCH[1]}" in
      6.*)
        printf '%s %s\n' "${BASH_REMATCH[1]}" "$(command -v "$tool")"
        return 0
        ;;
    esac
  done
}

# A Qt of another minor than the pin is reported as MISMATCH, which setup
# counts as missing with or without --web: unlike a Qt that is not there,
# it is taken without a word, and web:serve and web:shell then show another
# Qt's Controls.
report_qt() {
  local said pin found version tool
  if said="$( (find_qt && echo "$QT_INSTALL_QML") 2>&1)"; then
    echo "found Qt 6's QML at $said"
  else
    echo "SKIPPED Qt 6: web:serve and web:shell read Controls from it. $said"
    return 1
  fi
  pin="$(qt_pin)"
  [ -n "$pin" ] || return 0
  found="$(qt_version "$said")"
  [ -n "$found" ] || return 0
  version="${found%% *}"
  tool="${found#* }"
  case "$version" in "${pin%.*}".*) return 0 ;; esac
  local host=gcc_64
  [ "$(uname -s)" != Darwin ] || host=macos
  echo "MISMATCH Qt $version ($tool) is not the pinned Qt ${pin%.*} (.github/workflows/ci.yml installs $pin), so web:serve and web:shell would read another Qt's Controls: install Qt $pin (Qt's online installer, or aqt install-qt), then export QT_INSTALL_QML=~/Qt/$pin/$host/qml"
}

# --- measure-lookup and measure-members ----------------------------------------

# Both ask Qt's own runtime and write what it says into tracked tables, so
# each needs qml6 and python3 before anything starts.
need_measure_tools() {
  command -v python3 >/dev/null 2>&1 ||
    die "python3 is not on PATH: install Python 3 (the measuring scripts in mise-tasks/ are Python)"
  command -v qml6 >/dev/null 2>&1 ||
    die "qml6, Qt 6's QML runtime, is not on PATH: install Qt 6's declarative tools (Arch: pacman -S qt6-declarative; Debian, Ubuntu: apt install qml6-module-qtquick qt6-declarative-dev-tools), or put a link named qml6 to Qt's bin/qml on PATH"
  # Off-screen: no window opens, and a platform theme set for the desktop
  # (gtk3) would still try to open a display and kill qml6 without one.
  export QT_QPA_PLATFORM=offscreen
  unset QT_QPA_PLATFORMTHEME
}

# `measured <check> <scratch> <tracked file>…`, after a measuring script
# wrote each tracked file's new text to <scratch>/new/<name>: without
# --check the tracked files are replaced and each one is named, with whether
# it changed; with --check nothing is written, and a difference fails.
measured() {
  local check="$1" scratch="$2" file new drift=0
  shift 2
  for file in "$@"; do
    new="$scratch/new/$(basename "$file")"
    [ -s "$new" ] || die "the measuring script wrote nothing for $file"
    if [ "$check" = true ]; then
      if ! diff -u "$file" "$new"; then
        echo "$file is not what qml6 says now (the difference is above)" >&2
        drift=1
      fi
    elif cmp -s "$file" "$new"; then
      echo "wrote $file (unchanged)"
    else
      cp "$new" "$file"
      echo "wrote $file (changed: see git diff $file)"
    fi
  done
  if [ "$drift" -ne 0 ]; then
    die "${MISE_TASK_NAME:-measure} --check: run mise run ${MISE_TASK_NAME:-measure} to write what qml6 says, and commit it"
  fi
  if [ "$check" = true ]; then
    echo "$* match what qml6 $(qml6 --version 2>&1 | sed -n 's/^Qml Runtime //p' | head -n 1) says"
  fi
}
