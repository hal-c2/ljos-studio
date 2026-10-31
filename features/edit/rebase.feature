# Sources:
#   docs/MULTIPLAYER.md (section 5, items 1 and 2; section 6, seams 1 to 3)
#   docs/MCP.md (an edit names the generation it was read at)

Feature: An edit made against an older generation is rebased by its handles
  Several writers edit one document: agents over MCP and a person in the
  window. An edit names the generation it was read at. When the document
  has moved on since, Studio does not refuse it outright: an edit that
  names its objects by handle or `id` is applied to the text as it is now,
  unless another writer changed the same binding or the object itself in
  between. Then it is refused as a conflict that says who changed what and
  what is there now, so the writer can read again and decide. A path is
  true of one text only, so an edit by path is still refused as stale. A
  code-view edit is moved past what was written since by its byte ranges.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "red" }
          Text { id: label; text: "hello" }
      }
      """
    And the studio tools are connected
    And the agent has called the tool "document_tree" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent keeps the answer's "/generation" as "read"
    And the agent keeps the answer's "/nodes/1/node" as "card"

  Scenario: An edit is rebased past another writer's edit of another object
    Given a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"node": "$card"}, "name": "color", "value": "blue"}
      """
    Then the answer's "/rebased_from" is 1
    And the answer's "/generation" is 3
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "blue" }
          Text { id: label; text: "bye" }
      }
      """

  Scenario: An edit is rebased past another binding of the same object
    Given a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "width", "value": 40}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"id": "card"}, "name": "color", "value": "blue"}
      """
    Then the answer's "/rebased_from" is 1
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "blue"; width: 40 }
          Text { id: label; text: "hello" }
      }
      """

  Scenario: Another writer's change of the same binding is a conflict
    Given a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "color", "value": "green"}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"node": "$card"}, "name": "color", "value": "blue"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"
    And the refusal's "/details/label" is "set color"
    And the refusal's "/details/generation" is 2
    And the refusal's "/details/name" is "color"
    And the refusal's "/details/now" is "\"green\""
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "green" }
          Text { id: label; text: "hello" }
      }
      """

  Scenario: An object another writer removed is a conflict
    Given a second agent named "Ada" has called the tool "node_remove" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$card"}}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"node": "$card"}, "name": "color", "value": "blue"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"
    And the refusal's "/details/label" is "remove"
    And the refusal's "/details/now" is null

  Scenario: An object removed with its parent is a conflict
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "card"}, "text": "Rectangle { color: \"white\" }"}
      """
    And the agent keeps the answer's "/node" as "dot"
    And the agent keeps the answer's "/generation" as "read"
    And a second agent named "Ada" has called the tool "node_remove" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$card"}}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"node": "$dot"}, "name": "color", "value": "blue"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/label" is "remove"

  Scenario: A writer's own burst is rebased past its own edits
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"id": "card"}, "name": "color", "value": "green"}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"id": "card"}, "name": "color", "value": "blue"}
      """
    Then the answer's "/rebased_from" is 1
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "blue" }
          Text { id: label; text: "hello" }
      }
      """

  Scenario: An edit at the current generation says nothing of a rebase
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"id": "card"}, "name": "color", "value": "blue"}
      """
    Then the answer's "/rebased_from" is absent

  Scenario: An edit by path against an older generation is still refused
    Given a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"path": "root/0"}, "name": "color", "value": "blue"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "stale_generation"

  Scenario: A code-view edit is rebased past a structural edit elsewhere
    Given a second agent named "Ada" has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "index": 0, "text": "Rectangle { id: header }"}
      """
    # Bytes 121..128 are `"hello"` in the text that was read.
    When the agent calls the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "start": 121, "end": 128, "text": "\"bye\""}
      """
    Then the answer's "/rebased_from" is 1
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: header }
          Rectangle { id: card; color: "red" }
          Text { id: label; text: "bye" }
      }
      """

  Scenario: A code-view edit of bytes another writer changed is a conflict
    Given a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    When the agent calls the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "start": 121, "end": 128, "text": "\"hi\""}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"
    And the refusal's "/details/label" is "set text"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "red" }
          Text { id: label; text: "bye" }
      }
      """

  Scenario: A code-view edit that would newly break the document is refused
    # Ada comments out the line with the label; the agent, at the text it
    # read, opened the label's braces onto lines of their own. Either alone
    # parses; together the closing brace is left over.
    Given a second agent named "Ada" has called the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "start": 93, "end": 93, "text": "// "}
      """
    When the agent calls the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "start": 128, "end": 130, "text": "\n        font.bold: true\n    }"}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's message matches "would no longer parse"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "red" }
      //     Text { id: label; text: "hello" }
      }
      """

  Scenario: The history says who made each edit
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "color", "value": "blue"}
      """
    And a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "list"}
      """
    Then the answer's "/done/0/author" is "agent"
    And the answer's "/done/1/author" is "Ada"

  Scenario: Undo takes back the caller's own edit past another writer's later one
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "color", "value": "blue"}
      """
    And a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the answer's "/entry/label" is "set color"
    And the answer's "/entry/author" is "agent"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "red" }
          Text { id: label; text: "bye" }
      }
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "redo"}
      """
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "blue" }
          Text { id: label; text: "bye" }
      }
      """

  Scenario: Undo of an edit another writer has written over is a conflict
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    And a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "hi"}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"
    And the refusal's "/details/label" is "set text"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "red" }
          Text { id: label; text: "hi" }
      }
      """

  Scenario: Undo of all takes back the newest edit, whoever made it
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "color", "value": "blue"}
      """
    And a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo", "all": true}
      """
    Then the answer's "/entry/label" is "set text"
    And the answer's "/entry/author" is "Ada"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "blue" }
          Text { id: label; text: "hello" }
      }
      """
