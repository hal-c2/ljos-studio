# Sources:
#   docs/MCP.md (project_open's assets)

@ui
Feature: The Insert tab offers the project's pictures
  A person finds a picture by looking at it, and places it with one click, by
  the same tool an agent calls.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
      }
      """
    And the file "assets/logo.svg":
      """
      <svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="red"/></svg>
      """
    And the file "assets/badge.svg":
      """
      <svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8" fill="blue"/></svg>
      """
    And Studio's window shows the project

  Scenario: A click places the picture as an Image
    Given the person has opened "Main.qml" in the documents list
    When the person places "logo.svg" from the palette
    Then the file "Main.qml" says "Image {"
    And the file "Main.qml" says "assets/logo.svg"

  Scenario: The search finds a picture by its name
    Given the person has opened "Main.qml" in the documents list
    When the person searches the palette for "bad"
    Then the palette offers "badge.svg"
    And the palette does not offer "logo.svg"

  Scenario: A folder of many pictures shows some, and the search reaches the rest
    Given 30 files the bundle holds
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    Then the palette says "8 more: search for their names"
    When the person searches the palette for "0029"
    Then the palette offers "an-asset-with-a-name-long-enough-to-count-0029.svg"

  Scenario: A file of the project is imported from the panel
    Given the file "incoming/new.svg":
      """
      <svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>
      """
    And the person has opened "Main.qml" in the documents list
    When the person imports "incoming/new.svg" in the palette
    Then the palette offers "new.svg"
    And the file "assets/new.svg" exists
