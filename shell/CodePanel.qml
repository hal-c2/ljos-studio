// The document's text. Typing applies through `document_edit`, the tool a
// person typing in an agent's editor goes through too; the selected
// object's text is selected here.
import QtQuick
import QtQuick.Controls
import Ljos.Design
import Studio

Panel {
    id: panel
    required property Workspace workspace
    title: workspace.doc === "" ? "Code" : "Code of " + workspace.doc

    // Edits sent and not yet answered. While there are some, the text here
    // is ahead of the published one, which must not overwrite it.
    property int pending: 0
    // The text is being set from the workspace, not typed.
    property bool syncing: false

    function sync() {
        if (pending > 0 || area.text === workspace.text)
            return
        syncing = true
        area.text = workspace.text
        syncing = false
        showSelection()
    }

    function showSelection() {
        const s = workspace.selection
        if (pending === 0 && workspace.selected !== "" && s.end <= area.length)
            area.select(s.start, s.end)
    }

    Connections {
        target: panel.workspace
        function onTextChanged() { panel.sync() }
        function onSelectionChanged() { panel.showSelection() }
        function onSelectedChanged() { panel.showSelection() }
        function onAnswered(tool, answer) {
            if (tool === "document_edit") {
                --panel.pending
                panel.sync()
            }
        }
        function onRefused(tool, error) {
            if (tool === "document_edit") {
                --panel.pending
                panel.sync()
            }
        }
    }

    ScrollView {
        anchors.fill: parent

        TextArea {
            id: area
            objectName: "code"
            Accessible.name: panel.title
            font.family: "monospace"
            font.pixelSize: Typography.bodySize
            enabled: panel.workspace.doc !== ""
            selectByMouse: true
            onTextChanged: {
                if (panel.syncing || panel.workspace.doc === "")
                    return
                ++panel.pending
                panel.workspace.dispatch("document_edit", {
                    doc: panel.workspace.doc,
                    generation: panel.workspace.generation,
                    text: text
                })
            }
        }
    }
}
