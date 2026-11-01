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

    // What kind of thing a row is at a glance; a type of the project's own is
    // a component.
    function typeIcon(type) {
        const known = {
            Text: "text", Label: "text", TextField: "text", TextArea: "text",
            Image: "image", Row: "row", RowLayout: "row", Column: "column",
            ColumnLayout: "column", Grid: "grid", GridLayout: "grid", Flow: "grid",
            Rectangle: "frame", Item: "frame", Flickable: "frame", Pane: "frame",
            Button: "control", ToolButton: "control", CheckBox: "control", Switch: "control",
            Slider: "control", ListView: "list", GridView: "grid", Repeater: "list",
            View3D: "cube", Model: "cube", Node: "cube"
        }
        return known[type] || "component"
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
                implicitHeight: 28
                topPadding: 0
                bottomPadding: 0
                leftPadding: Spacing.xs + modelData.depth * Spacing.sm
                rightPadding: trailing.width + Spacing.xxs
                text: modelData.label
                contentItem: RowLayout {
                    spacing: Spacing.xxs
                    Icon {
                        name: panel.typeIcon(row.modelData.type)
                        color: row.highlighted ? Colors.accent : Colors.textMuted
                    }
                    Text {
                        text: row.text
                        color: Colors.text
                        font.pixelSize: Typography.captionSize
                        font.weight: row.highlighted ? Typography.bodyStrongWeight : Typography.bodyWeight
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }
                background: Rectangle {
                    color: row.highlighted ? Colors.surfaceSelected
                        : row.hovered ? Colors.surfaceHover : "transparent"
                }
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

                Field {
                    visible: panel.naming === row.modelData.node
                    anchors.fill: parent
                    anchors.leftMargin: row.leftPadding
                    text: row.modelData.id
                    Accessible.name: "Name of " + row.modelData.label
                    // What was typed and left is not offered again.
                    // Hidden, it would keep the keys, and the next Return would name the row left.
                    onVisibleChanged: {
                        if (visible) {
                            text = row.modelData.id
                            forceActiveFocus()
                        } else {
                            focus = false
                        }
                    }
                    // Left open it covers its row, and clicks there would select nothing.
                    // Another window taking the focus is a trip, to copy a name.
                    onActiveFocusChanged: if (!activeFocus && Window.active && panel.naming === row.modelData.node) panel.naming = ""
                    onAccepted: {
                        if (!visible)
                            return
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
                    id: trailing
                    anchors.right: parent.right
                    anchors.rightMargin: Spacing.xxs
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 0

                    Label {
                        objectName: "held"
                        visible: row.modelData.held !== ""
                        anchors.verticalCenter: parent.verticalCenter
                        text: "held by " + row.modelData.held
                        color: Colors.text
                        font.pixelSize: Typography.captionSize
                        rightPadding: Spacing.xxs
                        Accessible.role: Accessible.StaticText
                        Accessible.name: row.modelData.label + " held by " + row.modelData.held
                    }
                    Label {
                        visible: row.modelData.comments > 0
                        anchors.verticalCenter: parent.verticalCenter
                        text: String(row.modelData.comments)
                        color: Colors.text
                        font.pixelSize: Typography.captionSize
                        rightPadding: Spacing.xxs
                        Accessible.role: Accessible.StaticText
                        Accessible.name: row.modelData.comments
                                         + (row.modelData.comments === 1 ? " comment on " : " comments on ")
                                         + row.modelData.label
                    }
                    // The preview finds what it hides by its id.
                    IconButton {
                        visible: row.modelData.id !== ""
                        implicitWidth: 24
                        implicitHeight: 24
                        icon_: row.modelData.hidden ? "eyeOff" : "eye"
                        text: (row.modelData.hidden ? "Show" : "Hide") + " " + row.modelData.label
                        onClicked: panel.arrange(row.modelData.hidden ? "show" : "hide", row.modelData.node)
                    }
                    IconButton {
                        implicitWidth: 24
                        implicitHeight: 24
                        icon_: row.modelData.locked ? "lock" : "unlock"
                        text: (row.modelData.locked ? "Unlock" : "Lock") + " " + row.modelData.label
                        onClicked: panel.arrange(row.modelData.locked ? "unlock" : "lock", row.modelData.node)
                    }
                }
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.margins: Spacing.xxs
            spacing: 2

            IconButton {
                icon_: "up"
                text: "Move up"
                enabled: panel.layer !== null && panel.layer.index !== null && panel.layer.index > 0
                onClicked: panel.move(-1)
            }
            IconButton {
                icon_: "down"
                text: "Move down"
                enabled: panel.layer !== null && panel.layer.index !== null
                         && panel.layer.index < panel.siblings - 1
                onClicked: panel.move(1)
            }
            Item {
                Layout.fillWidth: true
            }
            IconButton {
                icon_: "trash"
                text: "Remove"
                enabled: panel.layer !== null && panel.layer.parent !== ""
                onClicked: panel.workspace.dispatch("node_remove", {
                    doc: panel.workspace.doc,
                    node: {node: panel.layer.node}
                })
            }
        }
    }
}
