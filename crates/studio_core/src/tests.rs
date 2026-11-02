use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use super::*;

const MAIN: &str = "import QtQuick

// The window's content.
Item {
    id: root
    width: 200 // wide enough

    Rectangle { color: \"red\" }
    Text {
        text: \"hello\"
    }
}
";

fn session() -> Session {
    Session::open(Box::new(MemStore::new([(
        "Main.qml".to_owned(),
        MAIN.to_owned(),
    )])))
}

fn session_of(text: &str) -> Session {
    Session::open(Box::new(MemStore::new([(
        "Main.qml".to_owned(),
        text.to_owned(),
    )])))
}

fn insert(text: &str, index: Option<usize>) -> Op {
    Op::Insert {
        parent: Address::Id("root".into()),
        index,
        text: text.into(),
        add_id: true,
        import: None,
    }
}

#[test]
fn an_inserted_object_gets_an_id_and_a_handle() {
    let mut s = session();
    let out = s
        .edit(
            "Main.qml",
            None,
            &insert("Rectangle { color: \"blue\" }", Some(0)),
            false,
        )
        .unwrap();
    let (text, _) = s.text("Main.qml").unwrap();
    assert!(
        text.contains("Rectangle { id: rectangle1; color: \"blue\" }"),
        "{text}"
    );
    let (_, node) = s
        .resolve("Main.qml", &Address::Node(out.node.unwrap()))
        .unwrap();
    assert_eq!(node.id.as_deref(), Some("rectangle1"));
}

#[test]
fn undo_gives_back_the_bytes() {
    let mut s = session();
    s.edit("Main.qml", None, &insert("Rectangle {}", None), false)
        .unwrap();
    // A path is true of one text: an edit by path names its generation.
    let g = now(&mut s);
    s.edit(
        "Main.qml",
        g,
        &Op::SetBinding {
            node: Address::Path("root/1".into()),
            name: "font.bold".into(),
            value: "true".into(),
        },
        false,
    )
    .unwrap();
    let g = now(&mut s);
    s.edit(
        "Main.qml",
        g,
        &Op::Move {
            node: Address::Path("root/0".into()),
            parent: Address::Id("root".into()),
            index: None,
        },
        false,
    )
    .unwrap();
    for _ in 0..3 {
        s.undo().unwrap();
    }
    assert_eq!(s.text("Main.qml").unwrap().0, MAIN);
    assert_eq!(s.store().read("Main.qml").unwrap(), MAIN);
    assert!(matches!(s.undo(), Err(Error::NothingTo("undo"))));
    s.redo().unwrap();
    assert!(s.text("Main.qml").unwrap().0.contains("rectangle1"));
}

#[test]
fn a_stale_generation_by_path_is_refused_and_changes_nothing() {
    let mut s = session();
    let g = s.generation("Main.qml").unwrap();
    s.edit("Main.qml", Some(g), &insert("Item {}", None), false)
        .unwrap();
    let by_path = Op::SetBinding {
        node: Address::Path("root".into()),
        name: "width".into(),
        value: "3".into(),
    };
    let err = s.edit("Main.qml", Some(g), &by_path, false).unwrap_err();
    assert!(matches!(err, Error::Stale { given, current, .. } if given == g && current == g + 1));
    assert_eq!(s.history().0.len(), 1);
}

#[test]
fn an_older_generation_by_id_is_rebased_past_another_writers_insert() {
    let mut s = session();
    let g = s.generation("Main.qml").unwrap();
    s.act_as("Ada", 0);
    s.edit("Main.qml", Some(g), &insert("Item {}", None), false)
        .unwrap();
    s.act_as("agent", 0);
    let out = s
        .edit("Main.qml", Some(g), &insert("Item {}", None), false)
        .unwrap();
    assert_eq!(out.rebased, Some(g));
    assert_eq!(out.generation, g + 2);
    let (done, _) = s.history();
    let authors: Vec<&str> = done.iter().map(|e| e.author.as_str()).collect();
    assert_eq!(authors, ["Ada", "agent"]);
}

#[test]
fn another_writers_change_of_the_binding_is_a_conflict() {
    let mut s = session();
    let g = s.generation("Main.qml").unwrap();
    let set = |value: &str| Op::SetBinding {
        node: Address::Id("root".into()),
        name: "width".into(),
        value: value.into(),
    };
    s.act_as("Ada", 0);
    s.edit("Main.qml", Some(g), &set("3"), false).unwrap();
    s.act_as("agent", 0);
    let err = s.edit("Main.qml", Some(g), &set("4"), false).unwrap_err();
    assert!(
        matches!(&err, Error::Conflict(c)
            if c.author == "Ada" && c.name.as_deref() == Some("width")
                && c.now.as_deref() == Some("3")),
        "{err:?}"
    );
}

#[test]
fn a_held_object_refuses_another_writer_until_the_lease_lapses() {
    let mut s = session();
    let (handle, _) = s.resolve("Main.qml", &Address::Id("root".into())).unwrap();
    s.act_as("Ada", 0);
    s.hold("Main.qml", std::slice::from_ref(&handle)).unwrap();
    s.act_as("agent", LEASE_TTL_MS - 1);
    let err = s
        .edit("Main.qml", None, &insert("Item {}", None), false)
        .unwrap_err();
    assert!(
        matches!(&err, Error::Leased { holder, .. } if holder == "Ada"),
        "{err:?}"
    );
    s.act_as("agent", LEASE_TTL_MS);
    s.edit("Main.qml", None, &insert("Item {}", None), false)
        .unwrap();
}

#[test]
fn handles_follow_their_objects_through_canvas_and_code_edits() {
    let mut s = session();
    let (tree, _) = s.tree("Main.qml").unwrap();
    let text_handle = tree.iter().find(|(_, n)| n.ty == "Text").unwrap().0.clone();
    let rect_handle = tree
        .iter()
        .find(|(_, n)| n.ty == "Rectangle")
        .unwrap()
        .0
        .clone();
    let text_at = |s: &mut Session| {
        s.resolve("Main.qml", &Address::Node(text_handle.clone()))
            .map(|(_, n)| n.path.to_string())
    };
    // Canvas: a new first child shifts the text to root/2.
    s.edit("Main.qml", None, &insert("Item {}", Some(0)), false)
        .unwrap();
    assert_eq!(text_at(&mut s).unwrap(), "root/2");
    // Code view: a sibling typed in above it by hand.
    let (text, g) = s.text("Main.qml").unwrap();
    let at = text.find("    Rectangle").unwrap();
    s.replace_text(
        "Main.qml",
        Some(g),
        Some(at..at),
        "    Text { text: \"new\" }\n",
        false,
    )
    .unwrap();
    // The new Text is the first unnamed Text now: keys alone would take it
    // for ours, so ours must have been followed by the canvas edit's path
    // and then by its key relative to its own place. It is root/3.
    let (_, node) = s
        .resolve("Main.qml", &Address::Node(text_handle.clone()))
        .unwrap();
    assert_eq!(
        node.bindings[0].value,
        "\"hello\"",
        "{}",
        s.text("Main.qml").unwrap().0
    );
    // Canvas again: move the rectangle to the end.
    s.edit(
        "Main.qml",
        None,
        &Op::Move {
            node: Address::Node(rect_handle.clone()),
            parent: Address::Id("root".into()),
            index: None,
        },
        false,
    )
    .unwrap();
    let (_, node) = s
        .resolve("Main.qml", &Address::Node(text_handle.clone()))
        .unwrap();
    assert_eq!(node.bindings[0].value, "\"hello\"");
    let (_, rect) = s
        .resolve("Main.qml", &Address::Node(rect_handle.clone()))
        .unwrap();
    assert_eq!(rect.ty, "Rectangle");
    // Removing it makes its handle say so.
    s.edit(
        "Main.qml",
        None,
        &Op::Remove {
            node: Address::Node(rect_handle.clone()),
        },
        false,
    )
    .unwrap();
    assert!(matches!(
        s.resolve("Main.qml", &Address::Node(rect_handle.clone())),
        Err(Error::Gone { .. })
    ));
    // Undo brings it back under the same handle.
    s.undo().unwrap();
    assert_eq!(
        s.resolve("Main.qml", &Address::Node(rect_handle))
            .unwrap()
            .1
            .ty,
        "Rectangle"
    );
}

#[test]
fn a_broken_text_holds_structural_edits_and_keeps_handles() {
    let mut s = session();
    let (tree, _) = s.tree("Main.qml").unwrap();
    let text_handle = tree.iter().find(|(_, n)| n.ty == "Text").unwrap().0.clone();
    let (text, g) = s.text("Main.qml").unwrap();
    let at = text.find("Text {").unwrap();
    s.replace_text("Main.qml", Some(g), Some(at..at), "Rectangle { ", false)
        .unwrap();
    assert!(matches!(
        s.edit("Main.qml", None, &insert("Item {}", None), false),
        Err(Error::Broken { .. })
    ));
    let (text, g) = s.text("Main.qml").unwrap();
    let at = text.find("Rectangle { Text").unwrap();
    s.replace_text(
        "Main.qml",
        Some(g),
        Some(at..at + "Rectangle { ".len()),
        "",
        false,
    )
    .unwrap();
    assert_eq!(s.text("Main.qml").unwrap().0, MAIN);
    let (_, node) = s.resolve("Main.qml", &Address::Node(text_handle)).unwrap();
    assert_eq!(node.ty, "Text");
}

#[test]
fn a_block_insert_adds_its_import_once() {
    let mut s = session();
    let op = Op::Insert {
        parent: Address::Id("root".into()),
        index: None,
        text: "Counter {}".into(),
        add_id: true,
        import: Some("Shop.Blocks".into()),
    };
    s.edit("Main.qml", None, &op, false).unwrap();
    s.edit("Main.qml", None, &op, false).unwrap();
    let (text, _) = s.text("Main.qml").unwrap();
    assert!(
        text.starts_with("import QtQuick\nimport Shop.Blocks\n\n// The window"),
        "{text}"
    );
    assert_eq!(text.matches("import Shop.Blocks").count(), 1);
    assert!(text.contains("Counter { id: counter2 }"), "{text}");
}

#[test]
fn a_created_document_is_removed_by_undo() {
    let mut s = session();
    s.create("Shell.qml", "import QtQuick\n\nItem {}\n", false)
        .unwrap();
    assert!(s.documents().contains(&"Shell.qml".to_owned()));
    assert!(matches!(
        s.create("Shell.qml", "Item {}", false),
        Err(Error::Exists(_))
    ));
    s.undo().unwrap();
    assert!(!s.store().exists("Shell.qml"));
    s.redo().unwrap();
    assert_eq!(
        s.text("Shell.qml").unwrap().0,
        "import QtQuick\n\nItem {}\n"
    );
}

#[test]
fn a_dry_run_changes_nothing() {
    let mut s = session();
    let out = s
        .edit("Main.qml", None, &insert("Item {}", None), true)
        .unwrap();
    assert!(out.text.unwrap().contains("Item { id: item1 }"));
    assert_eq!(s.text("Main.qml").unwrap(), (MAIN, 1));
    assert!(s.history().0.is_empty());
}

#[test]
fn literals_are_written_as_qml() {
    assert_eq!(
        Literal::String("a \"b\"\n".into()).to_qml(),
        "\"a \\\"b\\\"\\n\""
    );
    assert_eq!(Literal::Bool(true).to_qml(), "true");
}

#[test]
fn diff_is_one_change_on_char_boundaries() {
    assert_eq!(diff("aéb", "aéb"), vec![]);
    let d = diff("xáy", "xéy");
    assert_eq!(d[0].range, 1..3);
    assert_eq!(d[0].replacement, "é");
}

#[test]
fn an_added_id_comes_first() {
    let taken = vec!["item1".to_owned()];
    assert_eq!(with_id("Item {}", &taken).unwrap(), "Item { id: item2 }");
    assert_eq!(with_id("Item {\n}", &[]).unwrap(), "Item { id: item1 }");
    assert_eq!(
        with_id("Text { text: \"a\" }", &[]).unwrap(),
        "Text { id: text1; text: \"a\" }"
    );
    assert_eq!(
        with_id("Row {\n    spacing: 4\n    Text {}\n}", &[]).unwrap(),
        "Row {\n    id: row1\n    spacing: 4\n    Text {}\n}"
    );
    assert_eq!(
        with_id("Item { id: mine }", &[]).unwrap(),
        "Item { id: mine }"
    );
}

#[test]
fn an_added_line_is_read_as_a_whole_line() {
    let before = "a\n    Text {\n}\n";
    let after = "a\n    Text {}\n    Text {\n}\n";
    let d = diff(before, after);
    assert_eq!(d[0].range, 2..2);
    assert_eq!(d[0].replacement, "    Text {}\n");
    assert_eq!(qml_edit::splice(before, &d), after);
    let back = diff(after, before);
    assert_eq!(back[0].range, 2..14);
    assert_eq!(qml_edit::splice(after, &back), before);
}

/// A store whose writes fail while the flag is up.
struct Flaky {
    files: MemStore,
    failing: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Flaky {
    fn check(&self, path: &str) -> Result<(), StoreError> {
        if self.failing.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(StoreError::Io {
                path: path.to_owned(),
                message: "disk full".to_owned(),
            });
        }
        Ok(())
    }
}

impl Store for Flaky {
    fn list(&self, extension: &str) -> Vec<String> {
        self.files.list(extension)
    }
    fn read(&self, path: &str) -> Result<String, StoreError> {
        self.files.read(path)
    }
    fn write(&self, path: &str, text: &str) -> Result<(), StoreError> {
        self.check(path)?;
        self.files.write(path, text)
    }
    fn remove(&self, path: &str) -> Result<(), StoreError> {
        self.check(path)?;
        self.files.remove(path)
    }
    fn exists(&self, path: &str) -> bool {
        self.files.exists(path)
    }
    fn describe(&self) -> String {
        "flaky".to_owned()
    }
}

fn flaky() -> (Session, std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let failing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let store = Flaky {
        files: MemStore::new([("Main.qml".to_owned(), MAIN.to_owned())]),
        failing: failing.clone(),
    };
    (Session::open(Box::new(store)), failing)
}

#[test]
fn a_failed_undo_or_redo_leaves_the_history_where_it_was() {
    use std::sync::atomic::Ordering::SeqCst;
    let (mut s, failing) = flaky();
    s.edit("Main.qml", None, &insert("Item {}", None), false)
        .unwrap();
    let edited = s.text("Main.qml").unwrap().0.to_owned();
    let lists = s.history();

    failing.store(true, SeqCst);
    assert!(matches!(s.undo(), Err(Error::Store(_))));
    assert_eq!(s.history(), lists, "the entry stays to be undone");
    failing.store(false, SeqCst);
    assert_eq!(s.text("Main.qml").unwrap().0, edited);

    s.undo().unwrap();
    assert_eq!(s.text("Main.qml").unwrap().0, MAIN);
    let lists = s.history();

    failing.store(true, SeqCst);
    assert!(matches!(s.redo(), Err(Error::Store(_))));
    assert_eq!(s.history(), lists, "the entry stays to be redone");
    failing.store(false, SeqCst);
    assert_eq!(s.text("Main.qml").unwrap().0, MAIN);

    s.redo().unwrap();
    assert_eq!(s.text("Main.qml").unwrap().0, edited);
}

#[test]
fn a_failed_undo_of_a_created_document_keeps_it() {
    use std::sync::atomic::Ordering::SeqCst;
    let (mut s, failing) = flaky();
    s.create("New.qml", "Item {}\n", false).unwrap();
    failing.store(true, SeqCst);
    assert!(s.undo().is_err());
    failing.store(false, SeqCst);
    assert_eq!(s.text("New.qml").unwrap().0, "Item {}\n");
    s.undo().unwrap();
    assert!(!s.store().exists("New.qml"));
}

/// DESIGN.md §8: Studio names what it inserts and never touches an author's
/// objects, which may have no `id` on purpose.
#[test]
fn only_an_object_studio_inserts_is_given_an_id() {
    let ids = |s: &mut Session| s.text("Main.qml").unwrap().0.matches("id:").count();
    let mut s = session();
    let (rect, text) = (
        Address::Path("root/0".into()),
        Address::Path("root/1".into()),
    );
    let root = Address::Id("root".into());
    assert_eq!(ids(&mut s), 1);

    let set = Op::SetBinding {
        node: rect.clone(),
        name: "width".into(),
        value: "10".into(),
    };
    let g = now(&mut s);
    s.edit("Main.qml", g, &set, false).unwrap();
    assert_eq!(ids(&mut s), 1, "a binding set");
    let unset = Op::RemoveBinding {
        node: rect.clone(),
        name: "width".into(),
    };
    let g = now(&mut s);
    s.edit("Main.qml", g, &unset, false).unwrap();
    assert_eq!(ids(&mut s), 1, "a binding removed");
    let moved = Op::Move {
        node: text,
        parent: root,
        index: Some(0),
    };
    let g = now(&mut s);
    s.edit("Main.qml", g, &moved, false).unwrap();
    assert_eq!(ids(&mut s), 1, "a move");
    s.replace_text("Main.qml", None, Some(0..0), "// A note.\n", false)
        .unwrap();
    assert_eq!(ids(&mut s), 1, "a text edit");
    let g = now(&mut s);
    s.edit("Main.qml", g, &Op::Remove { node: rect }, false)
        .unwrap();
    assert_eq!(ids(&mut s), 1, "a removal");

    let nested = insert("Item { Text {} }", None);
    s.edit("Main.qml", None, &nested, false).unwrap();
    assert_eq!(ids(&mut s), 2, "the inserted object, not what it holds");
    let plain = Op::Insert {
        parent: Address::Id("root".into()),
        index: None,
        text: "Item {}".into(),
        add_id: false,
        import: None,
    };
    s.edit("Main.qml", None, &plain, false).unwrap();
    assert_eq!(ids(&mut s), 2, "asked not to");
}

fn now(s: &mut Session) -> Option<u64> {
    Some(s.generation("Main.qml").unwrap())
}

#[test]
fn an_edit_by_path_needs_the_generation_the_path_was_read_at() {
    let mut s = session();
    let remove = Op::Remove {
        node: Address::Path("root/0".into()),
    };
    let err = s.edit("Main.qml", None, &remove, false).unwrap_err();
    assert!(matches!(err, Error::Invalid(_)), "{err:?}");
    assert_eq!(s.text("Main.qml").unwrap().0, MAIN);
    let g = now(&mut s);
    s.edit("Main.qml", g, &remove, false).unwrap();
}

fn one_file(text: &str) -> Session {
    Session::open(Box::new(MemStore::new([(
        "Main.qml".to_owned(),
        text.to_owned(),
    )])))
}

#[test]
fn a_value_object_is_not_counted_as_a_child_of_its_parent() {
    // Counting it, the end of a parent with no other child came to -1 and
    // the call panicked before the move could be refused.
    let mut s = one_file("Item {\n    contentItem: Rectangle {}\n}\n");
    let (tree, g) = s.tree("Main.qml").unwrap();
    let content = tree
        .iter()
        .find(|(_, n)| n.ty == "Rectangle")
        .unwrap()
        .0
        .clone();
    let moved = Op::Move {
        node: Address::Node(content),
        parent: Address::Path("root".into()),
        index: None,
    };
    let err = s.edit("Main.qml", Some(g), &moved, false).unwrap_err();
    assert!(
        matches!(err, Error::Edit(EditError::NotAChild(_))),
        "{err:?}"
    );
    assert_eq!(s.history().0.len(), 0);
}

#[test]
fn a_link_and_the_document_it_leads_to_are_one_document() {
    // The reviewer's steps: read through one name, edit through the other,
    // edit through the first, undo. Held twice, each copy took the other's
    // write for a change made outside Studio, and undo wrote one copy's old
    // text over the other's edit.
    let dir = tempfile::tempdir().unwrap();
    let store = store::FsStore::new(dir.path()).unwrap();
    store.write("parts/Card.qml", "Item {\n}\n").unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("parts/Card.qml"),
        dir.path().join("Link.qml"),
    )
    .unwrap();
    let mut s = Session::open(Box::new(store));
    let set = |name: &str, value: &str| Op::SetBinding {
        node: Address::Path("root".into()),
        name: name.into(),
        value: value.into(),
    };
    let g = s.generation("parts/Card.qml").unwrap();
    let out = s
        .edit("Link.qml", Some(g), &set("width", "10"), false)
        .unwrap();
    assert_eq!(out.doc, "parts/Card.qml");
    let g = s.generation("parts/Card.qml").unwrap();
    assert_eq!(g, out.generation);
    s.edit("parts/Card.qml", Some(g), &set("height", "20"), false)
        .unwrap();
    let (done, _) = s.history();
    assert_eq!(
        done.iter().map(|e| e.label.as_str()).collect::<Vec<_>>(),
        ["set width", "set height"]
    );
    s.undo().unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("parts/Card.qml")).unwrap(),
        "Item {\n    width: 10\n}\n"
    );
    s.undo().unwrap();
    s.redo().unwrap();
    s.redo().unwrap();
    assert_eq!(
        s.text("Link.qml").unwrap().0,
        "Item {\n    width: 10\n    height: 20\n}\n"
    );
}

