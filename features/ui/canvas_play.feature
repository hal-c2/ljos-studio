# Sources:
#   docs/DESIGN.md (S3: canvas of frames; S6: the preview plays the flow)
#   features/preview/input.feature (scroll, swipe, press, type; fit content)
#   features/ui/canvas_view.feature (the wheel and a drag move the view)
#   features/ui/flows.feature (in play mode a click is the app's)

@ui @preview
Feature: A long page shows whole on the canvas and plays as it would on the device
  A frame is as tall as its target's screen, or with "Fit content" as
  tall as what the document lays out, so a page that scrolls shows below
  the fold and what is there can be picked. In play mode the frame is the
  device: the wheel over it and a drag in it scroll the app, not the
  canvas, and keys pressed and text typed reach the app, so a form can be
  filled in. Turns of the wheel that come while one is going in reach the
  app together, and none waits for the page to stop. Each is a
  `preview_open` or `preview_state` call an agent could make.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root

          Flickable {
              id: page
              anchors.fill: parent
              contentHeight: 2000

              Rectangle {
                  id: footer
                  y: 1800
                  width: 400
                  height: 200
                  color: "red"
              }
          }

          TextInput {
              id: field
              width: 200
              height: 40
          }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: Fit content makes the frame as tall as the page
    When the person clicks the button "Fit content"
    And the canvas has finished moving
    Then the canvas has a frame "Desktop" of 1280 by 2000
    And the button "Fit content" is on
    And the canvas labels the frames "Desktop · 1280 × 2000" from left to right
    And the frame "Desktop" fits in the canvas

  Scenario: What is below the fold is picked once the frame fits the page
    Given the person has clicked the button "Fit content"
    And the canvas has finished moving
    When the person clicks the frame "Desktop" at 100, 1900
    Then the layers panel has "Rectangle footer" selected

  Scenario: Fit content again gives the frame its screen's height back
    Given the person has clicked the button "Fit content"
    And the canvas has finished moving
    When the person clicks the button "Fit content"
    Then the canvas has a frame "Desktop" of 1280 by 800
    And the button "Fit content" is off

  Scenario: In play mode the wheel over the frame scrolls the app, not the canvas
    Given the person has clicked the button "Play"
    And the person has noted where the frame "Desktop" is
    When the person turns the wheel down over the frame "Desktop" at 600, 400
    Then the running app's "page" has "contentY" above 0
    And the frame "Desktop" is back where it was

  Scenario: In play mode turns of the wheel don't wait for the page to stop
    Given the person has clicked the button "Play"
    And the preview takes 3000 ms over each turn of the wheel
    When the person turns the wheel down over the frame "Desktop" at 600, 400 ten times
    Then the running app was given the 10 turns in at most 2 scrolls
    And no scroll waited for the page to stop
    And the running app's "page" has "contentY" above 0

  Scenario: In play mode turns of the wheel at two points reach the app at each
    Given the person has clicked the button "Play"
    When the person turns the wheel down over the frame "Desktop" at 600, 400 and at once at 200, 400
    Then the running app was given a scroll at 600, 400 and one at 200, 400

  Scenario: In play mode a drag in the frame flicks the app
    Given the person has clicked the button "Play"
    And the person has noted where the frame "Desktop" is
    When the person drags in the frame "Desktop" from 600, 700 to 600, 300
    Then the running app's "page" has "contentY" above 0
    And the frame "Desktop" is back where it was

  Scenario: In play mode text typed reaches the field clicked
    Given the person has clicked the button "Play"
    And the person has clicked the frame "Desktop" at 20, 20
    When the person types the text "hello"
    Then the running app's "field" has "text" "hello"

  Scenario: In play mode a key the canvas has a shortcut for is typed into the app
    Given the person has clicked the button "Play"
    And the person has clicked the frame "Desktop" at 20, 20
    When the person presses "Shift+!"
    Then the running app's "field" has "text" "!"

  Scenario: In play mode a key pressed reaches the app
    Given the person has clicked the button "Play"
    And the person has clicked the frame "Desktop" at 20, 20
    And the person has typed the text "hello"
    When the person presses "Backspace"
    Then the running app's "field" has "text" "hell"

  Scenario: Out of play mode the wheel over the frame still moves the canvas
    Given the person has noted where the frame "Desktop" is
    When the person turns the wheel down over the frame "Desktop" at 600, 400
    Then the frame "Desktop" has moved
    And the running app's "page" has "contentY" 0
