# Sources:
#   docs/MCP.md (`node_arrange`, layout tools)
#   Qt Quick docs: Row, Column, Grid, Flow; RowLayout, ColumnLayout, GridLayout

Feature: A layout is turned into another kind of layout
  Three texts in a Row don't fit a phone, and taking them out and putting them
  in a new Flow would lose the Row's place, its id and what reads it. `relayout`
  writes only the type's name. A positioner turns into a positioner and a
  layout into a layout: the children of one are placed by their own numbers,
  of the other by what they say of `Layout`.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      import QtQuick.Layouts

      Item {
          id: root
          Row {
              id: cards
              y: 40
              spacing: 8
              Text { text: "One" }
              Text { text: "Two" }
          }
          Grid {
              id: table
              columns: 2
              Text { text: "Cell" }
          }
          RowLayout {
              id: bar
              Text { text: "Bar" }
          }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"

  Scenario: A Row turns into a Flow and keeps everything else it says
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "relayout", "doc": "Main.qml", "nodes": [{"id": "cards"}], "name": "Flow"}
      """
    Then the file "Main.qml" contains "Flow {"
    And the file "Main.qml" does not contain "Row {"
    And the file "Main.qml" contains "spacing: 8"

  Scenario: Turning it is one undo step
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "relayout", "doc": "Main.qml", "nodes": [{"id": "cards"}], "name": "Column"}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Main.qml" contains "Row {"
    And the file "Main.qml" does not contain "Column {"

  Scenario: A layout turns into a layout
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "relayout", "doc": "Main.qml", "nodes": [{"id": "bar"}], "name": "ColumnLayout"}
      """
    Then the file "Main.qml" contains "ColumnLayout {"
    And the file "Main.qml" does not contain "RowLayout {"

  Scenario Outline: What would not load, or would move the children, is refused
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "relayout", "doc": "Main.qml", "nodes": [{"id": "<node>"}], "name": "<kind>"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "<why>"

    Examples:
      | node  | kind      | why          |
      | root  | Row       | not_a_layout |
      | cards | Rectangle | not_a_layout |
      | cards | RowLayout | other_family |
      | table | Row       | says_other   |
