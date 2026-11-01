# Sources:
#   docs/MCP.md (node_arrange `anchor`, `wrap`)
#   features/edit/anchors.feature (the same actions as an agent has them)

@ui
Feature: A person pins an object from the inspector's Layout section
  A small pad shows the four sides and the centre; pressing one pins or
  unpins it. A layout shows its spacing and columns here too, and the selected
  object can be wrapped from here.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          width: 400; height: 300
          Rectangle { id: box; x: 10; y: 20; width: 50; height: 40 }
          Grid {
              id: table
              x: 100
              spacing: 3
              columns: 2
              Text { id: cell }
          }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: Pressing a side pins it, pressing it again takes it off
    When the person selects "Rectangle box" in the layers panel
    And the person clicks the button "Pin left"
    Then the file "Main.qml" says "anchors.left: parent.left"
    When the person clicks the button "Pin left"
    Then the file "Main.qml" does not say "anchors.left"

  Scenario: The centre pins it to the middle
    When the person selects "Rectangle box" in the layers panel
    And the person clicks the button "Pin centre"
    Then the file "Main.qml" says "anchors.centerIn: parent"

  Scenario: An object is wrapped from the section
    When the person selects "Rectangle box" in the layers panel
    And the person clicks the button "Wrap in Column"
    Then the file "Main.qml" says "Column {"

  Scenario: A layout says its spacing here
    When the person selects "Grid table" in the layers panel
    Then the inspector's "Layout spacing" holds "3"
    And the inspector's "Layout columns" holds "2"

  Scenario: What a layout places cannot be pinned, and the section says so
    When the person selects "Text cell" in the layers panel
    Then the inspector has no button "Pin left"
    And the window says "It is placed by its Grid"
