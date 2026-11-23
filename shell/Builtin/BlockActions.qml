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
    // The tool asked and not yet answered, and what it was asked with: other
    // panels ask the same tools, and their answers are not this one's.
    property string asked: ""
    property var askedWith: null
    property string shell: ""
    property string refusal: ""

    visible: block !== null
    spacing: Spacing.xxs
    Accessible.role: Accessible.Pane
    Accessible.name: "Block"

    function ask(tool, args) {
        refusal = ""
        asked = tool
        askedWith = args
        workspace.dispatch(tool, args)
    }

    // Whether an answer is to this panel's call: the tool, and the document,
    // slot and shell it was asked with.
    function mine(tool, args) {
        const own = askedWith
        return tool === asked && own !== null && args !== null && args !== undefined
            && args.doc === own.doc && args.slot === own.slot && args.shell === own.shell
    }

    function root() {
        const layers = workspace.layers || []
        return layers.length > 0 ? layers[0].node : ""
    }

    function addSlot(name) {
        const parent = workspace.selected !== "" ? workspace.selected : root()
        if (name === "" || parent === "")
            return
        ask("node_insert", {doc: workspace.doc, parent: {node: parent}, slot: name})
    }

    function makeShell() {
        if (block === null)
            return
        shell = block.name + "Shell.qml"
        ask("document_new", {doc: shell, shell: block.module + "." + block.name})
    }

    Connections {
        target: actions.workspace
        function onAnswered(tool, answer, asked) {
            if (!actions.mine(tool, asked))
                return
            actions.asked = ""
            actions.askedWith = null
            if (tool === "node_insert") {
                slotName.text = ""
            } else if (tool === "document_new") {
                actions.workspace.doc = actions.shell
                if (actions.workspace.previews)
                    actions.workspace.dispatch("preview_open", {doc: actions.shell})
            }
        }
        function onRefused(tool, error, asked) {
            if (!actions.mine(tool, asked))
                return
            actions.asked = ""
            actions.askedWith = null
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
