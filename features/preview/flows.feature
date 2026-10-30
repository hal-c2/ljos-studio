# Sources:
#   docs/MCP.md (preview_state: click, click_at, variant)
#   docs/DESIGN.md (S6: the preview plays the flow)
#   features/edit/flows.feature (the flow as written)
#   ljos-ui docs/ARCHITECTURE.md section 19.4 (the inspector's input tools)

@preview
Feature: The preview plays a prototype's flow
  A flow is the app's own handlers, so the preview plays it by being
  clicked as a person clicks it: `preview_state` clicks an object of the
  document, or a point, through the preview's inspector, and then reads
  the object it names, whose `variant` is the state it is in now (empty
  for the base state).

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          states: [
              State {
                  name: "details"
                  PropertyChanges {
                      target: title
                      text: "Details"
                  }
              }
          ]

          Text {
              id: title
              Accessible.role: Accessible.StaticText
              text: "Overview"
          }

          Rectangle {
              id: card
              y: 100
              width: 200
              height: 100
          }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "state_edit" with:
      """json
      {"doc": "Main.qml", "action": "interaction", "trigger": {"id": "card"}, "state": "details"}
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "target": "phone"}
      """

  Scenario: The preview starts in the base state
    When the agent calls the tool "preview_state" with:
      """json
      {"node": {"id": "root"}}
      """
    Then the answer's "/variant" is ""
    And the answer's "/block" is absent
    And the preview shows "Overview"

  Scenario: A click on the trigger goes to the state
    When the agent calls the tool "preview_state" with:
      """json
      {"node": {"id": "root"}, "click": {"id": "card"}}
      """
    Then the answer's "/variant" is "details"
    And the preview shows "Details"

  Scenario: A click at a point goes to the state
    When the agent calls the tool "preview_state" with:
      """json
      {"node": {"id": "root"}, "click_at": {"x": 50, "y": 150}}
      """
    Then the answer's "/variant" is "details"

  Scenario: A click where no trigger is changes nothing
    When the agent calls the tool "preview_state" with:
      """json
      {"node": {"id": "root"}, "click_at": {"x": 300, "y": 600}}
      """
    Then the answer's "/variant" is ""

  Scenario: An object's state is not set from outside
    When the agent calls the tool "preview_state" with:
      """json
      {"node": {"id": "root"}, "set": {"state": "details"}}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's "/details/kind" is "not_a_block"
