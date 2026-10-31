//! Placing objects by hand: what a drag on the canvas may write, and the
//! geometry of aligning and copying. Pure, so a page's session and an
//! agent's get the same answers as the window's.
//!
//! What governs an object decides what a drag writes. x and y that a layout
//! or anchors override would be a lie the picture shows at once, so they are
//! never written there.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::outline::Node;

/// A box in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// What moving an object writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Moving {
    /// `x` and `y`.
    Position,
    /// The anchors' margins or centre offsets.
    Margins,
    /// A layout places it: a drag changes its order among its siblings
    /// (`node_move`), never its x or y.
    Reorder,
    /// It cannot be moved by hand.
    None,
}

/// What resizing an object writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Resizing {
    /// `width` and `height`, or margins on an axis anchored at both ends.
    Size,
    /// `Layout.preferredWidth` and `Layout.preferredHeight`.
    Preferred,
    /// It cannot be resized by hand.
    None,
}

/// What a drag does to an object, said before it is dragged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Drag {
    #[serde(rename = "move")]
    pub moving: Moving,
    pub resize: Resizing,
    /// A sentence for the person dragging: empty when it is plain x and y.
    pub says: String,
}

/// The size or place asked for, in the parent's coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wanted {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
}

/// Why an object cannot be placed as asked: `kind` is `arranged`, `anchored`,
/// `root`, or `needs_preview` when only the running preview can tell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub kind: &'static str,
    pub message: String,
}