#[test]
fn an_undone_insert_leaves_its_handle_gone_until_redo() {
    // The reviewer's probe: nothing listed before the insert, so every
    // handle but the snapshot's is given out on the text after it.
    let mut s = one_file("Column {\n    Rectangle { width: 1 }\n}\n");
    let g = now(&mut s);
    let out = s
        .edit(
            "Main.qml",
            g,
            &Op::Insert {
                parent: Address::Path("root".into()),
                index: Some(0),
                text: "Rectangle { width: 2 }".into(),
                add_id: true,
                import: None,
            },
            false,
        )
        .unwrap();
    let new = out.node.unwrap();
    s.undo().unwrap();
    let after_undo = s.resolve("Main.qml", &Address::Node(new.clone()));
    assert!(
        matches!(after_undo, Err(Error::Gone { .. })),
        "{after_undo:?}"
    );
    let (tree, _) = s.tree("Main.qml").unwrap();
    let old = tree.iter().find(|(_, n)| n.ty == "Rectangle").unwrap();
    assert_ne!(old.0, new);
    s.redo().unwrap();
    let (_, node) = s.resolve("Main.qml", &Address::Node(new)).unwrap();
    assert_eq!(node.path.to_string(), "root/0");
    assert!(node.bindings.iter().any(|b| b.value == "2"));
}

