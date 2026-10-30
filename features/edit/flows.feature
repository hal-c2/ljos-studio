# Sources:
#   docs/MCP.md (state_edit's interactions, node_inspect)
#   docs/DESIGN.md (S6: prototyping flows are states, transitions and
#   signal handlers)
#   https://doc.qt.io/qt-6/qml-qtquick-taphandler.html
#   https://doc.qt.io/qt-6/qml-qtquick-loader.html (source)
#   https://doc.qt.io/qt-6/qml-qtquick-controls-stackview.html (push)

Feature: An agent wires a prototype's flow
  A flow is what a tap does: it goes to one of a component's states or
  opens another document. Studio writes it as QML, a signal handler on the
  object that is tapped: the signal's own handler when the agent names
  the signal (`onClicked` of a button), else a `TapHandler`, which any
  item can hold. `state_edit` writes one (`interaction`) and takes one out
  (`remove_interaction`), checking what it names: the state, the
  document, the object that opens it. `node_inspect` lists the flows of an
  object and everything in it, so the root's lists the document's whole
  flow beside its states and transitions.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      import QtQuick.Controls

      Item {
          id: root
          states: [
              State {
                  name: "details"
              }
          ]

          Rectangle {
              id: card
          }

          Button {
              id: back
          }
      }
      """
    And the file "Details.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the studio tools are connected

  Rule: A tap goes to a state

    Scenario: A tap on an item goes to a state through a TapHandler
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "card"}, "state": "details"}
        """
      Then the answer's "/generation" is 2
      And the file "Main.qml" is:
        """qml
        import QtQuick
        import QtQuick.Controls

        Item {
            id: root
            states: [
                State {
                    name: "details"
                }
            ]

            Rectangle {
                id: card
                TapHandler {
                    onTapped: root.state = "details"
                }
            }

            Button {
                id: back
            }
        }
        """

    Scenario: A named signal is handled on the object itself
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "back"}, "signal": "clicked", "state": ""}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick
        import QtQuick.Controls

        Item {
            id: root
            states: [
                State {
                    name: "details"
                }
            ]

            Rectangle {
                id: card
            }

            Button {
                id: back
                onClicked: root.state = ""
            }
        }
        """

    Scenario: A state that is not there is not found
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "card"}, "state": "summary"}
        """
      Then the agent is refused with "not_found"
      And the refusal's "/details/kind" is "no_state"

    Scenario: A trigger that handles the signal already is refused
      Given the agent has called the tool "binding_set" with:
        """json
        {"doc": "Main.qml", "node": {"id": "back"}, "name": "onClicked", "value": "console.log(\"back\")"}
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "back"}, "signal": "clicked", "state": "details"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "exists"

    Scenario: A signal that is not a name is refused
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "back"}, "signal": "clicked()", "state": "details"}
        """
      Then the agent is refused with "invalid_argument"

  Rule: A tap opens a document

    Scenario: A Loader shows the document
      Given the agent has called the tool "node_insert" with:
        """json
        {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Loader { id: page }", "id": false}
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "card"}, "open": "Details.qml", "navigator": {"id": "page"}}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick
        import QtQuick.Controls

        Item {
            id: root
            states: [
                State {
                    name: "details"
                }
            ]

            Rectangle {
                id: card
                TapHandler {
                    onTapped: page.source = "Details.qml"
                }
            }

            Button {
                id: back
            }

            Loader { id: page }
        }
        """

    Scenario: A StackView pushes the document
      Given the agent has called the tool "node_insert" with:
        """json
        {"doc": "Main.qml", "parent": {"id": "root"}, "text": "StackView { id: stack }", "id": false}
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "back"}, "signal": "clicked", "open": "Details.qml", "navigator": {"id": "stack"}}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick
        import QtQuick.Controls

        Item {
            id: root
            states: [
                State {
                    name: "details"
                }
            ]

            Rectangle {
                id: card
            }

            Button {
                id: back
                onClicked: stack.push(Qt.resolvedUrl("Details.qml"))
            }

            StackView { id: stack }
        }
        """

    Scenario: A document that is not there is not found
      Given the agent has called the tool "node_insert" with:
        """json
        {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Loader { id: page }", "id": false}
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "card"}, "open": "Missing.qml", "navigator": {"id": "page"}}
        """
      Then the agent is refused with "not_found"
      And the refusal's "/details/kind" is "no_document"

    Scenario: Only a Loader or a StackView opens a document
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "card"}, "open": "Details.qml", "navigator": {"id": "back"}}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "not_a_navigator"

    Scenario: A state and a document together are refused
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "card"}, "state": "details", "open": "Details.qml"}
        """
      Then the agent is refused with "invalid_argument"

  Rule: The flow is listed and taken apart

    Background:
      Given the agent has called the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "card"}, "state": "details"}
        """
      And the agent has called the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "back"}, "signal": "clicked", "state": ""}
        """

    Scenario: The root lists the document's flow
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "root"}}
        """
      Then the answer's "/interactions" has 2 entries
      And the answer's "/interactions/0/trigger_id" is "card"
      And the answer's "/interactions/0/signal" is "tapped"
      And the answer's "/interactions/0/owner" is "root"
      And the answer's "/interactions/0/state" is "details"
      And the answer's "/interactions/1/trigger_id" is "back"
      And the answer's "/interactions/1/signal" is "clicked"
      And the answer's "/interactions/1/state" is ""

    Scenario: An object lists only the flows in it
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "back"}}
        """
      Then the answer's "/interactions" has 1 entry
      And the answer's "/interactions/0/state" is ""

    Scenario: A tap's TapHandler is taken out with it
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "remove_interaction", "trigger": {"id": "card"}}
        """
      And the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "remove_interaction", "trigger": {"id": "back"}, "signal": "clicked"}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick
        import QtQuick.Controls

        Item {
            id: root
            states: [
                State {
                    name: "details"
                }
            ]

            Rectangle {
                id: card
            }

            Button {
                id: back
            }
        }
        """

    Scenario: A handler Studio did not write is no flow to take out
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "remove_interaction", "trigger": {"id": "root"}}
        """
      Then the agent is refused with "not_found"
      And the refusal's "/details/kind" is "no_interaction"

    Scenario: Undo gives back the bytes
      When the agent calls the tool "history" with:
        """json
        {"action": "undo"}
        """
      And the agent calls the tool "history" with:
        """json
        {"action": "undo"}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick
        import QtQuick.Controls

        Item {
            id: root
            states: [
                State {
                    name: "details"
                }
            ]

            Rectangle {
                id: card
            }

            Button {
                id: back
            }
        }
        """
      And the answer's "/entry/label" is "tapped card to details"
