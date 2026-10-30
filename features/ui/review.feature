# Sources:
#   docs/DESIGN.md (S6: review comments in the sidecar)
#   features/edit/review.feature (the same comments over MCP)

@ui
Feature: A person reviews a design in Studio's window
  A comment shows on its object's layer, and the inspector lists the
  selected object's comments: a person adds one, resolves it or reopens
  it there. Each gesture is the `review` call an agent makes, so a
  comment an agent left shows as a person's does. The design's files do
  not change.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Column {
          id: root
          Text { id: title; text: "Hello" }
          Rectangle { color: "red" }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: A comment is added to the selected object
    Given the person has selected "Text title" in the layers panel
    When the person adds the comment "Too small for a heading" in the inspector
    Then the inspector shows the open comment "Too small for a heading"
    And the layers panel shows "1 comment on Text title"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Column {
          id: root
          Text { id: title; text: "Hello" }
          Rectangle { color: "red" }
      }
      """

  Scenario: An agent's comment is resolved and reopened by a person
    Given an agent has called the tool "review" with:
      """json
      {"action": "add", "doc": "Main.qml", "node": {"id": "title"}, "text": "Too small", "author": "agent"}
      """
    And the person has selected "Text title" in the layers panel
    Then the layers panel shows "1 comment on Text title"
    When the person clicks the button "Resolve Too small"
    Then the inspector shows the resolved comment "Too small"
    And the layers panel shows no comments
    When the person clicks the button "Reopen Too small"
    Then the inspector shows the open comment "Too small"

  Scenario: A comment whose object was removed shows as orphaned
    Given an agent has called the tool "review" with:
      """json
      {"action": "add", "doc": "Main.qml", "node": {"id": "title"}, "text": "Too small"}
      """
    And the person has selected "Rectangle" in the layers panel
    When an agent calls the tool "node_remove" with:
      """json
      {"doc": "Main.qml", "node": {"id": "title"}}
      """
    Then the inspector shows the orphaned comment "Too small"
    And the layers panel shows no comments
