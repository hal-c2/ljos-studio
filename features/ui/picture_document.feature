# Sources:
#   features/ui/canvas.feature (the frame with the preview's picture in it)
#   docs/DOGFOOD.md (found by editing a timer by hand and going back to it)

@ui @preview
Feature: The frame shows the document that is open
  Every preview counts its own generations from 1, so the number alone does
  not say that the picture on the canvas is this document's. Going to another
  document asks for its picture, whatever its number.

  Scenario: Going to a document at the same generation shows that document
    Given the file "Blue.qml":
      """qml
      import QtQuick
      Rectangle { width: 200; height: 200; color: "blue" }
      """
    And the file "Red.qml":
      """qml
      import QtQuick
      Rectangle { width: 200; height: 200; color: "red" }
      """
    And Studio's window shows the project
    And the person has opened "Blue.qml" in the documents list
    When the person opens "Red.qml" in the documents list
    Then the frame "Desktop"'s picture is red at 100, 100
