# Sources:
#   docs/DOGFOOD.md (P6: a site by hand: a value is found by typing names)
#   features/edit/layers.feature (the same edits over MCP)

@ui
Feature: The inspector says what an object can be given
  A person building a page should not need the documentation open to learn
  that a picture has a `fillMode`, nor how its values are spelled. The inspector
  offers the properties of the selected object's type as a name is typed, and
  the values of an enum as a list. Each choice is the `binding_set` an agent
  calls; `node_inspect` lists the same under `offers`.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          Text {
              id: title
              text: "Hello"
              wrapMode: Text.NoWrap
          }
          Rectangle {
              id: card
              x: 10
              width: 50
          }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: An enum's row offers its values
    Given the person has selected "Text title" in the layers panel
    When the person clicks the button "Choices for wrapMode"
    And the person clicks the button "Choose Text.WordWrap for wrapMode"
    Then the file "Main.qml" says "wrapMode: Text.WordWrap"
    And the history lists "set wrapMode"

  Scenario: A name typed offers the properties it could be
    Given the person has selected "Text title" in the layers panel
    When the person enters "weig" as the new property in the inspector
    And the person clicks the button "Offer font.weight"
    And the person clicks the button "Choices for new property"
    And the person clicks the button "Choose Font.Bold for font.weight"
    Then the file "Main.qml" says "font.weight: Font.Bold"

  Scenario: A value of the property's enum typed by hand needs no asking
    Given the person has selected "Text title" in the layers panel
    When the person sets "font.weight" to "Font.DemiBold" in the inspector
    Then the file "Main.qml" says "font.weight: Font.DemiBold"
    And the window shows no refusal

  Scenario: A group's parts are offered
    Given the person has selected "Rectangle card" in the layers panel
    When the person enters "border" as the new property in the inspector
    Then the inspector has the button "Offer border.color"

  Scenario: An object fills its parent with one press
    Given the person has selected "Rectangle card" in the layers panel
    When the person clicks the button "Fill parent"
    Then the file "Main.qml" says "anchors.fill: parent"
    And the file "Main.qml" does not say "x: 10"
    When the person clicks the button "Fill parent"
    Then the file "Main.qml" does not say "anchors.fill"

  Scenario: With nothing selected nothing is offered
    Then the inspector has no button "Write undefined as text"
