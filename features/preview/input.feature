# Sources:
#   docs/MCP.md (preview_state; preview_open)
#   features/preview/flows.feature (a click as a person clicks)
#   ljos-ui docs/ARCHITECTURE.md section 19.4 (the inspector's input tools)

@preview @native
Feature: The preview is scrolled, typed into and made as tall as its page, as a person would
  A long page is tried by scrolling it and a form by typing into it, so
  `preview_state` does what a person's hands do, through the preview's
  inspector: `scroll` turns the wheel at a point, `swipe` drags a finger
  from a point, `press` presses keys and `type` types text into whatever
  has the keyboard (after any `click`, which gives it the keyboard).
  What is read after a scroll or a swipe is where the page came to rest,
  and while it glides other tools still answer.
  `preview_open` with `fit: "content"` makes the preview as tall as what
  its document lays out, never shorter than its target's screen, so what
  is below the fold can be seen and picked; it stays so over each reload
  until `fit: "screen"`.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root

          Flickable {
              id: page
              anchors.fill: parent
              contentHeight: 2000

              Rectangle {
                  id: footer
                  y: 1800
                  width: 400
                  height: 200
                  color: "red"
              }
          }

          TextInput {
              id: field
              width: 200
              height: 40
          }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "target": "desktop"}
      """

  Scenario: The wheel turned over the page scrolls it
    When the agent calls the tool "preview_state" with:
      """json
      {"scroll": {"x": 600, "y": 400, "dy": -40}, "node": {"id": "page"}, "read": ["contentY"]}
      """
    Then the answer's "/state/contentY" is 1200

  Scenario: A finger dragged up the page scrolls it
    When the agent calls the tool "preview_state" with:
      """json
      {"swipe": {"x": 600, "y": 700, "dy": -400}, "node": {"id": "page"}, "read": ["contentY"]}
      """
    Then the answer's "/state/contentY" is above 100

  Scenario: Another tool answers while the page a wheel turned still glides
    When the agent calls the tool "preview_state" with the following, and the tool "preview_events" once its input is in:
      """json
      {"scroll": {"x": 600, "y": 400, "dy": -10}, "node": {"id": "page"}, "read": ["contentY"]}
      """
    Then the second tool answered before the first, while the page still moved
    And the answer's "/state/contentY" is above 100

  Scenario: Text typed after a click lands in the field clicked
    When the agent calls the tool "preview_state" with:
      """json
      {"click": {"id": "field"}, "type": "hello", "node": {"id": "field"}, "read": ["text"]}
      """
    Then the answer's "/state/text" is "hello"

  Scenario: Keys pressed reach what has the keyboard
    Given the agent has called the tool "preview_state" with:
      """json
      {"click": {"id": "field"}, "type": "hello"}
      """
    When the agent calls the tool "preview_state" with:
      """json
      {"press": "Backspace", "node": {"id": "field"}, "read": ["text"]}
      """
    Then the answer's "/state/text" is "hell"

  Scenario: A preview fitted to its content is as tall as the page
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "fit": "content"}
      """
    Then the answer's "/size" is {"width": 1280, "height": 2000}
    And the answer's "/target" is "desktop"
    And the answer's "/fit" is "content"

  Scenario: A page shorter than the screen keeps the screen's height
    Given the file "Short.qml":
      """qml
      import QtQuick

      Rectangle {
          id: root
          Rectangle { width: 100; height: 100 }
      }
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Short.qml", "target": "phone", "fit": "content"}
      """
    Then the answer's "/size" is {"width": 390, "height": 844}

  Scenario: A preview fitted to its content measures a page longer than one listing holds
    Given the file "Long.qml":
      """qml
      import QtQuick

      Item {
          id: root

          Repeater {
              model: 1000

              Rectangle {
                  y: index * 4
                  width: 100
                  height: 4
              }
          }
      }
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Long.qml", "fit": "content"}
      """
    Then the answer's "/size" is {"width": 1280, "height": 4000}

  Scenario: A preview fitted to its content grows with the page
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "fit": "content"}
      """
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "footer"}, "name": "y", "value": 2800}
      """
    And the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    Then the answer's "/size" is {"width": 1280, "height": 3000}

  Scenario: A preview fitted to its content gets shorter when the running page does
    Given the file "Fold.qml":
      """qml
      import QtQuick

      Item {
          id: root

          Rectangle {
              id: panel
              width: 400
              height: 2000
              color: "red"

              MouseArea {
                  id: fold
                  width: 100
                  height: 100
                  onClicked: panel.height = 1000
              }
          }
      }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Fold.qml", "fit": "content"}
      """
    And the agent has called the tool "preview_state" with:
      """json
      {"node": {"id": "panel"}, "click": {"id": "fold"}, "read": ["height"]}
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Fold.qml"}
      """
    Then the answer's "/size" is {"width": 1280, "height": 1000}

  Scenario: A preview fitted to an item as wide as its window is not resized when opened again
    Given the file "Tall.qml":
      """qml
      import QtQuick

      Item {
          id: root
          property bool counting: false
          property int resizes: 0
          onHeightChanged: if (counting) resizes += 1

          Rectangle {
              id: page
              width: parent.width
              height: 2000
              color: "white"

              Rectangle {
                  width: 400
                  height: 1500
                  color: "red"

                  MouseArea {
                      id: count
                      width: 100
                      height: 100
                      onClicked: root.counting = true
                  }
              }
          }
      }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Tall.qml", "fit": "content"}
      """
    And the agent has called the tool "preview_state" with:
      """json
      {"node": {"id": "root"}, "click": {"id": "count"}}
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Tall.qml"}
      """
    When the agent calls the tool "preview_state" with:
      """json
      {"node": {"id": "root"}, "read": ["resizes"]}
      """
    Then the answer's "/state/resizes" is 0

  Scenario: A preview grown to an item as wide as its window fits the rest when that item folds
    Given the file "Sheet.qml":
      """qml
      import QtQuick

      Item {
          id: root

          Rectangle {
              id: sheet
              width: parent.width
              height: 2000
              color: "white"
          }

          Rectangle {
              width: 400
              height: 1500
              color: "red"

              MouseArea {
                  id: fold
                  width: 100
                  height: 100
                  onClicked: sheet.height = 1000
              }
          }
      }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Sheet.qml", "fit": "content"}
      """
    And the agent has called the tool "preview_state" with:
      """json
      {"node": {"id": "sheet"}, "click": {"id": "fold"}, "read": ["height"]}
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Sheet.qml"}
      """
    Then the answer's "/size" is {"width": 1280, "height": 1500}

  Scenario: Fitted to the screen again, the preview is the screen's height
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "fit": "content"}
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml", "fit": "screen"}
      """
    Then the answer's "/size" is {"width": 1280, "height": 800}
    And the answer's "/fit" is "screen"
