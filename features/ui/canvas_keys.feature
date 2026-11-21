# Sources:
#   docs/DESIGN.md (every gesture is a tool call an agent could make)
#   features/ui/canvas_drag.feature (the outline, the handles, Ctrl+D)
#   features/ui/canvas_view.feature (the handles keep their size)

@ui @preview
Feature: The selection is copied, duplicated and climbed from the keyboard
  Ctrl+C copies the selection, Ctrl+V pastes and Ctrl+D duplicates it
  wherever it was chosen, on the canvas or in the layers panel; a field
  being typed in keeps its own keys. Escape on the canvas selects the
  parent of what is selected. The handles of an object too small for them
  sit outside it, so what it shows is not covered.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300
          color: "white"

          Rectangle {
              id: badge
              x: 20; y: 20; width: 100; height: 60
              color: "red"

              Text {
                  id: label
                  x: 4; y: 4; width: 12; height: 10
                  text: "Hi"
                  font.pixelSize: 8
              }
          }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has pressed "Ctrl+1"
    And the canvas has finished moving

  Scenario: Escape selects the parent of what is selected
    Given the person has clicked the frame "Desktop" at 100, 60
    When the person presses "Escape"
    Then the layers panel has "Rectangle root" selected

  Scenario: Ctrl+D duplicates what was chosen in the layers panel
    Given the person has selected "Rectangle badge" in the layers panel
    When the person presses "Ctrl+D"
    Then the file "Main.qml" says "id: badge1"

  Scenario: Copy and paste work on what was chosen in the layers panel
    Given the person has selected "Rectangle badge" in the layers panel
    And the person has pressed "Ctrl+C"
    When the person presses "Ctrl+V"
    Then the file "Main.qml" says "id: badge1"

  Scenario: A field being typed in keeps Ctrl+D
    Given the person has selected "Rectangle badge" in the layers panel
    And the person has put the keyboard in the code view
    When the person presses "Ctrl+D"
    Then the file "Main.qml" does not say "id: badge1"

  Scenario: The handles of a small object sit outside it
    Given the person has entered "60" in the zoom field
    And the canvas has finished moving
    When the person selects "Text label" in the layers panel
    Then the canvas shows 8 resize handles
    And the canvas's resize handles leave the selection uncovered

  Scenario: The handles of a large object sit on its edges
    When the person selects "Rectangle badge" in the layers panel
    Then the canvas's resize handles sit on the selection's corners
