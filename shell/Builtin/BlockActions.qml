// What a block offers to be skinned with, and the two gestures that change
// it: "Add slot" declares a slot in the selected object (the root when none
// is), "Make a shell" writes a document that skins every slot of the block.
// Each is the call an agent makes over MCP: `node_insert` with `slot`, and
// `document_new` with `shell`. Shown only while the open document is a
// block; where it stands is the shell's choice (the built-in one puts it
// under the documents).
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Column {
    id: actions
    required property Workspace workspace

    // The palette's row of the block the open document is, or null.
    readonly property var block: {
        const all = workspace.palette || []
        for (let i = 0; i < all.length; i++)
            if (all[i].doc !== "" && all[i].doc === workspace.doc)
                return all[i]
        return null
    }
    readonly property var slots: block === null ? [] : block.slots
    // The tool asked and not yet answered, so another panel's answer is not
    // taken for this one's.
    property string asked: ""
    property string shell: ""
    property string refusal: ""

    visible: block !== null
    spacing: Spacing.xxs
    Accessible.role: Accessible.Pane
    Accessible.name: "Block"

    function root() {
        const layers = workspace.layers || []
        return layers.length > 0 ? layers[0].node : ""
    }

    function addSlot(name) {
        const parent = workspace.selected !== "" ? workspace.selected : root()
        if (name === "" || parent === "")
            return
        refusal = ""
        asked = "node_insert"
        workspace.dispatch("node_insert", {doc: workspace.doc, parent: {node: parent}, slot: name})
    }

    function makeShell() {
        if (block === null)
            return
        refusal = ""
        asked = "document_new"
        shell = block.name + "Shell.qml"
        workspace.dispatch("document_new", {doc: shell, shell: block.module + "." + block.name})
    }

    Connections {
        target: actions.workspace
        function onAnswered(tool, answer) {
            if (tool !== actions.asked)
                return
            actions.asked = ""
            if (tool === "node_insert") {
                slotName.text = ""
            } else if (tool === "document_new") {
                actions.workspace.doc = actions.shell
                if (actions.workspace.previews)
                    actions.workspace.dispatch("preview_open", {doc: actions.shell})
            }
        }
        function onRefused(tool, error) {
            if (tool !== actions.asked)
                return
            actions.asked = ""
            actions.refusal = error.message
        }
    }

    Label {
        width: actions.width
        text: actions.slots.length === 0
            ? "No slots yet: a slot lets a shell or an instance show its own look here."
            : "Slots: " + actions.slots.join(", ")
        wrapMode: Text.Wrap
        color: Colors.textMuted
        font.pixelSize: Typography.captionSize
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    Row {
        spacing: Spacing.xxs
        Field {
            id: slotName
            width: actions.width - addSlot.width - Spacing.xxs
            placeholderText: "Slot name"
            Accessible.name: "New slot's name"
            onAccepted: addSlot.clicked()
        }
        TextButton {
            id: addSlot
            text: "Add slot"
            Accessible.name: "Add slot"
            enabled: slotName.text !== "" && actions.asked === ""
            onClicked: actions.addSlot(slotName.text)
        }
    }

    TextButton {
        width: actions.width
        text: "Make a shell"
        Accessible.name: "Make a shell"
        enabled: actions.slots.length > 0 && actions.asked === ""
        onClicked: actions.makeShell()
    }

    Label {
        objectName: "blockActionsRefusal"
        width: actions.width
        visible: text !== ""
        text: actions.refusal
        wrapMode: Text.Wrap
        color: Colors.danger
        font.pixelSize: Typography.captionSize
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
}