#[test]
fn an_undone_code_view_insert_leaves_its_handle_gone_until_redo() {
    // Typed in the code view, nothing listed first: no handle existed
    // before the edit, so none is in the history's snapshot of that side.
    let mut s = one_file("Column {\n    Rectangle { width: 1 }\n}\n");
    let g = s.generation("Main.qml").unwrap();
    s.replace_text(
        "Main.qml",
        Some(g),
        Some(9..9),
        "    Rectangle { width: 2 }\n",
        false,
    )
    .unwrap();
    let (tree, _) = s.tree("Main.qml").unwrap();
    let handle = |at: &str| {
        tree.iter()
            .find(|(_, n)| n.path.to_string() == at)
            .unwrap()
            .0
            .clone()
    };
    let (new, old) = (handle("root/0"), handle("root/1"));
    s.undo().unwrap();
    let after_undo = s.resolve("Main.qml", &Address::Node(new.clone()));
    assert!(
        matches!(after_undo, Err(Error::Gone { .. })),
        "{after_undo:?}"
    );
    let (_, node) = s.resolve("Main.qml", &Address::Node(old.clone())).unwrap();
    assert_eq!(node.path.to_string(), "root/0");
    s.redo().unwrap();
    let (_, node) = s.resolve("Main.qml", &Address::Node(new)).unwrap();
    assert_eq!(node.path.to_string(), "root/0");
    assert_eq!(node.bindings[0].value, "2");
    let (_, node) = s.resolve("Main.qml", &Address::Node(old)).unwrap();
    assert_eq!(node.path.to_string(), "root/1");
}

