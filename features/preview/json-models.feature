# Sources:
#   docs/MCP.md (document_new's data, preview_open's state, preview_events)
#   docs/DESIGN.md (S6: data is a model document; phase 3 / M1)
#   features/edit/models.feature (the data as written)
#   ljos-ui features/models/json-list-model.feature (a saved source is read
#   again under `--watch`; JsonListModel is Ljós's own)

@preview
Feature: A list of JSON shows its data in the preview
  Data the project keeps as JSON is read by a `JsonListModel` in a data
  module (`App.Team`). The preview shows a row for each record, a save of
  the file, by Studio or anything else, shows at once without a reload of
  the QML, and the mocked state swaps the file for a fixture the project
  keeps beside its other mocks (`mocks/App/Team`), with the list's text
  the same.

  Background:
    Given the file "App/Team/qmldir":
      """
      module App.Team
      Members 1.0 Members.qml
      """
    And the file "App/Team/Members.qml":
      """qml
      import Ljos.Models

      JsonListModel {
          source: Qt.resolvedUrl("members.json")
      }
      """
    And the file "App/Team/members.json":
      """json
      [{"name": "Ada", "team": "Navy"}, {"name": "Grace", "team": "Navy"}]
      """
    And the file "mocks/App/Team/qmldir":
      """
      module App.Team
      Members 1.0 Members.qml
      """
    And the file "mocks/App/Team/Members.qml":
      """qml
      import Ljos.Models

      JsonListModel {
          source: Qt.resolvedUrl("members.json")
      }
      """
    And the file "mocks/App/Team/members.json":
      """json
      [{"name": "Fixture person", "team": "Nowhere"}]
      """
    And the file "Team.qml":
      """qml
      import QtQuick
      import App.Team

      Column {
          id: root

          Repeater {
              id: members
              model: Members {}
              Text {
                  Accessible.role: Accessible.StaticText
                  text: model.name + ", " + model.team
              }
          }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"

  Scenario: The preview shows a row for each record
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Team.qml"}
      """
    Then the preview shows "Ada, Navy"
    And the preview shows "Grace, Navy"

  Scenario: A save of the JSON shows without a reload
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Team.qml"}
      """
    When the file "App/Team/members.json" is rewritten outside Studio:
      """json
      [{"name": "Ada", "team": "Navy"}, {"name": "Grace", "team": "Navy"}, {"name": "Hopper", "team": "Reserve"}]
      """
    # The preview's own watch reads the file again, after the write returns.
    Then the preview shows "Hopper, Reserve" within 5 seconds
    When the agent calls the tool "preview_events" with:
      """json
      {"since": 1}
      """
    Then the answer's "/events" has 0 entries

  Scenario: The mocked state reads the fixture
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Team.qml", "state": "mocked"}
      """
    Then the answer's "/mocked" lists "App.Team"
    And the preview shows "Fixture person, Nowhere"

  Scenario: A list's roles are the keys of the JSON it reads
    When the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Team.qml", "node": {"id": "members"}}
      """
    Then the answer's "/model/roles" lists "name, team"
    And the answer's "/model/rows" is 2
    And the answer's "/model/doc" is "App/Team/Members.qml"
    And the answer's "/model/source" is "App/Team/members.json"
