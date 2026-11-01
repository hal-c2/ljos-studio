# Sources:
#   docs/DESIGN.md (section 6: the drag is drawn by Studio, one edit on drop)
#   features/ui/canvas_drag.feature (snapping and guides)

@ui @preview
Feature: Drawing on the canvas and writing text where it is
  A person picks a tool and draws where the object should go, rather than
  choosing from a list and moving it afterwards. Each drawing is one call of
  `node_insert`, one undo step, and a line of QML in the file. Text is written
  on the canvas, over the object it belongs to, and what is typed there is a
  step of its own: one Undo gives back the placeholder, the next the drawing.

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
    Then the file "Main.qml" says "    Rectangle {\n        id: rectangle1\n        x: 30\n        y: 100\n        width: 100\n        height: 60\n"
    And the file "Main.qml" says "// The card."
    And the canvas outlines the selection at 30, 100 sized 100 by 60

  Scenario: The tool goes back to selecting once it has drawn
    Given the person has clicked the button "Draw rectangle"
    And the person has dragged in the frame "Desktop" from 30, 100 to 130, 160
    When the person drags in the frame "Desktop" from 60, 120 to 80, 140
    Then the file "Main.qml" does not say "x: 30"
    And the file "Main.qml" says "width: 100\n        height: 60\n"

  Scenario: A drawing is one undo step
    Given the person has clicked the button "Draw rectangle"
    And the person has dragged in the frame "Desktop" from 30, 100 to 130, 160
    When the person clicks the button "Undo"
    Then the file "Main.qml" does not say "x: 30"

  Scenario: Drawn inside a container, the object belongs to it
    Given the person has clicked the button "Draw rectangle"
    When the person drags in the frame "Desktop" from 220, 120 to 260, 150
    Then the file "Main.qml" says "Rectangle {\n            id: rectangle1\n            x: 20\n            y: 20\n            width: 40\n            height: 30\n"
    And the layers panel has "Rectangle rectangle1" selected

  Scenario: A drawing snaps to what is there and shows the guide
    Given the person has clicked the button "Draw rectangle"
    When the person presses in the frame "Desktop" at 30, 100 and holds it at 197, 160
    Then the canvas shows a x guide at 200

  Scenario: A drawing started just off an edge starts on it
    Given the person has clicked the button "Draw rectangle"
    When the person drags in the frame "Desktop" from 3, 2 to 130, 160
    Then the file "Main.qml" says "x: 0\n        y: 0\n"

  Scenario: A click with the tool places an object of a usable size
    Given the person has clicked the button "Draw rectangle"
    When the person clicks the frame "Desktop" at 30, 100
    Then the file "Main.qml" says "x: 30\n        y: 100\n        width: 100\n        height: 100\n"

  Scenario: Escape puts the tool down without writing
    Given the person has clicked the frame "Desktop" at 300, 20
    And the person has clicked the button "Draw rectangle"
    When the person presses "Escape"
    And the person drags in the frame "Desktop" from 30, 100 to 130, 160
    Then the file "Main.qml" does not say "x: 30"

  Scenario: Text drawn is written straight away
    Given the person has clicked the button "Draw text"
    And the person has dragged in the frame "Desktop" from 30, 100 to 130, 130
    When the person writes "Welcome" on the canvas and presses "Return"
    Then the file "Main.qml" says "x: 30\n        y: 100\n        width: 100\n        text: \"Welcome\"\n"

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

  Scenario: A letter picks the tool while the canvas has the keys
    Given the person has clicked the frame "Desktop" at 300, 20
    When the person presses "R"
    And the person drags in the frame "Desktop" from 30, 100 to 130, 160
    Then the file "Main.qml" says "x: 30\n        y: 100\n        width: 100\n        height: 60\n"

  Scenario: V puts the tool down again
    Given the person has clicked the frame "Desktop" at 300, 20
    And the person has pressed "R"
    When the person presses "V"
    And the person drags in the frame "Desktop" from 30, 100 to 130, 160
    Then the file "Main.qml" does not say "x: 30"

  Scenario: Holding space moves the view, and letting go gives back the tool
    Given the person has clicked the frame "Desktop" at 300, 20
    When the person holds "Space" and drags in the frame "Desktop" from 60, 40 to 100, 70
    Then the frame "Desktop" has moved
    And the file "Main.qml" does not say "x: 340"
    And the button "Select" is on

  Scenario: A letter written in a field is not a tool
    Given the person has double-clicked the frame "Desktop" at 25, 25
    When the person types "R" on the canvas
    Then the button "Select" is on

  Scenario: With the hand a drag moves the view
    Given the person has clicked the button "Hand"
    When the person holds "Alt" and drags in the frame "Desktop" from 60, 40 to 100, 70
    Then the frame "Desktop" has moved

  Scenario: After the hand a letter still picks the tool
    Given the person has clicked the frame "Desktop" at 300, 20
    When the person presses "H"
    And the person presses "R"
    And the person drags in the frame "Desktop" from 30, 100 to 130, 160
    Then the file "Main.qml" says "x: 30\n        y: 100\n        width: 100\n        height: 60\n"

  Scenario: After the Hand button a letter still picks the tool
    Given the person has clicked the button "Hand"
    When the person presses "R"
    And the person drags in the frame "Desktop" from 30, 100 to 130, 160
    Then the file "Main.qml" says "x: 30\n        y: 100\n        width: 100\n        height: 60\n"

  Scenario: Text in any of QML's quotes is written on
    Given an agent has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "title"}, "name": "text", "value": "'say \"hi\"\\x21'"}
      """
    When the person double-clicks the frame "Desktop" at 25, 25
    Then the canvas's text holds "say \"hi\"!"
    When the person writes "Changed" on the canvas and presses "Return"
    Then the file "Main.qml" says "text: \"Changed\""

  Scenario: A template is told to go to the inspector
    Given an agent has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "title"}, "name": "text", "value": "`at ${width}`"}
      """
    When the person double-clicks the frame "Desktop" at 25, 25
    Then the canvas has no text being written
    And the window says "This text is an expression: change it in the inspector"

  Scenario: Text being written is kept when the selection moves
    Given the person has double-clicked the frame "Desktop" at 25, 25
    And the person has typed "Moved" on the canvas
    When the person selects "Rectangle card" in the layers panel
    Then the canvas has no text being written
    And the file "Main.qml" says "id: title; x: 20; y: 20; text: \"Moved\""

  Scenario: Text being written goes to its own document when the document changes
    Given the file "Other.qml":
      """qml
      import QtQuick

      Text { id: title; text: "Other" }
      """
    And the person has double-clicked the frame "Desktop" at 25, 25
    And the person has typed "Moved" on the canvas
    When the person opens "Other.qml" in the documents list
    Then the file "Main.qml" says "text: \"Moved\""
    And the file "Other.qml" says "text: \"Other\""

  Scenario: A drawn text that was refused leaves nothing to write on the next object
    Given the project's folder takes no writes
    And the person has clicked the button "Draw text"
    When the person drags in the frame "Desktop" from 30, 100 to 130, 130
    Then the window shows a refusal that says "Permission denied"
    When the project's folder takes writes again
    And the person places "Rectangle" from the palette
    Then the canvas has no text being written
