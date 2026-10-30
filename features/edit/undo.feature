# Sources:
#   docs/DESIGN.md (goal 1: the file is the document; S1)
#   docs/MCP.md (every mutate tool returns a history id)

Feature: Undo gives back the bytes
  Studio edits files losslessly and records each edit as the ranges it
  changed and their inverses. Undo puts the inverse back, so after any run
  of edits and undos the file is byte for byte what it was: comments,
  blank lines and the author's layout included.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      // The window's content.
      Item {
          id: root
          width: 200 // wide enough

          Rectangle { color: "red" }
          Text {
              text: "hello"
          }
      }
      """
    And the studio tools are connected

  Scenario: An insert, a property and a move are undone to the original file
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Rectangle { color: \"blue\" }"}
      """
    And the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "rectangle1"}, "name": "width", "value": 40}
      """
    And the agent has called the tool "node_move" with:
      """json
      {"doc": "Main.qml", "node": {"path": "root/2"}, "parent": {"id": "root"}, "index": 0}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "list"}
      """
    Then the answer's "/done" has 3 entries
    And the answer's "/done/1/label" is "set width"
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    And the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    And the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the answer's "/done" has 0 entries
    And the answer's "/undone" has 3 entries
    And the file "Main.qml" is:
      """qml
      import QtQuick

      // The window's content.
      Item {
          id: root
          width: 200 // wide enough

          Rectangle { color: "red" }
          Text {
              text: "hello"
          }
      }
      """

  Scenario: Redo puts the edit back
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "index": 0, "text": "Item {}"}
      """
    And the agent has called the tool "history" with:
      """json
      {"action": "undo"}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "redo"}
      """
    Then the answer's "/entry/label" is "insert Item"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      // The window's content.
      Item {
          id: root
          width: 200 // wide enough

          Item { id: item1 }
          Rectangle { color: "red" }
          Text {
              text: "hello"
          }
      }
      """

  Scenario: There is nothing to undo in a new session
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "nothing_to_do"

  Scenario: An edit that changes nothing is said so and leaves no entry
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": 1, "node": {"id": "root"}, "name": "width", "value": 200}
      """
    Then the answer's "/unchanged" is true
    And the answer's "/generation" is 1
    And the answer's "/changes" has 0 entries
    And the answer's "/history" is absent
    When the agent calls the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": 1, "start": 0, "end": 6, "text": "import"}
      """
    Then the answer's "/unchanged" is true
    And the answer's "/generation" is 1
    When the agent calls the tool "history" with:
      """json
      {"action": "list"}
      """
    Then the answer's "/done" has 0 entries
