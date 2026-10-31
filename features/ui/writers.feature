# Sources:
#   docs/MULTIPLAYER.md (section 5, items 3 and 4; section 6, seams 4 and 5)
#   features/edit/rebase.feature, features/edit/lease.feature (the same
#   rules over MCP)

@ui
Feature: The window is one writer among the agents
  A person at Studio's window and the agents connected to the session edit
  the same documents. The window's Undo takes back the person's own edits,
  not an agent's that came after, and the history it lists is the
  person's. An object an agent holds says so in the layers panel, and the
  person's edit of it is refused until the agent lets go.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { color: "red" }
          Text { id: label; text: "hello" }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: The window's Undo takes back the person's edit, not an agent's later one
    Given the person has selected "Item root" in the layers panel
    And the person has set "width" to "320" in the inspector
    And an agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    When the person clicks the button "Undo"
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { color: "red" }
          Text { id: label; text: "bye" }
      }
      """

  Scenario: The window's history lists the person's edits, not an agent's
    When an agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    Then the history lists nothing

  Scenario: A layer an agent holds says who holds it
    When an agent named "Ada" calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"path": "root/0"}], "hold": true}
      """
    Then the layers panel shows "Rectangle held by Ada"

  Scenario: The person's edit of a layer an agent holds is refused
    Given an agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"path": "root/0"}], "hold": true}
      """
    And the person has selected "Rectangle" in the layers panel
    When the person sets "color" to "\"blue\"" in the inspector
    Then the window shows a refusal that says "held by Ada"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 200

          Rectangle { color: "red" }
          Text { id: label; text: "hello" }
      }
      """
