// The project's blocks. "Place" puts one into the selected object. Each
// lists the slots it offers; while one of its instances is selected, a
// contribution written in QML goes beside it (`slot_contribute`).
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Panel {
    id: panel
    required property Workspace workspace
    title: "Palette"

    function layer(node) {
        const all = workspace.layers
        for (let i = 0; i < all.length; i++) {
            if (all[i].node === node)
                return all[i]
        }
        return null
    }
    readonly property var selectedLayer: layer(workspace.selected)

    ListView {
        objectName: "palette"
        anchors.fill: parent
        clip: true
        model: panel.workspace.palette
        Accessible.role: Accessible.List
        Accessible.name: "Palette"

        delegate: ColumnLayout {
            id: row
            required property var modelData
            // An instance of this block is selected: its slots take
            // contributions.
            readonly property bool chosen: panel.selectedLayer !== null
                && panel.selectedLayer.type === modelData.name
                && panel.selectedLayer.parent !== ""
            width: ListView.view.width
            spacing: Spacing.xxs

            RowLayout {
                Layout.fillWidth: true
                spacing: Spacing.xxs

                Label {
                    text: row.modelData.name
                    color: Colors.text
                    Accessible.role: Accessible.StaticText
                    Accessible.name: row.modelData.name
                    Accessible.description: row.modelData.summary
                    Layout.fillWidth: true
                    leftPadding: Spacing.xs
                }
                Button {
                    text: "Place"
                    Accessible.name: "Place " + row.modelData.name
                    enabled: panel.workspace.selected !== ""
                    onClicked: panel.workspace.dispatch("node_insert", {
                        doc: panel.workspace.doc,
                        parent: {node: panel.workspace.selected},
                        block: row.modelData.module + "." + row.modelData.name
                    })
                }
            }

            Repeater {
                model: row.modelData.slots
                delegate: ColumnLayout {
                    id: slotRow
                    required property string modelData
                    Layout.fillWidth: true
                    spacing: Spacing.xxs

                    Label {
                        text: "slot " + slotRow.modelData
                        color: Colors.text
                        font.pixelSize: Typography.captionSize
                        leftPadding: Spacing.md
                        Accessible.role: Accessible.StaticText
                        Accessible.name: "Slot " + slotRow.modelData + " of " + row.modelData.name
                    }
                    RowLayout {
                        visible: row.chosen
                        Layout.fillWidth: true
                        spacing: Spacing.xxs

                        TextField {
                            id: contribution
                            placeholderText: "Text { text: \"…\" }"
                            Accessible.name: "Contribution to " + slotRow.modelData
                            Layout.fillWidth: true
                            onAccepted: contribute.clicked()
                        }
                        Button {
                            id: contribute
                            text: "Contribute"
                            Accessible.name: "Contribute to " + slotRow.modelData
                            enabled: contribution.text !== ""
                            // Beside the instance: slots are filled by name
                            // from anywhere in the shell (ADR 0003).
                            onClicked: {
                                panel.workspace.dispatch("slot_contribute", {
                                    doc: panel.workspace.doc,
                                    parent: {node: panel.selectedLayer.parent},
                                    slot: slotRow.modelData,
                                    text: contribution.text
                                })
                                contribution.text = ""
                            }
                        }
                    }
                }
            }
        }
    }
}
