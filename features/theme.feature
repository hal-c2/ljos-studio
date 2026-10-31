# Sources:
#   docs/DESIGN.md (S4: tokens, shells and the self-reskin)
#   docs/MCP.md (#22 theme_set; the restyle prompt)
#   ljos-ui docs/ARCHITECTURE.md section 12.1 (the theme file) and
#   features/design/theme.feature (what a theme may say; Ljós's messages)

Feature: Tokens are read and set in the project's theme file
  `theme_set` with no value reads the tokens: every role of `Ljos.Design`
  with the module's own value and the theme file's, light and dark and
  high contrast. With a token and a value it writes the theme file Ljós
  reads for the entry, the nearest `theme.json` from the entry's directory
  up to the project's, creating the project's own when there is none (the
  one Ljós watches then), and leaves the rest of the file as it was, byte
  for byte. Ljós's own theme reader and contrast
  checker judge the result, and a theme Ljós would refuse is refused with
  Ljós's message, so a theme Studio writes always loads. A write is an
  edit like any other: it has a generation, a dry run, a history entry
  and an undo.

  Background:
    Given the file "app/Main.qml":
      """qml
      import QtQuick
      import Ljos.Design

      Rectangle {
          color: Colors.accent
      }
      """
    And the studio tools are connected

  Rule: With no value the tokens are read

    Scenario: A token reads the module's value when the theme has none
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Spacing.md"}
        """
      Then the answer's "/file" is "theme.json"
      And the answer's "/exists" is false
      And the answer's "/tokens" has 1 entries
      And the answer's "/tokens/0/token" is "Spacing.md"
      And the answer's "/unchanged" is absent
      And the answer's "/tokens/0/kind" is "int"
      And the answer's "/tokens/0/module" is 16
      And the answer's "/tokens/0/theme" is absent

    Scenario: A colour reads light and dark, in normal and high contrast
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Colors.accent"}
        """
      Then the answer's "/tokens/0/kind" is "color"
      And the answer's "/tokens/0/module/light" is "#007cb7"
      And the answer's "/tokens/0/module/lightHigh" is "#005e8c"
      And the answer's "/tokens/0/module/dark" is "#007cb7"

    Scenario: A singleton reads all of its roles
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Colors"}
        """
      Then the answer's "/tokens" has 28 entries
      And the answer's "/tokens/0/token" is "Colors.seedHue"
      And the answer's "/tokens/1/token" is "Colors.canvas"

    Scenario: The theme file's value is read beside the module's
      Given the file "app/theme.json":
        """json
        { "Colors": { "accent": { "light": "#6a1b9a", "dark": "#9c4dc4" } }, "Spacing": { "md": 20 } }
        """
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Colors.accent"}
        """
      Then the answer's "/exists" is true
      And the answer's "/tokens/0/theme" is {"light": "#6a1b9a", "dark": "#9c4dc4"}
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Spacing.md"}
        """
      Then the answer's "/tokens/0/theme" is 20
      And the answer's "/tokens/0/module" is 16

    Scenario: A theme file nearer the project's root is the one Ljós reads
      Given the file "theme.json":
        """json
        { "Spacing": { "md": 20 } }
        """
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Spacing.md"}
        """
      Then the answer's "/file" is "theme.json"
      And the answer's "/tokens/0/theme" is 20
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Spacing.md", "value": 24}
        """
      Then the file "theme.json" is:
        """json
        { "Spacing": { "md": 24 } }
        """
      And the file "app/theme.json" does not exist

    Scenario: The nearest theme file wins over the project's own
      Given the file "theme.json":
        """json
        { "Spacing": { "md": 20 } }
        """
      And the file "app/theme.json":
        """json
        { "Spacing": { "md": 12 } }
        """
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Spacing.md"}
        """
      Then the answer's "/file" is "app/theme.json"
      And the answer's "/tokens/0/theme" is 12
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Spacing.md", "value": 24}
        """
      Then the file "app/theme.json" is:
        """json
        { "Spacing": { "md": 24 } }
        """
      And the file "theme.json" is:
        """json
        { "Spacing": { "md": 20 } }
        """

    Scenario: A token the module does not have is refused
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Colors.acent"}
        """
      Then the agent is refused with "not_found"

  Rule: A token and a value write the theme file

    Scenario: With no theme file the project's own is created, the one Ljós watches
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Colors.accent", "value": "#9c4dc4"}
        """
      Then the answer's "/doc" is "theme.json"
      And the answer's "/generation" is 1
      And the answer's "/unchanged" is false
      And the file "app/theme.json" does not exist
      And the file "theme.json" is:
        """json
        {
          "Colors": {
            "accent": "#9c4dc4"
          }
        }
        """

    Scenario: A value is set and the rest of the file stays byte for byte
      Given the file "app/theme.json":
        """json
        {
            "Colors":   { "seedHue": 30, "accent": "#6a1b9a" },

            "Typography": {"bodySize": 15}
        }
        """
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Colors.accent", "value": "#9c4dc4"}
        """
      Then the answer's "/changes" has 1 entries
      And the file "app/theme.json" is:
        """json
        {
            "Colors":   { "seedHue": 30, "accent": "#9c4dc4" },

            "Typography": {"bodySize": 15}
        }
        """

    Scenario: A value the theme file already has is said so and leaves no entry
      Given the file "app/theme.json":
        """json
        { "Colors": { "accent": "#9c4dc4" } }
        """
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Colors.accent", "value": "#9c4dc4"}
        """
      Then the answer's "/unchanged" is true
      And the answer's "/generation" is 1
      And the answer's "/changes" has 0 entries
      And the answer's "/history" is absent
      When the agent calls the tool "history" with:
        """json
        {"action": "list"}
        """
      Then the answer's "/done" has 0 entries

    Scenario: A role and a section the file lacks are added in its layout
      Given the file "app/theme.json":
        """json
        {
            "Colors": {
                "accent": "#9c4dc4"
            }
        }
        """
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Colors.scrim", "value": "#80000000"}
        """
      And the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Spacing.md", "value": 20}
        """
      Then the file "app/theme.json" is:
        """json
        {
            "Colors": {
                "accent": "#9c4dc4",
                "scrim": "#80000000"
            },
            "Spacing": {
                "md": 20
            }
        }
        """

    Scenario: A variant turns one colour into light and dark
      Given the file "app/theme.json":
        """json
        { "Colors": { "accent": "#9c4dc4" } }
        """
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Colors.accent", "variant": "light", "value": "#6a1b9a"}
        """
      Then the file "app/theme.json" is:
        """json
        { "Colors": { "accent": { "light": "#6a1b9a", "dark": "#9c4dc4" } } }
        """

    Scenario: A null value gives the role back to the module
      Given the file "app/theme.json":
        """json
        { "Colors": { "seedHue": 30, "accent": "#6a1b9a" } }
        """
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Colors.accent", "value": null}
        """
      Then the file "app/theme.json" is:
        """json
        { "Colors": { "seedHue": 30 } }
        """

    Scenario Outline: A theme Ljós would refuse is refused with Ljós's message
      Given the file "app/theme.json":
        """json
        { "Colors": { "accent": "#9c4dc4" } }
        """
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "<token>", "value": <value>}
        """
      Then the agent is refused with "invalid_argument"
      And the refusal's "/details/kind" is "theme"
      And the refusal's message matches "<message>"
      And the file "app/theme.json" is:
        """json
        { "Colors": { "accent": "#9c4dc4" } }
        """

      Examples:
        | token        | value      | message                                                          |
        | Colors.text  | "#f0f0f0"  | app/theme.json:1:[0-9]+: Colors.text on canvas is 1.[0-9]+:1 in light   |
        | Colors.text  | 12         | app/theme.json:1:[0-9]+: Colors.text is a colour                     |
        | Colors.text  | "#12345z"  | app/theme.json:1:[0-9]+: Colors.text: .#12345z. is no colour         |
        | Colors.dark  | true       | app/theme.json:1:[0-9]+: Colors.dark is the user's to choose         |

    Scenario: A dry run says what would be written and writes nothing
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Spacing.md", "value": 20, "dry_run": true}
        """
      Then the answer's "/history" is absent
      And the answer's "/text" is:
        """json
        {
          "Spacing": {
            "md": 20
          }
        }
        """
      And the file "theme.json" does not exist

    Scenario: A write against an old generation is refused
      Given the file "app/theme.json":
        """json
        { "Colors": { "accent": "#9c4dc4" } }
        """
      And the agent has called the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Spacing.md", "value": 20}
        """
      When the agent calls the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Spacing.md", "value": 24, "generation": 0}
        """
      Then the agent is refused with "stale_ref"

    Scenario: Undo of the write that made the theme file removes it
      Given the agent has called the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Colors.accent", "value": "#9c4dc4"}
        """
      When the agent calls the tool "history" with:
        """json
        {"action": "undo"}
        """
      Then the file "theme.json" does not exist

    Scenario: Undo gives the theme file back byte for byte
      Given the file "app/theme.json":
        """json
        {"Colors": {"accent": "#9c4dc4"}}
        """
      And the agent has called the tool "theme_set" with:
        """json
        {"doc": "app/Main.qml", "token": "Spacing.md", "value": 20}
        """
      When the agent calls the tool "history" with:
        """json
        {"action": "undo"}
        """
      Then the answer's "/done" has 0 entries
      And the file "app/theme.json" is:
        """json
        {"Colors": {"accent": "#9c4dc4"}}
        """
