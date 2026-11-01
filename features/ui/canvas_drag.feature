# Sources:
#   docs/DESIGN.md (section 6: the drag is drawn by Studio, one edit on drop)
#   docs/DOGFOOD.md (D1: direct manipulation)
#   features/edit/place.feature (the tools a drop goes through)

@ui @preview
Feature: Objects are moved and resized on the canvas
  A selected object has an outline and eight handles. A drag moves a ghost
  Studio draws, snapping to its siblings and parent; nothing is written until
  the drop, which is one edit and one undo step.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      import QtQuick.Layouts

      Rectangle {
          id: root
          width: 400; height: 300
          color: "white"

          Rectangle { id: a; x: 20; y: 20; width: 50; height: 50; color: "red" }
          Rectangle { id: b; x: 200; y: 100; width: 50; height: 50; color: "blue" }
          RowLayout {
              id: row
              x: 0; y: 200; width: 300; height: 40
              spacing: 0
              Rectangle { id: one; Layout.preferredWidth: 100; Layout.preferredHeight: 40; color: "green" }
              Rectangle { id: two; Layout.preferredWidth: 100; Layout.preferredHeight: 40; color: "gold" }
          }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has pressed "Ctrl+1"

  Scenario: A selected object has an outline and eight handles
    When the person clicks the frame "Desktop" at 40, 40
    Then the canvas outlines the selection at 20, 20 sized 50 by 50
    And the canvas shows 8 resize handles

  Scenario: Dragging writes nothing until the drop
    Given the person has clicked the frame "Desktop" at 40, 40
    When the person presses in the frame "Desktop" at 40, 40 and holds it at 90, 63
    Then the file "Main.qml" says "id: a; x: 20; y: 20;"

  Scenario: A drop moves the object as one undo step
    Given the person has clicked the frame "Desktop" at 40, 40
    When the person drags in the frame "Desktop" from 40, 40 to 90, 63
    Then the file "Main.qml" says "id: a; x: 70; y: 43;"
    And the history lists "place"
    And the canvas outlines the selection at 70, 43 sized 50 by 50
    When the person presses "Ctrl+Z"
    Then the file "Main.qml" says "id: a; x: 20; y: 20;"

  Scenario: An object can be pressed and dragged in one go
    When the person drags in the frame "Desktop" from 40, 40 to 90, 63
    Then the file "Main.qml" says "id: a; x: 70; y: 43;"

  Scenario: A corner handle resizes
    Given the person has clicked the frame "Desktop" at 40, 40
    When the person drags in the frame "Desktop" from 70, 70 to 110, 90
    Then the file "Main.qml" says "id: a; x: 20; y: 20; width: 90; height: 70;"

  Scenario: A drag snaps to a sibling's edge and shows the guide
    Given the person has clicked the frame "Desktop" at 40, 40
    When the person presses in the frame "Desktop" at 40, 40 and holds it at 218, 60
    Then the canvas shows a x guide at 200

  Scenario: Dragging in a layout says so and changes the order
    When the person clicks the frame "Desktop" at 50, 220
    Then the canvas says "The RowLayout places it: a drag changes its order"
    When the person drags in the frame "Desktop" from 50, 220 to 280, 220
    Then the file "Main.qml" says "id: two;"
    And the history lists "move"

  Scenario: A drop in a layout before the siblings are read still changes the order
    Given the person has clicked the frame "Desktop" at 50, 220
    When the person drags in the frame "Desktop" from 50, 220 to 280, 220 at once
    Then the file "Main.qml" says "id: two;"
    And the history lists "move"

  Scenario: Arrow keys nudge, shift by ten
    Given the person has clicked the frame "Desktop" at 40, 40
    When the person presses "Right"
    And the person presses "Shift+Down"
    Then the file "Main.qml" says "id: a; x: 21; y: 30;"

  Scenario: Ctrl+D duplicates the selection
    Given the person has clicked the frame "Desktop" at 40, 40
    When the person presses "Ctrl+D"
    Then the file "Main.qml" says "id: a1; x: 36; y: 36;"

  Scenario: Delete takes out every selected object in one undo step
    Given an agent has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}]}
      """
    When the person presses "Delete"
    Then the file "Main.qml" does not say "id: a;"
    And the file "Main.qml" does not say "id: b;"
    When the person presses "Ctrl+Z"
    Then the file "Main.qml" says "id: a;"
    And the file "Main.qml" says "id: b;"

  Scenario: Keys do not edit the document while the canvas plays
    Given the person has clicked the frame "Desktop" at 40, 40
    And the person has clicked the button "Play"
    When the person clicks the frame "Desktop" at 40, 40
    And the person presses "Delete"
    Then the file "Main.qml" says "id: a;"
    And the history lists nothing

  Scenario: Align buttons line up a multiple selection
    Given an agent has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}]}
      """
    When the person clicks the button "Align Top"
    Then the file "Main.qml" says "id: b; x: 200; y: 20;"

  Scenario: Arrow presses faster than the preview reloads all count
    When the person clicks the frame "Desktop" at 30, 30
    And the person presses "Right, Right, Right, Down" without waiting between them
    Then the file "Main.qml" says "id: a; x: 23; y: 21;"

  Scenario: The canvas keeps the keyboard after an edit reloads the preview
    When the person clicks the frame "Desktop" at 30, 30
    And the person presses "Right"
    And the person presses "Right"
    And the person presses "Down"
    Then the file "Main.qml" says "id: a; x: 22; y: 21;"
