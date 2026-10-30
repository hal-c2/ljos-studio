//! A component's variants: the `State`s an object lists in `states` and the
//! `Transition`s in `transitions`, read and edited as text.
//!
//! `qml_edit` addresses objects by the steps from the root, and a list of
//! objects (`states: [State {}, State {}]`) has no step for its elements. So
//! the elements are found here, and an edit inside one (a `when`, a
//! `PropertyChanges`) is `qml_edit` run on that element's text alone, put
//! back where it was. Adding and removing elements is a splice of the list
//! done here, with the same promise as `qml_edit`'s: every byte the edit does
//! not touch stays, and the answer parses or nothing changes.

use qml_edit::{Edit, NodePath};
use qml_syntax::{
    Allocator, GetSpan,
    ast::{QmlBindingValue, QmlMember, QmlObject},
    js,
};
use serde::Serialize;

use crate::{
    Error,
    outline::{self, Binding, is_group},
};

/// One `State` of an object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct StateInfo {
    pub name: String,
    /// The condition that turns it on, as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    /// The values it changes, in the order written.
    pub changes: Vec<ChangeInfo>,
    /// Byte range of the `State` in the text.
    pub start: usize,
    pub end: usize,
}

/// One value a state changes: `target`'s property `name` to `value`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ChangeInfo {
    /// The changed object's `id`.
    pub target: String,
    pub name: String,
    /// As written after the `:`.
    pub value: String,
}

/// One `Transition` of an object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct TransitionInfo {
    /// A state's name, or `*` for any.
    pub from: String,
    pub to: String,
    /// The objects it runs (its animations), as written.
    pub animations: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Variants {
    pub states: Vec<StateInfo>,
    pub transitions: Vec<TransitionInfo>,
}

/// A change to an object's states or transitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateEdit {
    /// A new state, last in the list.
    Add {
        name: String,
        when: Option<String>,
    },
    /// Also renames it where transitions and other states name it.
    Rename {
        name: String,
        to: String,
    },
    Remove {
        name: String,
    },
    /// Set (`Some`) or remove `target`'s property `property` in the state.
    /// `value` is QML as written after the `:`.
    Change {
        name: String,
        target: String,
        property: String,
        value: Option<String>,
    },
    /// Set or remove the state's `when`.
    When {
        name: String,
        when: Option<String>,
    },
    /// `animations` is QML: the objects the transition runs.
    AddTransition {
        from: String,
        to: String,
        animations: String,
    },
    RemoveTransition {
        from: String,
        to: String,
    },
}

impl StateEdit {
    pub(crate) fn label(&self) -> String {
        match self {
            StateEdit::Add { name, .. } => format!("add state {name}"),
            StateEdit::Rename { name, .. } => format!("rename state {name}"),
            StateEdit::Remove { name } => format!("remove state {name}"),
            StateEdit::Change { name, .. } => format!("change state {name}"),
            StateEdit::When { name, .. } => format!("set when of {name}"),
            StateEdit::AddTransition { .. } => "add transition".to_owned(),
            StateEdit::RemoveTransition { .. } => "remove transition".to_owned(),
        }
    }
}

/// The states and transitions of the object that starts at byte `owner` of
/// `source`; `None` when `source` does not parse or no object starts there.
pub fn variants(source: &str, owner: usize) -> Option<Variants> {
    let read = Read::of(source, owner)?;
    Some(Variants {
        states: read
            .states
            .iter()
            .flat_map(|l| &l.elements)
            .map(|e| StateInfo {
                name: e.string("name").unwrap_or_default(),
                when: e.binding("when").map(|b| b.value.clone()),
                changes: e
                    .changes
                    .iter()
                    .flat_map(|c| c.entries())
                    .map(|(target, b)| ChangeInfo {
                        target,
                        name: b.0,
                        value: b.1,
                    })
                    .collect(),
                start: e.start,
                end: e.end,
            })
            .collect(),
        transitions: read
            .transitions
            .iter()
            .flat_map(|l| &l.elements)
            .map(|e| TransitionInfo {
                from: e.ends().0,
                to: e.ends().1,
                animations: e.objects.clone(),
                start: e.start,
                end: e.end,
            })
            .collect(),
    })
}

