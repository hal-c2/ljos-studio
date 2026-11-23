# Sources:
#   features/ui/canvas_view.feature (clicking a frame beside the one shown
#   shows the document there; the frame left keeps the picture it last drew)

@ui @preview
Feature: A frame beside the one shown keeps the document's last picture
  Side by side, a target's frame beside the one shown keeps the last picture
  of the document drawn on that target. Another document's first picture can
  be the same as the last one's, pixel for pixel, as two empty pages are: it
  is still that document's picture, and kept for its target.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300
          color: "white"
      }
      """
    And the file "Copy.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300
          color: "white"
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has clicked the button "Side by side"

  # The picture was kept only when it differed from the last one drawn, and a
  # document's kept pictures are dropped when another is shown: the second
  # document's first picture, the same as the first's, was never kept, so the
  # frame beside had nothing to draw.
  Scenario: A document drawn the same as the last one keeps its picture beside the one shown
    Given the person has opened "Copy.qml" in the documents list
    When the person clicks the button "Show Tablet"
    Then the frame "Tablet" shows a picture of the preview
    And the frame beside it for "Desktop" shows the picture it last drew
