# Sources:
#   docs/DESIGN.md (section 6: the drag is drawn by Studio, one edit on drop)
#   docs/DOGFOOD.md (D1: direct manipulation; a Layout or anchored parent
#   says what a drag does)
#   docs/MCP.md (node_arrange)

Feature: Objects are placed by the tools a canvas drag drops through
  A drop on the canvas is one call an agent can make too: every object in
  it moves in one edit and one undo step. What is written depends on what
  governs the object: x and y where nothing does, margins where anchors do.
  Where a layout places the object, x and y would be overridden, so they are
  refused rather than written.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      import QtQuick.Layouts

      Rectangle {
          id: root
          width: 400; height: 300

          Rectangle { id: a; x: 10; y: 10; width: 50; height: 50 }
          Rectangle { id: b; x: 100; y: 30; width: 50; height: 50 }
          Rectangle { id: pinned; anchors.left: parent.left; anchors.leftMargin: 8; y: 200; width: 40; height: 40 }
          Rectangle { id: full; anchors.fill: parent; z: -1 }
          RowLayout {
              id: row
              y: 240
              Rectangle { id: first; Layout.preferredWidth: 20; Layout.preferredHeight: 20 }
              Rectangle { id: wide; Layout.fillWidth: true; height: 20 }
          }
      }
      """

  Scenario: Moving two objects is one edit and one undo step
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Main.qml", "places": [
        {"node": {"id": "a"}, "x": 20, "y": 25},
        {"node": {"id": "b"}, "x": 110}
      ]}
      """
    Then the file "Main.qml" contains "id: a; x: 20; y: 25;"
    And the file "Main.qml" contains "id: b; x: 110; y: 30;"
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Main.qml" contains "id: a; x: 10; y: 10;"
    And the file "Main.qml" contains "id: b; x: 100; y: 30;"

  Scenario: Resizing writes the object's size
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Main.qml", "places": [{"node": {"id": "a"}, "width": 80, "height": 90}]}
      """
    Then the file "Main.qml" contains "id: a; x: 10; y: 10; width: 80; height: 90"

  Scenario: Moving an object a layout places is refused and writes nothing
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Main.qml", "places": [{"node": {"id": "first"}, "x": 5}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "arranged"
    And the file "Main.qml" contains "id: first; Layout.preferredWidth: 20"

  Scenario: Resizing in a layout writes the size it asks the layout for
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Main.qml", "places": [{"node": {"id": "first"}, "width": 33}]}
      """
    Then the file "Main.qml" contains "Layout.preferredWidth: 33"

  Scenario: A size the layout fills is refused
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Main.qml", "places": [{"node": {"id": "wide"}, "width": 33}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "arranged"

  Scenario: An object anchors fill cannot be placed
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Main.qml", "places": [{"node": {"id": "full"}, "x": 5}]}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "anchored"

  Scenario: One refused object leaves every other where it was
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Main.qml", "places": [
        {"node": {"id": "a"}, "x": 20},
        {"node": {"id": "first"}, "x": 5}
      ]}
      """
    Then the agent is refused with "not_actionable"
    And the file "Main.qml" contains "id: a; x: 10;"

  @preview
  Scenario: Moving an anchored object changes its margin, not its x
    Given the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "size": {"width": 400, "height": 300}}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Main.qml", "places": [{"node": {"id": "pinned"}, "x": 30, "y": 210}]}
      """
    Then the file "Main.qml" contains "anchors.leftMargin: 30; y: 210;"

  @preview
  Scenario: Geometry says where objects are and what a drag does to them
    Given the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "size": {"width": 400, "height": 300}}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "geometry", "doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "pinned"}, {"id": "first"}, {"id": "full"}]}
      """
    Then the answer's "/boxes/0/window/x" is 10.0
    And the answer's "/boxes/0/drag/move" is "position"
    And the answer's "/boxes/1/drag/move" is "margins"
    And the answer's "/boxes/2/drag/move" is "reorder"
    And the answer's "/boxes/2/drag/resize" is "preferred"
    And the answer's "/boxes/3/drag/move" is "none"
    And the answer's "/siblings" has 2 entries

  @preview
  Scenario: Geometry without siblings reads only what was asked
    Given the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "size": {"width": 400, "height": 300}}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "geometry", "doc": "Main.qml", "nodes": [{"id": "a"}], "without_siblings": true}
      """
    Then the answer's "/boxes/0/window/x" is 10.0
    And the answer's "/siblings" is absent

  @preview
  Scenario: Geometry has a free object's place as the file does, ahead of the preview
    Given the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "size": {"width": 400, "height": 300}}
      """
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Main.qml", "places": [{"node": {"id": "a"}, "x": 123}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "geometry", "doc": "Main.qml", "nodes": [{"id": "a"}]}
      """
    Then the answer's "/boxes/0/local/x" is 123.0

  @preview
  Scenario: Objects without an id are found by where they are declared
    Given the file "Plain.qml":
      """qml
      import QtQuick

      Item {
          width: 200; height: 200
          Rectangle { x: 12; y: 14; width: 30; height: 30 }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Plain.qml", "size": {"width": 200, "height": 200}}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "geometry", "doc": "Plain.qml", "nodes": [{"path": "root/0"}], "generation": 1}
      """
    Then the answer's "/boxes/0/window/x" is 12.0
    And the answer's "/boxes/0/window/y" is 14.0

  @preview
  Scenario: Aligning moves every object to the selection's edge in one undo step
    Given the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "size": {"width": 400, "height": 300}}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "align", "doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}], "align": "top"}
      """
    Then the file "Main.qml" contains "id: b; x: 100; y: 10;"
    And the history has 1 entry

  @preview
  Scenario: Distributing evens the gaps between three objects
    Given the file "Three.qml":
      """qml
      import QtQuick

      Item {
          id: root
          width: 400; height: 300
          Rectangle { id: a; x: 0; y: 0; width: 10; height: 10 }
          Rectangle { id: b; x: 20; y: 0; width: 10; height: 10 }
          Rectangle { id: c; x: 100; y: 0; width: 10; height: 10 }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Three.qml", "size": {"width": 400, "height": 300}}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "align", "doc": "Three.qml", "nodes": [{"id": "a"}, {"id": "c"}, {"id": "b"}], "align": "distribute_h"}
      """
    Then the file "Three.qml" contains "id: b; x: 50;"

  Scenario: Duplicating gives the copy new ids and a place beside the original
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "duplicate", "doc": "Main.qml", "nodes": [{"id": "a"}]}
      """
    Then the file "Main.qml" contains "id: a1; x: 26; y: 26; width: 50"
    And the answer's "/nodes" has 1 entries
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Main.qml" does not contain "a1"

  Scenario: A copy's references to its own objects follow their new ids
    Given the file "Nest.qml":
      """qml
      import QtQuick

      Item {
          id: root
          Rectangle {
              id: card
              width: inner.width
              Text { id: inner; text: "card" }
          }
      }
      """
    And the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "duplicate", "doc": "Nest.qml", "nodes": [{"id": "card"}]}
      """
    Then the file "Nest.qml" contains "id: card1"
    And the file "Nest.qml" contains "width: inner1.width"
    And the file "Nest.qml" contains "id: inner1; text: \"card\""

  Scenario: What is copied is pasted into the object named, across documents
    Given the file "Other.qml":
      """qml
      import QtQuick

      Item {
          id: root
      }
      """
    And the studio tools are connected
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Other.qml", "parent": {"id": "root"}}
      """
    Then the file "Other.qml" contains "id: a; x: 10;"
    And the file "Other.qml" contains "id: b; x: 100;"
    And the answer's "/nodes" has 2 entries
    And the history has 1 entry

  Scenario: Pasting with nothing copied is refused
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Main.qml", "parent": {"id": "root"}}
      """
    Then the agent is refused with "not_actionable"

  Scenario: Selecting with add toggles an object in the selection
    Given the studio tools are connected
    And the agent has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "a"}]}
      """
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "b"}], "add": true}
      """
    Then the answer's "/selection" has 2 entries
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "a"}], "add": true}
      """
    Then the answer's "/selection" has 1 entries
    And the answer's "/selection/0/id" is "b"

  # Each of these wrote the file whatever `dry_run` said.
  Scenario: A dry run of duplicating writes nothing
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "duplicate", "doc": "Main.qml", "nodes": [{"id": "a"}], "dry_run": true}
      """
    Then the answer's "/text" contains "id: a1"
    And the file "Main.qml" does not contain "a1"

  Scenario: A dry run of pasting writes nothing
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Main.qml", "nodes": [{"id": "a"}]}
      """
    And the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Main.qml", "parent": {"id": "root"}, "dry_run": true}
      """
    Then the answer's "/text" contains "id: a1"
    And the file "Main.qml" does not contain "a1"

  @preview
  Scenario: A dry run of aligning writes nothing
    Given the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "size": {"width": 400, "height": 300}}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "align", "doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}], "align": "top", "dry_run": true}
      """
    Then the file "Main.qml" contains "id: b; x: 100; y: 30;"
    And the history has 0 entries

  Scenario: Taking out several objects is one undo step
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}]}
      """
    Then the file "Main.qml" does not contain "id: a"
    And the file "Main.qml" does not contain "id: b"
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Main.qml" contains "id: a"
    And the file "Main.qml" contains "id: b"

  # A path is true of the text it was read from: the rule every other edit
  # keeps. Turned into a handle first, it named whatever was there now.
  Scenario: A write naming an object by path needs the generation it was read at
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"path": "root/0"}]}
      """
    Then the agent is refused with "invalid_argument"
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"id": "b"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "generation": 1, "nodes": [{"path": "root/0"}]}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "stale_generation"
    And the file "Main.qml" contains "id: a"

  # What the canvas drops goes through the same log as every other edit:
  # another writer's edit, a lease and an undo all see it.
  Scenario: A write against an older generation is rebased past another object's edit
    Given the studio tools are connected
    And a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "b"}, "name": "z", "value": 1}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "generation": 1, "nodes": [{"id": "a"}]}
      """
    Then the answer's "/rebased_from" is 1
    And the answer's "/generation" is 3
    And the file "Main.qml" does not contain "id: a"
    And the file "Main.qml" contains "z: 1"

  Scenario: A write against an older generation conflicts with an edit of the same object
    Given the studio tools are connected
    And a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "a"}, "name": "z", "value": 1}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "generation": 1, "nodes": [{"id": "a"}]}
      """
    Then the agent is refused with "stale_ref"
    And the refusal's "/details/kind" is "conflict"
    And the refusal's "/details/author" is "Ada"
    And the file "Main.qml" contains "id: a"

  Scenario: A write reaching an object another writer holds is refused and changes nothing
    Given the studio tools are connected
    And a second agent named "Ada" has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "b"}], "hold": true}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"id": "a"}, {"id": "b"}]}
      """
    Then the agent is refused with "busy"
    And the refusal's "/details/kind" is "leased"
    And the refusal's "/details/holder" is "Ada"
    And the file "Main.qml" contains "id: a"
    And the file "Main.qml" contains "id: b"

  Scenario: Undo takes back this writer's arranging, past another writer's edit
    Given the studio tools are connected
    And the agent has called the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"id": "a"}]}
      """
    And a second agent named "Ada" has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "b"}, "name": "z", "value": 1}
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the answer's "/entry/label" is "remove"
    And the file "Main.qml" contains "id: a"
    And the file "Main.qml" contains "z: 1"

  Scenario: Taking out an object and what is inside it is one edit
    Given the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "remove", "doc": "Main.qml", "nodes": [{"id": "row"}, {"id": "first"}, {"id": "row"}]}
      """
    Then the answer's "/nodes" has 1 entries
    And the file "Main.qml" does not contain "id: row"
    And the file "Main.qml" does not contain "id: first"
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Main.qml" contains "id: first"

  Scenario: A selection stays as it was when adding to it is refused
    Given the studio tools are connected
    And the agent has called the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "a"}]}
      """
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "nope"}], "add": true}
      """
    Then the agent is refused with "not_found"
    When the agent calls the tool "node_select" with:
      """json
      {"doc": "Main.qml", "nodes": [{"id": "b"}], "add": true}
      """
    Then the answer's "/selection" has 2 entries

  Scenario: A copy's template string follows the copy's new id
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          Rectangle {
              id: card
              width: 10
              Text { text: `card ${card.width}` }
          }
      }
      """
    And the studio tools are connected
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "duplicate", "doc": "Main.qml", "nodes": [{"id": "card"}]}
      """
    Then the file "Main.qml" contains "`card ${card.width}`"
    And the file "Main.qml" contains "`card ${card1.width}`"

  Scenario: A place can be recorded into a state rather than the object
    Given the studio tools are connected
    And the agent has called the tool "state_edit" with:
      """json
      {"action": "add", "doc": "Main.qml", "node": {"id": "root"}, "state": "compact"}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Main.qml", "places": [{"node": {"id": "a"}, "x": 33}],
       "record": {"state": "compact", "on": {"id": "root"}}}
      """
    Then the file "Main.qml" contains "id: a; x: 10; y: 10;"
    And the file "Main.qml" contains "target: a"
    And the file "Main.qml" contains "x: 33"

  Scenario: An object without an id cannot be placed into a state
    Given the studio tools are connected
    And the file "Plain.qml":
      """qml
      import QtQuick

      Item {
          id: root
          states: [State { name: "compact" }]
          Rectangle { x: 1; width: 4; height: 4 }
      }
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "place", "doc": "Plain.qml", "places": [{"node": {"path": "root/0"}, "x": 9}],
       "generation": 1, "record": {"state": "compact", "on": {"id": "root"}}}
      """
    Then the agent is refused with "invalid_argument"
    And the file "Plain.qml" contains "Rectangle { x: 1;"
