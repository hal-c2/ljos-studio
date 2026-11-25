// A project file that is not a document: a README, a TypeScript module, a
// style sheet. Unlike a document, which is saved as it is typed, an edit
// here is held until Save, through the same `project_open` `files` calls an
// agent makes, so the file changes on disk at one moment the person chose.
// When another program saves the file, the text here follows it while no
// edit is held; under held edits the person picks whose text stays.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Panel {
    id: editor
    required property Workspace workspace
    // The file shown, empty for none.
    property string path: ""
    title: path === "" ? "File" : path

    // The file as the workspace lists it: whether edits are held, and the
    // stamp that moves when the disk's text replaced the one read.
    readonly property var row: {
        for (const f of workspace.files) {
            if (f.path === path)
                return f
        }
        return null
    }
    readonly property bool dirty: row !== null && row.dirty
    readonly property bool outside: row !== null && row.outside
    property string refusal: ""
    // Edits sent and not yet answered, by file: until they are, the text
    // here is ahead of the held one and must not be set from an answer, and
    // the file has edits not saved even once another one is shown.
    // Kept without a prototype and read only as its own keys, so a file
    // named `constructor` or `toString` counts from 0 like any other.
    property var unanswered: Object.create(null)
    readonly property int pending: waiting(unanswered, path)
    property bool syncing: false
    property real stamp: -1

    signal closeAsked()

    // `by` more edits of `file` on their way, or answered when negative.
    function waiting(counts, file) {
        return Object.prototype.hasOwnProperty.call(counts, file) ? counts[file] : 0
    }
    function sent(file, by) {
        const next = Object.assign(Object.create(null), unanswered)
        next[file] = waiting(next, file) + by
        if (next[file] <= 0)
            delete next[file]
        unanswered = next
    }

    function take(text) {
        if (pending > 0 || area.text === text)
            return
        syncing = true
        area.text = text
        syncing = false
    }

    function read() {
        if (path !== "")
            workspace.dispatch("project_open", {files: "read", file: path})
    }
    function save(force) {
        refusal = ""
        workspace.dispatch("project_open", {files: "save", file: path, force: force === true})
    }
    function revert() {
        refusal = ""
        workspace.dispatch("project_open", {files: "revert", file: path})
    }

    onPathChanged: {
        refusal = ""
        stamp = row === null ? -1 : row.stamp
        syncing = true
        area.text = ""
        syncing = false
        read()
    }
    // Another program's save of a file with no edits held.
    onRowChanged: {
        if (row !== null && row.stamp !== stamp) {
            stamp = row.stamp
            if (!row.dirty)
                read()
        }
    }

    Connections {
        target: editor.workspace
        function onAnswered(tool, answer, asked) {
            if (tool !== "project_open")
                return
            if (asked.files === "edit")
                editor.sent(asked.file, -1)
            if (asked.file !== editor.path || answer.file === undefined)
                return
            if (answer.file.text !== undefined)
                editor.take(answer.file.text)
        }
        function onRefused(tool, error, asked) {
            if (tool !== "project_open")
                return
            if (asked.files === "edit")
                editor.sent(asked.file, -1)
            if (asked.file !== editor.path)
                return
            editor.refusal = error.message
        }
    }

    // What a window too narrow for the header cannot show stays on the card.
    clip: true
    // Too narrow for the path and the buttons side by side: the path keeps
    // its row, with Close, and Revert and Save go under it, so nothing
    // covers anything.
    readonly property bool narrow: width < actions.implicitWidth + 14 * Typography.bodySize

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        RowLayout {
            id: header
            spacing: Spacing.xxs
            Layout.fillWidth: true
            Layout.margins: Spacing.xxs
            Layout.leftMargin: Spacing.sm
            Icon {
                name: "document"
                color: Colors.textMuted
            }
            Label {
                objectName: "filePath"
                text: editor.path
                color: Colors.text
                font.pixelSize: Typography.bodySize
                font.weight: Typography.bodyStrongWeight
                elide: Text.ElideMiddle
                // On a narrow window the buttons stay whole and the words
                // give way: they take only the room the buttons leave.
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                Layout.minimumWidth: 0
            }
            Label {
                objectName: "fileState"
                // Only where the row has room for them; Save going dim, the
                // dot in the tree and the banner say the same.
                visible: !editor.narrow && header.width >= 30 * Typography.captionSize
                text: editor.outside ? "Changed on disk" : editor.dirty ? "Not saved" : "Saved"
                color: editor.outside ? Colors.danger : Colors.textMuted
                font.pixelSize: Typography.captionSize
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
            // The buttons' room while they sit on this row.
            Item {
                implicitHeight: actions.implicitHeight
                Layout.preferredWidth: editor.narrow ? 0 : actions.implicitWidth
            }
            IconButton {
                id: closeButton
                icon_: "close"
                text: "Close file"
                shortcut: ""
                onClicked: editor.closeAsked()
            }
        }

        // Under the path on a narrow card, the buttons' row.
        Item {
            visible: editor.narrow
            Layout.preferredHeight: actions.implicitHeight + Spacing.xxs
        }

        // Someone else saved the file under the edits held here.
        Rectangle {
            visible: editor.outside
            color: Colors.surfaceRaised
            border.width: 1
            border.color: Colors.danger
            radius: 8
            implicitHeight: banner.implicitHeight + 2 * Spacing.xs
            Layout.fillWidth: true
            Layout.leftMargin: Spacing.xs
            Layout.rightMargin: Spacing.xs
            RowLayout {
                id: banner
                anchors.fill: parent
                anchors.margins: Spacing.xs
                spacing: Spacing.xs
                Label {
                    objectName: "outsideBanner"
                    text: "Another program saved this file after you started editing it."
                    wrapMode: Text.Wrap
                    color: Colors.text
                    font.pixelSize: Typography.captionSize
                    Accessible.role: Accessible.AlertMessage
                    Accessible.name: text
                    Layout.fillWidth: true
                }
                TextButton {
                    text: "Take theirs"
                    Accessible.description: "Drop your edits for the text on disk"
                    onClicked: editor.revert()
                }
                TextButton {
                    text: "Keep mine"
                    Accessible.description: "Save your edits over the text on disk"
                    onClicked: editor.save(true)
                }
            }
        }

        Label {
            objectName: "fileRefusal"
            visible: text !== ""
            text: editor.refusal
            wrapMode: Text.Wrap
            color: Colors.danger
            font.pixelSize: Typography.captionSize
            Layout.fillWidth: true
            Layout.leftMargin: Spacing.sm
            Layout.rightMargin: Spacing.sm
        }

        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            // A long line scrolls inside the card, not over its edge.
            clip: true

            TextArea {
                id: area
                objectName: "fileText"
                Accessible.name: "Text of " + editor.path
                font.family: "monospace"
                font.pixelSize: Typography.captionSize
                selectByMouse: true
                color: Colors.text
                leftPadding: Spacing.sm
                topPadding: Spacing.xs
                background: null
                onTextChanged: {
                    if (editor.syncing || editor.path === "")
                        return
                    editor.sent(editor.path, 1)
                    editor.workspace.dispatch("project_open", {files: "edit", file: editor.path, text: text})
                }
            }
        }
    }

    // Revert and Save, beside the path or under it.
    RowLayout {
        id: actions
        spacing: Spacing.xxs
        anchors.right: parent.right
        anchors.rightMargin: Spacing.xxs + (editor.narrow ? 0 : closeButton.width + header.spacing)
        y: editor.narrow ? header.y + header.height + Spacing.xxs : header.y + (header.height - height) / 2
        TextButton {
            text: "Revert"
            enabled: editor.dirty
            Accessible.description: "Drop the edits not saved"
            onClicked: editor.revert()
        }
        TextButton {
            text: "Save"
            accent: true
            enabled: editor.dirty && !editor.outside
            Accessible.description: "Write the edits to disk (Ctrl+S)"
            onClicked: editor.save(false)
        }
    }
}
