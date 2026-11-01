# Sources:
#   docs/DESIGN.md (S3: the inspector shows the selection)

@ui
Feature: A row of the inspector holds what the selected object has
  Rows are not made anew for a list that reads the same, so two objects
  with the same bindings share them: what was typed and not entered in one
  is not the other's.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Column {
          Rectangle { width: 10 }
          Item { width: 10 }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: What is typed and not entered does not go with the selection
    Given the person has selected "Rectangle" in the layers panel
    When the person types "99" in the inspector's "width" and leaves it
    And the person selects "Item" in the layers panel
    Then the inspector's "width" holds "10"
