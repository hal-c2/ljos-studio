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
    objectName: "exportPanel"
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

    // Which of the page's fields the person changed since the panel opened.
    // Until one is, the export reads the project's ljos-page.json; the
    // fields not changed show what it says, so a page written whole keeps
    // what the person did not touch.
    property bool titleEdited: false
    property bool descriptionEdited: false
    property bool iconEdited: false
    property bool imageEdited: false
    readonly property bool pageEdited: titleEdited || descriptionEdited || iconEdited || imageEdited
    // A build: running, and what the last one answered, or why it failed.
    property bool building: false
    property var built: null
    // What the last build was asked: its site stays the one to copy while
    // the export checked is the one it built.
    property var builtAsked: null
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
        // A real export replaces the bundle the site was built from; a check
        // of another export is not of that site.
        if (!dry || builtAsked === null || !current(builtAsked)) {
            built = null
            builtAsked = null
        }
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
        builtAsked = null
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

    // Each field says what the project's ljos-page.json says until the
    // person changes it. An answer to a call that sent a page says that
    // page, not the project's.
    function showPage(a, asked) {
        if (a === null || asked.page !== undefined || !a.manifest || a.manifest.target !== "web")
            return
        const p = a.manifest.page || {}
        if (!titleEdited)
            title.text = p.title || ""
        if (!descriptionEdited)
            description.text = p.description || ""
        if (!iconEdited)
            icon.text = p.icon || ""
        if (!imageEdited)
            image.text = p.image || ""
    }

    // Whether an answer to `asked` is of the export the fields choose now:
    // one that began before the person chose another is not.
    function current(asked) {
        return asked.entry === entry && asked.target === target
            && (asked.out || "") === out.text.trim()
    }

    function check() {
        run(true)
    }

    function show() {
        answer = null
        refusal = ""
        titleEdited = false
        descriptionEdited = false
        iconEdited = false
        imageEdited = false
        built = null
        builtAsked = null
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

    // A line's kind, as lines() begins it, and its mark. The build's own
    // lines have none.
    readonly property var kinds_: [
        ["Left out", "close"], ["Picture for shared links", "image"], ["Entry", "play"],
        ["Manifest", "data"], ["Page", "window"], ["Icon", "image"], ["File", "document"],
        ["Block", "component"], ["Theme", "sun"], ["Link", "link"], ["Import", "down"]
    ]
    function lineKind(row) {
        for (const k of kinds_)
            if (row.startsWith(k[0] + " "))
                return {word: k[0], icon: k[1]}
        return {word: "", icon: "check"}
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
                panel.builtAsked = asked
                if (panel.current(asked)) {
                    panel.refusal = ""
                    panel.answer = answer
                }
                return
            }
            panel.waiting -= 1
            panel.refusal = ""
            panel.answer = answer
            if (asked.dry_run === true)
                panel.buildReady = answer.build !== undefined && answer.build.ready === true
            panel.showPage(answer, asked)
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
                // The bundle was written before the build failed.
                if (d.export && panel.current(asked)) {
                    panel.refusal = ""
                    panel.answer = d.export
                }
                return
            }
            panel.waiting -= 1
            // A bundle written before the refusal (its page was not saved)
            // is still what the export did.
            const written = (error.details || {}).export
            panel.answer = written && panel.current(asked) ? written : null
            panel.buildReady = false
            panel.refusal = error.message
        }
    }

    // Text for the clipboard; a field the person never sees.
    TextInput {
        id: clip
        visible: false
    }

    padding: Spacing.md
    // The window behind dims, so the dialog is the one thing to read.
    Overlay.modal: Rectangle {
        color: Qt.rgba(0, 0, 0, Colors.dark ? 0.5 : 0.25)
    }
    background: Rectangle {
        radius: 12
        color: Colors.surfaceRaised
        border.width: 1
        border.color: Colors.surfacePressed
    }

    contentItem: ColumnLayout {
        id: body
        spacing: Spacing.xs
        Accessible.role: Accessible.Pane
        Accessible.name: "Export panel"

        RowLayout {
            spacing: Spacing.xs
            Layout.fillWidth: true
            Layout.bottomMargin: Spacing.xs
            Rectangle {
                implicitWidth: 28
                implicitHeight: 28
                radius: 8
                color: Colors.surfaceSelected
                Icon {
                    anchors.centerIn: parent
                    name: "export"
                    color: Colors.link
                }
            }
            Label {
                text: "Export"
                font.pixelSize: Typography.titleSize
                font.weight: Typography.titleWeight
                color: Colors.text
                Layout.fillWidth: true
            }
        }

        // The two kinds as one segmented switch; each is still a radio
        // button to a reader.
        Rectangle {
            implicitHeight: 32
            radius: 8
            color: Colors.surfaceHover
            Layout.fillWidth: true
            ButtonGroup {
                id: kinds
            }
            RowLayout {
                anchors.fill: parent
                anchors.margins: 2
                spacing: 2
                Repeater {
                    model: [
                        {kind: "native", text: "Native app", icon: "window"},
                        {kind: "web", text: "Web site", icon: "code"}
                    ]
                    delegate: RadioButton {
                        id: kindButton
                        required property var modelData
                        text: modelData.text
                        Accessible.name: text
                        checked: modelData.kind === "native"
                        ButtonGroup.group: kinds
                        hoverEnabled: true
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        onClicked: {
                            panel.buildReady = false
                            panel.check()
                        }
                        onCheckedChanged: if (modelData.kind === "web") web.checked = checked
                        indicator: Item { }
                        contentItem: Row {
                            spacing: Spacing.xxs
                            Item {
                                width: (kindButton.availableWidth - mark.width - word.implicitWidth - Spacing.xxs) / 2
                                height: 1
                            }
                            Icon {
                                id: mark
                                name: kindButton.modelData.icon
                                size: 14
                                color: kindButton.checked ? Colors.link : Colors.textMuted
                                anchors.verticalCenter: parent.verticalCenter
                            }
                            Text {
                                id: word
                                text: kindButton.text
                                color: kindButton.checked ? Colors.text : Colors.textMuted
                                font.pixelSize: Typography.bodySize
                                font.weight: kindButton.checked ? Typography.bodyStrongWeight : Typography.bodyWeight
                                anchors.verticalCenter: parent.verticalCenter
                                Accessible.ignored: true
                            }
                        }
                        background: Rectangle {
                            radius: 6
                            color: kindButton.checked ? Colors.surfaceRaised
                                : kindButton.hovered ? Colors.surfacePressed : "transparent"
                            border.width: kindButton.checked ? 1 : 0
                            border.color: Colors.surfacePressed
                            Behavior on color { ColorAnimation { duration: Motion.enterSmallFrequent; easing.type: Motion.easeOut } }
                        }
                    }
                }
            }
        }
        // Which kind is picked, for the rest of the panel to read.
        QtObject {
            id: web
            property bool checked: false
        }

        RowLayout {
            spacing: Spacing.xs
            Label {
                text: "Entry"
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
                // As wide as the page's widest label, so the fields line up.
                Layout.preferredWidth: widest.implicitWidth
            }
            Choice {
                id: entries
                leadIcon: "document"
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
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
                Layout.preferredWidth: widest.implicitWidth
            }
            Field {
                id: out
                leadIcon: "document"
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
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
            }
            Field {
                id: title
                placeholderText: "the entry's name"
                Accessible.name: "Page title"
                Layout.fillWidth: true
                onTextEdited: panel.titleEdited = true
                onAccepted: panel.check()
            }
            Label {
                id: widest
                text: "Description"
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
            }
            Field {
                id: description
                placeholderText: "what a search result says of the page"
                Accessible.name: "Page description"
                Layout.fillWidth: true
                onTextEdited: panel.descriptionEdited = true
                onAccepted: panel.check()
            }
            Label {
                text: "Icon"
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
            }
            Field {
                id: icon
                placeholderText: "a picture of the project, as images/icon.svg"
                Accessible.name: "Page icon"
                Layout.fillWidth: true
                onTextEdited: {
                    panel.iconEdited = true
                    settle.restart()
                }
                onAccepted: {
                    settle.stop()
                    panel.check()
                }
            }
            Label {
                text: "Picture"
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
            }
            Field {
                id: image
                placeholderText: "the picture a shared link shows"
                Accessible.name: "Page picture for shared links"
                Layout.fillWidth: true
                onTextEdited: {
                    panel.imageEdited = true
                    settle.restart()
                }
                onAccepted: {
                    settle.stop()
                    panel.check()
                }
            }
            Label {
                text: "ljos-web"
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
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
            padding: Spacing.xs
            background: Rectangle {
                radius: 6
                color: Colors.surfaceSelected
            }
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
                radius: 6
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
                radius: 6
                color: Colors.danger
            }
            Layout.fillWidth: true
        }

        Label {
            objectName: "exportNext"
            text: panel.next(panel.answer)
            visible: text !== ""
            color: Colors.text
            font.family: "monospace"
            font.pixelSize: Typography.captionSize
            wrapMode: Text.Wrap
            padding: Spacing.xs
            background: Rectangle {
                radius: 6
                color: Colors.surfaceHover
            }
            Accessible.role: Accessible.StaticText
            Accessible.name: text
            Layout.fillWidth: true
        }

        ListView {
            id: files
            objectName: "exportFiles"
            clip: true
            spacing: 2
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

            // The kind in a quiet column and a mark for it, then what it is.
            // A line of the build's own, timed, has no kind and a tick.
            delegate: RowLayout {
                id: line
                required property string modelData
                readonly property var kind: panel.lineKind(modelData)
                width: ListView.view.width
                spacing: Spacing.xs
                Accessible.role: Accessible.StaticText
                Accessible.name: modelData
                Icon {
                    name: line.kind.icon
                    size: 14
                    color: line.kind.word === "Left out" ? Colors.textMuted : Colors.link
                }
                Text {
                    text: line.kind.word === "Picture for shared links" ? "Picture" : line.kind.word
                    color: Colors.textMuted
                    font.pixelSize: Typography.captionSize
                    elide: Text.ElideRight
                    Layout.preferredWidth: 52
                    Accessible.ignored: true
                }
                Text {
                    text: line.kind.word === "" ? line.modelData : line.modelData.slice(line.kind.word.length + 1)
                    color: line.kind.word === "Left out" ? Colors.textMuted : Colors.text
                    font.pixelSize: Typography.captionSize
                    elide: Text.ElideMiddle
                    Layout.fillWidth: true
                    Accessible.ignored: true
                }
            }
        }

        // The way out on the left, the one thing to do on the right.
        RowLayout {
            spacing: Spacing.xs
            Layout.topMargin: Spacing.xs
            TextButton {
                text: "Copy path"
                Accessible.name: text
                // A built site stays when a check answers after it.
                enabled: panel.built !== null || panel.wrote
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
                accent: true
                icon_: "export"
                onClicked: panel.run(false)
            }
            // After the bundle, and only once a check finds the site can be built.
            TextButton {
                text: "Build and open"
                Accessible.name: text
                visible: panel.target === "web"
                enabled: panel.entry !== "" && !panel.busy && !panel.building && panel.buildReady
                onClicked: panel.build()
            }
        }

        // Enter checks, as on a form; the buttons take their own Enter.
        Keys.onReturnPressed: panel.check()
    }
}
