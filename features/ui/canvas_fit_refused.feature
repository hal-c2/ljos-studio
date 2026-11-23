# Sources:
#   features/ui/canvas_view.feature (a zoom chosen stays when the target changes)
#   features/ui/canvas_play.feature (Fit content)
#   docs/MCP.md (a host that cannot say how tall its content is refuses)

@ui @preview
Feature: A refused Fit content leaves the view where the person put it
  A preview that cannot say how tall its content is refuses "Fit content".
  The frame keeps its screen's height, and nothing is left waiting for a
  fit that will not come: a later change of the frame's size does not take
  the view from where the person put it.

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
    And the preview does not say where its objects are
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: A zoom chosen before a refused Fit content stays when the target changes
    Given the person has pressed "Ctrl+1"
    And the canvas has finished moving
    And the person has clicked the button "Fit content"
    When the person chooses the target "Phone"
    And the canvas has finished moving
    Then the zoom reads "100%"
