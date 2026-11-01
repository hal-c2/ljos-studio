# Sources:
#   docs/MCP.md (node_arrange `wrap` and `unwrap`)
#   docs/DESIGN.md (the lossless edit seam: what an edit does not touch stays)

Feature: Objects are put in a layout and taken out again
  Wrapping siblings puts a new Row, Column, Grid or Item where they are
  together, with them inside in reading order. The layout now places them, so
  the x and y written on them go; an Item places nothing, so theirs are kept,
  counted from its corner. Unwrapping takes them out again. Either is one undo
  step, and the text and comments of the moved objects stay as written.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300

          // the second one
          Rectangle { id: b; x: 100; y: 20; width: 50; height: 40; color: "red" }
          Rectangle { id: a; x: 10; y: 20; width: 50; height: 40 } // the first one
          Text { id: other; text: "stays" }
          Rectangle { id: c; x: 170; y: 90; width: 50; height: 40 }
          Rectangle { id: pinned; anchors.left: parent.left; y: 200; width: 40; height: 40 }
          Rectangle { id: half; width: parent.width / 2; height: 40 }
      }
      """
    And the studio tools are connected

  Scenario: A Row takes them left to right at their joint box, and keeps what they say
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "b"}, {"id": "a"}], "in": "Row", "spacing": 6}
      """
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300

          Row {
              id: row
              x: 10
              y: 20
              spacing: 6
              Rectangle { id: a; width: 50; height: 40 } // the first one
              // the second one
              Rectangle { id: b; width: 50; height: 40; color: "red" }
          }
          Text { id: other; text: "stays" }
          Rectangle { id: c; x: 170; y: 90; width: 50; height: 40 }
          Rectangle { id: pinned; anchors.left: parent.left; y: 200; width: 40; height: 40 }
          Rectangle { id: half; width: parent.width / 2; height: 40 }
      }
      """

  Scenario: Wrapping is one undo step
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}], "in": "Column"}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Main.qml" contains:
      """qml
      // the second one
          Rectangle { id: b; x: 100; y: 20; width: 50; height: 40; color: "red" }
          Rectangle { id: a; x: 10; y: 20; width: 50; height: 40 } // the first one
      """
    And the file "Main.qml" does not contain "Column"

  Scenario: Without a spacing, a Row keeps the gap they had
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}, {"id": "c"}], "in": "Row"}
      """
    Then the file "Main.qml" contains "spacing: 30"

  Scenario: A Column takes them top to bottom
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "c"}, {"id": "a"}], "in": "Column"}
      """
    Then the answer's "/nodes" has 3 entries
    And the file "Main.qml" contains:
      """qml
      Column {
              id: column
              x: 10
              y: 20
              spacing: 30
              Rectangle { id: a; width: 50; height: 40 } // the first one
              Rectangle { id: c; width: 50; height: 40 }
          }
      """

  Scenario: A Grid reads them row by row, as many across as the first row has
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "c"}, {"id": "b"}, {"id": "a"}], "in": "Grid", "spacing": 4}
      """
    Then the file "Main.qml" contains:
      """qml
      Grid {
              id: grid
              x: 10
              y: 20
              columns: 2
              spacing: 4
              Rectangle { id: a; width: 50; height: 40 } // the first one
              // the second one
              Rectangle { id: b; width: 50; height: 40; color: "red" }
              Rectangle { id: c; width: 50; height: 40 }
          }
      """

  Scenario: A Grid is told how many across
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}], "in": "Grid", "columns": 1}
      """
    Then the file "Main.qml" contains "columns: 1"

  Scenario: An Item places nothing, so what is in it stays where it was
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "b"}, {"id": "a"}], "in": "Item"}
      """
    Then the file "Main.qml" contains:
      """qml
      Item {
              id: item
              x: 10
              y: 20
              width: 140
              height: 40
              // the second one
              Rectangle { id: b; x: 90; y: 0; width: 50; height: 40; color: "red" }
              Rectangle { id: a; x: 0; y: 0; width: 50; height: 40 } // the first one
          }
      """

  Scenario: A taken name is not used again
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Item { id: row }"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "a"}], "in": "Row"}
      """
    Then the file "Main.qml" contains "id: row1"

  Scenario Outline: What would change its meaning in a layout is refused and writes nothing
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [<nodes>], "in": "Row"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "<kind>"
    And the file "Main.qml" does not contain "Row"

    Examples:
      | nodes                        | kind         |
      | {"id": "pinned"}             | anchored     |
      | {"id": "half"}               | reads_parent |
      | {"id": "root"}               | root         |

  Scenario: What is not side by side is not wrapped together
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "a"}, "text": "Item { id: inner }"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "b"}, {"id": "inner"}], "in": "Row"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "not_siblings"

  Scenario: A wrap asks for its kind of layout
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "a"}]}
      """
    Then the agent is refused with "invalid_argument"

  Scenario: Only a Grid has columns
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "a"}], "in": "Row", "columns": 2}
      """
    Then the agent is refused with "invalid_argument"

  Scenario: A locked object is not wrapped
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "lock", "doc": "Main.qml", "nodes": [{"id": "a"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "a"}], "in": "Row"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "locked"

  Scenario: Unwrapping an Item puts them back where they are, as one undo step
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "b"}, {"id": "a"}], "in": "Item"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "unwrap", "doc": "Main.qml", "nodes": [{"id": "item"}]}
      """
    Then the file "Main.qml" contains:
      """qml
      // the second one
          Rectangle { id: b; x: 100; y: 20; width: 50; height: 40; color: "red" }
          Rectangle { id: a; x: 10; y: 20; width: 50; height: 40 } // the first one
          Text { id: other
      """
    And the file "Main.qml" does not contain "Item"
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Main.qml" contains "Item {"

  Scenario: Unwrapping a Row with no preview leaves them without a position
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "a"}], "in": "Row"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "unwrap", "doc": "Main.qml", "nodes": [{"id": "row"}]}
      """
    Then the file "Main.qml" contains "Rectangle { id: a; width: 50; height: 40 } // the first one"
    And the file "Main.qml" does not contain "Row"

  Scenario: What holds more than a place cannot be unwrapped
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Row { id: used; Text {} }"}
      """
    And the agent has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "other"}, "name": "x", "value": "used.width"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "unwrap", "doc": "Main.qml", "nodes": [{"id": "used"}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "referenced"
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "unwrap", "doc": "Main.qml", "nodes": [{"id": "a"}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "not_a_layout"

  @preview
  Scenario: Unwrapping puts them back where the preview shows them
    Given the file "Shown.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300
          Row {
              id: line
              x: 20; y: 30
              spacing: 5
              Rectangle { id: one; width: 50; height: 40 }
              Rectangle { id: two; width: 50; height: 40 }
          }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Shown.qml", "size": {"width": 400, "height": 300}}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "unwrap", "doc": "Shown.qml", "nodes": [{"id": "line"}]}
      """
    Then the file "Shown.qml" contains "Rectangle { id: one; width: 50; height: 40; x: 20; y: 30 }"
    And the file "Shown.qml" contains "Rectangle { id: two; width: 50; height: 40; x: 75; y: 30 }"

  Scenario: A wrap is refused while a sibling that stays is anchored to what would move
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Rectangle { id: beside; anchors.left: a.right }"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "a"}], "in": "Row"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "anchored_to"
    And the refusal's message matches "beside"
    And the file "Main.qml" does not contain "Row"

  Scenario: A declared property reading its parent is refused a wrap too
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Rectangle { id: reader; property real w: parent.width }"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "reader"}], "in": "Row"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "reads_parent"

  Scenario Outline: A wrapper that does more than place is not unwrapped: <what>
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Item { id: box; <what>\n Text {} }"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "unwrap", "doc": "Main.qml", "nodes": [{"id": "box"}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "holds_more"
    And the file "Main.qml" contains "id: box"

    Examples:
      | what                                        |
      | visible: false                              |
      | opacity: 0.5                                |
      | Component.onCompleted: console.log('up')    |
      | Keys.onPressed: {}                          |
      | states: [ State { name: 's' } ]             |
      | NumberAnimation on x { to: 5 }              |
      | Behavior on x { NumberAnimation {} }        |
      | enum Mode { A, B }                          |

  Scenario Outline: With no preview, a wrapper whose place only a run knows is not unwrapped: <what>
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "<type> { id: box; <what>; Rectangle { id: kid; width: 10; height: 10 } }"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "unwrap", "doc": "Main.qml", "nodes": [{"id": "box"}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "needs_preview"
    And the file "Main.qml" contains "id: box"

    Examples:
      | type | what                         |
      | Item | anchors.right: parent.right  |
      | Row  | anchors.right: parent.right  |
      | Item | x: 2 + 3                     |

  @preview
  Scenario: An anchored Item's children are put back where the preview shows them
    Given the file "Shown.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300
          Item {
              id: box
              anchors.right: parent.right
              anchors.bottom: parent.bottom
              width: 100; height: 100
              Rectangle { id: kid; x: 5; y: 5; width: 10; height: 10 }
          }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Shown.qml", "size": {"width": 400, "height": 300}}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "unwrap", "doc": "Shown.qml", "nodes": [{"id": "box"}]}
      """
    Then the file "Shown.qml" contains "x: 305"
    And the file "Shown.qml" contains "y: 205"

  Scenario: What only places goes with the wrapper
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Row { id: box; x: 5; y: 6; width: 100; spacing: 4; Text {} }"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "unwrap", "doc": "Main.qml", "nodes": [{"id": "box"}]}
      """
    Then the file "Main.qml" does not contain "id: box"

  Scenario: A child anchored to the wrapper is not unwrapped
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Item { id: box; width: 100; height: 100; Rectangle { anchors.fill: parent } }"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "unwrap", "doc": "Main.qml", "nodes": [{"id": "box"}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "reads_parent"
    And the file "Main.qml" contains "id: box"
