// The open document's flow as a graph: the root's states are its nodes,
// the base state first, and its transitions and interactions its edges.
// Each state wires the selected object to go to it when tapped, or when
// one of its own signals is emitted (picked above the graph from those
// `node_inspect` lists, change signals left out), and each interaction is
// taken out again, as `state_edit` does for an agent.
//
// In play mode (the canvas's "Play") a click in a frame clicks the running
// app; the state `preview_state` then reads is the node marked playing.
// Out of play, pressing a state's name shows it on the canvas, which is the
// same call with `show`.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Shapes
import Ljos.Design
import Studio

Panel {
    id: panel
    required property Workspace workspace
    title: "Flow"

    readonly property var flow: workspace.flow
    // The state the running app was last read in; null before a click.
    property var playing: null
    // What the selection's wires handle: a tap (a TapHandler, which any
    // item can hold) or a signal the selection has. A new selection starts
    // from a tap again.
    readonly property var triggers: ["tapped"].concat(workspace.signals
        .filter(s => !s.change && s.name !== "tapped")
        .map(s => s.name))
    // The trigger the person picked, kept by name: an edit gives the
    // selection's signals again, and the pick holds while it still has it.
    property string picked: "tapped"
    readonly property string trigger: triggers.indexOf(picked) >= 0 ? picked : "tapped"
    readonly property string selected: workspace.selected
    onSelectedChanged: picked = "tapped"

    function stateName(state) {
        return state === "" ? "Base" : state
    }

    // Where the states stand, and each edge as an arc down their right side:
    // a stack rather than a free layout, because a narrow panel has no room
    // to lay out a graph and the file has no place to keep where it was put.
    readonly property int nodeHeight: 30
    readonly property int nodeGap: Spacing.xs
    readonly property var nodes: flow.states !== undefined ? flow.states : []
    readonly property bool hasAny: (flow.edges || []).some(e => e.from === "*")
    function nodeY(state) {
        if (state === "*")
            return nodeHeight / 2
        const at = nodes.indexOf(state)
        return (at + (hasAny ? 1 : 0)) * (nodeHeight + nodeGap) + nodeHeight / 2
    }
    function arc(edge, index, from) {
        const y0 = nodeY(edge.from)
        const y1 = nodeY(edge.to)
        const reach = 20 + 12 * (index % 5)
        const tip = "M " + (from + 7) + " " + (y1 - 4) + " L " + from + " " + y1 + " L " + (from + 7) + " " + (y1 + 4)
        if (y0 === y1) {
            return "M " + from + " " + (y1 - 6) + " C " + (from + reach) + " " + (y1 - 18) + " "
                + (from + reach) + " " + (y1 + 18) + " " + from + " " + y1 + " " + tip
        }
        return "M " + from + " " + y0 + " C " + (from + reach) + " " + y0 + " "
            + (from + reach) + " " + y1 + " " + from + " " + y1 + " " + tip
    }

    // On the root, whose states these are: the preview finds it by its id.
    function show(state) {
        const layers = workspace.layers
        if (workspace.previews && layers.length > 0 && layers[0].id !== "")
            workspace.dispatch("preview_state", {node: {node: layers[0].node}, show: state})
    }

    function edit(args) {
        args.doc = workspace.doc
        workspace.dispatch("state_edit", args)
    }

    Connections {
        target: panel.workspace

        function onAnswered(tool, answer) {
            // A click that read nothing leaves where the flow is unknown.
            if (tool === "preview_state")
                panel.playing = answer.variant !== undefined ? answer.variant : null
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: Spacing.xs
        spacing: Spacing.xxs

        Label {
            objectName: "flowPlaying"
            visible: panel.playing !== null
            text: panel.playing !== null ? "Playing " + panel.stateName(panel.playing) : ""
            color: Colors.text
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        // A box however many signals the selection has: a MouseArea has
        // ten, which as a row of buttons would push the graph out of the
        // panel.
        ComboBox {
            id: triggerBox
            Layout.fillWidth: true
            visible: panel.workspace.selected !== ""
            model: panel.triggers
            currentIndex: panel.triggers.indexOf(panel.trigger)
            displayText: "Wire " + panel.trigger
            Accessible.name: "Wire"
            onActivated: index => panel.picked = panel.triggers[index]
            // The popup's list is the style's; it is named here so a
            // reader of the window can tell it from the others.
            Component.onCompleted: {
                popup.contentItem.Accessible.role = Accessible.List
                popup.contentItem.Accessible.name = "Wire list"
            }
        }

        Item {
            id: graph
            Layout.fillWidth: true
            readonly property int nodeWidth: Math.min(width - 96, 150)
            implicitHeight: (panel.nodes.length + (panel.hasAny ? 1 : 0)) * (panel.nodeHeight + panel.nodeGap)

            // Where a flow from any state starts.
            Label {
                visible: panel.hasAny
                x: graph.nodeWidth - width - Spacing.xs
                y: 0
                height: panel.nodeHeight
                verticalAlignment: Text.AlignVCenter
                text: "any state"
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
            }

            Repeater {
                model: panel.flow.edges !== undefined ? panel.flow.edges : []
                delegate: Shape {
                    required property var modelData
                    required property int index
                    anchors.fill: parent
                    Accessible.ignored: true
                    ShapePath {
                        strokeColor: modelData.kind === "interaction" ? Colors.accent : Colors.textMuted
                        strokeWidth: 1.5
                        fillColor: "transparent"
                        capStyle: ShapePath.RoundCap
                        joinStyle: ShapePath.RoundJoin
                        PathSvg {
                            path: panel.arc(modelData, index, graph.nodeWidth + 1)
                        }
                    }
                }
            }

            Repeater {
                model: panel.nodes
                delegate: Rectangle {
                    id: node
                    required property string modelData
                    readonly property string name: panel.stateName(modelData)
                    y: panel.nodeY(modelData) - panel.nodeHeight / 2
                    width: graph.nodeWidth
                    height: panel.nodeHeight
                    radius: 6
                    color: panel.playing === modelData ? Colors.accent : Colors.surfaceRaised
                    border.color: Colors.border
                    Accessible.role: Accessible.StaticText
                    Accessible.name: "Flow state " + name

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: Spacing.xs
                        spacing: 0

                        AbstractButton {
                            id: showState
                            text: node.name
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            Accessible.role: Accessible.Button
                            Accessible.name: "Show " + node.name
                            onClicked: panel.show(node.modelData)
                            contentItem: Label {
                                text: node.name
                                elide: Text.ElideRight
                                verticalAlignment: Text.AlignVCenter
                                color: panel.playing === node.modelData ? Colors.textOnAccent : Colors.text
                            }
                            background: Item { }
                        }
                        IconButton {
                            icon_: "motion"
                            text: "Animate to " + node.name
                            visible: node.modelData !== ""
                            onClicked: panel.edit({
                                action: "add_transition",
                                from: "*",
                                to: node.modelData,
                                animations: "NumberAnimation { properties: \"x,y,width,height,opacity\"; duration: " + Motion.durationMedium + " }"
                            })
                        }
                        TextButton {
                            text: "+"
                            enabled: panel.selected !== ""
                            Accessible.name: panel.trigger === "tapped"
                                             ? "Tap selection to go to " + node.name
                                             : "On " + panel.trigger + " of selection go to " + node.name
                            onClicked: {
                                const args = {
                                    action: "interaction",
                                    trigger: {node: panel.workspace.selected},
                                    state: node.modelData
                                }
                                if (panel.trigger !== "tapped")
                                    args.signal = panel.trigger
                                panel.edit(args)
                            }
                        }
                    }
                }
            }
        }

        Repeater {
            model: panel.flow.edges !== undefined ? panel.flow.edges : []
            delegate: RowLayout {
                id: edge
                required property var modelData
                Layout.fillWidth: true
                spacing: Spacing.xxs

                Label {
                    text: (edge.modelData.from === "*" ? "any" : panel.stateName(edge.modelData.from))
                          + " → " + panel.stateName(edge.modelData.to)
                          + (edge.modelData.kind === "interaction" ? " · " + edge.modelData.label : "")
                    color: Colors.text
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                    Accessible.role: Accessible.StaticText
                    Accessible.name: "Flow edge " + edge.modelData.label
                }
                TextButton {
                    text: "Remove"
                    Accessible.name: "Remove flow " + edge.modelData.label
                    onClicked: {
                        if (edge.modelData.kind !== "interaction") {
                            panel.edit({action: "remove_transition", from: edge.modelData.from, to: edge.modelData.to})
                            return
                        }
                        const args = {action: "remove_interaction", trigger: {node: edge.modelData.trigger}}
                        if (edge.modelData.signal !== "")
                            args.signal = edge.modelData.signal
                        panel.edit(args)
                    }
                }
            }
        }

        Item {
            Layout.fillHeight: true
        }
    }
}