/// How the parent's type places its children, by the short name of any of
/// its types.
fn arranger<'a>(parent_types: &[&'a str]) -> Option<(Arranger, &'a str)> {
    parent_types.iter().find_map(|ty| {
        let short = ty.rsplit(['.', '/']).next().unwrap_or(ty);
        match short {
            "RowLayout" | "ColumnLayout" | "GridLayout" | "StackLayout" => {
                Some((Arranger::Layout, short))
            }
            "Row" | "Column" | "Grid" | "Flow" => Some((Arranger::Positioner, short)),
            _ => None,
        }
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Arranger {
    Layout,
    Positioner,
}

fn binding<'n>(node: &'n Node, name: &str) -> Option<&'n str> {
    node.bindings
        .iter()
        .find(|b| b.name == name)
        .map(|b| b.value.trim())
}

fn is_number(value: &str) -> bool {
    value.parse::<f64>().is_ok()
}

/// What a drag does to `node`, whose parent has the types `parent_types`
/// (none for a document's root).
pub fn drag_of(node: &Node, parent_types: &[&str]) -> Drag {
    if node.depth == 0 {
        return Drag {
            moving: Moving::None,
            resize: Resizing::Size,
            says: "The root fills the window: it can be resized, not moved".to_owned(),
        };
    }
    if let Some((arranger, name)) = arranger(parent_types) {
        return match arranger {
            Arranger::Layout => Drag {
                moving: Moving::Reorder,
                resize: Resizing::Preferred,
                says: format!("The {name} places it: a drag changes its order"),
            },
            Arranger::Positioner => Drag {
                moving: Moving::Reorder,
                resize: Resizing::Size,
                says: format!("The {name} places it: a drag changes its order"),
            },
        };
    }
    if binding(node, "anchors.fill").is_some() {
        return Drag {
            moving: Moving::None,
            resize: Resizing::None,
            says: "Anchors fill its parent: a drag cannot place it".to_owned(),
        };
    }
    if ANCHORS.iter().any(|a| binding(node, a).is_some()) {
        return Drag {
            moving: Moving::Margins,
            resize: Resizing::Size,
            says: "Anchored: a drag changes its margins".to_owned(),
        };
    }
    let bound: Vec<&str> = ["x", "y", "width", "height"]
        .into_iter()
        .filter(|name| binding(node, name).is_some_and(|v| !is_number(v)))
        .collect();
    Drag {
        moving: Moving::Position,
        resize: Resizing::Size,
        says: if bound.is_empty() {
            String::new()
        } else {
            format!("A drop replaces the binding of {}", bound.join(", "))
        },
    }
}

const ANCHORS: [&str; 7] = [
    "anchors.left",
    "anchors.right",
    "anchors.horizontalCenter",
    "anchors.top",
    "anchors.bottom",
    "anchors.verticalCenter",
    "anchors.centerIn",
];

/// A number as a file would have it: whole where it is, else two places.
pub fn number(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded == rounded.trunc() {
        format!("{rounded:.0}")
    } else {
        rounded.to_string()
    }
}

/// `value` as a margin moved by `by`: a literal stays one; an expression (a
/// token, say) goes on being read, offset.
fn moved(value: Option<&str>, by: f64) -> String {
    match value {
        None => number(by),
        Some(v) => match v.parse::<f64>() {
            Ok(n) => number(n + by),
            Err(_) if by < 0.0 => format!("({v}) - {}", number(-by)),
            Err(_) => format!("({v}) + {}", number(by)),
        },
    }
}

/// One axis's names.
struct Axis {
    place: &'static str,
    size: &'static str,
    preferred: &'static str,
    fill: &'static str,
    low: &'static str,
    high: &'static str,
    center: &'static str,
    low_margin: &'static str,
    high_margin: &'static str,
    offset: &'static str,
}

const HORIZONTAL: Axis = Axis {
    place: "x",
    size: "width",
    preferred: "Layout.preferredWidth",
    fill: "Layout.fillWidth",
    low: "anchors.left",
    high: "anchors.right",
    center: "anchors.horizontalCenter",
    low_margin: "anchors.leftMargin",
    high_margin: "anchors.rightMargin",
    offset: "anchors.horizontalCenterOffset",
};

const VERTICAL: Axis = Axis {
    place: "y",
    size: "height",
    preferred: "Layout.preferredHeight",
    fill: "Layout.fillHeight",
    low: "anchors.top",
    high: "anchors.bottom",
    center: "anchors.verticalCenter",
    low_margin: "anchors.topMargin",
    high_margin: "anchors.bottomMargin",
    offset: "anchors.verticalCenterOffset",
};

/// The bindings to write to place `node` as `wanted`. An anchored axis is
/// written as a change, so it needs `now`: its absence is refused with the
/// kind `needs_preview`.
///
/// # Errors
/// What the object's layout, anchors or place in the document do not allow.
pub fn bindings_for(
    node: &Node,
    parent_types: &[&str],
    now: Option<Rect>,
    wanted: Wanted,
) -> Result<Vec<(String, String)>, Refusal> {
    let drag = drag_of(node, parent_types);
    let label = node.id.as_deref().unwrap_or(&node.ty);
    let moves = wanted.x.is_some() || wanted.y.is_some();
    let resizes = wanted.width.is_some() || wanted.height.is_some();
    if moves {
        match drag.moving {
            Moving::Reorder => {
                return Err(Refusal {
                    kind: "arranged",
                    message: format!(
                        "`{label}` is placed by its parent: x and y would be overridden; \
                         `node_move` changes its order"
                    ),
                });
            }
            Moving::None if node.depth == 0 => {
                return Err(Refusal {
                    kind: "root",
                    message: "the document's root has no place to move to".to_owned(),
                });
            }
            Moving::None => {
                return Err(Refusal {
                    kind: "anchored",
                    message: format!("`{label}` fills its parent by its anchors"),
                });
            }
            Moving::Position | Moving::Margins => {}
        }
    }
    if resizes && drag.resize == Resizing::None {
        return Err(Refusal {
            kind: "anchored",
            message: format!("`{label}` fills its parent by its anchors"),
        });
    }
    let mut out = Vec::new();
    let horizontal = now.map(|r| (r.x, r.width));
    let vertical = now.map(|r| (r.y, r.height));
    axis(
        node,
        &drag,
        &HORIZONTAL,
        horizontal,
        wanted.x,
        wanted.width,
        &mut out,
    )?;
    axis(
        node,
        &drag,
        &VERTICAL,
        vertical,
        wanted.y,
        wanted.height,
        &mut out,
    )?;
    Ok(out)
}

fn axis(
    node: &Node,
    drag: &Drag,
    names: &Axis,
    now: Option<(f64, f64)>,
    place: Option<f64>,
    size: Option<f64>,
    out: &mut Vec<(String, String)>,
) -> Result<(), Refusal> {
    if place.is_none() && size.is_none() {
        return Ok(());
    }
    let label = node.id.as_deref().unwrap_or(&node.ty);
    let mut set = |name: &str, value: String| out.push((name.to_owned(), value));
    if drag.resize == Resizing::Preferred {
        if let Some(size) = size {
            if binding(node, names.fill) == Some("true") {
                return Err(Refusal {
                    kind: "arranged",
                    message: format!(
                        "the layout fills `{label}`'s {} (`{}`): a size would be overridden",
                        names.size, names.fill
                    ),
                });
            }
            set(names.preferred, number(size));
        }
        return Ok(());
    }
    let low = binding(node, names.low).is_some();
    let high = binding(node, names.high).is_some();
    let center =
        binding(node, names.center).is_some() || binding(node, "anchors.centerIn").is_some();
    if !low && !high && !center {
        if let Some(place) = place {
            set(names.place, number(place));
        }
        if let Some(size) = size {
            set(names.size, number(size));
        }
        return Ok(());
    }
    let Some((at, length)) = now else {
        return Err(Refusal {
            kind: "needs_preview",
            message: format!(
                "`{label}` is anchored: where it is now takes a running preview; `preview_open` \
                 starts one"
            ),
        });
    };
    let new_at = place.unwrap_or(at);
    let new_length = size.unwrap_or(length);
    let (low_by, high_by) = (new_at - at, new_at + new_length - at - length);
    let margin = |name: &str| binding(node, name).or_else(|| binding(node, "anchors.margins"));
    if low && high {
        if low_by != 0.0 {
            set(names.low_margin, moved(margin(names.low_margin), low_by));
        }
        if high_by != 0.0 {
            set(
                names.high_margin,
                moved(margin(names.high_margin), -high_by),
            );
        }
        return Ok(());
    }
    if low && low_by != 0.0 {
        set(names.low_margin, moved(margin(names.low_margin), low_by));
    }
    if high && high_by != 0.0 {
        set(
            names.high_margin,
            moved(margin(names.high_margin), -high_by),
        );
    }
    if center {
        let by = (low_by + high_by) / 2.0;
        if by != 0.0 {
            set(names.offset, moved(binding(node, names.offset), by));
        }
    }
    if let Some(size) = size.filter(|s| *s != length) {
        set(names.size, number(size));
    }
    Ok(())
}

/// How `node_arrange` `align` lines objects up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Left,
    Hcenter,
    Right,
    Top,
    Vcenter,
    Bottom,
    DistributeH,
    DistributeV,
}