/// `source` with `edit` made to the states of the object at `owner`, whose
/// path is `path`.
pub(crate) fn apply(
    source: &str,
    owner: usize,
    path: &NodePath,
    edit: &StateEdit,
) -> Result<String, Error> {
    let read = Read::of(source, owner).ok_or_else(|| Error::Invalid("no object".to_owned()))?;
    let unit = unit(source);
    let out = match edit {
        StateEdit::Add { name, when } => {
            check_name(name)?;
            if read.state(name).is_some() {
                return Err(Error::Exists(format!("state `{name}`")));
            }
            let mut text = format!("State {{\n{unit}name: {}", quoted(name));
            if let Some(when) = when {
                text.push_str(&format!("\n{unit}when: {when}"));
            }
            text.push_str("\n}");
            add(source, path, "states", read.states.as_ref(), &text, &unit)?
        }
        StateEdit::Rename { name, to } => {
            check_name(to)?;
            let state = read.need_state(name)?;
            if read.state(to).is_some() {
                return Err(Error::Exists(format!("state `{to}`")));
            }
            let mut out = within(
                source,
                state.start,
                state.end,
                &Edit::SetBinding {
                    object: NodePath::root(),
                    name: "name".to_owned(),
                    value: quoted(to),
                },
            )?;
            // What names the state by its name follows it: a transition's
            // ends and another state's `extend`. One at a time, each found
            // again in the text the last one left.
            loop {
                let read =
                    Read::of(&out, owner).ok_or_else(|| Error::Invalid("no object".into()))?;
                let next = read
                    .transitions
                    .iter()
                    .flat_map(|l| &l.elements)
                    .flat_map(|e| ["from", "to"].map(|key| (e, key)))
                    .chain(
                        read.states
                            .iter()
                            .flat_map(|l| &l.elements)
                            .map(|e| (e, "extend")),
                    )
                    .find(|(e, key)| e.string(key).as_deref() == Some(name.as_str()));
                let Some((element, key)) = next else { break };
                out = within(
                    &out,
                    element.start,
                    element.end,
                    &Edit::SetBinding {
                        object: NodePath::root(),
                        name: key.to_owned(),
                        value: quoted(to),
                    },
                )?;
            }
            out
        }
        StateEdit::Remove { name } => {
            let list = read.states.as_ref().ok_or_else(|| no_state(name))?;
            let index = list
                .elements
                .iter()
                .position(|e| e.string("name").as_deref() == Some(name.as_str()))
                .ok_or_else(|| no_state(name))?;
            remove(source, path, "states", list, index)?
        }
        StateEdit::Change {
            name,
            target,
            property,
            value,
        } => {
            let state = read.need_state(name)?;
            change(source, state, target, property, value.as_deref(), &unit)?
        }
        StateEdit::When { name, when } => {
            let state = read.need_state(name)?;
            let edit = match when {
                Some(when) => Edit::SetBinding {
                    object: NodePath::root(),
                    name: "when".to_owned(),
                    value: when.clone(),
                },
                None if state.binding("when").is_none() => {
                    return Err(Error::NothingTo("remove: the state has no `when`"));
                }
                None => Edit::RemoveBinding {
                    object: NodePath::root(),
                    name: "when".to_owned(),
                },
            };
            within(source, state.start, state.end, &edit)?
        }
        StateEdit::AddTransition {
            from,
            to,
            animations,
        } => {
            if read.transition(from, to).is_some() {
                return Err(Error::Exists(format!("transition from `{from}` to `{to}`")));
            }
            let mut text = format!(
                "Transition {{\n{unit}from: {}\n{unit}to: {}",
                quoted(from),
                quoted(to)
            );
            let animations = animations.trim();
            if !animations.is_empty() {
                text.push_str(&format!(
                    "\n{unit}{}",
                    animations.replace('\n', &format!("\n{unit}"))
                ));
            }
            text.push_str("\n}");
            if outline::outline(&text).is_none() {
                return Err(Error::Edit(qml_edit::Error::WouldBreak(format!(
                    "the animations `{animations}` do not parse"
                ))));
            }
            add(
                source,
                path,
                "transitions",
                read.transitions.as_ref(),
                &text,
                &unit,
            )?
        }
        StateEdit::RemoveTransition { from, to } => {
            let none = || Error::NoTransition {
                from: from.clone(),
                to: to.clone(),
            };
            let list = read.transitions.as_ref().ok_or_else(none)?;
            let index = list
                .elements
                .iter()
                .position(|e| e.has_ends(from, to))
                .ok_or_else(none)?;
            remove(source, path, "transitions", list, index)?
        }
    };
    if outline::outline(&out).is_none() {
        return Err(Error::Edit(qml_edit::Error::WouldBreak(
            "the states would not parse".to_owned(),
        )));
    }
    Ok(out)
}

