# Sources:
#   docs/DOGFOOD.md (D15: the inspector's colour picker)
#   docs/MCP.md (node_select: an agent moves the selection the window shows)

@ui
Feature: The colour picker belongs to the object it was opened on
  The selection is shared, so an agent can move it while a picker is open. A
  picker writes to the object it was opened on, goes away when the selection
  moves, and takes no answer the preview gave about another object.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: a
          property color shade: Qt.rgba(1, 0, 0, 1)
          color: "#ff0000"
          width: 200
          height: 200

          Rectangle {
              id: b
              property color shade: Qt.rgba(0, 0, 1, 1)
              color: "#00ff00"
              width: 100
              height: 100
          }

          Rectangle {
              color: "#ffff00"
              width: 50
              height: 50
          }
      }
      """
    And the file "Other.qml":
      """qml
      import QtQuick

      Rectangle {
          id: elsewhere
          color: "#00ffff"
          width: 100
          height: 100
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has selected "Rectangle a" in the layers panel

  Scenario: A picker closes when an agent moves the selection
    When the person clicks the button "Pick color"
    And an agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "b"}]}
      """
    Then the window has no colour picker open

  Scenario: A slider let go after the selection moved writes to the object it was opened on
    When the person clicks the button "Pick color"
    And the person drags the picker's "Brightness" to its middle while an agent selects "b"
    Then the file "Main.qml" says "#800000"
    And the file "Main.qml" says "#00ff00"

  Scenario: A slider let go after the selection moved records on the object it was opened on
    Given the person has added the state "s" in the inspector
    And the person has picked the state "s" in the inspector
    When the person clicks the button "Pick color"
    And the person drags the picker's "Brightness" to its middle while an agent calls "node_select" with '{"doc": "Main.qml", "nodes": [{"id": "b"}]}'
    Then the file "Main.qml" says "target: a"
    And the file "Main.qml" does not say "target: b"

  Scenario: A slider let go after its object was named again records under the new name
    Given the person has added the state "s" in the inspector
    And the person has picked the state "s" in the inspector
    When the person clicks the button "Pick color"
    And the person drags the picker's "Brightness" to its middle while an agent calls "node_arrange" with '{"doc": "Main.qml", "action": "rename", "nodes": [{"id": "a"}], "name": "c"}'
    Then the file "Main.qml" says "target: c"

  Scenario: A slider let go after its object was removed writes nothing
    Given the person has selected "Rectangle b" in the layers panel
    And the person has added the state "s" in the inspector
    And the person has picked the state "s" in the inspector
    When the person clicks the button "Pick color"
    And the person drags the picker's "Brightness" to its middle while an agent calls "node_remove" with '{"doc": "Main.qml", "node": {"id": "b"}}'
    Then the file "Main.qml" does not say "#008000"
    And the window says "The object this picker was opened on has gone"

  Scenario: A slider let go after a selection in another document writes to the object it was opened on
    Given the person has selected "Rectangle b" in the layers panel
    When the person clicks the button "Pick color"
    And the person drags the picker's "Brightness" to its middle while an agent calls "node_select" with '{"doc": "Other.qml", "nodes": [{"id": "elsewhere"}]}'
    Then the file "Main.qml" says "#008000"
    And the file "Other.qml" says "#00ffff"

  Scenario: An object with no id cannot record into a state
    Given the person has added the state "s" in the inspector
    And the person has picked the state "s" in the inspector
    And the person has selected "Rectangle" in the layers panel
    When the person clicks the button "Pick color"
    And the person drags the picker's "Brightness" to its middle
    Then the window says "A state names the object it changes: give this one an id"
    And the file "Main.qml" does not say "#808000"

  @preview
  Scenario: A late answer about another object does not seed the picker
    Given the person has selected "Rectangle b" in the layers panel
    When the person clicks the button "Pick shade" and the preview's answer about "a" comes first, saying "#ff00ff"
    Then the colour picker holds "#0000ff"

  @preview
  Scenario: The answer to another call about the object does not end the picker's wait
    Given the person has selected "Rectangle b" in the layers panel
    When the person clicks the button "Pick shade" and the answer to showing a state on the same object comes first
    Then the colour picker still waits for the preview

  @preview
  Scenario: A read with no value for the name goes on from grey
    Given the person has selected "Rectangle b" in the layers panel
    When the person clicks the button "Pick shade" and the preview's answer has no value for it
    Then the colour picker waits on nothing
    And the colour picker goes on from grey, with no reading in its hex
    And the window shows no refusal

  @preview
  Scenario: A value the running app has not got yet starts the picker from a default
    When the person replaces "id: a" with "id: a\n    property var borderColor" in the code view
    And the person selects "Rectangle a" in the layers panel
    And the person clicks the button "Pick borderColor"
    Then the window does not say "null is not a colour"
    And the colour picker waits on nothing
    And the colour picker holds "#000000"
    And the picker's "Brightness" can be moved
    And the file "Main.qml" does not say "#000000"
