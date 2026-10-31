//! What `tools/list` says of each tool, cut to what an agent acts on. Every
//! session pays for the whole list before its first call, and the list is one
//! answer, under the 40 KB cap of any other.
//!
//! Arguments keep every description: they are how a call gets written right.
//! An answer's schema keeps its fields and their types and no more, since the
//! answer shows the rest; its full shape stays in the types, and the scenarios
//! hold every answer to that (`answer_schema`).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use ljos_mcp_core::ToolDef;
use schemars::JsonSchema;
use serde_json::{Map, Value, json};

static ANSWERS: Mutex<BTreeMap<&'static str, fn() -> Value>> = Mutex::new(BTreeMap::new());

pub(crate) trait Answers {
    /// `ToolDef::output`, for the list: `O` is what the tool answers.
    #[must_use]
    fn answers<O: JsonSchema>(self) -> Self;
}

impl Answers for ToolDef {
    fn answers<O: JsonSchema>(mut self) -> Self {
        let mut input = (*self.input_schema).clone();
        trim(&mut input);
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
    schema.remove("title");
    schema.remove("format");
    schema.remove("minimum");
    if schema.get("additionalProperties") == Some(&Value::Bool(false)) {
        schema.remove("additionalProperties");
    }
    for (key, value) in schema.iter_mut() {
        // Under these the keys are names, and a field may be called `title`.
        if key == "properties" || key == "$defs" {
            if let Value::Object(named) = value {
                named.values_mut().for_each(trim_value);
            }
        } else {
            trim_value(value);
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
    let fields: Map<String, Value> = fields
        .iter()
        .map(|(name, field)| (name.clone(), field_only(field)))
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
    Value::Object(out)
}
