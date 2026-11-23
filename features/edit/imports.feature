# Sources:
#   docs/MCP.md (node_insert, binding_set, state_edit)
#   $S dogfood backlog P7-11 (an edit that writes a type or a singleton
#   leaves the file without its import, and the preview stops loading)
#   qml6 6.11, measured: `Colors.accent` without `import Ljos.Design` is a
#   ReferenceError, `ColumnLayout {}` without `import QtQuick.Layouts` stops
#   the file loading

Feature: An edit brings the imports what it writes needs
  An agent that writes `ColumnLayout {}` or `color: Colors.accent` means
  the toolkit's layout and the design module's colours. The edit that
  writes them adds the import each needs, once, after the file's others,
  so the file goes on loading. It does so only where one module Studio
  knows of has the name. What the file can already name, or names under
  another word, it leaves as it is.

  Background:
    Given the file "Plain.qml":
      """qml
      import QtQuick

      Item {
          id: plain
          Rectangle { id: card; width: 10 }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, fs"

  Scenario: A toolkit type the file does not import brings its module
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "ColumnLayout {}"}
      """
    Then the answer's "/id" is "columnLayout1"
    And the file "Plain.qml" is:
      """qml
      import QtQuick
      import QtQuick.Layouts

      Item {
          id: plain
          Rectangle { id: card; width: 10 }
          ColumnLayout { id: columnLayout1 }
      }
      """

  Scenario: The answer tells the import and the object as two changes
    Given the file "Long.qml":
      """qml
      import QtQuick

      Item {
          id: long
          width: 200
          height: 100
          Rectangle { id: one; width: 10 }
          Rectangle { id: two; width: 20 }
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Long.qml", "parent": {"id": "long"}, "text": "ColumnLayout {}"}
      """
    Then the answer's "/changes" has 2 entries
    And the answer's "/changes/0/text" is "import QtQuick.Layouts\n"
    And the answer's "/changes/1/text" contains "ColumnLayout"
    And the answer's "/changes/1/text" does not contain "Rectangle"

  Scenario: An object holding types of several modules brings each
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "ColumnLayout { Button {} }"}
      """
    Then the file "Plain.qml" contains "import QtQuick.Layouts"
    And the file "Plain.qml" contains "import QtQuick.Controls"

  Scenario: The imports an agent names come as a list
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "Rectangle { color: Colors.accent; RowLayout {} }", "import": ["QtQuick.Layouts", "Ljos.Design"]}
      """
    Then the file "Plain.qml" contains "import QtQuick.Layouts"
    And the file "Plain.qml" contains "import Ljos.Design"

  Scenario Outline: A binding that reads a design singleton brings the design module
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Plain.qml", "node": {"id": "card"}, "name": "<name>", "value": "<value>"}
      """
    Then the file "Plain.qml" contains:
      """qml
      import QtQuick
      import Ljos.Design

      Item {
      """

    Examples:
      | name         | value                                      |
      | color        | Colors.accent                              |
      | radius       | Spacing.sm                                 |
      | border.color | dark ? Colors.border : \\"red\\"             |
      | color        | `${Colors.accent}`                         |
      | objectName   | `inset ${({a: 1}).a + Spacing.sm}`         |
      | color        | `${dark ? `${Colors.border}` : 'red'}`     |

  Scenario: An object put with a design colour brings the design module
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "Rectangle { color: Colors.surface }"}
      """
    Then the file "Plain.qml" contains "import Ljos.Design"

  Scenario: A state's change that reads a design singleton brings the design module
    Given the agent has called the tool "state_edit" with:
      """json
      {"doc": "Plain.qml", "action": "add", "state": "on"}
      """
    When the agent calls the tool "state_edit" with:
      """json
      {"doc": "Plain.qml", "action": "change", "state": "on", "target": {"id": "card"}, "name": "color", "binding": "Colors.accent"}
      """
    Then the file "Plain.qml" contains "import Ljos.Design"

  Scenario Outline: Words that only read like a singleton bring nothing
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Plain.qml", "node": {"id": "card"}, "name": "<name>", "value": "<value>"}
      """
    Then the file "Plain.qml" does not contain "Ljos.Design"

    Examples:
      | name       | value                          |
      | objectName | \\"Colors.accent\\"            |
      | width      | card.Colors.size               |
      | width      | /* Colors.accent */ 10         |
      | objectName | `Colors.accent`                |
      | objectName | `${'Colors.accent'}`           |
      | objectName | `${card.width} Colors.accent`  |

  Scenario Outline: A file that names the module already keeps its imports
    Given the file "Named.qml":
      """qml
      import QtQuick
      <import>

      Rectangle {
          id: named
      }
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Named.qml", "node": {"id": "named"}, "name": "color", "value": "<value>"}
      """
    Then the file "Named.qml" contains:
      """qml
      import QtQuick
      <import>

      Rectangle {
      """

    Examples:
      | import                  | value           |
      | import Ljos.Design      | Colors.accent   |
      | import Ljos.Design as D | D.Colors.accent |

  # Only `D.Colors` reads the module through `import Ljos.Design as D`: a
  # bare `Colors.accent` is a ReferenceError there, as with no import.
  Scenario: A bare singleton in a file that imports the design module under a name brings it bare
    Given the file "Named.qml":
      """qml
      import QtQuick
      import Ljos.Design as D

      Rectangle {
          id: named
      }
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Named.qml", "node": {"id": "named"}, "name": "color", "value": "Colors.accent"}
      """
    Then the file "Named.qml" contains:
      """qml
      import QtQuick
      import Ljos.Design as D
      import Ljos.Design

      Rectangle {
      """

  Scenario: The import is the same undo step, is told, and moves no other byte
    Given the file "Odd.qml":
      """qml
      import QtQuick   // as written

      Item {
          id: odd
          property int   gap:4  // nor this
          Rectangle { id: one;width:10 }
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Odd.qml", "parent": {"id": "odd"}, "text": "RowLayout {}"}
      """
    Then the answer's "/changes/0/text" is "import QtQuick.Layouts\n"
    And the file "Odd.qml" is:
      """qml
      import QtQuick   // as written
      import QtQuick.Layouts

      Item {
          id: odd
          property int   gap:4  // nor this
          Rectangle { id: one;width:10 }
          RowLayout { id: rowLayout1 }
      }
      """
    And the history has 1 entry
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Odd.qml" is:
      """qml
      import QtQuick   // as written

      Item {
          id: odd
          property int   gap:4  // nor this
          Rectangle { id: one;width:10 }
      }
      """
    And the history has 0 entries

  # Measured in a toolkit module, and in no list of what Studio offers from it.
  Scenario Outline: A type no offered module lists is refused as before
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "<text>"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"
    And the file "Plain.qml" is:
      """qml
      import QtQuick

      Item {
          id: plain
          Rectangle { id: card; width: 10 }
      }
      """

    Examples:
      | text                    |
      | FlexboxLayout {}        |
      | Item { SearchField {} } |

  Scenario: An unknown qualified type is refused and brings no import
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "Nowhere.Thing {}"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"
    And the file "Plain.qml" is:
      """qml
      import QtQuick

      Item {
          id: plain
          Rectangle { id: card; width: 10 }
      }
      """

  # loads.feature: what Studio has no word on goes through.
  Scenario: A bare type Studio has no word on brings no import
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "Nowhere {}"}
      """
    Then the file "Plain.qml" contains "Nowhere {"
    And the file "Plain.qml" does not contain "import QtQuick."

  Scenario Outline: A type two known modules have is refused, naming both
    Given the file "acme/qmldir":
      """
      module Acme.Controls
      Slider 1.0 Slider.qml
      """
    And the file "acme/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "<text>"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"
    And the refusal's message matches "Slider.*Acme.Controls.*QtQuick.Controls"
    And the file "Plain.qml" is:
      """qml
      import QtQuick

      Item {
          id: plain
          Rectangle { id: card; width: 10 }
      }
      """

    Examples:
      | text                           |
      | Slider {}                      |
      | Column { Button {} Slider {} } |

  Scenario: A type dump's module is a known one
    Given the file "native/ljostypes.json":
      """json
      {"format": 2, "modules": [{"uri": "App.Native", "versions": ["1.0"], "types": [
        {"name": "Slider", "since": "1.0", "base": "Item", "baseModule": "QtQuick",
         "creatable": true, "defaultProperty": null, "attached": null,
         "properties": [], "signals": [], "methods": []}]}]}
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "Slider {}"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"
    And the refusal's message matches "Slider.*App.Native.*QtQuick.Controls"
    And the file "Plain.qml" is:
      """qml
      import QtQuick

      Item {
          id: plain
          Rectangle { id: card; width: 10 }
      }
      """

  Scenario: The module an agent names settles which of two is meant
    Given the file "acme/qmldir":
      """
      module Acme.Controls
      Slider 1.0 Slider.qml
      """
    And the file "acme/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "Slider {}", "import": "QtQuick.Controls"}
      """
    Then the file "Plain.qml" contains "import QtQuick.Controls"
    And the file "Plain.qml" does not contain "Acme"

  Scenario: A type both QtQml and QtQuick have brings neither
    Given the file "Rows.qml":
      """qml
      import QtQuick.Layouts

      RowLayout {
          id: rows
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Rows.qml", "parent": {"id": "rows"}, "text": "Timer {}"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"
    And the refusal's message matches "Timer.*QtQml.*QtQuick"
    And the file "Rows.qml" does not contain "import QtQml"

  Scenario Outline: A name the file resolves already gets no import
    Given the file "Named.qml":
      """qml
      import QtQuick
      <import>

      Item {
          id: named
          <beside>
      }
      """
    And the file "lib/Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Named.qml", "parent": {"id": "named"}, "text": "<type> {}"}
      """
    Then the file "Named.qml" contains:
      """qml
      import QtQuick
      <import>

      Item {
      """
    And the file "Named.qml" contains "<type> {"

    Examples:
      | import                        | beside                      | type     |
      | import QtQuick.Controls       | Rectangle {}                | Slider   |
      | import QtQuick.Controls as C  | Rectangle {}                | C.Slider |
      | import "lib"               | Rectangle {}                | Slider   |
      | // no other import            | component Slider: Rectangle {} | Slider |

  Scenario: A file beside of the type's name gets no import
    Given the file "Slider.qml":
      """qml
      import QtQuick

      Item {}
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "Slider {}"}
      """
    Then the file "Plain.qml" contains "Slider {"
    And the file "Plain.qml" does not contain "QtQuick.Controls"

  Scenario: A bare name of a module imported under another is refused as before
    Given the file "Aliased.qml":
      """qml
      import QtQuick
      import QtQuick.Controls as C

      Item {
          id: aliased
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Aliased.qml", "parent": {"id": "aliased"}, "text": "Item { Slider {} }"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"
    And the file "Aliased.qml" does not contain "Slider {"

  # The import an insert writes its object under is the one that has the
  # object's type, wherever the agent lists it.
  Scenario: An object is written under the module that has its type, not the first named
    Given the file "Aliased.qml":
      """qml
      import QtQuick
      import QtQuick.Controls as C

      Item {
          id: aliased
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Aliased.qml", "parent": {"id": "aliased"}, "text": "RowLayout { C.Button {} }", "import": ["QtQuick.Controls", "QtQuick.Layouts"]}
      """
    Then the file "Aliased.qml" contains "import QtQuick.Layouts"
    And the file "Aliased.qml" contains "RowLayout {"
    And the file "Aliased.qml" contains "C.Button {"
    And the file "Aliased.qml" does not contain "C.RowLayout"

  # What a module imported `as Ext` holds is written `Ext.Slider`: an import
  # nothing here can read, under a name, says nothing about a bare `Slider`.
  Scenario: A module nothing knows, imported under a name, doesn't keep a bare type's import out
    Given the file "Elsewhere.qml":
      """qml
      import QtQuick
      import Elsewhere.Things as Ext

      Item {
          id: elsewhere
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Elsewhere.qml", "parent": {"id": "elsewhere"}, "text": "Slider {}"}
      """
    Then the file "Elsewhere.qml" contains:
      """qml
      import QtQuick
      import Elsewhere.Things as Ext
      import QtQuick.Controls

      Item {
      """

  # qml6 6.11: a module's Button beats the file beside it.
  Scenario Outline: An import that would change what a name already is stays out
    Given the file "Button.qml":
      """qml
      import QtQuick

      Item {}
      """
    And the file "Holds.qml":
      """qml
      import QtQuick

      Item {
          id: holds
          <held>
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Holds.qml", "parent": {"id": "holds"}, "text": "<text>"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"
    And the file "Holds.qml" does not contain "QtQuick.Controls"

    Examples:
      | held          | text                           |
      | Button {}     | Slider {}                      |
      | Rectangle {}  | Column { Button {} Slider {} } |

  # qml6 6.11: `Item is not a type`, Controls imported or not.
  Scenario: An import that still leaves the file not loading stays out
    Given the file "Bare.qml":
      """qml
      Item {
          id: bare
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Bare.qml", "parent": {"id": "bare"}, "text": "Slider {}"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"
    And the file "Bare.qml" does not contain "import"

  Scenario Outline: A design singleton's name that may be the app's brings no design module
    Given the file "<file>":
      """
      <says>
      """
    And the file "Themed.qml":
      """qml
      import QtQuick
      <import>

      Rectangle {
          id: themed
      }
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Themed.qml", "node": {"id": "themed"}, "name": "color", "value": "Colors.accent"}
      """
    Then the file "Themed.qml" contains "Colors.accent"
    And the file "Themed.qml" does not contain "Ljos.Design"

    Examples:
      | file          | says                                           | import                          |
      | Colors.qml    | QtObject {}                                    | // no other import              |
      | lib/Colors.qml | QtObject {}                                   | import "lib"                 |
      | colors.js     | var accent = 1                                 | import "colors.js" as Colors |
      | app/qmldir    | module App.Theme\nsingleton Colors 1.0 C.qml   | // no other import              |
      | app/qmldir    | module App.Theme\nsingleton Colors 1.0 C.qml   | import App.Theme                |
      | notes.txt     | nothing describes App.Native                   | import App.Native               |

  Scenario: The design module doesn't take a name the file already uses
    Given the file "Spacing.qml":
      """qml
      import QtQuick

      QtObject {}
      """
    And the file "Themed.qml":
      """qml
      import QtQuick

      Rectangle {
          id: themed
          radius: Spacing.sm
      }
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Themed.qml", "node": {"id": "themed"}, "name": "color", "value": "Colors.accent"}
      """
    Then the file "Themed.qml" does not contain "Ljos.Design"

  Scenario: An app's module without those names doesn't keep the design module out
    Given the file "app/qmldir":
      """
      module App.Parts
      Dial 1.0 Dial.qml
      """
    And the file "Themed.qml":
      """qml
      import QtQuick
      import App.Parts

      Rectangle {
          id: themed
      }
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Themed.qml", "node": {"id": "themed"}, "name": "color", "value": "Colors.accent"}
      """
    Then the file "Themed.qml" contains "import Ljos.Design"
