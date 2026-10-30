//! Contributions to a block's slots (ADR 0003, section 4).
//!
//! A slot is a block's `list<Contribution>` property; a `Contribution` in a
//! shell names it and carries what it shows as a `Component`, so the block
//! decides when, and how often, to make it.

use qml_syntax::Allocator;
use serde::Deserialize;

use crate::Error;

/// How a contribution joins what the slot already holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Append,
    Prepend,
    Replace,
}

impl Mode {
    fn qml(self) -> &'static str {
        match self {
            Mode::Append => "Contribution.Append",
            Mode::Prepend => "Contribution.Prepend",
            Mode::Replace => "Contribution.Replace",
        }
    }
}

/// The `Contribution` to `slot` that shows `delegate` (one object in QML),
/// indented by `unit`. `mode` and `order` are written only when given, so
/// the runtime's defaults stay the runtime's.
///
/// # Errors
/// [`Error::Invalid`] when `delegate` is not one object that parses: the
/// caller wrote it, so it is a bad argument, not an edit that would break.
pub fn contribution(
    slot: &str,
    mode: Option<Mode>,
    order: Option<i64>,
    delegate: &str,
    unit: &str,
) -> Result<String, Error> {
    let delegate = delegate.trim();
    let allocator = Allocator::default();
    let parsed = qml_syntax::parse(&allocator, delegate);
    if !parsed.is_ok() {
        let problem = parsed
            .diagnostics
            .first()
            .map_or_else(String::new, |e| format!(": {}", e.message));
        return Err(Error::Invalid(format!(
            "the delegate does not parse{problem}"
        )));
    }
    // The text must be the root object and nothing else: an `import` or a
    // `pragma` before it would not survive being put inside a `Component`.
    let root = &parsed.document.root;
    if parsed.panicked || root.span.start != 0 || root.span.end as usize != delegate.len() {
        return Err(Error::Invalid(
            "the delegate is one object, with no imports".to_owned(),
        ));
    }
    let mut out = format!(
        "Contribution {{\n{unit}slot: {}\n",
        crate::Literal::String(slot.to_owned()).to_qml()
    );
    if let Some(mode) = mode {
        out.push_str(&format!("{unit}mode: {}\n", mode.qml()));
    }
    if let Some(order) = order {
        out.push_str(&format!("{unit}order: {order}\n"));
    }
    out.push_str(&format!("{unit}delegate: Component {{\n"));
    for line in delegate.lines() {
        if line.trim().is_empty() {
            out.push('\n');
        } else {
            out.push_str(&format!("{unit}{unit}{line}\n"));
        }
    }
    out.push_str(&format!("{unit}}}\n}}"));
    Ok(out)
}
