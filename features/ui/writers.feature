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

  Scenario: A binding refused for a layer an agent holds is not left in its row
    Given an agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"path": "root/0"}], "hold": true}
      """
    And the person has selected "Rectangle" in the layers panel
    When the person binds "color" to "root.color" in the inspector
    Then the window shows a refusal that says "held by Ada"
    And the inspector's "color" holds "\"red\""

  Scenario: A state's change refused for a layer an agent holds is not left in its row
    Given the person has selected "Item root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    And an agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "root"}], "hold": true}
      """
    And the person has selected "Text label" in the layers panel
    When the person sets "text" to "\"bye\"" in the inspector
    Then the window shows a refusal that says "held by Ada"
    And the inspector's "text" holds "\"hello\""

  Scenario: A state's fields keep what the file says when an agent's hold refuses them
    Given the person has selected "Item root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    And the person has selected "Text label" in the layers panel
    And the person has set "text" to "\"bye\"" in the inspector
    And an agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "root"}], "hold": true}
      """
    When the person sets "label.text" to "\"no\"" in the inspector
    Then the window shows a refusal that says "held by Ada"
    And the inspector's "label.text" holds "\"bye\""
    When the person sets "When" to "root.width > 100" in the inspector
    Then the inspector's "When" holds ""
    When the person sets "Name of compact" to "narrow" in the inspector
    Then the inspector's "Name of compact" holds "compact"
    And the inspector's "label.text" holds "\"bye\""

  @preview
  Scenario: A refused state field keeps its reason with a preview open
    Given the person has selected "Item root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    And an agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "root"}], "hold": true}
      """
    Then the frame "Desktop" shows the state "compact"
    When the person sets "Name of compact" to "narrow" in the inspector
    Then the inspector's "Name of compact" holds "compact"
    And the window shows a refusal that says "held by Ada"
    When the person sets "When" to "root.width > 100" in the inspector
    Then the inspector's "When" holds ""
    And the window shows a refusal that says "held by Ada"

  @preview
  Scenario: A picked state the running app cannot show says why
    When the person replaces "import QtQuick" with "import QtQuick\nimport NoSuchModule" in the code view
    And the person replaces "id: root" with "id: top" in the code view
    Then the window shows no refusal
    Given the person has selected "Item top" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    Then the window shows a refusal that says "top"

  @preview
  Scenario: A state that can be shown again takes the old refusal away
    When the person replaces "import QtQuick" with "import QtQuick\nimport NoSuchModule" in the code view
    And the person replaces "id: root" with "id: top" in the code view
    Then the window shows no refusal
    Given the person has selected "Item top" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has added the state "wide" in the inspector
    And the person has picked the state "compact" in the inspector
    Then the window shows a refusal that says "top"
    Given an agent named "Ada" has called the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": 5, "start": 15, "end": 35, "text": ""}
      """
    When the person picks the state "wide" in the inspector
    Then the window shows no refusal

  @preview
  Scenario: A refused follow-on says why when the edit before it emptied the line
    When the person replaces "import QtQuick" with "import QtQuick\nimport NoSuchModule" in the code view
    And the person replaces "id: root" with "id: top" in the code view
    Then the window shows no refusal
    Given the person has selected "Item top" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    Then the window shows a refusal that says "top"
    When the person sets "Name of compact" to "narrow" in the inspector
    Then the inspector's "Name of narrow" holds "narrow"
    And the window shows a refusal that says "top"

  @preview
  Scenario: A picked state's refusal replaces an older one
    When the person replaces "import QtQuick" with "import QtQuick\nimport NoSuchModule" in the code view
    And the person replaces "id: root" with "id: top" in the code view
    Then the window shows no refusal
    Given the person has selected "Item top" in the layers panel
    And the person has added the state "compact" in the inspector
    And an agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "top"}], "hold": true}
      """
    When the person sets "width" to "300" in the inspector
    Then the window shows a refusal that says "held by Ada"
    When the person picks the state "compact" in the inspector
    Then the window shows a refusal that says "top"

  Scenario: The person's edit of a layer an agent holds is refused
    Given an agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"path": "root/0"}], "hold": true}
      """
    And the person has selected "Rectangle" in the layers panel
    When the person sets "color" to "\"blue\"" in the inspector
    Then the window shows a refusal that says "held by Ada"
    And the inspector's "color" holds "\"red\""
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
