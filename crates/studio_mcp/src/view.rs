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
use studio_core::{Interaction, flows::handler_name};

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
    /// How many open review comments are on it.
    pub comments: usize,
    /// Who holds it, or an object it is in, when that is another writer
    /// than the window (`node_select` with `hold`); empty otherwise.
    pub held: String,
    /// `node_arrange` `hide` took it out of the preview.
    pub hidden: bool,
    /// `node_arrange` `lock` keeps gestures off it.
    pub locked: bool,
}

/// A review comment of the open document, as a window lists it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Commented {
    pub id: String,
    /// Its object's handle; empty once the object was taken out.
    pub node: String,
    /// What the comment says; its replies are `replies` more.
    pub text: String,
    pub replies: usize,
    pub resolved: bool,
    pub orphaned: bool,
    /// Its object as the layers panel names it, or named it last.
    pub label: String,
}

/// A binding of the selected object, as written.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Shown {
    pub name: String,
    pub value: String,
    /// A row whose value is a colour by its declaration, a name too.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub colour: bool,
    /// A row holding an expression; a row with nothing written holds none.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub expression: bool,
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
    /// Clicks the running app has been given since it started.
    pub clicks: u64,
    /// `live` or `mocked`.
    pub state: String,
    /// The root's state whose `when` is the breakpoint of the preview's
    /// width, as `state_edit` writes it; empty when none is.
    pub variant: String,
}

/// The open document's flow, a graph whose nodes are its root's states.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Flow {
    /// The states, the base state (`""`) first.
    pub states: Vec<String>,
    pub edges: Vec<Edge>,
}

/// A way from one of the root's states to another, or to a document.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Edge {
    /// `transition` or `interaction`.
    pub kind: String,
    /// A state's name, or `*` for any: an interaction goes from wherever
    /// the app is.
    pub from: String,
    /// A state's name, or the document an interaction opens.
    pub to: String,
    /// How the flow panel names it: `tapped card to details`.
    pub label: String,
    /// An interaction's trigger, by handle, and the signal to name when
    /// taking it out: empty for a `TapHandler`'s, which needs none.
    pub trigger: String,
    pub signal: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct View {
    pub documents: Vec<String>,
    /// The project's directory has nothing in it a walk would list.
    pub blank: bool,
    pub doc: String,
    pub generation: u64,
    pub text: String,
    /// Why the document's objects are not listed: it does not parse.
    pub problem: String,
    pub layers: Vec<Layer>,
    /// The handle of the selected object in `doc`; empty when none is.
    pub selected: String,
    /// Every selected object in `doc`, `selected` first.
    pub chosen: Vec<String>,
    pub selection: Span,
    pub inspector: Vec<Shown>,
    /// The selected object's states.
    pub states: Vec<StateInfo>,
    /// The root's states, which stay editable while another object is
    /// selected.
    pub root_states: Vec<StateInfo>,
    /// The roles of the data the selected object sees, as a delegate's
    /// bindings read them (`model.name`).
    pub roles: Vec<String>,
    /// The selected object's signals, as `node_inspect` lists them.
    pub signals: Vec<crate::signals::Signal>,
    pub flow: Flow,
    pub palette: Vec<Offered>,
    /// The toolkit types the document can place, by module.
    pub toolkit: Vec<crate::toolkit::Group>,
    pub done: Vec<String>,
    pub undone: Vec<String>,
    /// The document's review comments, in the order they were made.
    pub comments: Vec<Commented>,
    pub preview: Option<Previewed>,
    /// The server can start previews.
    pub previews: bool,
    /// The theme file Ljós reads for `doc` (`tools::theme_file`).
    pub theme: String,
    /// Why the theme file is not read: Ljós would refuse it.
    pub theme_problem: String,
    pub tokens: Vec<TokenRow>,
    /// What other writers hold now, in every document: what the window's
    /// edit of would be refused as `leased`.
    pub leases: Vec<Held>,
}

/// An object another writer holds.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Held {
    pub doc: String,
    pub node: String,
    pub holder: String,
    /// How long it lasts if the holder calls no more: a window reads the
    /// session again then, as nothing else would tell it.
    pub ends_in_ms: u64,
}

