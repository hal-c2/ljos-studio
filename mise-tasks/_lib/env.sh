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

own_target() {
  printf '%s\n' "${CARGO_TARGET_DIR:-$studio_root/target}"
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

# Sets QMLC. Built when the checkout has none: that writes only in ljos-web's
# own target, whatever CARGO_TARGET_DIR says for ours.
find_qmlc() {
  if [ -z "${QMLC:-}" ]; then
    local profile
    for profile in release debug; do
      if [ -x "$LJOS_WEB/target/$profile/qmlc" ]; then
        QMLC="$LJOS_WEB/target/$profile/qmlc"
        break
      fi
    done
  fi
  if [ -z "${QMLC:-}" ]; then
    echo "ljos-web has no qmlc yet; building it: (cd $ljos_web_shown && cargo build --bin qmlc)" >&2
    (
      unset CARGO_TARGET_DIR
      cd "$LJOS_WEB"
      LJOS_QT_TYPES="$LJOS_WEB/crates/ljos_web/types.txt" cargo build --bin qmlc
    ) || die "building qmlc failed: run (cd $ljos_web_shown && cargo build --bin qmlc) and see what it says, or name one with QMLC=/path/to/qmlc"
    QMLC="$LJOS_WEB/target/debug/qmlc"
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
    cargo build --manifest-path "$sibling/Cargo.toml" -p ljos_cli --bin ljos
    LJOS="${CARGO_TARGET_DIR:-$sibling/target}/debug/ljos"
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

# rust-toolchain.toml's components and targets for the toolchain in use:
# under mise, RUSTUP_TOOLCHAIN overrides that file, so rustup installs none
# of them. rustup skips what is there.
install_rust_parts() {
  command -v rustup >/dev/null || die "no rustup: Studio's Rust (rust-toolchain.toml) is installed through rustup"
  local components targets
  components="$(toolchain_list components)"
  targets="$(toolchain_list targets)"
  echo "rustup component add" $components
  rustup component add $components
  echo "rustup target add" $targets
  rustup target add $targets
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
  # The built-in shell, in a window of the page's own: the native window
  # loads a person's shell through `Ljos.Blocks`, which ljos-web lacks.
  cp "$studio_root"/shell/Builtin/*.qml "$studio_root"/web/shell/Window.qml "$app/qml/"
  cp -r "$studio_root/web/standins" "$studio_root/web/pkg" "$app/"
  cp "$studio_root"/web/{store,studio,Workspace,page,preview}.js "$app/"
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
