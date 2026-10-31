// The open document's objects in order, indented by depth. Selecting one
// is `node_select`, as an agent selects. A layer with open review comments
// says how many.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Panel {
    id: panel
    required property Workspace workspace
    title: "Layers"

    readonly property var layer: {
        const layers = workspace.layers
        for (let i = 0; i < layers.length; ++i) {
            if (layers[i].node === workspace.selected)
                return layers[i]
        }
        return null
    }
    // How many objects are written in the selected one's parent.
    readonly property int siblings: {
        if (layer === null || layer.index === null)
            return 0
        let n = 0
        const layers = workspace.layers
        for (let i = 0; i < layers.length; ++i) {
            if (layers[i].parent === layer.parent && layers[i].index !== null)
                ++n
        }
        return n
    }

    function move(by) {
        workspace.dispatch("node_move", {
            doc: workspace.doc,
            node: {node: layer.node},
            parent: {node: layer.parent},
            index: layer.index + by
        })
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        ListView {
            objectName: "layers"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: panel.workspace.layers
            Accessible.role: Accessible.List
            Accessible.name: "Layers"

            delegate: ItemDelegate {
                id: row
                required property var modelData
                width: ListView.view.width
                leftPadding: Spacing.xs + modelData.depth * Spacing.md
                text: modelData.label
                Accessible.name: modelData.label
                highlighted: panel.workspace.chosen.indexOf(modelData.node) >= 0
                onClicked: panel.workspace.dispatch("node_select", {
                    doc: panel.workspace.doc,
                    nodes: [{node: modelData.node}]
                })

                TapHandler {
                    acceptedModifiers: Qt.ShiftModifier
                    onTapped: panel.workspace.dispatch("node_select", {
                        doc: panel.workspace.doc,
                        nodes: [{node: row.modelData.node}],
                        add: true
                    })
                }

                Label {
                    anchors.right: parent.right
                    anchors.rightMargin: Spacing.xs
                    anchors.verticalCenter: parent.verticalCenter
                    visible: row.modelData.comments > 0
                    text: String(row.modelData.comments)
                    color: Colors.text
                    Accessible.role: Accessible.StaticText
                    Accessible.name: row.modelData.comments
                                     + (row.modelData.comments === 1 ? " comment on " : " comments on ")
                                     + row.modelData.label
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: Spacing.xxs

            Button {
                text: "Move up"
                Accessible.name: text
                enabled: panel.layer !== null && panel.layer.index !== null && panel.layer.index > 0
                onClicked: panel.move(-1)
            }
            Button {
                text: "Move down"
                Accessible.name: text
                enabled: panel.layer !== null && panel.layer.index !== null
                         && panel.layer.index < panel.siblings - 1
                onClicked: panel.move(1)
            }
            Button {
                text: "Remove"
                Accessible.name: text
                enabled: panel.layer !== null && panel.layer.parent !== ""
                onClicked: panel.workspace.dispatch("node_remove", {
                    doc: panel.workspace.doc,
                    node: {node: panel.layer.node}
                })
            }
        }
    }
}
