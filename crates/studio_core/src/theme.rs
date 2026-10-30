//! Tokens: the roles of `Ljos.Design`, the values the module gives them
//! and the ones a project's theme file gives them instead.
//!
//! What a role is, what a theme may say and what makes a theme wrong are
//! Ljós's: the roles come from [`ljos_design::theme::roles`], the
//! module's values from the palette and the singleton files Ljós writes,
//! and a theme is judged by [`ljos_design::theme::parse`] and
//! [`Theme::check_contrast`](ljos_design::theme::Theme::check_contrast).
//! Studio only edits the file's bytes ([`set`]): one value or member, the
//! rest of the file as it was.

use std::collections::HashMap;
use std::sync::OnceLock;

use ljos_design::module::{self, DEFAULT_HUE, Seeds, qml_role};
use ljos_design::palette::{self, Contrast, ROLES, Scheme, Seed};
use ljos_design::theme::{self as ljos_theme, Kind, Setting, Theme};
use serde_json::{Map, Value as Json, json};

pub use ljos_design::theme::{FILE, ThemeError};

/// One role: its name as a theme names it (`Colors.accent`), what it
/// holds, the module's value and the theme's.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub token: String,
    /// `color`, `int`, `real` or `curve`.
    pub kind: &'static str,
    /// A colour's is `light`, `dark`, `lightHigh` and `darkHigh`; any
    /// other role's is its value.
    pub module: Json,
    /// As `module`, a colour with only the variants the theme gives.
    pub theme: Option<Json>,
}

/// The variants a colour has, as a theme file names them.
pub const VARIANTS: [&str; 4] = ["light", "dark", "lightHigh", "darkHigh"];

/// Every role, with the module's value under `seed_hue` (the module's
/// own when `None`) and `theme`'s value where it sets one, in the order
/// the module declares them. `filter` keeps one role (`Colors.accent`)
/// or one singleton's (`Colors`).
pub fn tokens(theme: Option<&Theme>, filter: Option<&str>) -> Vec<Token> {
    let hue = ljos_theme::palette_hue(
        theme
            .and_then(Theme::seed_hue)
            .unwrap_or(f64::from(DEFAULT_HUE)),
    );
    let palettes = [
        (Scheme::Light, Contrast::Normal),
        (Scheme::Dark, Contrast::Normal),
        (Scheme::Light, Contrast::High),
        (Scheme::Dark, Contrast::High),
    ]
    .map(|(scheme, contrast)| palette::palette(Seed::hue(hue), scheme, contrast));
    let values = module_values();
    ljos_theme::roles()
        .into_iter()
        .filter_map(|role| {
            let token = format!("{}.{}", role.singleton, role.name);
            let wanted = filter.is_none_or(|f| f == token || f == role.singleton);
            if !wanted {
                return None;
            }
            let module = match role.kind {
                Kind::Color => {
                    let name = ROLES
                        .iter()
                        .find(|r| qml_role(r) == role.name)
                        .copied()
                        .unwrap_or(role.name.as_str());
                    let mut variants = Map::new();
                    for (variant, palette) in VARIANTS.iter().zip(&palettes) {
                        if let Some(color) = palette.role(name) {
                            variants.insert((*variant).to_owned(), json!(color));
                        }
                    }
                    Json::Object(variants)
                }
                _ => values.get(&token).cloned().unwrap_or(Json::Null),
            };
            let theme = theme.and_then(|t| {
                t.entries
                    .iter()
                    .find(|e| e.singleton == role.singleton && e.name == role.name)
                    .map(|e| setting(&e.value))
            });
            Some(Token {
                token,
                kind: kind(role.kind),
                module,
                theme,
            })
        })
        .collect()
}

fn kind(kind: Kind) -> &'static str {
    match kind {
        Kind::Color => "color",
        Kind::Int { .. } => "int",
        Kind::Real { .. } => "real",
        Kind::Curve => "curve",
    }
}

