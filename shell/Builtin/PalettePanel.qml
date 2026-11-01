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
    // first object must still go somewhere. A type that draws its own content
    // shows nothing put inside it, so what is placed goes beside it.
    readonly property string target: {
        const picked = selectedLayer
        if (picked === null)
            return workspace.layers.length > 0 ? workspace.layers[0].node : ""
        const bare = picked.type.split(".").pop()
        return picked.parent !== "" && panel.drawsItsOwn.includes(bare) ? picked.parent : picked.node
    }
    readonly property var drawsItsOwn: ["Text", "TextInput", "TextEdit", "Image", "AnimatedImage",
        "BorderImage", "Canvas", "Button", "CheckBox", "RadioButton", "Switch", "Slider",
        "ProgressBar", "SpinBox", "TextField", "TextArea", "ComboBox", "Label", "ToolButton"]
    readonly property string needle: search.text.toLowerCase()
    // A string, which changes only when the list does: `toolkit` is a new
    // value at each publish, and rebuilding the buttons would drop a press.
    readonly property string toolkit: JSON.stringify(workspace.toolkit)
    // By what they are for, which is how a person looks for them; the module
    // a type comes from goes along, for the import.
    readonly property var groups: {
        const needle = panel.needle
        const order = ["This project", "Basics", "Layout", "Controls", "Input", "Motion", "3D", "More"]
        const out = order.map(name => ({name: name, types: []}))
        // A file beside the document is a type it can use with no import, and
        // the one open cannot hold itself. A file in another directory needs
        // an import of that directory, which `node_insert` does not write.
        const beside = panel.workspace.doc.substring(0, panel.workspace.doc.lastIndexOf("/") + 1)
        for (const doc of panel.workspace.documents) {
            const name = doc.substring(beside.length, doc.length - 4)
            if (doc !== panel.workspace.doc && doc.startsWith(beside) && doc.endsWith(".qml")
                    && /^[A-Z][A-Za-z0-9_]*$/.test(name) && name.toLowerCase().includes(needle))
                out[0].types.push({name: name, module: ""})
        }
        for (const g of JSON.parse(toolkit)) {
            for (const t of g.types) {
                if (t.toLowerCase().includes(needle))
                    out[order.indexOf(panel.category(t, g.module))].types.push({name: t, module: g.module})
            }
        }
        // What most people want first goes first in its group.
        const first = ["Rectangle", "Text", "Image", "Row", "Column", "Grid", "Button", "Label", "TextField"]
        const rank = t => first.indexOf(t.name) < 0 ? first.length : first.indexOf(t.name)
        for (const g of out)
            g.types.sort((a, b) => rank(a) - rank(b))
        return out.filter(g => g.types.length > 0)
    }
    function category(type, module) {
        if (["Rectangle", "Text", "Image", "Item", "TextInput", "TextEdit", "AnimatedImage", "BorderImage", "Canvas"].includes(type))
            return "Basics"
        if (/^(Row|Column|Grid|Flow|Stack)(Layout|View)?$|^(Flickable|Positioner|ListView|GridView|PathView|Repeater|Loader)$/.test(type))
            return "Layout"
        if (module.includes("3D"))
            return "3D"
        if (/Handler$|^(MouseArea|DropArea|Keys|Shortcut|FocusScope)$/.test(type))
            return "Input"
        if (/Animat|Transition|Behavior|^State|Timeline|Keyframe/.test(type))
            return "Motion"
        if (module.includes("Controls"))
            return "Controls"
        return "More"
    }
    // A picture's url as the open document writes it: urls resolve from the
    // file that says them.
    function fromDocument(url) {
        return "../".repeat(panel.workspace.doc.split("/").length - 1) + url
    }
    // What a tile draws of it: its kind, in the look of the thing.
    function typeIcon(type) {
        const known = {
            Text: "text", Label: "text", TextEdit: "text", TextInput: "field",
            TextField: "field", TextArea: "field", Image: "image", AnimatedImage: "image",
            BorderImage: "image", Row: "row", RowLayout: "row", Column: "column",
            ColumnLayout: "column", Grid: "grid", GridLayout: "grid", Flow: "grid",
            Rectangle: "frame", Item: "frame", Flickable: "frame", Pane: "frame", Frame: "frame",
            Button: "button", ToolButton: "button", RoundButton: "button", CheckBox: "checkbox",
            RadioButton: "radio", Switch: "toggle", Slider: "slider", RangeSlider: "slider",
            ProgressBar: "progress", ComboBox: "combo", SpinBox: "combo", TabBar: "tabs",
            TabButton: "tabs", Dialog: "window", Popup: "window", Drawer: "window",
            ApplicationWindow: "window", ListView: "list", GridView: "grid", Repeater: "list",
            Transition: "motion", State: "logic", Timer: "logic", Loader: "component"
        }
        if (known[type] !== undefined)
            return known[type]
        const kind = category(type, "")
        return /Handler$|^MouseArea$/.test(type) ? "pointer"
            : kind === "Motion" ? "motion"
            : /Camera|Light|Model|Node|View3D|Material/.test(type) ? "cube" : "component"
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
        // A view with nothing to see draws one colour, and a model without
        // a material or a camera without distance shows nothing either.
        case "View3D":
            return "View3D {\n    width: 320\n    height: 240\n    environment: SceneEnvironment {\n        backgroundMode: SceneEnvironment.Color\n        clearColor: \"#20242c\"\n    }\n\n    PerspectiveCamera {\n        z: 300\n    }\n\n    DirectionalLight {\n        eulerRotation.x: -30\n    }\n\n    Model {\n        source: \"#Cube\"\n        eulerRotation: Qt.vector3d(30, 30, 0)\n        materials: PrincipledMaterial {\n            baseColor: \"#40c080\"\n        }\n    }\n}"
        case "Model":
            return "Model {\n    source: \"#Cube\"\n    materials: PrincipledMaterial {\n        baseColor: \"#40c080\"\n    }\n}"
        case "PerspectiveCamera":
        case "OrthographicCamera":
            return name + " {\n    z: 300\n}"
        default:
            return name + " {\n}"
        }
    }

    Field {
        id: search
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: Spacing.xs
        y: Spacing.xs
        placeholderText: "Search"
        Accessible.name: "Search the palette"
    }

    ListView {
        objectName: "palette"
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: search.bottom
        anchors.bottom: parent.bottom
        anchors.margins: Spacing.xs
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
                    font.pixelSize: Typography.captionSize
                    font.weight: Typography.bodyStrongWeight
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                    leftPadding: Spacing.xxs
                    ToolTip.visible: hover.hovered && row.modelData.summary !== ""
                    ToolTip.text: row.modelData.summary
                    HoverHandler {
                        id: hover
                    }
                }
                IconButton {
                    icon_: "plus"
                    text: "Place " + row.modelData.name
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

                        Field {
                            id: contribution
                            placeholderText: "Text { text: \"…\" }"
                            Accessible.name: "Contribution to " + slotRow.modelData
                            Layout.fillWidth: true
                            onAccepted: contribute.clicked()
                        }
                        TextButton {
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

            Label {
                visible: panel.groups.length === 0 && panel.needle !== ""
                width: parent.width
                topPadding: Spacing.sm
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
                text: "Nothing called \"" + search.text + "\""
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            Section {
                id: assetsSection
                title: "Assets"
                width: parent.width

                Flow {
                    Layout.fillWidth: true
                    spacing: Spacing.xxs

                    Repeater {
                        model: panel.workspace.assets
                        delegate: AbstractButton {
                            id: picture
                            required property var modelData
                            visible: modelData.name.toLowerCase().includes(panel.needle)
                            text: modelData.name
                            Accessible.name: "Place " + modelData.name
                            Accessible.role: Accessible.Button
                            enabled: panel.target !== ""
                            width: (assetsSection.width - 2 * Spacing.xxs) / 3
                            height: 68
                            ToolTip.visible: hovered
                            ToolTip.text: "Place " + modelData.name
                            ToolTip.delay: 500
                            background: Rectangle {
                                radius: 6
                                color: picture.down ? Colors.surfacePressed
                                    : picture.hovered ? Colors.surfaceHover : Colors.surfaceRaised
                                border.width: picture.visualFocus ? 2 : 1
                                border.color: picture.visualFocus ? Colors.focusRing : Colors.surfacePressed
                            }
                            contentItem: Column {
                                spacing: Spacing.xxs
                                topPadding: Spacing.xxs
                                Item {
                                    width: parent.width
                                    height: 40
                                    Image {
                                        id: thumbnail
                                        anchors.fill: parent
                                        anchors.margins: 2
                                        source: picture.modelData.file
                                        sourceSize.width: 96
                                        sourceSize.height: 96
                                        fillMode: Image.PreserveAspectFit
                                        asynchronous: true
                                    }
                                    Icon {
                                        anchors.centerIn: parent
                                        visible: thumbnail.status !== Image.Ready
                                        name: "image"
                                        size: 24
                                        color: Colors.textMuted
                                    }
                                }
                                Text {
                                    width: parent.width
                                    leftPadding: 2
                                    rightPadding: 2
                                    text: picture.modelData.name
                                    color: picture.enabled ? Colors.text : Colors.textDisabled
                                    font.pixelSize: Typography.captionSize
                                    horizontalAlignment: Text.AlignHCenter
                                    elide: Text.ElideRight
                                }
                            }
                            onClicked: panel.workspace.dispatch("node_insert", {
                                doc: panel.workspace.doc,
                                parent: {node: panel.target},
                                import: "QtQuick",
                                text: "Image {\n    source: \"" + panel.fromDocument(modelData.url) + "\"\n}"
                            })
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: Spacing.xxs
                    spacing: Spacing.xxs

                    Field {
                        id: incoming
                        placeholderText: "Path of a file to import"
                        Accessible.name: "Path of a file to import"
                        Layout.fillWidth: true
                        onAccepted: bring.clicked()
                    }
                    TextButton {
                        id: bring
                        text: "Import"
                        Accessible.name: "Import asset"
                        enabled: incoming.text !== ""
                        onClicked: {
                            panel.workspace.dispatch("project_open", {asset: "import", file: incoming.text})
                            incoming.text = ""
                        }
                    }
                }
            }

            Repeater {
                model: panel.groups
                delegate: Column {
                    id: group
                    required property var modelData
                    width: parent.width
                    spacing: Spacing.xxs

                    Label {
                        text: group.modelData.name
                        color: Colors.textMuted
                        font.pixelSize: Typography.captionSize
                        font.weight: Typography.bodyStrongWeight
                        topPadding: Spacing.xs
                        Accessible.role: Accessible.StaticText
                        Accessible.name: group.modelData.name
                    }
                    Flow {
                        width: parent.width
                        spacing: Spacing.xxs

                        Repeater {
                            model: group.modelData.types
                            delegate: AbstractButton {
                                id: tile
                                required property var modelData
                                text: modelData.name
                                Accessible.name: "Place " + modelData.name
                                Accessible.description: modelData.module
                                Accessible.role: Accessible.Button
                                enabled: panel.target !== ""
                                width: (group.width - 2 * Spacing.xxs) / 3
                                height: 68
                                ToolTip.visible: hovered
                                ToolTip.text: "Place " + modelData.name + " (" + (modelData.module || "this project") + ")"
                                ToolTip.delay: 500
                                background: Rectangle {
                                    radius: 6
                                    color: tile.down ? Colors.surfacePressed
                                        : tile.hovered ? Colors.surfaceHover : Colors.surfaceRaised
                                    border.width: tile.visualFocus ? 2 : 1
                                    border.color: tile.visualFocus ? Colors.focusRing : Colors.surfacePressed
                                }
                                contentItem: Column {
                                    spacing: Spacing.xxs
                                    topPadding: Spacing.xs
                                    Icon {
                                        anchors.horizontalCenter: parent.horizontalCenter
                                        name: panel.typeIcon(tile.modelData.name)
                                        size: 24
                                        color: tile.enabled ? Colors.text : Colors.textDisabled
                                    }
                                    Text {
                                        width: parent.width
                                        leftPadding: 2
                                        rightPadding: 2
                                        text: tile.modelData.name
                                        color: tile.enabled ? Colors.text : Colors.textDisabled
                                        font.pixelSize: Typography.captionSize
                                        horizontalAlignment: Text.AlignHCenter
                                        elide: Text.ElideRight
                                    }
                                }
                                onClicked: {
                                    const place = {
                                        doc: panel.workspace.doc,
                                        parent: {node: panel.target},
                                        text: panel.starter(modelData.name)
                                    }
                                    if (modelData.module !== "")
                                        place.import = modelData.module
                                    panel.workspace.dispatch("node_insert", place)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
