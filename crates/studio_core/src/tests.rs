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
    s.edit(
        "Main.qml",
        None,
        &Op::SetBinding {
            node: Address::Path("root/1".into()),
            name: "font.bold".into(),
            value: "true".into(),
        },
        false,
    )
    .unwrap();
    s.edit(
        "Main.qml",
        None,
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
fn a_stale_generation_is_refused_and_changes_nothing() {
    let mut s = session();
    let g = s.generation("Main.qml").unwrap();
    s.edit("Main.qml", Some(g), &insert("Item {}", None), false)
        .unwrap();
    let err = s
        .edit("Main.qml", Some(g), &insert("Item {}", None), false)
        .unwrap_err();
    assert!(matches!(err, Error::Stale { given, current, .. } if given == g && current == g + 1));
    assert_eq!(s.history().0.len(), 1);
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
    s.edit("Main.qml", None, &set, false).unwrap();
    assert_eq!(ids(&mut s), 1, "a binding set");
    let unset = Op::RemoveBinding {
        node: rect.clone(),
        name: "width".into(),
    };
    s.edit("Main.qml", None, &unset, false).unwrap();
    assert_eq!(ids(&mut s), 1, "a binding removed");
    let moved = Op::Move {
        node: text,
        parent: root,
        index: Some(0),
    };
    s.edit("Main.qml", None, &moved, false).unwrap();
    assert_eq!(ids(&mut s), 1, "a move");
    s.replace_text("Main.qml", None, Some(0..0), "// A note.\n", false)
        .unwrap();
    assert_eq!(ids(&mut s), 1, "a text edit");
    s.edit("Main.qml", None, &Op::Remove { node: rect }, false)
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