impl Align {
    pub fn distributes(self) -> bool {
        matches!(self, Align::DistributeH | Align::DistributeV)
    }
}

/// How far each of `boxes` moves, in their order. They line up with the
/// box around them all, or with `parent` when there is only one; distributing
/// keeps the outermost two where they are and evens the gaps.
pub fn aligned(boxes: &[Rect], how: Align, parent: Option<Rect>) -> Vec<(f64, f64)> {
    if how.distributes() {
        return distributed(boxes, how == Align::DistributeH);
    }
    let around = match (boxes, parent) {
        ([_], Some(parent)) => parent,
        _ => around(boxes),
    };
    boxes
        .iter()
        .map(|b| match how {
            Align::Left => (around.x - b.x, 0.0),
            Align::Hcenter => (around.x + (around.width - b.width) / 2.0 - b.x, 0.0),
            Align::Right => (around.x + around.width - b.width - b.x, 0.0),
            Align::Top => (0.0, around.y - b.y),
            Align::Vcenter => (0.0, around.y + (around.height - b.height) / 2.0 - b.y),
            Align::Bottom => (0.0, around.y + around.height - b.height - b.y),
            Align::DistributeH | Align::DistributeV => (0.0, 0.0),
        })
        .collect()
}

fn around(boxes: &[Rect]) -> Rect {
    let left = boxes.iter().map(|b| b.x).fold(f64::INFINITY, f64::min);
    let top = boxes.iter().map(|b| b.y).fold(f64::INFINITY, f64::min);
    let right = boxes
        .iter()
        .map(|b| b.x + b.width)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = boxes
        .iter()
        .map(|b| b.y + b.height)
        .fold(f64::NEG_INFINITY, f64::max);
    Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}

fn distributed(boxes: &[Rect], horizontal: bool) -> Vec<(f64, f64)> {
    let span = |b: &Rect| {
        if horizontal {
            (b.x, b.width)
        } else {
            (b.y, b.height)
        }
    };
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    order.sort_by(|&a, &b| span(&boxes[a]).0.total_cmp(&span(&boxes[b]).0));
    let mut out = vec![(0.0, 0.0); boxes.len()];
    let (Some(&first), Some(&last)) = (order.first(), order.last()) else {
        return out;
    };
    let lengths: f64 = boxes.iter().map(|b| span(b).1).sum();
    let whole = span(&boxes[last]).0 + span(&boxes[last]).1 - span(&boxes[first]).0;
    let gap = (whole - lengths) / (boxes.len().max(2) - 1) as f64;
    let mut at = span(&boxes[first]).0;
    for &i in &order {
        let (start, length) = span(&boxes[i]);
        out[i] = if horizontal {
            (at - start, 0.0)
        } else {
            (0.0, at - start)
        };
        at += length + gap;
    }
    out
}

