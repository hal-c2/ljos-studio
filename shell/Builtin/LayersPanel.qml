// The open document's objects in order, indented by depth. Selecting one
// is `node_select`, as an agent selects. A layer with open review comments
// says how many, and one another writer holds says who: the person's edit
// of it is refused until they let go.
//
// A row dragged onto another takes its place (`node_move`); a double-click
// names the object (`node_arrange` `rename`). The eye and the lock are
// `node_arrange` too, Studio's own: the file does not hear of them.
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

    // The row being named, by its handle; empty when none is.
    property string naming: ""

    // Drops `from` where the row at `y` in the list is: written in the same
    // object at the same place, so it takes that row's place. Dropped on the
    // root, it becomes the root's last object.
    function drop(from, y) {
        const at = list.indexAt(0, y + list.contentY)
        if (at < 0)
            return
        const onto = workspace.layers[at]
        if (onto.node === from.node)
            return
        const args = {
            doc: workspace.doc,
            node: {node: from.node}
        }
        if (onto.parent === "") {
            args.parent = {node: onto.node}
        } else if (onto.index !== null) {
            args.parent = {node: onto.parent}
            args.index = onto.index
        } else {
            return
        }
        workspace.dispatch("node_move", args)
    }

    function arrange(action, node) {
        workspace.dispatch("node_arrange", {
            action: action,
            doc: workspace.doc,
            nodes: [{node: node}]
        })
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        ListView {
            id: list
            objectName: "layers"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: panel.workspace.layers
            Accessible.role: Accessible.List
            Accessible.name: "Layers"
            ScrollBar.vertical: ScrollBar {
                Accessible.name: "Scroll the layers"
                // As the documents' bar: nothing else says there is more.
                policy: ScrollBar.AlwaysOn
                visible: list.contentHeight > list.height
            }

            delegate: ItemDelegate {
                id: row
                required property var modelData
                width: ListView.view.width
                leftPadding: Spacing.xs + modelData.depth * Spacing.md
                text: modelData.label
                Accessible.name: modelData.label
                highlighted: panel.workspace.chosen.indexOf(modelData.node) >= 0
                opacity: modelData.hidden ? 0.5 : 1
                onClicked: {
                    panel.naming = ""
                    panel.workspace.dispatch("node_select", {
                        doc: panel.workspace.doc,
                        nodes: [{node: modelData.node}]
                    })
                }
                onDoubleClicked: panel.naming = modelData.node

                DragHandler {
                    target: null
                    grabPermissions: PointerHandler.CanTakeOverFromAnything
                    enabled: row.modelData.parent !== ""
                    onActiveChanged: {
                        if (!active)
                            panel.drop(row.modelData, row.mapToItem(list, centroid.position.x, centroid.position.y).y)
                    }
                }

                TextField {
                    visible: panel.naming === row.modelData.node
                    anchors.fill: parent
                    anchors.leftMargin: row.leftPadding
                    text: row.modelData.id
                    Accessible.name: "Name of " + row.modelData.label
                    onVisibleChanged: if (visible) forceActiveFocus()
                    // Left open it covers its row, and clicks there would select nothing.
                    onActiveFocusChanged: if (!activeFocus && panel.naming === row.modelData.node) panel.naming = ""
                    onAccepted: {
                        panel.naming = ""
                        if (text !== row.modelData.id) {
                            panel.workspace.dispatch("node_arrange", {
                                action: "rename",
                                doc: panel.workspace.doc,
                                nodes: [{node: row.modelData.node}],
                                name: text
                            })
                        }
                    }
                    Keys.onEscapePressed: panel.naming = ""
                }

                TapHandler {
                    acceptedModifiers: Qt.ShiftModifier
                    onTapped: panel.workspace.dispatch("node_select", {
                        doc: panel.workspace.doc,
                        nodes: [{node: row.modelData.node}],
                        add: true
                    })
                }

                Row {
                    anchors.right: parent.right
                    anchors.rightMargin: Spacing.xs
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: Spacing.xs

                    Label {
                        objectName: "held"
                        visible: row.modelData.held !== ""
                        text: "held by " + row.modelData.held
                        color: Colors.text
                        font.pixelSize: Typography.captionSize
                        Accessible.role: Accessible.StaticText
                        Accessible.name: row.modelData.label + " held by " + row.modelData.held
                    }
                    Label {
                        visible: row.modelData.comments > 0
                        text: String(row.modelData.comments)
                        color: Colors.text
                        Accessible.role: Accessible.StaticText
                        Accessible.name: row.modelData.comments
                                         + (row.modelData.comments === 1 ? " comment on " : " comments on ")
                                         + row.modelData.label
                    }
                    // The preview finds what it hides by its id.
                    ToolButton {
                        visible: row.modelData.id !== ""
                        text: row.modelData.hidden ? "Show" : "Hide"
                        font.pixelSize: Typography.captionSize
                        Accessible.name: text + " " + row.modelData.label
                        onClicked: panel.arrange(row.modelData.hidden ? "show" : "hide", row.modelData.node)
                    }
                    ToolButton {
                        text: row.modelData.locked ? "Unlock" : "Lock"
                        font.pixelSize: Typography.captionSize
                        Accessible.name: text + " " + row.modelData.label
                        onClicked: panel.arrange(row.modelData.locked ? "unlock" : "lock", row.modelData.node)
                    }
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
