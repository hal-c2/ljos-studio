# Sources:
#   docs/DESIGN.md (S6: prototyping flows; CMS-like content is models)
#   features/edit/flows.feature, features/edit/models.feature (the same
#   edits over MCP; the file text is the oracle for both)
#   features/preview/flows.feature (the preview plays the flow)

@ui
Feature: A person wires and plays a prototype's flow in Studio's window
  The flow panel draws the open document's flow as a graph: the root's
  states are its nodes, the base state first, and its transitions and
  interactions are its edges. A person wires the selected object to go to
  a state when tapped, and takes a wire out again, each the `state_edit`
  call an agent makes. In play mode a click in a frame is a click on the
  running app (`preview_state` with `click_at`), and the panel says which
  state the app is in. The inspector lists the roles of the data the
  selected object sees (a JSON file's keys, when a `JsonListModel` reads
  it), and binds one to a property as `model.<role>`,
  and the signals it has; the flow panel wires one of them, or a tap, as
  `node_inspect` lists them for an agent.

  Background:
    Given the file "Contacts.qml":
      """qml
      import QtQuick

      ListModel {
          ListElement {
              name: "Ada"
              email: "ada@example.com"
          }
      }
      """
    And the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          states: [
              State {
                  name: "details"
                  PropertyChanges {
                      target: title
                      text: "Details"
                  }
              }
          ]

          Text {
              id: title
              text: "Overview"
          }

          Rectangle {
              id: card
              y: 100
              width: 200
              height: 100
          }

          Repeater {
              id: people
              model: Contacts {}
              Text {
                  id: row
              }
          }
      }
      """
    And the file "Pages.qml":
      """qml
      import QtQuick

      Rectangle {
          width: 400
          height: 800

          Text {
              id: label
              text: "One"
          }

          MouseArea {
              anchors.fill: parent
              onClicked: label.text = "Two"
          }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: The flow panel draws the states and the flows between them
    Given an agent has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "card"}, "state": "details"}
      """
    And an agent has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "action": "add_transition", "from": "*", "to": "details", "animations": "NumberAnimation { duration: 200 }"}
      """
    Then the flow panel lists the states "Base, details"
    And the flow panel lists the flows "* to details, tapped card to details"

  Scenario: The selection is wired to go to a state
    Given the person has selected "Rectangle card" in the layers panel
    When the person clicks the button "Tap selection to go to details"
    Then the flow panel lists the flows "tapped card to details"
    And the history lists "tapped card to details"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Rectangle {
          id: root
          states: [
              State {
                  name: "details"
                  PropertyChanges {
                      target: title
                      text: "Details"
                  }
              }
          ]

          Text {
              id: title
              text: "Overview"
          }

          Rectangle {
              id: card
              y: 100
              width: 200
              height: 100
              TapHandler {
                  onTapped: root.state = "details"
              }
          }

          Repeater {
              id: people
              model: Contacts {}
              Text {
                  id: row
              }
          }
      }
      """

  Scenario: The selection's own signal is wired to go to a state
    Given an agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "card"}, "text": "MouseArea { id: area }", "id": false}
      """
    And the person has selected "MouseArea area" in the layers panel
    Then the inspector lists the signals "pressAndHold, pressed, released, clicked, doubleClicked, positionChanged, entered, exited, canceled, wheel"
    When the person picks the signal "clicked" to wire in the flow panel
    And the person clicks the button "On clicked of selection go to details"
    Then the flow panel lists the flows "clicked area to details"
    And the history lists "clicked area to details"

  Scenario: The flow panel still answers when the selection's signals change with its list open
    Given the person has selected "Rectangle card" in the layers panel
    And the person has opened the list of what to wire in the flow panel
    When an agent calls the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": 1, "start": 316, "end": 316, "text": "        signal flipped()\n"}
      """
    Then the inspector lists the signals "flipped"
    When the person picks the signal "flipped" to wire in the flow panel
    And the person clicks the button "On flipped of selection go to details"
    Then the flow panel lists the flows "flipped card to details"
    And the history lists "flipped card to details"

  Scenario: A wire is taken out
    Given the person has selected "Rectangle card" in the layers panel
    And the person has clicked the button "Tap selection to go to details"
    When the person clicks the button "Remove flow tapped card to details"
    Then the flow panel lists the flows ""
    And the history lists "remove tapped interaction"

  @preview
  Scenario: In play mode a click in the frame plays the flow
    Given the person has selected "Rectangle card" in the layers panel
    And the person has clicked the button "Tap selection to go to details"
    And the person has clicked the button "Play"
    When the person clicks the frame "Desktop" at 50, 150
    Then the flow panel says the app is in the state "details"

  # The badge has no `id` and is drawn over the card's right end, reaching
  # past it: the click is at its point, not at the middle of the badge.
  @preview
  Scenario: In play mode a click plays the flow at the very point clicked
    Given the person has selected "Rectangle card" in the layers panel
    And the person has clicked the button "Tap selection to go to details"
    And an agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "card"}, "id": false, "text": "Rectangle { x: 150; width: 250; height: 20 }"}
      """
    And the person has clicked the button "Play"
    When the person clicks the frame "Desktop" at 160, 110
    Then the flow panel says the app is in the state "details"

  @preview
  Scenario: In play mode a click plays a document whose root has no id
    Given the person has opened "Pages.qml" in the documents list
    And the person has clicked the button "Play"
    When the person clicks the frame "Desktop" at 50, 150
    Then the window shows no refusal

  Scenario: The inspector lists the roles a delegate sees
    When the person selects "Text row" in the layers panel
    Then the inspector lists the roles "name, email"

  Scenario: The inspector lists the keys of a JSON list's records as roles
    Given the file "data/staff.json":
      """json
      [{"name": "Ada", "room": "4"}, {"name": "Grace", "phone": "12"}]
      """
    And an agent has called the tool "document_new" with:
      """json
      {"doc": "Staff.qml", "data": "data/staff.json"}
      """
    And an agent has called the tool "document_new" with:
      """json
      {"doc": "Directory.qml", "text": "import QtQuick\n\nColumn {\n    Repeater {\n        model: Staff {}\n        Text {\n            id: member\n        }\n    }\n}\n"}
      """
    And the person has opened "Directory.qml" in the documents list
    When the person selects "Text member" in the layers panel
    Then the inspector lists the roles "name, room, phone"

  Scenario: A role is bound to a property
    Given the person has selected "Text row" in the layers panel
    When the person binds the role "name" to "text" in the inspector
    Then the inspector shows "text" as "model.name"
    And the history lists "set text"
