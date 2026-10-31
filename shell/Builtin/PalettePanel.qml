// The project's blocks and the toolkit's types. "Place" puts one into the
// selected object. Each block lists the slots it offers; while one of its
// instances is selected, a contribution written in QML goes beside it
// (`slot_contribute`). A type placed gets what it needs to be seen, and its
// module's import if the document has none: `node_insert` adds it.
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
    // The selection, or the root: a new document has nothing selected and a
    // first object must still go somewhere.
    readonly property string target: workspace.selected !== "" ? workspace.selected
        : (workspace.layers.length > 0 ? workspace.layers[0].node : "")
    readonly property string needle: search.text.toLowerCase()
    // A string, which changes only when the list does: `toolkit` is a new
    // value at each publish, and rebuilding the buttons would drop a press.
    readonly property string toolkit: JSON.stringify(workspace.toolkit)
    readonly property var groups: {
        const needle = panel.needle
        return JSON.parse(toolkit).map(g => ({
            module: g.module,
            types: g.types.filter(t => t.toLowerCase().includes(needle))
        })).filter(g => g.types.length > 0)
    }

    // Starting with nothing, a Rectangle has no size and a Text no words, and
    // the person sees nothing happen.
    function starter(name) {
        switch (name) {
        case "Rectangle":
            return "Rectangle {\n    width: 100\n    height: 100\n    color: \"#cccccc\"\n}"
        case "Text":
        case "Label":
            return name + " {\n    text: \"" + name + "\"\n}"
        case "Button":
        case "CheckBox":
        case "RadioButton":
        case "Switch":
            return name + " {\n    text: \"" + name + "\"\n}"
        case "Image":
        case "Item":
            return name + " {\n    width: 100\n    height: 100\n}"
        default:
            return name + " {\n}"
        }
    }

    TextField {
        id: search
        anchors.left: parent.left
        anchors.right: parent.right
        placeholderText: "Search"
        Accessible.name: "Search the palette"
    }

    ListView {
        objectName: "palette"
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: search.bottom
        anchors.bottom: parent.bottom
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
            readonly property bool shown: row.modelData.name.toLowerCase().includes(panel.needle)
            visible: shown
            height: shown ? implicitHeight : 0
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

        footer: Column {
            width: ListView.view.width
            spacing: Spacing.xs

            Repeater {
                model: panel.groups
                delegate: Column {
                    id: group
                    required property var modelData
                    width: parent.width
                    spacing: Spacing.xxs

                    Label {
                        text: group.modelData.module
                        color: Colors.text
                        font.pixelSize: Typography.captionSize
                        leftPadding: Spacing.xs
                        Accessible.role: Accessible.StaticText
                        Accessible.name: group.modelData.module
                    }
                    Flow {
                        width: parent.width
                        spacing: Spacing.xxs

                        Repeater {
                            model: group.modelData.types
                            delegate: Button {
                                required property string modelData
                                text: modelData
                                Accessible.name: "Place " + modelData
                                Accessible.description: group.modelData.module
                                enabled: panel.target !== ""
                                onClicked: panel.workspace.dispatch("node_insert", {
                                    doc: panel.workspace.doc,
                                    parent: {node: panel.target},
                                    text: panel.starter(modelData),
                                    import: group.modelData.module
                                })
                            }
                        }
                    }
                }
            }
        }
    }
}
