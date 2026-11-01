# Sources:
#   docs/DESIGN.md (section 6: the drag is drawn by Studio, one edit on drop)
#   features/ui/canvas_drag.feature (snapping and guides)

@ui @preview
Feature: Drawing on the canvas and writing text where it is
  A person picks a tool and draws where the object should go, rather than
  choosing from a list and moving it afterwards. Each drawing is one call of
  `node_insert`, one undo step, and a line of QML in the file. Text is written
  on the canvas, over the object it belongs to.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 400; height: 300
          color: "white"

          // The card.
          Rectangle { id: card; x: 200; y: 100; width: 150; height: 150; color: "gold" }
          Text { id: title; x: 20; y: 20; text: "Hello" }
          Text { id: live; x: 20; y: 60; text: "n " + width }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has pressed "Ctrl+1"

  Scenario: A rectangle is drawn where the hand dragged
    Given the person has clicked the button "Draw rectangle"
    When the person drags in the frame "Desktop" from 30, 100 to 130, 160
    Then the file "Main.qml" says "x: 30; y: 100; width: 100; height: 60;"
    And the file "Main.qml" says "// The card."
    And the canvas outlines the selection at 30, 100 sized 100 by 60

  Scenario: The tool goes back to selecting once it has drawn
    Given the person has clicked the button "Draw rectangle"
    And the person has dragged in the frame "Desktop" from 30, 100 to 130, 160
    When the person drags in the frame "Desktop" from 60, 120 to 80, 140
    Then the file "Main.qml" does not say "x: 30; y: 100"
    And the file "Main.qml" says "width: 100; height: 60;"

  Scenario: A drawing is one undo step
    Given the person has clicked the button "Draw rectangle"
    And the person has dragged in the frame "Desktop" from 30, 100 to 130, 160
    When the person clicks the button "Undo"
    Then the file "Main.qml" does not say "x: 30; y: 100"

  Scenario: Drawn inside a container, the object belongs to it
    Given the person has clicked the button "Draw rectangle"
    When the person drags in the frame "Desktop" from 220, 120 to 260, 150
    Then the file "Main.qml" says "Rectangle { id: rectangle1; x: 20; y: 20; width: 40; height: 30;"
    And the layers panel has "Rectangle rectangle1" selected

  Scenario: A drawing snaps to what is there and shows the guide
    Given the person has clicked the button "Draw rectangle"
    When the person presses in the frame "Desktop" at 30, 100 and holds it at 197, 160
    Then the canvas shows a x guide at 200

  Scenario: A click with the tool places an object of a usable size
    Given the person has clicked the button "Draw rectangle"
    When the person clicks the frame "Desktop" at 30, 100
    Then the file "Main.qml" says "x: 30; y: 100; width: 100; height: 100;"

  Scenario: Escape puts the tool down without writing
    Given the person has clicked the frame "Desktop" at 300, 20
    And the person has clicked the button "Draw rectangle"
    When the person presses "Escape"
    And the person drags in the frame "Desktop" from 30, 100 to 130, 160
    Then the file "Main.qml" does not say "x: 30; y: 100"

  Scenario: Text drawn is written straight away
    Given the person has clicked the button "Draw text"
    And the person has dragged in the frame "Desktop" from 30, 100 to 130, 130
    When the person writes "Welcome" on the canvas and presses "Return"
    Then the file "Main.qml" says "x: 30; y: 100; width: 100; text: \"Welcome\";"

  Scenario: Double-clicking text writes on it, leaving its line otherwise as it was
    When the person double-clicks the frame "Desktop" at 25, 25
    And the person writes "Hi \"you\"" on the canvas and presses "Return"
    Then the file "Main.qml" says "Text { id: title; x: 20; y: 20; text: \"Hi \\"you\\"\" }"

  Scenario: Escape leaves the text as it was
    When the person double-clicks the frame "Desktop" at 25, 25
    And the person writes "Gone" on the canvas and presses "Escape"
    Then the file "Main.qml" says "text: \"Hello\""

  Scenario: Text that is an expression stays one
    When the person double-clicks the frame "Desktop" at 25, 65
    Then the canvas says "This text is an expression: change it in the inspector"
    And the canvas has no text being written
