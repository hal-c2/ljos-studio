# Sources:
#   docs/MCP.md (node_arrange `anchor`, `wrap`)
#   features/edit/anchors.feature (the same actions as an agent has them)
#   studio7 dogfood backlog P7-16 (margin fields per pin, centre lines, a pin
#   writes no `topMargin: 0` over `anchors.margins`, per-child alignment)

@ui
Feature: A person pins an object from the inspector's Layout section
  A small pad shows the four sides, the centre and each centre line; pressing
  one pins or unpins it. Under the pad each pinned side has its margin to
  write, and all of them have the margin they share. What a row, column or
  grid layout places is not pinned: it is told where it sits in its place
  there. A layout shows its spacing and columns here too, and the selected
  object can be wrapped from here.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      import QtQuick.Layouts

      Item {
          id: root
          width: 400; height: 300
          Rectangle { id: box; x: 10; y: 20; width: 50; height: 40 }
          Grid {
              id: table
              x: 100
              spacing: 3
              columns: 2
              Text { id: cell }
          }
          Timer { id: tick }
          RowLayout {
              id: bar
              y: 200
              Rectangle { id: chip; width: 10; height: 10 }
          }
      }
      """
    And Studio's window shows the project
    And the person has opened "Main.qml" in the documents list

  Scenario: Pressing a side pins it, pressing it again takes it off
    When the person selects "Rectangle box" in the layers panel
    And the person clicks the button "Pin left"
    Then the file "Main.qml" says "anchors.left: parent.left"
    When the person clicks the button "Pin left"
    Then the file "Main.qml" does not say "anchors.left"

  Scenario: The centre pins it to the middle
    When the person selects "Rectangle box" in the layers panel
    And the person clicks the button "Pin centre"
    Then the file "Main.qml" says "anchors.centerIn: parent"

  Scenario: A centre line pins one axis to the middle, and pressing it again lets it go
    When the person selects "Rectangle box" in the layers panel
    And the person clicks the button "Pin horizontal centre"
    Then the file "Main.qml" says "anchors.horizontalCenter: parent.horizontalCenter"
    When the person clicks the button "Pin horizontal centre"
    Then the file "Main.qml" does not say "anchors.horizontalCenter"

  Scenario: Letting one line of a centring go keeps the other
    When the person selects "Rectangle box" in the layers panel
    And the person clicks the button "Pin centre"
    And the person clicks the button "Pin horizontal centre"
    Then the file "Main.qml" says "anchors.verticalCenter: parent.verticalCenter"
    And the file "Main.qml" does not say "anchors.centerIn"

  Scenario: A pinned side's margin is written in its field
    When the person selects "Rectangle box" in the layers panel
    And the person clicks the button "Pin left"
    And the person sets "Left margin" to "12" in the inspector
    Then the file "Main.qml" says "anchors.leftMargin: 12"

  Scenario: A side that is not pinned has no margin to write
    When the person selects "Rectangle box" in the layers panel
    And the person clicks the button "Pin left"
    Then the inspector's field "Left margin" is enabled
    And the inspector's field "Right margin" is disabled

  # The box sits 10 from the left, where the shared margin puts it too.
  Scenario: A pin where the shared margin already puts it writes no margin of its own
    When the person selects "Rectangle box" in the layers panel
    And the person sets "Margins" to "10" in the inspector
    And the person clicks the button "Pin left"
    Then the file "Main.qml" says "anchors.left: parent.left"
    And the file "Main.qml" says "anchors.margins: 10"
    And the file "Main.qml" does not say "leftMargin"

  # A scrub the window ends before the hand lets go (a popup opening, the
  # pointer leaving the window) keeps what it showed, as letting go does.
  Scenario: A margin scrubbed when the window takes the press away is written
    When the person selects "Rectangle box" in the layers panel
    And the person sets "Margins" to "10" in the inspector
    And the person scrubs the inspector's "Margins" by 5 and the window takes the press away
    Then the file "Main.qml" says "anchors.margins: 15"

  # Emptied while a state is picked, the state's own margin is taken out:
  # the object's own one shows again, and no `null` is written for it.
  Scenario: A margin emptied in a state takes the state's margin out
    Given the person has selected "Rectangle box" in the layers panel
    And the person has clicked the button "Pin left"
    And the person has set "Left margin" to "6" in the inspector
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    When the person sets "Left margin" to "12" in the inspector
    Then the file "Main.qml" says "anchors.leftMargin: 12"
    When the person sets "Left margin" to "" in the inspector
    Then the file "Main.qml" says "anchors.leftMargin: 6"
    And the file "Main.qml" does not say "anchors.leftMargin: 12"
    And the file "Main.qml" does not say "null"

  Scenario: What a row layout places is told where it sits in the row
    When the person selects "Rectangle chip" in the layers panel
    Then the inspector has no button "Pin left"
    When the person clicks the button "Bottom of the row"
    Then the file "Main.qml" says "Layout.alignment: Qt.AlignBottom"
    When the person clicks the button "Bottom of the row"
    Then the file "Main.qml" does not say "Layout.alignment"

  # A grid's cell has both axes: a flag of one is changed with the other's
  # kept, and a flag that is not one of the buttons' is read for its axis.
  Scenario: A cell's flag on one axis is changed and the other's is kept
    When the person replaces "RowLayout {" with "GridLayout {" in the code view
    And the person replaces "id: chip;" with "id: chip; Layout.alignment: Qt.AlignRight | Qt.AlignBaseline;" in the code view
    And the person selects "Rectangle chip" in the layers panel
    And the person clicks the button "Top of its cell"
    Then the file "Main.qml" says "Layout.alignment: Qt.AlignRight | Qt.AlignTop"

  Scenario: A cell centred both ways keeps its centre across when put at the top
    When the person replaces "RowLayout {" with "GridLayout {" in the code view
    And the person replaces "id: chip;" with "id: chip; Layout.alignment: Qt.AlignCenter;" in the code view
    And the person selects "Rectangle chip" in the layers panel
    And the person clicks the button "Top of its cell"
    Then the file "Main.qml" says "Layout.alignment: Qt.AlignHCenter | Qt.AlignTop"

  Scenario: An object is wrapped from the section
    When the person selects "Rectangle box" in the layers panel
    And the person clicks the button "Wrap in Column"
    Then the file "Main.qml" says "Column {"

  Scenario: A layout is turned into another kind from the section
    Given an agent has called the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "table"}, "name": "columns", "value": null}
      """
    And the person has selected "Grid table" in the layers panel
    When the person clicks the button "Turn into Flow"
    Then the file "Main.qml" says "Flow {"
    And the file "Main.qml" does not say "Grid {"
    When the person clicks the button "Turn into Column"
    Then the file "Main.qml" says "Column {"

  Scenario: A layout says its spacing here
    When the person selects "Grid table" in the layers panel
    Then the inspector's "Layout spacing" holds "3"
    And the inspector's "Layout columns" holds "2"

  # A GridLayout has no `spacing`: its rows and columns are spaced apart.
  Scenario: A grid layout's spacing is written by rows and columns
    When the person replaces "RowLayout {" with "GridLayout {" in the code view
    And the person selects "GridLayout bar" in the layers panel
    And the person sets "Layout rowSpacing" to "6" in the inspector
    Then the file "Main.qml" says "rowSpacing: 6"
    And the inspector does not offer the field "Layout spacing"

  Scenario: A layout's spacing set while a state is picked goes into the state
    Given the person has selected "Item root" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    When the person selects "Grid table" in the layers panel
    And the person sets "Layout spacing" to "12" in the inspector
    Then the file "Main.qml" says "spacing: 3"
    And the file "Main.qml" says "spacing: 12"
    And the file "Main.qml" says "target: table"
    And the inspector's "Layout spacing" holds "12"

  # A scrub writes over what the field showed; once the state is let go of,
  # the field shows the object's own value again, not the scrubbed one.
  Scenario: A layout's spacing scrubbed in a state shows its own once the state is let go of
    Given the person has selected "Grid table" in the layers panel
    And the person has added the state "compact" in the inspector
    And the person has picked the state "compact" in the inspector
    When the person scrubs the inspector's "Layout spacing" by 5 and the window takes the press away
    Then the file "Main.qml" says "spacing: 8"
    When the person lets go of the state "compact" in the inspector
    Then the inspector's "Layout spacing" holds "3"

  Scenario: What a layout places cannot be pinned, and the section says so
    When the person selects "Text cell" in the layers panel
    Then the inspector has no button "Pin left"
    And the window says "It is placed by its Grid"

  # A Row just placed has no x written, and an empty one has no size to drag.
  @preview
  Scenario: Where an object is can be written without knowing the names
    When the person selects "Grid table" in the layers panel
    Then the inspector offers the field "y"
    When the person sets "y" to "30" in the inspector
    Then the file "Main.qml" says "y: 30"

  @preview
  Scenario: What has no place on the canvas is offered none
    When the person selects "Timer tick" in the layers panel
    Then the inspector does not offer the field "y"

  @preview
  Scenario: What a layout places is offered no place
    When the person selects "Text cell" in the layers panel
    Then the inspector does not offer the field "y"
