# Sources:
#   docs/DOGFOOD.md (D13: the Layers panel cannot reorder, rename, hide or lock)
#   features/edit/layers.feature (the same actions as an agent has them)

@ui
Feature: A person arranges the layers in the panel
  Dragging a row is `node_move`; a double-click names the object; the eye and
  the lock are `node_arrange`, and neither writes the file.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          Text { id: first }
          Text { id: second }
          Text { id: third }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: Dragging a row reorders the objects
    When the person drags the layer "Text third" onto "Text first"
    Then the file "Main.qml" says:
      """qml
      import QtQuick

      Item {
          id: root
          Text { id: third }
          Text { id: first }
          Text { id: second }
      }
      """

  Scenario: A drag that goes nowhere writes nothing
    When the person drags the layer "Text third" onto "Text third"
    Then the history does not list "move"

  Scenario: A double-click names the object
    When the person double-clicks the layer "Text second"
    And the person enters "subtitle" as the layer's name
    Then the file "Main.qml" says "Text { id: subtitle }"
    And the layers panel lists "Item root, Text first, Text subtitle, Text third"

  Scenario: Selecting another row closes the name field and changes nothing
    When the person double-clicks the layer "Text second"
    And the person selects "Text third" in the layers panel
    Then the layers panel names no object
    And the layers panel has "Text third" selected
    And the file "Main.qml" says "Text { id: second }"

  Scenario: A name typed and left is not offered again
    When the person double-clicks the layer "Text second"
    And the person presses "x"
    And the person selects "Text third" in the layers panel
    And the person double-clicks the layer "Text second"
    And the person presses "Return"
    Then the file "Main.qml" says "Text { id: second }"

  Scenario: The eye and the lock are the row's and write nothing
    When the person clicks the button "Hide Text second"
    And the person clicks the button "Lock Text second"
    Then the layers panel has the button "Show Text second"
    And the layers panel has the button "Unlock Text second"
    And the file "Main.qml" says "Text { id: second }"
    And the history does not list "hide"
