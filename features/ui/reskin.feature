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
  person's shell back. An edit of a shell that runs keeps what it made:
  one that does not compile keeps the shell, and the banner says why.

  Background:
    Given Studio's window runs the person's shell, a copy of its own
    And the person has opened "Shell.qml" in the documents list

  Scenario: The window starts on the person's shell
    Then the window shows the person's shell
    And the window has no shell banner

  Scenario: A token edit reskins the running window
    When the person sets "Colors.canvas light" to "#ffffff" in the tokens panel
    And the window takes the change
    Then the window's canvas is "#ffffff"
    And the window shows the person's shell
    And the window has no shell banner

  Scenario: A QML edit reskins the running window
    Given the person has opened "DocumentsPanel.qml" in the documents list
    When the person replaces "title: \"Documents\"" with "title: \"Files\"" in the code view
    And the window takes the change
    Then the window has a pane "Files"
    And the window shows the person's shell

  Scenario: A shell that throws when the window opens falls back, and mending it brings it back
    Given the person's "Shell.qml" has "id: shell" replaced with "id: shell\n    Component.onCompleted: { throw new Error(\"no shell today\") }"
    When Studio's window opens again
    Then the window shows the built-in shell
    And the shell banner says "Shell\.qml:\d+: .*no shell today"
    Given the person has opened "Shell.qml" in the documents list
    When the person replaces "\n    Component.onCompleted: { throw new Error(\"no shell today\") }" with "" in the code view
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

  Scenario: A theme that does not load says why, and mending it takes the banner away
    Given the file "theme.json":
      """
      {"Colors": {"noSuchRole": "#ff0000"}}
      """
    When the window takes the change
    Then the window shows the person's shell
    And the shell banner says "theme\.json:1:\d+: .*noSuchRole"
    Given the file "theme.json":
      """
      {"Colors": {"seedHue": 30}}
      """
    When the window takes the change
    Then the window has no shell banner

  Scenario: A theme that does not load when the window opens is told at once
    Given the file "theme.json":
      """
      {"Colors": {"noSuchRole": "#ff0000"}}
      """
    When Studio's window opens again
    Then the window shows the person's shell
    And the shell banner says "theme\.json:1:\d+: .*noSuchRole"

  Scenario: A shell edit that does not compile is still told after the theme loads
    When the person replaces "id: shell" with "id: shell\n    NoSuchThing { }" in the code view
    And the window takes the change
    Then the shell banner says "Shell\.qml:\d+:\d+: .*NoSuchThing"
    Given the file "theme.json":
      """
      {"Colors": {"seedHue": 30}}
      """
    When the window takes the change
    Then the shell banner says "Shell\.qml:\d+:\d+: .*NoSuchThing"

  Scenario: A theme still wrong is still told after the shell is mended
    Given the file "theme.json":
      """
      {"Colors": {"noSuchRole": "#ff0000"}}
      """
    And the window has taken the change
    When the person replaces "id: shell" with "id: shell\n    NoSuchThing { }" in the code view
    And the window takes the change
    And the person presses "Ctrl+Z"
    And the window takes the change
    Then the shell banner says "theme\.json:1:\d+: .*noSuchRole"

  Scenario: A watched window nobody touches does no work
    When nobody touches the window for 2 seconds
    Then Studio's window ran no frames