fn setting(value: &Setting) -> Json {
    match value {
        Setting::Color(v) => {
            let mut out = Map::new();
            for (name, color) in
                VARIANTS
                    .iter()
                    .zip([&v.light, &v.dark, &v.light_high, &v.dark_high])
            {
                if let Some(color) = color {
                    out.insert((*name).to_owned(), json!(color));
                }
            }
            Json::Object(out)
        }
        Setting::Int(n) => json!(n),
        Setting::Real(n) => json!(n),
        Setting::Curve(points) => json!(points),
    }
}

/// The values the module's singletons declare for every role that is not
/// a colour, read from the files Ljós writes for the module.
fn module_values() -> &'static HashMap<String, Json> {
    static VALUES: OnceLock<HashMap<String, Json>> = OnceLock::new();
    VALUES.get_or_init(|| {
        let mut out = HashMap::new();
        for file in module::files(Seeds::Hues) {
            let Some(singleton) = file.name.strip_suffix(".qml") else {
                continue;
            };
            if !module::SINGLETONS.contains(&singleton) {
                continue;
            }
            let Some(nodes) = crate::outline::outline(&file.text) else {
                continue;
            };
            for binding in nodes.iter().take(1).flat_map(|n| &n.bindings) {
                // A literal reads as JSON; anything else is kept as written.
                let value = serde_json::from_str(&binding.value)
                    .unwrap_or_else(|_| Json::String(binding.value.clone()));
                out.insert(format!("{singleton}.{}", binding.name), value);
            }
        }
        out
    })
}

/// Reads `text` as Ljós would.
///
/// # Errors
/// Ljós's reasons, each `file:line:column: message`.
pub fn read(file: &str, text: &str) -> Result<Theme, Vec<ThemeError>> {
    ljos_theme::parse(file, text)
}

/// Whether Ljós would load `text`: it reads, and its colours keep text
/// readable.
///
/// # Errors
/// Ljós's reasons.
pub fn check(file: &str, text: &str) -> Result<Theme, Vec<ThemeError>> {
    let theme = read(file, text)?;
    theme.check_contrast(None)?;
    Ok(theme)
}

/// Why [`set`] could not edit the text.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SetError {
    #[error("`{0}` is not a token: write it `Singleton.role`, as `Colors.accent`")]
    NotAToken(String),
    #[error("`{0}` is not a variant: light, dark, lightHigh or darkHigh")]
    NotAVariant(String),
    #[error("the theme does not set `{0}`")]
    NotSet(String),
    #[error("`{token}` is not a colour in the theme, so it has no variant `{variant}`")]
    NoVariants { token: String, variant: String },
    #[error("the theme file is not JSON Studio can edit: {0}")]
    NotJson(String),
}

