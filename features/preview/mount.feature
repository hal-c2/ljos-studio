# Sources:
#   docs/MCP.md (previews)

@preview
Feature: A program that throws when it starts is told, and the session lives on
  A root whose `Component.onCompleted` throws is a program that compiled and
  does not run. That is the app's fault, not the preview's: the agent is told
  what threw, the picture is not a half-built tree, and the tools go on
  answering. The browser's mount and the native process are different code,
  so this runs on both.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 300; height: 200
          Component.onCompleted: { throw new Error("no start today") }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"

  Scenario: A mount that throws is told and the session lives on
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent calls the tool "preview_events" with:
      """json
      {"since": 0}
      """
    Then the answer's "/events/1/kind" is "log"
    And the answer's "/events/1/message" contains "no start today"
    When the agent calls the tool "document_tree" with:
      """json
      {"doc": "Main.qml"}
      """
    Then the answer's "/doc" is "Main.qml"
