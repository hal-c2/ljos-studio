// Export a bundle: the native app `ljos run` starts or the site ljos-web
// builds, as the `export` call an agent makes. Opening the panel (and
// every change of what to export) asks for a dry run, so the list of what
// the bundle would hold, what is left out and why, and whether it replaces
// a bundle already there are on screen before the person writes anything.
// What the panel shows is the last answer or refusal of `export`; it keeps
// no other state about the project. A site's page (title, description,
// pictures) is the project's `ljos-page.json`, which an export with a `page`
// writes; "Build and open" is the same call with `build` and `open`, run
// aside from the window's queue since a build takes as long as ljos-web.
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
    // the fields say only when none has, and nothing is being typed.
    property int waiting: 0
    readonly property bool busy: waiting > 0 || settle.running
    // The document to export. Kept here and not in the box: a new list of
    // documents, which any tool's answer brings, sets the box's row anew.
    property string chosen: ""
    // Whether the answer in view was a real export, not a dry run.
    readonly property bool wrote: answer !== null && answer.written

    // Whether the person changed the page's fields since the panel opened:
    // until then the export reads the project's ljos-page.json, and the fields
    // show what it says.
    property bool pageEdited: false
    // A build: running, and what the last one answered, or why it failed.
    property bool building: false
    property var built: null
    property string buildRefusal: ""
    // Whether the last check found what a build needs.
    property bool buildReady: false
    // Where the site's links are made: a block a page uses as any other.
    readonly property string linkBlock: "Site/Link.qml"

    readonly property string target: web.checked ? "web" : "native"
    readonly property string entry: workspace.documents.indexOf(chosen) >= 0 ? chosen : ""

    // Main.qml when the project has one, else the document shown, else the
    // first.
    function defaultEntry() {
        const docs = workspace.documents
        if (docs.indexOf("Main.qml") >= 0)
            return "Main.qml"
        if (docs.indexOf(workspace.doc) >= 0)
            return workspace.doc
        return docs.length > 0 ? docs[0] : ""
    }

    // The page as the fields say it, each field left empty left out.
    function page() {
        const p = {}
        const fields = {title: title, description: description, icon: icon, image: image}
        for (const key in fields) {
            const text = fields[key].text.trim()
            if (text !== "")
                p[key] = text
        }
        return p
    }

    function args(dry, builds) {
        const a = {entry: entry, target: target, dry_run: dry}
        if (out.text.trim() !== "")
            a.out = out.text.trim()
        if (target === "web") {
            if (pageEdited)
                a.page = page()
            // A check says whether a build would find what it needs. Writing
            // the bundle builds nothing, and only a build takes a folder to
            // build with.
            if (dry || builds) {
                a.build = true
                if (builder.text.trim() !== "")
                    a.ljos_web = builder.text.trim()
            }
        }
        return a
    }

    function run(dry) {
        if (entry === "")
            return
        // What a build made is of the export before this one.
        built = null
        buildRefusal = ""
        waiting += 1
        workspace.dispatch("export", args(dry, false))
    }

    // Exports, builds and opens the site in the person's browser.
    function build() {
        if (entry === "" || building)
            return
        const a = args(false, true)
        a.open = true
        building = true
        built = null
        buildRefusal = ""
        workspace.aside("export", a)
    }

    // The block a page's links are made of: on the web an `a` the browser
    // follows, in an app a tap that opens the address in the browser.
    function addLink() {
        workspace.dispatch("document_new", {doc: linkBlock, block: true, text: [
            "import QtQuick",
            "import Ljos.Design",
            "",
            "// A link: on the web an `a` with this `href`, which the browser follows and",
            "// a crawler or a screen reader reads as a link; in an app, a tap opens the",
            "// address in the person's browser.",
            "Text {",
            "    id: link",
            "    property url href",
            "    text: \"Link\"",
            "    color: Colors.link",
            "    font.underline: hover.hovered",
            "    Accessible.role: Accessible.Link",
            "    Accessible.name: link.text",
            "    HoverHandler {",
            "        id: hover",
            "        cursorShape: Qt.PointingHandCursor",
            "    }",
            "    TapHandler {",
            "        // On the web the page's `a` is followed by the browser itself.",
            "        enabled: Qt.platform.os !== \"wasm\"",
            "        onTapped: Qt.openUrlExternally(link.href)",
            "    }",
            "}",
            ""].join("\n")})
    }

    // The fields say what the project's ljos-page.json says until the person
    // changes one.
    function showPage(a) {
        if (pageEdited || a === null || !a.manifest || a.manifest.target !== "web")
            return
        const p = a.manifest.page || {}
        title.text = p.title || ""
        description.text = p.description || ""
        icon.text = p.icon || ""
        image.text = p.image || ""
    }

    function check() {
        run(true)
    }

    function show() {
        answer = null
        refusal = ""
        pageEdited = false
        built = null
        buildRefusal = ""
        buildReady = false
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
        const rows = ["Entry " + m.entry, "Manifest " + manifestName(a)]
        if (m.page) {
            const p = m.page
            rows.push("Page " + (p.title || "with no title") + (p.description ? ": " + p.description : ""))
            if (p.icon)
                rows.push("Icon " + p.icon)
            if (p.image)
                rows.push("Picture for shared links " + p.image)
        }
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

    function manifestName(a) {
        return a.manifest.target === "web" ? "ljos-site.json" : "ljos-bundle.json"
    }

    // As many files as the export writes: the project's and its manifest.
    function summary(a) {
        if (a === null)
            return busy ? "Checking…" : ""
        return (a.written ? "Wrote " : "Would write ") + a.writes + " files to " + a.out + ": "
            + a.files + " of the project's (" + size(a.bytes)
            + ") and " + manifestName(a)
    }

    // Whether the export (and, for a site, a build) can go ahead, as the
    // last check found.
    function verdict(a) {
        if (a === null || a.written || refusal !== "")
            return ""
        if (a.manifest.target !== "web")
            return "Ready to write"
        const b = a.build
        if (!b)
            return "Ready to write"
        if (b.ready)
            return "Ready to write and to build: Build and open uses the ljos-web in " + b.ljos_web
        return "Ready to write; to build the site here Studio needs: " + b.missing.join("; ")
    }

    // What the build is doing or did, in plain words.
    function buildSays() {
        if (building)
            return "Building the site with ljos-web…"
        if (built === null)
            return ""
        return "Built the site, " + size(built.bytes) + ", into " + built.site + ". It is served at "
            + built.url + (built.opened ? ", which your browser was asked to open" : "")
    }

    function buildSteps() {
        if (building || built === null)
            return []
        return (built.steps || []).map(s => s.what + " (" + Math.round(s.ms) + " ms)")
    }

    // A word as a shell reads it, for a line the person pastes.
    function quoted(word) {
        if (/^[A-Za-z0-9_\/.:=+-]+$/.test(word))
            return word
        return "'" + word.replace(/'/g, "'\\''") + "'"
    }

    // What to do with the bundle next.
    function next(a) {
        if (a === null || !a.written)
            return ""
        if (a.manifest.run)
            return "Start it with: " + a.manifest.run.map(quoted).join(" ") + "  (in " + quoted(a.path) + ")"
        if (a.build)
            return ""
        return "Build and open makes the site from it and opens it in your browser"
    }

    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay
    width: Math.min(640, parent.width - Spacing.lg * 2)
    height: Math.min(target === "web" ? 720 : 560, parent.height - Spacing.lg * 2)
    modal: true
    focus: true
    padding: Spacing.sm
    closePolicy: Popup.CloseOnEscape
    onClosed: settle.stop()

    // A directory half typed is no directory: it is checked when typing
    // pauses, so each letter does not flash a refusal.
    Timer {
        id: settle
        interval: 250
        onTriggered: panel.check()
    }

    Connections {
        target: panel.workspace
        // The list is only as new as the last answer, which may be the one
        // that says the chosen document is gone.
        function onDocumentsChanged() {
            if (!panel.visible || panel.entry !== "")
                return
            panel.chosen = panel.defaultEntry()
            panel.check()
        }
        function onAnswered(tool, answer, asked) {
            if (tool !== "export")
                return
            if (asked.build === true && asked.dry_run !== true) {
                panel.building = false
                panel.built = answer.build
                panel.refusal = ""
                panel.answer = answer
                return
            }
            panel.waiting -= 1
            panel.refusal = ""
            panel.answer = answer
            if (asked.dry_run === true)
                panel.buildReady = answer.build !== undefined && answer.build.ready === true
            panel.showPage(answer)
        }
        function onRefused(tool, error, asked) {
            if (tool !== "export")
                return
            if (asked.build === true && asked.dry_run !== true) {
                panel.building = false
                const d = error.details || {}
                panel.buildRefusal = error.message
                    + (d.step ? ". It stopped at: " + d.step : "")
                    + (d.log ? ". What ljos-web said is in " + d.log : "")
                return
            }
            panel.waiting -= 1
            panel.answer = null
            panel.buildReady = false
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
                onClicked: {
                    panel.buildReady = false
                    panel.check()
                }
            }
            RadioButton {
                id: web
                text: "Web site"
                Accessible.name: text
                ButtonGroup.group: kinds
                onClicked: {
                    panel.buildReady = false
                    panel.check()
                }
            }
        }

        RowLayout {
            spacing: Spacing.xs
            Label {
                text: "Entry"
                color: Colors.text
                // As wide as the page's widest label, so the fields line up.
                Layout.preferredWidth: widest.implicitWidth
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
                Layout.preferredWidth: widest.implicitWidth
            }
            Field {
                id: out
                placeholderText: "build/" + panel.target
                Accessible.name: "Output directory"
                Layout.fillWidth: true
                // As typed, so "Write bundle" never writes somewhere the
                // warning above it has not looked at.
                onTextEdited: settle.restart()
                onAccepted: {
                    settle.stop()
                    panel.check()
                }
            }
        }

        // The site's page: what a browser's tab, a search result and a
        // shared link show of it, and the ljos-web it is built with.
        GridLayout {
            visible: panel.target === "web"
            columns: 2
            columnSpacing: Spacing.xs
            rowSpacing: Spacing.xxs
            Layout.fillWidth: true

            Label {
                text: "Title"
                color: Colors.text
            }
            Field {
                id: title
                placeholderText: "the entry's name"
                Accessible.name: "Page title"
                Layout.fillWidth: true
                onTextEdited: panel.pageEdited = true
                onAccepted: panel.check()
            }
            Label {
                id: widest
                text: "Description"
                color: Colors.text
            }
            Field {
                id: description
                placeholderText: "what a search result says of the page"
                Accessible.name: "Page description"
                Layout.fillWidth: true
                onTextEdited: panel.pageEdited = true
                onAccepted: panel.check()
            }
            Label {
                text: "Icon"
                color: Colors.text
            }
            Field {
                id: icon
                placeholderText: "a picture of the project, as images/icon.svg"
                Accessible.name: "Page icon"
                Layout.fillWidth: true
                onTextEdited: {
                    panel.pageEdited = true
                    settle.restart()
                }
                onAccepted: {
                    settle.stop()
                    panel.check()
                }
            }
            Label {
                text: "Picture"
                color: Colors.text
            }
            Field {
                id: image
                placeholderText: "the picture a shared link shows"
                Accessible.name: "Page picture for shared links"
                Layout.fillWidth: true
                onTextEdited: {
                    panel.pageEdited = true
                    settle.restart()
                }
                onAccepted: {
                    settle.stop()
                    panel.check()
                }
            }
            Label {
                text: "ljos-web"
                color: Colors.text
            }
            Field {
                id: builder
                placeholderText: "found by itself"
                Accessible.name: "ljos-web folder"
                Layout.fillWidth: true
                onTextEdited: settle.restart()
                onAccepted: {
                    settle.stop()
                    panel.check()
                }
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

        Label {
            objectName: "exportVerdict"
            text: panel.verdict(panel.answer)
            visible: text !== ""
            color: Colors.text
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

        // An answer over the size an answer may be holds part of its lists.
        Label {
            objectName: "exportCut"
            text: panel.answer !== null && panel.answer.truncated
                ? "This list is cut short: the bundle holds all " + panel.answer.files
                    + " files, and its manifest lists them and what is left out" : ""
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
            objectName: "exportBuild"
            text: panel.buildSays()
            visible: text !== ""
            color: Colors.text
            font.weight: Typography.bodyStrongWeight
            wrapMode: Text.Wrap
            Accessible.role: Accessible.StaticText
            Accessible.name: text
            Layout.fillWidth: true
        }

        // Why the build stopped, in the builder's words.
        Label {
            objectName: "exportBuildRefusal"
            text: panel.buildRefusal
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
            id: files
            objectName: "exportFiles"
            clip: true
            model: panel.buildSteps().concat(panel.lines(panel.answer))
            Accessible.role: Accessible.List
            Accessible.name: "Bundle contents"
            Layout.fillWidth: true
            Layout.fillHeight: true
            ScrollBar.vertical: ScrollBar {
                Accessible.name: "Scroll the bundle's contents"
                // As the documents' bar: its last row, cut in half above the
                // buttons, is all else that says the list goes on.
                policy: ScrollBar.AlwaysOn
                visible: files.contentHeight > files.height
            }

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
            TextButton {
                text: "Check"
                Accessible.name: text
                enabled: panel.entry !== "" && !panel.busy
                onClicked: panel.check()
            }
            TextButton {
                text: "Write bundle"
                Accessible.name: text
                // A build reads the bundle this would replace.
                enabled: panel.entry !== "" && !panel.busy && !panel.building
                highlighted: true
                onClicked: panel.run(false)
            }
            TextButton {
                text: "Build and open"
                Accessible.name: text
                visible: panel.target === "web"
                enabled: panel.entry !== "" && !panel.busy && !panel.building && panel.buildReady
                onClicked: panel.build()
            }
            TextButton {
                text: "Copy path"
                Accessible.name: text
                enabled: panel.wrote
                onClicked: {
                    clip.text = panel.built !== null ? panel.built.path : panel.answer.path
                    clip.selectAll()
                    clip.copy()
                }
            }
            TextButton {
                text: "Add a Link block"
                Accessible.name: text
                visible: panel.target === "web"
                enabled: panel.workspace.documents.indexOf(panel.linkBlock) < 0
                onClicked: panel.addLink()
            }
            Item {
                Layout.fillWidth: true
            }
            TextButton {
                text: "Close"
                Accessible.name: text
                onClicked: panel.close()
            }
        }

        // Enter checks, as on a form; the buttons take their own Enter.
        Keys.onReturnPressed: panel.check()
    }
}
