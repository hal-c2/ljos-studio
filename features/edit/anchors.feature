# Sources:
#   docs/MCP.md (node_arrange `anchor`)
#   docs/DESIGN.md (the lossless edit seam)

Feature: An object is pinned to its parent or a sibling
  Anchoring writes `anchors.*` and takes away the x, y, width and height that
  would now disagree with them, as one undo step. In a layout the layout says
  where things go, so anchoring there is refused with what to use instead.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300

          Rectangle { id: box; x: 10; y: 20; width: 50; height: 40 } // keep this note
          Rectangle { id: other; x: 100; y: 100; width: 50; height: 40 }
          Row {
              id: line
              Rectangle { id: inside; width: 10; height: 10 }
          }
      }
      """
    And the studio tools are connected

  Scenario: Fill takes away the box
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "fill": true}
      """
    Then the file "Main.qml" contains "Rectangle { id: box; anchors.fill: parent } // keep this note"

  Scenario: Centre takes away the place and keeps the size
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "centerIn": true}
      """
    Then the file "Main.qml" contains "Rectangle { id: box; width: 50; height: 40; anchors.centerIn: parent } // keep this note"

  Scenario: One side takes the place on its axis and writes its margin
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "sides": {"left": 12}}
      """
    Then the file "Main.qml" contains "Rectangle { id: box; y: 20; width: 50; height: 40; anchors.left: parent.left; anchors.leftMargin: 12 }"

  Scenario: Both sides of an axis take the size as well
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "sides": {"left": 0, "right": 0}}
      """
    Then the file "Main.qml" contains "Rectangle { id: box; y: 20; height: 40; anchors.left: parent.left; anchors.right: parent.right }"

  Scenario: A sibling can be the target
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "sides": {"top": 4}, "target": "other"}
      """
    Then the file "Main.qml" contains "anchors.top: other.top; anchors.topMargin: 4"

  Scenario: Null takes a pin off with its margin, and undo brings it back as one step
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "sides": {"left": 12}}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "sides": {"left": null}}
      """
    Then the file "Main.qml" contains "Rectangle { id: box; y: 20; width: 50; height: 40 }"
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Main.qml" contains "anchors.left: parent.left; anchors.leftMargin: 12"

  Scenario: False takes away a fill
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "fill": true}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "fill": false}
      """
    Then the file "Main.qml" does not contain "anchors"

  Scenario: A new pin replaces the fill
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "fill": true}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "sides": {"left": 3}}
      """
    Then the file "Main.qml" contains "anchors.left: parent.left; anchors.leftMargin: 3"
    And the file "Main.qml" does not contain "anchors.fill"

  Scenario: The group form is edited in its place
    Given the file "Grouped.qml":
      """qml
      import QtQuick

      Rectangle {
          Rectangle {
              id: g
              anchors { left: parent.left; top: parent.top }
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Grouped.qml", "nodes": [{"id": "g"}], "sides": {"left": null}}
      """
    Then the file "Grouped.qml" contains "top: parent.top"
    And the file "Grouped.qml" does not contain "left: parent.left"

  Scenario: In a layout the layout decides, and the refusal says what to use
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "inside"}], "fill": true}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "arranged"
    And the refusal's message matches "Row"
    And the file "Main.qml" does not contain "anchors"

  Scenario Outline: What cannot be pinned is refused
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "<node>"}], "sides": {"left": 0}, "target": "<target>"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "<kind>"

    Examples:
      | node | target | kind          |
      | root | parent | root          |
      | box  | inside | not_a_sibling |
      | box  | box    | not_a_sibling |

  Scenario: Fill and centre are not asked for together
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "fill": true, "centerIn": true}
      """
    Then the agent is refused with "invalid_argument"

  @preview
  Scenario: Letting go keeps it where the preview shows it
    Given the file "Shown.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300
          Rectangle { id: full; anchors.fill: parent }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Shown.qml", "size": {"width": 400, "height": 300}}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Shown.qml", "nodes": [{"id": "full"}], "fill": false}
      """
    Then the file "Shown.qml" contains "x: 0"
    And the file "Shown.qml" contains "width: 400"
    And the file "Shown.qml" contains "height: 300"
