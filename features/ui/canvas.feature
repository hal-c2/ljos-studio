# Sources:
#   docs/DESIGN.md (S3: canvas of frames; selection sync; idle 0 %)
#   features/preview/preview.feature (the preview the canvas shows)

@ui
Feature: The canvas shows the document running on a target
  The canvas is a frame per target with the preview's picture in it, the
  same preview `preview_open` starts for an agent. Clicking an object in
  the frame selects it everywhere: in the layers panel, the inspector and
  the code view. A window nobody touches does no work.

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
          }
      }
      """
    And Studio's window shows the project

  @preview
  Scenario: Opening a document shows it in a frame for the desktop
    When the person opens "Main.qml" in the documents list
    Then the canvas has a frame "Desktop" of 1280 by 800
    And the frame "Desktop" shows a picture of the preview

  @preview
  Scenario: Choosing a target lays the frame out for it
    Given the person has opened "Main.qml" in the documents list
    When the person chooses the target "Phone"
    Then the canvas has a frame "Phone" of 390 by 844

  @preview
  Scenario: Clicking an object in the frame selects it everywhere
    Given the person has opened "Main.qml" in the documents list
    When the person clicks the frame "Desktop" at 40, 40
    Then the layers panel has "Rectangle badge" selected
    And the inspector shows "color" as "\"red\""

  @preview
  Scenario: An edit in the inspector shows in the frame
    Given the person has opened "Main.qml" in the documents list
    And the person has selected "Rectangle badge" in the layers panel
    When the person sets "color" to "\"blue\"" in the inspector
    Then the frame "Desktop" shows the preview's generation 2

  Scenario: A window nobody touches does no work
    Given the person has opened "Main.qml" in the documents list
    When nobody touches the window for 2 seconds
    Then Studio's window ran no frames
