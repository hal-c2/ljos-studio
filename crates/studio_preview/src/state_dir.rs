//! The directory a preview keeps its inspector token and log in.
//!
//! The token grants control of the preview, and the default place is below
//! the shared temporary directory when there is no `XDG_RUNTIME_DIR`. So a
//! directory somebody else made or can reach is refused, not used.

use std::path::Path;

use studio_core::preview::PreviewError;

/// Makes `dir` readable by this user alone, or checks that it already is.
///
/// # Errors
/// It cannot be made, or it exists and is a link, another user's, or open to
/// group or others.
#[cfg(unix)]
pub fn private_dir(dir: &Path) -> Result<(), PreviewError> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};

    let refuse = |why: String| PreviewError::Start(format!("{}: {why}", dir.display()));
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| refuse(e.to_string()))?;
    // Checked after making it, not instead: whichever of two got there
    // first, what is judged is what is there now, and a link is not followed.
    let found = dir.symlink_metadata().map_err(|e| refuse(e.to_string()))?;
    if !found.is_dir() {
        return Err(refuse("not a directory".to_owned()));
    }
    if ljos_mcp_core::effective_uid() != Some(found.uid()) {
        return Err(refuse("it belongs to another user".to_owned()));
    }
    if found.mode() & 0o077 != 0 {
        return Err(refuse(format!(
            "group or others can reach it (mode {:o})",
            found.mode() & 0o777
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn private_dir(dir: &Path) -> Result<(), PreviewError> {
    std::fs::create_dir_all(dir).map_err(|e| PreviewError::Start(format!("{}: {e}", dir.display())))
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    use super::*;

    fn refused(dir: &Path) -> String {
        match private_dir(dir) {
            Err(PreviewError::Start(why)) => why,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_new_directory_and_its_parents_are_this_users_alone() {
        let top = tempfile::tempdir().unwrap();
        let dir = top.path().join("a/b");
        private_dir(&dir).unwrap();
        for made in [dir.as_path(), dir.parent().unwrap()] {
            assert_eq!(made.metadata().unwrap().mode() & 0o777, 0o700, "{made:?}");
        }
        private_dir(&dir).expect("its own directory, again");
    }

    #[test]
    fn a_directory_others_can_reach_is_refused_and_left_as_it_is() {
        let top = tempfile::tempdir().unwrap();
        for mode in [0o750, 0o705, 0o777] {
            let dir = top.path().join(format!("{mode:o}"));
            std::fs::create_dir(&dir).unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(mode)).unwrap();
            assert!(refused(&dir).contains("group or others"), "{mode:o}");
            assert_eq!(dir.metadata().unwrap().mode() & 0o777, mode);
        }
    }

    #[test]
    fn a_link_or_a_file_is_refused() {
        let top = tempfile::tempdir().unwrap();
        let real = top.path().join("real");
        std::fs::create_dir(&real).unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o700)).unwrap();
        let link = top.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(refused(&link).contains("not a directory"));
        let file = top.path().join("file");
        std::fs::write(&file, "").unwrap();
        refused(&file);
    }

    /// `/` is root's. As root nothing here is another user's, so only a test
    /// run by anyone else can see this.
    #[test]
    fn another_users_directory_is_refused() {
        if ljos_mcp_core::effective_uid() == Some(0) {
            return;
        }
        assert!(refused(Path::new("/")).contains("another user"));
    }

    #[test]
    fn a_preview_does_not_start_in_a_directory_others_can_reach() {
        let top = tempfile::tempdir().unwrap();
        let state = top.path().join("state");
        std::fs::create_dir(&state).unwrap();
        std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(top.path().join("Main.qml"), "Item {}").unwrap();
        let mut host = crate::NativeHost::new(top.path(), "/nonexistent/ljos", &state);
        let spec = studio_core::preview::PreviewSpec {
            entry: "Main.qml".to_owned(),
            size: studio_core::preview::Target::Desktop.size(),
            aliases: Vec::new(),
        };
        use studio_core::preview::PreviewHost as _;
        match host.open(&spec) {
            Err(PreviewError::Start(why)) => assert!(why.contains("group or others"), "{why}"),
            other => panic!("{other:?}"),
        }
    }
}
