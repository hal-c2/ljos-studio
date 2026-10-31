//! The shell Studio ships, as text: what a watched window compiles from
//! the disk beneath the person's own, and what a person's shell starts as.
//!
//! `Window.qml` shows the person's `Shell.qml` through a `ShellLoader`, or
//! `Builtin/Shell.qml` when they have none or theirs does not work. The
//! same texts are compiled ahead of time into the program (the `studio`
//! build script), so the copy written here is the shell the program runs.

use std::path::{Path, PathBuf};

/// The window, by its path from the shell's root.
pub const WINDOW: (&str, &str) = ("Window.qml", include_str!("../../../shell/Window.qml"));

/// Studio's own shell, by its path from `Builtin/`.
pub const BUILTIN: &[(&str, &str)] = &[
    (
        "CanvasPanel.qml",
        include_str!("../../../shell/Builtin/CanvasPanel.qml"),
    ),
    (
        "CodePanel.qml",
        include_str!("../../../shell/Builtin/CodePanel.qml"),
    ),
    (
        "DocumentsPanel.qml",
        include_str!("../../../shell/Builtin/DocumentsPanel.qml"),
    ),
    (
        "FlowPanel.qml",
        include_str!("../../../shell/Builtin/FlowPanel.qml"),
    ),
    (
        "InspectorPanel.qml",
        include_str!("../../../shell/Builtin/InspectorPanel.qml"),
    ),
    (
        "LayersPanel.qml",
        include_str!("../../../shell/Builtin/LayersPanel.qml"),
    ),
    (
        "PalettePanel.qml",
        include_str!("../../../shell/Builtin/PalettePanel.qml"),
    ),
    (
        "Panel.qml",
        include_str!("../../../shell/Builtin/Panel.qml"),
    ),
    (
        "Shell.qml",
        include_str!("../../../shell/Builtin/Shell.qml"),
    ),
    (
        "TokensPanel.qml",
        include_str!("../../../shell/Builtin/TokensPanel.qml"),
    ),
    (
        "Toolbar.qml",
        include_str!("../../../shell/Builtin/Toolbar.qml"),
    ),
];

/// Writes the window and Studio's shell below `root`, as `shell/` is in
/// Studio's sources.
///
/// # Errors
/// A directory or file cannot be written.
pub fn write(root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(root.join("Builtin"))?;
    std::fs::write(root.join(WINDOW.0), WINDOW.1)?;
    for (name, text) in BUILTIN {
        std::fs::write(root.join("Builtin").join(name), text)?;
    }
    Ok(())
}

/// Writes a copy of Studio's shell into `dir`, a person's own to change:
/// `dir/Shell.qml` and the panels beside it. A file already there is
/// kept.
///
/// # Errors
/// The directory or a file cannot be written.
pub fn seed(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, text) in BUILTIN {
        let file = dir.join(name);
        if !file.exists() {
            std::fs::write(file, text)?;
        }
    }
    Ok(())
}

/// `file` as the `file://` URL a `ShellLoader`'s source takes, absolute.
///
/// # Errors
/// The current directory, which a relative `file` is read against, is
/// unknown.
pub fn url(file: &Path) -> std::io::Result<String> {
    let file: PathBuf = std::path::absolute(file)?;
    Ok(format!("file://{}", file.display()))
}

#[cfg(test)]
mod tests {
    /// A panel added to `shell/Builtin` and not here would be missing from
    /// the copy a watched window compiles, and from a person's new shell.
    #[test]
    fn builtin_lists_every_file_of_the_shell() {
        // Cargo names the crate's directory to the running test; the
        // shared compile cache may hand a binary built in another clone.
        let crate_dir = std::env::var("CARGO_MANIFEST_DIR")
            .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
        let dir = format!("{crate_dir}/../../shell/Builtin");
        let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        on_disk.sort();
        let listed: Vec<&str> = super::BUILTIN.iter().map(|(name, _)| *name).collect();
        assert_eq!(on_disk, listed);
    }
}