/// `text` (a theme file; empty when there is none) with `token` set to
/// `value` (removed when `None`), only in `variant` of a colour when one
/// is given. Everything else keeps its bytes; what is added follows the
/// layout around it.
///
/// # Errors
/// The token or variant is malformed, there is nothing to remove, or the
/// text is not JSON.
pub fn set(
    text: &str,
    token: &str,
    variant: Option<&str>,
    value: Option<&Json>,
) -> Result<String, SetError> {
    let (singleton, role) = token
        .split_once('.')
        .filter(|(s, r)| !s.is_empty() && !r.is_empty() && !r.contains('.'))
        .ok_or_else(|| SetError::NotAToken(token.to_owned()))?;
    if let Some(v) = variant
        && !VARIANTS.contains(&v)
    {
        return Err(SetError::NotAVariant(v.to_owned()));
    }
    let not_set = || SetError::NotSet(token.to_owned());
    let value = value.map(|v| serde_json::to_string(v).expect("JSON writes"));
    // The value a member gets: the colour's variant alone, or the value.
    let leaf = |value: &str| match variant {
        Some(v) => format!("{{ {}: {value} }}", quote(v)),
        None => value.to_owned(),
    };
    if text.trim().is_empty() {
        let value = value.ok_or_else(not_set)?;
        return Ok(format!(
            "{{\n  {}: {{\n    {}: {}\n  }}\n}}\n",
            quote(singleton),
            quote(role),
            leaf(&value)
        ));
    }
    let root = json::parse(text).map_err(SetError::NotJson)?;
    let json::Node::Object(root_obj) = &root.node else {
        return Err(SetError::NotJson("the file is not an object".to_owned()));
    };
    let Some(section) = root_obj.get(singleton) else {
        let value = value.ok_or_else(not_set)?;
        let inner = Member::text(role, &leaf(&value));
        return Ok(insert(text, &root, root_obj, singleton, &|indent| {
            nest(&inner, indent)
        }));
    };
    let json::Node::Object(section_obj) = &section.value.node else {
        return Err(SetError::NotJson(format!("`{singleton}` is not an object")));
    };
    let Some(member) = section_obj.get(role) else {
        let value = value.ok_or_else(not_set)?;
        let leaf = leaf(&value);
        return Ok(insert(text, &section.value, section_obj, role, &|_| {
            leaf.clone()
        }));
    };
    let Some(variant) = variant else {
        return Ok(match value {
            Some(value) => splice(text, member.value.span.clone(), &value),
            None => remove(
                text,
                &[(&root, root_obj), (&section.value, section_obj)],
                member,
            ),
        });
    };
    match &member.value.node {
        json::Node::Object(colour) => match colour.get(variant) {
            Some(slot) => Ok(match value {
                Some(value) => splice(text, slot.value.span.clone(), &value),
                None => remove(
                    text,
                    &[
                        (&root, root_obj),
                        (&section.value, section_obj),
                        (&member.value, colour),
                    ],
                    slot,
                ),
            }),
            None => {
                let value = value.ok_or_else(not_set)?;
                Ok(insert(text, &member.value, colour, variant, &|_| {
                    value.clone()
                }))
            }
        },
        // One colour for light and dark: the two become apart.
        json::Node::String => {
            let old = &text[member.value.span.clone()];
            let mut variants: Vec<(&str, String)> =
                vec![("light", old.to_owned()), ("dark", old.to_owned())];
            match value {
                Some(value) => match variants.iter_mut().find(|(v, _)| *v == variant) {
                    Some(slot) => slot.1 = value,
                    None => variants.push((variant, value)),
                },
                None => variants.retain(|(v, _)| *v != variant),
            }
            let object = variants
                .iter()
                .map(|(v, value)| format!("{}: {value}", quote(v)))
                .collect::<Vec<_>>()
                .join(", ");
            Ok(splice(
                text,
                member.value.span.clone(),
                &format!("{{ {object} }}"),
            ))
        }
        _ => Err(SetError::NoVariants {
            token: token.to_owned(),
            variant: variant.to_owned(),
        }),
    }
}

fn quote(s: &str) -> String {
    serde_json::to_string(s).expect("JSON writes")
}

/// A member's text: `"key": value`.
struct Member;

impl Member {
    fn text(key: &str, value: &str) -> String {
        format!("{}: {value}", quote(key))
    }
}

/// Where a member goes: its indent and the step one level in, or `None`
/// on one line.
type Layout<'a> = Option<(&'a str, &'a str)>;

/// A section's value holding `inner`, one level in from `indent`; on one
/// line when `indent` is `None`.
fn nest(inner: &str, indent: Layout) -> String {
    match indent {
        Some((outer, unit)) => format!("{{\n{outer}{unit}{inner}\n{outer}}}"),
        None => format!("{{ {inner} }}"),
    }
}

fn splice(text: &str, span: std::ops::Range<usize>, with: &str) -> String {
    let mut out = String::with_capacity(text.len() + with.len());
    out.push_str(&text[..span.start]);
    out.push_str(with);
    out.push_str(&text[span.end..]);
    out
}

