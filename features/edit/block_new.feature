# Sources:
#   docs/MCP.md (`document_new`)
#   ljos-ui docs/adr/0003-blocks-and-shells.md (blocks, `# block` qmldir comments)

Feature: An agent makes a functional block
  A block is a QML file its module's `qmldir` declares, and `qmldir` is no
  document of the project. So `document_new` with `block` does both as one
  step: the module is the file's directory, the name the file's.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      Item {}
      """
    And the studio tools are connected

  Scenario: A new block is declared in a new module and listed
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "App/Timer/Countdown.qml", "block": true, "text": "import QtQuick\nItem {\n    property int seconds: 300\n}\n"}
      """
    Then the file "App/Timer/qmldir" is:
      """text
      module App.Timer
      # block Countdown 1.0 Countdown.qml
      """
    When the agent calls the tool "blocks_list" with:
      """json
      {"search": "Countdown"}
      """
    Then the answer's "/blocks/0/name" is "Countdown"
    And the answer's "/blocks/0/module" is "App.Timer"

  Scenario: A block joins the module of a qmldir that is there, keeping what it holds
    Given the file "App/Timer/qmldir":
      """text
      module App.Clocks
      # A comment of the module's.
      # block Lap 1.0 Lap.qml
      """
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "App/Timer/Countdown.qml", "block": true}
      """
    Then the file "App/Timer/qmldir" is:
      """text
      module App.Clocks
      # A comment of the module's.
      # block Lap 1.0 Lap.qml
      # block Countdown 1.0 Countdown.qml
      """

  Scenario: Undo takes back the file and the module it made, and redo brings them back
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "App/Timer/Countdown.qml", "block": true}
      """
    And the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "App/Timer/Countdown.qml" does not exist
    And the file "App/Timer/qmldir" does not exist
    When the agent calls the tool "history" with:
      """json
      {"action": "redo"}
      """
    Then the file "App/Timer/qmldir" is:
      """text
      module App.Timer
      # block Countdown 1.0 Countdown.qml
      """

  Scenario: Undo takes back only its own line of a qmldir written to since
    Given the file "App/Timer/qmldir":
      """text
      module App.Timer
      """
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "App/Timer/Countdown.qml", "block": true}
      """
    And the file "App/Timer/qmldir" is rewritten outside Studio:
      """text
      module App.Timer
      # block Countdown 1.0 Countdown.qml
      # block Lap 1.0 Lap.qml
      """
    And the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "App/Timer/qmldir" is:
      """text
      module App.Timer
      # block Lap 1.0 Lap.qml
      """

  Scenario: A dry run writes neither file
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "App/Timer/Countdown.qml", "block": true, "dry_run": true}
      """
    Then the file "App/Timer/Countdown.qml" does not exist
    And the file "App/Timer/qmldir" does not exist

  Scenario Outline: A block needs a module and a type's name: <doc>
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "<doc>", "block": true}
      """
    Then the agent is refused with "invalid_argument"
    And the file "<doc>" does not exist

    Examples:
      | doc                    |
      | Countdown.qml          |
      | App/Timer/countdown.qml |
      | app-x/Countdown.qml    |
