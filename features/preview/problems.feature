# Sources:
#   docs/MCP.md (preview_events; every edit reloads the preview)
#   $S dogfood backlog P7-1 (errors reach the window and the agent) and
#   P7-23 (the preview's warning hides the cause)

@preview @native
Feature: What is wrong in the preview reaches whoever made it wrong
  An edit that breaks a binding used to be told only in `preview_events`,
  which a person never sees and an agent seldom reads, and in words that
  hid the cause ("Unable to assign [undefined] to QColor"). Now the call
  that reloaded the preview answers with its `warnings`, each placed on the
  file, line and object written there, in the compiler's words where it
  said any; the window's problems list shows the same, and both empty as
  the problems are fixed.

  The compiler's warnings come from the preview's own output, which only a
  native preview has: a browser's preview reports its runtime's alone.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          width: 200; height: 120

          Rectangle {
              id: card
              width: 80; height: 80
              border.width: 2
              border.color: "black"
          }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    And the agent has called the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}}
      """
    And the agent keeps the answer's "/node" as "card"

  Scenario: The edit that breaks a binding answers with what broke, where
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "border.color", "value": "transparent"}
      """
    Then the answer's "/warnings" has 1 entry
    And the answer's "/warnings/0/message" contains "`transparent` is not a name in sight"
    And the answer's "/warnings/0/severity" is "warning"
    And the answer's "/warnings/0/file" is "Main.qml"
    And the answer's "/warnings/0/line" is 11
    And the answer's "/warnings/0/node" is "$card"

  Scenario: The cause stays told while the line is still wrong
    Given the agent has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "border.color", "value": "transparent"}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "root"}, "name": "width", "value": 240}
      """
    Then the answer's "/warnings" has 1 entry
    And the answer's "/warnings/0/message" contains "`transparent` is not a name in sight"
    And the answer's "/warnings/0/message" does not contain "Unable to assign"
    When the agent calls the tool "preview_events"
    Then the answer's "/problems" has 1 entry
    And the answer's "/problems/0/line" is 11

  Scenario: A look at the app's rests still says what is wrong
    Given the agent has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "border.color", "value": "transparent"}
      """
    When the agent calls the tool "preview_events" with:
      """json
      {"wait_ms": 0}
      """
    Then the answer's "/problems" has 1 entry
    And the answer's "/problems/0/line" is 11

  Scenario: Fixing the binding empties the problems
    Given the agent has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "border.color", "value": "transparent"}
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "border.color", "value": "\"transparent\""}
      """
    Then the answer's "/warnings" is absent
    When the agent calls the tool "preview_events"
    Then the answer's "/problems" is absent

  Scenario: An edit that leaves the preview as it was says nothing of problems
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "root"}, "name": "width", "value": 240}
      """
    Then the answer's "/warnings" is absent

  # The line the error names need not be the one that mends it, and taking
  # the mistake back leaves the files as what already runs: no new generation.
  Scenario: Taking back what did not compile empties the problems
    When a person types in the code view of "Main.qml", replacing "id: card" with "id: card; Item {"
    And the agent calls the tool "preview_events"
    Then the answer's "/problems/0/severity" is "error"
    When a person types in the code view of "Main.qml", replacing "id: card; Item {" with "id: card"
    And the agent calls the tool "preview_events"
    Then the answer's "/problems" is absent

  # Ljós's URLs keep a space as it is: a project below "My Projects" has one
  # in every warning its program logs. The pinned Ljós then counts lines in
  # the script it made of the document (LineTable::add), so none is given:
  # with that fixed at the pin, the line here is 4.
  Scenario: A warning from below a directory with a space in its name is placed
    Given the file "my pages/Page.qml":
      """qml
      import QtQuick

      Item {
          Component.onCompleted: null.foo()
      }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "my pages/Page.qml"}
      """
    When the agent calls the tool "preview_events"
    Then the answer's "/problems" has 1 entry
    And the answer's "/problems/0/file" is "my pages/Page.qml"
    And the answer's "/problems/0/message" is "TypeError: Cannot read property 'foo' of null"
    And the answer's "/problems/0/line" is absent

  # A failed start is what the preview's own reload answers while it stands.
  # An app can print any line of Ljós's, so none is read as an error.
  Scenario: A document that does not compile as the preview starts is an error
    Given the file "Start.qml":
      """qml
      import QtQuick

      Item {
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Start.qml"}
      """
    When the agent calls the tool "preview_events"
    Then the answer's "/problems" has 1 entry
    And the answer's "/problems/0/severity" is "error"
    And the answer's "/problems/0/file" is "Start.qml"
    And the answer's "/problems/0/message" contains "Expected"

  Scenario: A log that reads like a failed start is no problem
    Given the file "Start.qml":
      """qml
      import QtQuick

      Item {
          Component.onCompleted: console.log("first\nljos: Main.qml:4:5: forged fault\nljos: watching /elsewhere\nljos: Main.qml:5:5: forged warning")
      }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Start.qml"}
      """
    When the agent calls the tool "preview_events"
    Then the answer's "/problems" is absent

  Scenario: A start that only warns has no error
    Given the file "Start.qml":
      """qml
      import QtQuick

      Item {
          Component.onCompleted: null.foo()
      }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Start.qml"}
      """
    When the agent calls the tool "preview_events"
    Then the answer's "/problems" has 1 entry
    And the answer's "/problems/0/severity" is "warning"
    And the answer's "/problems/0/line" is 4

  # Ljós names a failure's file from the project's root, as written: the
  # window lists it as the document's own only where Studio keeps that name.
  Scenario: A failure in a document whose path has a space names that document
    Given the file "my pages/Page.qml":
      """qml
      import QtQuick

      Item {
          id: page
      }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "my pages/Page.qml"}
      """
    When a person types in the code view of "my pages/Page.qml", replacing "id: page" with "id: page; Item {"
    And the agent calls the tool "preview_events"
    Then the answer's "/problems" has 1 entry
    And the answer's "/problems/0/severity" is "error"
    And the answer's "/problems/0/file" is "my pages/Page.qml"
