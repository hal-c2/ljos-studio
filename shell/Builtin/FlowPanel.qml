// The open document's flow as a graph: the root's states are its nodes,
// the base state first, and its transitions and interactions its edges.
// Each state wires the selected object to go to it when tapped, or when
// one of its own signals is emitted (picked above the graph from those
// `node_inspect` lists, change signals left out), and each interaction is
// taken out again, as `state_edit` does for an agent.
//
// In play mode (the canvas's "Play") a click in a frame clicks the running
// app; the state `preview_state` then reads is the node marked playing.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
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
    readonly property string trigger: triggerList.currentIndex > 0
                                      ? triggers[triggerList.currentIndex] : "tapped"
    readonly property string selected: workspace.selected
    onSelectedChanged: triggerList.currentIndex = 0

    function stateName(state) {
        return state === "" ? "Base" : state
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

        // A short list however many signals the selection has: a MouseArea
        // has ten, which as a row of buttons would push the graph out of
        // the panel.
        ListView {
            id: triggerList
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentHeight, 72)
            clip: true
            visible: panel.workspace.selected !== ""
            model: panel.triggers
            Accessible.role: Accessible.List
            Accessible.name: "Wire"

            delegate: ItemDelegate {
                required property string modelData
                required property int index
                width: ListView.view.width
                text: "Wire " + modelData
                Accessible.name: modelData
                highlighted: index === triggerList.currentIndex
                onClicked: triggerList.currentIndex = index
            }
        }

        Flow {
            Layout.fillWidth: true
            spacing: Spacing.xs

            Repeater {
                model: panel.flow.states !== undefined ? panel.flow.states : []
                delegate: Rectangle {
                    id: node
                    required property string modelData
                    readonly property string name: panel.stateName(modelData)
                    width: nodeRow.implicitWidth + 2 * Spacing.xs
                    height: nodeRow.implicitHeight + 2 * Spacing.xxs
                    color: panel.playing === modelData ? Colors.accent : Colors.surface
                    border.color: Colors.border
                    Accessible.role: Accessible.StaticText
                    Accessible.name: "Flow state " + name

                    RowLayout {
                        id: nodeRow
                        anchors.centerIn: parent
                        spacing: Spacing.xxs

                        Label {
                            text: node.name
                            color: Colors.text
                        }
                        Button {
                            text: "+"
                            enabled: panel.workspace.selected !== ""
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
                Button {
                    visible: edge.modelData.kind === "interaction"
                    text: "Remove"
                    Accessible.name: "Remove flow " + edge.modelData.label
                    onClicked: {
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