#[test]
fn a_document_made_again_does_not_start_its_generations_over() {
    // The reviewer's probe: a generation read from the first life of a
    // path must not pass for one read from the second.
    let mut s = one_file("Item {}\n");
    let first = s.create("N.qml", "Item {\n}\n", false).unwrap().generation;
    s.undo().unwrap();
    let again = s.create("N.qml", "Item {\n}\n", false).unwrap().generation;
    assert!(again > first, "{again} after {first}");
    let err = s
        .replace_text("N.qml", Some(first), Some(0..4), "Rectangle", false)
        .unwrap_err();
    assert!(matches!(err, Error::Stale { .. }), "{err:?}");
    assert_eq!(s.text("N.qml").unwrap().0, "Item {\n}\n");
}

#[test]
fn a_place_names_the_object_declared_there_not_its_parent() {
    let mut s = Session::open(Box::new(MemStore::new([(
        "Main.qml".to_owned(),
        "import QtQuick\n\nItem {\n    Row { Text { text: \"é\" } Text {} }\n}\n".to_owned(),
    )])));
    let ty = |s: &mut Session, line, column| {
        s.declared_at("Main.qml", line, column)
            .map(|(_, n)| n.path.to_string())
    };
    assert_eq!(ty(&mut s, 3, 1).unwrap(), "root");
    assert_eq!(ty(&mut s, 4, 5).unwrap(), "root/0");
    assert_eq!(ty(&mut s, 4, 11).unwrap(), "root/0/0");
    // Columns count characters: `é` is one.
    assert_eq!(ty(&mut s, 4, 30).unwrap(), "root/0/1");
    assert!(matches!(ty(&mut s, 4, 12), Err(Error::NotFound { .. })));
    // Without a column the line must say which.
    assert_eq!(ty(&mut s, 3, 0).unwrap(), "root");
    assert!(matches!(ty(&mut s, 4, 0), Err(Error::Invalid(_))));
}

