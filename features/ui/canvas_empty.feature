# Sources:
#   features/ui/canvas.feature (the canvas shows the document in a frame)
#   features/ui/canvas_view.feature (Ctrl+0 fits the frame into the view)
#   features/ui/new_document.feature (a project opens on its entry, Main.qml)

@ui @preview
Feature: A move of the empty canvas leaves the first document fitted
  With more than one document and no entry, Main.qml, among them, Studio
  opens none by itself, and the canvas stays empty until the person picks
  one. A move of the view there has no frame to keep in sight, so the first
  document opened still comes fitted.

  Background:
    Given the file "Home.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300
          color: "white"
      }
      """
    And the file "Other.qml":
      """qml
      import QtQuick

      Item { }
      """
    And Studio's window shows the project

  # The view was the person's to keep once they had moved it, even with no
  # frame on the canvas: the first document opened at 100 %, its frame's
  # name and the top of the app above the canvas's edge.
  Scenario: A turn of the wheel before any document is open leaves the first one fitted
    Given the person has turned the wheel over the empty canvas
    When the person opens "Home.qml" in the documents list
    And the canvas has finished moving
    Then the frame "Desktop" fits in the canvas
