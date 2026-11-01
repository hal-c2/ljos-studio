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

  Scenario: What a drag says stays clear of the bar saying which state it goes into
    Given the person has selected "Rectangle root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    When the person clicks the frame "Desktop" at 50, 220
    Then the canvas says "The RowLayout places it: a drag changes its order"
    And the canvas's words are clear of its recording bar

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

  Scenario: A flick's outline does not go back while the drop is on its way
    Given the person has clicked the frame "Desktop" at 40, 40
    When the person flicks in the frame "Desktop" from 40, 40 to 90, 63, watching the outline
    Then the file "Main.qml" says "id: a; x: 70; y: 43;"
    And the outline was never shown back at 20, 20 once the hand let go
    And the canvas outlines the selection at 70, 43 sized 50 by 50

  # The ghost of a drop is kept until the drop has ended, so each way it can
  # end is a row: one left out leaves a dim outline and no handles behind.
  Scenario Outline: However a drop ends, the ghost goes and the handles come back: <ending>
    Given <before>
    And the person has clicked the frame "Desktop" at <at>
    When <gesture>
    And <meanwhile>
    And the canvas has had time to settle
    Then no ghost is left on the canvas
    And the canvas shows <handles> resize handles
    And <written>

    Examples:
      | ending                       | before                 | at      | gesture                                                                                 | meanwhile                                       | handles | written                                                 |
      | place answered               | nothing has happened   | 40, 40  | the person drags in the frame "Desktop" from 40, 40 to 90, 63                           | nothing has happened                            | 8       | the file "Main.qml" says "id: a; x: 70; y: 43;"         |
      | place refused                | Ada holds "a"          | 40, 40  | the person drags in the frame "Desktop" from 40, 40 to 90, 63                           | nothing has happened                            | 8       | the history lists nothing                               |
      | reorder answered             | nothing has happened   | 50, 220 | the person drags in the frame "Desktop" from 50, 220 to 280, 220                        | nothing has happened                            | 8       | the history lists "move"                                |
      | reorder refused              | Ada holds "one"        | 50, 220 | the person drags in the frame "Desktop" from 50, 220 to 280, 220                        | nothing has happened                            | 8       | the history lists nothing                               |
      | waiting drop reorders        | the preview takes 500 ms to say where its objects are | 50, 220 | the person drags in the frame "Desktop" from 50, 220 to 280, 220 at once                | nothing has happened                            | 8       | the history lists "move"                                |
      | waiting drop changes nothing | the preview takes 500 ms to say where its objects are | 50, 220 | the person drags in the frame "Desktop" from 50, 220 to 60, 220 at once                 | nothing has happened                            | 8       | the history lists nothing                               |
      | drop just after a nudge      | nothing has happened   | 40, 40  | the person presses "Right" and drags in the frame "Desktop" from 41, 40 to 91, 63       | nothing has happened                            | 8       | the file "Main.qml" says "id: a; x: 71; y: 43;"         |
      | drag cancelled               | nothing has happened   | 40, 40  | the person presses in the frame "Desktop" at 40, 40 and holds it at 90, 63              | the window takes the press away                 | 8       | the history lists nothing                               |
      | selection moved in the drag  | nothing has happened   | 40, 40  | the person presses in the frame "Desktop" at 40, 40 and holds it at 90, 63              | an agent selects "b" and the hand lets go       | 8       | the history lists nothing                               |
      | selection moved in the wait  | nothing has happened   | 40, 40  | the person presses in the frame "Desktop" at 40, 40 and holds it at 90, 63              | the drop waits on a read and an agent selects "b" | 8       | the history lists nothing                               |

  # A press is a new gesture: the read of the last drop answering while the
  # hand is down would otherwise decide that drop with where this drag has got.
  Scenario: A press ends a drop that was waiting on a read
    Given the person has clicked the frame "Desktop" at 50, 220
    And the preview takes 500 ms to say where its objects are
    And the person has dragged in the frame "Desktop" from 50, 220 to 60, 220 at once, leaving the read on its way
    When the person presses in the frame "Desktop" at 50, 220 and at once holds it at 280, 220
    Then the history lists nothing

  # The canvas is built on the preview: without one there is no overlay left
  # to hold a ghost. The object the hand holds is still in the file, so the
  # drop goes in.
  Scenario Outline: A preview that goes takes the drop's canvas with it: <when>
    Given the person has clicked the frame "Desktop" at 40, 40
    When <gesture>
    And <meanwhile>
    Then the canvas has gone with the preview
    And <written>

    Examples:
      | when           | gesture                                                                             | meanwhile                               | written                                         |
      | in the drag    | the person presses in the frame "Desktop" at 40, 40 and holds it at 90, 63          | the preview closes and the hand lets go | the file "Main.qml" says "id: a; x: 70; y: 43;" |
      | after the drop | the person lets a drag go in the frame "Desktop" from 40, 40 to 90, 63, not waiting | the preview closes                      | the file "Main.qml" says "id: a; x: 70; y: 43;" |

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

  # A drop used to write the object's own value whatever the inspector said it
  # was recording, so the state stayed empty and the base value changed.
  Scenario: A drop while a state is picked goes into that state
    Given the person has selected "Rectangle root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    And the person has clicked the frame "Desktop" at 40, 40
    When the person drags in the frame "Desktop" from 40, 40 to 90, 63
    Then the file "Main.qml" says "id: a; x: 20; y: 20;"
    And the file "Main.qml" says "target: a"
    And the file "Main.qml" says "x: 70"
    And the file "Main.qml" says "y: 43"

  Scenario: An arrow press while a state is picked goes into that state
    Given the person has selected "Rectangle root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    And the person has clicked the frame "Desktop" at 40, 40
    When the person presses "Right"
    Then the file "Main.qml" says "id: a; x: 20; y: 20;"
    And the file "Main.qml" says "x: 21"

  Scenario: A handle resize while a state is picked goes into that state
    Given the person has selected "Rectangle root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    And the person has clicked the frame "Desktop" at 40, 40
    When the person drags in the frame "Desktop" from 70, 70 to 110, 90
    Then the file "Main.qml" says "id: a; x: 20; y: 20; width: 50; height: 50;"
    And the file "Main.qml" says "target: a"
    And the file "Main.qml" says "width: 90"
    And the file "Main.qml" says "height: 70"

  Scenario: An align while a state is picked goes into that state
    Given the person has selected "Rectangle root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    And an agent has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}]}
      """
    When the person clicks the button "Align Top"
    Then the file "Main.qml" says "id: b; x: 200; y: 100;"
    And the file "Main.qml" says "target: b"
    And the file "Main.qml" says "y: 20"

  Scenario: A refused place into a state is said on the window's line
    Given the person has selected "Rectangle root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    When the person replaces "id: a; " with "" in the code view
    Given the person has clicked the frame "Desktop" at 40, 40
    When the person presses "Right"
    Then the window shows a refusal that says "id"
    And the file "Main.qml" does not say "target:"

  Scenario: What a state could not do is not said once another object is selected
    Given the person has selected "Rectangle root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    And the person has clicked the frame "Desktop" at 50, 220
    And the person has dragged in the frame "Desktop" from 50, 220 to 160, 220
    Then the window says "A state cannot change the order of a layout"
    When the person selects "Rectangle a" in the layers panel
    Then the window does not say "A state cannot change the order of a layout"

  Scenario: What a state could not do is not said once the state is done
    Given the person has selected "Rectangle root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    And the person has clicked the frame "Desktop" at 50, 220
    And the person has dragged in the frame "Desktop" from 50, 220 to 160, 220
    Then the window says "A state cannot change the order of a layout"
    When the person clicks the button "Done editing state"
    Then the window does not say "A state cannot change the order of a layout"

  Scenario: A state cannot change the order of a layout
    Given the person has selected "Rectangle root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    And the person has clicked the frame "Desktop" at 50, 220
    When the person drags in the frame "Desktop" from 50, 220 to 160, 220
    Then the window says "A state cannot change the order of a layout"
    And the file "Main.qml" says "id: one;"
    And the file "Main.qml" does not say "target: one"

  Scenario: A multiple selection is wrapped from the bar above the canvas
    Given an agent has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}]}
      """
    When the person clicks the button "Wrap in Column"
    Then the file "Main.qml" says "Column {"
    And the layers panel lists "Rectangle root, Column column, Rectangle a, Rectangle b, RowLayout row, Rectangle one, Rectangle two"

  Scenario: Objects are gathered with Shift on the canvas and wrapped together
    When the person clicks the frame "Desktop" at 30, 30
    And the person clicks the frame "Desktop" at 210, 110 holding Shift
    And the person clicks the button "Wrap in Row"
    Then the layers panel lists "Rectangle root, Row row1, Rectangle a, Rectangle b, RowLayout row, Rectangle one, Rectangle two"

  Scenario: Objects are gathered with Shift in the layers panel and wrapped together
    Given the person has selected "Rectangle a" in the layers panel
    When the person selects "Rectangle b" in the layers panel holding Shift
    And the person clicks the button "Wrap in Row"
    Then the layers panel lists "Rectangle root, Row row1, Rectangle a, Rectangle b, RowLayout row, Rectangle one, Rectangle two"

  Scenario: What only holds its children is taken away from around them
    Given an agent has called the tool "node_arrange" with:
      """json
      {"action": "wrap", "doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}], "in": "Item"}
      """
    When the person selects "Item item" in the layers panel
    And the person clicks the button "Unwrap"
    Then the file "Main.qml" says "id: a; x: 20; y: 20;"
