# Sources:
#   docs/MCP.md (node_inspect's model, preview_open's state)
#   docs/DESIGN.md (S6: CMS-like content is models; mocked data)
#   features/edit/models.feature (the data as written)
#   https://doc.qt.io/qt-6/qml-qtquick-repeater.html

@preview
Feature: A list shows its data in the preview
  A list's data is a document of a data module (`App.Data`), so the
  preview shows its rows as the app does, an edit to the data shows at
  once, and the mocked state swaps the data for a fixture the project
  keeps beside its other mocks (`mocks/App.Data`), with the list's text
  the same.

  Background:
    Given the file "App/Data/qmldir":
      """
      module App.Data
      Contacts 1.0 Contacts.qml
      """
    And the file "App/Data/Contacts.qml":
      """qml
      import QtQuick

      ListModel {
          ListElement {
              name: "Ada"
          }
          ListElement {
              name: "Grace"
          }
      }
      """
    And the file "mocks/App/Data/qmldir":
      """
      module App.Data
      Contacts 1.0 Contacts.qml
      """
    And the file "mocks/App/Data/Contacts.qml":
      """qml
      import QtQuick

      ListModel {
          ListElement {
              name: "Fixture person"
          }
      }
      """
    And the file "Main.qml":
      """qml
      import QtQuick
      import App.Data

      Column {
          id: root

          Repeater {
              id: people
              model: Contacts {}
              Text {
                  Accessible.role: Accessible.StaticText
                  text: model.name
              }
          }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"

  Scenario: The list's roles come from the data module
    When the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"id": "people"}}
      """
    Then the answer's "/model/roles" lists "name"
    And the answer's "/model/rows" is 2
    And the answer's "/model/doc" is "App/Data/Contacts.qml"

  Scenario: The preview shows a row for each record
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    Then the preview shows "Ada"
    And the preview shows "Grace"

  Scenario: A record added to the data shows at once
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "App/Data/Contacts.qml", "parent": {"path": "root"}, "text": "ListElement { name: \"Hopper\" }", "id": false}
      """
    Then the preview shows "Hopper"

  Scenario: The mocked state shows the fixture
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "state": "mocked"}
      """
    Then the answer's "/mocked" lists "App.Data"
    And the preview shows "Fixture person"
