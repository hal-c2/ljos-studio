# Sources:
#   docs/DESIGN.md (S5: Studio in a browser, as built)
#   crates/studio_web/src/store.rs (what a write promises on the web)

@web
Feature: A project kept in the browser's storage
  On the web the project lives in the browser's storage, which answers
  later than a tool does. A call's answer comes once storage has every
  file the call wrote, so an answer that says a file changed is true of
  storage too. A write storage refuses refuses the call that made it,
  naming the file, and goes to storage again with the next call unless a
  newer text of the file came since.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          width: 10
      }
      """
    And the studio tools are connected

  Scenario: What a call wrote is in storage when its answer comes
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "root"}, "name": "width", "value": 20}
      """
    Then the answer's "/doc" is "Main.qml"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 20
      }
      """

  Scenario: A write storage refuses refuses the call, and goes again with the next
    Given storage refuses to keep "Main.qml"
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "root"}, "name": "width", "value": 20}
      """
    Then the agent is refused with "internal"
    And the refusal's "/details/kind" is "not_stored"
    And the refusal's "/details/files/0/path" is "Main.qml"
    And the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 10
      }
      """
    When storage keeps files again
    And the agent calls the tool "document_new" with:
      """json
      {"doc": "Other.qml"}
      """
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          width: 20
      }
      """
    And the file "Other.qml" is:
      """qml
      import QtQuick

      Item {
      }
      """
