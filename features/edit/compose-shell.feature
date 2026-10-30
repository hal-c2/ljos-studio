# Sources:
#   docs/DESIGN.md (S1 exit: an agent composes a shell over MCP alone)
#   ljos-ui docs/adr/0003-blocks-and-shells.md (blocks, `# block` qmldir comments)

Feature: An agent composes a shell from the app's blocks
  An app offers functional blocks; a shell is ordinary QML that places and
  skins them. An agent with only Studio's tools finds the blocks, makes a
  shell, places two blocks and sets an input, and every step is a history
  entry: undoing them all leaves the project as it was.

  Background:
    Given the file "App/Media/qmldir":
      """
      module App.Media
      # block Player 1.0 Player.qml Media "Plays one source"
      """
    And the file "App/Media/Player.qml":
      """qml
      import QtQuick

      Item {
          property url source
          readonly property bool playing: false
          function play() {}
          signal finished()
      }
      """
    And the file "App/Chat/qmldir":
      """
      module App.Chat
      # block Thread 1.0 Thread.qml Chat
      """
    And the file "App/Chat/Thread.qml":
      """qml
      import QtQuick

      Item {
          property string title
      }
      """
    And the studio tools are connected

  Scenario: The agent lists the blocks it may place
    When the agent calls the tool "blocks_list"
    Then the answer's "/blocks" has 2 entries
    And the answer's "/blocks/0/module" is "App.Chat"
    And the answer's "/blocks/0/inputs/0/name" is "title"
    And the answer's "/blocks/1/name" is "Player"
    And the answer's "/blocks/1/summary" is "Plays one source"
    And the answer's "/blocks/1/actions/0/name" is "play"
    And the answer's "/errors" has 0 entries

  Scenario: A two-block shell, made and undone over MCP alone
    Given the agent has called the tool "document_new" with:
      """json
      {"doc": "Shell.qml", "text": "import QtQuick\nimport QtQuick.Layouts\n\nColumnLayout {\n}\n"}
      """
    And the agent has called the tool "node_insert" with:
      """json
      {"doc": "Shell.qml", "parent": {"path": "root"}, "block": "App.Media.Player"}
      """
    And the agent keeps the answer's "/node" as "player"
    And the agent has called the tool "node_insert" with:
      """json
      {"doc": "Shell.qml", "parent": {"path": "root"}, "block": "App.Chat.Thread", "text": "Thread { title: \"Inbox\" }"}
      """
    And the agent has called the tool "property_set" with:
      """json
      {"doc": "Shell.qml", "node": {"node": "$player"}, "name": "source", "value": "song.ogg"}
      """
    Then the file "Shell.qml" is:
      """qml
      import QtQuick
      import QtQuick.Layouts
      import App.Media
      import App.Chat

      ColumnLayout {
          Player { id: player1; source: "song.ogg" }
          Thread { id: thread1; title: "Inbox" }
      }
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "list"}
      """
    Then the answer's "/done" has 4 entries
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    And the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    And the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    And the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the file "Shell.qml" does not exist
    And the file "App/Media/Player.qml" is:
      """qml
      import QtQuick

      Item {
          property url source
          readonly property bool playing: false
          function play() {}
          signal finished()
      }
      """

  Scenario: A block that is not there is refused
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "App/Chat/Thread.qml", "parent": {"path": "root"}, "block": "App.Chat.Missing"}
      """
    Then the agent is refused with "not_found"
    And the refusal's "/details/kind" is "no_block"

  Scenario: Text of another type is not a block
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "App/Chat/Thread.qml", "parent": {"path": "root"}, "block": "App.Media.Player", "text": "Rectangle {}"}
      """
    Then the agent is refused with "invalid_argument"
    And the file "App/Chat/Thread.qml" is:
      """qml
      import QtQuick

      Item {
          property string title
      }
      """
