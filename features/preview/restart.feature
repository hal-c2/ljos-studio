# Sources:
#   docs/DESIGN.md (S6: mocked data)
#   docs/MCP.md (previews)

@preview
Feature: Another program in a browser's preview starts as a program does
  Switching a block between live and mocked runs other code, and what the old
  program's objects were given says nothing of it. That holds when the switch
  fails: the save that mends it is still the first of the new program. A
  native preview keeps the state over a switch, as `ljos run --watch` keeps it
  over a reload (docs/MCP.md); `preview_open` says which happened in
  `state_kept`.

  Background:
    Given the file "App/Block/qmldir":
      """
      module App.Block
      # block Block 1.0 Block.qml Data "Says where it is from"
      """
    And the file "App/Block/Block.qml":
      """qml
      import QtQuick

      Item { readonly property string from: "the live one" }
      """
    And the file "mocks/App/Block/qmldir":
      """
      module App.Block
      Block 1.0 Block.qml
      """
    And the file "Main.qml":
      """qml
      import QtQuick
      import App.Block

      Rectangle {
          id: root
          width: 300; height: 200
          property int count: 0

          Block { id: block }

          Text {
              objectName: "status"
              Accessible.role: Accessible.StaticText
              text: "Count " + root.count + " from " + block.from
          }

          Rectangle {
              id: card
              y: 50; width: 100; height: 100
              color: "red"

              MouseArea {
                  anchors.fill: parent
                  onClicked: root.count = root.count + 1
              }
          }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"

  # The mock names a file it does not have: a browser compiles the whole
  # project, so a mock that does not parse stops the live preview as well.
  # Both hosts: a mock that does not compile is told, and the program that
  # was running goes on, state and all.
  Scenario: A broken mock is told and the last good preview stays
    Given the file "mocks/App/Block/qmldir":
      """
      module App.Block
      Block 1.0 Missing.qml
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent has called the tool "preview_state" with:
      """json
      {"node": {"id": "root"}, "click": {"id": "card"}}
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "state": "mocked"}
      """
    Then the preview shows "Count 1 from the live one"
    When the agent calls the tool "preview_events" with:
      """json
      {"since": 1}
      """
    Then the answer's "/events/0/kind" is "reload_failed"

  @web
  Scenario: A switch that did not compile starts over when a save mends it
    Given the file "mocks/App/Block/qmldir":
      """
      module App.Block
      Block 1.0 Missing.qml
      """
    And the file "mocks/App/Block/Block.qml":
      """qml
      import QtQuick

      Item { readonly property string from: "the mock" }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    When the agent calls the tool "preview_state" with:
      """json
      {"node": {"id": "root"}, "click": {"id": "card"}}
      """
    Then the preview shows "Count 1 from the live one"
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "state": "mocked"}
      """
    And the file "mocks/App/Block/qmldir" is rewritten outside Studio:
      """
      module App.Block
      Block 1.0 Block.qml
      """
    Then the preview shows "Count 0 from the mock" within 20 seconds

  @web
  Scenario: A switch starts over
    Given the file "mocks/App/Block/Block.qml":
      """qml
      import QtQuick

      Item { readonly property string from: "the mock" }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    When the agent calls the tool "preview_state" with:
      """json
      {"node": {"id": "root"}, "click": {"id": "card"}}
      """
    Then the preview shows "Count 1 from the live one"
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "state": "mocked"}
      """
    Then the preview shows "Count 0 from the mock"
    And the answer's "/state_kept" is false

  # Where the host reloads in place, the count survives a switch.
  @native
  Scenario: A switch where the host can keeps the state and says so
    Given the file "mocks/App/Block/Block.qml":
      """qml
      import QtQuick

      Item { readonly property string from: "the mock" }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    When the agent calls the tool "preview_state" with:
      """json
      {"node": {"id": "root"}, "click": {"id": "card"}}
      """
    Then the preview shows "Count 1 from the live one"
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "state": "mocked"}
      """
    Then the preview shows "Count 1 from the mock"
    And the answer's "/state_kept" is true

  # Both hosts: what a real object lacks is not a missing object.
  Scenario: A state the mock leaves out is not a missing object
    Given the file "mocks/App/Block/Block.qml":
      """qml
      import QtQuick

      Item { property string other: "the mock" }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "state": "mocked"}
      """
    When the agent calls the tool "preview_state" with:
      """json
      {"node": {"id": "block"}, "set": {"from": "somewhere"}}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "refused"
