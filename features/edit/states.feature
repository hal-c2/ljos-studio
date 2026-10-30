# Sources:
#   docs/MCP.md (state_edit, node_inspect)
#   docs/DESIGN.md (S4: states and transitions as a component's variants;
#   breakpoints as width bindings)
#   https://doc.qt.io/qt-6/qml-qtquick-state.html
#   https://doc.qt.io/qt-6/qml-qtquick-propertychanges.html
#   https://doc.qt.io/qt-6/qml-qtquick-transition.html

Feature: An agent edits a component's states and transitions
  A component's variants are its `State`s: each has a name, the property
  changes it makes and, optionally, a `when` that turns it on. `state_edit`
  adds, renames and removes them, sets or removes a value one of them
  changes, sets its `when`, and adds or removes a `Transition`. Like every
  edit it keeps the rest of the file byte for byte, takes the generation it
  was made against, can be tried with `dry_run`, and goes into the history.
  `node_inspect` lists an object's states and transitions.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root

          Text {
              id: label
              text: "wide"
          }
      }
      """
    And the studio tools are connected

  Rule: States are added, renamed and removed

    Scenario: A state is added with the condition that turns it on
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add", "state": "compact", "when": "root.width < 600"}
        """
      Then the answer's "/generation" is 2
      And the file "Main.qml" is:
        """qml
        import QtQuick

        Item {
            id: root
            states: [
                State {
                    name: "compact"
                    when: root.width < 600
                }
            ]

            Text {
                id: label
                text: "wide"
            }
        }
        """

    Scenario: A second state joins the list after the first
      Given the agent has called the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add", "state": "compact"}
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add", "state": "hidden"}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick

        Item {
            id: root
            states: [
                State {
                    name: "compact"
                },
                State {
                    name: "hidden"
                }
            ]

            Text {
                id: label
                text: "wide"
            }
        }
        """

    Scenario: A state is not added over a list that is an expression
      Given the file "Bound.qml":
        """qml
        import QtQuick

        Item {
            states: someList
        }
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Bound.qml", "action": "add", "state": "compact"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's message matches "`states` is an expression; edit it as text"
      And the file "Bound.qml" is:
        """qml
        import QtQuick

        Item {
            states: someList
        }
        """

    Scenario: A transition is not added over a list that is an expression
      Given the file "Bound.qml":
        """qml
        import QtQuick

        Item {
            states: State { name: "compact" }
            transitions: [someTransition]
        }
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Bound.qml", "action": "add_transition", "to": "compact", "animations": "NumberAnimation { duration: 100 }"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's message matches "`transitions` is an expression; edit it as text"

    Scenario: A list with only a comment in it takes a state and keeps the comment
      Given the file "Empty.qml":
        """qml
        import QtQuick

        Item {
            states: [ /* none yet */ ]
        }
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Empty.qml", "action": "add", "state": "compact"}
        """
      Then the file "Empty.qml" is:
        """qml
        import QtQuick

        Item {
            states: [
                /* none yet */
                State {
                    name: "compact"
                }
            ]
        }
        """

    Scenario: Two states of one name are refused
      Given the agent has called the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add", "state": "compact"}
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add", "state": "compact"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "exists"

    Scenario: Renaming a state renames the transitions to and from it
      Given the file "Main.qml":
        """qml
        import QtQuick

        Item {
            id: root
            states: [
                State { name: "compact" },
                State { name: "hidden" }
            ]
            transitions: Transition {
                from: "*"; to: "compact"
                NumberAnimation { properties: "width" }
            }
        }
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "rename", "state": "compact", "new_name": "narrow"}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick

        Item {
            id: root
            states: [
                State { name: "narrow" },
                State { name: "hidden" }
            ]
            transitions: Transition {
                from: "*"; to: "narrow"
                NumberAnimation { properties: "width" }
            }
        }
        """

    Scenario: Removing a state takes its comma with it
      Given the file "Main.qml":
        """qml
        import QtQuick

        Item {
            id: root
            states: [
                State { name: "compact" },
                // Shown while loading.
                State { name: "hidden" }
            ]
        }
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "remove", "state": "compact"}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick

        Item {
            id: root
            states: [
                // Shown while loading.
                State { name: "hidden" }
            ]
        }
        """

    Scenario: Removing the last state removes the list
      Given the agent has called the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add", "state": "compact"}
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "remove", "state": "compact"}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick

        Item {
            id: root

            Text {
                id: label
                text: "wide"
            }
        }
        """

    Scenario: A state that is not there is not found
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "remove", "state": "compact"}
        """
      Then the agent is refused with "not_found"
      And the refusal's "/details/kind" is "no_state"

  Rule: A state's changes and its condition are set and removed

    Background:
      Given the agent has called the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add", "state": "compact"}
        """

    Scenario: A value the state changes
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "change", "state": "compact", "target": {"id": "label"}, "name": "text", "value": "narrow"}
        """
      And the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "change", "state": "compact", "target": {"id": "label"}, "name": "opacity", "binding": "root.width / 600"}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick

        Item {
            id: root
            states: [
                State {
                    name: "compact"
                    PropertyChanges {
                        target: label
                        text: "narrow"
                        opacity: root.width / 600
                    }
                }
            ]

            Text {
                id: label
                text: "wide"
            }
        }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "root"}}
        """
      Then the answer's "/states/0/name" is "compact"
      And the answer's "/states/0/changes" is:
        """json
        [
          {"target": "label", "name": "text", "value": "\"narrow\""},
          {"target": "label", "name": "opacity", "value": "root.width / 600"}
        ]
        """

    Scenario: Removing a state's last change for an object removes its PropertyChanges
      Given the agent has called the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "change", "state": "compact", "target": {"id": "label"}, "name": "text", "value": "narrow"}
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "change", "state": "compact", "target": {"id": "label"}, "name": "text"}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick

        Item {
            id: root
            states: [
                State {
                    name: "compact"
                }
            ]

            Text {
                id: label
                text: "wide"
            }
        }
        """

    Scenario: A change to an object without an id is refused
      Given the agent has called the tool "node_insert" with:
        """json
        {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Text {}", "id": false}
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "change", "state": "compact", "target": {"path": "root/1"}, "name": "text", "value": "narrow"}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "no_id"

    Scenario: The condition is set and removed
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "when", "state": "compact", "when": "root.width < 600"}
        """
      And the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "root"}}
        """
      Then the answer's "/states/0/when" is "root.width < 600"
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "when", "state": "compact"}
        """
      And the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "root"}}
        """
      Then the answer's "/states/0/when" is absent

    Scenario: A condition that would not parse is refused
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "when", "state": "compact", "when": "root.width <"}
        """
      Then the agent is refused with "would_break"

  Rule: Transitions are added and removed

    Scenario: A transition into a state, with its animations
      Given the agent has called the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add", "state": "compact"}
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add_transition", "to": "compact", "animations": "NumberAnimation { properties: \"opacity\"; duration: 200 }"}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick

        Item {
            id: root
            states: [
                State {
                    name: "compact"
                }
            ]
            transitions: [
                Transition {
                    from: "*"
                    to: "compact"
                    NumberAnimation { properties: "opacity"; duration: 200 }
                }
            ]

            Text {
                id: label
                text: "wide"
            }
        }
        """
      When the agent calls the tool "node_inspect" with:
        """json
        {"doc": "Main.qml", "node": {"id": "root"}}
        """
      Then the answer's "/transitions/0/to" is "compact"
      And the answer's "/transitions/0/animations" is "NumberAnimation { properties: \"opacity\"; duration: 200 }"
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "remove_transition", "to": "compact"}
        """
      Then the file "Main.qml" is:
        """qml
        import QtQuick

        Item {
            id: root
            states: [
                State {
                    name: "compact"
                }
            ]

            Text {
                id: label
                text: "wide"
            }
        }
        """

    Scenario: Animations that would not parse are refused
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add_transition", "animations": "NumberAnimation { duration: }"}
        """
      Then the agent is refused with "would_break"

  Rule: State edits are edits like any other

    Scenario: A dry run says what it would write and writes nothing
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add", "state": "compact", "dry_run": true}
        """
      Then the answer's "/history" is absent
      And the answer's "/generation" is 1
      And the file "Main.qml" is:
        """qml
        import QtQuick

        Item {
            id: root

            Text {
                id: label
                text: "wide"
            }
        }
        """

    Scenario: A stale generation is refused
      Given the agent has called the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add", "state": "compact"}
        """
      When the agent calls the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "generation": 1, "action": "add", "state": "hidden"}
        """
      Then the agent is refused with "stale_ref"

    Scenario: Undo gives back the bytes
      Given the agent has called the tool "state_edit" with:
        """json
        {"doc": "Main.qml", "action": "add", "state": "compact"}
        """
      When the agent calls the tool "history" with:
        """json
        {"action": "undo"}
        """
      Then the answer's "/entry/label" is "add state compact"
      And the file "Main.qml" is:
        """qml
        import QtQuick

        Item {
            id: root

            Text {
                id: label
                text: "wide"
            }
        }
        """
