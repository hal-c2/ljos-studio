# Sources:
#   docs/MULTIPLAYER.md (section 5, item 3; section 6, seam 4)
#   docs/MCP.md (node_select's `hold`; the `busy` refusal)

Feature: A writer can hold the objects it is working on for a few seconds
  An agent about to make several edits to one part of a design can hold
  it: `node_select` with `hold` leases the selected objects and all they
  hold to the caller. While the lease lasts, another writer's edit inside
  it is refused as `busy`, naming the holder and until when, and the
  holder's own edits go through. Every call by the holder renews the
  lease for a few more seconds; a holder that stops calling loses it, and
  no lease lasts longer than five minutes. Holding changes what others may
  write, so it needs the `mutate` capability.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root

          Row {
              id: row
              Rectangle { id: card; color: "red" }
          }
          Text { id: label; text: "hello" }
      }
      """
    And the studio tools are connected

  Scenario: Another writer's edit inside a held object is refused, the holder's goes through
    Given a second agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "row"}], "hold": true}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "color", "value": "blue"}
      """
    Then the agent is refused with "busy"
    And the refusal's "/details/kind" is "leased"
    And the refusal's "/details/holder" is "Ada"
    When a second agent named "Ada" calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "color", "value": "green"}
      """
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root

          Row {
              id: row
              Rectangle { id: card; color: "green" }
          }
          Text { id: label; text: "hello" }
      }
      """

  Scenario: An edit outside the held objects goes through
    Given a second agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "row"}], "hold": true}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "bye"}
      """
    Then the answer's "/generation" is 2

  Scenario: A code-view edit of a held object's bytes is refused
    Given a second agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "card"}], "hold": true}
      """
    And the agent has called the tool "document_text" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent keeps the answer's "/generation" as "read"
    # Bytes 100..105 are the card's `"red"`.
    When the agent calls the tool "document_edit" with:
      """json
      {"doc": "Main.qml", "generation": "$read", "start": 100, "end": 105, "text": "\"blue\""}
      """
    Then the agent is refused with "busy"
    And the refusal's "/details/kind" is "leased"

  Scenario: A comment on a held object by another writer is refused
    Given a second agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "row"}], "hold": true}
      """
    When the agent calls the tool "review" with:
      """json
      {"action": "add", "doc": "Main.qml", "node": {"id": "card"}, "text": "Too red?"}
      """
    Then the agent is refused with "busy"
    And the refusal's "/details/kind" is "leased"

  Scenario: Selecting nothing lets go of what the caller holds
    Given a second agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "row"}], "hold": true}
      """
    And a second agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": []}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "color", "value": "blue"}
      """
    Then the answer's "/generation" is 2

  Scenario: Selecting nothing with hold lets go too
    Given the agent has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "row"}], "hold": true}
      """
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [], "hold": true}
      """
    Then the answer's "/held" has 0 entries

  Scenario: Holding needs the mutate capability
    Given the studio tools are connected with the capabilities "inspect"
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "row"}], "hold": true}
      """
    Then the agent is refused with "capability_denied"

  @clock
  Scenario: A lease nobody renews ends after a few seconds
    Given a second agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "row"}], "hold": true}
      """
    When 6 seconds pass with no call
    And the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "color", "value": "blue"}
      """
    Then the answer's "/generation" is 2

  @clock
  Scenario: Every call by the holder renews the lease
    Given a second agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "row"}], "hold": true}
      """
    When 4 seconds pass with no call
    And a second agent named "Ada" calls the tool "document_tree" with:
      """json
      {"doc": "Main.qml"}
      """
    And 4 seconds pass with no call
    And the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "color", "value": "blue"}
      """
    Then the agent is refused with "busy"

  @clock
  Scenario: No lease lasts longer than five minutes
    Given a second agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "row"}], "hold": true}
      """
    When "Ada" calls the tool "document_tree" on "Main.qml" every 4 seconds for 300 seconds
    And the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "color", "value": "blue"}
      """
    Then the answer's "/generation" is 2
    When a second agent named "Ada" calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "row"}], "hold": true}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "lease_spent"
    And the refusal's message matches "release .* and plan again"

  Scenario: Writing the binding an object is written in reaches into its lease
    Given the file "Boxed.qml":
      """qml
      import QtQuick

      Item {
          id: root
          box: Rectangle { id: inner; color: "red" }
      }
      """
    And a second agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Boxed.qml", "nodes": [{"id": "inner"}], "hold": true}
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Boxed.qml", "node": {"id": "root"}, "name": "box", "value": "Item {}"}
      """
    Then the agent is refused with "busy"
    And the refusal's "/details/kind" is "leased"
    And the refusal's "/details/holder" is "Ada"
