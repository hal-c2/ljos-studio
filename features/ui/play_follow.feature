# Sources:
#   docs/DOGFOOD.md (D4: a continuous animation went stale after the looks ran out)
#   features/ui/flows.feature (play mode)

@ui @preview
Feature: The frame follows what the app draws in play mode
  Looks after a click stop at a bound, so an app that animates for ever went
  stale there. In play mode the picture is asked for when the app has drawn,
  and an app at rest is asked for nothing.

  Background:
    Given the file "Spin.qml":
      """qml
      import QtQuick

      Rectangle {
          width: 200; height: 200
          Rectangle {
              width: 20; height: 20; color: "red"
              NumberAnimation on x { from: 0; to: 180; duration: 1500; loops: Animation.Infinite }
          }
      }
      """
    And the file "Still.qml":
      """qml
      import QtQuick

      Rectangle { width: 200; height: 200; color: "white" }
      """
    And Studio's window shows the project

  Scenario: A continuous animation keeps moving in the frame in play mode
    Given the person has opened "Spin.qml" in the documents list
    And the person has clicked the button "Play"
    And the person has clicked the frame "Desktop" at 100, 100
    And 4 seconds have passed
    And the person has looked at the frame "Desktop"
    When 1 seconds pass
    Then the frame "Desktop" has drawn a different picture since

  Scenario: A still app in play mode is asked for no pictures
    Given the person has opened "Still.qml" in the documents list
    And the person has clicked the button "Play"
    And 1 seconds have passed
    And the person has counted the pictures asked for
    When 2 seconds pass
    Then no more pictures have been asked for

  Scenario: Leaving play mode stops following the animation
    Given the person has opened "Spin.qml" in the documents list
    And the person has clicked the button "Play"
    And 1 seconds have passed
    And the person has clicked the button "Play"
    And 1 seconds have passed
    And the person has counted the pictures asked for
    When 2 seconds pass
    Then no more pictures have been asked for
