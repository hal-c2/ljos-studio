# Sources:
#   docs/MCP.md (node_arrange `anchor`)
#   docs/DESIGN.md (the lossless edit seam)
#   studio7 dogfood backlog P7-16 (a pin wrote `topMargin: 0` over the
#   `anchors.margins` already there; there was no vertical or horizontal
#   centre pin)

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

  Scenario: A vertical centre takes the place on its axis and keeps the size
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "sides": {"verticalCenter": 0}}
      """
    Then the file "Main.qml" contains "Rectangle { id: box; x: 10; width: 50; height: 40; anchors.verticalCenter: parent.verticalCenter } // keep this note"

  Scenario: A centre's value is its offset
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "sides": {"horizontalCenter": 8}}
      """
    Then the file "Main.qml" contains "Rectangle { id: box; y: 20; width: 50; height: 40; anchors.horizontalCenter: parent.horizontalCenter; anchors.horizontalCenterOffset: 8 }"

  Scenario: A centre takes the place of the sides of its axis
    Given the file "Pinned.qml":
      """qml
      import QtQuick

      Rectangle {
          Rectangle { id: p; x: 3; width: 10; height: 10; anchors.top: parent.top; anchors.topMargin: 4 }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Pinned.qml", "nodes": [{"id": "p"}], "sides": {"verticalCenter": 0}}
      """
    Then the file "Pinned.qml" contains "Rectangle { id: p; x: 3; width: 10; height: 10; anchors.verticalCenter: parent.verticalCenter }"

  Scenario: Letting one axis of a centring go keeps the other centred
    Given the file "Centred.qml":
      """qml
      import QtQuick

      Rectangle {
          Rectangle { id: c; width: 10; height: 10; anchors.centerIn: parent }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Centred.qml", "nodes": [{"id": "c"}], "sides": {"horizontalCenter": null}}
      """
    Then the file "Centred.qml" contains "anchors.verticalCenter: parent.verticalCenter"
    And the file "Centred.qml" does not contain "centerIn"
    And the file "Centred.qml" does not contain "horizontalCenter"

  Scenario: Letting a centring go with a centre line asked is not a split
    Given the file "Loosed.qml":
      """qml
      import QtQuick

      Rectangle {
          Rectangle { id: c; width: 10; height: 10; anchors.centerIn: parent }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Loosed.qml", "nodes": [{"id": "c"}], "centerIn": false, "sides": {"horizontalCenter": 0}}
      """
    Then the file "Loosed.qml" contains "anchors.horizontalCenter: parent.horizontalCenter"
    And the file "Loosed.qml" does not contain "centerIn"
    And the file "Loosed.qml" does not contain "verticalCenter"

  Scenario: Letting a centre line go while pinning a side of its axis pins the side
    Given the file "Sided.qml":
      """qml
      import QtQuick

      Rectangle {
          Rectangle { id: c; width: 10; height: 10; anchors.centerIn: parent }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Sided.qml", "nodes": [{"id": "c"}], "sides": {"horizontalCenter": null, "left": 12}}
      """
    Then the file "Sided.qml" contains "anchors.left: parent.left"
    And the file "Sided.qml" contains "anchors.leftMargin: 12"
    And the file "Sided.qml" contains "anchors.verticalCenter: parent.verticalCenter"
    And the file "Sided.qml" does not contain "centerIn"
    And the file "Sided.qml" does not contain "horizontalCenter"

  Scenario: The axis a split keeps centred is centred on the whole of what the centring named
    Given the file "Either.qml":
      """qml
      import QtQuick

      Rectangle {
          property bool useOther
          Rectangle { id: other; width: 50; height: 50 }
          Rectangle { id: c; width: 10; height: 10; anchors.centerIn: useOther ? other : parent }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Either.qml", "nodes": [{"id": "c"}], "sides": {"horizontalCenter": null}}
      """
    Then the file "Either.qml" contains "anchors.verticalCenter: (useOther ? other : parent).verticalCenter"
    And the file "Either.qml" does not contain "centerIn"

  Scenario: A centre and a side of one axis are not asked for together
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Main.qml", "nodes": [{"id": "box"}], "sides": {"top": 0, "verticalCenter": 0}}
      """
    Then the agent is refused with "invalid_argument"

  Scenario: A pin at the shared margin leaves the shared margin to say it
    Given the file "Margins.qml":
      """qml
      import QtQuick

      Rectangle {
          Rectangle {
              id: m
              anchors.margins: 24
              width: 50
          }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Margins.qml", "nodes": [{"id": "m"}], "sides": {"top": 24, "left": 0}}
      """
    Then the file "Margins.qml" contains "anchors.top: parent.top"
    And the file "Margins.qml" does not contain "anchors.topMargin"
    And the file "Margins.qml" contains "anchors.leftMargin: 0"
    And the file "Margins.qml" contains "anchors.margins: 24"

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

  # The centre line was all that held the axis: let go of with a side that
  # was never pinned, it keeps the place the preview shows.
  @preview
  Scenario: Letting the last centre line go with a side keeps it where the preview shows it
    Given the file "Lined.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300
          Rectangle { id: c; width: 50; height: 10; anchors.horizontalCenter: parent.horizontalCenter }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Lined.qml", "size": {"width": 400, "height": 300}}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "anchor", "doc": "Lined.qml", "nodes": [{"id": "c"}], "sides": {"horizontalCenter": null, "left": null}}
      """
    Then the file "Lined.qml" contains "x: 175"
    And the file "Lined.qml" does not contain "horizontalCenter"
