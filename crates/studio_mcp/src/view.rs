//! What a window shows of the session, read in one go: the state
//! `Studio.Workspace` publishes (DESIGN.md, S3 "As built").
//!
//! It is only read. Every change a window makes goes through
//! [`StudioServer::call`](crate::StudioServer::call) like an agent's, so this
//! is no second way into the session; it saves a window from asking five
//! tools, and holding the lock five times, for one picture of it.

use qml_edit::Step;
use serde::Serialize;
use serde_json::Value as Json;
use studio_core::{
    Address, StateInfo,
    preview::{PreviewEvent, Target},
};

use crate::{Studio, blocks, preview};

/// One object of the open document, in order.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Layer {
    pub node: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub id: String,
    pub depth: usize,
    /// Its type and its `id`, as the layers panel names it: `Item root`.
    pub label: String,
    /// The handle of the object it is written in; empty for the root.
    pub parent: String,
    /// Its place among the objects written in `parent`, as `node_move`
    /// counts; `None` for one a property holds (`contentItem: Item {}`),
    /// which has no place among them.
    pub index: Option<usize>,
}

/// A binding of the selected object, as written.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Shown {
    pub name: String,
    pub value: String,
}

/// The selected object's place in [`View::text`], in UTF-16 units, as a
/// script indexes a string and a text editor selects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// A block the palette offers.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Offered {
    pub module: String,
    pub name: String,
    pub summary: String,
    pub category: String,
    /// The slots it offers, by name.
    pub slots: Vec<String>,
}

/// A role of `Ljos.Design` as the tokens panel lists it: what the window
/// shows, the theme file's value where it has one and the module's where it
/// does not.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TokenRow {
    pub token: String,
    /// `color`, `int`, `real` or `curve`.
    pub kind: String,
    /// A colour's light and dark; empty for any other role.
    pub light: String,
    pub dark: String,
    /// Any other role's value as JSON writes it; empty for a colour.
    pub value: String,
    /// The theme file sets it, in any variant.
    pub set: bool,
}

/// The preview of the open document, when one runs.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Previewed {
    pub doc: String,
    /// `Desktop`, `Tablet`, `Phone`, or empty for a size of its own.
    pub target: String,
    pub width: u32,
    pub height: u32,
    /// What the preview shows: 1 when it started, one more for each reload
    /// it applied.
    pub generation: u64,
    /// `live` or `mocked`.
    pub state: String,
    /// The root's state whose `when` is the breakpoint of the preview's
    /// width, as `state_edit` writes it; empty when none is.
    pub variant: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct View {
    pub documents: Vec<String>,
    pub doc: String,
    pub generation: u64,
    pub text: String,
    /// Why the document's objects are not listed: it does not parse.
    pub problem: String,
    pub layers: Vec<Layer>,
    /// The handle of the selected object in `doc`; empty when none is.
    pub selected: String,
    pub selection: Span,
    pub inspector: Vec<Shown>,
    /// The selected object's states.
    pub states: Vec<StateInfo>,
    pub palette: Vec<Offered>,
    pub done: Vec<String>,
    pub undone: Vec<String>,
    pub preview: Option<Previewed>,
    /// The server can start previews.
    pub previews: bool,
    /// The theme file beside `doc` (or the project's, with none open).
    pub theme: String,
    /// Why the theme file is not read: Ljós would refuse it.
    pub theme_problem: String,
    pub tokens: Vec<TokenRow>,
}

