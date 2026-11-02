# Sources:
#   docs/MCP.md (slot_contribute, blocks_list)
#   ljos-ui docs/adr/0003-blocks-and-shells.md (section 4: Ljos.Blocks'
#   Contribution and Slot; a slot is a block's list<Contribution> property)

Feature: An agent contributes to a block's slot
  A block offers slots: places in it that a shell, a plugin or an agent
  fills without changing the block. A slot is a `list<Contribution>`
  property of the block, and a `Contribution` names it: its `slot`, how it
  joins what is there (`mode`: append, prepend or replace), where among
  the others (`order`), and what it shows (`delegate`), which may be any
  QML or an instance of another block. `slot_contribute` writes one into
  the shell, importing `Ljos.Blocks` and the block's module if the shell
  does not yet, or takes one out. It refuses a slot no block offers.

  Background:
    Given the file "App/Media/qmldir":
      """
      module App.Media
      # block Player 1.0 Player.qml Media "Plays one source"
      # block Volume 1.0 Volume.qml Media "Sets the volume"
      """
    And the file "App/Media/Player.qml":
      """qml
      import QtQuick
      import Ljos.Blocks

      Column {
          property url source
          property list<Contribution> controls
          Text { text: "Player" }
          Slot { name: "controls" }
      }
      """
    And the file "App/Media/Volume.qml":
      """qml
      import QtQuick

      Text {
          property real level: 1
          text: "Volume"
      }
      """
    And the file "Shell.qml":
      """qml
      import QtQuick
      import App.Media

      Column {
          id: root
          Player { id: player1 }
      }
      """
    And the studio tools are connected

  Scenario: An agent asks which toolkit types a document can place
    Given the file "Main.qml":
      """qml
      import QtQuick.Layouts

      Item {}
      """
    When the agent calls the tool "blocks_list" with:
      """json
      {"doc": "Main.qml", "search": "row"}
      """
    Then the answer's "/types/0/module" is "QtQuick.Layouts"
    And the answer's "/types/0/imported" is true
    And the answer's "/types/0/types/0" is "RowLayout"
    And the answer's "/types/1/module" is "QtQuick"
    And the answer's "/types/1/imported" is false
    And the answer's "/types/1/types/0" is "Row"

  Scenario: Toolkit types are asked for by document
    When the agent calls the tool "blocks_list"
    Then the answer's "/types" has 0 entries

  Scenario: A contribution written in QML
    When the agent calls the tool "slot_contribute" with:
      """json
      {"doc": "Shell.qml", "slot": "controls", "text": "Text { text: \"Shuffle\" }"}
      """
    Then the file "Shell.qml" is:
      """qml
      import QtQuick
      import App.Media
      import Ljos.Blocks

      Column {
          id: root
          Player { id: player1 }
          Contribution {
              id: contribution1
              slot: "controls"
              delegate: Component {
                  Text { text: "Shuffle" }
              }
          }
      }
      """

  Scenario: A block contributed first among the others
    When the agent calls the tool "slot_contribute" with:
      """json
      {"doc": "Shell.qml", "slot": "controls", "block": "App.Media.Volume", "mode": "prepend", "order": 2}
      """
    Then the file "Shell.qml" is:
      """qml
      import QtQuick
      import App.Media
      import Ljos.Blocks

      Column {
          id: root
          Player { id: player1 }
          Contribution {
              id: contribution1
              slot: "controls"
              mode: Contribution.Prepend
              order: 2
              delegate: Component {
                  Volume {}
              }
          }
      }
      """

  Scenario: A slot no block offers is refused
    When the agent calls the tool "slot_contribute" with:
      """json
      {"doc": "Shell.qml", "slot": "sidebar", "text": "Text {}"}
      """
    Then the agent is refused with "not_found"
    And the refusal's "/details/kind" is "no_slot"
    And the file "Shell.qml" is:
      """qml
      import QtQuick
      import App.Media

      Column {
          id: root
          Player { id: player1 }
      }
      """

  Scenario: A delegate that would not parse is refused
    When the agent calls the tool "slot_contribute" with:
      """json
      {"doc": "Shell.qml", "slot": "controls", "text": "Text { text: }"}
      """
    Then the agent is refused with "invalid_argument"

  Scenario: A contribution is taken out, and undo puts it back
    Given the agent has called the tool "slot_contribute" with:
      """json
      {"doc": "Shell.qml", "slot": "controls", "text": "Text { text: \"Shuffle\" }"}
      """
    When the agent calls the tool "slot_contribute" with:
      """json
      {"doc": "Shell.qml", "remove": {"id": "contribution1"}}
      """
    Then the file "Shell.qml" is:
      """qml
      import QtQuick
      import App.Media
      import Ljos.Blocks

      Column {
          id: root
          Player { id: player1 }
      }
      """
    When the agent calls the tool "history" with:
      """json
      {"action": "undo"}
      """
    Then the answer's "/entry/label" is "remove contribution"
    And the file "Shell.qml" is:
      """qml
      import QtQuick
      import App.Media
      import Ljos.Blocks

      Column {
          id: root
          Player { id: player1 }
          Contribution {
              id: contribution1
              slot: "controls"
              delegate: Component {
                  Text { text: "Shuffle" }
              }
          }
      }
      """

  Scenario: Only a contribution is taken out
    When the agent calls the tool "slot_contribute" with:
      """json
      {"doc": "Shell.qml", "remove": {"id": "player1"}}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's "/details/kind" is "not_a_contribution"

  @preview
  Scenario: The preview shows the contribution in the block's slot
    Given the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "preview_open" with:
      """json
      {"doc": "Shell.qml"}
      """
    When the agent calls the tool "slot_contribute" with:
      """json
      {"doc": "Shell.qml", "slot": "controls", "text": "Text { objectName: \"shuffle\"; Accessible.role: Accessible.StaticText; text: \"Shuffle\" }"}
      """
    Then the preview shows "Shuffle"
