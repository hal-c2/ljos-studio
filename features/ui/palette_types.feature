# Sources:
#   docs/DOGFOOD.md (D5: every built-in object was typed in the code view)
#   docs/MCP.md (blocks_list, node_insert)

@ui
Feature: A person places the toolkit's types from the palette
  The palette offers what the document imports and the modules most objects
  come from, grouped by module, with a search field over them and the
  project's blocks. Placing a type goes through `node_insert`, which adds a
  missing import, and starts it with enough to be seen.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: A type the document imports is placed with enough to be seen
    Given the person has selected "Item root" in the layers panel
    When the person searches the palette for "Rectangle"
    And the person places "Rectangle" from the palette
    Then the file "Main.qml" says "Rectangle {"
    And the file "Main.qml" says "width: 100"
    And the history lists "insert Rectangle"

  Scenario: A type of a module not imported yet brings its import
    Given the person has selected "Item root" in the layers panel
    When the person searches the palette for "Button"
    And the person places "Button" from the palette
    Then the file "Main.qml" says "import QtQuick.Controls"
    And the file "Main.qml" says "Button {"

  Scenario: With nothing selected a type goes into the document's root
    When the person searches the palette for "Text"
    And the person places "Text" from the palette
    Then the file "Main.qml" says "Text {"

  Scenario: The search field narrows the palette
    When the person searches the palette for "rect"
    Then the palette offers "Rectangle"
    And the palette does not offer "Text"
    When the person searches the palette for ""
    Then the palette offers "Text"
