# Sources:
#   docs/MCP.md (node_arrange's `component_extract`)

@ui
Feature: A selected object becomes a component from the layers panel
  The person names it; it then waits in the palette with the project's own.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          Rectangle {
              id: box
              width: 40
              height: 40
              color: "gold"
          }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: The named component replaces the object and is offered by the palette
    When the person selects "Rectangle box" in the layers panel
    And the person makes a component "Tile" of the selection
    Then the file "Main.qml" says "Tile {"
    And the file "Tile.qml" says "color: \"gold\""
    And the palette offers "Tile"

  Scenario: The window's undo takes back both files
    When the person selects "Rectangle box" in the layers panel
    And the person makes a component "Tile" of the selection
    And the person clicks the button "Undo"
    Then the file "Main.qml" says "Rectangle {"
    And the file "Tile.qml" does not exist
