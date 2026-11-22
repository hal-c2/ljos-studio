# Sources:
#   docs/MCP.md (preview_open: a document opened alone)
#   https://doc.qt.io/qt-6/qtqml-syntax-objectattributes.html (required
#   properties: an object whose required property is not set is not made)

@preview
Feature: A component or a store opened alone shows something
  A delegate or a component that requires properties is not made when
  nothing sets them, and a store shows nothing of itself: opened alone,
  either would leave the preview empty, and a person reshaping it blind.
  So the preview runs one instance of it, with a sample value for each
  property it requires: one given in `.studio/samples.json` for the
  document, else one of its type. A block that shows nothing is shown as
  what it offers, read from a live instance. The document is not changed.

  Background:
    Given the file "Blocks/qmldir":
      """
      module Blocks
      # block HabitStore 1.0 HabitStore.qml Habits "Keeps the habits"
      """
    And the file "Blocks/HabitCard.qml":
      """qml
      import QtQuick

      Rectangle {
          required property string title
          required property bool done
          color: done ? "#6750a4" : "#e8e8ec"

          Text {
              objectName: "title"
              Accessible.role: Accessible.StaticText
              text: parent.title
          }
      }
      """
    And the file "Blocks/HabitStore.qml":
      """qml
      import QtQuick

      Item {
          property alias habits: model
          signal added(string title)

          ListModel { id: model }

          function add(title) {
              model.append({ title: title, done: false })
              added(title)
          }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, run"

  Scenario: A card that requires properties shows with sample values
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Blocks/HabitCard.qml"}
      """
    Then the preview shows "Title"
    And the file "Blocks/HabitCard.qml" does not contain "Title"

  Scenario: Samples given once for the document are used
    Given the file ".studio/samples.json":
      """json
      {"Blocks/HabitCard.qml": {"title": "Read a book", "done": true}}
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Blocks/HabitCard.qml"}
      """
    Then the preview shows "Read a book"

  Scenario: A store shows what it offers
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Blocks/HabitStore.qml"}
      """
    Then the preview shows "HabitStore"
    And the preview shows "add(title)"
    And the preview shows "added(string title)"

  Scenario: What the preview runs in the document's place is no document
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Blocks/HabitCard.qml"}
      """
    When the agent calls the tool "project_open"
    Then the answer's "/documents" lists "Blocks/HabitCard.qml, Blocks/HabitStore.qml"

  Scenario: The stand-in goes when another document is previewed
    Given the file "Main.qml":
      """qml
      import QtQuick

      Text { text: "Home" }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Blocks/HabitCard.qml"}
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    Then the preview shows "Home"
    And the file "Blocks/.HabitCard.preview.qml" does not exist

  Scenario: The stand-in goes when another project is opened
    Given the agent has called the tool "preview_open" with:
      """json
      {"doc": "Blocks/HabitCard.qml"}
      """
    When the agent calls the tool "project_open" with:
      """json
      {"path": "Blocks"}
      """
    Then the file "Blocks/.HabitCard.preview.qml" does not exist

  Scenario: A preview that does not start leaves no stand-in
    Given previews run the program "no-such-ljos"
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Blocks/HabitCard.qml"}
      """
    Then the agent is refused with "not_actionable"
    And the file "Blocks/.HabitCard.preview.qml" does not exist

  Scenario: A file of the person's in the stand-in's place is left alone
    Given the file "Blocks/.HabitCard.preview.qml":
      """qml
      // Mine
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Blocks/HabitCard.qml"}
      """
    Then the file "Blocks/.HabitCard.preview.qml" contains "// Mine"

  Scenario: A document that comes to require a property is previewed again
    Given the file "Note.qml":
      """qml
      import QtQuick

      Text { text: "Plain" }
      """
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Note.qml"}
      """
    When somebody writes the file "Note.qml":
      """qml
      import QtQuick

      Text {
          required property string note
          text: note
      }
      """
    And the agent calls the tool "preview_open" with:
      """json
      {"doc": "Note.qml"}
      """
    Then the preview shows "Note"
