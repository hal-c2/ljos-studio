# Sources:
#   docs/DESIGN.md (S4: breakpoints as width bindings)
#   docs/MCP.md (state_edit's `breakpoint`; preview targets)
#   https://doc.qt.io/qt-6/qml-qtquick-state.html#when-prop

Feature: A breakpoint is a state the root's width turns on
  A layout for phones is not a second document: it is a `State` of the
  shell whose `when` compares the root's width with a bound between the
  preview targets. Phones are narrower than 600, tablets from 600 up to
  1024, desktops from 1024. `state_edit` writes that `when` for a
  `breakpoint`, so a person sees the condition in the file and may change
  it, and a preview on that target shows the state.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root

          Text {
              id: label
              objectName: "label"
              Accessible.role: Accessible.StaticText
              text: "wide"
          }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"

  Scenario Outline: A breakpoint for the <target> writes its width bound
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "action": "add", "state": "<target>", "breakpoint": "<target>"}
      """
    And the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"id": "root"}}
      """
    Then the answer's "/states/0/when" is "<when>"

    Examples:
      | target  | when                                   |
      | phone   | root.width < 600                       |
      | tablet  | root.width >= 600 && root.width < 1024 |
      | desktop | root.width >= 1024                     |

  Scenario: A breakpoint is set on a state that has one
    Given the agent has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "action": "add", "state": "compact", "when": "root.width < 500"}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "action": "when", "state": "compact", "breakpoint": "phone"}
      """
    Then the file "Main.qml" is:
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
              objectName: "label"
              Accessible.role: Accessible.StaticText
              text: "wide"
          }
      }
      """

  Scenario: A condition and a breakpoint together are refused
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "action": "add", "state": "compact", "when": "true", "breakpoint": "phone"}
      """
    Then the agent is refused with "invalid_argument"

  Scenario: A breakpoint needs the root to have an id
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "action": "add", "state": "compact", "breakpoint": "phone"}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's "/details/kind" is "no_id"

  @preview
  Scenario: The preview on a phone shows the phone's state
    Given the agent has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "action": "add", "state": "compact", "breakpoint": "phone"}
      """
    And the agent has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "action": "change", "state": "compact", "target": {"id": "label"}, "name": "text", "value": "narrow"}
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "target": "phone"}
      """
    Then the preview shows "narrow"
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "target": "desktop"}
      """
    Then the preview shows "wide"
