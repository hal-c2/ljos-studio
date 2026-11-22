# Sources:
#   features/preview/problems.feature (what the preview says is wrong)
#   shell/Builtin/Shell.qml (the shell Studio shows with no --shell)
#   the dogfood backlog's P7-1 (errors reach the window and the agent) and
#   P7-2 (nothing a problem says moves what the person is working on)

@ui @preview @native
Feature: The built-in shell shows what is wrong in the preview
  A person who breaks a binding sees it in the window they already have: a
  badge with how many problems the preview has, at the foot of the left
  panels. A click on it opens the list of them, each with where it is, and
  choosing one shows its object. The list stays open or closed as the person
  left it. With no problems there is neither badge nor list. Half-typed text
  is a problem too, so neither the code nor the frame moves as one comes and
  goes. While the left panels are folded the toolbar has a button to them. It
  is a panel of the shell like the others, so a person's copy of the shell
  may restyle it.

  Background:
    # First in the documents list, also beside a shell's own files.
    Given the file "App.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 200; height: 120

          Rectangle {
              id: card
              width: 80; height: 80
              border.width: 2
              border.color: "black"
          }
      }
      """

  Scenario: A preview with no problems shows no badge
    Given Studio's window shows the project
    When the person opens "App.qml" in the documents list
    Then the window has no pane "No problems in the preview"
    And the window does not say "No problems"
    And the window does not say "Problem count: 0"

  Scenario: A binding broken by hand shows as a badge, then in the list
    Given Studio's window shows the project
    And the person has opened "App.qml" in the documents list
    When the person replaces "\"black\"" with "transparent" in the code view
    Then the window has a pane "1 problem in the preview"
    And the window says "Problem count: 1"
    And the window does not say "warning: `transparent` is not a name in sight: it is looked up when the program runs, App.qml:11"
    When the person clicks "Show the problems"
    Then the window says "warning: `transparent` is not a name in sight: it is looked up when the program runs, App.qml:11"
    When the person clicks "Hide the problems"
    Then the window does not say "warning: `transparent` is not a name in sight: it is looked up when the program runs, App.qml:11"
    And the window says "Problem count: 1"

  Scenario: A problem an agent's edit made shows in the window too
    Given Studio's window shows the project
    And the person has opened "App.qml" in the documents list
    When an agent calls the tool "binding_set" with:
      """json
      {"doc": "App.qml", "node": {"id": "card"}, "name": "border.color", "value": "transparent"}
      """
    Then the window has a pane "1 problem in the preview"
    And the window says "Problem count: 1"

  Scenario: Mending the binding takes the badge and the list away
    Given Studio's window shows the project
    And the person has opened "App.qml" in the documents list
    When the person replaces "\"black\"" with "transparent" in the code view
    Then the window has a pane "1 problem in the preview"
    When the person replaces "border.color: transparent" with "border.color: \"transparent\"" in the code view
    Then the window has no pane "1 problem in the preview"
    And the window has no pane "No problems in the preview"
    And the window does not say "Problem count: 1"

  Scenario: The list stays open for the next problem
    Given Studio's window shows the project
    And the person has opened "App.qml" in the documents list
    When the person replaces "\"black\"" with "transparent" in the code view
    And the person clicks "Show the problems"
    And the person replaces "border.color: transparent" with "border.color: \"transparent\"" in the code view
    Then the window does not say "Hide the problems"
    When the person replaces "border.color: \"transparent\"" with "border.color: transparent" in the code view
    Then the window says "warning: `transparent` is not a name in sight: it is looked up when the program runs, App.qml:11"

  Scenario: A problem that comes and goes moves neither the code nor the frame
    Given Studio's window shows the project
    And the person has opened "App.qml" in the documents list
    And the person has noted where the code view and the frame "Desktop" are
    When the person replaces "\"black\"" with "transparent" in the code view
    Then the window has a pane "1 problem in the preview"
    And the code view and the frame "Desktop" are where they were
    When the person clicks "Show the problems"
    Then the code view and the frame "Desktop" are where they were
    When the person replaces "border.color: transparent" with "border.color: \"transparent\"" in the code view
    Then the window has no pane "1 problem in the preview"
    And the code view and the frame "Desktop" are where they were

  Scenario: Text that doesn't compile shows as an error, and moves nothing
    Given Studio's window shows the project
    And the person has opened "App.qml" in the documents list
    And the person has noted where the code view and the frame "Desktop" are
    When the person replaces "id: card" with "id: card; Item {" in the code view
    Then the window says "Problem count: 1"
    And the code view and the frame "Desktop" are where they were
    When the person clicks "Show the problems"
    Then the window says something starting "error: "
    When the person replaces "id: card; Item {" with "id: card" in the code view
    Then the window does not say "Problem count: 1"
    And the code view and the frame "Desktop" are where they were

  # A file broken before it was opened never had a preview to fail a reload.
  Scenario: A document that doesn't parse when it is opened is an error too
    Given the file "Other.qml":
      """qml
      import QtQuick

      Item {
      """
    And Studio's window shows the project
    When the person opens "Other.qml" in the documents list
    Then the window says "Problem count: 1"
    When the person clicks "Show the problems"
    Then the window says something starting "error: "

  # A lost connection names no file. The copy of the panel is given one: the
  # test can't cut the preview's.
  Scenario: An error with no file doesn't hide that the document doesn't parse
    Given the file "Aside.qml":
      """qml
      import QtQuick

      Item {
      """
    And Studio's window runs the person's shell, a copy of its own
    And the person's "PreviewProblems.qml" has "workspace.problems ?? []" replaced with "[{severity: \"error\", message: \"the connection was lost\", file: \"\"}]"
    And the window has taken the change
    When the person opens "Aside.qml" in the documents list
    Then the window says "Problem count: 2"
    When the person clicks "Show the problems"
    Then the window says "error: the connection was lost"
    And the window says something starting "error: `Aside.qml` does not parse"

  Scenario: Choosing a problem selects its object
    Given Studio's window shows the project
    And the person has opened "App.qml" in the documents list
    And the person has selected "Rectangle root" in the layers panel
    When an agent calls the tool "binding_set" with:
      """json
      {"doc": "App.qml", "node": {"id": "card"}, "name": "border.color", "value": "transparent"}
      """
    Then the layers panel has "Rectangle root" selected
    When the person clicks "Show the problems"
    And the person clicks "warning: `transparent` is not a name in sight: it is looked up when the program runs, App.qml:11"
    Then the window shows the document "App.qml"
    And the layers panel has "Rectangle card" selected

  Scenario: Choosing a problem in another document shows that document
    Given the file "Tile.qml":
      """qml
      import QtQuick

      Rectangle {
          id: face
          width: 80; height: 80

          Rectangle {
              id: mark
              border.width: 2
              border.color: transparent
          }
      }
      """
    And the file "Board.qml":
      """qml
      import QtQuick

      Item {
          width: 200; height: 120

          Tile {}
      }
      """
    And Studio's window shows the project
    And the person has opened "Board.qml" in the documents list
    When the person clicks "Show the problems"
    And the person clicks "warning: `transparent` is not a name in sight: it is looked up when the program runs, Tile.qml:10"
    Then the window shows the document "Tile.qml"
    And the layers panel has "Rectangle mark" selected

  Scenario: A person's copy of the shell restyles the problems like any panel
    Given Studio's window runs the person's shell, a copy of its own
    And the person's "PreviewProblems.qml" has "\"Problems\"" replaced with "\"Faults\""
    And the window has taken the change
    And the person has opened "App.qml" in the documents list
    When the person replaces "\"black\"" with "transparent" in the code view
    Then the window shows the person's shell
    And the window says "Faults"
    And the window says "Problem count: 1"

  Scenario: The problems cover none of the canvas
    Given Studio's window shows the project
    And the person has opened "App.qml" in the documents list
    When the person replaces "\"black\"" with "transparent" in the code view
    Then the window has a pane "1 problem in the preview"
    And the problems cover none of the canvas
    When the person clicks "Show the problems"
    Then the problems cover none of the canvas

  Scenario: With the left panels folded a button still leads to the problems
    Given Studio's window shows the project
    And the person has opened "App.qml" in the documents list
    When the person replaces "\"black\"" with "transparent" in the code view
    And the person clicks "Left panels"
    Then the window has a button "Problems in the left panels"
    When the person clicks "Problems in the left panels"
    And the person clicks "Show the problems"
    Then the window says "warning: `transparent` is not a name in sight: it is looked up when the program runs, App.qml:11"
    When the person clicks "Left panels"
    Then the window has a button "Problems in the left panels"

  # The list keeps the first 1024, and whatever the compiler says. The row
  # for what it dropped is no problem: it isn't counted.
  Scenario: A problem the full list dropped is told in a row of its own
    Given the preview's log starts with 1025 warnings
    And Studio's window shows the project
    And the person has opened "App.qml" in the documents list
    Then the window counts 1024 problems within 10 seconds
    When the person clicks "Show the problems"
    Then the window says "Past the first 1024, only the compiler's problems are added"
    And the window has a pane "1024 problems in the preview"
    And the window says "Problem count: 1024"

  Scenario: A full list that dropped nothing has no such row
    Given the preview's log starts with 1024 warnings
    And Studio's window shows the project
    And the person has opened "App.qml" in the documents list
    Then the window counts 1024 problems within 10 seconds
    When the person clicks "Show the problems"
    Then the window does not say "Past the first 1024, only the compiler's problems are added"
    When the person replaces "id: card" with "id: card; Item {" in the code view
    Then the window counts 1025 problems within 10 seconds
    And the window does not say "Past the first 1024, only the compiler's problems are added"

  # Nothing writes or asks once the window shows the document: the app's own
  # timer logs.
  Scenario: A warning the app logs after it started reaches the window
    Given the file "Late.qml":
      """qml
      import QtQuick

      Item {
          width: 200; height: 120

          Timer {
              interval: 1500
              running: true
              onTriggered: {
                  const gone = null
                  gone.back()
              }
          }
      }
      """
    And Studio's window shows the project
    When the person opens "Late.qml" in the documents list
    Then the window counts 1 problem within 10 seconds
