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
    And the file "Once.qml":
      """qml
      import QtQuick

      Rectangle {
          width: 200; height: 200
          Rectangle {
              width: 20; height: 20; color: "red"
              NumberAnimation on x { from: 0; to: 180; duration: 1500 }
          }
      }
      """
    And the file "Slow.qml":
      """qml
      import QtQuick

      Rectangle {
          width: 200; height: 200
          Timer { interval: 60000; running: true; repeat: true }
          Rectangle {
              width: 20; height: 20; color: "red"
              NumberAnimation on x {
                  id: spin
                  running: false
                  from: 0; to: 180; duration: 1500; loops: Animation.Infinite
              }
          }
          MouseArea { anchors.fill: parent; onClicked: spin.start() }
      }
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

  Scenario: A click starting an animation is seen though the app's next wake is far off
    Given the person has opened "Slow.qml" in the documents list
    And the person has clicked the button "Play"
    And 1 seconds have passed
    And the person has clicked the frame "Desktop" at 100, 100
    And 4 seconds have passed
    And the person has looked at the frame "Desktop"
    When 1 seconds pass
    Then the frame "Desktop" has drawn a different picture since

  Scenario: The frame ends on the frame an animation ends on
    Given the person has opened "Once.qml" in the documents list
    And the person has clicked the button "Play"
    When 4 seconds pass
    Then the frame "Desktop"'s picture is red at 198, 10

  # The next picture is asked for after the last has landed. Asked for before,
  # the job lane is never free, and a step waiting for it to be never ends.
  Scenario: A picture slower than the app moves leaves the window time to rest
    Given the preview takes 300 ms over each picture
    And the person has opened "Spin.qml" in the documents list
    And the person has clicked the button "Play"
    And 2 seconds have passed
    And the person has looked at the frame "Desktop"
    When 2 seconds pass
    Then the frame "Desktop" has drawn a different picture since

  Scenario: A click in a picture slower than the app moves leaves the window time to rest
    Given the preview takes 300 ms over each picture
    And the person has opened "Spin.qml" in the documents list
    And the person has clicked the button "Play"
    And the person has clicked the frame "Desktop" at 100, 100
    And 2 seconds have passed
    And the person has looked at the frame "Desktop"
    When 2 seconds pass
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