#[test]
fn each_target_is_inside_its_own_breakpoint() {
    use crate::preview::Target;
    for target in Target::ALL {
        assert_eq!(Target::of_width(target.size().width), target);
        assert_eq!(
            Target::of_condition(&target.condition("root"), "root"),
            Some(target)
        );
    }
    assert_eq!(Target::of_width(599), Target::Phone);
    assert_eq!(Target::of_width(600), Target::Tablet);
    assert_eq!(Target::of_width(1024), Target::Desktop);
}

#[test]
fn a_state_in_a_list_is_edited_inside_its_own_text() {
    let mut s = session_of(
        "Item {\n    id: root\n    states: [\n        State { name: \"a\" },\n        State {\n            name: \"b\"\n        }\n    ]\n}\n",
    );
    let when = |w: &str| Op::State {
        node: Address::Id("root".into()),
        edit: crate::StateEdit::When {
            name: "b".into(),
            when: Some(w.into()),
        },
    };
    s.edit("Main.qml", None, &when("root.width < 600"), false)
        .unwrap();
    assert_eq!(
        s.text("Main.qml").unwrap().0,
        "Item {\n    id: root\n    states: [\n        State { name: \"a\" },\n        State {\n            name: \"b\"\n            when: root.width < 600\n        }\n    ]\n}\n"
    );
    let v = s.variants("Main.qml", &Address::Id("root".into())).unwrap();
    assert_eq!(v.states[1].when.as_deref(), Some("root.width < 600"));
    assert!(matches!(
        s.edit("Main.qml", None, &when("root.width <"), false),
        Err(Error::Edit(qml_edit::Error::WouldBreak(_)))
    ));
}

