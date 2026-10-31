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

  # The second undo's bytes were changed by the edit the first undo took
  # back, which is no change any more.
  Scenario: Undo takes back a writer's edits of one binding in turn, past another writer's
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "color", "value": "blue"}
      """
    And a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    And the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "color", "value": "green"}
      """
    And the agent has called the tool "history" with:
      """json
      {"action": "undo"}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "red" }
          Text { id: label; text: "bye" }
      }
      """

  Scenario: A conflict with a change of a text that is no one object's names no object
    Given a second agent named "Ada" has called the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "start": 85, "end": 128, "text": "\"blue\" }\n    Text { id: label; text: \"bye\""}
      """
    When the agent calls the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "start": 85, "end": 90, "text": "\"pink\""}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/handle" is null
    And the refusal's "/details/author" is "Ada"
    And the refusal's message matches "Ada changed the same text in"

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

  Scenario: Writing a binding conflicts with a change inside the object written in it
    Given the file "Boxed.qml":
      """qml
      import QtQuick

      Item {
          id: root
          box: Rectangle { id: inner; color: "red" }
      }
      """
    And the agent has called the tool "document_tree" with:
      """json
      {"doc": "Boxed.qml"}
      """
    And the agent keeps the answer's "/generation" as "boxed"
    And a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Boxed.qml", "generation": "$boxed", "node": {"id": "inner"}, "name": "color", "value": "blue"}
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Boxed.qml", "generation": "$boxed", "node": {"id": "root"}, "name": "box", "value": "Item {}"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"

  Scenario: A child index read before another writer's insert is a conflict
    Given a second agent named "Ada" has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "parent": {"id": "root"}, "index": 0, "text": "Item { id: ada }"}
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "parent": {"id": "root"}, "index": 1, "text": "Item { id: mine }"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"

  Scenario: A child index read before another writer's remove is a conflict
    Given a second agent named "Ada" has called the tool "node_remove" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"id": "card"}}
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "parent": {"id": "root"}, "index": 1, "text": "Item { id: mine }"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"

  Scenario: An insert with no index still lands past another writer's
    Given a second agent named "Ada" has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "parent": {"id": "root"}, "index": 0, "text": "Item { id: ada }"}
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "parent": {"id": "root"}, "text": "Item { id: mine }"}
      """
    Then the answer's "/rebased_from" is 1

  Scenario: Writing a handler conflicts with another writer's flow on it
    Given a second agent named "Ada" has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "action": "interaction", "trigger": {"id": "card"}, "signal": "colorChanged", "state": ""}
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"id": "card"}, "name": "onColorChanged", "value": "console.log(1)"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"

  Scenario: A child index read before another writer's tap flow is a conflict
    Given a second agent named "Ada" has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "action": "interaction", "trigger": {"id": "root"}, "state": ""}
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "parent": {"id": "root"}, "index": 1, "text": "Item { id: mine }"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"

  Scenario: A child index read before a text edit that took out a child is a conflict
    Given a second agent named "Ada" has called the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "start": 56, "end": 92, "text": ""}
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "parent": {"id": "root"}, "index": 1, "text": "Item { id: mine }"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"

  Scenario: A flow on an object another writer took out is a conflict
    Given a second agent named "Ada" has called the tool "node_remove" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"id": "card"}}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "action": "interaction", "trigger": {"node": "$card"}, "signal": "colorChanged", "state": ""}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"

  Scenario: A flow on a handler another writer wrote is a conflict
    Given a second agent named "Ada" has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"id": "card"}, "name": "onColorChanged", "value": "console.log(1)"}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "action": "interaction", "trigger": {"node": "$card"}, "signal": "colorChanged", "state": ""}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"

  Scenario: A tap flow after another writer's tap flow on the object is a conflict
    Given a second agent named "Ada" has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "action": "interaction", "trigger": {"node": "$card"}, "state": ""}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "action": "interaction", "trigger": {"node": "$card"}, "state": ""}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"

  Scenario: Taking out a flow another writer already took out is a conflict
    Given the agent has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "action": "interaction", "trigger": {"node": "$card"}, "signal": "colorChanged", "state": ""}
      """
    And the agent keeps the answer's "/generation" as "flowed"
    And a second agent named "Ada" has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$flowed", "action": "remove_interaction", "trigger": {"node": "$card"}, "signal": "colorChanged"}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$flowed", "action": "remove_interaction", "trigger": {"node": "$card"}, "signal": "colorChanged"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"

  Scenario: Taking out a tap flow whose handler another writer changed is a conflict
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "card"}, "text": "TapHandler { id: tap; enabled: true; onTapped: root.state = \"\" }"}
      """
    And the agent keeps the answer's "/generation" as "tapped"
    And a second agent named "Ada" has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "generation": "$tapped", "node": {"id": "tap"}, "name": "onTapped", "value": "card.state = \"\""}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$tapped", "action": "remove_interaction", "trigger": {"node": "$card"}}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"

  Scenario: Taking out a tap flow another writer already took out of its handler is a conflict
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "card"}, "text": "TapHandler { id: tap; enabled: true; onTapped: root.state = \"\" }"}
      """
    And the agent keeps the answer's "/generation" as "tapped"
    And a second agent named "Ada" has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$tapped", "action": "remove_interaction", "trigger": {"id": "card"}}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$tapped", "action": "remove_interaction", "trigger": {"node": "$card"}}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"

  Scenario: A tap flow lands past a child another writer added to the object
    Given a second agent named "Ada" has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "parent": {"id": "card"}, "text": "Item { id: ada }"}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "action": "interaction", "trigger": {"node": "$card"}, "state": ""}
      """
    Then the answer's "/rebased_from" is 1
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle {
              id: card
              color: "red"
              Item { id: ada }
              TapHandler {
                  onTapped: root.state = ""
              }
          }
          Text { id: label; text: "hello" }
      }
      """

  Scenario: Taking out a tap flow lands past a change to another TapHandler of the object
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "card"}, "text": "TapHandler { id: one; onTapped: root.state = \"\" }"}
      """
    And the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "card"}, "text": "TapHandler { id: two; enabled: true }"}
      """
    And the agent keeps the answer's "/generation" as "tapped"
    And a second agent named "Ada" has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "generation": "$tapped", "node": {"id": "two"}, "name": "enabled", "value": "false"}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$tapped", "action": "remove_interaction", "trigger": {"node": "$card"}}
      """
    Then the answer's "/rebased_from" is 3
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { id: card; color: "red"; TapHandler { id: one } TapHandler { id: two; enabled: false } }
          Text { id: label; text: "hello" }
      }
      """

  Scenario: Taking out a tap flow another writer replaced is a conflict
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "card"}, "text": "TapHandler { id: mine; onTapped: root.state = \"\" }"}
      """
    And the agent keeps the answer's "/generation" as "tapped"
    And a second agent named "Ada" has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$tapped", "action": "remove_interaction", "trigger": {"id": "card"}}
      """
    And a second agent named "Ada" has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "card"}, "text": "TapHandler { id: adas; onTapped: root.state = \"\" }"}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$tapped", "action": "remove_interaction", "trigger": {"node": "$card"}}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"

  Scenario: A conflict over a tap flow names who took it out, not who added a child since
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "card"}, "text": "TapHandler { id: mine; onTapped: root.state = \"\" }"}
      """
    And the agent keeps the answer's "/generation" as "tapped"
    And a second agent named "Ada" has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$tapped", "action": "remove_interaction", "trigger": {"id": "card"}}
      """
    And a second agent named "Bob" has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "card"}, "text": "Item { id: bob }"}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$tapped", "action": "remove_interaction", "trigger": {"node": "$card"}}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"
