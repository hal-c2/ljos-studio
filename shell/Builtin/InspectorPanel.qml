// The selected object's bindings as written, and the properties it declares
// without a value. What is typed is read for what it is, whatever the row held:
// a literal is written as typed, a colour's name or `#` in a colour row as
// text, a design role or any expression bound; a bare word that names nothing
// is asked about. "Bind" binds whatever is typed. The last row adds a property
// not yet bound; × takes one away. A colour has a swatch, which opens a picker.
//
// Below them, its states (`state_edit`): one is picked to edit the values
// it changes and the condition that turns it on, or to give it a
// breakpoint, a width bound of the window. While one is picked, the frame
// shows it (`preview_state`) and what this panel sets goes into it, whichever
// object is selected, until the person is done.
//
// Then the signals it has, with their arguments, which the flow panel
// wires; and the roles of the data it sees, in a list's delegate: one is
// bound to a property as `model.<role>`, the form a delegate reads it in.
//
// Last, its review comments (`review`), and those of the document whose
// objects were taken out: a person adds one, resolves it or reopens it.
// They are kept in Studio's sidecar; the file does not change.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Panel {
    id: panel
    required property Workspace workspace
    title: "Inspector"

    // The new-property row's edit: answered, the row is emptied for the next.
    property bool rowEditing: false
    // The same for the row that adds a change to the picked state.
    property bool changing: false

    // What was refused, and the slot whose note says it: the field the person
    // last entered, or the row of fields it is in. Every field keeps a line
    // under it for this, so being refused moves nothing the person aims at.
    QtObject {
        id: refusal
        property string text: ""
        property string at: ""
        // The call this panel sent last, until it is answered: what the file
        // refuses of it is said where it was asked, and what others asked is
        // left to the window's error line.
        property string asking: ""
    }
    function says(slot) {
        return refusal.at === slot ? refusal.text : ""
    }
    function offering(slot) {
        return refusal.at === slot && offered !== null && refusal.text === offeredSays
    }
    function offersBind(slot) {
        return offering(slot) ? "Bind " + offered.name + " to " + offered.value : ""
    }
    function offersText(slot) {
        return offering(slot) && offered.asText ? "Write " + offered.value + " as text" : ""
    }
    function takeOffer(asText) {
        if (asText)
            set(offered.name, JSON.stringify(offered.value))
        else
            bind(offered.name, offered.value)
    }
    function asked(tool, args) {
        return tool + ":" + (args.name !== undefined ? args.name : args.action !== undefined ? args.action : "")
    }
    // Every write this panel makes goes through here, from the slot `at`.
    function ask(tool, args) {
        refusal.asking = asked(tool, args)
        workspace.dispatch(tool, args)
    }

    function fromRow(edit) {
        refusal.at = "new"
        rowEditing = true
        edit()
        // Stopped before it was asked: there will be no answer.
        if (refusal.text !== "")
            rowEditing = false
    }

    readonly property var selectedLayer: {
        const all = workspace.layers
        for (let i = 0; i < all.length; i++) {
            if (all[i].node === workspace.selected)
                return {at: i, layer: all[i]}
        }
        return null
    }
    readonly property string selectedType: selectedLayer !== null ? selectedLayer.layer.type : ""
    readonly property bool isRoot: selectedLayer !== null && selectedLayer.at === 0
    readonly property bool isLayout: ["Row", "Column", "Grid", "Flow", "RowLayout", "ColumnLayout", "GridLayout"]
        .indexOf(selectedType) >= 0
    // The nearest layer above in the list, one less deep, is its parent.
    readonly property string placedBy: {
        if (selectedLayer === null)
            return ""
        const all = workspace.layers
        for (let i = selectedLayer.at - 1; i >= 0; i--) {
            if (all[i].depth < selectedLayer.layer.depth) {
                const type = all[i].type
                return ["Row", "Column", "Grid", "Flow", "RowLayout", "ColumnLayout", "GridLayout", "StackLayout"]
                    .indexOf(type) >= 0 ? type : ""
            }
        }
        return ""
    }
    function written(name) {
        const row = workspace.inspector.find(r => r.name === name)
        return row !== undefined ? row.value : ""
    }
    function isPinned(side) {
        // A centring holds it on both centre lines.
        if (side === "horizontalCenter" || side === "verticalCenter")
            return written("anchors." + side) !== "" || written("anchors.centerIn") !== ""
        return written("anchors." + side) !== ""
    }
    // Whether the selected object takes `name`, written yet or not.
    function has(name) {
        return offer(name) !== null || written(name) !== ""
    }
    // The document reads the theme, so a value may name one of its roles.
    readonly property bool themed: /^\s*import\s+Ljos\.Design\b/m.test(workspace.text)
    // What the selected object's kind is about, in a section of its own:
    // its rows whether the file writes them yet or not, so nobody has to
    // know their names to find them.
    readonly property var kindSection: {
        if (workspace.selected === "")
            return null
        const rows = list => list.filter(r => has(r.name))
        if (has("font.pixelSize")) {
            return {
                title: "Text",
                rows: rows([
                    {name: "font.pixelSize", lead: "Size"},
                    {name: "font.weight", lead: "Weight"},
                    {name: "wrapMode", lead: "Wrap"}
                ]),
                styles: themed,
                aligns: has("horizontalAlignment")
            }
        }
        if (has("fillMode") && (has("sourceSize") || has("sourceSize.width"))) {
            return {
                title: "Image",
                rows: [
                    {name: "fillMode", lead: "Fit"},
                    {name: "sourceSize.width", lead: "Decode W"},
                    {name: "sourceSize.height", lead: "Decode H"}
                ]
            }
        }
        // A view makes its own content from its model; a plain scrolling
        // area holds what is put in it, and its content is that size.
        if (has("contentHeight") && has("contentY")) {
            return {
                title: "Scrolling",
                rows: rows([
                    {name: "contentWidth", lead: "Content W", fits: !has("model")},
                    {name: "contentHeight", lead: "Content H", fits: !has("model")}
                ])
            }
        }
        if (selectedType === "Item")
            return {title: "Fill", rows: [], fill: true}
        return null
    }
    readonly property var kindNames: kindSection !== null ? kindSection.rows.map(r => r.name) : []
    // The theme's text styles: a size and a weight that go together.
    readonly property var textStyles: [
        {key: "caption", label: "Caption"},
        {key: "body", label: "Body"},
        {key: "bodyStrong", label: "Body strong"},
        {key: "subtitle", label: "Subtitle"},
        {key: "title", label: "Title"},
        {key: "headline", label: "Headline"},
        {key: "display", label: "Display"}
    ]
    function textStyle() {
        const size = inState("font.pixelSize", written("font.pixelSize")).trim()
        return textStyles.findIndex(t => size === "Typography." + t.key + "Size")
    }
    function giveStyle(key) {
        refusal.at = "kind"
        set("font.pixelSize", "Typography." + key + "Size")
        set("font.weight", "Typography." + key + "Weight")
    }
    // Writes what a row of the kind's section says, as a row of the list does.
    function put(name, text) {
        const row = workspace.inspector.find(r => r.name === name)
        if (text.trim() !== "")
            enter(row !== undefined ? row : {name: name, value: ""}, text)
        else if (inState(name, written(name)) !== "") {
            refusal.at = name
            set(name, "null")
        }
    }
    // A scrolling area's content is as big as its first child, or as what
    // all its children cover when that one has no name to read.
    function fitContent(name) {
        const across = name === "contentWidth"
        const all = workspace.layers
        const at = selectedLayer.at
        const child = at + 1 < all.length && all[at + 1].depth === selectedLayer.layer.depth + 1 ? all[at + 1] : null
        refusal.at = name
        set(name, child !== null && child.id !== ""
            ? child.id + (across ? ".implicitWidth" : ".implicitHeight")
            : "contentItem.childrenRect." + (across ? "width" : "height"))
    }
    // A plain box turned into one that can be filled, then filled: the page
    // with the theme's canvas, anything else with its surface.
    property bool filling: false
    function giveFill() {
        filling = true
        refusal.at = "kind"
        refusal.text = ""
        ask("node_arrange", {
            action: "relayout",
            doc: workspace.doc,
            nodes: [{node: workspace.selected}],
            name: "Rectangle"
        })
    }
    function filled(node) {
        filling = false
        refusal.at = "kind"
        set("color", themed ? (isRoot ? "Colors.canvas" : "Colors.surface") : '"white"', node)
    }

    // What the selected object declares of its own: each property, signal
    // and function at its own depth of the text, in the order written. A
    // child's are the child's, and strings and comments say nothing.
    readonly property var declared: {
        const at = workspace.selection
        if (workspace.selected === "" || at === null || at === undefined || at.start === undefined)
            return []
        const text = workspace.text.slice(at.start, at.end)
        const member = /^(?:(?:readonly|required|default)\s+)*(?:property\s+[\w.<>]+\s+([A-Za-z_$][\w$]*)|signal\s+([A-Za-z_$][\w$]*)|function\s+([A-Za-z_$][\w$]*))/
        const out = []
        let depth = 0
        let words = ""
        const end = () => {
            const said = words.trim()
            const m = member.exec(said)
            if (m !== null)
                out.push({name: m[1] || m[2] || m[3], says: said.replace(/\s*\{$/, "").replace(/\s+/g, " ")})
            words = ""
        }
        for (let i = 0; i < text.length; i++) {
            const c = text[i]
            if (c === "/" && text[i + 1] === "/") {
                while (i < text.length && text[i] !== "\n")
                    i++
                if (depth === 1)
                    end()
                continue
            }
            if (c === "/" && text[i + 1] === "*") {
                const close = text.indexOf("*/", i + 2)
                i = close < 0 ? text.length : close + 1
                continue
            }
            if (c === '"' || c === "'" || c === "`") {
                let j = i + 1
                while (j < text.length && text[j] !== c)
                    j += text[j] === "\\" ? 2 : 1
                if (depth === 1)
                    words += text.slice(i, j + 1)
                i = j
                continue
            }
            if (c === "{") {
                if (depth === 1)
                    end()
                depth++
            } else if (c === "}") {
                depth--
                words = ""
            } else if (depth === 1) {
                if (c === "\n" || c === ";")
                    end()
                else
                    words += c
            }
        }
        return out
    }
    // The kinds a member is added as, in the order the row offers them.
    readonly property var memberKinds: ["Property", "Signal", "Function"]
    property bool addingMember: false
    // One more member of the selected object, as its row says it; the row
    // is emptied once the file has it, and kept for a fix when it refuses.
    function addMember() {
        const name = memberName.text.trim()
        if (name === "" || workspace.selected === "")
            return
        let text
        if (memberKind.currentIndex === 0) {
            const value = memberValue.text.trim()
            text = "property " + (memberType.text.trim() || "var") + " " + name + (value !== "" ? ": " + value : "")
        } else if (memberKind.currentIndex === 1) {
            const args = memberArgs.text.trim()
            text = "signal " + name + (args !== "" ? "(" + args + ")" : "")
        } else {
            const body = memberBody.text.trim()
            text = "function " + name + "(" + memberArgs.text.trim() + ") {" + (body !== "" ? "\n" + body + "\n" : "") + "}"
        }
        refusal.at = "api"
        refusal.text = ""
        addingMember = true
        ask("node_insert", {doc: workspace.doc, parent: {node: workspace.selected}, text: text})
    }
    function memberAdded() {
        addingMember = false
        memberName.text = ""
        memberType.text = ""
        memberArgs.text = ""
        memberValue.text = ""
        memberBody.text = ""
    }

    // The signal whose handler is open over several lines, or empty.
    property string handlerOf: ""
    function handlerName(signal) {
        return "on" + signal[0].toUpperCase() + signal.slice(1)
    }
    // A handler's statements as a person writes them: its braces taken off
    // and its lines brought back to the left edge together.
    function handlerBody(signal) {
        let said = written(handlerName(signal)).trim()
        if (!/^\{[\s\S]*\}$/.test(said))
            return said
        let lines = said.slice(1, -1).split("\n")
        while (lines.length > 0 && lines[0].trim() === "")
            lines.shift()
        while (lines.length > 0 && lines[lines.length - 1].trim() === "")
            lines.pop()
        const indents = lines.filter(l => l.trim() !== "").map(l => l.match(/^\s*/)[0].length)
        const cut = indents.length > 0 ? Math.min(...indents) : 0
        return lines.map(l => l.slice(cut).replace(/\s+$/, "")).join("\n").trim()
    }
    function writeHandler(signal, text) {
        const lines = text.replace(/\s+$/, "").split("\n").map(l => l.replace(/\s+$/, ""))
        while (lines.length > 0 && lines[0].trim() === "")
            lines.shift()
        const name = handlerName(signal)
        refusal.at = "handle " + signal
        refusal.text = ""
        let value
        if (lines.length === 0) {
            // Emptied, the handler goes; one never written stays unwritten.
            if (written(name) === "") {
                handlerOf = ""
                return
            }
            value = null
        } else {
            value = lines.length === 1 ? lines[0].trim()
                : "{\n" + lines.map(l => l === "" ? "" : "    " + l).join("\n") + "\n}"
        }
        ask("binding_set", {doc: workspace.doc, node: {node: workspace.selected}, name: name, value: value})
    }
    // Neither the root nor what a layout places is pinned.
    readonly property bool pinnable: placedBy === "" && !isRoot
    readonly property var marginNames: ["anchors.margins", "anchors.topMargin", "anchors.leftMargin",
        "anchors.rightMargin", "anchors.bottomMargin"]
    // What a row, column or grid layout places is told where it sits in its
    // place there, on the axes that layout leaves room on.
    readonly property var alignments: {
        const across = [
            {flag: "Qt.AlignLeft", word: "Left", icon: "alignLeft"},
            {flag: "Qt.AlignHCenter", word: "Centre", icon: "alignHCenter"},
            {flag: "Qt.AlignRight", word: "Right", icon: "alignRight"}
        ]
        const down = [
            {flag: "Qt.AlignTop", word: "Top", icon: "alignTop"},
            {flag: "Qt.AlignVCenter", word: "Middle", icon: "alignVCenter"},
            {flag: "Qt.AlignBottom", word: "Bottom", icon: "alignBottom"}
        ]
        const tell = (list, axis, where) => list.map(a => ({
            flag: a.flag, icon: a.icon, axis: axis, label: a.word + " of " + where
        }))
        if (placedBy === "RowLayout")
            return tell(down, "down", "the row")
        if (placedBy === "ColumnLayout")
            return tell(across, "across", "the column")
        if (placedBy === "GridLayout")
            return tell(across, "across", "its cell").concat(tell(down, "down", "its cell"))
        return []
    }
    // The flags `Layout.alignment` is written with, or null for an expression
    // that is more than flags.
    function alignmentFlags() {
        const text = inState("Layout.alignment", written("Layout.alignment")).trim()
        if (text === "")
            return []
        if (!/^Qt\.Align[A-Za-z]+(\s*\|\s*Qt\.Align[A-Za-z]+)*$/.test(text))
            return null
        return text.split("|").map(f => f.trim())
    }
    // One axis's flag in place of whatever that axis had, or none when it was
    // the one written; the other axis keeps its own.
    function align(flag, axis) {
        const sameAxis = alignments.filter(a => a.axis === axis).map(a => a.flag)
        const had = alignmentFlags() || []
        const kept = had.filter(f => sameAxis.indexOf(f) < 0)
        if (had.indexOf(flag) < 0)
            kept.push(flag)
        refusal.at = "align"
        set("Layout.alignment", kept.length > 0 ? kept.join(" | ") : "null")
    }
    // The newest boxes the canvas asked for: a new pin keeps the distance the
    // object has, where one is known.
    property var seenBox: null
    property var seenParent: null
    function margin(side) {
        const box = seenBox
        const outer = seenParent
        // Unmeasured, it is where the margin it shares puts it.
        if (box === null || outer === null || box.window === undefined) {
            const shared = written("anchors.margins")
            return isNumber(shared) ? Number(shared) : 0
        }
        const at = box.window
        const away = side === "left" ? at.x - outer.x
            : side === "top" ? at.y - outer.y
            : side === "right" ? outer.x + outer.width - at.x - at.width
            : outer.y + outer.height - at.y - at.height
        return Math.round(away)
    }
    // Where the running app has it, for what the file does not say: only what
    // is on the canvas has a place, and only where no layout or anchor says it.
    readonly property var placeRows: {
        const box = seenBox
        if (box === null || box.local === undefined || box.scene !== undefined || placedBy !== "" || isRoot)
            return []
        const pinned = workspace.inspector.some(r => r.name.startsWith("anchors."))
        return pinned ? [] : [
            {name: "x", at: box.local.x},
            {name: "y", at: box.local.y},
            {name: "width", at: box.local.width},
            {name: "height", at: box.local.height}
        ].filter(p => !workspace.inspector.some(r => r.name === p.name))
    }
    // Where and how big, apart from the other properties: the four numbers
    // the file writes or, where it does not, the running app's, in a grid.
    readonly property var geometryNames: ["x", "y", "width", "height"]
    readonly property var geometry: {
        const out = []
        const rows = workspace.inspector
        const measured = placeRows
        for (let i = 0; i < geometryNames.length; i++) {
            const name = geometryNames[i]
            const row = rows.find(r => r.name === name)
            if (row !== undefined) {
                out.push({name: name, row: row, at: null})
            } else {
                const seen = measured.find(m => m.name === name)
                if (seen !== undefined)
                    out.push({name: name, row: null, at: seen.at})
            }
        }
        return out
    }
    // What a value is at a glance: a literal, a design role (a token), or an
    // expression the runtime keeps up to date (a binding). An enumeration a
    // property offers reads as the literal it is.
    function valueKind(name, text) {
        const t = text.trim()
        // An id is a name, neither a value nor an expression.
        if (name === "id" || t === "" || workspace.literal(t) || choices(name).indexOf(t) >= 0)
            return "literal"
        if (/^(Colors|Typography|Spacing|Motion|Radius|Elevation)\.[A-Za-z]+$/.test(t))
            return "token"
        return "binding"
    }
    function isNumber(text) {
        return /^-?\d+(\.\d+)?$/.test(String(text).trim())
    }
    function unitOf(name, text) {
        return isNumber(text) && /^(x|y|z|width|height|radius|spacing|padding|font\.pixelSize|.*Padding|.*[mM]argin|.*[mM]argins|.*Width|.*Height|.*Spacing)$/.test(name)
            ? "px" : ""
    }
    // A colour role's paint as the document's theme has it, in the look shown.
    function tokenPaint(text) {
        const t = text.trim()
        if (!/^Colors\.[A-Za-z]+$/.test(t))
            return ""
        const found = workspace.tokens.find(r => r.token === t)
        if (found === undefined)
            return ""
        return Colors.dark ? found.dark : found.light
    }

    function togglePin(side) {
        const args = {
            action: "anchor",
            doc: workspace.doc,
            nodes: workspace.chosen.map(h => ({node: h}))
        }
        if (side === "centerIn") {
            args.centerIn = !isPinned(side)
        } else if (side === "fill") {
            args.fill = !isPinned(side)
        } else if (side === "horizontalCenter" || side === "verticalCenter") {
            // A centre line's value is its offset from the parent's.
            args.sides = {}
            args.sides[side] = isPinned(side) ? null : 0
        } else {
            args.sides = {}
            args.sides[side] = isPinned(side) ? null : margin(side)
        }
        refusal.at = "pin"
        ask("node_arrange", args)
    }

    function arrange(args) {
        refusal.at = "arrange"
        refusal.text = ""
        ask("node_arrange", args)
    }

    function offer(name) {
        const found = workspace.offers.find(o => o.name === name)
        return found !== undefined ? found : null
    }
    function choices(name) {
        const found = offer(name)
        return found !== null && found.values !== undefined ? found.values : []
    }
    // The properties the name typed so far could be, not those already
    // written: the first few, since this is a nudge and not a reference.
    function namesOffered(typed, focused) {
        const word = typed.toLowerCase()
        if (word === "" && !focused)
            return []
        if (offer(typed) !== null)
            return []
        const rows = workspace.inspector
        return workspace.offers
            .filter(o => o.name.toLowerCase().indexOf(word) >= 0
                         && !rows.some(r => r.name === o.name))
            .slice(0, 6)
    }

    signal reseed()
    // What was begun on the object selected before: a write reads the selection
    // when it is sent, and would land on this one.
    signal selectionMoved()
    onSelectionMoved: {
        newName.text = ""
        newValue.text = ""
        componentName.text = ""
        newComment.text = ""
        memberAdded()
        handlerOf = ""
    }
    Connections {
        target: panel.workspace
        function onSelectedChanged() {
            panel.reseed()
            panel.selectionMoved()
            // The selection is shared, and what is open was opened on another.
            picker.close()
            choiceList.close()
            // An object removed under an open picker empties the selection,
            // and what the picker says of that is still to be read. Any other
            // refusal was about what is gone.
            if (panel.workspace.selected !== "" || refusal.text !== panel.pickerGone)
                panel.forgetRefused()
            else
                panel.offered = null
            // Those boxes were another object's, and a pin or a place offered
            // from them would be wrong for this one.
            panel.seenBox = null
            panel.seenParent = null
        }
        function onRefused(tool, error, asked) {
            if (tool === "node_arrange" && asked.action === "relayout")
                panel.filling = false
            if (tool === "node_insert")
                panel.addingMember = false
            if (["property_set", "binding_set", "state_edit", "node_arrange"].includes(tool))
                panel.reseed()
            if (refusal.asking !== "" && refusal.asking === panel.asked(tool, asked)) {
                refusal.asking = ""
                refusal.text = error.message
            }
            if (tool === "state_edit" && panel.renamedPicked !== "") {
                panel.pick(panel.renamedPicked, panel.pickedOn, true)
                panel.renamedPicked = ""
            }
        }
        function onAnswered(tool, answer, asked) {
            if (refusal.asking !== "" && refusal.asking === panel.asked(tool, asked))
                refusal.asking = ""
            if (tool === "node_arrange" && answer.boxes !== undefined) {
                panel.seenBox = answer.boxes.length === 1 ? answer.boxes[0] : null
                panel.seenParent = answer.parent !== undefined ? answer.parent : null
            }
            if (tool === "state_edit")
                panel.renamedPicked = ""
            if (tool === "node_insert" && panel.addingMember)
                panel.memberAdded()
            if (tool === "binding_set" && panel.handlerOf !== "" && asked.name === panel.handlerName(panel.handlerOf))
                panel.handlerOf = ""
            if (tool === "node_arrange" && asked.action === "relayout" && panel.filling)
                panel.filled(answer.nodes !== undefined && answer.nodes.length === 1 ? answer.nodes[0] : "")
        }
    }
    // The picked state's old name while its rename is unanswered: the pick
    // goes back to it if the rename is refused.
    property string renamedPicked: ""

    // What a row offers of colour: all of the swatch, the picker and its note
    // follow this, and features/ui/inspector.feature's table says what each
    // kind of row gets. The row's type or name says it is meant to be a colour,
    // which the runtime cannot: "red" is a fine colour and a fine label. So
    // text only earns a picker by itself when it is `#`, a written colour by
    // any reading.
    function colourRow(row, written) {
        const text = writtenText(written)
        const colour = text !== null && writesColour(text) ? text : ""
        return {
            picker: row.colour === true || /(^|\.)[a-zA-Z]*[cC]olor$/.test(row.name) || colour.charAt(0) === "#",
            paint: colour,
            note: written !== "" && !workspace.literal(written)
        }
    }
    // The text a value spells out, in any quotes, or null when it spells out
    // none. The parser says it is a literal; its first character says which.
    function writtenText(value) {
        return workspace.literal(value) && /^["'`]/.test(value) ? value.slice(1, -1) : null
    }
    // The runtime's own reading: what it does not parse would not paint.
    function writesColour(text) {
        try {
            Qt.color(text)
            return true
        } catch (e) {
            return false
        }
    }

    function enter(row, text) {
        // An id is no value: what names it elsewhere has to follow it.
        refusal.at = row.name
        if (row.name === "id") {
            refusal.text = ""
            if (text.trim() !== row.value) {
                ask("node_arrange", {
                    action: "rename",
                    doc: workspace.doc,
                    nodes: [{node: workspace.selected}],
                    name: text.trim()
                })
            }
            return
        }
        heldText = (row.value === "" && row.notText !== true) || String(row.value).startsWith('"')
        set(row.name, text)
        heldText = false
    }
    // The row being entered held a string or nothing yet, so a word typed in it
    // may be meant as one.
    property bool heldText: false

    // A refused value the refusal offers to bind, while that refusal stands.
    property var offered: null
    property string offeredSays: ""

    // On the selected object unless a picker says which it was opened on.
    function set(name, text, node) {
        if (picked !== "") {
            record(name, text, node)
            return
        }
        // The parser's word on what QML reads as a literal, and written as typed:
        // `0x20` stays `0x20`. Only a colour starts with `#`: no expression
        // does. A colour's name in a colour row is that colour, not a name
        // the file could mean, so neither needs asking.
        const trimmed = text.trim()
        const colour = /^#[0-9a-fA-F]+$/.test(trimmed)
            || (isColourRow(name) && /^[A-Za-z]+$/.test(trimmed) && writesColour(trimmed))
        const typed = colour ? '"' + trimmed + '"' : trimmed
        // One of the property's own values is written as it is too.
        const literal = workspace.literal(typed) || choices(name).indexOf(typed) >= 0
        // Two guesses are left, and both are asked about, not written. A bare
        // word that names nothing the file has is text, or a name mistyped.
        // Figures run together where text may stand are how QML writes a date,
        // point, size, rect or vector as text (`2020-01-02`, `1,2`, `1x2`):
        // as an expression they are another value, or none.
        const unnamed = /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(typed) && !names(typed)
        const figures = heldText && /^[+-]?\d+(\.\d+)?((, *|[-:xT])[+-]?\d+(\.\d+)?)+Z?$/.test(typed)
        if (!literal && (unnamed || figures)) {
            // The field goes back to what the file says; what was typed is
            // kept for the offers under the refusal, which is what they write.
            offered = {name: name, value: typed, asText: heldText}
            refusal.text = typed + " is not a literal: bind it as an expression"
            offeredSays = refusal.text
            reseed()
            return
        }
        refusal.text = ""
        ask("binding_set", {
            doc: workspace.doc,
            node: {node: node || workspace.selected},
            name: name,
            value: typed === "null" ? null : typed
        })
    }
    // Whether a bare word names something an expression here could read: an
    // object's id, the parent, a property of the selected object, or one of
    // the language's own.
    function names(word) {
        if (["parent", "undefined", "NaN", "Infinity"].indexOf(word) >= 0)
            return true
        if (workspace.layers.some(l => l.id === word))
            return true
        return workspace.inspector.some(r => r.name === word) || offer(word) !== null
    }
    function isColourRow(name) {
        const row = workspace.inspector.find(r => r.name === name)
        return (row !== undefined && row.colour === true) || /(^|\.)[a-zA-Z]*[cC]olor$/.test(name)
    }
    // The colours a value paints, at most two: what it writes, or the roles
    // and colours an expression names, as the theme has them, for a person
    // to see what it picks between. What only the running app can work out
    // (`Qt.rgba(width, 0, 0, 1)`) paints nothing here.
    function paints(written) {
        const t = written.trim()
        const own = writtenText(t)
        if (own !== null)
            return own !== "" && writesColour(own) ? [own] : []
        if (t === "" || workspace.literal(t))
            return []
        const out = []
        const add = c => {
            if (c !== "" && out.indexOf(c) < 0)
                out.push(c)
        }
        const parts = /Colors\.[A-Za-z]+|"([^"\\]*)"|'([^'\\]*)'|Qt\.(rgba|hsla|hsva)\(([^()]*)\)/g
        let m
        while ((m = parts.exec(t)) !== null) {
            if (m[0].startsWith("Colors.")) {
                add(tokenPaint(m[0]))
            } else if (m[3] !== undefined) {
                const n = m[4].split(",").map(a => a.trim())
                if ((n.length === 3 || n.length === 4) && n.every(a => isNumber(a))) {
                    const v = n.map(Number)
                    add(String(Qt[m[3]](v[0], v[1], v[2], v.length === 4 ? v[3] : 1)))
                }
            } else {
                const quoted = m[1] !== undefined ? m[1] : m[2]
                if (quoted !== "" && writesColour(quoted))
                    add(quoted)
            }
        }
        return out.slice(0, 2)
    }

    // The state picked to edit; empty for none. It belongs to the object
    // selected when it was picked, and stays picked as the selection moves.
    property string picked: ""
    property string pickedOn: ""
    // For what else sets values: the canvas records into the same state.
    readonly property var recording: picked === "" ? null : {state: picked, on: {node: pickedOn}}
    readonly property var current: {
        const all = workspace.selected === pickedOn ? workspace.states : workspace.rootStates
        for (let i = 0; i < all.length; i++) {
            if (all[i].name === picked)
                return all[i]
        }
        return null
    }
    readonly property string selectedId: {
        const found = layer(workspace.selected)
        return found !== null ? found.id : ""
    }
    // Another document has its own states, and its own preview.
    readonly property string pickedDoc: workspace.doc
    onPickedDocChanged: {
        picked = ""
        pickedOn = ""
        forgetRefused()
    }
    readonly property string pickerGone: "The object this picker was opened on has gone"
    // A refused value was typed on one object, in one document, into one state;
    // its offer would write it to whatever is there now.
    function forgetRefused() {
        refusal.text = ""
        offered = null
    }

    function layer(node) {
        const all = workspace.layers
        for (let i = 0; i < all.length; i++) {
            if (all[i].node === node)
                return all[i]
        }
        return null
    }

    // The frame shows what is being edited, by the object's id, which is how
    // a running preview is read. A state with a `when` is the app's to turn
    // on: forcing it would have a frame claim what its width does not do.
    // With `follows` this comes after an edit, whose refusal it leaves alone;
    // without it, it is the person's own gesture.
    function pick(name, on, follows) {
        if (!follows)
            forgetRefused()
        const was = pickedOn
        const had = picked !== ""
        picked = name
        pickedOn = on
        const conditioned = workspace.states.some(s => s.name === name && s.when !== undefined)
        if (name !== "" && !conditioned)
            show(on, name, follows)
        else if (had)
            show(was, "", follows)
    }
    // Starts recording into the root's state for a width, making it when it is
    // not there. A breakpoint compares the root by its id, so a root drawn
    // without one is named first.
    function editLayout(target, state) {
        const root = workspace.layers.length > 0 ? workspace.layers[0] : null
        if (root === null)
            return
        if (state !== "") {
            pick(state, root.node)
            return
        }
        forgetRefused()
        if (root.id === "") {
            let name = "root"
            const taken = workspace.layers.map(l => l.id)
            for (let n = 2; taken.indexOf(name) >= 0; n++)
                name = "root" + n
            workspace.dispatch("node_arrange", {
                action: "rename",
                doc: workspace.doc,
                nodes: [{node: root.node}],
                name: name
            })
        }
        picked = target
        pickedOn = root.node
        editState({
            action: workspace.rootStates.some(s => s.name === target) ? "when" : "add",
            state: target,
            breakpoint: target
        })
    }
    function show(on, state, follows) {
        const object = layer(on)
        if (!workspace.previews || object === null || object.id === "")
            return
        if (follows)
            workspace.follow("preview_state", {node: {node: on}, show: state})
        else
            workspace.dispatch("preview_state", {node: {node: on}, show: state})
    }

    // What a state changes is written with the object's id: there is no other
    // way for a `PropertyChanges` to name it.
    function record(name, text, node) {
        const object = layer(node || workspace.selected)
        if (object === null || object.id === "") {
            refusal.text = "A state names the object it changes: give this one an id"
            return
        }
        change(object.id, name, text)
    }

    // The value the picked state gives the selected object's property.
    function inState(name, base) {
        if (current === null)
            return base
        const changes = current.changes
        for (let i = 0; i < changes.length; i++) {
            if (changes[i].target === selectedId && changes[i].name === name)
                return changes[i].value
        }
        return base
    }

    function editState(args) {
        refusal.text = ""
        args.doc = workspace.doc
        args.node = {node: pickedOn !== "" && args.state === picked ? pickedOn : workspace.selected}
        ask("state_edit", args)
    }

    // A change is QML as written after the `:`, so what a person types is
    // what the file says; an empty one takes the change out.
    function change(target, name, text) {
        const args = {action: "change", state: picked, target: {id: target}, name: name}
        if (text !== "")
            args.binding = text
        editState(args)
    }

    // The selected object's comments, then those whose objects are gone.
    readonly property var comments: {
        const all = workspace.comments
        const out = []
        for (let i = 0; i < all.length; i++) {
            if (all[i].orphaned || all[i].node === workspace.selected)
                out.push(all[i])
        }
        return out
    }

    // The box keeps its text until the sidecar took the comment, so a
    // refused one can be sent again rather than typed again.
    property bool adding: false
    Connections {
        target: panel.workspace
        function onAnswered(tool, answer, asked) {
            if (tool === "review" && panel.adding) {
                panel.adding = false
                newComment.text = ""
            }
            if (tool === "preview_state" && picker.asks(asked)) {
                const value = answer.state[picker.name]
                const read = value !== undefined ? String(value) : null
                // The running app has no value for it yet, so any is a start.
                if (value === null) {
                    picker.start("#000000")
                } else if (read !== null && panel.writesColour(read)) {
                    picker.start(read)
                } else {
                    picker.unread()
                    if (read !== null)
                        refusal.text = read + " is not a colour"
                }
            }
            if (panel.changing && tool === "state_edit") {
                panel.changing = false
                changeTarget.text = ""
                changeName.text = ""
                changeValue.text = ""
            }
            if (panel.rowEditing && ["property_set", "binding_set", "state_edit"].includes(tool)) {
                panel.rowEditing = false
                newName.text = ""
                newValue.text = ""
            }
        }
        function onRefused(tool, error, asked) {
            if (tool === "review")
                panel.adding = false
            // The reason is on the window's error line; the sliders go on from
            // grey, since picking replaces what could not be read.
            if (tool === "preview_state" && picker.asks(asked))
                picker.unread()
            panel.rowEditing = false
            panel.changing = false
        }
    }

    function review(args) {
        refusal.at = "comment"
        refusal.text = ""
        workspace.dispatch("review", args)
    }

    function bind(name, text) {
        if (picked !== "") {
            record(name, text)
            return
        }
        refusal.text = ""
        ask("binding_set", {
            doc: workspace.doc,
            node: {node: workspace.selected},
            name: name,
            value: text
        })
    }

    // The values of an enum, for the row that opened it.
    Popup {
        id: choiceList
        property string name: ""
        // The slot whose note says what the choice is refused for.
        property string slot: ""
        property var values: []
        width: 200
        padding: Spacing.xxs
        Accessible.role: Accessible.List
        Accessible.name: "Choices"

        function show(name, at, slot) {
            choiceList.name = name
            choiceList.slot = slot
            values = panel.choices(name)
            const place = at.mapToItem(panel, 0, at.height)
            x = Math.max(0, Math.min(place.x, panel.width - width))
            y = place.y
            open()
        }

        contentItem: ColumnLayout {
            spacing: 0

            Repeater {
                model: choiceList.values
                delegate: TextButton {
                    required property string modelData
                    Layout.fillWidth: true
                    text: modelData.split(".").pop()
                    Accessible.name: "Choose " + modelData + " for " + choiceList.name
                    onClicked: {
                        choiceList.close()
                        if (choiceList.slot === "new") {
                            panel.fromRow(() => panel.set(choiceList.name, modelData))
                        } else {
                            refusal.at = choiceList.slot
                            panel.set(choiceList.name, modelData)
                        }
                        newName.text = newName.text === choiceList.name ? "" : newName.text
                    }
                }
            }
        }
    }

    // Hue, saturation and value, or the hex a person has from elsewhere.
    // Set when the hand lets go or Enter is pressed: a drag is one undo step.
    // A colour an expression works out to is asked of the preview, so the
    // sliders start from what is on the canvas; when it cannot say, they go
    // on from a mid grey and an empty hex rather than stay off or start black.
    Popup {
        id: picker
        objectName: "colourPicker"
        property var row: ({})
        property string name: ""
        // The object it was opened on: the selection can move while it is open.
        property string node: ""
        // Showing a state or clicking in play reads the same object: only this
        // picker's own read ends its wait.
        function asks(call) {
            return waiting && call.read !== undefined && call.read.includes(name)
                && call.node.node === node
        }
        property string written: ""
        property bool waiting: false
        property bool known: false
        // What the sliders have no say over stays as it was written.
        property real alpha: 1
        readonly property color chosen: Qt.hsva(hue.value, saturation.value, brightness.value, alpha)

        function start(colour) {
            const start = Qt.color(colour)
            hue.value = Math.max(0, start.hsvHue)
            saturation.value = start.hsvSaturation
            brightness.value = start.hsvValue
            alpha = start.a
            hex.text = colour
            waiting = false
            known = true
        }
        // Nothing says what colour it is now. A mid grey moves every way, and
        // the empty hex says it is no reading of the file.
        function unread() {
            start("#808080")
            hex.text = ""
        }
        function show(row, written, at) {
            refusal.at = row.name
            refusal.text = ""
            picker.row = row
            picker.name = row.name
            picker.written = written
            picker.node = panel.workspace.selected
            // Beside the inspector, where it hides none of the rows.
            x = -implicitWidth - Spacing.xs
            y = at.mapToItem(panel, 0, 0).y
            const paint = panel.colourRow(row, written).paint
            if (paint !== "") {
                start(paint)
            } else {
                known = false
                hex.text = ""
                waiting = true
                panel.workspace.dispatch("preview_state", {
                    doc: panel.workspace.doc,
                    node: {node: picker.node},
                    read: [name]
                })
            }
            open()
        }
        function take(text) {
            written = JSON.stringify(text)
            if (panel.layer(node) === null) {
                refusal.text = panel.pickerGone
                return
            }
            panel.set(name, written, node)
        }

        onClosed: waiting = false
        padding: Spacing.xs
        ColumnLayout {
            spacing: Spacing.xxs
            Label {
                visible: panel.colourRow(picker.row, picker.written).note
                text: "Worked out by " + picker.written + ": picking replaces it"
                color: Colors.text
                font.pixelSize: Typography.captionSize
                Layout.maximumWidth: 200
                wrapMode: Text.Wrap
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 32
                color: picker.chosen
                border.color: Colors.border
            }
            Label {
                text: "Hue"
                color: Colors.text
                font.pixelSize: Typography.captionSize
            }
            Slider {
                id: hue
                enabled: picker.known
                Accessible.name: "Hue"
                onPressedChanged: if (!pressed) picker.take(picker.chosen.toString())
                onMoved: hex.text = picker.chosen.toString()
            }
            Label {
                text: "Saturation"
                color: Colors.text
                font.pixelSize: Typography.captionSize
            }
            Slider {
                id: saturation
                objectName: "pickerSaturation"
                enabled: picker.known
                Accessible.name: "Saturation"
                onPressedChanged: if (!pressed) picker.take(picker.chosen.toString())
                onMoved: hex.text = picker.chosen.toString()
            }
            Label {
                text: "Brightness"
                color: Colors.text
                font.pixelSize: Typography.captionSize
            }
            Slider {
                id: brightness
                objectName: "pickerBrightness"
                enabled: picker.known
                Accessible.name: "Brightness"
                onPressedChanged: if (!pressed) picker.take(picker.chosen.toString())
                onMoved: hex.text = picker.chosen.toString()
            }
            Field {
                id: hex
                Layout.fillWidth: true
                placeholderText: "#rrggbb"
                Accessible.name: "Colour"
                onAccepted: {
                    if (!panel.writesColour(text))
                        return
                    picker.take(text)
                    // The sliders go on from it, not from what it replaced.
                    picker.start(text)
                }
            }
        }
    }

    Flickable {
        id: scroll
        anchors.fill: parent
        clip: true
        contentHeight: rows.implicitHeight
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {
            Accessible.name: "Scroll the inspector"
        }

        ColumnLayout {
            id: rows
            width: scroll.width
            spacing: 0

            // Nothing chosen: what the panel is for, and how to give it something.
            ColumnLayout {
                visible: panel.workspace.selected === ""
                spacing: Spacing.xs
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: Spacing.xxl
                Layout.bottomMargin: Spacing.xl
                Rectangle {
                    implicitWidth: 40
                    implicitHeight: 40
                    radius: 12
                    color: Colors.surfaceHover
                    Layout.alignment: Qt.AlignHCenter
                    Icon {
                        anchors.centerIn: parent
                        name: "pointer"
                        size: 20
                        color: Colors.textMuted
                    }
                }
                Label {
                    text: "Nothing selected"
                    color: Colors.text
                    font.pixelSize: Typography.subtitleSize
                    font.weight: Typography.subtitleWeight
                    Layout.alignment: Qt.AlignHCenter
                }
                Label {
                    text: "Select something to edit it"
                    color: Colors.textMuted
                    font.pixelSize: Typography.bodySize
                    Layout.alignment: Qt.AlignHCenter
                }
                // What was refused of what went away: nothing here to aim at moves.
                Label {
                    visible: refusal.text !== ""
                    text: refusal.text
                    color: Colors.danger
                    font.pixelSize: Typography.captionSize
                    wrapMode: Text.Wrap
                    horizontalAlignment: Text.AlignHCenter
                    Layout.maximumWidth: 240
                    Layout.alignment: Qt.AlignHCenter
                    Accessible.role: Accessible.StaticText
                    Accessible.name: text
                }
            }

            Label {
                visible: panel.workspace.chosen.length > 1
                text: visible ? panel.workspace.chosen.length + " selected. This edits the first." : ""
                color: Colors.textMuted
                font.pixelSize: Typography.captionSize
                wrapMode: Text.WordWrap
                padding: Spacing.xs
                Layout.fillWidth: true
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            // Each lies over the next: a refusal hangs from a section's last gap.
            Section {
                z: 8
                tail: Spacing.xxs
                title: "Layout"
                gap: 0
                visible: panel.workspace.selected !== ""
                Layout.fillWidth: true

                Label {
                    visible: panel.placedBy !== ""
                    text: "It is placed by its " + panel.placedBy
                    Layout.bottomMargin: Spacing.xs
                    color: Colors.textMuted
                    font.pixelSize: Typography.captionSize
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                RowLayout {
                    visible: panel.alignments.length > 0
                    spacing: 2
                    Layout.fillWidth: true
                    Repeater {
                        model: panel.alignments
                        delegate: IconButton {
                            required property var modelData
                            required property int index
                            icon_: modelData.icon
                            text: modelData.label
                            iconSize: 14
                            checked: (panel.alignmentFlags() || []).indexOf(modelData.flag) >= 0
                            // A gap between a grid's two axes.
                            Layout.leftMargin: index === 3 ? Spacing.sm : 0
                            onClicked: panel.align(modelData.flag, modelData.axis)
                        }
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                }
                Note {
                    visible: panel.alignments.length > 0
                    text: panel.says("align")
                    Layout.fillWidth: true
                }

                // The parent as a box and the object as a box inside it: a strut
                // from an inner edge to the outer one is a pin on that side, with
                // its margin beside it. Where a button sits is which side it pins.
                RowLayout {
                    visible: panel.pinnable
                    spacing: Spacing.xs
                    Layout.fillWidth: true

                    Rectangle {
                        id: pad
                        readonly property bool filled: panel.isPinned("fill")
                        readonly property bool centred: panel.isPinned("horizontalCenter") || panel.isPinned("verticalCenter")
                        implicitHeight: 88
                        Layout.preferredWidth: 148
                        // Less, and a centre line's button meets the bottom pin's.
                        Layout.minimumWidth: 140
                        radius: 8
                        color: Colors.surfaceHover
                        border.width: 1
                        border.color: filled ? Colors.link : Colors.surfacePressed

                        Rectangle {
                            id: inner
                            anchors.centerIn: parent
                            width: 64
                            height: 28
                            radius: 6
                            color: Colors.surfaceRaised
                            border.width: 1
                            border.color: pad.filled || pad.centred ? Colors.link : Colors.surfacePressed
                        }

                        Repeater {
                            model: [
                                {side: "top", label: "Pin top", icon: "pinTop"},
                                {side: "left", label: "Pin left", icon: "pinLeft"},
                                {side: "right", label: "Pin right", icon: "pinRight"},
                                {side: "bottom", label: "Pin bottom", icon: "pinBottom"}
                            ]
                            delegate: Item {
                                id: strut
                                required property var modelData
                                readonly property bool across: modelData.side === "left" || modelData.side === "right"
                                readonly property bool on: panel.isPinned(modelData.side) || pad.filled
                                readonly property string gap: {
                                    const own = panel.written("anchors." + modelData.side + "Margin")
                                    return own !== "" ? own : panel.written("anchors.margins")
                                }
                                // The span between the inner edge and the outer one.
                                x: modelData.side === "left" ? 0
                                    : modelData.side === "right" ? inner.x + inner.width
                                    : inner.x
                                y: modelData.side === "top" ? 0
                                    : modelData.side === "bottom" ? inner.y + inner.height
                                    : inner.y
                                width: across ? inner.x : inner.width
                                height: across ? inner.height : inner.y

                                Rectangle {
                                    anchors.centerIn: parent
                                    width: strut.across ? parent.width : (strut.on ? 2 : 1)
                                    height: strut.across ? (strut.on ? 2 : 1) : parent.height
                                    color: strut.on ? Colors.link : Colors.surfacePressed
                                    Behavior on color {
                                        // Only as its pin is clicked: showing another object's pins is no
                                        // move, and would have a window nobody touches ask for frames.
                                        enabled: pinButton.hovered
                                        ColorAnimation {
                                            duration: Motion.enterSmallFrequent
                                            easing.type: Easing.BezierSpline
                                            easing.bezierCurve: Motion.easeOut
                                        }
                                    }
                                }
                                IconButton {
                                    id: pinButton
                                    anchors.centerIn: parent
                                    icon_: strut.modelData.icon
                                    text: strut.modelData.label
                                    iconSize: 14
                                    checked: panel.isPinned(strut.modelData.side)
                                    onClicked: panel.togglePin(strut.modelData.side)
                                }
                                Text {
                                    visible: strut.on && strut.gap !== ""
                                    text: strut.gap
                                    color: Colors.textMuted
                                    font.pixelSize: Typography.captionSize
                                    elide: Text.ElideRight
                                    width: Math.min(implicitWidth, strut.across ? strut.width : 28)
                                    // Clear of its button and of the corners' buttons.
                                    x: strut.across ? (parent.width - width) / 2 : parent.width / 2 + 16
                                    y: strut.across ? -height : (parent.height - height) / 2
                                }
                            }
                        }

                        IconButton {
                            anchors.centerIn: inner
                            icon_: "pinCentre"
                            text: "Pin centre"
                            iconSize: 14
                            checked: panel.isPinned("centerIn")
                            onClicked: panel.togglePin("centerIn")
                        }
                        IconButton {
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            icon_: "frame"
                            text: "Fill parent"
                            iconSize: 14
                            checked: pad.filled
                            onClicked: panel.togglePin("fill")
                        }
                        // One centre line each, in the corner across from the fill.
                        Row {
                            anchors.left: parent.left
                            anchors.bottom: parent.bottom
                            spacing: 0
                            IconButton {
                                icon_: "centreAcross"
                                text: "Pin horizontal centre"
                                iconSize: 14
                                checked: panel.isPinned("horizontalCenter")
                                onClicked: panel.togglePin("horizontalCenter")
                            }
                            IconButton {
                                icon_: "centreDown"
                                text: "Pin vertical centre"
                                iconSize: 14
                                checked: panel.isPinned("verticalCenter")
                                onClicked: panel.togglePin("verticalCenter")
                            }
                        }
                    }
                    // The margin every side shares, then each side's own: a side
                    // has one only while it is pinned.
                    GridLayout {
                        id: margins
                        columns: 2
                        columnSpacing: 2
                        rowSpacing: 2
                        Layout.fillWidth: true
                        readonly property string shared: panel.written("anchors.margins")
                        Repeater {
                            model: [
                                {name: "anchors.margins", label: "Margins", lead: "All", side: ""},
                                {name: "anchors.topMargin", label: "Top margin", lead: "T", side: "top"},
                                {name: "anchors.bottomMargin", label: "Bottom margin", lead: "B", side: "bottom"},
                                {name: "anchors.leftMargin", label: "Left margin", lead: "L", side: "left"},
                                {name: "anchors.rightMargin", label: "Right margin", lead: "R", side: "right"}
                            ]
                            delegate: Field {
                                id: marginField
                                required property var modelData
                                readonly property string written: panel.inState(modelData.name, panel.written(modelData.name))
                                lead: modelData.lead
                                scrub: true
                                enabled: modelData.side === "" || pad.filled || panel.isPinned(modelData.side)
                                bound: panel.valueKind(modelData.name, text) === "binding"
                                token: panel.valueKind(modelData.name, text) === "token"
                                text: written
                                // What the side has when it says nothing of its own.
                                placeholderText: modelData.side !== "" && enabled ? margins.shared : ""
                                Accessible.name: modelData.label
                                Layout.fillWidth: true
                                Layout.columnSpan: modelData.side === "" ? 2 : 1
                                Layout.preferredWidth: 1
                                Layout.minimumWidth: 32
                                onAccepted: {
                                    refusal.at = "margin"
                                    // Emptied, it says nothing of its own any more.
                                    if (text.trim() !== "")
                                        panel.set(modelData.name, text)
                                    else if (written !== "")
                                        panel.set(modelData.name, "null")
                                }
                                Connections {
                                    target: panel
                                    function onReseed() {
                                        marginField.text = Qt.binding(() => marginField.written)
                                    }
                                }
                            }
                        }
                    }
                }
                Note {
                    visible: panel.pinnable
                    text: panel.says("pin") || panel.says("margin")
                    Layout.fillWidth: true
                }

                // Two to a line, each line with the note its two share.
                Repeater {
                    model: {
                        const all = panel.geometry
                        const out = []
                        for (let i = 0; i < all.length; i += 2)
                            out.push(all.slice(i, i + 2))
                        return out
                    }
                    delegate: ColumnLayout {
                        id: pair
                        required property var modelData
                        readonly property string slot: pair.modelData.some(c => c.name === refusal.at) ? refusal.at : "-"
                        z: panel.says(slot) !== "" ? 1 : 0
                        spacing: 0
                        Layout.fillWidth: true
                        GridLayout {
                            columns: 2
                            columnSpacing: Spacing.xs
                            Layout.fillWidth: true
                            Repeater {
                                model: pair.modelData
                                delegate: RowLayout {
                                    id: cell
                                    required property var modelData
                                    readonly property string written: modelData.row !== null
                                        ? panel.inState(modelData.name, modelData.row.value) : ""
                                    spacing: 2
                                    Layout.fillWidth: true
                                    // Equal columns, whatever each holds.
                                    Layout.preferredWidth: 1
                                    Field {
                                        id: cellValue
                                        lead: ({x: "X", y: "Y", width: "W", height: "H"})[cell.modelData.name]
                                        unit: panel.unitOf(cell.modelData.name, text !== "" ? text : placeholderText)
                                        scrub: true
                                        bound: panel.valueKind(cell.modelData.name, text) === "binding"
                                        token: panel.valueKind(cell.modelData.name, text) === "token"
                                        text: cell.written
                                        placeholderText: cell.modelData.at !== null ? Math.round(cell.modelData.at) : ""
                                        Accessible.name: cell.modelData.name
                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 48
                                        onAccepted: {
                                            refusal.at = cell.modelData.name
                                            if (cell.modelData.row !== null)
                                                panel.enter(cell.modelData.row, text)
                                            else if (text.trim() !== "")
                                                panel.set(cell.modelData.name, text)
                                        }
                                        Connections {
                                            target: panel
                                            function onReseed() {
                                                cellValue.text = Qt.binding(() => cell.written)
                                            }
                                            function onSelectionMoved() {
                                                if (cell.modelData.row === null)
                                                    cellValue.text = ""
                                            }
                                        }
                                    }
                                    IconButton {
                                        icon_: "link"
                                        text: "Bind " + cell.modelData.name
                                        enabled: cell.modelData.row !== null || cellValue.text.trim() !== ""
                                        onClicked: {
                                            refusal.at = cell.modelData.name
                                            panel.bind(cell.modelData.name, cellValue.text)
                                        }
                                    }
                                }
                            }
                        }
                        Note {
                            text: panel.says(pair.slot)
                            bindSays: panel.offersBind(pair.slot)
                            textSays: panel.offersText(pair.slot)
                            onBindAsked: panel.takeOffer(false)
                            onTextAsked: panel.takeOffer(true)
                            Layout.fillWidth: true
                        }
                    }
                }

                RowLayout {
                    visible: panel.isLayout
                    spacing: Spacing.xxs
                    Layout.fillWidth: true
                    Repeater {
                        model: panel.isLayout ? ["spacing", "columns"].filter(n => n === "spacing" || panel.selectedType.startsWith("Grid")) : []
                        delegate: Field {
                            required property string modelData
                            lead: modelData === "spacing" ? "Gap" : "Cols"
                            unit: panel.unitOf(modelData, text)
                            scrub: true
                            bound: panel.valueKind(modelData, text) === "binding"
                            token: panel.valueKind(modelData, text) === "token"
                            text: panel.written(modelData)
                            Accessible.name: "Layout " + modelData
                            Layout.fillWidth: true
                            Layout.preferredWidth: 1
                            onAccepted: {
                                refusal.at = "layout"
                                refusal.text = ""
                                panel.ask("binding_set", {
                                    doc: panel.workspace.doc,
                                    node: {node: panel.workspace.selected},
                                    name: modelData,
                                    value: text === "" ? null : text
                                })
                            }
                        }
                    }
                }
                Note {
                    visible: panel.isLayout
                    text: panel.says("layout")
                    Layout.fillWidth: true
                }

                // Its family only: a Row's children are placed by their numbers,
                // a RowLayout's by what they say of `Layout`.
                Flow {
                    visible: panel.isLayout
                    spacing: Spacing.xxs
                    Layout.fillWidth: true
                    Layout.bottomMargin: Spacing.xs
                    Label {
                        text: "Turn into"
                        color: Colors.textMuted
                        font.pixelSize: Typography.captionSize
                        width: 64
                        height: 28
                        verticalAlignment: Text.AlignVCenter
                    }
                    Repeater {
                        model: {
                            const kinds = panel.selectedType.endsWith("Layout")
                                ? ["RowLayout", "ColumnLayout", "GridLayout"]
                                : ["Row", "Column", "Grid", "Flow"]
                            return kinds.filter(k => k !== panel.selectedType)
                        }
                        delegate: TextButton {
                            required property string modelData
                            text: modelData
                            Accessible.name: "Turn into " + modelData
                            onClicked: panel.arrange({
                                action: "relayout",
                                doc: panel.workspace.doc,
                                nodes: [{node: panel.workspace.selected}],
                                name: modelData
                            })
                        }
                    }
                }

                Row {
                    visible: !panel.isRoot
                    spacing: Spacing.xxs
                    Layout.bottomMargin: Spacing.xs
                    Label {
                        text: "Wrap in"
                        color: Colors.textMuted
                        font.pixelSize: Typography.captionSize
                        width: 64
                        height: 28
                        verticalAlignment: Text.AlignVCenter
                    }
                    Repeater {
                        model: [
                            {kind: "Row", icon: "row"},
                            {kind: "Column", icon: "column"},
                            {kind: "Grid", icon: "grid"},
                            {kind: "Item", icon: "frame"}
                        ]
                        delegate: IconButton {
                            required property var modelData
                            icon_: modelData.icon
                            text: "Wrap in " + modelData.kind
                            onClicked: panel.arrange({
                                action: "wrap",
                                doc: panel.workspace.doc,
                                nodes: panel.workspace.chosen.map(h => ({node: h})),
                                in: modelData.kind
                            })
                        }
                    }
                }

                RowLayout {
                    visible: !panel.isRoot && panel.workspace.chosen.length === 1
                    Layout.fillWidth: true
                    spacing: Spacing.xxs

                    Field {
                        id: componentName
                        placeholderText: "Component name"
                        Accessible.name: "Component name"
                        Layout.fillWidth: true
                        onAccepted: extract.clicked()
                    }
                    IconButton {
                        id: extract
                        icon_: "component"
                        text: "Make component"
                        enabled: componentName.text !== ""
                        onClicked: {
                            panel.arrange({
                                action: "component_extract",
                                doc: panel.workspace.doc,
                                nodes: [{node: panel.workspace.selected}],
                                name: componentName.text
                            })
                            componentName.text = ""
                        }
                    }
                }
                Note {
                    visible: !panel.isRoot
                    text: panel.says("arrange")
                    Layout.fillWidth: true
                }
            }

            // What the kind is about: a text's type, a picture's fit, a
            // scrolling area's content, a plain box's fill.
            Section {
                id: kind
                z: 7
                tail: Spacing.xxs
                visible: panel.kindSection !== null
                title: panel.kindSection !== null ? panel.kindSection.title : ""
                gap: 0
                Layout.fillWidth: true

                Choice {
                    id: style
                    visible: panel.kindSection !== null && panel.kindSection.styles === true
                    lead: "Style"
                    model: panel.textStyles.map(t => t.label)
                    currentIndex: panel.textStyle()
                    displayText: currentIndex >= 0 ? currentText : "Custom"
                    Accessible.name: "Text style"
                    Layout.fillWidth: true
                    Layout.bottomMargin: Spacing.xxs
                    onActivated: index => {
                        panel.giveStyle(panel.textStyles[index].key)
                        currentIndex = Qt.binding(() => panel.textStyle())
                    }
                }
                Repeater {
                    model: panel.kindSection !== null ? panel.kindSection.rows : []
                    delegate: RowLayout {
                        id: kindRow
                        required property var modelData
                        readonly property string written: panel.inState(modelData.name, panel.written(modelData.name))
                        spacing: 2
                        Layout.fillWidth: true
                        Field {
                            id: kindValue
                            lead: kindRow.modelData.lead
                            unit: panel.unitOf(kindRow.modelData.name, text)
                            scrub: panel.choices(kindRow.modelData.name).length === 0
                            bound: panel.valueKind(kindRow.modelData.name, text) === "binding"
                            token: panel.valueKind(kindRow.modelData.name, text) === "token"
                            text: kindRow.written
                            Accessible.name: kindRow.modelData.name
                            Layout.fillWidth: true
                            onAccepted: panel.put(kindRow.modelData.name, text)
                            Connections {
                                target: panel
                                function onReseed() {
                                    kindValue.text = Qt.binding(() => kindRow.written)
                                }
                            }
                        }
                        IconButton {
                            visible: panel.choices(kindRow.modelData.name).length > 0
                            icon_: "chevronDown"
                            text: "Choices for " + kindRow.modelData.name
                            onClicked: choiceList.show(kindRow.modelData.name, this, kindRow.modelData.name)
                        }
                        IconButton {
                            visible: kindRow.modelData.fits === true
                            icon_: "fit"
                            text: kindRow.modelData.name === "contentWidth"
                                ? "Fit the width to the content" : "Fit the height to the content"
                            onClicked: panel.fitContent(kindRow.modelData.name)
                        }
                    }
                }
                RowLayout {
                    visible: panel.kindSection !== null && panel.kindSection.aligns === true
                    spacing: 2
                    Layout.fillWidth: true
                    Repeater {
                        model: [
                            {flag: "Text.AlignLeft", label: "Align the text left", icon: "alignLeft"},
                            {flag: "Text.AlignHCenter", label: "Centre the text", icon: "alignHCenter"},
                            {flag: "Text.AlignRight", label: "Align the text right", icon: "alignRight"}
                        ]
                        delegate: IconButton {
                            required property var modelData
                            readonly property bool on: panel.inState("horizontalAlignment",
                                panel.written("horizontalAlignment")).trim() === modelData.flag
                            icon_: modelData.icon
                            text: modelData.label
                            iconSize: 14
                            checked: on
                            onClicked: {
                                refusal.at = "kind"
                                panel.set("horizontalAlignment", on ? "null" : modelData.flag)
                            }
                        }
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                }
                TextButton {
                    visible: panel.kindSection !== null && panel.kindSection.fill === true
                    text: "Give it a fill"
                    Accessible.name: text
                    onClicked: panel.giveFill()
                }
                Note {
                    text: panel.kindNames.concat(["kind"]).map(n => panel.says(n)).find(t => t !== "") || ""
                    bindSays: panel.kindNames.map(n => panel.offersBind(n)).find(t => t !== "") || ""
                    textSays: panel.kindNames.map(n => panel.offersText(n)).find(t => t !== "") || ""
                    onBindAsked: panel.takeOffer(false)
                    onTextAsked: panel.takeOffer(true)
                    Layout.fillWidth: true
                }
            }

            Section {
                z: 6
                tail: Spacing.xxs
                title: "Properties"
                gap: 0
                visible: panel.workspace.selected !== ""
                Layout.fillWidth: true

                Repeater {
                    // Where and how big, and a pinned object's margins, have
                    // their own fields above.
                    model: panel.workspace.inspector.filter(r => panel.geometryNames.indexOf(r.name) < 0
                        && panel.kindNames.indexOf(r.name) < 0
                        && !(panel.pinnable && panel.marginNames.indexOf(r.name) >= 0))
                    delegate: ColumnLayout {
                        id: row
                        required property var modelData
                        z: panel.says(row.modelData.name) !== "" ? 1 : 0
                        Layout.fillWidth: true
                        spacing: 0
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Spacing.xxs

                            Label {
                                text: row.modelData.name
                                color: Colors.textMuted
                                font.pixelSize: Typography.captionSize
                                Layout.preferredWidth: 72
                                elide: Text.ElideRight
                                // A number's name is its handle: drag across it to change it.
                                MouseArea {
                                    anchors.fill: parent
                                    enabled: panel.isNumber(value.text)
                                    cursorShape: enabled ? Qt.SizeHorCursor : Qt.ArrowCursor
                                    property real startX: 0
                                    onPressed: mouse => {
                                        startX = mapToItem(null, mouse.x, 0).x
                                        value.scrubStart()
                                    }
                                    onPositionChanged: mouse => value.scrubTo(mapToItem(null, mouse.x, 0).x - startX)
                                    onReleased: value.scrubEnd()
                                }
                            }
                            TextButton {
                                id: swatch
                                readonly property var kind: panel.colourRow(row.modelData, value.text)
                                // What it paints: written, or what an expression picks between,
                                // the second as the swatch's right half.
                                readonly property var paints: panel.paints(value.text)
                                readonly property string shown: paints.length > 0 ? paints[0] : ""
                                visible: kind.picker
                                Layout.preferredWidth: 28
                                Layout.minimumWidth: 28
                                Layout.preferredHeight: 28
                                padding: 0
                                leftPadding: 0
                                rightPadding: 0
                                topPadding: 0
                                bottomPadding: 0
                                Accessible.name: "Pick " + row.modelData.name
                                Accessible.description: paints.join(" or ")
                                onClicked: picker.show(row.modelData, value.text, this)
                                background: null
                                contentItem: Rectangle {
                                    radius: 6
                                    color: swatch.shown || "transparent"
                                    border.width: 1
                                    border.color: swatch.hovered ? Colors.border : Colors.surfacePressed
                                    Rectangle {
                                        visible: swatch.paints.length > 1
                                        x: parent.width / 2
                                        width: parent.width / 2
                                        height: parent.height
                                        radius: 6
                                        color: visible ? swatch.paints[1] : "transparent"
                                        border.width: 1
                                        border.color: parent.border.color
                                        // Square where the halves meet.
                                        Rectangle {
                                            width: 6
                                            height: parent.height - 2
                                            y: 1
                                            color: parent.color
                                        }
                                    }
                                    // No paint known: a slash, as an empty well.
                                    Rectangle {
                                        visible: swatch.shown === ""
                                        anchors.centerIn: parent
                                        width: parent.width * 1.2
                                        height: 1.5
                                        rotation: -45
                                        color: Colors.textMuted
                                    }
                                }
                            }
                            Field {
                                id: value
                                text: panel.inState(row.modelData.name, row.modelData.value)
                                placeholderText: "no value"
                                Accessible.name: row.modelData.name
                                bound: panel.valueKind(row.modelData.name, text) === "binding"
                                token: panel.valueKind(row.modelData.name, text) === "token"
                                // A colour role shows its hex; a size its unit.
                                unit: swatch.visible && token ? swatch.shown : panel.unitOf(row.modelData.name, text)
                                Layout.fillWidth: true
                                Layout.minimumWidth: 96
                                onAccepted: panel.enter(row.modelData, text)
                                // The rows are not made anew for a list that reads the
                                // same, and what is typed in one is not the next object's,
                                // nor stays when the file refused it.
                                Connections {
                                    target: panel
                                    function onReseed() {
                                        value.text = Qt.binding(() => panel.inState(row.modelData.name, row.modelData.value))
                                    }
                                }
                            }
                            // A face shown in itself.
                            Text {
                                readonly property var face: row.modelData.name === "font.family" ? panel.writtenText(value.text) : null
                                visible: face !== null && face !== ""
                                text: "Ag"
                                font.family: visible ? face : "sans-serif"
                                font.pixelSize: Typography.titleSize
                                color: Colors.text
                                Accessible.ignored: true
                            }
                            IconButton {
                                visible: panel.choices(row.modelData.name).length > 0
                                icon_: "chevronDown"
                                text: "Choices for " + row.modelData.name
                                onClicked: choiceList.show(row.modelData.name, this, row.modelData.name)
                            }
                            IconButton {
                                icon_: "link"
                                text: "Bind " + row.modelData.name
                                onClicked: {
                                    refusal.at = row.modelData.name
                                    panel.bind(row.modelData.name, value.text)
                                }
                            }
                            // What is written, taken away. A declaration's own value
                            // is the property, and the code's to take; an id is
                            // what others name the object by.
                            // Its place is kept where there is nothing to take, so
                            // the fields of every row end at one edge.
                            Item {
                                implicitWidth: remove.implicitWidth
                                implicitHeight: remove.implicitHeight
                                IconButton {
                                    id: remove
                                    anchors.fill: parent
                                    visible: row.modelData.value !== "" && row.modelData.declared !== true
                                        && row.modelData.name !== "id" && panel.picked === ""
                                    icon_: "close"
                                    text: "Remove " + row.modelData.name
                                    onClicked: {
                                        refusal.at = row.modelData.name
                                        refusal.text = ""
                                        panel.ask("binding_set", {
                                            doc: panel.workspace.doc,
                                            node: {node: panel.workspace.selected},
                                            name: row.modelData.name,
                                            value: null
                                        })
                                    }
                                }
                            }
                        }
                        Note {
                            text: panel.says(row.modelData.name)
                            bindSays: panel.offersBind(row.modelData.name)
                            textSays: panel.offersText(row.modelData.name)
                            onBindAsked: panel.takeOffer(false)
                            onTextAsked: panel.takeOffer(true)
                            Layout.fillWidth: true
                        }
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Spacing.xxs
                    visible: offeredRows.count > 0
                    Layout.bottomMargin: Spacing.xs

                    Repeater {
                        id: offeredRows
                        model: panel.namesOffered(newName.text, newName.activeFocus)
                        delegate: TextButton {
                            required property var modelData
                            text: modelData.name
                            Accessible.name: "Offer " + modelData.name
                            onClicked: {
                                newName.text = modelData.name
                                newValue.forceActiveFocus()
                            }
                        }
                    }
                }

                // Two lines: on one, the panel's width left the value a few pixels.
                RowLayout {
                    Layout.fillWidth: true
                    spacing: Spacing.xxs

                    Field {
                        id: newName
                        placeholderText: "property"
                        Accessible.name: "New property"
                        Layout.preferredWidth: 96
                    }
                    Field {
                        id: newValue
                        placeholderText: "value"
                        Accessible.name: "New value"
                        Layout.fillWidth: true
                        Layout.minimumWidth: 48
                        onAccepted: add.clicked()
                    }
                    IconButton {
                        visible: panel.choices(newName.text).length > 0
                        icon_: "chevronDown"
                        text: "Choices for new property"
                        onClicked: choiceList.show(newName.text, this, "new")
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: Spacing.xxs

                    Item {
                        Layout.fillWidth: true
                    }
                    TextButton {
                        id: add
                        text: "Set"
                        Accessible.name: "Set new property"
                        enabled: newName.text !== ""
                        onClicked: panel.fromRow(() => panel.set(newName.text, newValue.text))
                    }
                    TextButton {
                        text: "Bind"
                        Accessible.name: "Bind new property"
                        enabled: newName.text !== ""
                        onClicked: panel.fromRow(() => panel.bind(newName.text, newValue.text))
                    }
                }
                Note {
                    text: panel.says("new")
                    bindSays: panel.offersBind("new")
                    textSays: panel.offersText("new")
                    onBindAsked: panel.takeOffer(false)
                    onTextAsked: panel.takeOffer(true)
                    Layout.fillWidth: true
                }
            }

            Section {
                z: 5
                tail: Spacing.xxs
                title: "States"
                gap: 0
                visible: panel.workspace.selected !== ""
                Layout.fillWidth: true

                Repeater {
                    model: panel.workspace.states
                    delegate: ColumnLayout {
                        id: stateRow
                        required property var modelData
                        z: panel.says("state " + stateRow.modelData.name) !== "" ? 1 : 0
                        Layout.fillWidth: true
                        spacing: 0
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Spacing.xxs

                            TextButton {
                                text: stateRow.modelData.name
                                checkable: true
                                checked: panel.picked === stateRow.modelData.name
                                Accessible.name: "State " + stateRow.modelData.name
                                onClicked: panel.pick(checked ? stateRow.modelData.name : "", checked ? panel.workspace.selected : "")
                            }
                            Field {
                                id: stateName
                                text: stateRow.modelData.name
                                Accessible.name: "Name of " + stateRow.modelData.name
                                Layout.fillWidth: true
                                onAccepted: {
                                    const was = stateRow.modelData.name
                                    if (text === was || text === "")
                                        return
                                    refusal.at = "state " + was
                                    panel.editState({action: "rename", state: was, new_name: text})
                                    if (panel.picked === was) {
                                        panel.renamedPicked = was
                                        panel.pick(text, panel.pickedOn, true)
                                    }
                                }
                                Connections {
                                    target: panel
                                    function onReseed() {
                                        stateName.text = Qt.binding(() => stateRow.modelData.name)
                                    }
                                }
                            }
                            IconButton {
                                icon_: "trash"
                                text: "Remove state " + stateRow.modelData.name
                                onClicked: {
                                    const name = stateRow.modelData.name
                                    refusal.at = "state " + name
                                    panel.editState({action: "remove", state: name})
                                    if (panel.picked === name)
                                        panel.pick("", "", true)
                                }
                            }
                        }
                        Note {
                            text: panel.says("state " + stateRow.modelData.name)
                            Layout.fillWidth: true
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Spacing.xxs

                    Field {
                        id: newState
                        placeholderText: "state"
                        Accessible.name: "New state"
                        Layout.fillWidth: true
                        onAccepted: addState.clicked()
                    }
                    TextButton {
                        id: addState
                        text: "Add"
                        Accessible.name: "Add state"
                        enabled: newState.text !== ""
                        onClicked: {
                            refusal.at = "state"
                            panel.editState({action: "add", state: newState.text})
                            newState.text = ""
                        }
                    }
                }
                Note {
                    text: panel.says("state")
                    Layout.fillWidth: true
                }

                ColumnLayout {
                    visible: panel.current !== null
                    Layout.fillWidth: true
                    spacing: 0

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Spacing.xxs

                        Label {
                            text: "when"
                            color: Colors.textMuted
                            font.pixelSize: Typography.captionSize
                            Layout.preferredWidth: 72
                        }
                        Field {
                            id: stateWhen
                            text: panel.current !== null && panel.current.when !== undefined ? panel.current.when : ""
                            placeholderText: "always off"
                            Accessible.name: "When"
                            Layout.fillWidth: true
                            onAccepted: {
                                refusal.at = "when"
                                const args = {action: "when", state: panel.picked}
                                if (text !== "")
                                    args.when = text
                                panel.editState(args)
                                panel.show(panel.pickedOn, "", true)
                            }
                            Connections {
                                target: panel
                                function onReseed() {
                                    stateWhen.text = Qt.binding(() => panel.current !== null && panel.current.when !== undefined ? panel.current.when : "")
                                }
                            }
                        }
                    }
                    Note {
                        text: panel.says("when")
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Spacing.xxs

                        Label {
                            text: "at"
                            color: Colors.textMuted
                            font.pixelSize: Typography.captionSize
                        }
                        Repeater {
                            model: ["phone", "tablet", "desktop"]
                            delegate: TextButton {
                                required property string modelData
                                text: modelData
                                Accessible.name: "Breakpoint " + modelData
                                onClicked: {
                                    refusal.at = "at"
                                    panel.editState({action: "when", state: panel.picked, breakpoint: modelData})
                                    panel.show(panel.pickedOn, "", true)
                                }
                            }
                        }
                    }
                    Note {
                        text: panel.says("at")
                        Layout.fillWidth: true
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Spacing.xxs

                        Field {
                            id: changeTarget
                            placeholderText: "id"
                            Accessible.name: "Change target"
                            Layout.preferredWidth: 64
                        }
                        Field {
                            id: changeName
                            placeholderText: "property"
                            Accessible.name: "Change property"
                            Layout.preferredWidth: 64
                        }
                        Field {
                            id: changeValue
                            placeholderText: "value"
                            Accessible.name: "Change value"
                            Layout.fillWidth: true
                            onAccepted: setChange.clicked()
                        }
                        TextButton {
                            id: setChange
                            text: "Set"
                            Accessible.name: "Set change"
                            enabled: changeTarget.text !== "" && changeName.text !== ""
                            onClicked: {
                                refusal.at = "change"
                                panel.changing = true
                                panel.change(changeTarget.text, changeName.text, changeValue.text)
                                // Stopped before it was asked: there will be no answer.
                                if (refusal.text !== "")
                                    panel.changing = false
                            }
                        }
                    }
                    Note {
                        text: panel.says("change")
                        Layout.fillWidth: true
                    }

                    Repeater {
                        model: panel.current !== null ? panel.current.changes : []
                        delegate: ColumnLayout {
                            id: changeRow
                            required property var modelData
                            readonly property string name: modelData.target + "." + modelData.name
                            Layout.fillWidth: true
                            spacing: 0
                            z: panel.says("change " + changeRow.name) !== "" ? 1 : 0
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Spacing.xxs

                                Label {
                                    text: changeRow.name
                                    color: Colors.textMuted
                                    font.pixelSize: Typography.captionSize
                                    Layout.preferredWidth: 72
                                    elide: Text.ElideRight
                                }
                                Field {
                                    id: changeNow
                                    text: changeRow.modelData.value
                                    Accessible.name: changeRow.name
                                    Layout.fillWidth: true
                                    onAccepted: {
                                        refusal.at = "change " + changeRow.name
                                        panel.change(changeRow.modelData.target, changeRow.modelData.name, text)
                                    }
                                    Connections {
                                        target: panel
                                        function onReseed() {
                                            changeNow.text = Qt.binding(() => changeRow.modelData.value)
                                        }
                                    }
                                }
                            }
                            Note {
                                text: panel.says("change " + changeRow.name)
                                Layout.fillWidth: true
                            }
                        }
                    }
                }
            }

            Section {
                z: 4
                tail: Spacing.xxs
                title: "Signals"
                gap: 0
                Layout.fillWidth: true
                visible: panel.workspace.selected !== "" && signalList.count > 0

                // What a flow may handle on it, with the arguments a handler is
                // given; a property's change signal, which every property has, is
                // left out.
                Repeater {
                    id: signalList
                    model: panel.workspace.signals.filter(s => !s.change)
                    delegate: ColumnLayout {
                        id: signalRow
                        required property var modelData
                        readonly property string handler: "on" + modelData.name[0].toUpperCase() + modelData.name.slice(1)
                        z: panel.says("handle " + signalRow.modelData.name) !== "" ? 1 : 0
                        Layout.fillWidth: true
                        spacing: Spacing.xxs

                        readonly property bool opened: panel.handlerOf === modelData.name
                        RowLayout {
                            spacing: Spacing.xxs
                            Layout.fillWidth: true
                            Label {
                                text: signalRow.modelData.name + "(" + signalRow.modelData.arguments
                                    .map(a => a.type === "" ? a.name : a.name + ": " + a.type).join(", ") + ")"
                                color: Colors.textMuted
                                font.pixelSize: Typography.captionSize
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                                Accessible.role: Accessible.StaticText
                                Accessible.name: "Signal " + signalRow.modelData.name
                            }
                            // As high as the words beside it: the button reaches into
                            // the gaps over and under, and the field keeps its width.
                            Item {
                                implicitWidth: 24
                                implicitHeight: 14
                                IconButton {
                                    anchors.centerIn: parent
                                    implicitWidth: 24
                                    implicitHeight: 24
                                    iconSize: 14
                                    icon_: "code"
                                    checked: signalRow.opened
                                    text: "Edit the " + signalRow.modelData.name + " handler"
                                    onClicked: {
                                        if (signalRow.opened) {
                                            panel.handlerOf = ""
                                            return
                                        }
                                        panel.handlerOf = signalRow.modelData.name
                                        handlerLines.text = panel.handlerBody(signalRow.modelData.name)
                                        handlerLines.forceActiveFocus()
                                    }
                                }
                            }
                        }
                        // Over as many lines as it takes: Return is a new line,
                        // and the button (or Ctrl+Return) writes it.
                        TextArea {
                            id: handlerLines
                            visible: signalRow.opened
                            wrapMode: TextEdit.NoWrap
                            selectByMouse: true
                            color: Colors.text
                            selectionColor: Colors.surfaceSelected
                            selectedTextColor: Colors.text
                            placeholderText: "what it does, a line at a time"
                            placeholderTextColor: Colors.textMuted
                            font.family: "monospace"
                            font.pixelSize: Typography.captionSize
                            padding: Spacing.xs
                            Layout.fillWidth: true
                            Layout.minimumHeight: 72
                            Accessible.name: "Handler for " + signalRow.modelData.name
                            Keys.onPressed: event => {
                                if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
                                    && (event.modifiers & Qt.ControlModifier)) {
                                    panel.writeHandler(signalRow.modelData.name, text)
                                    event.accepted = true
                                } else if (event.key === Qt.Key_Escape) {
                                    panel.handlerOf = ""
                                    event.accepted = true
                                }
                            }
                            background: Rectangle {
                                radius: 6
                                color: handlerLines.activeFocus ? Colors.surfaceRaised : Colors.surfaceHover
                                border.width: handlerLines.activeFocus ? 2 : 1
                                border.color: handlerLines.activeFocus ? Colors.focusRing : Colors.border
                            }
                        }
                        RowLayout {
                            visible: signalRow.opened
                            spacing: Spacing.xxs
                            Layout.fillWidth: true
                            Item {
                                Layout.fillWidth: true
                            }
                            TextButton {
                                text: "Cancel"
                                Accessible.name: "Leave the " + signalRow.modelData.name + " handler as it was"
                                onClicked: panel.handlerOf = ""
                            }
                            TextButton {
                                text: "Write"
                                accent: true
                                Accessible.name: "Write the " + signalRow.modelData.name + " handler"
                                onClicked: panel.writeHandler(signalRow.modelData.name, handlerLines.text)
                            }
                        }
                        // Once written the handler is a row of Properties, like
                        // anything else the file says.
                        Field {
                            id: handlerText
                            visible: !signalRow.opened && !panel.workspace.inspector.some(r => r.name === signalRow.handler)
                            placeholderText: "what it does"
                            Accessible.name: "Handle " + signalRow.modelData.name
                            Layout.fillWidth: true
                            onAccepted: {
                                refusal.at = "handle " + signalRow.modelData.name
                                if (text.trim() !== "")
                                    panel.bind(signalRow.handler, text)
                                text = ""
                            }
                            Connections {
                                target: panel
                                function onSelectionMoved() { handlerText.text = "" }
                            }
                        }
                        Note {
                            text: panel.says("handle " + signalRow.modelData.name)
                            Layout.fillWidth: true
                        }
                    }
                }
            }

            Section {
                z: 3
                tail: Spacing.xxs
                title: "Roles"
                gap: 0
                Layout.fillWidth: true
                visible: panel.workspace.roles.length > 0

                Repeater {
                    model: panel.workspace.roles
                    delegate: ColumnLayout {
                        id: roleRow
                        required property string modelData
                        z: panel.says("role " + roleRow.modelData) !== "" ? 1 : 0
                        Layout.fillWidth: true
                        spacing: 0
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Spacing.xxs

                            Label {
                                text: roleRow.modelData
                                color: Colors.textMuted
                                font.pixelSize: Typography.captionSize
                                Layout.preferredWidth: 72
                                elide: Text.ElideRight
                                Accessible.role: Accessible.StaticText
                                Accessible.name: "Role " + roleRow.modelData
                            }
                            Field {
                                id: roleName
                                placeholderText: "property"
                                Accessible.name: "Bind role " + roleRow.modelData + " to"
                                Layout.fillWidth: true
                                onAccepted: {
                                    if (text === "")
                                        return
                                    refusal.at = "role " + roleRow.modelData
                                    panel.bind(text, "model." + roleRow.modelData)
                                    text = ""
                                }
                                Connections {
                                    target: panel
                                    function onSelectionMoved() { roleName.text = "" }
                                }
                            }
                        }
                        Note {
                            text: panel.says("role " + roleRow.modelData)
                            Layout.fillWidth: true
                        }
                    }
                }
            }

            Section {
                z: 2
                tail: Spacing.xxs
                title: "Comments"
                gap: 0
                Layout.fillWidth: true

                Repeater {
                    model: panel.comments
                    delegate: RowLayout {
                        id: commentRow
                        required property var modelData
                        readonly property string kind: modelData.orphaned ? "Orphaned"
                                                       : modelData.resolved ? "Resolved" : "Open"
                        Layout.fillWidth: true
                        Layout.bottomMargin: Spacing.xxs
                        spacing: Spacing.xxs

                        Label {
                            text: commentRow.modelData.orphaned
                                  ? commentRow.modelData.text + " (" + commentRow.modelData.label + ", removed)"
                                  : commentRow.modelData.text
                            color: commentRow.modelData.resolved ? Colors.textMuted : Colors.text
                            wrapMode: Text.Wrap
                            Layout.fillWidth: true
                            Accessible.role: Accessible.StaticText
                            Accessible.name: commentRow.kind + " comment: " + commentRow.modelData.text
                        }
                        TextButton {
                            text: commentRow.modelData.resolved ? "Reopen" : "Resolve"
                            Accessible.name: text + " " + commentRow.modelData.text
                            onClicked: panel.review({
                                action: commentRow.modelData.resolved ? "reopen" : "resolve",
                                comment: commentRow.modelData.id
                            })
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Spacing.xxs

                    Field {
                        id: newComment
                        placeholderText: "comment"
                        Accessible.name: "New comment"
                        Layout.fillWidth: true
                        onAccepted: if (addComment.enabled) addComment.clicked()
                    }
                    TextButton {
                        id: addComment
                        text: "Comment"
                        Accessible.name: "Add comment"
                        enabled: newComment.text.trim() !== "" && panel.workspace.selected !== ""
                        onClicked: {
                            panel.adding = true
                            panel.review({
                                action: "add",
                                doc: panel.workspace.doc,
                                node: {node: panel.workspace.selected},
                                text: newComment.text
                            })
                        }
                    }
                }
                Note {
                    text: panel.says("comment")
                    Layout.fillWidth: true
                }
            }

            // What the object declares of its own, and one more added by its
            // kind (`node_insert` of a member).
            Section {
                z: 1
                tail: Spacing.xxs
                title: "API"
                gap: 0
                visible: panel.workspace.selected !== ""
                Layout.fillWidth: true

                Repeater {
                    model: panel.declared
                    delegate: Label {
                        required property var modelData
                        text: modelData.says
                        color: Colors.textMuted
                        font.family: "monospace"
                        font.pixelSize: Typography.captionSize
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                        Layout.preferredHeight: 20
                        verticalAlignment: Text.AlignVCenter
                        Accessible.role: Accessible.StaticText
                        Accessible.name: "Declares " + modelData.name
                    }
                }
                RowLayout {
                    spacing: Spacing.xxs
                    Layout.fillWidth: true
                    Layout.topMargin: panel.declared.length > 0 ? Spacing.xxs : 0
                    Choice {
                        id: memberKind
                        model: panel.memberKinds
                        Accessible.name: "Member kind"
                        Layout.preferredWidth: 96
                    }
                    Field {
                        id: memberName
                        placeholderText: "name"
                        Accessible.name: "Member name"
                        Layout.fillWidth: true
                    }
                }
                RowLayout {
                    spacing: Spacing.xxs
                    Layout.fillWidth: true
                    Layout.topMargin: Spacing.xxs
                    Field {
                        id: memberType
                        visible: memberKind.currentIndex === 0
                        placeholderText: "var"
                        lead: "Type"
                        Accessible.name: "Member type"
                        Layout.preferredWidth: 96
                    }
                    Field {
                        id: memberValue
                        visible: memberKind.currentIndex === 0
                        placeholderText: "first value"
                        Accessible.name: "Member value"
                        Layout.fillWidth: true
                        onAccepted: panel.addMember()
                    }
                    Field {
                        id: memberArgs
                        visible: memberKind.currentIndex !== 0
                        placeholderText: "string name, int index"
                        lead: "("
                        Accessible.name: "Member arguments"
                        Layout.fillWidth: true
                        onAccepted: if (memberKind.currentIndex === 1) panel.addMember()
                    }
                    IconButton {
                        icon_: "plus"
                        text: "Add the member"
                        enabled: memberName.text.trim() !== ""
                        onClicked: panel.addMember()
                    }
                }
                Field {
                    id: memberBody
                    visible: memberKind.currentIndex === 2
                    placeholderText: "what it does"
                    lead: "{"
                    Accessible.name: "Member body"
                    Layout.fillWidth: true
                    Layout.topMargin: Spacing.xxs
                    onAccepted: panel.addMember()
                }
                Note {
                    text: panel.says("api")
                    Layout.fillWidth: true
                }
            }

            // Room for what the last gap may be saying: the list is clipped.
            Item {
                implicitHeight: Spacing.sm
            }
        }
    }
}
