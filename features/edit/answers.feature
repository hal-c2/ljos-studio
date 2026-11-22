# Sources:
#   docs/MCP.md (addresses, node_arrange, review)
#   $S dogfood backlog P7-19 (MCP naming and answers are uneven)

Feature: Every tool is asked and answers the same way
  An agent that learns one tool should not trip on the next: an object is
  `node` wherever there is one, a refusal for a missing argument says what
  it takes, and an answer names what an agent would ask for next.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          Text { id: footer }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, fs"

  Scenario: One object is `node` in node_arrange too
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "rename", "doc": "Main.qml", "node": {"id": "footer"}, "new_name": "credit"}
      """
    Then the file "Main.qml" contains "Text { id: credit }"

  Scenario: A missing argument is said with the values it takes
    When the agent calls the tool "review" with:
      """json
      {"doc": "Main.qml"}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's message matches "^`review` needs `action`: one of .*add"

  Scenario: A path that is not one says how one reads
    When the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"path": "0"}}
      """
    Then the refusal's message matches "`root/0/1`"

  Scenario: An object put answers with its id as well as its handle
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "text": "Rectangle {}"}
      """
    Then the answer's "/id" is "rectangle1"
