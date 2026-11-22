# Sources:
#   features/preview/problems.feature (what the preview says is wrong)
#   shell/Builtin/Shell.qml (the shell Studio shows with no --shell)
#   the dogfood backlog's P7-1 (errors reach the window and the agent)

@ui @preview @native
Feature: The built-in shell shows what is wrong in the preview
  A person who breaks a binding sees it in the window they already have: a
  badge with how many problems the preview has, and under it the list of
  them, each with where it is. With none there is neither. It is a panel of
  the shell like the others, so a person's copy of the shell may restyle it.

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
    And the window says "warning: `transparent` is not a name in sight: it is looked up when the program runs, App.qml:11"

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

  Scenario: A person's copy of the shell restyles the problems like any panel
    Given Studio's window runs the person's shell, a copy of its own
    And the person's "PreviewProblems.qml" has "\"Problems\"" replaced with "\"Faults\""
    And the window has taken the change
    And the person has opened "App.qml" in the documents list
    When the person replaces "\"black\"" with "transparent" in the code view
    Then the window shows the person's shell
    And the window says "Faults"
    And the window says "Problem count: 1"
