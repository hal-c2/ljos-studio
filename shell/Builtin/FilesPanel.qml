// The project's files as a tree, documents and the rest. A document opens in
// the designer as the documents panel opens it; any other text file opens in
// the file editor, which holds edits until they are saved. Making, renaming
// and removing a file are `project_open` `files` calls, as an agent makes
// them; a folder anywhere opens through the system's folder dialog or the
// list of folders opened last.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Panel {
    id: panel
    required property Workspace workspace
    // The file the editor shows, which the shell keeps.
    property string editing: ""
    // Whether the window can ask the system for a folder.
    property bool choosesFolders: false
    title: "Files"

    signal documentAsked(string doc)
    signal fileAsked(string path)
    signal folderAsked()
    signal recentAsked(string folder)
    // The removal of `path`, which the shell confirms first.
    signal removeAsked(string path)

    readonly property real rowHeight: 26
    // The row chosen last: what Rename and Delete act on.
    property string chosen: ""
    // Folders folded away, by "/" and their path: a folder named
    // `constructor` or `__proto__` is no key an object already has.
    property var folded: ({})
    // What a gesture asked for and has no answer yet.
    property string making: ""
    property string refusal: ""

    readonly property var rows: tree(workspace.files, folded)
    readonly property string folderName: {
        const parts = workspace.folder.split("/").filter(p => p !== "")
        return parts.length > 0 ? parts[parts.length - 1] : workspace.folder
    }
    readonly property var otherFolders: workspace.recent.filter(f => f !== workspace.folder)

    function isDocument(path) {
        return path.endsWith(".qml")
    }

    // Folders first, each sorted, as a file manager lists them. A folder's
    // folders have no inherited keys, so any name is a folder's.
    function tree(files, folded) {
        const root = {dirs: Object.create(null), files: []}
        for (const f of files) {
            const parts = f.path.split("/")
            let node = root
            for (let i = 0; i < parts.length - 1; ++i) {
                if (node.dirs[parts[i]] === undefined)
                    node.dirs[parts[i]] = {dirs: Object.create(null), files: []}
                node = node.dirs[parts[i]]
            }
            node.files.push({name: parts[parts.length - 1], file: f})
        }
        const rows = []
        const walk = (node, prefix, depth) => {
            for (const name of Object.keys(node.dirs).sort()) {
                const path = prefix + name
                const open = folded["/" + path] !== true
                rows.push({path: path, name: name, depth: depth, folder: true, open: open,
                           dirty: false, outside: false})
                if (open)
                    walk(node.dirs[name], path + "/", depth + 1)
            }
            for (const e of node.files)
                rows.push({path: e.file.path, name: e.name, depth: depth, folder: false, open: false,
                           dirty: e.file.dirty, outside: e.file.outside})
        }
        walk(root, "", 0)
        return rows
    }

    function choose(row) {
        if (row.folder) {
            const next = Object.assign({}, folded)
            next["/" + row.path] = folded["/" + row.path] !== true
            folded = next
            return
        }
        chosen = row.path
        if (isDocument(row.path))
            documentAsked(row.path)
        else
            fileAsked(row.path)
    }

    function make(path) {
        refusal = ""
        making = path
        if (isDocument(path))
            workspace.dispatch("document_new", {doc: path})
        else
            workspace.dispatch("project_open", {files: "new", file: path, text: ""})
    }

    function rename(from, to) {
        refusal = ""
        making = to
        workspace.dispatch("project_open", {files: "rename", file: from, to: to})
    }

    readonly property bool chosenExists: workspace.files.some(f => f.path === chosen)

    Connections {
        target: panel.workspace
        function onAnswered(tool, answer, asked) {
            if (panel.making === "")
                return
            const made = tool === "document_new" && asked.doc === panel.making
            const moved = tool === "project_open" && (asked.files === "new" || asked.files === "rename")
                && (asked.file === panel.making || asked.to === panel.making)
            if (!made && !moved)
                return
            const path = panel.making
            panel.making = ""
            nameField.text = ""
            panel.choose({path: path, folder: false})
        }
        function onRefused(tool, error, asked) {
            if (panel.making === "")
                return
            if (tool === "document_new" || (tool === "project_open" && asked.files !== undefined)) {
                panel.making = ""
                panel.refusal = error.message
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Spacing.xxs

        // Where the files are, and the way to another folder.
        RowLayout {
            spacing: Spacing.xxs
            Layout.fillWidth: true
            Layout.leftMargin: Spacing.xs
            Layout.rightMargin: Spacing.xxs
            Icon {
                name: "folderOpen"
                color: Colors.textMuted
            }
            Label {
                objectName: "folderName"
                text: panel.folderName
                color: Colors.text
                font.pixelSize: Typography.bodySize
                font.weight: Typography.bodyStrongWeight
                elide: Text.ElideMiddle
                Accessible.role: Accessible.StaticText
                Accessible.name: "Folder " + panel.workspace.folder
                Layout.fillWidth: true
            }
            TextButton {
                visible: panel.choosesFolders
                text: "Open…"
                icon_: "folder"
                Accessible.name: "Open folder"
                Accessible.description: "Open a folder as the project (Ctrl+O)"
                onClicked: panel.folderAsked()
            }
        }

        ListView {
            id: list
            objectName: "files"
            clip: true
            model: panel.rows
            Accessible.role: Accessible.List
            Accessible.name: "Files"
            Layout.fillWidth: true
            Layout.fillHeight: true
            ScrollBar.vertical: ScrollBar {
                Accessible.name: "Scroll the files"
                policy: ScrollBar.AlwaysOn
                visible: list.contentHeight > list.height
            }

            delegate: ItemDelegate {
                id: entry
                required property var modelData
                readonly property bool shown: !modelData.folder
                    && modelData.path === (panel.editing !== "" ? panel.editing : panel.workspace.doc)
                width: ListView.view.width
                implicitHeight: panel.rowHeight
                padding: 0
                leftPadding: Spacing.xs + modelData.depth * Spacing.sm
                rightPadding: Spacing.xs
                text: modelData.name
                Accessible.name: modelData.path
                Accessible.description: modelData.folder ? (modelData.open ? "Open folder" : "Folded folder")
                    : modelData.outside ? "Changed on disk, with edits not saved"
                    : modelData.dirty ? "Edits not saved" : ""
                highlighted: shown || modelData.path === panel.chosen
                onClicked: panel.choose(modelData)
                contentItem: RowLayout {
                    spacing: Spacing.xxs
                    Icon {
                        name: entry.modelData.folder ? (entry.modelData.open ? "chevronDown" : "chevronRight")
                            : "document"
                        color: entry.shown ? Colors.link : Colors.textMuted
                    }
                    Text {
                        text: entry.text
                        color: Colors.text
                        font.pixelSize: Typography.bodySize
                        font.weight: entry.shown ? Typography.bodyStrongWeight : Typography.bodyWeight
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                    // Edits held: a dot, the editor's mark for the same.
                    Rectangle {
                        visible: entry.modelData.dirty
                        implicitWidth: 8
                        implicitHeight: 8
                        radius: 4
                        color: entry.modelData.outside ? Colors.danger : Colors.accent
                        Accessible.ignored: true
                    }
                }
                background: Rectangle {
                    radius: 6
                    color: entry.highlighted ? Colors.surfaceSelected
                        : entry.hovered ? Colors.surfaceHover : "transparent"
                }
            }
        }

        // A file by its path from the project: new, or what the chosen one
        // is renamed to.
        RowLayout {
            spacing: Spacing.xxs
            Layout.fillWidth: true
            Layout.leftMargin: Spacing.xxs
            Layout.rightMargin: Spacing.xxs
            Field {
                id: nameField
                placeholderText: "path/to/file"
                Accessible.name: "File's path"
                onAccepted: newFile.clicked()
                Layout.fillWidth: true
            }
            TextButton {
                id: newFile
                text: "New"
                Accessible.name: "New file"
                enabled: nameField.text !== ""
                onClicked: panel.make(nameField.text)
            }
        }
        RowLayout {
            visible: panel.chosenExists
            spacing: Spacing.xxs
            Layout.fillWidth: true
            Layout.leftMargin: Spacing.xxs
            Layout.rightMargin: Spacing.xxs
            Label {
                text: panel.chosen
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
                elide: Text.ElideMiddle
                Layout.fillWidth: true
            }
            TextButton {
                text: "Rename"
                Accessible.name: "Rename file"
                Accessible.description: "To the path typed above"
                enabled: nameField.text !== "" && nameField.text !== panel.chosen
                onClicked: panel.rename(panel.chosen, nameField.text)
            }
            TextButton {
                text: "Delete"
                Accessible.name: "Delete file"
                onClicked: panel.removeAsked(panel.chosen)
            }
        }
        Label {
            objectName: "filesRefusal"
            visible: text !== ""
            text: panel.refusal
            wrapMode: Text.Wrap
            color: Colors.danger
            font.pixelSize: Typography.captionSize
            Accessible.role: Accessible.StaticText
            Accessible.name: text
            Layout.fillWidth: true
            Layout.leftMargin: Spacing.xs
            Layout.rightMargin: Spacing.xs
        }

        // The folders opened before, to go back to one.
        Section {
            title: "Recent folders"
            visible: panel.otherFolders.length > 0
            inset: Spacing.xxs
            Layout.fillWidth: true
            Repeater {
                model: panel.otherFolders
                delegate: ItemDelegate {
                    id: folderRow
                    required property string modelData
                    implicitHeight: panel.rowHeight
                    padding: 0
                    leftPadding: Spacing.xs
                    text: modelData
                    Accessible.name: "Open recent " + modelData
                    onClicked: panel.recentAsked(modelData)
                    contentItem: RowLayout {
                        spacing: Spacing.xxs
                        Icon {
                            name: "folder"
                            color: Colors.textMuted
                        }
                        Text {
                            text: folderRow.text
                            color: Colors.text
                            font.pixelSize: Typography.captionSize
                            elide: Text.ElideMiddle
                            Layout.fillWidth: true
                        }
                    }
                    background: Rectangle {
                        radius: 6
                        color: folderRow.hovered ? Colors.surfaceHover : "transparent"
                    }
                    Layout.fillWidth: true
                }
            }
        }
    }
}
