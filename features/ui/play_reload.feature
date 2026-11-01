# Sources:
#   features/ui/play_follow.feature (what follows the picture in play mode)
#   features/ui/reskin.feature (a shell made again as it runs)

@ui @preview
Feature: A shell made again during Play leaves nothing following the preview
  Play mode follows the preview's picture from a thread. Leaving play mode
  stops it, and so does the Workspace it follows for going away, which no
  QML handler hears of when a reload takes the shell.

  Scenario: The thread following the picture ends with the Workspace it follows
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
    And Studio's window runs the person's shell, a copy of its own
    And the person has opened "Spin.qml" in the documents list
    And the person has clicked the button "Play"
    And 1 thread is following the preview's picture
    And the person has remembered the window's workspace
    And the person's "Shell.qml" has "id: shell" replaced with "id: shell\n    Component.onCompleted: { throw new Error(\"no shell today\") }"
    When the window takes the change
    Then the remembered workspace is gone
    And no thread is following the preview's picture
