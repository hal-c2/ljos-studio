# Sources:
#   docs/DESIGN.md (S4: tokens, shells and the self-reskin)
#   features/theme.feature (the same edits over MCP; the file text is the
#   oracle for both)

@ui
Feature: A person reads and sets tokens in Studio's window
  The tokens panel lists every role of `Ljos.Design`: a colour with a
  swatch for light and one for dark, any other role with its value, the
  theme file's where it has one and the module's where it does not.
  Setting one is the `theme_set` call an agent makes, so it writes the
  theme file beside the document shown, and what Ljós refuses is
  refused in the panel with Ljós's message.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      import Ljos.Design

      Rectangle {
          color: Colors.accent
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: The panel shows the module's values while there is no theme
    Then the tokens panel shows "Colors.accent light: #007cb7"
    And the tokens panel shows "Colors.accent dark: #007cb7"
    And the tokens panel shows "Spacing.md: 16"

  Scenario: A token set writes the theme file and shows its value
    When the person sets "Spacing.md" to "20" in the tokens panel
    Then the file "theme.json" is:
      """json
      {
        "Spacing": {
          "md": 20
        }
      }
      """
    And the tokens panel shows "Spacing.md: 20"
    And the history lists "set Spacing.md"

  Scenario: A colour's light and dark are set apart
    When the person sets "Colors.accent light" to "#6a1b9a" in the tokens panel
    And the person sets "Colors.accent dark" to "#9c4dc4" in the tokens panel
    Then the file "theme.json" is:
      """json
      {
        "Colors": {
          "accent": { "light": "#6a1b9a", "dark": "#9c4dc4" }
        }
      }
      """
    And the tokens panel shows "Colors.accent light: #6a1b9a"
    And the tokens panel shows "Colors.accent dark: #9c4dc4"

  Scenario: A token Ljós refuses is refused in the panel with Ljós's message
    When the person sets "Colors.text light" to "#f0f0f0" in the tokens panel
    Then the tokens panel says "theme.json:\d+:\d+: Colors.text on canvas is 1.\d+:1 in light"
    And the file "theme.json" does not exist
    And the tokens panel shows "Colors.text light: #242729"

  Scenario: Undo takes the token back
    Given the person has set "Spacing.md" to "20" in the tokens panel
    When the person presses "Ctrl+Z"
    Then the file "theme.json" does not exist
    And the tokens panel shows "Spacing.md: 16"
