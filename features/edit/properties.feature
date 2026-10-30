# Sources:
#   docs/MCP.md (property_set, binding_set)
#   https://doc.qt.io/qt-6/qtqml-syntax-objectattributes.html (grouped properties)

Feature: An agent sets properties and bindings
  `property_set` writes a literal: a bool, a number or a string, quoted
  and escaped as QML needs. `binding_set` writes an expression or a
  signal handler as given, and refuses one that would leave the file not
  parsing. `null` removes either. A grouped property is one property
  whether the file writes it `font.bold` or `font { bold }`, and a new
  one follows the way the file already writes that group.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick

      Item {
          id: root
          font { bold: true }

          Text {
              id: label
              font.pixelSize: 12
          }
      }
      """
    And the studio tools are connected

  Scenario: A string is quoted and escaped
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "text", "value": "say \"hi\""}
      """
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          font { bold: true }

          Text {
              id: label
              font.pixelSize: 12
              text: "say \"hi\""
          }
      }
      """

  Scenario: A grouped property is changed where it is written
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "root"}, "name": "font.bold", "value": false}
      """
    And the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "font", "value": null}
      """
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          font { bold: false }

          Text {
              id: label
              font.pixelSize: 12
          }
      }
      """

  Scenario: A new member of a group follows the file's way of writing it
    Given the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "root"}, "name": "font.italic", "value": true}
      """
    And the agent has called the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "font.italic", "value": true}
      """
    Then the file "Main.qml" is:
      """qml
      import QtQuick

      Item {
          id: root
          font { bold: true; italic: true }

          Text {
              id: label
              font.pixelSize: 12
              font.italic: true
          }
      }
      """

  Scenario: A binding and a handler
    Given the agent has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "width", "value": "parent.width / 2"}
      """
    And the agent has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "onTextChanged", "value": "console.log(text)"}
      """
    When the agent calls the tool "node_inspect" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}}
      """
    Then the answer's "/bindings/2/name" is "width"
    And the answer's "/bindings/2/value" is "parent.width / 2"
    And the answer's "/bindings/3/name" is "onTextChanged"

  Scenario: A binding that would not parse is refused
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "label"}, "name": "width", "value": "parent.width /"}
      """
    Then the agent is refused with "would_break"
