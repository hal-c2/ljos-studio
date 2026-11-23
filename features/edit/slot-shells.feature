# Sources:
#   docs/MCP.md (node_insert's `slot`, document_new's `shell`,
#   slot_contribute's `node`, blocks_list)
#   ljos-ui docs/adr/0003-blocks-and-shells.md (section 4: a slot is a
#   block's list<Contribution> property, shown by a Slot)

Feature: A block gets a slot, and a shell skins it
  A block offers a slot by declaring a `list<Contribution>` property and
  showing it with a `Slot` of that name. `node_insert` with `slot` writes
  both, and `import Ljos.Blocks`, in one edit; `blocks_list` then reports
  the slot. `document_new` with `shell` writes a shell of a block: an
  instance of it that replaces what each of its slots shows, a starting
  point to restyle, with a sample value for each property the block
  requires, as a block opened alone is given one. `slot_contribute` with `node` skins one instance alone,
  setting the slot on it.

  Background:
    Given the file "Blocks/qmldir":
      """
      module Blocks
      # block HabitCard 1.0 HabitCard.qml Habits "One habit, done or not"
      # block Plain 1.0 Plain.qml Habits "Has no slots"
      """
    And the file "Blocks/HabitCard.qml":
      """qml
      import QtQuick

      Column {
          id: card
          property string title: "Read"
          property bool done: false

          Text { text: card.title }
      }
      """
    And the file "Blocks/Plain.qml":
      """qml
      import QtQuick

      Item {
          property string title
      }
      """
    And the file "Main.qml":
      """qml
      import QtQuick
      import Blocks

      Column {
          id: root
          HabitCard { id: card1; title: "Walk" }
          Plain { id: plain1 }
      }
      """
    And the studio tools are connected

  Scenario: A slot is declared, imported and shown in one edit
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Blocks/HabitCard.qml", "parent": {"id": "card"}, "slot": "face"}
      """
    Then the file "Blocks/HabitCard.qml" contains "import Ljos.Blocks"
    And the file "Blocks/HabitCard.qml" contains "property list<Contribution> face"
    And the file "Blocks/HabitCard.qml" contains:
      """qml
      Column {
          id: card
          property string title: "Read"
          property bool done: false

          Text { text: card.title }
          Slot {
              id: slot1
              name: "face"
              contributions: card.face
      """
    And the history has 1 entry

  Scenario: blocks_list reports the slot
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Blocks/HabitCard.qml", "parent": {"id": "card"}, "slot": "face"}
      """
    When the agent calls the tool "blocks_list" with:
      """json
      {"module": "Blocks", "search": "HabitCard"}
      """
    Then the answer's "/blocks/0/slots/0/name" is "face"

  Scenario: A root with no id is given one for its slot to read
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Blocks/Plain.qml", "parent": {"path": "root"}, "slot": "badge"}
      """
    Then the file "Blocks/Plain.qml" contains "id: root"
    And the file "Blocks/Plain.qml" contains "contributions: root.badge"

  Scenario: A slot named like another member is refused
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Blocks/HabitCard.qml", "parent": {"id": "card"}, "slot": "title"}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's "/details/kind" is "member_taken"
    And the file "Blocks/HabitCard.qml" does not contain "Ljos.Blocks"

  Scenario: A shell of a block skins each of its slots
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Blocks/HabitCard.qml", "parent": {"id": "card"}, "slot": "face"}
      """
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "HabitCardShell.qml", "shell": "Blocks.HabitCard"}
      """
    Then the file "HabitCardShell.qml" contains "import Blocks"
    And the file "HabitCardShell.qml" contains "import Ljos.Blocks"
    And the file "HabitCardShell.qml" contains "HabitCard {"
    And the file "HabitCardShell.qml" contains "face: Contribution {"
    And the file "HabitCardShell.qml" contains "mode: Contribution.Replace"
    And the file "HabitCardShell.qml" contains "text: habitCard.title"

  @preview
  Scenario: A shell of a block named like a word of the language still loads
    Given the studio tools are connected with the capabilities "inspect, mutate, run"
    And the file "Blocks/qmldir":
      """
      module Blocks
      # block Switch 1.0 Switch.qml Habits "On or off"
      """
    And the file "Blocks/Switch.qml":
      """qml
      import QtQuick

      Item {
          id: toggle
          property string label: "On"
      }
      """
    And the agent has called the tool "node_insert" with:
      """json
      {"doc": "Blocks/Switch.qml", "parent": {"id": "toggle"}, "slot": "face"}
      """
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "SwitchShell.qml", "shell": "Blocks.Switch"}
      """
    Then the file "SwitchShell.qml" contains "id: switchBlock"
    And the file "SwitchShell.qml" contains "text: switchBlock.label"
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "SwitchShell.qml"}
      """
    Then the preview shows "On"

  @preview
  Scenario: A shell sets what its block requires, so it loads
    Given the studio tools are connected with the capabilities "inspect, mutate, run"
    And the file "Blocks/qmldir":
      """
      module Blocks
      # block NoteCard 1.0 NoteCard.qml Notes "One note"
      """
    And the file "Blocks/NoteCard.qml":
      """qml
      import QtQuick

      Item {
          id: note
          required property string title
      }
      """
    And the agent has called the tool "node_insert" with:
      """json
      {"doc": "Blocks/NoteCard.qml", "parent": {"id": "note"}, "slot": "face"}
      """
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "NoteCardShell.qml", "shell": "Blocks.NoteCard"}
      """
    Then the file "NoteCardShell.qml" contains:
      """qml
      NoteCard {
          id: noteCard
          title: "Title"
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "NoteCardShell.qml"}
      """
    Then the preview shows "Title"

  Scenario: A block with no slots has no shell
    When the agent calls the tool "document_new" with:
      """json
      {"doc": "PlainShell.qml", "shell": "Blocks.Plain"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "no_slots"
    And the file "PlainShell.qml" does not exist

  Scenario: One instance is skinned alone
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Blocks/HabitCard.qml", "parent": {"id": "card"}, "slot": "face"}
      """
    When the agent calls the tool "slot_contribute" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card1"}, "slot": "face", "mode": "replace", "text": "Text { text: \"Mine\" }"}
      """
    Then the file "Main.qml" contains "import Ljos.Blocks"
    And the file "Main.qml" contains "face: Contribution {"
    And the file "Main.qml" contains "mode: Contribution.Replace"
    And the file "Main.qml" does not contain "slot: "

  # A path is true of one text only: another writer's insert moves what it names.
  Scenario: An instance named by path is skinned only at the generation the path was read at
    Given the agent has called the tool "node_insert" with:
      """json
      {"doc": "Blocks/HabitCard.qml", "parent": {"id": "card"}, "slot": "face"}
      """
    When the agent calls the tool "slot_contribute" with:
      """json
      {"doc": "Main.qml", "node": {"path": "root/0"}, "slot": "face", "text": "Text {}"}
      """
    Then the agent is refused with "invalid_argument"
    And the refusal's "/details/kind" is "invalid"
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Main.qml", "parent": {"id": "root"}, "generation": 1, "index": 0, "text": "HabitCard { id: card2 }"}
      """
    When the agent calls the tool "slot_contribute" with:
      """json
      {"doc": "Main.qml", "generation": 1, "node": {"path": "root/0"}, "slot": "face", "text": "Text {}"}
      """
    Then the agent is refused with "stale_ref"
    And the file "Main.qml" does not contain "face:"
    When the agent calls the tool "slot_contribute" with:
      """json
      {"doc": "Main.qml", "generation": 2, "node": {"path": "root/1"}, "slot": "face", "text": "Text {}"}
      """
    Then the file "Main.qml" contains "face: Contribution {"

  Scenario: An instance of a block without that slot is refused
    When the agent calls the tool "slot_contribute" with:
      """json
      {"doc": "Main.qml", "node": {"id": "plain1"}, "slot": "face", "text": "Text {}"}
      """
    Then the agent is refused with "not_found"
    And the refusal's "/details/kind" is "no_slot"

  @preview
  Scenario: The preview shows an instance's own skin in its slot
    Given the studio tools are connected with the capabilities "inspect, mutate, run"
    And the agent has called the tool "node_insert" with:
      """json
      {"doc": "Blocks/HabitCard.qml", "parent": {"id": "card"}, "slot": "face"}
      """
    And the agent has called the tool "slot_contribute" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card1"}, "slot": "face", "text": "Text { objectName: \"mine\"; Accessible.role: Accessible.StaticText; text: \"Mine\" }"}
      """
    When the agent calls the tool "preview_open" with:
      """json
      {"doc": "Main.qml"}
      """
    Then the preview shows "Mine"
