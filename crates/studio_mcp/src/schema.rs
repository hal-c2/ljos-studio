//! What `tools/list` says of each tool, cut to what an agent acts on. Every
//! session pays for the whole list before its first call, and the list is one
//! answer, under the 40 KB cap of any other.
//!
//! Arguments keep every description: they are how a call gets written right.
//! An answer's schema keeps its fields and their types and no more, since the
//! answer shows the rest; its full shape stays in the types, and the scenarios
//! hold every answer to that (`answer_schema`). A field an answer does not
//! always have is named and not typed, and a type used once is written where
//! it is used.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use ljos_mcp_core::ToolDef;
use schemars::JsonSchema;
use serde_json::{Map, Value, json};

type AnswerTable = BTreeMap<&'static str, fn() -> Value>;

static ANSWERS: Mutex<AnswerTable> = Mutex::new(BTreeMap::new());

pub(crate) trait Answers {
    /// `ToolDef::output`, for the list: `O` is what the tool answers.
    #[must_use]
    fn answers<O: JsonSchema>(self) -> Self;
}

impl Answers for ToolDef {
    fn answers<O: JsonSchema>(mut self) -> Self {
        let mut input = (*self.input_schema).clone();
        trim(&mut input);
        inline_single_use(&mut input);
        self.input_schema = Arc::new(input);
        let full = full_schema::<O>();
        self.output_schema = Some(Arc::new(fields_only(&full)));
        ANSWERS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(self.name, || full_schema::<O>().into());
        self
    }
}

/// The whole shape of what `tool` answers, which the list leaves out.
#[must_use]
pub fn answer_schema(tool: &str) -> Option<Value> {
    let answers = ANSWERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    answers.get(tool).map(|schema| schema())
}

fn full_schema<O: JsonSchema>() -> Map<String, Value> {
    let Value::Object(map) = schemars::schema_for!(O).to_value() else {
        unreachable!("a schemars root schema is always an object")
    };
    map
}

/// Removes what no call depends on: the server refuses unknown arguments and
/// bad numbers whatever the schema says.
fn trim(schema: &mut Map<String, Value>) {
    trim_field(schema, false);
}

fn trim_field(schema: &mut Map<String, Value>, required: bool) {
    schema.remove("title");
    schema.remove("format");
    schema.remove("minimum");
    if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
        schema.remove("additionalProperties");
    }
    // A bare boolean is false when left out, and the server counts the
    // members of an address whatever the schema says.
    if schema.get("default") == Some(&Value::Bool(false)) {
        schema.remove("default");
    }
    if matches!(schema.get("default"), Some(Value::Null))
        || schema.get("default") == Some(&json!([]))
    {
        schema.remove("default");
    }
    // A doc comment's line breaks cost a byte more than the space they are.
    if let Some(Value::String(note)) = schema.get_mut("description") {
        *note = note.replace('\n', " ");
    }
    schema.remove("minProperties");
    schema.remove("maxProperties");
    merge_choices(schema);
    if !required {
        not_null(schema);
    }
    let must: Vec<String> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|r| {
            r.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    for (key, value) in schema.iter_mut() {
        // Under these the keys are names, and a field may be called `title`.
        if key == "properties" || key == "$defs" {
            if let Value::Object(named) = value {
                for (name, field) in named.iter_mut() {
                    if let (true, Value::Object(field)) = (must.contains(name), &mut *field) {
                        trim_field(field, key == "properties");
                    } else {
                        trim_value(field);
                    }
                }
                // A named type's note restates the fields that use it; only
                // an address is used bare, so it keeps how it is written.
                if key == "$defs" {
                    for (name, def) in named.iter_mut() {
                        if name != "Address"
                            && let Value::Object(def) = def
                        {
                            def.remove("description");
                        }
                    }
                }
            }
        } else {
            trim_value(value);
        }
    }
}

/// A field that may be `null` and need not be there is one that may be left
/// out, which the `required` list already says: `["string", "null"]` is
/// `"string"`, and `anyOf: [X, null]` is `X` with the field's own description
/// beside it. One that must be there says `null` for a reason, such as removing
/// the value (`trim_field`), and keeps it.
fn not_null(schema: &mut Map<String, Value>) {
    if let Some(Value::Array(types)) = schema.get("type")
        && types.len() == 2
        && types.iter().any(|t| t == "null")
        && let Some(other) = types.iter().find(|t| *t != "null").cloned()
    {
        schema.insert("type".to_owned(), other);
    }
    if let Some(Value::Array(choices)) = schema.get("anyOf")
        && choices.len() == 2
        && choices.iter().any(|c| c == &json!({ "type": "null" }))
        && let Some(Value::Object(other)) = choices
            .iter()
            .find(|c| *c != &json!({ "type": "null" }))
            .cloned()
    {
        schema.remove("anyOf");
        for (key, value) in other {
            schema.entry(key).or_insert(value);
        }
    }
}

/// A named type used once is written where it is used: its name and the
/// reference to it cost more than they save.
fn inline_single_use(schema: &mut Map<String, Value>) {
    let Some(Value::Object(defs)) = schema.get("$defs").cloned() else {
        return;
    };
    let mut uses: BTreeMap<String, usize> = BTreeMap::new();
    count_refs(&Value::Object(schema.clone()), &mut uses);
    let once: Vec<&String> = defs
        .keys()
        .filter(|name| uses.get(&format!("#/$defs/{name}")) == Some(&1))
        .collect();
    if once.is_empty() {
        return;
    }
    for value in schema.values_mut() {
        inline_refs(value, &defs, &once);
    }
    if let Some(Value::Object(left)) = schema.get_mut("$defs") {
        for name in &once {
            left.remove(*name);
        }
        if left.is_empty() {
            schema.remove("$defs");
        }
    }
}

