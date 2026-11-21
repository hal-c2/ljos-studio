# Sources:
#   docs/DESIGN.md (S3: canvas of frames; idle 0 %)
#   features/ui/design.feature (the wheel, the middle button, the hand, Fit)
#   features/ui/canvas.feature (the frame a target lays out)

@ui @preview
Feature: The canvas is a surface without edges that pans and zooms
  The canvas goes on in every direction. Space and a drag, the middle
  button, the wheel and Shift with the wheel move the view; Ctrl or ⌘ with
  the wheel zooms about the point under the pointer, from 2 % to 6400 %.
  A zoom chosen by a key, a button or the zoom field eases there in the
  time the design's motion gives a frequent move across the screen; the
  wheel and a pinch follow the hand. Whatever the zoom, a click lands on
  what is drawn under it and the selection's handles keep their size on
  the screen. The targets' frames can sit side by side, each labelled;
  clicking one shows the document there.

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
    And the person has opened "Main.qml" in the documents list

  Scenario: Space and a drag move the view by as much as the hand moved
    When the person holds "Space" and drags in the frame "Desktop" from 60, 40 to 100, 70
    Then the frame "Desktop" has moved by 40, 30
    And the frame "Desktop" has kept its size

  Scenario: Shift and the wheel move the view across
    When the person turns the wheel over the canvas holding Shift
    Then the frame "Desktop" has moved across and not down
    And the frame "Desktop" has kept its size

  Scenario: Zooming out with Ctrl and the wheel keeps the point under the pointer
    When the person zooms out with Ctrl and the wheel over the frame "Desktop" at 300, 200
    Then the frame "Desktop" shrank, with its point under the pointer still there

  Scenario: A zoom key eases to its zoom rather than jumping there
    Given the person has noted where the frame "Desktop" is
    When the person presses "Ctrl+1"
    And 40 ms pass on the canvas
    Then the frame "Desktop" is drawn wider than it was and narrower than 1280
    And the canvas, once it has finished moving, draws the frame "Desktop" at 1280 by 800
    And the zoom reads "100%"

  Scenario: Ctrl+0 fits the frame back into the view
    Given the person has pressed "Ctrl+1"
    And the canvas has finished moving
    When the person presses "Ctrl+0"
    And the canvas has finished moving
    Then the frame "Desktop" fits in the canvas

  Scenario: The zoom stops at 2 %
    When the person zooms out with the button 30 times
    And the canvas has finished moving
    Then the zoom reads "2%"

  Scenario: The zoom stops at 6400 %
    When the person zooms in with the button 40 times
    And the canvas has finished moving
    Then the zoom reads "6400%"

  Scenario: The zoom readout takes a zoom typed into it
    When the person enters "250" in the zoom field
    And the canvas has finished moving
    Then the zoom reads "250%"
    And the canvas draws the frame "Desktop" at 3200 by 2000

  Scenario: A click lands on what is drawn under it at any zoom, and the handles keep their size
    Given the person has entered "250" in the zoom field
    And the canvas has finished moving
    When the person clicks the frame "Desktop" at 40, 40
    Then the canvas outlines the selection at 20, 20 sized 100 by 60
    And the canvas shows 8 resize handles
    And the canvas's resize handles are 8 by 8 on the screen

  Scenario: The targets' frames sit side by side, each labelled
    When the person clicks the button "Side by side"
    Then the canvas labels the frames "Desktop · 1280 × 800, Tablet · 820 × 1180, Phone · 390 × 844" from left to right
    And the frames do not overlap

  Scenario: Clicking a frame beside the one shown shows the document there
    Given the person has clicked the button "Side by side"
    When the person clicks the button "Show Tablet"
    Then the canvas has a frame "Tablet" of 820 by 1180
    And the frame "Tablet" shows a picture of the preview
    And the canvas labels the frames "Desktop · 1280 × 800, Tablet · 820 × 1180, Phone · 390 × 844" from left to right
    And the frame beside it for "Desktop" shows the picture it last drew

  Scenario: Shift+1 fits every frame side by side into the view
    Given the person has clicked the button "Side by side"
    And the person has pressed "Ctrl+1"
    And the canvas has finished moving
    When the person presses "Shift+1"
    And the canvas has finished moving
    Then the frames "Desktop, Tablet, Phone" fit in the canvas

  Scenario: A canvas nobody touches does no work once it has finished moving
    Given the person has pressed "Ctrl+1"
    And the canvas has finished moving
    When nobody touches the window for 2 seconds
    Then Studio's window ran no frames