#[test]
fn a_single_state_becomes_a_list_of_two() {
    let mut s = session_of("Item {\n    id: root\n    states: State { name: \"a\" }\n}\n");
    let add = Op::State {
        node: Address::Id("root".into()),
        edit: crate::StateEdit::Add {
            name: "b".into(),
            when: None,
        },
    };
    s.edit("Main.qml", None, &add, false).unwrap();
    assert_eq!(
        s.text("Main.qml").unwrap().0,
        "Item {\n    id: root\n    states: [\n        State { name: \"a\" },\n        State {\n            name: \"b\"\n        }\n    ]\n}\n"
    );
}

fn set_width(s: &mut Session, width: &str) -> Result<Outcome, Error> {
    s.edit(
        "Main.qml",
        None,
        &Op::SetBinding {
            node: Address::Id("root".into()),
            name: "width".into(),
            value: width.into(),
        },
        false,
    )
}

/// What a person sees through every part of the session afterwards: the
/// text, the file and the history agree, and undo gives the bytes back.
fn assert_whole(s: &mut Session, before: &str, after: &str) {
    assert_eq!(s.text("Main.qml").unwrap().0, after);
    assert_eq!(s.store().read("Main.qml").unwrap(), after);
    s.undo().unwrap();
    assert_eq!(s.text("Main.qml").unwrap().0, before);
    assert_eq!(s.store().read("Main.qml").unwrap(), before);
}