/// `text` (one object as written) with every id in `ids` that `taken` holds
/// renamed to one nothing else has, and what its own text says of them
/// following. The new names go into `taken`.
pub fn with_fresh_ids(text: &str, ids: &[String], taken: &mut Vec<String>) -> String {
    let mut renames: Vec<(String, String)> = Vec::new();
    for id in ids {
        if !taken.contains(id) {
            taken.push(id.clone());
            continue;
        }
        let stem = id.trim_end_matches(|c: char| c.is_ascii_digit());
        let fresh = (1..)
            .map(|n| format!("{stem}{n}"))
            .find(|name| !taken.contains(name))
            .expect("an unused name");
        taken.push(fresh.clone());
        renames.push((id.clone(), fresh));
    }
    if renames.is_empty() {
        return text.to_owned();
    }
    renamed(text, &renames)
}

/// Replaces the identifiers `renames` names, outside strings and comments,
/// where one is a name of an object: not `foo.inner` and not a member `inner:`.
fn renamed(text: &str, renames: &[(String, String)]) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b'"' | b'\'' | b'`' => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != c {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                i = (i + 1).min(bytes.len());
                out.push_str(&text[start..i]);
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                let end = text[i..].find('\n').map_or(bytes.len(), |n| i + n);
                out.push_str(&text[i..end]);
                i = end;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let end = text[i + 2..]
                    .find("*/")
                    .map_or(bytes.len(), |n| i + 2 + n + 2);
                out.push_str(&text[i..end]);
                i = end;
            }
            c if c == b'_' || c == b'$' || c.is_ascii_alphabetic() => {
                let start = i;
                while i < bytes.len()
                    && (bytes[i] == b'_' || bytes[i] == b'$' || bytes[i].is_ascii_alphanumeric())
                {
                    i += 1;
                }
                let word = &text[start..i];
                let before = text[..start].trim_end().as_bytes().last().copied();
                let after = text[i..].trim_start().as_bytes().first().copied();
                let member = before == Some(b'.') || (after == Some(b':') && before != Some(b'?'));
                match renames.iter().find(|(old, _)| old == word) {
                    Some((_, new)) if !member => out.push_str(new),
                    _ => out.push_str(word),
                }
            }
            _ => {
                let len = text[i..].chars().next().map_or(1, char::len_utf8);
                out.push_str(&text[i..i + len]);
                i += len;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn one_object_aligns_with_its_parent() {
        let parent = r(0.0, 0.0, 100.0, 100.0);
        assert_eq!(
            aligned(&[r(10.0, 10.0, 20.0, 20.0)], Align::Right, Some(parent)),
            [(70.0, 0.0)]
        );
    }

    #[test]
    fn several_align_with_the_box_around_them() {
        let boxes = [r(10.0, 0.0, 10.0, 10.0), r(50.0, 5.0, 10.0, 10.0)];
        assert_eq!(
            aligned(&boxes, Align::Hcenter, None),
            [(20.0, 0.0), (-20.0, 0.0)]
        );
    }

    #[test]
    fn distributing_keeps_the_outermost_and_evens_the_gaps_in_any_order() {
        let boxes = [
            r(0.0, 0.0, 10.0, 10.0),
            r(100.0, 0.0, 10.0, 10.0),
            r(20.0, 0.0, 10.0, 10.0),
        ];
        assert_eq!(
            aligned(&boxes, Align::DistributeH, None),
            [(0.0, 0.0), (0.0, 0.0), (30.0, 0.0)]
        );
    }

    #[test]
    fn renaming_leaves_strings_members_and_other_words_alone() {
        let renames = [("card".to_owned(), "card1".to_owned())];
        assert_eq!(
            renamed(
                "Item { id: card; text: \"card\"; x: card.x; y: a.card; card: 1; w: b ? card : 2 } // card",
                &renames
            ),
            "Item { id: card1; text: \"card\"; x: card1.x; y: a.card; card: 1; w: b ? card1 : 2 } // card"
        );
    }

    #[test]
    fn a_taken_id_gets_the_next_free_number_after_its_stem() {
        let mut taken = vec!["a1".to_owned(), "a2".to_owned()];
        assert_eq!(
            with_fresh_ids("Item { id: a1 }", &["a1".to_owned()], &mut taken),
            "Item { id: a3 }"
        );
    }

    #[test]
    fn numbers_are_written_as_a_file_has_them() {
        assert_eq!(number(12.0), "12");
        assert_eq!(number(12.345), "12.35");
        assert_eq!(number(-0.4), "-0.4");
    }
}
