# Sources:
#   docs/DESIGN.md (goal 1: the file is the document)
#   docs/MCP.md (a stale generation is refused; a change made outside Studio)

Feature: A file changed outside Studio is never written over
  The file is the document, and other programs write it too: an editor, a
  formatter, `git checkout`. So Studio compares the file with what it holds
  before every read and every edit. A change it finds takes a new generation
  like any edit, so an edit made against the old text is refused; handles
  stay with their objects; and the change is an entry in the history, so
  undo walks back through it byte for byte instead of applying old offsets
  to new text.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          Text { id: label; text: "hello" }
      }
      """
    And the studio tools are connected
    And the agent has called the tool "document_tree" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent keeps the answer's "/generation" as "read"
    And the agent keeps the answer's "/nodes/1/node" as "label"

  Scenario: An edit made against the text before the change is refused
    When the file "Main.qml" is rewritten outside Studio:
      """qml
      import QtQuick

      Item {
          id: root
          // Saved in another editor.
          Text { id: label; text: "hello" }
      }
      """
    And the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "stale_generation"
    And the refusal's "/details/given" is 1
    And the refusal's "/details/current" is 2
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          // Saved in another editor.
          Text { id: label; text: "hello" }
      }
      """

  Scenario: A read shows the change, with its new generation
    When the file "Main.qml" is rewritten outside Studio:
      """qml
      import QtQuick

      Rectangle {
      }
      """
    And the agent calls the tool "document_text" with:
      """json
      {"doc": "Main.qml"}
      """
    Then the answer's "/generation" is 2
    And the answer's "/text" is:
      """qml
      import QtQuick

      Rectangle {
      }
      """

  Scenario: An edit with no generation is made on the new text
    When the file "Main.qml" is rewritten outside Studio:
      """qml
      import QtQuick

      Item {
          id: root
          // Saved in another editor.
          Text { id: label; text: "hello" }
      }
      """
    And the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    Then the answer's "/generation" is 3
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          // Saved in another editor.
          Text { id: label; text: "bye" }
      }
      """

  Scenario: A handle stays with its object, found by its id
    When the file "Main.qml" is rewritten outside Studio:
      """qml
      import QtQuick

      Item {
          id: root
          Rectangle { color: "red" }
          Text { text: "new"; id: label }
      }
      """
    And the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$label"}}
      """
    Then the answer's "/path" is "root/1"
    And the answer's "/id" is "label"

  Scenario: A handle whose object went says so
    When the file "Main.qml" is rewritten outside Studio:
      """qml
      import QtQuick

      Item {
          id: root
      }
      """
    And the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"node": "$label"}}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "gone"

  Scenario: Undo walks back through the change, byte for byte
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    When the file "Main.qml" is rewritten outside Studio:
      """qml
      import QtQuick

      // Saved in another editor, a long way above the offsets of the last edit.
      Item {
          id: root
          Text { id: label; text: "bye" }
      }
      """
    And the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the answer's "/entry/label" is "changed outside Studio"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          Text { id: label; text: "bye" }
      }
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the answer's "/entry/label" is "set text"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          Text { id: label; text: "hello" }
      }
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "redo"}
      """
    And the agent calls the tool "history" with:
      """json
      {"action": "redo"}
      """
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      // Saved in another editor, a long way above the offsets of the last edit.
      Item {
          id: root
          Text { id: label; text: "bye" }
      }
      """

  Scenario: A change ends what could be redone
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    And the agent has called the tool "history" with:
      """json
      {"action": "undo"}
      """
    When the file "Main.qml" is rewritten outside Studio:
      """qml
      import QtQuick

      Item {
      }
      """
    And the agent calls the tool "history" with:
      """json
      {"action": "redo"}
      """
    Then the agent is refused with "not_actionable"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
      }
      """

  Scenario: A file deleted outside Studio takes its history with it
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    When the file "Main.qml" is deleted outside Studio
    And the agent calls the tool "document_text" with:
      """json
      {"doc": "Main.qml"}
      """
    Then the agent is refused with "not_found"
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the agent is refused with "not_actionable"
    And the file "Main.qml" does not exist

  Scenario: A file that comes back does not take an old generation
    When the file "Main.qml" is deleted outside Studio
    And the agent calls the tool "document_text" with:
      """json
      {"doc": "Main.qml"}
      """
    And the file "Main.qml" is rewritten outside Studio:
      """qml
      import QtQuick

      Item {
      }
      """
    And the agent calls the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "text": ""}
      """
    Then the agent is refused with "stale_ref"
