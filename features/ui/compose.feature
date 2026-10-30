# Sources:
#   docs/DESIGN.md (S3 exit: the S1 scenarios pass driven through the UI)
#   features/edit/compose-shell.feature, features/edit/undo.feature (the same
#   flows over MCP; the file text is the oracle for both)

@ui
Feature: A person composes a shell in Studio's window
  Studio's window is a Ljós app whose shell binds Studio's own blocks: a
  documents list, a layers panel, an inspector, a palette of the project's
  blocks and a code view. Every gesture is one of the actions an agent
  calls over MCP, so what a person does leaves the same file text and the
  same history as the agent's calls.

  Background:
    Given the file "App/Media/qmldir":
      """
      module App.Media
      # block Player 1.0 Player.qml Media "Plays one source"
      """
    And the file "App/Media/Player.qml":
      """qml
      import QtQuick

      Item {
          property url source
          readonly property bool playing: false
      }
      """
    And the file "Main.qml":
      """qml
      import QtQuick

      // The window's content.
      Item {
          id: root
          width: 200 // wide enough

          Rectangle { color: "red" }
          Text {
              text: "hello"
          }
      }
      """
    And Studio's window shows the project

  Scenario: Opening a document lists its objects in the layers panel
    When the person opens "Main.qml" in the documents list
    Then the layers panel lists "Item root, Rectangle, Text"
    And the code view shows "Main.qml"

  Scenario: Selecting a layer shows its properties in the inspector
    Given the person has opened "Main.qml" in the documents list
    When the person selects "Rectangle" in the layers panel
    Then the inspector shows "color" as "\"red\""
    And the code view selects "Rectangle { color: \"red\" }"

  Scenario: The inspector sets a property, and undo gives back the bytes
    Given the person has opened "Main.qml" in the documents list
    And the person has selected "Item root" in the layers panel
    When the person sets "width" to "320" in the inspector
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      // The window's content.
      Item {
          id: root
          width: 320 // wide enough

          Rectangle { color: "red" }
          Text {
              text: "hello"
          }
      }
      """
    When the person clicks the button "Undo"
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      // The window's content.
      Item {
          id: root
          width: 200 // wide enough

          Rectangle { color: "red" }
          Text {
              text: "hello"
          }
      }
      """

  Scenario: A binding is set as an expression
    Given the person has opened "Main.qml" in the documents list
    And the person has selected "Rectangle" in the layers panel
    When the person binds "width" to "root.width / 2" in the inspector
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      // The window's content.
      Item {
          id: root
          width: 200 // wide enough

          Rectangle { color: "red"; width: root.width / 2 }
          Text {
              text: "hello"
          }
      }
      """

  Scenario: A block from the palette is placed into the selected object
    Given the person has opened "Main.qml" in the documents list
    And the person has selected "Item root" in the layers panel
    When the person places "Player" from the palette
    Then the layers panel lists "Item root, Rectangle, Text, Player player1"
    And the file "Main.qml" is:
      """qml
      import QtQuick
      import App.Media

      // The window's content.
      Item {
          id: root
          width: 200 // wide enough

          Rectangle { color: "red" }
          Text {
              text: "hello"
          }
          Player { id: player1 }
      }
      """
    And the history lists "insert Player"

  Scenario: A layer is moved up among its siblings
    Given the person has opened "Main.qml" in the documents list
    And the person has selected "Text" in the layers panel
    When the person clicks the button "Move up"
    Then the layers panel lists "Item root, Text, Rectangle"
    And the layers panel has "Text" selected

  Scenario: A layer is removed and put back
    Given the person has opened "Main.qml" in the documents list
    And the person has selected "Rectangle" in the layers panel
    When the person clicks the button "Remove"
    Then the layers panel lists "Item root, Text"
    When the person presses "Ctrl+Z"
    Then the layers panel lists "Item root, Rectangle, Text"

  Scenario: Typing in the code view updates the layers panel
    Given the person has opened "Main.qml" in the documents list
    When the person replaces "Rectangle { color: \"red\" }" with "Rectangle { color: \"red\" }\n    Image {}" in the code view
    Then the layers panel lists "Item root, Rectangle, Image, Text"

  Scenario: An agent's edit shows in the window
    Given the person has opened "Main.qml" in the documents list
    When an agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"path": "root/0"}, "name": "color", "value": "blue"}
      """
    And the person selects "Rectangle" in the layers panel
    Then the inspector shows "color" as "\"blue\""
    And the history lists "set color"
