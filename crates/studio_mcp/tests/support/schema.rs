//! A JSON Schema check for the subset schemars emits, so a scenario fails
//! when a tool answers something its advertised `outputSchema` does not
//! allow (a client SDK that validates rejects such an answer). No validator
//! crate is a dependency; this one knows exactly the keywords below and
//! refuses any other validation keyword rather than passing it unchecked.

use serde_json::Value;

/// Where the answer breaks the schema, and which rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// JSON pointer into the answer.
    pub pointer: String,
    pub rule: String,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let at = if self.pointer.is_empty() {
            "(the root)"
        } else {
            &self.pointer
        };
        write!(f, "{at}: {}", self.rule)
    }
}

/// Every violation of `schema` by `value`; empty when it conforms.
pub fn validate(schema: &Value, value: &Value) -> Vec<Violation> {
    let mut out = Vec::new();
    Check { root: schema }.at(schema, value, "", &mut out);
    out
}

/// Keywords that describe rather than constrain.
const ANNOTATIONS: &[&str] = &[
    "$schema",
    "$id",
    "$comment",
    "$defs",
    "definitions",
    "title",
    "description",
    "default",
    "examples",
    "format",
    "readOnly",
    "writeOnly",
    "deprecated",
];

struct Check<'s> {
    root: &'s Value,
}

impl<'s> Check<'s> {
    fn at(&self, schema: &'s Value, value: &Value, pointer: &str, out: &mut Vec<Violation>) {
        let fail = |out: &mut Vec<Violation>, rule: String| {
            out.push(Violation {
                pointer: pointer.to_owned(),
                rule,
            });
        };
        let schema = match schema {
            Value::Bool(true) => return,
            Value::Bool(false) => return fail(out, "the schema `false` allows nothing".into()),
            Value::Object(s) => s,
            other => return fail(out, format!("the schema {other} is not a schema")),
        };
        for (key, rule) in schema {
            match key.as_str() {
                "type" => {
                    let names: Vec<&str> = match rule {
                        Value::String(t) => vec![t.as_str()],
                        Value::Array(ts) => ts.iter().filter_map(Value::as_str).collect(),
                        _ => vec![],
                    };
                    if !names.iter().any(|t| is_type(value, t)) {
                        fail(out, format!("type: {} is not {rule}", type_name(value)));
                    }
                }
                "enum" => {
                    let allowed = rule.as_array().map(Vec::as_slice).unwrap_or_default();
                    if !allowed.contains(value) {
                        fail(out, format!("enum: {value} is not one of {rule}"));
                    }
                }
                "const" => {
                    if value != rule {
                        fail(out, format!("const: {value} is not {rule}"));
                    }
                }
                "minimum" => {
                    if let (Some(v), Some(min)) = (value.as_f64(), rule.as_f64())
                        && v < min
                    {
                        fail(out, format!("minimum: {value} is below {rule}"));
                    }
                }
                "maximum" => {
                    if let (Some(v), Some(max)) = (value.as_f64(), rule.as_f64())
                        && v > max
                    {
                        fail(out, format!("maximum: {value} is above {rule}"));
                    }
                }
                "required" => {
                    if let Value::Object(obj) = value {
                        for name in rule.as_array().into_iter().flatten() {
                            let name = name.as_str().unwrap_or_default();
                            if !obj.contains_key(name) {
                                fail(out, format!("required: `{name}` is missing"));
                            }
                        }
                    }
                }
                "properties" => {
                    if let (Value::Object(obj), Value::Object(props)) = (value, rule) {
                        for (name, sub) in props {
                            if let Some(v) = obj.get(name) {
                                self.at(sub, v, &child(pointer, name), out);
                            }
                        }
                    }
                }
                "additionalProperties" => {
                    if let Value::Object(obj) = value {
                        let props = schema.get("properties").and_then(Value::as_object);
                        for (name, v) in obj {
                            if props.is_some_and(|p| p.contains_key(name)) {
                                continue;
                            }
                            if *rule == Value::Bool(false) {
                                fail(
                                    out,
                                    format!("additionalProperties: `{name}` is not a property"),
                                );
                            } else {
                                self.at(rule, v, &child(pointer, name), out);
                            }
                        }
                    }
                }
                "items" => {
                    if let Value::Array(items) = value {
                        for (i, v) in items.iter().enumerate() {
                            let sub = match rule {
                                // The tuple form of drafts before 2020-12.
                                Value::Array(each) => match each.get(i) {
                                    Some(s) => s,
                                    None => continue,
                                },
                                one => one,
                            };
                            self.at(sub, v, &child(pointer, &i.to_string()), out);
                        }
                    }
                }
                "$ref" => match rule.as_str().and_then(|r| self.resolve(r)) {
                    Some(target) => self.at(target, value, pointer, out),
                    None => fail(out, format!("$ref: {rule} names no schema")),
                },
                "allOf" | "anyOf" | "oneOf" => {
                    let subs = rule.as_array().map(Vec::as_slice).unwrap_or_default();
                    let results: Vec<Vec<Violation>> = subs
                        .iter()
                        .map(|s| {
                            let mut v = Vec::new();
                            self.at(s, value, pointer, &mut v);
                            v
                        })
                        .collect();
                    let passed = results.iter().filter(|r| r.is_empty()).count();
                    match key.as_str() {
                        "allOf" => out.extend(results.into_iter().flatten()),
                        "anyOf" if passed == 0 => fail(
                            out,
                            format!(
                                "anyOf: matches none of {} ({})",
                                subs.len(),
                                first(&results)
                            ),
                        ),
                        "oneOf" if passed != 1 => fail(
                            out,
                            format!(
                                "oneOf: matches {passed} of {}, not one{}",
                                subs.len(),
                                if passed == 0 {
                                    format!(" ({})", first(&results))
                                } else {
                                    String::new()
                                }
                            ),
                        ),
                        _ => {}
                    }
                }
                k if ANNOTATIONS.contains(&k) => {}
                k => fail(out, format!("{k}: this checker does not know the keyword")),
            }
        }
    }

    /// A `#/…` pointer into the root schema (`$defs` and `definitions`).
    fn resolve(&self, reference: &str) -> Option<&'s Value> {
        self.root.pointer(reference.strip_prefix('#')?)
    }
}

fn first(results: &[Vec<Violation>]) -> String {
    results
        .iter()
        .flatten()
        .next()
        .map(ToString::to_string)
        .unwrap_or_default()
}

fn child(pointer: &str, step: &str) -> String {
    format!("{pointer}/{}", step.replace('~', "~0").replace('/', "~1"))
}

fn is_type(value: &Value, name: &str) -> bool {
    match name {
        "null" => value.is_null(),
        "boolean" => value.is_boolean(),
        "string" => value.is_string(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        "number" => value.is_number(),
        // JSON Schema counts 1.0 as an integer.
        "integer" => {
            value.is_i64() || value.is_u64() || value.as_f64().is_some_and(|f| f.fract() == 0.0)
        }
        _ => false,
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_f64() => "number",
        Value::Number(_) => "integer",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
