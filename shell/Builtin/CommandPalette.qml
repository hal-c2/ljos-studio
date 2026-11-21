// Every action of Studio in one list, found by typing: Ctrl+K, ⌘K or
// Ctrl+Shift+P. What is typed finds an action by its letters in order; the
// rows used lately come first. Enter runs the row picked and closes the
// palette, Escape closes it and gives the keys back.
//
// Studio's actions are its MCP tools, so each row is a `dispatch` of one,
// as the panels' own buttons are: the history, the targets, inserting a
// type. Every tool is there by name too; picking one asks for its
// arguments as JSON. The rest are the window's own: the canvas's zoom and
// tools, the look and which panels show. The palette keeps no state of the
// project; what it lists is read when it opens.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Popup {
    id: palette
    required property Workspace workspace
    // The window's parts the palette drives; any may be missing from a
    // person's shell, and their rows then are.
    property var canvas: null
    property var toolbar: null
    property var exporter: null

    // The names of the rows run, the latest first.
    property var recent: []
    // The tool whose arguments are being typed, as `workspace.tools` lists
    // it; null while the palette lists.
    property var asking: null
    property string refusal: ""
    property var actions: []

    parent: Overlay.overlay
    x: Math.round((parent.width - width) / 2)
    y: Math.round(parent.height * 0.12)
    width: Math.min(560, parent.width - 2 * Spacing.lg)
    height: Math.min(column.implicitHeight + 2 * padding, parent.height - y - Spacing.lg)
    modal: true
    focus: true
    padding: Spacing.xs
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    function show() {
        asking = null
        refusal = ""
        actions = listActions()
        query.text = ""
        open()
        query.forceActiveFocus()
        list.currentIndex = 0
    }

    // ------------------------------------------------------------ the rows

    // A row: what it is called, its shortcut, a line of what it does, and
    // what running it is (`run`, a function of none).
    function row(name, shortcut, detail, run) {
        return {name: name, shortcut: shortcut || "", detail: detail || "", run: run}
    }
    // The object a type inserted goes into, as the palette panel picks it:
    // the selection, or the root; beside what draws its own content.
    readonly property var drawsItsOwn: ["Text", "TextInput", "TextEdit", "Image", "AnimatedImage",
        "BorderImage", "Canvas", "Button", "CheckBox", "RadioButton", "Switch", "Slider",
        "ProgressBar", "SpinBox", "TextField", "TextArea", "ComboBox", "Label", "ToolButton"]
    function insertTarget() {
        const layers = workspace.layers
        const picked = layers.find(l => l.node === workspace.selected)
        if (picked === undefined)
            return layers.length > 0 ? layers[0].node : ""
        return picked.parent !== "" && drawsItsOwn.includes(picked.type.split(".").pop()) ? picked.parent : picked.node
    }
    // What a type starts with to be seen, as the palette panel gives it.
    function starter(name) {
        switch (name) {
        case "Rectangle":
            return "Rectangle {\n    width: 100\n    height: 100\n    color: \"#cccccc\"\n}"
        case "Text":
        case "Label":
        case "Button":
        case "CheckBox":
        case "RadioButton":
        case "Switch":
            return name + " {\n    text: \"" + name + "\"\n}"
        case "Image":
        case "Item":
            return name + " {\n    width: 100\n    height: 100\n}"
        default:
            return name + " {\n}"
        }
    }

    function listActions() {
        const ws = workspace
        const out = []
        const preview = ws.preview
        const cv = canvas
        if (cv !== null && preview !== null) {
            out.push(row("Zoom to 100%", "Ctrl+1", "Show the frame at its real size", () => cv.actualSize()))
            out.push(row("Zoom to fit", "Ctrl+0", "Fit the frame into the canvas", () => cv.fitNow()))
            out.push(row("Fit all frames", "Shift+1", "Fit every frame shown into the canvas", () => cv.fitAll()))
            out.push(row("Zoom in", "Ctrl+=", "One step closer, about the middle", () => cv.zoomBy(1)))
            out.push(row("Zoom out", "Ctrl+-", "One step back, about the middle", () => cv.zoomBy(-1)))
            out.push(row(cv.sideBySide ? "Show one frame" : "Show the frames side by side", "",
                         "Every target's frame on the canvas", () => { cv.sideBySide = !cv.sideBySide }))
            out.push(row("Select tool", "V", "Click to pick, drag to move", () => cv.useTool("select")))
            out.push(row("Hand tool", "H", "Drag to move the view", () => cv.useTool("hand")))
            out.push(row("Draw rectangle", "R", "Drag out a Rectangle", () => cv.useTool("Rectangle")))
            out.push(row("Draw text", "T", "Click to place a Text", () => cv.useTool("Text")))
            out.push(row(cv.playing ? "Stop playing" : "Play", "", "Clicks go to the running app", () => cv.useTool("play")))
            if (ws.chosen.length > 0) {
                for (const a of [["left", "Left"], ["hcenter", "Centre"], ["right", "Right"],
                                 ["top", "Top"], ["vcenter", "Middle"], ["bottom", "Bottom"]])
                    out.push(row("Align " + a[1], "", "node_arrange", () => cv.arrange("align", {align: a[0]})))
                for (const kind of ["Row", "Column", "Grid", "Item"])
                    out.push(row("Wrap in " + kind, "", "node_arrange", () => cv.arrange("wrap", {in: kind})))
            }
        }
        out.push(row("Undo", "Ctrl+Z", ws.done.length > 0 ? ws.done[ws.done.length - 1] : "Nothing to undo",
                     () => ws.dispatch("history", {action: "undo"})))
        out.push(row("Redo", "Ctrl+Shift+Z", ws.undone.length > 0 ? ws.undone[ws.undone.length - 1] : "Nothing to redo",
                     () => ws.dispatch("history", {action: "redo"})))
        if (ws.doc !== "" && ws.previews) {
            const state = preview !== null ? preview.state : "live"
            for (const t of ["Desktop", "Tablet", "Phone"])
                out.push(row("Show on " + t, "", "preview_open",
                             () => ws.dispatch("preview_open", {doc: ws.doc, target: t.toLowerCase(), state: state})))
            if (preview !== null) {
                const mocked = preview.state === "mocked"
                out.push(row(mocked ? "Use live data" : "Use mocked data", "", "preview_open",
                             () => ws.dispatch("preview_open", {doc: ws.doc, target: preview.target.toLowerCase(),
                                                                state: mocked ? "live" : "mocked"})))
            }
        }
        out.push(row(Colors.dark ? "Light look" : "Dark look", "", "Switch the theme", () => { Colors.dark = !Colors.dark }))
        const bar = toolbar
        if (bar !== null) {
            out.push(row((bar.leftOpen ? "Hide" : "Show") + " the left panels", "", "Layers, documents and insert",
                         () => { bar.leftOpen = !bar.leftOpen }))
            out.push(row((bar.rightOpen ? "Hide" : "Show") + " the right panels", "", "Design, tokens and flow",
                         () => { bar.rightOpen = !bar.rightOpen }))
            out.push(row((bar.codeOpen ? "Hide" : "Show") + " the code", "", "", () => { bar.codeOpen = !bar.codeOpen }))
        }
        const ex = exporter
        if (ex !== null)
            out.push(row("Export…", "Ctrl+E", "Write a bundle", () => ex.show()))
        for (const doc of ws.documents) {
            if (doc !== ws.doc)
                out.push(row("Open " + doc, "", "Show it on the canvas", () => {
                    ws.doc = doc
                    if (ws.previews)
                        ws.dispatch("preview_open", {doc: doc})
                }))
        }
        if (ws.doc !== "") {
            const seen = {}
            for (const group of ws.toolkit) {
                for (const type of group.types) {
                    if (seen[type])
                        continue
                    seen[type] = true
                    out.push(row("Insert " + type, "", group.module || "this project", () => {
                        const into = insertTarget()
                        if (into === "")
                            return
                        const place = {doc: ws.doc, parent: {node: into}, text: starter(type)}
                        if (group.module !== "")
                            place.import = group.module
                        ws.dispatch("node_insert", place)
                    }))
                }
            }
        }
        // A Studio that does not list its tools has only the rows above.
        const tools = ws["tools"] || []
        for (const tool of tools)
            out.push(row(tool.name, "", tool.description, () => ask(tool)))
        return out
    }

    // ------------------------------------------------------------ finding

    // How well `needle`'s letters, in order, fit `hay`: higher is better,
    // -1 is not at all. A letter that starts a word or follows the last one
    // counts for more, so "zm100" finds "Zoom to 100%" and "ins rect"
    // "Insert Rectangle". Spaces in what is typed are not letters to find.
    function score(needle, hay) {
        const h = hay.toLowerCase()
        let at = 0
        let total = 0
        let last = -2
        for (let i = 0; i < needle.length; i++) {
            const c = needle[i]
            if (c === " ")
                continue
            const found = h.indexOf(c, at)
            if (found < 0)
                return -1
            const wordStart = found === 0 || /[\s_\-.·]/.test(h[found - 1])
            total += 1 + (found === last + 1 ? 2 : 0) + (wordStart ? 3 : 0)
            last = found
            at = found + 1
        }
        // Of two that fit as well, the shorter is the closer.
        return total - h.length / 100
    }
    readonly property var rows: {
        const needle = query.text.trim().toLowerCase()
        const used = recent
        const scored = []
        for (const a of actions) {
            const s = needle === "" ? 0 : score(needle, a.name)
            if (s < 0)
                continue
            const lately = used.indexOf(a.name)
            scored.push({a: a, s: s + (lately >= 0 ? 4 - lately / used.length : 0), lately: lately})
        }
        scored.sort((p, q) => {
            if (needle === "") {
                // Lately used first, in the order used; then as listed.
                const pl = p.lately < 0 ? Infinity : p.lately
                const ql = q.lately < 0 ? Infinity : q.lately
                return pl - ql
            }
            return q.s - p.s
        })
        return scored.map(p => p.a)
    }
    onRowsChanged: list.currentIndex = 0

    function run(index) {
        const a = rows[index]
        if (a === undefined)
            return
        recent = [a.name].concat(recent.filter(n => n !== a.name)).slice(0, 12)
        // The palette closes first: what runs may open a panel of its own.
        if (asking === null)
            close()
        a.run()
    }

    // A tool by name: the palette asks for its arguments, the document
    // shown already filled in when the tool takes one.
    function ask(tool) {
        asking = tool
        refusal = ""
        const args = {}
        if ((tool.args || []).indexOf("doc") >= 0 && workspace.doc !== "")
            args.doc = workspace.doc
        query.text = JSON.stringify(args)
        query.forceActiveFocus()
    }
    function send() {
        let args
        try {
            args = JSON.parse(query.text)
        } catch (e) {
            refusal = "Not JSON: " + e.message
            return
        }
        if (args === null || typeof args !== "object" || Array.isArray(args)) {
            refusal = "The arguments are one JSON object"
            return
        }
        const name = asking.name
        close()
        workspace.dispatch(name, args)
    }
    onClosed: {
        asking = null
        refusal = ""
    }

    background: Rectangle {
        radius: 8
        color: Colors.surface
        border.color: Colors.border
        Rectangle {
            x: 0
            y: 6
            z: -1
            width: parent.width
            height: parent.height
            radius: parent.radius
            color: Colors.shadow
            opacity: 0.2
        }
    }

    contentItem: ColumnLayout {
        id: column
        spacing: Spacing.xs
        Accessible.role: Accessible.Pane
        Accessible.name: "Command palette"

        Field {
            id: query
            objectName: "commandQuery"
            Layout.fillWidth: true
            implicitHeight: 36
            font.pixelSize: Typography.bodySize
            placeholderText: palette.asking !== null ? "Arguments for " + palette.asking.name + ", as JSON"
                                                     : "Find an action, a tool, a type or a document"
            Accessible.name: palette.asking !== null ? "Arguments" : "Find an action"
            onTextEdited: palette.refusal = ""
            Keys.onUpPressed: list.currentIndex = Math.max(0, list.currentIndex - 1)
            Keys.onDownPressed: list.currentIndex = Math.min(list.count - 1, list.currentIndex + 1)
            Keys.onPressed: (event) => {
                if (palette.asking !== null)
                    return
                if (event.key === Qt.Key_PageDown) {
                    list.currentIndex = Math.min(list.count - 1, list.currentIndex + 8)
                    event.accepted = true
                } else if (event.key === Qt.Key_PageUp) {
                    list.currentIndex = Math.max(0, list.currentIndex - 8)
                    event.accepted = true
                }
            }
            onAccepted: palette.asking !== null ? palette.send() : palette.run(list.currentIndex)
        }

        // What the tool asked for does and takes.
        Label {
            visible: palette.asking !== null
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: palette.asking === null ? "" : palette.asking.description
                  + ((palette.asking.args || []).length > 0 ? "\nTakes: " + palette.asking.args.join(", ") : "")
                  + ((palette.asking.needs || []).length > 0 ? "\nNeeds: " + palette.asking.needs.join(", ") : "")
            color: Colors.textMuted
            font.pixelSize: Typography.captionSize
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
        Label {
            visible: palette.refusal !== ""
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            text: palette.refusal
            color: Colors.danger
            font.pixelSize: Typography.captionSize
            Accessible.role: Accessible.AlertMessage
            Accessible.name: text
        }

        ListView {
            id: list
            visible: palette.asking === null
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(count, 10) * 36
            clip: true
            model: palette.rows
            boundsBehavior: Flickable.StopAtBounds
            highlightMoveDuration: 0
            Accessible.role: Accessible.List
            Accessible.name: "Commands"

            delegate: ItemDelegate {
                id: rowItem
                required property var modelData
                required property int index
                width: ListView.view.width
                height: 36
                padding: 0
                leftPadding: Spacing.sm
                rightPadding: Spacing.sm
                highlighted: ListView.isCurrentItem
                Accessible.role: Accessible.ListItem
                Accessible.name: modelData.name
                Accessible.description: modelData.shortcut
                background: Rectangle {
                    radius: 6
                    color: rowItem.highlighted ? Colors.surfaceSelected
                        : rowItem.hovered ? Colors.surfaceHover : "transparent"
                }
                contentItem: RowLayout {
                    spacing: Spacing.sm
                    Label {
                        text: rowItem.modelData.name
                        color: Colors.text
                        font.pixelSize: Typography.bodySize
                        elide: Text.ElideRight
                        Layout.maximumWidth: rowItem.width * 0.6
                        Accessible.ignored: true
                    }
                    Label {
                        text: rowItem.modelData.detail
                        color: Colors.textMuted
                        font.pixelSize: Typography.captionSize
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                        Accessible.ignored: true
                    }
                    // The key that does the same without the palette.
                    Label {
                        visible: rowItem.modelData.shortcut !== ""
                        text: rowItem.modelData.shortcut
                        color: Colors.textMuted
                        font.pixelSize: Typography.captionSize
                        leftPadding: Spacing.xxs
                        rightPadding: Spacing.xxs
                        background: Rectangle {
                            radius: 4
                            color: Colors.surfaceRaised
                            border.color: Colors.surfacePressed
                        }
                        Accessible.ignored: true
                    }
                }
                onClicked: palette.run(index)
            }
        }
        Label {
            visible: palette.asking === null && list.count === 0
            Layout.fillWidth: true
            text: "Nothing is called that"
            color: Colors.textMuted
            font.pixelSize: Typography.captionSize
            padding: Spacing.xs
        }
    }
}
