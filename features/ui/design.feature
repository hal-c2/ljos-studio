# Sources:
#   docs/DESIGN.md (S4: tokens, shells and the self-reskin)
#   Ljós design rules (icon-only buttons, one accent, status lines)

@ui
Feature: Studio's window is laid out to work in
  The window is one shell like any other: a bar of icon buttons, panels on
  both sides that fold away and resize, tabs for what shares a side, and a
  canvas that goes on past the frame. Every icon button has a name for a
  reader and a tooltip for the eye.

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

  Scenario: Panels sharing a side are behind tabs
    Then the window has a pane "Layers"
    And the window has a pane "Inspector"
    And the window has no pane "Palette"
    And the window has no pane "Tokens"
    When the person opens the "Insert tab"
    Then the window has a pane "Palette"
    And the window has no pane "Layers"
    When the person opens the "Tokens tab"
    Then the window has a pane "Tokens"
    And the window has no pane "Inspector"

  Scenario: Both sides and the code fold away and come back
    When the person clicks the button "Left panels"
    Then the window has no pane "Layers"
    When the person clicks the button "Right panels"
    Then the window has no pane "Inspector"
    When the person clicks the button "Show code"
    Then the window has no pane "Code"
    When the person clicks the button "Left panels"
    And the person clicks the button "Right panels"
    And the person clicks the button "Show code"
    Then the window has a pane "Layers"
    And the window has a pane "Inspector"
    And the window has a pane "Code"

  Scenario: The look turns dark and back, and the button says which is next
    When the person clicks the button "Dark look"
    Then the window has a button "Light look"
    When the person clicks the button "Light look"
    Then the window has a button "Dark look"

  @preview
  Scenario: The wheel moves over the canvas without changing the frame's size
    Given the person has opened "Main.qml" in the documents list
    When the person turns the wheel over the canvas
    Then the frame "Desktop" has moved
    And the frame "Desktop" has kept its size

  @preview
  Scenario: Zooming keeps what is at the middle of the view there
    Given the person has opened "Main.qml" in the documents list
    When the person zooms in with the button
    Then the middle of the view shows the same place of the frame "Desktop"

  @preview
  Scenario: Zooming a long way keeps what is at the middle of the view there
    Given the person has opened "Main.qml" in the documents list
    When the person zooms in with the button 5 times
    Then the middle of the view shows the same place of the frame "Desktop"

  @preview
  Scenario: An edit leaves the view where the person moved it
    Given the person has opened "Main.qml" in the documents list
    And the person has selected "Rectangle root" in the layers panel
    When the person turns the wheel over the canvas
    And the person sets "width" to "401" in the inspector
    Then the frame "Desktop" has moved

  @preview
  Scenario: The middle button moves the view
    Given the person has opened "Main.qml" in the documents list
    When the person pans the canvas with the middle button
    Then the frame "Desktop" has moved

  @preview
  Scenario: Fit brings back a view moved without zooming
    Given the person has opened "Main.qml" in the documents list
    When the person turns the wheel over the canvas
    And the person clicks the button "Fit"
    Then the frame "Desktop" is back where it was

  @preview
  Scenario: The tool in use stays on when it is clicked again
    Given the person has opened "Main.qml" in the documents list
    When the person clicks the button "Hand"
    And the person clicks the button "Hand"
    Then the button "Hand" is on
    And the button "Select" is off

  @preview
  Scenario: With the hand a drag in the frame moves the view
    Given the person has opened "Main.qml" in the documents list
    And the person has clicked the button "Hand"
    When the person drags with the hand in the frame "Desktop" from 60, 40 by 40, 30
    Then the frame "Desktop" has moved by 40, 30
    And the file "Main.qml" says "x: 20; y: 20"

  @preview
  Scenario: Ctrl and the wheel zoom about the pointer
    Given the person has opened "Main.qml" in the documents list
    When the person zooms in with Ctrl and the wheel over the frame "Desktop" at 60, 40
    Then the frame "Desktop" grew, with its point under the pointer still there

  @preview
  Scenario: With the hand a click in the frame selects nothing
    Given the person has opened "Main.qml" in the documents list
    And the person has clicked the button "Hand"
    When the person clicks the frame "Desktop" at 60, 40
    Then the inspector asks for something to be selected

  Scenario: Nothing selected, the inspector says what it is for
    Then the inspector asks for something to be selected
