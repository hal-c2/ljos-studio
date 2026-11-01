# Sources:
#   docs/MCP.md (state_edit's interactions, node_inspect)
#   docs/DESIGN.md (S6: prototyping flows are states, transitions and
#   signal handlers)
#   https://doc.qt.io/qt-6/qml-qtquick-taphandler.html
#   https://doc.qt.io/qt-6/qml-qtquick-loader.html (source)
#   https://doc.qt.io/qt-6/qml-qtquick-controls-stackview.html (push)
#   https://doc.qt.io/qt-6/qtqml-syntax-objectattributes.html (signal
#   attributes, signal handlers, property change signals)

Feature: An agent wires a prototype's flow
  A flow is what a tap does: it goes to one of a component's states or
  opens another document. Studio writes it as QML, a signal handler on the
  object that is tapped: the signal's own handler when the agent names
  the signal (`onClicked` of a button), else a `TapHandler`, which any
  item can hold. `state_edit` writes one (`interaction`) and takes one out
  (`remove_interaction`), checking what it names: the state, the
  document, the object that opens it, the signal. `node_inspect` lists the
  flows of an object and everything in it, so the root's lists the
  document's whole flow beside its states and transitions, and the signals
  the object has with their arguments: what it declares, then what its type
  declares and inherits, as Ljós's table of types, the project's QML files
  and its type dumps (ljostypes.json, a type written in Rust) describe it.

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

  Rule: A flow handles a signal the object has

    Scenario: What an object's type takes is listed, written or not
      Given the file "Chip.qml":
        """qml
        import QtQuick

        Item { property int count: 0; property string label }
        """
      And the file "Main.qml":
        """qml
        import QtQuick

        Item { Chip { id: chip; count: 2 } }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "chip"}}
        """
      Then the answer's "/takes" is ["count", "label"]

    Scenario: An object's signals are listed with their arguments
      Given the agent has called the tool "node_insert" with:
        """json
        {"doc": "Main.qml", "parent": {"id": "root"}, "text": "MouseArea { id: area }", "id": false}
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "area"}}
        """
      Then the answer's "/signals" has the entry:
        """json
        {"name": "clicked", "arguments": [{"name": "mouse", "type": "QtObject"}], "change": false}
        """
      And the answer's "/signals" has the entry:
        """json
        {"name": "widthChanged", "arguments": [], "change": true}
        """

    Scenario: A signal the trigger does not have is refused, naming those it has
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "back"}, "signal": "tapped", "state": "details"}
        """
      Then the agent is refused with "not_found"
      And the refusal's "/details/kind" is "no_signal"
      And the refusal's message matches "has no signal `tapped`; it has .*\bclicked\b"

    Scenario: A property's change signal is one a flow may handle
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "card"}, "signal": "colorChanged", "state": "details"}
        """
      Then the answer's "/generation" is 2

    Scenario: A component of the project has what its file declares
      Given the file "Card.qml":
        """qml
        import QtQuick

        Rectangle {
            property string title
            signal chosen(int index)
        }
        """
      And the agent has called the tool "node_insert" with:
        """json
        {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Card { id: picked }", "id": false}
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "picked"}}
        """
      Then the answer's "/signals/0" is {"name": "chosen", "arguments": [{"name": "index", "type": "int"}], "change": false}
      And the answer's "/signals/1" is {"name": "titleChanged", "arguments": [], "change": true}
      And the answer's "/signals" has the entry:
        """json
        {"name": "colorChanged", "arguments": [], "change": true}
        """

    Scenario: A type written in Rust has the signals its dump lists
      Given the file "native/ljostypes.json":
        """json
        {"format": 2, "modules": [{"uri": "App.Media", "versions": ["1.0"], "types": [
          {"name": "Player", "since": "1.0", "base": "Item", "baseModule": "QtQuick",
           "creatable": true, "defaultProperty": null, "attached": null,
           "properties": [{"name": "source", "type": "url", "readonly": false, "final": true,
             "required": false, "constant": false, "group": false, "notify": "sourceChanged"}],
           "signals": [{"name": "finished", "parameters": [{"name": "reason", "type": "string"}]}],
           "methods": []}]}]}
        """
      And the file "Player.qml":
        """qml
        import QtQuick
        import App.Media

        Item {
            id: root
            states: [
                State {
                    name: "done"
                }
            ]

            Player {
                id: player
            }
        }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Player.qml", "node": {"id": "player"}}
        """
      Then the answer's "/signals/0" is {"name": "finished", "arguments": [{"name": "reason", "type": "string"}], "change": false}
      And the answer's "/signals/1" is {"name": "sourceChanged", "arguments": [], "change": true}
      And the answer's "/signals" has the entry:
        """json
        {"name": "visibleChanged", "arguments": [], "change": true}
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Player.qml", "action": "interaction", "trigger": {"id": "player"}, "signal": "finished", "state": "done"}
        """
      Then the file "Player.qml" is:
        """qml
        import QtQuick
        import App.Media

        Item {
            id: root
            states: [
                State {
                    name: "done"
                }
            ]

            Player {
                id: player
                onFinished: root.state = "done"
            }
        }
        """

    # A type no import, project file, dump or table of Ljós's describes may
    # be one Studio does not see (a module of the app's, built elsewhere):
    # its signals are not listed, and a handler of one is written as asked
    # for Ljós to check when it compiles.
    Scenario: A type described nowhere lists no signals and refuses none
      Given the file "Other.qml":
        """qml
        import QtQuick
        import Somewhere.Else

        Item {
            id: root
            states: [
                State {
                    name: "on"
                }
            ]

            Gadget {
                id: gadget
            }
        }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Other.qml", "node": {"id": "gadget"}}
        """
      Then the answer's "/signals" is absent
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Other.qml", "action": "interaction", "trigger": {"id": "gadget"}, "signal": "fired", "state": "on"}
        """
      Then the answer's "/generation" is 2

    # A dump without `baseModule` names its base in the dump's own module,
    # where nothing describes `Item`: what Player inherits is not known, so
    # Player is a type described only in part, and refuses nothing.
    Scenario: A dumped type whose base is described nowhere lists no signals and refuses none
      Given the file "native/ljostypes.json":
        """json
        {"format": 2, "modules": [{"uri": "App.Media", "versions": ["1.0"], "types": [
          {"name": "Player", "since": "1.0", "base": "Item",
           "creatable": true, "defaultProperty": null, "attached": null,
           "properties": [], "signals": [{"name": "finished", "parameters": []}],
           "methods": []}]}]}
        """
      And the file "Media.qml":
        """qml
        import QtQuick
        import App.Media

        Item {
            id: root
            states: [
                State {
                    name: "wide"
                }
            ]

            Player {
                id: player
            }
        }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Media.qml", "node": {"id": "player"}}
        """
      Then the answer's "/signals" is absent
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Media.qml", "action": "interaction", "trigger": {"id": "player"}, "signal": "widthChanged", "state": "wide"}
        """
      Then the file "Media.qml" is:
        """qml
        import QtQuick
        import App.Media

        Item {
            id: root
            states: [
                State {
                    name: "wide"
                }
            ]

            Player {
                id: player
                onWidthChanged: root.state = "wide"
            }
        }
        """

    # The dump is the type `import App.Media` names, so a same-named file
    # beside the document does not fill in what it inherits.
    Scenario: A dumped type described in part is not filled in by another of its name
      Given the file "native/ljostypes.json":
        """json
        {"format": 2, "modules": [{"uri": "App.Media", "versions": ["1.0"], "types": [
          {"name": "Player", "since": "1.0", "base": "Mystery",
           "creatable": true, "defaultProperty": null, "attached": null,
           "properties": [], "signals": [{"name": "finished", "parameters": []}],
           "methods": []}]}]}
        """
      And the file "Player.qml":
        """qml
        import QtQuick

        Item {
        }
        """
      And the file "Media.qml":
        """qml
        import QtQuick
        import App.Media

        Item {
            Player {
                id: player
            }
        }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Media.qml", "node": {"id": "player"}}
        """
      Then the answer's "/signals" is absent

    # The same for a file: the first import that has the type is the type,
    # however little is known of its root.
    Scenario: A file whose root is described nowhere is not filled in by a later import
      Given the file "native/ljostypes.json":
        """json
        {"format": 2, "modules": [{"uri": "App.Media", "versions": ["1.0"], "types": [
          {"name": "Player", "since": "1.0", "base": "Item", "baseModule": "QtQuick",
           "creatable": true, "defaultProperty": null, "attached": null,
           "properties": [], "signals": [{"name": "finished", "parameters": []}],
           "methods": []}]}]}
        """
      And the file "parts/Player.qml":
        """qml
        Mystery {
        }
        """
      And the file "Media.qml":
        """qml
        import QtQuick
        import "parts"
        import App.Media

        Item {
            Player {
                id: player
            }
        }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Media.qml", "node": {"id": "player"}}
        """
      Then the answer's "/signals" is absent

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
