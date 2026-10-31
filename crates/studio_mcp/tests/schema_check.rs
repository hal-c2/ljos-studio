//! The output-schema checker the scenario runner (`features.rs`) holds every
//! tool answer to, tested on its own: a passing and a failing case per
//! keyword. The tests live here, not in the module: the runner has no test
//! harness, so tests inside it would be dead code there.

#[path = "support/schema.rs"]
mod schema;

use serde_json::{Value, json};

use schema::validate;

fn rules(schema: Value, value: Value) -> Vec<String> {
    validate(&schema, &value)
        .into_iter()
        .map(|v| v.to_string())
        .collect()
}

fn passes(schema: Value, value: Value) {
    let broken = rules(schema.clone(), value.clone());
    assert!(broken.is_empty(), "{value} against {schema}: {broken:?}");
}

#[test]
fn type_one_and_many() {
    passes(json!({"type": "string"}), json!("a"));
    passes(json!({"type": ["integer", "null"]}), json!(null));
    passes(json!({"type": "integer"}), json!(2.0));
    passes(json!({"type": "number"}), json!(2.5));
    assert_eq!(
        rules(json!({"type": "integer"}), json!(2.5)),
        ["(the root): type: number is not \"integer\""]
    );
    assert_eq!(
        rules(json!({"type": ["string", "null"]}), json!(true)),
        ["(the root): type: boolean is not [\"string\",\"null\"]"]
    );
}

#[test]
fn required() {
    passes(json!({"required": ["a"]}), json!({"a": 1}));
    assert_eq!(
        rules(json!({"required": ["a", "b"]}), json!({"a": 1})),
        ["(the root): required: `b` is missing"]
    );
}

#[test]
fn properties() {
    let schema = json!({"properties": {"a/b": {"type": "string"}}});
    passes(schema.clone(), json!({"a/b": "x", "other": 1}));
    assert_eq!(
        rules(schema, json!({"a/b": 1})),
        ["/a~1b: type: integer is not \"string\""]
    );
}

#[test]
fn additional_properties() {
    let closed = json!({"properties": {"a": {}}, "additionalProperties": false});
    passes(closed.clone(), json!({"a": 1}));
    assert_eq!(
        rules(closed, json!({"a": 1, "b": 2})),
        ["(the root): additionalProperties: `b` is not a property"]
    );
    let typed = json!({"additionalProperties": {"type": "integer"}});
    passes(typed.clone(), json!({"x": 1}));
    assert_eq!(
        rules(typed, json!({"x": "1"})),
        ["/x: type: string is not \"integer\""]
    );
}

#[test]
fn items() {
    let schema = json!({"items": {"type": "integer"}});
    passes(schema.clone(), json!([1, 2]));
    assert_eq!(
        rules(schema, json!([1, "2"])),
        ["/1: type: string is not \"integer\""]
    );
    let tuple = json!({"items": [{"type": "string"}, {"type": "integer"}]});
    passes(tuple.clone(), json!(["a", 1]));
    assert_eq!(
        rules(tuple, json!([1, 1])),
        ["/0: type: integer is not \"string\""]
    );
}

#[test]
fn enum_and_const() {
    passes(json!({"enum": ["a", "b"]}), json!("b"));
    assert_eq!(
        rules(json!({"enum": ["a", "b"]}), json!("c")),
        ["(the root): enum: \"c\" is not one of [\"a\",\"b\"]"]
    );
    passes(json!({"const": "a"}), json!("a"));
    assert_eq!(
        rules(json!({"const": "a"}), json!("b")),
        ["(the root): const: \"b\" is not \"a\""]
    );
}

#[test]
fn refs_into_defs_and_definitions() {
    let schema = json!({
        "$defs": {"A": {"type": "string"}},
        "definitions": {"B": {"type": "integer"}},
        "properties": {"a": {"$ref": "#/$defs/A"}, "b": {"$ref": "#/definitions/B"}}
    });
    passes(schema.clone(), json!({"a": "x", "b": 1}));
    assert_eq!(
        rules(schema, json!({"a": 1, "b": "x"})),
        [
            "/a: type: integer is not \"string\"",
            "/b: type: string is not \"integer\""
        ]
    );
    assert_eq!(
        rules(json!({"$ref": "#/$defs/Gone"}), json!(1)),
        ["(the root): $ref: \"#/$defs/Gone\" names no schema"]
    );
}

#[test]
fn any_one_and_all_of() {
    let any = json!({"anyOf": [{"type": "string"}, {"type": "integer"}]});
    passes(any.clone(), json!(1));
    assert_eq!(
        rules(any, json!(true)),
        ["(the root): anyOf: matches none of 2 ((the root): type: boolean is not \"string\")"]
    );
    let one = json!({"oneOf": [{"type": "integer"}, {"minimum": 0}]});
    passes(one.clone(), json!(-1));
    assert_eq!(
        rules(one, json!(1)),
        ["(the root): oneOf: matches 2 of 2, not one"]
    );
    let all = json!({"allOf": [{"type": "integer"}, {"minimum": 0}]});
    passes(all.clone(), json!(1));
    assert_eq!(
        rules(all, json!(-1)),
        ["(the root): minimum: -1 is below 0"]
    );
}

#[test]
fn minimum_and_maximum() {
    passes(json!({"minimum": 0, "maximum": 10}), json!(10));
    assert_eq!(
        rules(json!({"minimum": 0}), json!(-1)),
        ["(the root): minimum: -1 is below 0"]
    );
    assert_eq!(
        rules(json!({"maximum": 10}), json!(11)),
        ["(the root): maximum: 11 is above 10"]
    );
}

#[test]
fn format_and_annotations_are_ignored() {
    passes(
        json!({"format": "uint64", "description": "d", "title": "T", "default": 1}),
        json!(-1),
    );
}

#[test]
fn boolean_schemas() {
    passes(json!(true), json!(1));
    assert_eq!(
        rules(json!(false), json!(1)),
        ["(the root): the schema `false` allows nothing"]
    );
}

#[test]
fn an_unknown_keyword_is_refused_not_passed() {
    assert_eq!(
        rules(json!({"pattern": "^a$"}), json!("b")),
        ["(the root): pattern: this checker does not know the keyword"]
    );
}