fn no_state(name: &str) -> Error {
    Error::NoState(name.to_owned())
}

fn check_name(name: &str) -> Result<(), Error> {
    if name.is_empty() || name.contains(['"', '\\', '\n']) {
        return Err(Error::Invalid(format!("`{name}` cannot name a state")));
    }
    Ok(())
}

fn quoted(s: &str) -> String {
    crate::Literal::String(s.to_owned()).to_qml()
}

/// The state's `target` set or unset: in the `PropertyChanges` for it if
/// there is one, else in a new one after the state's other objects.
fn change(
    source: &str,
    state: &Element,
    target: &str,
    property: &str,
    value: Option<&str>,
    unit: &str,
) -> Result<String, Error> {
    // A PropertyChanges with a `target` names properties of it; one without
    // names them through ids (`label.text`).
    let found = state.changes.iter().find_map(|c| match &c.target {
        Some(t) if t == target => Some((c, property.to_owned())),
        None => {
            let dotted = format!("{target}.{property}");
            c.bindings
                .iter()
                .any(|b| b.name == dotted)
                .then_some((c, dotted))
        }
        _ => None,
    });
    match (found, value) {
        (Some((changes, name)), Some(value)) => within(
            source,
            changes.start,
            changes.end,
            &Edit::SetBinding {
                object: NodePath::root(),
                name,
                value: value.to_owned(),
            },
        ),
        (None, Some(value)) => within(
            source,
            state.start,
            state.end,
            &Edit::InsertObject {
                parent: NodePath::root(),
                index: state.children,
                text: format!(
                    "PropertyChanges {{\n{unit}target: {target}\n{unit}{property}: {value}\n}}"
                ),
            },
        ),
        (Some((changes, name)), None) => {
            let missing = !changes.bindings.iter().any(|b| b.name == name);
            if missing {
                return Err(no_change(state, target, property));
            }
            let others = changes
                .bindings
                .iter()
                .filter(|b| b.name != name && b.name != "target")
                .count();
            if others == 0 {
                within(
                    source,
                    state.start,
                    state.end,
                    &Edit::RemoveObject {
                        object: NodePath::root().child(changes.child),
                    },
                )
            } else {
                within(
                    source,
                    changes.start,
                    changes.end,
                    &Edit::RemoveBinding {
                        object: NodePath::root(),
                        name,
                    },
                )
            }
        }
        (None, None) => Err(no_change(state, target, property)),
    }
}

fn no_change(state: &Element, target: &str, property: &str) -> Error {
    Error::NoChange {
        state: state.string("name").unwrap_or_default(),
        name: format!("{target}.{property}"),
    }
}

/// `edit` made by `qml_edit` to the object written at `start..end` of
/// `source`, as if that object were a file of its own; the path of the
/// object is the root. Its line's indentation goes in front of it so that
/// what is put in is indented as the lines around it.
fn within(source: &str, start: usize, end: usize, edit: &Edit) -> Result<String, Error> {
    let line = line_start(source, start);
    let indent: String = source[line..start]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let piece = format!("{indent}{}", &source[start..end]);
    let applied = qml_edit::apply(&piece, edit)?;
    let inner = applied
        .text
        .strip_prefix(indent.as_str())
        .ok_or_else(|| Error::Invalid("the edit moved the object's start".to_owned()))?;
    Ok(format!("{}{inner}{}", &source[..start], &source[end..]))
}

