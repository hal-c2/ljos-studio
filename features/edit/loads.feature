# Sources:
#   docs/MCP.md (node_insert, property_set, node_arrange)
#   qml6 6.11, measured: what is not a member or type here stops a file loading

Feature: A tool does not write what would not load or would not show
  A tool's answer is what an agent goes on. A file that stops loading, or an
  object that shows something else, must be a refusal, naming why, and not an
  edit that went well. A type or member Studio has no description of is not
  refused: only running it could tell.

  Background:
    Given the file "Main.qml":
      """qml
      import QtQuick
      import QtQuick.Layouts
      import App.Native

      Item {
          id: root
          property int count: 0
          states: State { name: "on" }
          RowLayout { id: row; spacing: 4; uniformCellSizes: true }
          GridLayout { id: grid; uniformCellWidths: true }
          Rectangle { id: card; width: 10 }
          Text { id: label }
          Thing { id: thing }
      }
      """
    And the file "Plain.qml":
      """qml
      import QtQuick

      Item {
          id: plain
      }
      """
    And the file "Named.qml":
      """qml
      import QtQuick
      import QtQuick.Layouts as L

      Item {
          id: named
          L.RowLayout { id: row }
      }
      """
    And the studio tools are connected with the capabilities "inspect, mutate, fs"

  Scenario Outline: A member the object has no such thing as is refused
    When the agent calls the tool "<tool>" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "nothere", "value": <value>}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "no_such_member"

    Examples:
      | tool         | value  |
      | property_set | 1      |
      | binding_set  | "1+1"  |

  Scenario Outline: What a toolkit type has by way of its bases is written
    Given the file "Wide.qml":
      """qml
      import QtQuick
      import QtQuick.Controls
      import QtQuick.Layouts

      Item {
          id: item
          Rectangle { id: rect }
          Text { id: text }
          MouseArea { id: area }
          Button { id: pick }
          Timer { id: tick }
          ListView { id: list }
          Image { id: pic }
          Flickable { id: flick }
          NumberAnimation { id: ani }
      }
      """
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Wide.qml", "node": {"id": "<node>"}, "name": "<name>", "value": "null"}
      """
    Then the file "Wide.qml" contains "<name>: null"

    Examples:
      | node  | name                   |
      | item  | layer.enabled          |
      | item  | focus                  |
      | item  | visibleChildren        |
      | item  | onWindowChanged        |
      | pick  | focusPolicy            |
      | pick  | onFocusPolicyChanged   |
      | list  | delegateModelAccess    |
      | item  | transform              |
      | item  | containmentMask        |
      | item  | childrenRect.width     |
      | item  | onChildrenRectChanged  |
      | item  | onParentChanged        |
      | item  | Keys.onPressed         |
      | item  | anchors.fill           |
      | item  | Layout.fillWidth       |
      | item  | onStateChanged         |
      | rect  | border.width           |
      | rect  | gradient               |
      | text  | onLinkActivated        |
      | text  | font.pixelSize         |
      | area  | onClicked              |
      | area  | drag.target            |
      | pick  | onClicked              |
      | pick  | background             |
      | pick  | palette.button         |
      | tick  | onTriggered            |
      | list  | onCurrentIndexChanged  |
      | list  | delegate               |
      | pic   | onStatusChanged        |
      | flick | contentItem            |
      | ani   | onFinished             |

  Scenario Outline: A handler for what the object does not say is refused
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "card"}, "name": "<name>", "value": "count = 1"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "no_such_member"

    Examples:
      | name           |
      | onClicked      |
      | onHeightStolen |

  Scenario Outline: A handler for what the object says is written
    When the agent calls the tool "binding_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "<node>"}, "name": "<name>", "value": "count = 1"}
      """
    Then the file "Main.qml" contains "<name>: count = 1"

    Examples:
      | node  | name                 |
      | card  | onWidthChanged       |
      | card  | onVisibleChanged     |
      | root  | onCountChanged       |
      | root  | Component.onCompleted |
      | thing | onAnything           |

  Scenario Outline: What the object has is written
    When the agent calls the tool "property_set" with:
      """json
      {"doc": "Main.qml", "node": {"id": "<node>"}, "name": "<name>", "value": 1}
      """
    Then the file "Main.qml" contains "<name>: 1"

    Examples:
      | node  | name                |
      | card  | width               |
      | card  | opacity             |
      | label | font.pixelSize      |
      | label | anchors.topMargin   |
      | card  | Layout.columnSpan   |
      | root  | count               |
      | thing | whatever            |

  Scenario Outline: A type nothing the document imports has is refused
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "<text>"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "<why>"

    Examples:
      | text                     | why            |
      | RowLayout {}             | unknown_type   |
      | Nowhere {}               | unknown_type   |
      | Item { RowLayout {} }    | unknown_type   |
      | Rectangle { nothere: 1 } | no_such_member |

  Scenario Outline: A type the document has, or will, is inserted
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "<doc>", "parent": {"id": "<parent>"}, "text": "<type> {}"<more>}
      """
    Then the file "<doc>" contains "<type> {"

    Examples:
      | doc       | parent | type         | more                          |
      | Plain.qml | plain  | Rectangle    |                               |
      | Plain.qml | plain  | QtObject     |                               |
      | Plain.qml | plain  | RowLayout    | , "import": "QtQuick.Layouts" |
      | Main.qml  | root   | NotDescribed |                               |

  Scenario Outline: A type the toolkit's modules have is not called unknown
    Given the file "Wide.qml":
      """qml
      import QtQuick
      import QtQuick.Controls
      import QtQuick.Layouts

      Item {
          id: root
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Wide.qml", "parent": {"id": "root"}, "text": "<type> {}"}
      """
    Then the file "Wide.qml" contains "<type> {"

    Examples:
      | type |
      | ListModel |
      | Component |
      | Connections |
      | Binding |
      | Timer |
      | Instantiator |
      | ObjectModel |
      | DelegateModel |
      | Shortcut |
      | FontLoader |
      | Loader |
      | Repeater |
      | Canvas |
      | ShaderEffect |
      | DropArea |
      | TapHandler |
      | HoverHandler |
      | PointHandler |
      | DragHandler |
      | PinchHandler |
      | WheelHandler |
      | Behavior |
      | SmoothedAnimation |
      | SpringAnimation |
      | LayoutMirroring |
      | Popup |
      | Menu |
      | Dialog |
      | DialogButtonBox |
      | SplitView |
      | StackView |
      | SwipeView |
      | TabBar |
      | ToolBar |
      | ToolTip |
      | Drawer |
      | ScrollView |
      | QtObject |

  # qml6 6.11 loads each of these with `import QtQuick` alone.
  Scenario Outline: What comes with QtQuick is not called unknown
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Plain.qml", "parent": {"id": "plain"}, "text": "<type> {}"}
      """
    Then the file "Plain.qml" contains "<type> {"

    Examples:
      | type |
      | QtObject |
      | Timer |
      | ListModel |
      | ListElement |
      | WorkerScript |
      | Instantiator |
      | DelegateModel |
      | ObjectModel |
      | DelegateModelGroup |
      | ItemSelectionModel |
      | Package |
      | LoggingCategory |
      | StateGroup |
      | State |
      | Transition |
      | AnchorChanges |
      | ParentChange |
      | StateChangeScript |
      | Rotation |
      | Scale |
      | Translate |
      | Matrix4x4 |
      | TextMetrics |
      | FontMetrics |
      | Path |
      | PathLine |
      | PathArc |
      | PathCubic |
      | PathQuad |
      | PathSvg |
      | PathAttribute |
      | PathPercent |
      | PathInterpolator |
      | PathAnimation |
      | Row |
      | AnimatedImage |
      | AnimatedSprite |
      | Sprite |
      | SpriteSequence |
      | BorderImage |
      | IntValidator |
      | DoubleValidator |
      | RegularExpressionValidator |
      | SystemPalette |
      | ShaderEffectSource |
      | MultiPointTouchArea |
      | PinchArea |
      | TouchPoint |
      | Gradient |
      | GradientStop |
      | Vector3dAnimation |
      | ColorAnimation |
      | RotationAnimation |
      | SequentialAnimation |
      | ParallelAnimation |
      | PauseAnimation |
      | PropertyAction |
      | ScriptAction |
      | PropertyAnimation |
      | AnimationController |
      | TextInput |
      | TextEdit |
      | Window |

  # qml6 refuses even QtObject in a file that imports nothing.
  Scenario: In a file that imports nothing, no type is there
    Given the file "Bare.qml":
      """qml
      Item {
          id: bare
      }
      """
    When the agent calls the tool "node_insert" with:
      """json
      {"doc": "Bare.qml", "parent": {"id": "bare"}, "text": "QtObject {}"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"

  Scenario: A paste into a document without the type brings its import
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Main.qml", "nodes": [{"id": "row"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the file "Plain.qml" contains "import QtQuick.Layouts"
    And the file "Plain.qml" contains "RowLayout"

  Scenario: A paste that needs what no plain import brings is refused
    Given the agent has called the tool "node_arrange" with:
      """json
      {"action": "copy", "doc": "Named.qml", "nodes": [{"id": "row"}]}
      """
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "paste", "doc": "Plain.qml", "parent": {"id": "plain"}}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "unknown_type"

  Scenario Outline: A component is not named what the document's imports have
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "component_extract", "doc": "Main.qml", "nodes": [{"id": "card"}], "name": "<name>"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "name_taken"
    And the file "Main.qml" contains "Rectangle { id: card"

    Examples:
      | name      |
      | Text      |
      | GridLayout |
      | Timer     |
      | QtObject  |

  Scenario Outline: A layout keeps only what its new kind has
    When the agent calls the tool "node_arrange" with:
      """json
      {"action": "relayout", "doc": "Main.qml", "nodes": [{"id": "<node>"}], "name": "<kind>"}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "says_other"

    Examples:
      | node | kind       |
      | row  | GridLayout |
      | grid | RowLayout  |

  Scenario: What a state changes is what the object has
    When the agent calls the tool "state_edit" with:
      """json
      {"action": "change", "doc": "Main.qml", "state": "on", "target": {"id": "card"}, "name": "nothere", "value": 1}
      """
    Then the agent is refused with "not_actionable"
    And the refusal's "/details/kind" is "no_such_member"
