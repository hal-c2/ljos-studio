// The document's text. Typing applies through `document_edit`, the tool a
// person typing in an agent's editor goes through too. The selected object is
// marked by a band behind nothing: a text selection there would be replaced
// by the next key pressed.
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
        if (pending > 0 || workspace.selected === "" || s.end > area.length) {
            band.marks = ""
            return
        }
        const first = area.positionToRectangle(s.start)
        const last = area.positionToRectangle(s.end)
        band.y = first.y
        band.height = last.y + last.height - first.y
        band.marks = area.getText(s.start, s.end)
        // Someone typing here keeps their caret; anyone else starts typing at
        // the object they chose.
        if (!area.activeFocus)
            area.cursorPosition = s.start
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
                // The band went with the first key, and text that is already
                // the file's gives no other reason to look again.
                panel.showSelection()
            }
        }
        function onRefused(tool, error) {
            if (tool === "document_edit") {
                --panel.pending
                panel.sync()
                // The band went with the first key, and text that is already
                // the file's gives no other reason to look again.
                panel.showSelection()
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
            font.pixelSize: Typography.captionSize
            enabled: panel.workspace.doc !== ""
            selectByMouse: true
            color: Colors.text
            leftPadding: Spacing.sm
            topPadding: Spacing.xs
            background: Rectangle {
                color: Colors.surface
            }

            Rectangle {
                id: band
                objectName: "codeBand"
                property string marks: ""
                visible: marks !== ""
                width: area.width
                color: Colors.accent
                opacity: 0.2
            }

            onTextChanged: {
                if (panel.syncing || panel.workspace.doc === "")
                    return
                ++panel.pending
                panel.workspace.dispatch("document_edit", {
                    doc: panel.workspace.doc,
                    generation: panel.workspace.generation,
                    text: text,
                    typing: true
                })
                band.marks = ""
            }
        }
    }
}