/// `source` with `text` (an object, its lines after the first indented
/// from 0 in steps of `unit`) last in the owner's list `name`, which is
/// made if `list` is `None`.
fn add(
    source: &str,
    path: &NodePath,
    name: &str,
    list: Option<&List>,
    text: &str,
    unit: &str,
) -> Result<String, Error> {
    let nl = newline(source);
    let Some(list) = list else {
        let value = format!("[\n{unit}{}\n]", text.replace('\n', &format!("\n{unit}")));
        return Ok(qml_edit::apply(
            source,
            &Edit::SetBinding {
                object: path.clone(),
                name: name.to_owned(),
                value: value.replace('\n', nl),
            },
        )?
        .text);
    };
    let outer = indent_of(source, list.binding_start).to_owned();
    let inner = format!("{outer}{unit}");
    let indented = |at: &str| text.replace('\n', &format!("{nl}{at}"));
    match (&list.form, list.elements.last()) {
        (Form::Array { .. }, Some(last)) => {
            let rest = &source[last.end..];
            let trailing = rest.trim_start().starts_with(',');
            let at = if trailing {
                last.end + rest.find(',').map_or(0, |i| i + 1)
            } else {
                last.end
            };
            let own_line = source[line_start(source, last.start)..last.start]
                .trim()
                .is_empty();
            let put = if own_line {
                let indent = indent_of(source, last.start);
                let comma = if trailing { "," } else { "" };
                let lead = if trailing { "" } else { "," };
                format!("{lead}{nl}{indent}{}{comma}", indented(indent))
            } else {
                format!(", {}", indented(&outer))
            };
            Ok(splice(source, at, at, &put))
        }
        (Form::Array { open, close }, None) => {
            // What is between the brackets can only be comments, which stay.
            let comments = source[*open + 1..*close - 1].trim();
            let comments = if comments.is_empty() {
                String::new()
            } else {
                format!("{inner}{comments}{nl}")
            };
            let put = format!("[{nl}{comments}{inner}{}{nl}{outer}]", indented(&inner));
            Ok(splice(source, *open, *close, &put))
        }
        (Form::Single, Some(only)) => {
            // One object becomes a list of two, one element to a line.
            let existing = source[only.start..only.end].replace('\n', &format!("\n{unit}"));
            let put = format!(
                "[{nl}{inner}{existing},{nl}{inner}{}{nl}{outer}]",
                indented(&inner)
            );
            Ok(splice(source, only.start, only.end, &put))
        }
        (Form::Single, None) => unreachable!("a single form has its object"),
        (Form::Expression, _) => Err(Error::Invalid(format!(
            "`{name}` is an expression; edit it as text"
        ))),
    }
}

/// `source` without element `index` of the owner's list `name`, and
/// without the list when it was the last.
fn remove(
    source: &str,
    path: &NodePath,
    name: &str,
    list: &List,
    index: usize,
) -> Result<String, Error> {
    if list.elements.len() == 1 {
        return Ok(qml_edit::apply(
            source,
            &Edit::RemoveBinding {
                object: path.clone(),
                name: name.to_owned(),
            },
        )?
        .text);
    }
    let element = &list.elements[index];
    if index > 0 {
        // From the end of the one before: its comma, the comments above
        // this one and this one go.
        let before = &list.elements[index - 1];
        return Ok(splice(source, before.end, element.end, ""));
    }
    // The first: up to what follows its comma, keeping a comment there.
    let next = &list.elements[1];
    let comma = source[element.end..next.start]
        .find(',')
        .map_or(element.end, |i| element.end + i + 1);
    let after = comma
        + source[comma..]
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(0);
    let own_line = source[line_start(source, element.start)..element.start]
        .trim()
        .is_empty();
    let (start, end) = if own_line && source[comma..after].contains('\n') {
        (
            comments_above(source, line_start(source, element.start)),
            line_start(source, after),
        )
    } else {
        (element.start, after)
    };
    Ok(splice(source, start, end, ""))
}

