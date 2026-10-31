//! A prototype's flow: what a tap does, written as the app's own QML.
//!
//! An interaction is a signal handler on the object that is tapped (its
//! trigger): the signal's own handler when one is named (`onClicked` of a
//! button), else a `TapHandler`, which any item can hold. Its body is one
//! of the statements Studio writes, so a flow is read back only from those
//! forms; a handler a person wrote otherwise is code, not a flow:
//!
//! - `owner.state = "S"`: go to the state `S` of `owner` (`""` is the base);
//! - `loader.source = "D.qml"`: show the document `D` in a `Loader`;
//! - `stack.push(Qt.resolvedUrl("D.qml"))`: push it on a `StackView`.

use serde::Serialize;

use crate::{Address, Error, Literal, Node, NodePath, Op, Session, states};
use qml_edit::Step;

/// The type that turns a tap on any item into a signal.
const TAP_HANDLER: &str = "TapHandler";
/// The types a flow can open a document in.
const NAVIGATORS: [&str; 2] = ["Loader", "StackView"];

/// Where a flow goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Goal {
    /// A state of the owner (the document's root unless named); `""` is the
    /// base state.
    State {
        owner: Option<Address>,
        state: String,
    },
    /// A document, shown by a `Loader` or pushed on a `StackView`.
    Open { navigator: Address, doc: String },
}

/// One flow of a document, as [`Session::interactions`] lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Interaction {
    /// The handle of the object that is tapped.
    pub trigger: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_id: Option<String>,
    /// The signal handled: `tapped` for a `TapHandler`'s.
    pub signal: String,
    /// The `id` of the object whose state it sets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// The `id` of the `Loader` or `StackView` that opens the document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub navigator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<String>,
}

/// What a handler's body does, when it is one of the forms Studio writes.
#[derive(Debug, PartialEq, Eq)]
enum Statement {
    SetState { owner: String, state: String },
    Source { navigator: String, doc: String },
    Push { navigator: String, doc: String },
}

/// `onClicked` for `clicked`.
pub fn handler_name(signal: &str) -> String {
    let mut chars = signal.chars();
    match chars.next() {
        Some(first) => format!("on{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => "on".to_owned(),
    }
}

/// `clicked` for `onClicked`; `None` for a name that is no signal handler.
fn signal_of(name: &str) -> Option<String> {
    let rest = name.strip_prefix("on")?;
    let mut chars = rest.chars();
    let first = chars.next()?;
    if !first.is_ascii_uppercase() || rest.contains('.') {
        return None;
    }
    Some(format!("{}{}", first.to_ascii_lowercase(), chars.as_str()))
}

pub fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn short(ty: &str) -> &str {
    ty.rsplit('.').next().unwrap_or(ty)
}

fn parent_path(path: &NodePath) -> Option<NodePath> {
    match path.0.last() {
        Some(Step::Child(_)) => Some(NodePath(path.0[..path.0.len() - 1].to_vec())),
        _ => None,
    }
}

fn is_child_of(node: &Node, parent: &NodePath) -> bool {
    node.path.0.len() == parent.0.len() + 1
        && node.path.0.starts_with(&parent.0)
        && matches!(node.path.0.last(), Some(Step::Child(_)))
}

/// `path` of a document beside `doc`, as QML resolves a relative URL.
pub(crate) fn beside(doc: &str, path: &str) -> String {
    match doc.rsplit_once('/') {
        Some((dir, _)) => format!("{dir}/{path}"),
        None => path.to_owned(),
    }
}

impl Statement {
    fn to_qml(&self) -> String {
        let quote = |s: &str| Literal::String(s.to_owned()).to_qml();
        match self {
            Statement::SetState { owner, state } => format!("{owner}.state = {}", quote(state)),
            Statement::Source { navigator, doc } => format!("{navigator}.source = {}", quote(doc)),
            Statement::Push { navigator, doc } => {
                format!("{navigator}.push(Qt.resolvedUrl({}))", quote(doc))
            }
        }
    }

    /// The statement `text` is, when Studio wrote it.
    fn read(text: &str) -> Option<Statement> {
        let mut p = Parser(text.trim());
        let target = p.identifier()?;
        p.eat(".")?;
        let member = p.identifier()?;
        let out = match member {
            "state" | "source" => {
                p.eat("=")?;
                let value = p.string()?;
                if member == "state" {
                    Statement::SetState {
                        owner: target.to_owned(),
                        state: value,
                    }
                } else {
                    Statement::Source {
                        navigator: target.to_owned(),
                        doc: value,
                    }
                }
            }
            "push" => {
                p.eat("(")?;
                p.eat("Qt")?;
                p.eat(".")?;
                p.eat("resolvedUrl")?;
                p.eat("(")?;
                let doc = p.string()?;
                p.eat(")")?;
                p.eat(")")?;
                Statement::Push {
                    navigator: target.to_owned(),
                    doc,
                }
            }
            _ => return None,
        };
        p.eat(";");
        p.0.trim().is_empty().then_some(out)
    }
}

/// Reads the few tokens of Studio's own statements, skipping blanks.
struct Parser<'t>(&'t str);

