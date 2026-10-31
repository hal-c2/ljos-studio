# Sources:
#   docs/DESIGN.md (the window's export panel)
#   features/ui/export.feature

@ui
Feature: The export panel writes the entry it says
  Every tool's answer brings a new list of documents. The entry on screen and
  the entry written stay the one the person chose or the panel offered.

  Scenario: The entry the person chooses is the one written, whatever answers come back
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Other.qml":
      """qml
      import QtQuick

      Item {}
      """
    And Studio's window shows the project
    And the person has clicked the button "Export"
    When the person chooses the entry "Other.qml" in the export panel
    Then the export panel says "^Entry Other.qml$"
    When the person clicks the button "Write bundle"
    Then the export panel says "^Start it with: ljos run Other.qml --project"
    And the export panel says "^Entry Other.qml$"

  Scenario: With no Main.qml the entry is the document shown, through its dry run
    Given the file "Second.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Third.qml":
      """qml
      import QtQuick

      Item {}
      """
    And Studio's window shows the project
    And the person has opened "Third.qml" in the documents list
    And the person has clicked the button "Export"
    When the person clicks the button "Write bundle"
    Then the export panel says "^Start it with: ljos run Third.qml --project"

  Scenario: A shown document that is gone from disk gives way to the first listed
    Given the file "Second.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Third.qml":
      """qml
      import QtQuick

      Item {}
      """
    And Studio's window shows the project
    And the person has opened "Third.qml" in the documents list
    When the file "Third.qml" is deleted
    And the person clicks the button "Export"
    Then the export panel says "^Entry Second.qml$"
    And the export panel's button "Write bundle" is enabled
