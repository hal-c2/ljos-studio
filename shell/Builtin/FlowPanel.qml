// The open document's flow as a graph: the root's states are its nodes,
// the base state first, and its transitions and interactions its edges.
// Each state wires the selected object to go to it when tapped, and each
// interaction is taken out again, as `state_edit` does for an agent.
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
                            Accessible.name: "Tap selection to go to " + node.name
                            onClicked: panel.edit({
                                action: "interaction",
                                trigger: {node: panel.workspace.selected},
                                state: node.modelData
                            })
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