impl<'t> Parser<'t> {
    fn skip(&mut self) {
        self.0 = self.0.trim_start();
    }

    fn eat(&mut self, token: &str) -> Option<()> {
        self.skip();
        self.0 = self.0.strip_prefix(token)?;
        Some(())
    }

    fn identifier(&mut self) -> Option<&'t str> {
        self.skip();
        let end = self
            .0
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(self.0.len());
        let (name, rest) = self.0.split_at(end);
        if !is_identifier(name) {
            return None;
        }
        self.0 = rest;
        Some(name)
    }

    /// A double-quoted string, with the escapes [`Literal::to_qml`] writes.
    fn string(&mut self) -> Option<String> {
        self.skip();
        let mut chars = self.0.strip_prefix('"')?.char_indices();
        let mut out = String::new();
        while let Some((i, c)) = chars.next() {
            match c {
                '"' => {
                    self.0 = &self.0[i + 2..];
                    return Some(out);
                }
                '\\' => out.push(match chars.next()?.1 {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    c @ ('"' | '\\') => c,
                    _ => return None,
                }),
                c => out.push(c),
            }
        }
        None
    }
}

impl Session {
    /// The flows written in the object `address` names and everything in
    /// it, in the order written; the root's are the document's whole flow.
    ///
    /// # Errors
    /// An address that names nothing, or a document that does not parse.
    pub fn interactions(
        &mut self,
        path: &str,
        address: &Address,
    ) -> Result<Vec<Interaction>, Error> {
        let (_, node) = self.resolve(path, address)?;
        let (tree, _) = self.tree(path)?;
        let mut out = Vec::new();
        for (_, n) in tree
            .iter()
            .filter(|(_, n)| n.path.0.starts_with(&node.path.0))
        {
            let tap = short(&n.ty) == TAP_HANDLER;
            let trigger = if tap {
                parent_path(&n.path).and_then(|p| tree.iter().find(|(_, t)| t.path == p))
            } else {
                tree.iter().find(|(_, t)| t.path == n.path)
            };
            let Some((trigger, trigger_node)) = trigger else {
                continue;
            };
            for b in &n.bindings {
                let (Some(signal), Some(statement)) =
                    (signal_of(&b.name), Statement::read(&b.value))
                else {
                    continue;
                };
                let mut flow = Interaction {
                    trigger: trigger.clone(),
                    trigger_id: trigger_node.id.clone(),
                    signal,
                    owner: None,
                    state: None,
                    navigator: None,
                    open: None,
                };
                match statement {
                    Statement::SetState { owner, state } => {
                        flow.owner = Some(owner);
                        flow.state = Some(state);
                    }
                    Statement::Source { navigator, doc } | Statement::Push { navigator, doc } => {
                        flow.navigator = Some(navigator);
                        flow.open = Some(doc);
                    }
                }
                out.push(flow);
            }
        }
        Ok(out)
    }

    /// What an interaction op writes: the edit it is made of.
    pub(crate) fn plan_interaction(
        &mut self,
        path: &str,
        trigger: &Address,
        signal: Option<&str>,
        goal: &Goal,
    ) -> Result<Op, Error> {
        if let Some(signal) = signal
            && !is_identifier(signal)
        {
            return Err(Error::Invalid(format!("`{signal}` is not a signal's name")));
        }
        let (handle, trigger_node) = self.resolve(path, trigger)?;
        let statement = match goal {
            Goal::State { owner, state } => {
                let owner = owner.clone().unwrap_or(Address::Path("root".to_owned()));
                let (owner_handle, owner_node) = self.resolve(path, &owner)?;
                let id = owner_node
                    .id
                    .clone()
                    .ok_or_else(|| Error::NoId(format!("{owner_handle} (`{}`)", owner_node.ty)))?;
                if !state.is_empty() {
                    let text = &self.text(path)?.0;
                    let known = states::variants(text, owner_node.start)
                        .is_some_and(|v| v.states.iter().any(|s| s.name == *state));
                    if !known {
                        return Err(Error::NoState(state.clone()));
                    }
                }
                Statement::SetState {
                    owner: id,
                    state: state.clone(),
                }
            }
            Goal::Open { navigator, doc } => {
                let (nav_handle, nav) = self.resolve(path, navigator)?;
                let kind = short(&nav.ty);
                if !NAVIGATORS.contains(&kind) {
                    return Err(Error::NotANavigator(format!("{nav_handle} (`{}`)", nav.ty)));
                }
                let id = nav
                    .id
                    .clone()
                    .ok_or_else(|| Error::NoId(format!("{nav_handle} (`{}`)", nav.ty)))?;
                if !doc.ends_with(".qml") || !self.store().exists(&beside(path, doc)) {
                    return Err(Error::NoDocument(doc.clone()));
                }
                if kind == "Loader" {
                    Statement::Source {
                        navigator: id,
                        doc: doc.clone(),
                    }
                } else {
                    Statement::Push {
                        navigator: id,
                        doc: doc.clone(),
                    }
                }
            }
        };
        let value = statement.to_qml();
        match signal {
            Some(signal) => {
                let name = handler_name(signal);
                if trigger_node.bindings.iter().any(|b| b.name == name) {
                    return Err(Error::Exists(format!("{handle}'s `{name}`")));
                }
                Ok(Op::SetBinding {
                    node: Address::Node(handle),
                    name,
                    value,
                })
            }
            None => {
                let (tree, _) = self.tree(path)?;
                let tapped = tree.iter().any(|(_, n)| {
                    is_child_of(n, &trigger_node.path)
                        && short(&n.ty) == TAP_HANDLER
                        && n.bindings.iter().any(|b| b.name == "onTapped")
                });
                if tapped {
                    return Err(Error::Exists(format!("{handle}'s `TapHandler`")));
                }
                Ok(Op::Insert {
                    parent: Address::Node(handle),
                    index: None,
                    text: format!("{TAP_HANDLER} {{\n    onTapped: {value}\n}}"),
                    add_id: false,
                    import: None,
                })
            }
        }
    }

