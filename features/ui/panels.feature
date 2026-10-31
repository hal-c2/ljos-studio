# Sources:
#   docs/DOGFOOD.md (D11: seven rows and no scroll bar; D15: the new-property
#   row keeps the old text)

@ui
Feature: The panels say what they hold and start clean
  A list with more than fits says so with a scroll bar. The inspector's row
  for a new property is emptied once its edit is made, so the next one starts
  clean; an edit that is refused leaves it, to be put right.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root

          Text {
              id: label
              text: "hello"
          }
      }
      """

  Scenario: A set property leaves the row empty
    Given Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has selected "Text label" in the layers panel
    When the person sets "opacity" to "0.5" in the inspector
    Then the file "Main.qml" says "opacity: 0.5"
    And the inspector's new property row holds ""

  Scenario: A refused property stays in the row
    Given Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has selected "Text label" in the layers panel
    When the person sets "opacity" to "{" in the inspector
    Then the inspector's new property row holds "opacity"

  Scenario: A long list of documents has a scroll bar
    Given 40 more documents
    And Studio's window shows the project
    Then the documents panel shows a scroll bar

  Scenario: A short list of documents has none
    Given Studio's window shows the project
    Then the documents panel shows no scroll bar

  Scenario: A long list of documents at rest does no work
    Given 40 more documents
    And Studio's window shows the project
    When nobody touches the window for 2 seconds
    Then Studio's window ran no frames

  # Going back to the preview after a refusal must not ask for ever when the
  # refused call is the reading itself.
  @preview
  Scenario: A preview that cannot say where objects are leaves the window at rest
    Given the preview does not say where its objects are
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list
    And the person has selected "Text label" in the layers panel
    When nobody touches the window for 2 seconds
    Then Studio's window ran no frames
