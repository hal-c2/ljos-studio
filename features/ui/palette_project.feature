# Sources:
#   docs/DOGFOOD.md (D: a component written in the project was nowhere in the palette)

@ui
Feature: The palette offers the project's own components
  A file beside the document is a type it can use with no import. A person who
  wrote one looks for it where every other type is.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
      }
      """
    And the file "Badge.qml":
      """qml
      import QtQuick

      Rectangle { width: 80; height: 24; color: "gold" }
      """
    And the file "Page.qml":
      """qml
      import QtQuick

      Item {
          id: page
          Text { id: heading; text: "Hi" }
      }
      """
    And Studio's window shows the project

  Scenario: A component of the project is placed like any type
    Given the person has opened "Main.qml" in the documents list
    When the person places "Badge" from the palette
    Then the file "Main.qml" says "Badge {"
    And the file "Main.qml" does not say "import Badge"

  Scenario: The search finds it
    Given the person has opened "Main.qml" in the documents list
    When the person searches the palette for "bad"
    Then the palette offers "Badge"
    And the palette does not offer "Rectangle"

  Scenario: A document does not offer itself
    Given the person has opened "Badge.qml" in the documents list
    Then the palette does not offer "Badge"
    And the palette offers "Rectangle"

  Scenario: A search that finds nothing says so
    Given the person has opened "Main.qml" in the documents list
    When the person searches the palette for "zzz"
    Then the palette says "Nothing called \"zzz\""

  Scenario: Placed with text selected, an object goes beside it
    Given the person has opened "Page.qml" in the documents list
    And the person has selected "Text heading" in the layers panel
    When the person places "Rectangle" from the palette
    Then the file "Page.qml" says "text: \"Hi\" }\n"