    /// The edit that takes a flow out: its handler, and the `TapHandler`
    /// that holds only it.
    pub(crate) fn plan_remove_interaction(
        &mut self,
        path: &str,
        trigger: &Address,
        signal: Option<&str>,
    ) -> Result<Op, Error> {
        let (handle, trigger_node) = self.resolve(path, trigger)?;
        let no_flow = || Error::NoInteraction(format!("{handle} `{}`", signal.unwrap_or("tapped")));
        let is_flow = |n: &Node, name: &str| {
            n.bindings
                .iter()
                .any(|b| b.name == name && Statement::read(&b.value).is_some())
        };
        match signal {
            Some(signal) => {
                let name = handler_name(signal);
                if !is_flow(&trigger_node, &name) {
                    return Err(no_flow());
                }
                Ok(Op::RemoveBinding {
                    node: Address::Node(handle),
                    name,
                })
            }
            None => {
                let (tree, _) = self.tree(path)?;
                let (tap, tap_node) = tree
                    .iter()
                    .find(|(_, n)| {
                        is_child_of(n, &trigger_node.path)
                            && short(&n.ty) == TAP_HANDLER
                            && is_flow(n, "onTapped")
                    })
                    .ok_or_else(no_flow)?;
                let holds_more = tap_node.bindings.len() > 1
                    || tree.iter().any(|(_, n)| {
                        n.path.0.len() > tap_node.path.0.len()
                            && n.path.0.starts_with(&tap_node.path.0)
                    });
                Ok(if holds_more {
                    Op::RemoveBinding {
                        node: Address::Node(tap.clone()),
                        name: "onTapped".to_owned(),
                    }
                } else {
                    Op::Remove {
                        node: Address::Node(tap.clone()),
                    }
                })
            }
        }
    }

    /// An interaction's history label: `tapped card to details`.
    pub(crate) fn interaction_label(
        &mut self,
        path: &str,
        trigger: &Address,
        signal: Option<&str>,
        goal: &Goal,
    ) -> String {
        let name = self
            .resolve(path, trigger)
            .map(|(handle, node)| node.id.unwrap_or(handle))
            .unwrap_or_else(|_| "object".to_owned());
        let to = match goal {
            Goal::State { state, .. } if state.is_empty() => "base state",
            Goal::State { state, .. } => state,
            Goal::Open { doc, .. } => doc,
        };
        format!("{} {name} to {to}", signal.unwrap_or("tapped"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_read_back_as_written() {
        for statement in [
            Statement::SetState {
                owner: "root".into(),
                state: "a \"b\"".into(),
            },
            Statement::SetState {
                owner: "root".into(),
                state: String::new(),
            },
            Statement::Source {
                navigator: "page".into(),
                doc: "Details.qml".into(),
            },
            Statement::Push {
                navigator: "stack".into(),
                doc: "pages/Details.qml".into(),
            },
        ] {
            assert_eq!(Statement::read(&statement.to_qml()), Some(statement));
        }
    }

    #[test]
    fn other_code_is_no_flow() {
        for text in [
            "console.log(\"x\")",
            "root.state = name",
            "{ root.state = \"a\" }",
            "root.state = \"a\"; go()",
            "stack.push(\"Details.qml\")",
            "page.source = \"a\" + b",
        ] {
            assert_eq!(Statement::read(text), None, "{text}");
        }
    }

    #[test]
    fn handler_names() {
        assert_eq!(handler_name("clicked"), "onClicked");
        assert_eq!(signal_of("onClicked").as_deref(), Some("clicked"));
        assert_eq!(signal_of("one"), None);
        assert_eq!(signal_of("onA.b"), None);
    }
}
