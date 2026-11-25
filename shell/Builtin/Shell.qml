// Studio's own shell: panels that skin the one block Studio binds,
// Studio.Workspace. Every gesture is a `dispatch` of a Studio tool, the
// call an agent makes over MCP. The window (../Window.qml) shows it when
// the person has no shell of their own, or when theirs does not work; a
// person's shell starts as a copy of this directory.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Item {
    id: shell
    // What the window's title says.
    readonly property string title: workspace.doc === "" ? "Ljós Studio" : workspace.doc + " · Ljós Studio"

    Workspace {
        id: workspace
    }

    // ------------------------------------------------------------ files

    // The file the editor shows in place of the canvas, empty for none.
    property string editing: ""
    // Whether the window can ask the system for a folder: it sets this
    // when it does, and answers `folderAsked` with `openFolder`.
    property bool choosesFolders: false
    signal folderAsked()
    // Everything held is saved or dropped, so the window may close.
    signal closeAgreed()

    // The files with edits not saved: the editor's, too, while an edit typed
    // there has not been answered yet and so is in no row.
    readonly property var unsaved: {
        const dirty = workspace.files.filter(f => f.dirty).map(f => f.path)
        if (fileEditor.pending > 0 && editing !== "" && dirty.indexOf(editing) < 0)
            dirty.push(editing)
        return dirty
    }
    // What runs once the edits held are saved: the person's way on after
    // "Save all", null for none, with the request it waits for, so the
    // answer to an earlier save asked for something else runs nothing.
    property var afterSave: null

    // Asks before `go(force)` drops edits held: `force` is whether it may.
    function leave(go) {
        if (unsaved.length === 0) {
            go(false)
            return
        }
        const n = unsaved.length
        asker.ask = (which) => {
            if (which === 1) {
                afterSave = {files: "save_all", file: undefined, then: () => go(false)}
                workspace.dispatch("project_open", {files: "save_all"})
            } else if (which === 2) {
                go(true)
            }
        }
        asker.show(n === 1 ? "Save the changes to " + unsaved[0] + "?"
                           : "Save the changes to " + n + " files?",
                   n === 1 ? "They are lost if you don't save them."
                           : unsaved.join(", ") + ": their changes are lost if you don't save them.",
                   "Save all", "Don't save")
    }
    function openFolder(folder) {
        leave(force => workspace.dispatch("project_open", force ? {folder: folder, force: true} : {folder: folder}))
    }
    // What the window asks before it closes.
    function askClose() {
        leave(() => shell.closeAgreed())
    }
    function closeFile() {
        const path = editing
        const close = force => {
            workspace.dispatch("project_open", {files: "close", file: path, force: force})
            editing = ""
        }
        if (unsaved.indexOf(path) < 0) {
            close(false)
            return
        }
        asker.ask = (which) => {
            if (which === 1) {
                afterSave = {files: "save", file: path, then: () => close(false)}
                workspace.dispatch("project_open", {files: "save", file: path})
            } else if (which === 2) {
                close(true)
            }
        }
        asker.show("Save the changes to " + path + "?", "They are lost if you don't save them.",
                   "Save", "Don't save")
    }
    function removeFile(path) {
        asker.ask = (which) => {
            if (which === 1) {
                if (editing === path)
                    editing = ""
                workspace.dispatch("project_open", {files: "remove", file: path})
            }
        }
        asker.show("Delete " + path + "?", "It is removed from the folder on disk.", "Delete", "", true)
    }
    // A project opened anew starts on its entry, as the window's first
    // look does.
    function entered(documents) {
        editing = ""
        const doc = documents.indexOf("Main.qml") >= 0 ? "Main.qml" : documents.length > 0 ? documents[0] : ""
        workspace.doc = doc
        if (doc !== "" && workspace.previews)
            workspace.dispatch("preview_open", {doc: doc})
    }

    Connections {
        target: workspace
        function onAnswered(tool, answer, asked) {
            if (tool !== "project_open")
                return
            const opened = (asked.folder !== undefined || asked.path !== undefined)
                && asked.files === undefined && asked.asset === undefined
            if (opened) {
                shell.entered(answer.documents || [])
                return
            }
            // The document shown moved or went: show it where it is now, or
            // the project's entry.
            if (asked.files === "rename" && asked.file === workspace.doc) {
                workspace.doc = asked.to
                return
            }
            if (asked.files === "remove" && asked.file === workspace.doc) {
                shell.entered(workspace.documents.filter(d => d !== asked.file))
                return
            }
            const waiting = shell.afterSave
            if (waiting !== null && asked.files === waiting.files && asked.file === waiting.file) {
                shell.afterSave = null
                waiting.then()
            }
        }
        function onRefused(tool, error, asked) {
            const waiting = shell.afterSave
            if (tool === "project_open" && waiting !== null
                    && asked.files === waiting.files && asked.file === waiting.file)
                shell.afterSave = null
        }
        // A document shown is the designer's: the file editor steps aside.
        function onDocChanged() {
            if (workspace.doc !== "")
                shell.editing = ""
        }
    }

    AskPanel {
        id: asker
        // What the person's answer does: 0 is cancel.
        property var ask: null
        onAnswered: (which) => {
            const then = asker.ask
            asker.ask = null
            if (then !== null)
                then(which)
        }
    }

    Shortcut {
        sequence: StandardKey.Save
        enabled: shell.editing !== ""
        onActivated: workspace.dispatch("project_open", {files: "save", file: shell.editing})
    }
    Shortcut {
        sequence: "Ctrl+Shift+S"
        enabled: shell.unsaved.length > 0
        onActivated: workspace.dispatch("project_open", {files: "save_all"})
    }
    Shortcut {
        sequence: StandardKey.Open
        enabled: shell.choosesFolders
        onActivated: shell.folderAsked()
    }

    // A refusal says what an agent should send next; a person in the window
    // is told what is the matter instead. Kept with the message it replaces,
    // so the next refusal, which may need no other words, shows as it is.
    property var plainly: ({message: "", words: ""})
    function plain(tool, error, asked) {
        if (tool === "project_open" && asked.asset === "import" && error.code === "path_denied")
            return "There is no " + asked.file + " in the project's folder: copy the file in, then import it"
        if (tool === "preview_open" && error.details !== undefined && error.details.kind === "no_mocks")
            return "This project has no mock data: it is a mocks folder with a stand-in for each block's module"
        return ""
    }
    Connections {
        target: workspace
        function onRefused(tool, error, asked) {
            shell.plainly = {message: error.message, words: shell.plain(tool, error, asked)}
        }
    }

    Shortcut {
        sequences: [StandardKey.Undo]
        onActivated: workspace.dispatch("history", {action: "undo"})
    }
    Shortcut {
        sequences: [StandardKey.Redo]
        onActivated: workspace.dispatch("history", {action: "redo"})
    }

    Shortcut {
        sequence: "Ctrl+E"
        onActivated: exporter.show()
    }

    ExportPanel {
        id: exporter
        workspace: workspace
    }
    ProjectEntry {
        workspace: workspace
    }

    // The canvas's ground, under the panels that float on it.
    Rectangle {
        anchors.fill: parent
        color: Colors.canvas
    }

    Shortcut {
        sequences: ["Ctrl+K", "Meta+K", "Ctrl+Shift+P"]
        onActivated: commands.show()
    }
    CommandPalette {
        id: commands
        workspace: workspace
        canvas: canvas
        toolbar: toolbar
        exporter: exporter
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Toolbar {
            id: toolbar
            workspace: workspace
            problems: previewProblems.count
            onExportRequested: exporter.show()
            Layout.fillWidth: true
        }

        SplitView {
            orientation: Qt.Horizontal
            // A gutter of the canvas's own ground: the panels float beside it.
            handle: Item {
                implicitWidth: Spacing.xs
                Rectangle {
                    anchors.centerIn: parent
                    width: 2
                    height: parent.height / 3
                    radius: 1
                    color: Colors.border
                    opacity: SplitHandle.hovered || SplitHandle.pressed ? 1 : 0
                }
            }
            Layout.fillWidth: true
            Layout.fillHeight: true

            // What the document is made of, and what can go in it.
            Item {
                visible: toolbar.leftOpen
                SplitView.preferredWidth: 268
                SplitView.minimumWidth: 208

                Rectangle {
                    id: leftCard
                    anchors.fill: parent
                    anchors.margins: Spacing.xs
                    anchors.rightMargin: 0
                    anchors.topMargin: 0
                    anchors.bottomMargin: Spacing.xxs
                    radius: 12
                    color: Colors.surface
                    border.width: 1
                    border.color: Colors.surfacePressed
                }
                ColumnLayout {
                    anchors.fill: leftCard
                    anchors.margins: 1
                    spacing: 0

                    TabStrip {
                        id: leftTabs
                        tabs: ["Layers", "Insert", "Files"]
                        Layout.fillWidth: true
                    }
                    ColumnLayout {
                        visible: leftTabs.current === 0
                        spacing: 0
                        Layout.fillWidth: true
                        Layout.fillHeight: true

                        Section {
                            title: "Documents"
                            ruled: false
                            inset: Spacing.xxs
                            Layout.fillWidth: true
                            DocumentsPanel {
                                id: documentsPanel
                                workspace: workspace
                                Layout.fillWidth: true
                                Layout.preferredHeight: documentsPanel.wants
                                Layout.minimumHeight: documentsPanel.least
                            }
                        }
                        // Beside the documents: what the open one offers when it is a block.
                        Section {
                            title: "Block"
                            visible: blockActions.block !== null
                            inset: Spacing.xxs
                            Layout.fillWidth: true
                            BlockActions {
                                id: blockActions
                                workspace: workspace
                                Layout.fillWidth: true
                            }
                        }
                        Section {
                            id: layersSection
                            title: "Layers"
                            inset: Spacing.xxs
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            LayersPanel {
                                workspace: workspace
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                            }
                        }
                        // A closed Layers leaves its room empty below, so the
                        // card keeps its tabs at the top.
                        Item {
                            visible: !layersSection.open
                            Layout.fillHeight: true
                        }
                    }
                    PalettePanel {
                        visible: leftTabs.current === 1
                        workspace: workspace
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                    }
                    FilesPanel {
                        visible: leftTabs.current === 2
                        workspace: workspace
                        editing: shell.editing
                        choosesFolders: shell.choosesFolders
                        onDocumentAsked: (doc) => {
                            shell.editing = ""
                            documentsPanel.show(doc)
                        }
                        onFileAsked: (path) => shell.editing = path
                        onFolderAsked: shell.folderAsked()
                        onRecentAsked: (folder) => shell.openFolder(folder)
                        onRemoveAsked: (path) => shell.removeFile(path)
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                    }
                    // Under whichever fills: its room comes out of that, so it
                    // covers nothing and the code and the frame stay put.
                    PreviewProblems {
                        id: previewProblems
                        workspace: workspace
                        Layout.fillWidth: true
                        Layout.margins: Spacing.xs
                    }
                }
            }

            // A file that is no document takes the canvas's place, and the
            // code under it, which is the document's.
            Item {
                SplitView.fillWidth: true

                SplitView {
                    orientation: Qt.Vertical
                    anchors.fill: parent
                    visible: shell.editing === ""
                    handle: Item {
                        implicitHeight: Spacing.xs
                        Rectangle {
                            anchors.centerIn: parent
                            width: parent.width / 3
                            height: 2
                            radius: 1
                            color: Colors.border
                            opacity: SplitHandle.hovered || SplitHandle.pressed ? 1 : 0
                        }
                    }

                    Item {
                        SplitView.fillHeight: true

                        CanvasPanel {
                            id: canvas
                            anchors.fill: parent
                            workspace: workspace
                            recording: inspector.recording
                            // The window's own ground, so the gutters round the
                            // panels are the canvas going on behind them.
                            fill: Colors.canvas
                            onDoneRecording: inspector.pick("", "")
                            onLayoutAsked: (target, state) => inspector.editLayout(target, state)
                        }
                    }
                    Item {
                        visible: toolbar.codeOpen
                        SplitView.preferredHeight: 188
                        SplitView.minimumHeight: 88

                        Rectangle {
                            id: codeCard
                            anchors.fill: parent
                            anchors.bottomMargin: Spacing.xxs
                            radius: 12
                            color: Colors.surface
                            border.width: 1
                            border.color: Colors.surfacePressed
                        }
                        CodePanel {
                            workspace: workspace
                            anchors.fill: codeCard
                            anchors.margins: 1
                        }
                    }
                }
                Rectangle {
                    id: fileCard
                    visible: shell.editing !== ""
                    anchors.fill: parent
                    anchors.bottomMargin: Spacing.xxs
                    radius: 12
                    color: Colors.surface
                    border.width: 1
                    border.color: Colors.surfacePressed
                }
                FileEditor {
                    id: fileEditor
                    visible: shell.editing !== ""
                    workspace: workspace
                    path: shell.editing
                    anchors.fill: fileCard
                    anchors.margins: 1
                    onCloseAsked: shell.closeFile()
                }
            }

            // What the selection is, and what the document's look and flow are.
            Item {
                visible: toolbar.rightOpen
                SplitView.preferredWidth: 300
                SplitView.minimumWidth: 240

                Rectangle {
                    id: rightCard
                    anchors.fill: parent
                    anchors.margins: Spacing.xs
                    anchors.leftMargin: 0
                    anchors.topMargin: 0
                    anchors.bottomMargin: Spacing.xxs
                    radius: 12
                    color: Colors.surface
                    border.width: 1
                    border.color: Colors.surfacePressed
                }
                ColumnLayout {
                    anchors.fill: rightCard
                    anchors.margins: 1
                    spacing: 0

                    TabStrip {
                        id: rightTabs
                        tabs: ["Design", "Tokens", "Flow"]
                        Layout.fillWidth: true
                    }
                    InspectorPanel {
                        id: inspector
                        visible: rightTabs.current === 0
                        workspace: workspace
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                    }
                    TokensPanel {
                        visible: rightTabs.current === 1
                        workspace: workspace
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                    }
                    FlowPanel {
                        visible: rightTabs.current === 2
                        workspace: workspace
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                    }
                }
            }
        }

        // The person's own last refusal, until the next gesture is answered.
        // Text that doesn't parse is the problems' to say: as a row here it
        // would move the code view on every half-typed word.
        Label {
            objectName: "status"
            Accessible.role: Accessible.StatusBar
            Accessible.name: "Status"
            Accessible.description: text
            padding: Spacing.xs
            leftPadding: Spacing.xs + 14 + Spacing.xs
            // The preview's problems have a pane of their own; this line says what was refused.
            text: workspace.error === shell.plainly.message && shell.plainly.words !== "" ? shell.plainly.words
                : workspace.error
            color: Colors.text
            font.pixelSize: Typography.captionSize
            elide: Text.ElideRight
            // A slim bar on the canvas's ground, its mark saying it is a
            // refusal and not news.
            background: Rectangle {
                color: Colors.surface
                Rectangle {
                    width: parent.width
                    height: 1
                    color: Colors.surfacePressed
                }
                Icon {
                    x: Spacing.xs
                    anchors.verticalCenter: parent.verticalCenter
                    name: "close"
                    size: 14
                    color: Colors.danger
                }
            }
            visible: text !== ""
            Layout.fillWidth: true
        }
    }
}