/// `f` panics as it tells who watches the session about the write.
fn panicking_on_write(s: &mut Session, f: impl FnOnce(&mut Session)) {
    let armed = Arc::new(AtomicBool::new(true));
    let disarm = Arc::clone(&armed);
    s.on_write(move |_| {
        if disarm.swap(false, Ordering::SeqCst) {
            panic!("told of a write");
        }
    });
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(s))).is_err();
    assert!(panicked && !armed.load(Ordering::SeqCst));
}

#[test]
fn a_panic_telling_of_an_edit_leaves_the_session_whole() {
    let mut s = session();
    panicking_on_write(&mut s, |s| {
        let _ = set_width(s, "300");
    });
    assert_whole(&mut s, MAIN, &MAIN.replace("200", "300"));
}

#[test]
fn a_panic_telling_of_an_undo_leaves_the_session_whole() {
    let mut s = session();
    set_width(&mut s, "300").unwrap();
    panicking_on_write(&mut s, |s| {
        let _ = s.undo();
    });
    assert_eq!(s.text("Main.qml").unwrap().0, MAIN);
    assert_eq!(s.store().read("Main.qml").unwrap(), MAIN);
    s.redo().unwrap();
    assert_whole(&mut s, MAIN, &MAIN.replace("200", "300"));
}

#[test]
fn a_panic_telling_of_a_new_document_leaves_the_session_whole() {
    let mut s = session();
    panicking_on_write(&mut s, |s| {
        let _ = s.create("New.qml", "Item {}\n", false);
    });
    assert_eq!(s.text("New.qml").unwrap().0, "Item {}\n");
    s.undo().unwrap();
    assert!(!s.store().exists("New.qml"));
}

#[test]
fn a_theme_file_reached_through_a_link_is_one_document() {
    // s1 holds a document under where it really is; a theme write through
    // a link must not make a second copy that takes the first's writes for
    // changes made outside Studio.
    let dir = tempfile::tempdir().unwrap();
    let store = store::FsStore::new(dir.path()).unwrap();
    store
        .write("app/theme.json", "{ \"Spacing\": { \"md\": 20 } }\n")
        .unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("app/theme.json"),
        dir.path().join("theme.json"),
    )
    .unwrap();
    let mut s = Session::open(Box::new(store));
    let g = s.text("app/theme.json").unwrap().1;
    let value = serde_json::json!(24);
    let out = s
        .set_token(
            "theme.json",
            Some(g),
            "Spacing.md",
            None,
            Some(&value),
            false,
        )
        .unwrap();
    assert_eq!(out.doc, "app/theme.json");
    let read = s.tokens("theme.json", Some("Spacing.md")).unwrap();
    assert_eq!(read.file, "app/theme.json");
    assert_eq!(read.generation, out.generation);
    let (text, g) = s.text("app/theme.json").unwrap();
    assert!(text.contains("24"), "{text}");
    assert_eq!(g, out.generation);
    s.undo().unwrap();
    assert!(
        std::fs::read_to_string(dir.path().join("app/theme.json"))
            .unwrap()
            .contains("20")
    );
}