fn count_refs(value: &Value, uses: &mut BTreeMap<String, usize>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref") {
                *uses.entry(r.clone()).or_default() += 1;
            }
            map.values().for_each(|v| count_refs(v, uses));
        }
        Value::Array(items) => items.iter().for_each(|v| count_refs(v, uses)),
        _ => {}
    }
}

fn inline_refs(value: &mut Value, defs: &Map<String, Value>, once: &[&String]) {
    match value {
        Value::Object(map) => {
            let named = map
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|r| r.strip_prefix("#/$defs/"))
                .filter(|name| once.iter().any(|o| o.as_str() == *name))
                .and_then(|name| defs.get(name));
            if let Some(Value::Object(def)) = named {
                map.remove("$ref");
                for (key, v) in def {
                    // The field's own note comes first, and the type's may be
                    // all that says how the field is written.
                    match (key.as_str(), map.get_mut(key), v) {
                        ("description", Some(Value::String(own)), Value::String(note)) => {
                            own.push(' ');
                            own.push_str(note);
                        }
                        _ => {
                            map.entry(key.clone()).or_insert_with(|| v.clone());
                        }
                    }
                }
            }
            map.values_mut().for_each(|v| inline_refs(v, defs, once));
        }
        Value::Array(items) => items.iter_mut().for_each(|v| inline_refs(v, defs, once)),
        _ => {}
    }
}

/// Choices that only name scalar types are one `type` list, and choices that
/// are only strings are one `enum`: the per-choice notes restate what the
/// fields that use them already say.
fn merge_choices(schema: &mut Map<String, Value>) {
    for key in ["anyOf", "oneOf"] {
        let Some(Value::Array(choices)) = schema.get(key) else {
            continue;
        };
        let scalars: Option<Vec<Value>> = choices
            .iter()
            .map(|c| match c.as_object() {
                Some(o) if o.len() == 1 => o.get("type").filter(|t| t.is_string()).cloned(),
                _ => None,
            })
            .collect();
        let names: Option<Vec<Value>> = choices
            .iter()
            .map(|c| {
                let o = c
                    .as_object()
                    .filter(|o| o.get("type") == Some(&json!("string")))?;
                match (o.get("enum"), o.get("const")) {
                    (Some(Value::Array(e)), None) => Some(e.clone()),
                    (None, Some(c)) => Some(vec![c.clone()]),
                    _ => None,
                }
            })
            .collect::<Option<Vec<_>>>()
            .map(|v| v.concat());
        if let Some(types) = scalars {
            schema.remove(key);
            schema.insert("type".to_owned(), Value::Array(types));
        } else if let Some(names) = names {
            schema.remove(key);
            schema.insert("type".to_owned(), json!("string"));
            schema.insert("enum".to_owned(), Value::Array(names));
        }
    }
}

fn trim_value(value: &mut Value) {
    match value {
        Value::Object(map) => trim(map),
        Value::Array(items) => items.iter_mut().for_each(trim_value),
        _ => {}
    }
}

fn fields_only(schema: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("type".to_owned(), json!("object"));
    let Some(Value::Object(fields)) = schema.get("properties") else {
        return out;
    };
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    // What is not always there is named and no more: its type is the
    // answer's to show when it is.
    let fields: Map<String, Value> = fields
        .iter()
        .map(|(name, field)| {
            let field = if required.contains(&name.as_str()) {
                field_only(field)
            } else {
                Value::Object(Map::new())
            };
            (name.clone(), field)
        })
        .collect();
    out.insert("properties".to_owned(), Value::Object(fields));
    if let Some(required) = schema.get("required") {
        out.insert("required".to_owned(), required.clone());
    }
    out
}

fn field_only(field: &Value) -> Value {
    let mut out = Map::new();
    // A field of a named type or a choice of them says nothing without them.
    for key in ["type", "enum"] {
        if let Some(value) = field.get(key) {
            out.insert(key.to_owned(), value.clone());
        }
    }
    not_null(&mut out);
    Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trimmed(schema: Value) -> Value {
        let Value::Object(mut schema) = schema else {
            unreachable!()
        };
        trim(&mut schema);
        inline_single_use(&mut schema);
        Value::Object(schema)
    }

    #[test]
    fn a_field_that_must_be_there_keeps_its_null() {
        let schema = trimmed(json!({
            "properties": {
                "value": {"type": ["string", "null"]},
                "note": {"type": ["string", "null"]},
                "either": {"type": ["string", "number"]}
            },
            "required": ["value"]
        }));
        assert_eq!(
            schema["properties"]["value"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(schema["properties"]["note"]["type"], json!("string"));
        assert_eq!(
            schema["properties"]["either"]["type"],
            json!(["string", "number"])
        );
    }

    #[test]
    fn a_type_written_where_it_is_used_keeps_both_notes() {
        let schema = trimmed(json!({
            "properties": {"node": {"$ref": "#/$defs/Address", "description": "The object."}},
            "$defs": {"Address": {"type": "object", "description": "Named by `id` or `path`."}}
        }));
        assert_eq!(
            schema["properties"]["node"]["description"],
            json!("The object. Named by `id` or `path`.")
        );
    }
}
