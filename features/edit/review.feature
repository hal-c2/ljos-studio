# Sources:
#   docs/DESIGN.md (goal 1: the sidecar is deletable; S6: review comments)
#   docs/MCP.md (review)
#   crates/studio_core/src/address.rs (handles and keys that survive edits)

Feature: Design review comments sit on objects, in the sidecar
  A reviewer, a person or an agent, comments on an object of a document.
  The comment stays on that object while others edit around it, as a
  handle does, and it is kept in Studio's sidecar (`.studio/`), never in
  the QML: deleting the sidecar loses the comments and nothing of the
  design. A comment whose object is taken out is kept, orphaned, with the
  place it last had.

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
    And the studio tools are connected

  Rule: Comments are added, answered and resolved on an object

    Scenario: A comment on an object
      When the agent calls the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"id": "title"}, "text": "Too small for a heading", "author": "reviewer"}
        """
      Then the answer's "/comment/id" is "c1"
      And the answer's "/comment/anchor" is {"path": "root/0", "line": 5, "type": "Text", "id": "title"}
      And the answer's "/comment/orphaned" is false
      And the answer's "/comment/resolved" is false
      And the answer's "/comment/thread/0/text" is "Too small for a heading"
      And the answer's "/comment/thread/0/author" is "reviewer"
      And the file "Main.qml" is:
        """qml
        import QtQuick

        Column {
            id: root
            Text { id: title; text: "Hello" }
            Rectangle { color: "red" }
        }
        """

    Scenario: A reply and a resolution
      Given the agent has called the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"id": "title"}, "text": "Too small for a heading"}
        """
      And the agent has called the tool "review" with:
        """json
        {"action": "reply", "comment": "c1", "text": "Made it 24", "author": "designer"}
        """
      When the agent calls the tool "review" with:
        """json
        {"action": "resolve", "comment": "c1"}
        """
      Then the answer's "/comment/resolved" is true
      And the answer's "/comment/thread" has 2 entries
      And the answer's "/comment/thread/1/text" is "Made it 24"

    Scenario: A resolved comment is reopened
      Given the agent has called the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"id": "title"}, "text": "Too small"}
        """
      And the agent has called the tool "review" with:
        """json
        {"action": "resolve", "comment": "c1"}
        """
      When the agent calls the tool "review" with:
        """json
        {"action": "reopen", "comment": "c1"}
        """
      Then the answer's "/comment/resolved" is false

    Scenario: Comments are listed by document and by object
      Given the agent has called the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"id": "title"}, "text": "Too small"}
        """
      And the agent has called the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"path": "root/1"}, "text": "Use a token"}
        """
      When the agent calls the tool "review" with:
        """json
        {"action": "list", "doc": "Main.qml", "node": {"path": "root/1"}}
        """
      Then the answer's "/comments" has 1 entries
      And the answer's "/comments/0/thread/0/text" is "Use a token"
      When the agent calls the tool "review" with:
        """json
        {"action": "list", "doc": "Main.qml"}
        """
      Then the answer's "/comments" has 2 entries

    Scenario: A comment names its object by the handle the tree gives it
      Given the agent has called the tool "node_find" with:
        """json
        {"doc": "Main.qml", "type": "Rectangle"}
        """
      And the agent keeps the answer's "/nodes/0/node" as "rect"
      When the agent calls the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"node": "$rect"}, "text": "Use a token"}
        """
      Then the answer's "/comment/node" is "$rect"

  Rule: A comment stays on its object through others' edits

    Scenario: An object inserted above moves the comment's place, not its object
      Given the agent has called the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"path": "root/1"}, "text": "Use a token"}
        """
      And the agent has called the tool "node_insert" with:
        """json
        {"doc": "Main.qml", "parent": {"id": "root"}, "index": 0, "text": "Item {}"}
        """
      When the agent calls the tool "review" with:
        """json
        {"action": "list", "doc": "Main.qml"}
        """
      Then the answer's "/comments/0/anchor" is {"path": "root/2", "line": 7, "type": "Rectangle"}
      And the answer's "/comments/0/orphaned" is false

    Scenario: A comment whose object is removed is kept, orphaned, at its last place
      Given the agent has called the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"path": "root/1"}, "text": "Use a token"}
        """
      And the agent has called the tool "node_remove" with:
        """json
        {"doc": "Main.qml", "node": {"path": "root/1"}}
        """
      When the agent calls the tool "review" with:
        """json
        {"action": "list", "doc": "Main.qml"}
        """
      Then the answer's "/comments/0/orphaned" is true
      And the answer's "/comments/0/anchor" is {"path": "root/1", "line": 6, "type": "Rectangle"}
      And the answer's "/comments/0/node" is absent

    Scenario: Undoing the removal gives the comment its object back
      Given the agent has called the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"path": "root/1"}, "text": "Use a token"}
        """
      And the agent has called the tool "node_remove" with:
        """json
        {"doc": "Main.qml", "node": {"path": "root/1"}}
        """
      And the agent has called the tool "history" with:
        """json
        {"action": "undo"}
        """
      When the agent calls the tool "review" with:
        """json
        {"action": "list", "doc": "Main.qml"}
        """
      Then the answer's "/comments/0/orphaned" is false

    Scenario: A reviewer cannot comment on an orphaned comment's object, only answer it
      Given the agent has called the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"path": "root/1"}, "text": "Use a token"}
        """
      And the agent has called the tool "node_remove" with:
        """json
        {"doc": "Main.qml", "node": {"path": "root/1"}}
        """
      When the agent calls the tool "review" with:
        """json
        {"action": "reply", "comment": "c1", "text": "Removed it instead"}
        """
      Then the answer's "/comment/thread" has 2 entries
      And the answer's "/comment/orphaned" is true

  Rule: Comments live in the sidecar, apart from the design

    Scenario: Comments are written to the sidecar
      When the agent calls the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"id": "title"}, "text": "Too small"}
        """
      Then the file ".studio/review.json" exists

    Scenario: A new session finds the comments on their objects again
      Given the agent has called the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"id": "title"}, "text": "Too small"}
        """
      And the studio tools are reconnected
      When the agent calls the tool "review" with:
        """json
        {"action": "list", "doc": "Main.qml", "node": {"id": "title"}}
        """
      Then the answer's "/comments" has 1 entries
      And the answer's "/comments/0/orphaned" is false

    Scenario: Deleting the sidecar loses the comments and keeps the design
      Given the agent has called the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"id": "title"}, "text": "Too small"}
        """
      And the directory ".studio" is deleted
      And the studio tools are reconnected
      When the agent calls the tool "review" with:
        """json
        {"action": "list", "doc": "Main.qml"}
        """
      Then the answer's "/comments" has 0 entries
      And the file "Main.qml" is:
        """qml
        import QtQuick

        Column {
            id: root
            Text { id: title; text: "Hello" }
            Rectangle { color: "red" }
        }
        """

  Rule: Reading comments needs inspect, writing them mutate

    Scenario: A reader lists comments but cannot add one
      Given the studio tools are connected with the capabilities "inspect"
      When the agent calls the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"id": "title"}, "text": "Too small"}
        """
      Then the agent is refused with "capability_denied"
      And the refusal's "/details/required" is "mutate"
      When the agent calls the tool "review" with:
        """json
        {"action": "list", "doc": "Main.qml"}
        """
      Then the answer's "/comments" has 0 entries

    Scenario: An unknown comment is not found
      When the agent calls the tool "review" with:
        """json
        {"action": "resolve", "comment": "c9"}
        """
      Then the agent is refused with "not_found"

    Scenario: A comment needs text
      When the agent calls the tool "review" with:
        """json
        {"action": "add", "doc": "Main.qml", "node": {"id": "title"}, "text": " "}
        """
      Then the agent is refused with "invalid_argument"
