# Sources:
#   docs/DESIGN.md (S5: Studio in a browser, as built: the shell on ljos-web)
#   web/Workspace.js (Studio.Workspace's face over the session in wasm)
#   crates/studio_blocks/src/lib.rs (the native block the face follows)
#
# Run by `mise run web:shell` (`web/shell.mjs`): the shell compiled by
# ljos-web in headless Chromium. It needs an ljos-web checkout, its qmlc and
# Qt's QML, so neither ledger runs it.

@web @ui
Feature: Studio's shell in a browser
  Studio's built-in shell runs on ljos-web, compiled from the same QML,
  against the same session built for wasm. Studio.Workspace has a face
  there that does what the native block does: a gesture is a tool call,
  and what the session holds after it is what the shell shows. A person's
  own shell is the native window's (`Ljos.Blocks`' ShellLoader), which
  ljos-web does not have yet.

  Background:
    Given the project in the browser's memory has the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          width: 400
          height: 300

          Rectangle {
              id: card
              width: 120
              height: 80
              color: "steelblue"
          }
      }
      """
    And the shell is open in a page 1440 by 900 pixels

  Scenario: A person opens a document, selects an object and changes it
    Then the documents panel lists "Main.qml"
    When the person clicks "Main.qml" in the documents panel
    Then the layers panel lists "Item root" and "Rectangle card"
    When the person clicks "Rectangle card" in the layers panel
    Then the inspector shows a row for "color"
    When the person types "150" in the inspector's "width" field and presses Enter
    Then "Main.qml" has "width: 150"
    And the window's status is empty
    When the person clicks "Undo"
    Then "Main.qml" has "width: 120"
    And the page logged no errors

  Scenario: A person sets a token in the tokens panel
    When the person types "Spacing.md" in "Filter tokens"
    And the person types "20" in the tokens panel's "Spacing.md" field and presses Enter
    Then "theme.json" has "\"md\": 20"
    And the tokens panel's "Spacing.md" field shows "20"
    And the page logged no errors