impl Studio {
    /// The session as a window showing `doc` sees it now.
    pub fn view(&mut self, doc: &str) -> View {
        let documents = self.session.documents();
        let mut view = View {
            blank: documents.is_empty() && self.session.blank(),
            documents,
            previews: self
                .hosts
                .as_ref()
                .is_some_and(|h| h.runs(self.dir().as_deref())),
            ..View::default()
        };
        // The window's Undo takes the person's own edits, so its lists
        // are the person's.
        let (done, undone) = self.session.history_by(studio_core::PERSON);
        let now = crate::millis(self.timer.now());
        self.session.tick(now);
        let now = self.session.now_ms();
        view.leases = self
            .session
            .leases()
            .into_iter()
            .filter(|l| l.holder != studio_core::PERSON)
            .map(|l| Held {
                doc: l.doc,
                node: l.handle,
                holder: l.holder,
                ends_in_ms: l.until_ms.saturating_sub(now),
            })
            .collect();
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
            let shown = shown(&preview::lock(&self.previews));
            p.variant = shown
                .or_else(|| self.variant(&p.doc, p.width))
                .unwrap_or_default();
        }
        view.theme = crate::tools::running_theme_file(self.session.store(), doc);
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
        view.toolkit = crate::toolkit::offered(text, "");
        view.text = text.to_owned();
        view.generation = generation;
        if let Ok(variants) = self
            .session
            .variants(doc, &Address::Path("root".to_owned()))
        {
            view.root_states = variants.states;
        }
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
                            comments: 0,
                            held: String::new(),
                            hidden: false,
                            locked: self.locked.contains(node),
                        }
                    })
                    .collect();
            }
            Err(e) => view.problem = e.to_string(),
        }
        {
            let previews = preview::lock(&self.previews);
            for layer in &mut view.layers {
                layer.hidden = previews.is_hidden(doc, &layer.node);
            }
        }
        view.flow = self.flow(doc).unwrap_or_default();
        // A sidecar that does not read shows no comments here; `review`
        // says why when asked.
        view.comments = self
            .session
            .comments(Some(doc))
            .unwrap_or_default()
            .into_iter()
            .map(|v| {
                let c = v.comment;
                let anchor = c.anchor;
                Commented {
                    id: c.id,
                    node: v.node.unwrap_or_default(),
                    text: c.thread.first().map(|n| n.text.clone()).unwrap_or_default(),
                    replies: c.thread.len().saturating_sub(1),
                    resolved: c.resolved,
                    orphaned: c.orphaned,
                    label: anchor
                        .id
                        .map_or_else(|| anchor.ty.clone(), |id| format!("{} {id}", anchor.ty)),
                }
            })
            .collect();
        // A lease holds an object and all it holds, so a layer is held when
        // it or one it is written in is.
        if view.leases.iter().any(|l| l.doc == doc) {
            let index: std::collections::HashMap<String, usize> = view
                .layers
                .iter()
                .enumerate()
                .map(|(i, l)| (l.node.clone(), i))
                .collect();
            for i in 0..view.layers.len() {
                let mut at = Some(i);
                while let Some(j) = at {
                    let layer = &view.layers[j];
                    if let Some(l) = view
                        .leases
                        .iter()
                        .find(|l| l.doc == doc && l.node == layer.node)
                    {
                        view.layers[i].held = l.holder.clone();
                        break;
                    }
                    at = index.get(&layer.parent).copied();
                }
            }
        }
        for c in view
            .comments
            .iter()
            .filter(|c| !c.resolved && !c.node.is_empty())
        {
            if let Some(layer) = view.layers.iter_mut().find(|l| l.node == c.node) {
                layer.comments += 1;
            }
        }
        view.chosen = self
            .session
            .selection()
            .into_iter()
            .filter(|(d, _)| d == doc)
            .map(|(_, handle)| handle)
            .collect();
        let selected = view.chosen.first().cloned();
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
                    colour: b.colour,
                    expression: !b.literal,
                })
                .collect();
            // Declared without a value (`property var stack`): nothing is
            // written to show, and a row is how it gets one.
            for name in &node.properties {
                if !node.bindings.iter().any(|b| &b.name == name) {
                    view.inspector.push(Shown {
                        name: name.clone(),
                        value: String::new(),
                        colour: node.colours.contains(name),
                        expression: false,
                    });
                }
            }
            if let Ok(variants) = self.session.variants(doc, &Address::Node(handle.clone())) {
                view.states = variants.states;
            }
            if let Ok(Some(model)) = self.session.model(doc, &Address::Node(handle.clone())) {
                view.roles = model.roles;
            }
            view.signals = crate::signals::of(self.session.store(), doc, &view.text, &node)
                .unwrap_or_default();
            view.selected = handle;
        }
        view
    }

    /// `doc`'s root's states and the ways between them. Only interactions
    /// that set the root's state, or open a document, are its flow; one
    /// that sets another object's state is that object's.
    fn flow(&mut self, doc: &str) -> Option<Flow> {
        let root = Address::Path("root".to_owned());
        let (_, node) = self.session.resolve(doc, &root).ok()?;
        let variants = self.session.variants(doc, &root).ok()?;
        let interactions = self.session.interactions(doc, &root).ok()?;
        let (tree, _) = self.session.tree(doc).ok()?;
        let mut flow = Flow {
            states: std::iter::once(String::new())
                .chain(variants.states.into_iter().map(|s| s.name))
                .collect(),
            edges: Vec::new(),
        };
        for t in variants.transitions {
            flow.edges.push(Edge {
                kind: "transition".to_owned(),
                label: format!("{} to {}", t.from, t.to),
                from: t.from,
                to: t.to,
                trigger: String::new(),
                signal: String::new(),
            });
        }
        for i in interactions {
            let Interaction {
                trigger,
                trigger_id,
                signal,
                owner,
                state,
                open,
                ..
            } = i;
            let to = match (state, open) {
                (Some(state), _) if owner.is_some() && owner == node.id => state,
                (_, Some(open)) => open,
                _ => continue,
            };
            // A handler of the trigger's own is named by its signal; a
            // `TapHandler`'s is not.
            let own = tree.iter().any(|(h, n)| {
                *h == trigger && n.bindings.iter().any(|b| b.name == handler_name(&signal))
            });
            let name = trigger_id.unwrap_or_else(|| trigger.clone());
            let shown = if to.is_empty() { "base state" } else { &to };
            flow.edges.push(Edge {
                kind: "interaction".to_owned(),
                label: format!("{signal} {name} to {shown}"),
                from: "*".to_owned(),
                to,
                trigger,
                signal: if own { signal } else { String::new() },
            });
        }
        Some(flow)
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

/// The state `preview_state` is showing, which wins over a breakpoint's.
fn shown(previews: &preview::Previews) -> Option<String> {
    previews
        .open
        .as_ref()?
        .shown
        .as_ref()
        .map(|(_, s)| s.clone())
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
        clicks: open.clicks,
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