/// The start of the comment lines directly above the line at `line`.
fn comments_above(source: &str, mut line: usize) -> usize {
    while line > 0 {
        let above = line_start(source, line - 1);
        if !source[above..line].trim_start().starts_with("//") {
            break;
        }
        line = above;
    }
    line
}

fn splice(source: &str, start: usize, end: usize, text: &str) -> String {
    format!("{}{text}{}", &source[..start], &source[end..])
}

fn line_start(source: &str, at: usize) -> usize {
    source[..at].rfind('\n').map_or(0, |i| i + 1)
}

fn indent_of(source: &str, at: usize) -> &str {
    let line = &source[line_start(source, at)..];
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

fn newline(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// One level of indentation as the file writes it: the first indented
/// line's; four spaces when there is none.
pub fn unit(source: &str) -> String {
    source
        .lines()
        .map(|l| &l[..l.len() - l.trim_start_matches([' ', '\t']).len()])
        .find(|indent| !indent.is_empty())
        .unwrap_or("    ")
        .to_owned()
}

/// What the parse says of the owner's two lists.
struct Read {
    states: Option<List>,
    transitions: Option<List>,
}

struct List {
    /// Start of the `states:` binding.
    binding_start: usize,
    form: Form,
    elements: Vec<Element>,
}

enum Form {
    /// `states: State {}`.
    Single,
    /// `states: [ ... ]`, with the byte range of the brackets.
    Array { open: usize, close: usize },
    /// `states: someList`: what it holds is known only when it runs.
    Expression,
}

/// A `State` or a `Transition`.
struct Element {
    start: usize,
    end: usize,
    bindings: Vec<Binding>,
    /// Its `PropertyChanges`.
    changes: Vec<Changes>,
    /// How many child objects it has.
    children: usize,
    /// The text of its child objects, first to last.
    objects: String,
}

struct Changes {
    start: usize,
    end: usize,
    /// Its place among the state's child objects.
    child: usize,
    target: Option<String>,
    bindings: Vec<Binding>,
}

impl Changes {
    /// (target, (property, value)) for each value it sets.
    fn entries(&self) -> Vec<(String, (String, String))> {
        self.bindings
            .iter()
            .filter(|b| b.name != "target")
            .filter_map(|b| match &self.target {
                Some(t) => Some((t.clone(), (b.name.clone(), b.value.clone()))),
                None => {
                    let (t, name) = b.name.split_once('.')?;
                    Some((t.to_owned(), (name.to_owned(), b.value.clone())))
                }
            })
            .collect()
    }
}

impl Element {
    fn binding(&self, name: &str) -> Option<&Binding> {
        self.bindings.iter().find(|b| b.name == name)
    }

    fn string(&self, name: &str) -> Option<String> {
        self.binding(name)
            .and_then(|b| outline::literal_string(&b.value))
    }

    /// A transition's ends; `*` where one is left out.
    fn ends(&self) -> (String, String) {
        let end = |key| self.string(key).unwrap_or_else(|| "*".to_owned());
        (end("from"), end("to"))
    }

    fn has_ends(&self, from: &str, to: &str) -> bool {
        let (f, t) = self.ends();
        f == from && t == to
    }
}

impl Read {
    fn of(source: &str, owner: usize) -> Option<Read> {
        let allocator = Allocator::default();
        let parsed = qml_syntax::parse(&allocator, source);
        if !parsed.is_ok() {
            return None;
        }
        let object = find(&parsed.document.root, owner)?;
        let mut read = Read {
            states: None,
            transitions: None,
        };
        for member in &object.members {
            let QmlMember::Binding(b) = member else {
                continue;
            };
            let name = b.name.to_string();
            let slot = match name.as_str() {
                "states" => &mut read.states,
                "transitions" => &mut read.transitions,
                _ => continue,
            };
            let start = b.span.start as usize;
            let list = match &b.value {
                QmlBindingValue::Object(o) => List {
                    binding_start: start,
                    form: Form::Single,
                    elements: vec![element(source, o)],
                },
                QmlBindingValue::Objects(objects) => {
                    let first = objects.first()?.span.start as usize;
                    let last = objects.last()?.span.end as usize;
                    let open = start + source[start..first].rfind('[')?;
                    let close = last + source[last..b.span.end as usize].find(']')? + 1;
                    List {
                        binding_start: start,
                        form: Form::Array { open, close },
                        elements: objects.iter().map(|o| element(source, o)).collect(),
                    }
                }
                QmlBindingValue::Expression(e) => {
                    let span = e.span();
                    let form = match e {
                        // Comments and all: an array of nothing is a list.
                        js::Expression::ArrayExpression(a) if a.elements.is_empty() => {
                            Form::Array {
                                open: span.start as usize,
                                close: span.end as usize,
                            }
                        }
                        _ => Form::Expression,
                    };
                    List {
                        binding_start: start,
                        form,
                        elements: Vec::new(),
                    }
                }
                QmlBindingValue::Statement(_) => List {
                    binding_start: start,
                    form: Form::Expression,
                    elements: Vec::new(),
                },
            };
            *slot = Some(list);
        }
        Some(read)
    }

    fn state(&self, name: &str) -> Option<&Element> {
        self.states
            .iter()
            .flat_map(|l| &l.elements)
            .find(|e| e.string("name").as_deref() == Some(name))
    }

    fn need_state(&self, name: &str) -> Result<&Element, Error> {
        self.state(name).ok_or_else(|| no_state(name))
    }

    fn transition(&self, from: &str, to: &str) -> Option<&Element> {
        self.transitions
            .iter()
            .flat_map(|l| &l.elements)
            .find(|e| e.has_ends(from, to))
    }
}

fn element(source: &str, object: &QmlObject<'_>) -> Element {
    let mut bindings = Vec::new();
    outline::collect_bindings(source, object, "", &mut bindings);
    let mut changes = Vec::new();
    let mut children = 0;
    let (mut first, mut last) = (None, None);
    for member in &object.members {
        let QmlMember::Object(inner) = member else {
            continue;
        };
        if is_group(inner) {
            continue;
        }
        first.get_or_insert(inner.span.start as usize);
        last = Some(inner.span.end as usize);
        if inner
            .type_name
            .parts
            .last()
            .is_some_and(|p| *p == "PropertyChanges")
        {
            let mut own = Vec::new();
            outline::collect_bindings(source, inner, "", &mut own);
            changes.push(Changes {
                start: inner.span.start as usize,
                end: inner.span.end as usize,
                child: children,
                target: own
                    .iter()
                    .find(|b| b.name == "target")
                    .map(|b| b.value.clone()),
                bindings: own,
            });
        }
        children += 1;
    }
    Element {
        start: object.span.start as usize,
        end: object.span.end as usize,
        bindings,
        changes,
        children,
        objects: match (first, last) {
            (Some(first), Some(last)) => source[first..last].to_owned(),
            _ => String::new(),
        },
    }
}

/// The object that starts at `at`, anywhere under `object`.
fn find<'b, 'a>(object: &'b QmlObject<'a>, at: usize) -> Option<&'b QmlObject<'a>> {
    if object.span.start as usize == at {
        return Some(object);
    }
    let inside = |o: &QmlObject<'_>| (o.span.start as usize) <= at && at < o.span.end as usize;
    for member in &object.members {
        let found = match member {
            QmlMember::Object(o) if inside(o) => find(o, at),
            QmlMember::Binding(b) => match &b.value {
                QmlBindingValue::Object(o) if inside(o) => find(o, at),
                QmlBindingValue::Objects(list) => {
                    list.iter().find(|o| inside(o)).and_then(|o| find(o, at))
                }
                _ => None,
            },
            QmlMember::Property(p) => match &p.value {
                Some(QmlBindingValue::Object(o)) if inside(o) => find(o, at),
                Some(QmlBindingValue::Objects(list)) => {
                    list.iter().find(|o| inside(o)).and_then(|o| find(o, at))
                }
                _ => None,
            },
            QmlMember::InlineComponent(c) if inside(&c.object) => find(&c.object, at),
            _ => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}
