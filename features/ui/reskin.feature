# Sources:
#   docs/DESIGN.md (S4: tokens, shells and the self-reskin)
#   ljos-ui features/blocks/shell-loader.feature (a user's shell, the
#   fallback, and an edit while the application runs) and
#   features/design/theme.feature (an edit of the theme)

@ui
Feature: A person reskins Studio from inside Studio
  Studio's window is a ShellLoader: its source is the person's own shell
  and its fallback the shell Studio ships. The person opens their shell
  (a copy of Studio's own to start with) as a project in Studio and edits
  its tokens or its QML; the window watches the files as any Ljós
  application under `--watch` does and shows each change at once, with
  no restart. A shell that breaks while it is made gives way to the
  built-in one, and a banner says where it broke; mending it brings the
  person's shell back. An edit that does not compile keeps the shell
  that runs, and the banner says why.

  Background:
    Given Studio's window runs the person's shell, a copy of its own
    And the person has opened "Shell.qml" in the documents list

  Scenario: The window starts on the person's shell
    Then the window shows the person's shell
    And the window has no shell banner

  Scenario: A token edit reskins the running window
    When the person sets "Colors.canvas light" to "#fdf6e3" in the tokens panel
    And the window takes the change
    Then the window's canvas is "#fdf6e3"
    And the window shows the person's shell
    And the window has no shell banner

  Scenario: A QML edit reskins the running window
    Given the person has opened "DocumentsPanel.qml" in the documents list
    When the person replaces "title: \"Documents\"" with "title: \"Files\"" in the code view
    And the window takes the change
    Then the window has a pane "Files"
    And the window shows the person's shell

  Scenario: A shell an edit breaks falls back, and mending it brings it back
    When the person replaces "id: shell" with "id: shell\n    Component.onCompleted: { throw new Error(\"no shell today\") }" in the code view
    And the window takes the change
    Then the window shows the built-in shell
    And the shell banner says "Shell\.qml:\d+: .*no shell today"
    When the person presses "Ctrl+Z"
    And the window takes the change
    Then the window shows the person's shell
    And the window has no shell banner

  Scenario: An edit that does not compile keeps the shell that runs
    When the person replaces "id: shell" with "id: shell\n    NoSuchThing { }" in the code view
    And the window takes the change
    Then the window shows the person's shell
    And the shell banner says "Shell\.qml:\d+:\d+: .*NoSuchThing"
    When the person presses "Ctrl+Z"
    And the window takes the change
    Then the window has no shell banner