impl Studio {
    /// The session as a window showing `doc` sees it now.
    pub fn view(&mut self, doc: &str) -> View {
        let mut view = View {
            documents: self.session.documents(),
            previews: self.hosts.is_some() && self.dir().is_some(),
            ..View::default()
        };
        let (done, undone) = self.session.history();
        view.done = done.into_iter().map(|e| e.label).collect();
        view.undone = undone.into_iter().map(|e| e.label).collect();
        let registry = blocks::registry(self.session.store());
        view.palette = registry
            .blocks()
            .map(|b| Offered {
                module: b.module.clone(),
                name: b.name.clone(),
                summary: b.summary.clone().unwrap_or_default(),
                category: b.category.clone().unwrap_or_default(),
                slots: b.slots.iter().map(|s| s.name.clone()).collect(),
            })
            .collect();
        view.preview = preview_of(&preview::lock(&self.previews));
        if let Some(p) = &mut view.preview {
            p.variant = self.variant(&p.doc, p.width).unwrap_or_default();
        }
        view.theme = crate::tools::theme_file(doc);
        if let Ok(read) = self.session.tokens(&view.theme, None) {
            view.theme_problem = read.problem.unwrap_or_default();
            view.tokens = read.tokens.into_iter().map(row).collect();
        }
        if doc.is_empty() {
            return view;
        }
        view.doc = doc.to_owned();
        let Ok((text, generation)) = self.session.text(doc) else {
            return view;
        };
        view.text = text.to_owned();
        view.generation = generation;
        match self.session.tree(doc) {
            Ok((tree, _)) => {
                let handle_of = |path: &[Step]| {
                    tree.iter()
                        .find(|(_, n)| n.path.0 == path)
                        .map(|(h, _)| h.clone())
                };
                view.layers = tree
                    .iter()
                    .map(|(node, n)| {
                        let id = n.id.clone().unwrap_or_default();
                        let steps = &n.path.0;
                        let (parent, index) = match steps.split_last() {
                            Some((last, up)) => (
                                handle_of(up).unwrap_or_default(),
                                match last {
                                    Step::Child(i) => Some(*i),
                                    _ => None,
                                },
                            ),
                            None => (String::new(), None),
                        };
                        Layer {
                            node: node.clone(),
                            ty: n.ty.clone(),
                            label: if id.is_empty() {
                                n.ty.clone()
                            } else {
                                format!("{} {id}", n.ty)
                            },
                            id,
                            depth: n.depth,
                            parent,
                            index,
                        }
                    })
                    .collect();
            }
            Err(e) => view.problem = e.to_string(),
        }
        let selected = self
            .session
            .selection()
            .into_iter()
            .find(|(d, _)| d == doc)
            .map(|(_, handle)| handle);
        if let Some(handle) = selected
            && let Ok((handle, node)) = self.session.resolve(doc, &Address::Node(handle))
        {
            view.selection = Span {
                start: utf16(&view.text, node.start),
                end: utf16(&view.text, node.end),
            };
            view.inspector = node
                .bindings
                .iter()
                .map(|b| Shown {
                    name: b.name.clone(),
                    value: b.value.clone(),
                })
                .collect();
            if let Ok(variants) = self.session.variants(doc, &Address::Node(handle.clone())) {
                view.states = variants.states;
            }
            view.selected = handle;
        }
        view
    }

    /// The state of `doc`'s root that a window `width` wide turns on by
    /// its breakpoint. Only the condition `state_edit` writes for a
    /// breakpoint is read as one: any other `when` is the app's own logic,
    /// which only running it can answer.
    fn variant(&mut self, doc: &str, width: u32) -> Option<String> {
        let root = Address::Path("root".to_owned());
        let (_, node) = self.session.resolve(doc, &root).ok()?;
        let condition = Target::of_width(width).condition(&node.id?);
        let variants = self.session.variants(doc, &root).ok()?;
        variants
            .states
            .into_iter()
            .find(|s| s.when.as_deref() == Some(condition.as_str()))
            .map(|s| s.name)
    }
}

fn row(t: studio_core::theme::Token) -> TokenRow {
    let set = t.theme.is_some();
    let shown = |variant: &str| {
        t.theme
            .as_ref()
            .and_then(|theme| theme.get(variant))
            .or_else(|| t.module.get(variant))
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let (light, dark, value) = if t.kind == "color" {
        (shown("light"), shown("dark"), String::new())
    } else {
        let value = t.theme.as_ref().unwrap_or(&t.module);
        (String::new(), String::new(), value.to_string())
    };
    TokenRow {
        token: t.token,
        kind: t.kind.to_owned(),
        light,
        dark,
        value,
        set,
    }
}

fn preview_of(previews: &preview::Previews) -> Option<Previewed> {
    let open = previews.open.as_ref()?;
    // The events since this preview started say how often it reloaded.
    let mut generation = 1;
    for event in previews.events.iter().rev() {
        match event {
            PreviewEvent::Reloaded { generation: g, .. } => {
                generation = g + 1;
                break;
            }
            PreviewEvent::Opened { .. } => break,
            _ => {}
        }
    }
    Some(Previewed {
        doc: open.doc.clone(),
        target: match open.target {
            Some(Target::Desktop) => "Desktop",
            Some(Target::Tablet) => "Tablet",
            Some(Target::Phone) => "Phone",
            None => "",
        }
        .to_owned(),
        width: open.size.width,
        height: open.size.height,
        generation,
        state: match open.state {
            preview::State::Live => "live",
            preview::State::Mocked => "mocked",
        }
        .to_owned(),
        variant: String::new(),
    })
}

/// A byte offset of `text` in UTF-16 units.
fn utf16(text: &str, byte: usize) -> usize {
    let mut end = byte.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].encode_utf16().count()
}