/// A store that refuses to write the sidecar while `refuse` is set.
struct Refusing {
    inner: MemStore,
    refuse: Arc<std::sync::atomic::AtomicBool>,
}

impl Store for Refusing {
    fn list(&self, extension: &str) -> Vec<String> {
        self.inner.list(extension)
    }
    fn read(&self, path: &str) -> Result<String, StoreError> {
        self.inner.read(path)
    }
    fn write(&self, path: &str, text: &str) -> Result<(), StoreError> {
        if path == review::FILE && self.refuse.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(StoreError::Io {
                path: path.to_owned(),
                message: "no space left on device".to_owned(),
            });
        }
        self.inner.write(path, text)
    }
    fn remove(&self, path: &str) -> Result<(), StoreError> {
        self.inner.remove(path)
    }
    fn exists(&self, path: &str) -> bool {
        self.inner.exists(path)
    }
    fn describe(&self) -> String {
        self.inner.describe()
    }
}

#[test]
fn a_comment_the_sidecar_did_not_take_is_not_made() {
    let refuse = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let mut session = Session::open(Box::new(Refusing {
        inner: MemStore::new([("Main.qml".to_owned(), MAIN.to_owned())]),
        refuse: refuse.clone(),
    }));
    let root = Address::Id("root".into());
    assert!(matches!(
        session.comment_add("Main.qml", &root, "Too wide", None, 1),
        Err(Error::Store(_))
    ));
    assert!(session.comments(None).unwrap().is_empty());
    refuse.store(false, std::sync::atomic::Ordering::SeqCst);
    let made = session
        .comment_add("Main.qml", &root, "Too wide", None, 2)
        .unwrap();
    assert_eq!(made.comment.id, "c1");
    refuse.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(session.comment_resolve("c1", true).is_err());
    assert!(session.comment_reply("c1", "Narrowed it", None, 3).is_err());
    let after = session.comments(None).unwrap();
    assert_eq!(after.len(), 1);
    assert!(!after[0].comment.resolved);
    assert_eq!(after[0].comment.thread.len(), 1);
}

#[cfg(feature = "fs")]
#[test]
fn a_sidecar_that_is_not_text_is_never_written_over() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Main.qml"), MAIN).unwrap();
    std::fs::create_dir(dir.path().join(".studio")).unwrap();
    let bytes = b"{\"version\": 1, \"comments\": [\xff\xfe]}";
    std::fs::write(dir.path().join(review::FILE), bytes).unwrap();
    let mut session = Session::open(Box::new(FsStore::new(dir.path()).unwrap()));
    let root = Address::Id("root".into());
    assert!(matches!(
        session.comment_add("Main.qml", &root, "Too wide", None, 1),
        Err(Error::ReviewUnreadable(_))
    ));
    assert!(matches!(
        session.comments(None),
        Err(Error::ReviewUnreadable(_))
    ));
    assert_eq!(std::fs::read(dir.path().join(review::FILE)).unwrap(), bytes);
}

#[test]
fn typing_joins_until_a_pause_and_undoes_and_redoes_as_one() {
    use std::time::Duration;
    let mut s = session_of("Item {}");
    let at = Duration::from_millis;
    let first = s.type_text("Main.qml", 1, Some(0..0), "a", at(0)).unwrap();
    let joined = s
        .type_text("Main.qml", 2, Some(1..1), "b", at(900))
        .unwrap();
    assert_eq!(joined.history, first.history);
    // The pause counts from the last character, not the first.
    let joined = s
        .type_text("Main.qml", 3, Some(2..2), "c", at(1800))
        .unwrap();
    assert_eq!(joined.history, first.history);
    let apart = s
        .type_text("Main.qml", 4, Some(3..3), "d", at(2800))
        .unwrap();
    assert_ne!(apart.history, first.history);

    s.undo().unwrap();
    assert_eq!(s.text("Main.qml").unwrap().0, "abcItem {}");
    s.undo().unwrap();
    assert_eq!(s.text("Main.qml").unwrap().0, "Item {}");
    s.redo().unwrap();
    assert_eq!(s.text("Main.qml").unwrap().0, "abcItem {}");
}

#[test]
fn typing_after_an_undo_does_not_join_what_is_left() {
    use std::time::Duration;
    let mut s = session_of("Item {}");
    let at = Duration::from_millis;
    s.type_text("Main.qml", 1, Some(0..0), "a", at(0)).unwrap();
    s.undo().unwrap();
    let generation = s.generation("Main.qml").unwrap();
    s.type_text("Main.qml", generation, Some(0..0), "b", at(1))
        .unwrap();
    s.undo().unwrap();
    assert_eq!(s.text("Main.qml").unwrap().0, "Item {}");
}
