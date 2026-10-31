// Export a bundle: the native app `ljos run` starts or the site ljos-web
// builds, as the `export` call an agent makes. Opening the panel (and
// every change of what to export) asks for a dry run, so the list of what
// the bundle would hold, what is left out and why, and whether it replaces
// a bundle already there are on screen before the person writes anything.
// What the panel shows is the last answer or refusal of `export`; it keeps
// no other state about the project.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Popup {
    id: panel
    required property Workspace workspace

    // The last answer of `export`, or null, and why the last call was
    // refused, or empty.
    property var answer: null
    property string refusal: ""
    // How many calls have no answer yet. What is on screen is true of what
    // the fields say only when none has.
    property int waiting: 0
    readonly property bool busy: waiting > 0
    // The document to export. Kept here and not in the box: a new list of
    // documents, which any tool's answer brings, sets the box's row anew.
    property string chosen: ""
    // Whether the answer in view was a real export, not a dry run.
    readonly property bool wrote: answer !== null && answer.written

    readonly property string target: web.checked ? "web" : "native"
    readonly property string entry: workspace.documents.indexOf(chosen) >= 0 ? chosen : ""

    // Main.qml when the project has one, else the document shown, else the
    // first.
    function defaultEntry() {
        const docs = workspace.documents
        if (docs.indexOf("Main.qml") >= 0)
            return "Main.qml"
        if (workspace.doc !== "")
            return workspace.doc
        return docs.length > 0 ? docs[0] : ""
    }

    function run(dry) {
        if (entry === "")
            return
        waiting += 1
        const args = {entry: entry, target: target, dry_run: dry}
        if (out.text.trim() !== "")
            args.out = out.text.trim()
        workspace.dispatch("export", args)
    }

    function check() {
        run(true)
    }

    function show() {
        answer = null
        refusal = ""
        chosen = defaultEntry()
        open()
        check()
    }

    function size(bytes) {
        if (bytes < 1024)
            return bytes + " bytes"
        if (bytes < 1024 * 1024)
            return (bytes / 1024).toFixed(1) + " KB"
        return (bytes / (1024 * 1024)).toFixed(1) + " MB"
    }

    // Everything the bundle would hold or leave out, one line each, as
    // the manifest lists it.
    function lines(a) {
        if (a === null)
            return []
        const m = a.manifest
        const rows = ["Entry " + m.entry]
        for (const f of m.files)
            rows.push("File " + f.path + " (" + size(f.bytes) + ")")
        for (const b of m.blocks)
            rows.push("Block " + b.block + " in " + b.file)
        if (m.theme)
            rows.push("Theme " + m.theme)
        for (const l of (m.links || []))
            rows.push("Link " + l.path + " to " + l.to)
        for (const i of (m.imports || []))
            rows.push("Import " + i)
        for (const o of m.left_out)
            rows.push("Left out " + o.path + ": " + o.why)
        return rows
    }

    function summary(a) {
        if (a === null)
            return busy ? "Checking…" : ""
        return (a.written ? "Wrote " : "Would write ") + a.files
            + (a.files === 1 ? " file" : " files") + ", " + size(a.bytes) + ", to " + a.out
    }

    // What to do with the bundle next.
    function next(a) {
        if (a === null || !a.written)
            return ""
        if (a.manifest.run)
            return "Start it with: " + a.manifest.run.join(" ") + "  (in " + a.path + ")"
        return "Build the site with: mise run export:web:build " + a.path
    }

    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay
    width: Math.min(640, parent.width - Spacing.lg * 2)
    height: Math.min(560, parent.height - Spacing.lg * 2)
    modal: true
    focus: true
    padding: Spacing.sm
    closePolicy: Popup.CloseOnEscape

    Connections {
        target: panel.workspace
        function onAnswered(tool, answer) {
            if (tool !== "export")
                return
            panel.waiting -= 1
            panel.refusal = ""
            panel.answer = answer
        }
        function onRefused(tool, error) {
            if (tool !== "export")
                return
            panel.waiting -= 1
            panel.answer = null
            panel.refusal = error.message
        }
    }

    // Text for the clipboard; a field the person never sees.
    TextInput {
        id: clip
        visible: false
    }

    background: Rectangle {
        color: Colors.surface
        border.color: Colors.border
    }

    contentItem: ColumnLayout {
        id: body
        spacing: Spacing.xs
        Accessible.role: Accessible.Pane
        Accessible.name: "Export panel"

        Label {
            text: "Export"
            font.pixelSize: Typography.titleSize
            font.weight: Typography.bodyStrongWeight
            color: Colors.text
            Layout.fillWidth: true
        }

        RowLayout {
            spacing: Spacing.sm
            ButtonGroup {
                id: kinds
            }
            RadioButton {
                id: native_
                text: "Native app"
                Accessible.name: text
                checked: true
                ButtonGroup.group: kinds
                onClicked: panel.check()
            }
            RadioButton {
                id: web
                text: "Web site"
                Accessible.name: text
                ButtonGroup.group: kinds
                onClicked: panel.check()
            }
        }

        RowLayout {
            spacing: Spacing.xs
            Label {
                text: "Entry"
                color: Colors.text
            }
            ComboBox {
                id: entries
                model: panel.workspace.documents
                currentIndex: panel.workspace.documents.indexOf(panel.chosen)
                Accessible.name: "Entry document"
                Layout.fillWidth: true
                onActivated: (index) => {
                    panel.chosen = panel.workspace.documents[index]
                    panel.check()
                }
            }
        }

        RowLayout {
            spacing: Spacing.xs
            Label {
                text: "Into"
                color: Colors.text
            }
            TextField {
                id: out
                placeholderText: "build/" + panel.target
                Accessible.name: "Output directory"
                Layout.fillWidth: true
                // As typed, so "Write bundle" never writes somewhere the
                // warning above it has not looked at.
                onTextEdited: panel.check()
                onAccepted: panel.check()
            }
        }

        Label {
            objectName: "exportSummary"
            text: panel.summary(panel.answer)
            visible: text !== ""
            color: Colors.text
            font.weight: Typography.bodyStrongWeight
            wrapMode: Text.Wrap
            Accessible.role: Accessible.StaticText
            Accessible.name: text
            Layout.fillWidth: true
        }

        // The tool replaces a bundle already in the output directory.
        Label {
            objectName: "exportReplaces"
            text: panel.answer !== null && panel.answer.replaces && !panel.wrote
                ? panel.answer.out + " already holds a bundle: exporting replaces it" : ""
            visible: text !== ""
            color: Colors.text
            wrapMode: Text.Wrap
            Accessible.role: Accessible.StaticText
            Accessible.name: text
            Layout.fillWidth: true
        }

        // The tool's own message for a refusal.
        Label {
            objectName: "exportRefusal"
            text: panel.refusal
            visible: text !== ""
            color: Colors.textOnDanger
            wrapMode: Text.Wrap
            padding: Spacing.xs
            Accessible.role: Accessible.AlertMessage
            Accessible.name: text
            background: Rectangle {
                color: Colors.danger
            }
            Layout.fillWidth: true
        }

        Label {
            objectName: "exportNext"
            text: panel.next(panel.answer)
            visible: text !== ""
            color: Colors.text
            wrapMode: Text.Wrap
            Accessible.role: Accessible.StaticText
            Accessible.name: text
            Layout.fillWidth: true
        }

        ListView {
            objectName: "exportFiles"
            clip: true
            model: panel.lines(panel.answer)
            Accessible.role: Accessible.List
            Accessible.name: "Bundle contents"
            Layout.fillWidth: true
            Layout.fillHeight: true
            ScrollBar.vertical: ScrollBar {}

            delegate: Label {
                required property string modelData
                width: ListView.view.width
                text: modelData
                color: Colors.text
                font.pixelSize: Typography.captionSize
                elide: Text.ElideMiddle
                Accessible.role: Accessible.StaticText
                Accessible.name: modelData
            }
        }

        RowLayout {
            spacing: Spacing.xs
            Button {
                text: "Check"
                Accessible.name: text
                enabled: panel.entry !== "" && !panel.busy
                onClicked: panel.check()
            }
            Button {
                text: "Write bundle"
                Accessible.name: text
                enabled: panel.entry !== "" && !panel.busy
                highlighted: true
                onClicked: panel.run(false)
            }
            Button {
                text: "Copy path"
                Accessible.name: text
                enabled: panel.wrote
                onClicked: {
                    clip.text = panel.answer.path
                    clip.selectAll()
                    clip.copy()
                }
            }
            Item {
                Layout.fillWidth: true
            }
            Button {
                text: "Close"
                Accessible.name: text
                onClicked: panel.close()
            }
        }

        // Enter checks, as on a form; the buttons take their own Enter.
        Keys.onReturnPressed: panel.check()
    }
}