/// The whitespace a line starts with.
fn indent_of(text: &str, at: usize) -> &str {
    let start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    let line = &text[start..];
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// `text` with the member `key` added last to `object` (the value `of`):
/// on a line of its own at its neighbours' indent when they have lines
/// of their own, else on the same line. `value` is given the member's
/// indent and the step one level in, or `None` on one line.
fn insert(
    text: &str,
    of: &json::Value,
    object: &json::Object,
    key: &str,
    value: &dyn Fn(Layout) -> String,
) -> String {
    let open = of.span.start;
    let close = of.span.end - 1;
    let outer = indent_of(text, open);
    match object.members.last() {
        Some(last) if text[open..last.span.start].contains('\n') => {
            let indent = indent_of(text, last.span.start);
            let unit = indent.strip_prefix(outer).unwrap_or(indent);
            let member = Member::text(key, &value(Some((indent, unit))));
            splice(
                text,
                last.value.span.end..last.value.span.end,
                &format!(",\n{indent}{member}"),
            )
        }
        Some(last) => {
            let member = Member::text(key, &value(None));
            splice(
                text,
                last.value.span.end..last.value.span.end,
                &format!(", {member}"),
            )
        }
        None => {
            let member = Member::text(key, &value(None));
            splice(text, open + 1..close, &format!(" {member} "))
        }
    }
}

/// `text` without `member`, the last object of `path` holding it. An
/// object left empty goes with it, but for the file's own.
fn remove(text: &str, path: &[(&json::Value, &json::Object)], member: &json::Member) -> String {
    let (of, object) = path[path.len() - 1];
    if object.members.len() == 1 {
        if path.len() > 1 {
            let (_, parent) = path[path.len() - 2];
            let holder = parent
                .members
                .iter()
                .find(|m| std::ptr::eq(&m.value, of))
                .expect("the parent holds the object");
            return remove(text, &path[..path.len() - 1], holder);
        }
        return splice(text, of.span.start + 1..of.span.end - 1, "");
    }
    let i = object
        .members
        .iter()
        .position(|m| std::ptr::eq(m, member))
        .expect("the object holds the member");
    let span = if i == 0 {
        // Up to the next member, so its place is the first's.
        member.span.start..object.members[1].span.start
    } else {
        // From the end of the one before, taking the comma.
        object.members[i - 1].value.span.end..member.value.span.end
    };
    splice(text, span, "")
}

/// A JSON reader that keeps where each value is, for edits of one value.
mod json {
    use std::ops::Range;

    pub struct Value {
        pub span: Range<usize>,
        pub node: Node,
    }

    pub enum Node {
        Object(Object),
        Array,
        String,
        Other,
    }

    pub struct Object {
        pub members: Vec<Member>,
    }

    pub struct Member {
        pub key: String,
        /// From the key's quote to the value's end.
        pub span: Range<usize>,
        pub value: Value,
    }

    impl Object {
        pub fn get(&self, key: &str) -> Option<&Member> {
            // The last of two alike, as a JSON reader takes it.
            self.members.iter().rev().find(|m| m.key == key)
        }
    }

    pub fn parse(text: &str) -> Result<Value, String> {
        let mut reader = Reader { text, at: 0 };
        let value = reader.value()?;
        reader.space();
        if reader.at != text.len() {
            return Err(reader.fail("more after the value"));
        }
        Ok(value)
    }

    struct Reader<'t> {
        text: &'t str,
        at: usize,
    }

    impl Reader<'_> {
        fn fail(&self, what: &str) -> String {
            // In bytes: a truncated escape leaves `at` at the end at most, and
            // an escaped multi-byte character inside it.
            let read = &self.text.as_bytes()[..self.at.min(self.text.len())];
            let line = read.iter().filter(|b| **b == b'\n').count() + 1;
            format!("line {line}: {what}")
        }

        fn peek(&self) -> Option<u8> {
            self.text.as_bytes().get(self.at).copied()
        }

        fn space(&mut self) {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                self.at += 1;
            }
        }

        fn eat(&mut self, byte: u8) -> Result<(), String> {
            self.space();
            if self.peek() == Some(byte) {
                self.at += 1;
                Ok(())
            } else {
                Err(self.fail(&format!("`{}` expected", byte as char)))
            }
        }

        fn value(&mut self) -> Result<Value, String> {
            self.space();
            let start = self.at;
            let node = match self.peek() {
                Some(b'{') => Node::Object(self.object()?),
                Some(b'[') => {
                    self.array()?;
                    Node::Array
                }
                Some(b'"') => {
                    self.string()?;
                    Node::String
                }
                Some(_) => {
                    self.scalar()?;
                    Node::Other
                }
                None => return Err(self.fail("a value expected")),
            };
            Ok(Value {
                span: start..self.at,
                node,
            })
        }

        fn object(&mut self) -> Result<Object, String> {
            self.at += 1;
            let mut members = Vec::new();
            self.space();
            if self.peek() == Some(b'}') {
                self.at += 1;
                return Ok(Object { members });
            }
            loop {
                self.space();
                let start = self.at;
                if self.peek() != Some(b'"') {
                    return Err(self.fail("a key expected"));
                }
                let key = self.string()?;
                self.eat(b':')?;
                let value = self.value()?;
                members.push(Member {
                    key,
                    span: start..value.span.end,
                    value,
                });
                self.space();
                match self.peek() {
                    Some(b',') => self.at += 1,
                    Some(b'}') => {
                        self.at += 1;
                        return Ok(Object { members });
                    }
                    _ => return Err(self.fail("`,` or `}` expected")),
                }
            }
        }

        fn array(&mut self) -> Result<(), String> {
            self.at += 1;
            self.space();
            if self.peek() == Some(b']') {
                self.at += 1;
                return Ok(());
            }
            loop {
                self.value()?;
                self.space();
                match self.peek() {
                    Some(b',') => self.at += 1,
                    Some(b']') => {
                        self.at += 1;
                        return Ok(());
                    }
                    _ => return Err(self.fail("`,` or `]` expected")),
                }
            }
        }

        fn string(&mut self) -> Result<String, String> {
            let start = self.at;
            self.at += 1;
            loop {
                match self.peek() {
                    Some(b'"') => {
                        self.at += 1;
                        break;
                    }
                    // The escaped byte may be missing: the text ends here.
                    Some(b'\\') => self.at = (self.at + 2).min(self.text.len()),
                    Some(_) => self.at += 1,
                    None => return Err(self.fail("the string does not end")),
                }
            }
            serde_json::from_str(&self.text[start..self.at])
                .map_err(|e| self.fail(&format!("a string that does not read: {e}")))
        }

        fn scalar(&mut self) -> Result<(), String> {
            let start = self.at;
            while matches!(self.peek(), Some(b) if b.is_ascii_alphanumeric() || b"+-.".contains(&b))
            {
                self.at += 1;
            }
            serde_json::from_str::<serde_json::Value>(&self.text[start..self.at])
                .map(drop)
                .map_err(|_| self.fail("a value that is not JSON"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_ok(text: &str, token: &str, variant: Option<&str>, value: Option<Json>) -> String {
        set(text, token, variant, value.as_ref()).unwrap()
    }

    #[test]
    fn a_string_cut_off_in_an_escape_is_an_error_not_a_panic() {
        for text in [r#"{"a": "\"#, "{\n\"a\": \"\\é"] {
            let error = set(text, "Colors.accent", None, Some(&json!("#fff"))).unwrap_err();
            assert!(error.to_string().contains("does not end"), "{error}");
        }
    }

    #[test]
    fn a_missing_file_is_made() {
        assert_eq!(
            set_ok("", "Colors.accent", None, Some(json!("#9c4dc4"))),
            "{\n  \"Colors\": {\n    \"accent\": \"#9c4dc4\"\n  }\n}\n"
        );
    }

    #[test]
    fn a_value_is_replaced_in_place() {
        let text = "{\n    \"Colors\":   { \"seedHue\": 30, \"accent\": \"#6a1b9a\" },\n\n    \"Typography\": {\"bodySize\": 15}\n}\n";
        assert_eq!(
            set_ok(text, "Colors.accent", None, Some(json!("#9c4dc4"))),
            text.replace("#6a1b9a", "#9c4dc4")
        );
    }

    #[test]
    fn members_follow_the_layout() {
        let text = "{\n    \"Colors\": {\n        \"accent\": \"#6a1b9a\"\n    }\n}\n";
        let text = set_ok(text, "Colors.scrim", None, Some(json!("#80000000")));
        let text = set_ok(&text, "Spacing.md", None, Some(json!(20)));
        assert_eq!(
            text,
            "{\n    \"Colors\": {\n        \"accent\": \"#6a1b9a\",\n        \"scrim\": \"#80000000\"\n    },\n    \"Spacing\": {\n        \"md\": 20\n    }\n}\n"
        );
    }

    #[test]
    fn one_line_stays_one_line() {
        let text = "{ \"Colors\": { \"accent\": \"#6a1b9a\" } }";
        assert_eq!(
            set_ok(text, "Spacing.md", None, Some(json!(20))),
            "{ \"Colors\": { \"accent\": \"#6a1b9a\" }, \"Spacing\": { \"md\": 20 } }"
        );
    }

    #[test]
    fn a_variant_splits_a_colour() {
        let text = "{ \"Colors\": { \"accent\": \"#6a1b9a\" } }\n";
        assert_eq!(
            set_ok(text, "Colors.accent", Some("dark"), Some(json!("#ce93d8"))),
            "{ \"Colors\": { \"accent\": { \"light\": \"#6a1b9a\", \"dark\": \"#ce93d8\" } } }\n"
        );
    }

    #[test]
    fn a_variant_is_added_to_one_that_is_made() {
        let text = set_ok("", "Colors.accent", Some("light"), Some(json!("#6a1b9a")));
        let text = set_ok(&text, "Colors.accent", Some("dark"), Some(json!("#ce93d8")));
        assert_eq!(
            text,
            "{\n  \"Colors\": {\n    \"accent\": { \"light\": \"#6a1b9a\", \"dark\": \"#ce93d8\" }\n  }\n}\n"
        );
    }

    #[test]
    fn removing_takes_the_comma_and_an_emptied_section() {
        let text = "{ \"Colors\": { \"seedHue\": 30, \"accent\": \"#6a1b9a\" } }\n";
        assert_eq!(
            set_ok(text, "Colors.accent", None, None),
            "{ \"Colors\": { \"seedHue\": 30 } }\n"
        );
        assert_eq!(
            set_ok(text, "Colors.seedHue", None, None),
            "{ \"Colors\": { \"accent\": \"#6a1b9a\" } }\n"
        );
        let text = "{ \"Colors\": { \"accent\": \"#6a1b9a\" }, \"Spacing\": { \"md\": 20 } }";
        assert_eq!(
            set_ok(text, "Spacing.md", None, None),
            "{ \"Colors\": { \"accent\": \"#6a1b9a\" } }"
        );
    }

    #[test]
    fn nothing_to_remove_is_said() {
        assert_eq!(
            set("{}", "Colors.accent", None, None),
            Err(SetError::NotSet("Colors.accent".to_owned()))
        );
    }

    #[test]
    fn the_module_values_are_ljos_own() {
        let all = tokens(None, None);
        assert_eq!(all.len(), ljos_theme::roles().len());
        let md = tokens(None, Some("Spacing.md"));
        assert_eq!(md[0].module, json!(16));
        let accent = tokens(None, Some("Colors.accent"));
        assert_eq!(accent[0].module["light"], json!("#007cb7"));
        assert!(
            all.iter().all(|t| !t.module.is_null()),
            "{:?}",
            all.iter().find(|t| t.module.is_null())
        );
    }
}
