# Sources:
#   studio7 dogfood backlog P7-2 (the inspector moves under the cursor, so a
#   typed value lands in the wrong field)

@ui
Feature: The inspector holds still while a person types into it
  A person edits one field after another without looking back at the list.
  Nothing an edit says moves a field: a refusal shows under the field that
  caused it, in a line that field keeps for it, a token's refusal under the
  list of tokens, and a change recorded into a state goes in below the row
  that adds it, which is emptied for the next.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 100
          height: 50
          color: "#ff0000"
          opacity: 1
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has selected "Rectangle root" in the layers panel

  Scenario: A size refused is said under its field, and the rows below stay put
    Given the person has noted where the inspector's "color" is
    When the person sets "width" to "cube" in the inspector
    Then the inspector says "cube is not a literal: bind it as an expression" under its "width"
    And the inspector's "color" is where it was
    And the file "Main.qml" says "width: 100"

  Scenario: A refusal and what it offers end above the row below
    When the person sets "color" to "cube" in the inspector
    Then the inspector's refusal "cube is not a literal: bind it as an expression" and its offers end above the row under its "color"

  Scenario: A property refused is said under its row, and the row below stays put
    Given the person has noted where the inspector's "opacity" is
    When the person sets "color" to "cube" in the inspector
    Then the inspector says "cube is not a literal: bind it as an expression" under its "color"
    And the inspector's "opacity" is where it was

  Scenario: What the file refuses is said under the row that asked for it
    Given the person has noted where the inspector's "New value" is
    When the person sets "nosuch" to "3" in the inspector
    Then the inspector says "a `Rectangle` has no `nosuch`" under its "New value"
    And the inspector's "New value" is where it was

  Scenario: The id refused stays under the id, and the next row stays put
    Given the person has noted where the inspector's "color" is
    When the person sets "id" to "2root" in the inspector
    Then the inspector's "color" is where it was
    And the file "Main.qml" says "id: root"

  Scenario: A change recorded into a state empties the row that adds it, which stays put
    Given the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    And the person has noted where the inspector's "Change value" is
    When the person sets "Change target" to "root" in the inspector
    And the person sets "Change property" to "height" in the inspector
    And the person sets "Change value" to "80" in the inspector
    Then the file "Main.qml" says "height: 80"
    And the inspector's "Change target" holds ""
    And the inspector's "Change property" holds ""
    And the inspector's "Change value" holds ""
    And the inspector's "Change value" is where it was

  Scenario: A token refused leaves the list of tokens where it was
    Given the person has noted where the tokens panel's "Colors.text light" is
    When the person sets "Colors.text light" to "#f0f0f0" in the tokens panel
    Then the tokens panel says "Colors.text on canvas is 1.\d+:1 in light"
    And the tokens panel's "Colors.text light" is where it was
