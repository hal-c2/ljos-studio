// What Studio draws over the preview's picture: the selection, its handles,
// and a drag. A drag touches no file and asks nothing of the running app: it
// moves a ghost, so a frame costs a few rectangles. The drop is one call of
// the tool an agent has, `node_arrange`, which is one undo step.
//
// An object of a 3D scene has no box. It gets three arrows instead, one per
// axis of its `x`, `y` and `z`, drawn from the camera the answer carries, so
// a drag follows the pointer without asking the running app. The drop writes
// one axis through the same tool.
//
// Everything here is in the document's logical pixels times `zoom`.
import QtQuick
import QtQuick.Controls
import Ljos.Design
import Studio

MouseArea {
    id: overlay
    required property Workspace workspace
    required property real zoom
    required property bool playing
    required property var recording
    // What a drag draws: a type, or "" to select and move.
    required property string drawing
    signal drawn()
    // Which tool is in use is the panel's. Says whether the key was one.
    required property var toolKey
    // Why the last drop wrote nothing; said until the next press.
    property string told: ""

    // `node_arrange` `geometry`'s answer for the selection.
    property var nodes: []
    property var parentBox: null
    property var siblings: []

    // "": at rest. "selecting": pressed, the selection under the pointer is
    // on its way. "pressed": could still be a click. "moving", "resizing",
    // "drawing".
    property string mode: ""
    property int handle: -1
    property point pressedAt: Qt.point(0, 0)
    property var startBox: null
    // The box a drag shows; kept after a drop until the new geometry answers,
    // so the outline does not jump back for the length of a reload.
    property var ghost: null
    // A drop's write is on its way: a read asked before it, the press's, says
    // the old box and is no reason to let go of the ghost.
    property bool dropping: false
    property real guideX: -1
    property real guideY: -1
    property var snapXs: []
    property var snapYs: []
    readonly property bool dragging: mode !== ""

    // The one selected object of a 3D scene, with what draws its arrows.
    readonly property var spatial: nodes.length === 1 && nodes[0].scene !== undefined ? nodes[0].scene : null
    // An arrow's length on screen, and how near the pointer must be to take it.
    readonly property real arrowLength: 72
    readonly property real arrowReach: 6
    // Which axis (0, 1, 2) is pressed, where on its line, and what its
    // value was; the value a drag would write is in `ghost3`.
    property int axis: -1
    property real axisFrom: 0
    property var ghost3: null
    readonly property var axisNames: ["x", "y", "z"]
    readonly property var arrows: arrowsOf(spatial, ghost3, zoom, playing)
    readonly property int arrowsShown: arrows.length

    // Past this many screen pixels a press is a drag.
    readonly property real dragStart: 3
    // Within this many screen pixels an edge snaps.
    readonly property real snapReach: 6
    readonly property real handleSize: 8

    // Lining up needs a box: an object of a 3D scene is seen and has none.
    readonly property bool hasBoxes: nodes.length === 0 || nodes.some(n => n.window !== undefined)
    readonly property var selectedOne: nodes.length === 1 && nodes[0].window !== undefined ? nodes[0] : null
    readonly property bool resizable: selectedOne !== null && selectedOne.drag.resize !== "none"
    readonly property int handlesShown: resizable && !playing && ghost === null && drawing === "" && editing === null ? 8 : 0
    onRecordingChanged: told = ""

    // What a drag does, said before and during it.
    readonly property string says: {
        if (workspace.chosen.length === 0 || playing)
            return ""
        if (told !== "")
            return told
        if (nodes.length > 0 && nodes.every(n => n.window === undefined))
            return "This object has no box on the canvas"
        return nodes.length === 1 ? nodes[0].drag.says : ""
    }

    hoverEnabled: !playing && nodes.length > 0
    preventStealing: true
    acceptedButtons: Qt.LeftButton
    focus: true
    cursorShape: hoverCursor

    property int hoverCursor: Qt.ArrowCursor
    readonly property var cursors: [Qt.SizeFDiagCursor, Qt.SizeVerCursor, Qt.SizeBDiagCursor,
                                    Qt.SizeHorCursor, Qt.SizeFDiagCursor, Qt.SizeVerCursor,
                                    Qt.SizeBDiagCursor, Qt.SizeHorCursor]
    // Where each handle sits on the box, clockwise from the top left.
    readonly property var handleX: [0, 0.5, 1, 1, 1, 0.5, 0, 0]
    readonly property var handleY: [0, 0, 0, 0.5, 1, 1, 1, 0.5]

    function refetch() {
        const chosen = workspace.chosen
        if (chosen.length === 0 || workspace.preview === null) {
            nodes = []
            parentBox = null
            siblings = []
            // No read is coming to let go of a ghost.
            if (!dragging)
                ghost = null
            return
        }
        ++reads
        workspace.dispatch("node_arrange", {
            action: "geometry",
            doc: workspace.doc,
            nodes: chosen.map(h => ({node: h})),
            // Only a press can start the drag that snaps to them, and each
            // sibling is a read of the running app: a selection made anywhere
            // but on the canvas does not wait for them.
            without_siblings: mode === "",
            within: mode === "drawing"
        })
    }

    function dot(a, b) {
        return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    function plus(a, b, by) {
        return [a[0] + by * b[0], a[1] + by * b[1], a[2] + by * b[2]]
    }

    // A point of the scene on the window, in logical pixels; null behind the
    // camera.
    function project(s, p) {
        const eye = s.eye
        const d = [p[0] - eye.position[0], p[1] - eye.position[1], p[2] - eye.position[2]]
        const depth = dot(d, eye.forward)
        if (depth <= 0.001)
            return null
        const nx = dot(d, eye.right) / (depth * eye.tan_y * eye.aspect)
        const ny = dot(d, eye.up) / (depth * eye.tan_y)
        return {x: s.view.x + (nx + 1) / 2 * s.view.width, y: s.view.y + (1 - ny) / 2 * s.view.height}
    }

    // The arrows for `s`, its origin moved by the drag in progress: where each
    // starts and ends on the window, and the value it would write.
    function arrowsOf(s, moved, scale, off) {
        if (s === null || off)
            return []
        let origin = s.origin
        if (moved !== null)
            origin = plus(origin, s.axes[moved.axis], moved.value - s.local[moved.axis])
        const from = project(s, origin)
        if (from === null)
            return []
        const out = []
        for (let i = 0; i < 3; ++i) {
            // One unit of the axis shows how long the arrow is to be, whatever
            // the view's distance, so it is drawn the same size.
            const unit = project(s, plus(origin, s.axes[i], 1))
            if (unit === null)
                continue
            const per = Math.hypot(unit.x - from.x, unit.y - from.y)
            if (per < 1e-6)
                continue
            const length = arrowLength / scale / per
            const to = project(s, plus(origin, s.axes[i], length))
            if (to === null)
                continue
            out.push({axis: i, name: axisNames[i], from: from, to: to, refused: s.refused[i]})
        }
        return out
    }

    // The arrow under a point of the overlay, if any.
    function arrowAt(x, y) {
        let best = -1
        let nearest = arrowReach + 1
        for (const a of arrows) {
            const ax = a.from.x * zoom
            const ay = a.from.y * zoom
            const dx = a.to.x * zoom - ax
            const dy = a.to.y * zoom - ay
            const along = Math.max(0, Math.min(1, ((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)))
            const off = Math.hypot(x - (ax + along * dx), y - (ay + along * dy))
            if (off <= arrowReach && off < nearest) {
                nearest = off
                best = a.axis
            }
        }
        return best
    }

    // Where on the line of axis `i` through the object the pointer is, in
    // units of that axis: the point of the line nearest the ray the pointer
    // casts. Perspective is in it, so the object stays under the pointer.
    // NaN when the line points at the camera and the pointer says nothing.
    function axisAt(i, x, y) {
        const s = spatial
        const eye = s.eye
        const nx = 2 * (x / zoom - s.view.x) / s.view.width - 1
        const ny = 1 - 2 * (y / zoom - s.view.y) / s.view.height
        const sx = nx * eye.tan_y * eye.aspect
        const sy = ny * eye.tan_y
        const d = [eye.forward[0] + eye.right[0] * sx + eye.up[0] * sy,
                   eye.forward[1] + eye.right[1] * sx + eye.up[1] * sy,
                   eye.forward[2] + eye.right[2] * sx + eye.up[2] * sy]
        const a = s.axes[i]
        const w = [s.origin[0] - eye.position[0], s.origin[1] - eye.position[1], s.origin[2] - eye.position[2]]
        const aa = dot(a, a)
        const ad = dot(a, d)
        const dd = dot(d, d)
        const denominator = aa * dd - ad * ad
        if (Math.abs(denominator) < 1e-9 * aa * dd)
            return NaN
        return (ad * dot(d, w) - dd * dot(a, w)) / denominator
    }

    function dragArrow(x, y, free) {
        const at = axisAt(axis, x, y)
        if (isNaN(at))
            return
        let value = spatial.local[axis] + at - axisFrom
        // Whole units unless Ctrl asks for the hand's own.
        value = free ? Math.round(value * 100) / 100 : Math.round(value)
        ghost3 = {axis: axis, value: value}
    }

    function dropArrow() {
        if (ghost3 === null || ghost3.value === spatial.local[axis])
            return false
        const place = {node: {node: nodes[0].node}}
        place[axisNames[axis]] = ghost3.value
        overlay.place([place])
        return true
    }

    function around() {
        const shown = nodes.filter(n => n.window !== undefined)
        if (shown.length === 0)
            return null
        const left = Math.min(...shown.map(n => n.window.x))
        const top = Math.min(...shown.map(n => n.window.y))
        const right = Math.max(...shown.map(n => n.window.x + n.window.width))
        const bottom = Math.max(...shown.map(n => n.window.y + n.window.height))
        return {x: left, y: top, width: right - left, height: bottom - top}
    }

    function handleAt(x, y) {
        if (!resizable)
            return -1
        const box = selectedOne.window
        const reach = handleSize / 2 + 1
        for (let i = 0; i < 8; ++i) {
            const hx = (box.x + box.width * handleX[i]) * zoom
            const hy = (box.y + box.height * handleY[i]) * zoom
            if (Math.abs(x - hx) <= reach && Math.abs(y - hy) <= reach)
                return i
        }
        return -1
    }

    function inside(x, y) {
        const at = Qt.point(x / zoom, y / zoom)
        return nodes.some(n => n.window !== undefined
                          && at.x >= n.window.x && at.x <= n.window.x + n.window.width
                          && at.y >= n.window.y && at.y <= n.window.y + n.window.height)
    }

    property bool gathering: false

    function selectAt(x, y, add) {
        const args = {doc: workspace.doc, at: {x: Math.floor(x / zoom), y: Math.floor(y / zoom)}}
        if (add)
            args.add = true
        workspace.dispatch("node_select", args)
    }

    function startSnaps() {
        const xs = []
        const ys = []
        const add = (box) => {
            xs.push(box.x, box.x + box.width / 2, box.x + box.width)
            ys.push(box.y, box.y + box.height / 2, box.y + box.height)
        }
        if (parentBox !== null)
            add(parentBox)
        for (const s of siblings)
            add(s.window)
        // While drawing, the object under the start is a target too.
        if (mode === "drawing") {
            for (const n of nodes) {
                if (n.window !== undefined)
                    add(n.window)
            }
        }
        snapXs = xs
        snapYs = ys
    }

    // The smallest move that puts one of `edges` on one of `targets`, and
    // where the guide goes; nothing within reach is no move and no guide.
    function snap(edges, targets) {
        const reach = snapReach / zoom
        let best = {by: 0, at: -1, off: reach + 1}
        for (const edge of edges) {
            for (const target of targets) {
                const off = Math.abs(target - edge)
                if (off <= reach && off < best.off)
                    best = {by: target - edge, at: target, off: off}
            }
        }
        return best
    }

    function dragMove(dx, dy, free) {
        const box = startBox
        const moved = {x: box.x + dx, y: box.y + dy, width: box.width, height: box.height}
        let sx = {by: 0, at: -1}
        let sy = {by: 0, at: -1}
        if (!free) {
            sx = snap([moved.x, moved.x + moved.width / 2, moved.x + moved.width], snapXs)
            sy = snap([moved.y, moved.y + moved.height / 2, moved.y + moved.height], snapYs)
        }
        moved.x += sx.by
        moved.y += sy.by
        guideX = sx.at
        guideY = sy.at
        ghost = moved
    }

    function dragResize(dx, dy, free) {
        const box = startBox
        let left = box.x
        let top = box.y
        let right = box.x + box.width
        let bottom = box.y + box.height
        const fx = handleX[handle]
        const fy = handleY[handle]
        let sx = {by: 0, at: -1}
        let sy = {by: 0, at: -1}
        if (fx === 0) {
            left += dx
            if (!free)
                sx = snap([left], snapXs)
            left += sx.by
        } else if (fx === 1) {
            right += dx
            if (!free)
                sx = snap([right], snapXs)
            right += sx.by
        }
        if (fy === 0) {
            top += dy
            if (!free)
                sy = snap([top], snapYs)
            top += sy.by
        } else if (fy === 1) {
            bottom += dy
            if (!free)
                sy = snap([bottom], snapYs)
            bottom += sy.by
        }
        guideX = sx.at
        guideY = sy.at
        // A handle dragged past the other side stops there: an object has no
        // negative size to write.
        ghost = {
            x: Math.min(left, right - 1),
            y: Math.min(top, bottom - 1),
            width: Math.max(1, right - left),
            height: Math.max(1, bottom - top)
        }
    }

    function dragDraw(atX, atY, free) {
        startSnaps()
        const sx = free ? {by: 0, at: -1} : snap([atX], snapXs)
        const sy = free ? {by: 0, at: -1} : snap([atY], snapYs)
        // Where it starts snaps to the frame only, which shows without a guide:
        // a box drawn from a corner would miss it by what a screen pixel is
        // worth when zoomed out.
        const startX = pressedAt.x / zoom
        const startY = pressedAt.y / zoom
        const fromX = startX + (free ? 0 : snap([startX], [0, width / zoom]).by)
        const fromY = startY + (free ? 0 : snap([startY], [0, height / zoom]).by)
        const toX = atX + sx.by
        const toY = atY + sy.by
        guideX = sx.at
        guideY = sy.at
        ghost = {
            x: Math.min(fromX, toX),
            y: Math.min(fromY, toY),
            width: Math.max(1, Math.abs(toX - fromX)),
            height: Math.max(1, Math.abs(toY - fromY))
        }
    }

    function dropMove() {
        const dx = Math.round(ghost.x - startBox.x)
        const dy = Math.round(ghost.y - startBox.y)
        if (dx === 0 && dy === 0)
            return false
        const places = []
        for (const n of nodes) {
            if (n.local === undefined || (n.drag.move !== "position" && n.drag.move !== "margins"))
                continue
            const place = {node: {node: n.node}}
            if (dx !== 0)
                place.x = Math.round(n.local.x + dx)
            if (dy !== 0)
                place.y = Math.round(n.local.y + dy)
            places.push(place)
        }
        if (places.length > 0) {
            overlay.place(places)
            return true
        }
        return selectedOne !== null && selectedOne.drag.move === "reorder" && dropReorder()
    }

    // Every gesture that sets a value, so all of them follow `recording`.
    function place(places) {
        told = ""
        const args = {action: "place", doc: workspace.doc, places: places}
        if (recording !== null)
            args.record = recording
        workspace.dispatch("node_arrange", args)
    }

    // A layout places the object: the drop picks its place among the others.
    function dropReorder() {
        if (recording !== null) {
            told = "A state cannot change the order of a layout"
            return false
        }
        // The hand let go before the press's read answered: the drop is decided
        // when it does.
        if (siblings.length === 0 && reads > 0) {
            dropWaits = true
            return true
        }
        const layers = workspace.layers
        const layerOf = (node) => layers.find(l => l.node === node)
        const self = layerOf(selectedOne.node)
        if (self === undefined || self.index === null)
            return false
        const cx = ghost.x + ghost.width / 2
        const cy = ghost.y + ghost.height / 2
        let nearest = null
        let distance = Infinity
        for (const s of siblings) {
            const sx = s.window.x + s.window.width / 2
            const sy = s.window.y + s.window.height / 2
            const d = Math.hypot(cx - sx, cy - sy)
            if (d < distance) {
                distance = d
                nearest = {layer: layerOf(s.node), after: Math.abs(cx - sx) > Math.abs(cy - sy) ? cx > sx : cy > sy}
            }
        }
        if (nearest === null || nearest.layer === undefined || nearest.layer.index === null)
            return false
        let index = nearest.layer.index + (nearest.after ? 1 : 0)
        if (self.index < index)
            index -= 1
        if (index === self.index)
            return false
        workspace.dispatch("node_move", {
            doc: workspace.doc,
            node: {node: self.node},
            parent: {node: self.parent},
            index: index
        })
        return true
    }

    function dropResize() {
        const n = selectedOne
        const dx = Math.round(ghost.x - startBox.x)
        const dy = Math.round(ghost.y - startBox.y)
        const width = Math.round(ghost.width)
        const height = Math.round(ghost.height)
        const place = {node: {node: n.node}}
        if (n.drag.move === "position" || n.drag.move === "margins") {
            if (dx !== 0)
                place.x = Math.round(n.local.x + dx)
            if (dy !== 0)
                place.y = Math.round(n.local.y + dy)
        }
        if (width !== Math.round(startBox.width))
            place.width = width
        if (height !== Math.round(startBox.height))
            place.height = height
        if (Object.keys(place).length === 1)
            return false
        overlay.place([place])
        return true
    }

    function endDrag(wrote) {
        mode = ""
        guideX = -1
        guideY = -1
        if (wrote)
            dropping = true
        else
            endDrop(false)
        axis = -1
    }

    // Every way a drop ends comes here. A write that went in keeps the ghost
    // for the read that says the new box, so the outline does not jump back;
    // otherwise there is nothing to wait for. The ghost of a drag still in
    // hand is that drag's.
    function endDrop(wentIn) {
        dropWaits = false
        dropping = false
        if (!wentIn && !dragging) {
            ghost = null
            ghost3 = null
        }
    }

    // What was held was for an object no longer selected: letting go must
    // not write it to whatever is selected now.
    function letGo() {
        mode = ""
        guideX = -1
        guideY = -1
        endDrop(false)
    }

    // Each press is its own undo step. The boxes move at once, so a key held
    // down adds up rather than repeating from where the file last was.
    function nudge(dx, dy) {
        const places = []
        const moved = []
        for (const n of nodes) {
            const movable = n.local !== undefined && (n.drag.move === "position" || n.drag.move === "margins")
            if (!movable) {
                moved.push(n)
                continue
            }
            const local = {x: n.local.x + dx, y: n.local.y + dy, width: n.local.width, height: n.local.height}
            const window = {x: n.window.x + dx, y: n.window.y + dy, width: n.window.width, height: n.window.height}
            const place = {node: {node: n.node}}
            if (dx !== 0)
                place.x = local.x
            if (dy !== 0)
                place.y = local.y
            places.push(place)
            moved.push({node: n.node, local: local, window: window, drag: n.drag})
        }
        if (places.length === 0)
            return
        nodes = moved
        writing = true
        settled.restart()
        overlay.place(places)
    }

    // A drawing goes in the object under where it starts when that can hold
    // it and holds all of it; otherwise in the root. A layout places what is
    // put in it, so nothing is written about where.
    function drawInto(box) {
        const layers = workspace.layers
        const root = layers.length > 0 ? layers[0] : null
        const under = nodes.length === 1 ? nodes[0] : null
        const row = under === null ? undefined : layers.find(l => l.node === under.node)
        if (row !== undefined && under.window !== undefined
                && /^(Item|Rectangle|Pane|Frame|Page|GroupBox)$/.test(row.type)
                && box.x >= under.window.x && box.y >= under.window.y
                && box.x + box.width <= under.window.x + under.window.width
                && box.y + box.height <= under.window.y + under.window.height) {
            return {node: row.node, x: under.window.x, y: under.window.y, placed: true}
        }
        if (row !== undefined && /^(Row|Column|Grid|Flow|RowLayout|ColumnLayout|GridLayout)$/.test(row.type))
            return {node: row.node, x: 0, y: 0, placed: false}
        return root === null ? null : {node: root.node, x: 0, y: 0, placed: true}
    }

    function dropDrawing() {
        // A click is a request for one of a usable size, starting there.
        const clicked = ghost === null
        const box = clicked ? {
            x: pressedAt.x / zoom,
            y: pressedAt.y / zoom,
            width: drawing === "Text" ? 160 : 100,
            height: drawing === "Text" ? 24 : 100
        } : ghost
        const into = drawInto(box)
        const kind = drawing
        drawn()
        if (into === null)
            return
        const parts = []
        if (into.placed)
            parts.push("x: " + Math.round(box.x - into.x), "y: " + Math.round(box.y - into.y))
        parts.push("width: " + Math.round(box.width))
        if (kind === "Rectangle") {
            parts.push("height: " + Math.round(box.height), 'color: "#c8ccd4"')
        } else {
            parts.push('text: "Text"', "font.pixelSize: 16", "wrapMode: Text.Wrap")
            writing_ = {box: box, size: 16, written: "Text"}
        }
        workspace.dispatch("node_insert", {
            doc: workspace.doc,
            parent: {node: into.node},
            text: kind + " {\n    " + parts.join("\n    ") + "\n}"
        })
    }

    // Text being written on the canvas: the object's `node`, its `box` and what
    // it said. Held in the overlay so the picture below does not have to follow.
    property var editing: null
    // A drawn text waits for `node_insert` to name it.
    property var writing_: null

    function editSelected() {
        const n = selectedOne
        if (n === null || recording !== null)
            return
        const row = workspace.inspector.find(r => r.name === "text")
        if (row === undefined)
            return
        const written = workspace.literal(row.value) && /^["'`]/.test(row.value) ? decoded(row.value) : null
        if (written === null) {
            told = "This text is an expression: change it in the inspector"
            return
        }
        const size = workspace.inspector.find(r => r.name === "font.pixelSize")
        editing = {
            doc: workspace.doc,
            node: n.node,
            box: n.window,
            size: size !== undefined && !isNaN(Number(size.value)) ? Number(size.value) : 14,
            written: written
        }
    }

    // What a QML string says, in any of its quotes. A template with a
    // placeholder says nothing until it runs: null, as for what cannot be read.
    function decoded(literal) {
        const body = literal.slice(1, -1)
        if (literal[0] === "`" && body.includes("${"))
            return null
        const named = {n: "\n", t: "\t", r: "\r", b: "\b", f: "\f", v: "\v", "0": "\0"}
        let readable = true
        const text = body.replace(/\\(u\{[0-9a-fA-F]+\}|u[0-9a-fA-F]{4}|x[0-9a-fA-F]{2}|\r?\n|[^])/g, (_, what) => {
            if (what[0] === "\n" || what[0] === "\r")
                return ""
            if (named[what] !== undefined)
                return named[what]
            if (what.length > 1)
                return String.fromCodePoint(parseInt(what.replace(/[ux{}]/g, ""), 16))
            if (what === "x" || what === "u")
                readable = false
            return what
        })
        return readable ? text : null
    }

    function finishEditing(keep) {
        const was = editing
        if (was === null)
            return
        const text = canvasText.text
        const hadKeys = canvasText.activeFocus
        editing = null
        if (hadKeys)
            forceActiveFocus()
        if (keep && text !== was.written) {
            workspace.dispatch("binding_set", {
                doc: was.doc,
                node: {node: was.node},
                name: "text",
                value: JSON.stringify(text)
            })
        }
    }

    function selectedHandles() {
        return workspace.chosen.map(h => ({node: h}))
    }

    function pasteParent() {
        const layers = workspace.layers
        const first = layers.find(l => l.node === workspace.selected)
        if (first !== undefined && first.parent !== "")
            return first.parent
        return layers.length > 0 ? layers[0].node : ""
    }

    onPressed: (mouse) => {
        forceActiveFocus()
        endDrop(true)
        if (playing)
            return
        finishEditing(true)
        told = ""
        pressedAt = Qt.point(mouse.x, mouse.y)
        if (drawing !== "") {
            ghost = null
            mode = "drawing"
            handle = -1
            selectAt(mouse.x, mouse.y, false)
            return
        }
        const shift = (mouse.modifiers & Qt.ShiftModifier) !== 0
        gathering = shift
        handle = shift ? -1 : handleAt(mouse.x, mouse.y)
        const arrow = shift || spatial === null ? -1 : arrowAt(mouse.x, mouse.y)
        if (arrow >= 0) {
            // An axis that an expression sets is not drawn over: the person
            // is told, and the file is not touched.
            if (spatial.refused[arrow] !== "") {
                told = spatial.refused[arrow]
                return
            }
            axis = arrow
            axisFrom = axisAt(arrow, mouse.x, mouse.y)
            mode = "pressed3"
        } else if (handle >= 0) {
            startBox = selectedOne.window
            mode = "pressed"
            refetch()
        } else if (!shift && inside(mouse.x, mouse.y)) {
            startBox = around()
            mode = "pressed"
            refetch()
        } else {
            mode = "selecting"
            selectAt(mouse.x, mouse.y, shift)
        }
    }

    onPositionChanged: (mouse) => {
        if (playing)
            return
        if (mode === "") {
            const at = handleAt(mouse.x, mouse.y)
            hoverCursor = at >= 0 ? cursors[at]
                : (arrowAt(mouse.x, mouse.y) >= 0 ? Qt.SizeAllCursor
                   : (inside(mouse.x, mouse.y) ? Qt.SizeAllCursor : Qt.ArrowCursor))
            return
        }
        if (mode === "pressed3" || mode === "moving3") {
            const away = Math.hypot(mouse.x - pressedAt.x, mouse.y - pressedAt.y)
            if (mode === "pressed3" && away < dragStart)
                return
            mode = "moving3"
            dragArrow(mouse.x, mouse.y, (mouse.modifiers & Qt.ControlModifier) !== 0)
            return
        }
        if (mode === "selecting")
            return
        const free = (mouse.modifiers & Qt.ControlModifier) !== 0
        if (mode === "drawing") {
            if (ghost !== null || Math.hypot(mouse.x - pressedAt.x, mouse.y - pressedAt.y) >= dragStart)
                dragDraw(mouse.x / zoom, mouse.y / zoom, free)
            return
        }
        if (mode === "pressed") {
            const away = Math.hypot(mouse.x - pressedAt.x, mouse.y - pressedAt.y)
            if (away < dragStart)
                return
            startSnaps()
            mode = handle >= 0 ? "resizing" : "moving"
        }
        const dx = (mouse.x - pressedAt.x) / zoom
        const dy = (mouse.y - pressedAt.y) / zoom
        if (mode === "moving")
            dragMove(dx, dy, free)
        else
            dragResize(dx, dy, free)
    }

    onReleased: (mouse) => {
        if (playing) {
            const x = mouse.x / zoom
            const y = mouse.y / zoom
            // The app is clicked at the very point, as a person's pointer
            // would be. The root's state says where the flow is, when the
            // preview can find the root: by its `id`.
            const layers = workspace.layers
            const args = {click_at: {x: x, y: y}}
            if (layers.length > 0 && layers[0].id !== "")
                args.node = {path: "root"}
            workspace.dispatch("preview_state", args)
            return
        }
        let wrote = false
        if (mode === "drawing") {
            dropDrawing()
            endDrag(false)
            return
        }
        if (mode === "moving3")
            wrote = dropArrow()
        else if (mode === "moving")
            wrote = dropMove()
        else if (mode === "resizing")
            wrote = dropResize()
        else if (mode === "pressed")
            // A click inside the selection may mean what is under it, unless
            // it was Shift's: that one has just added it.
            if (!gathering)
                selectAt(mouse.x, mouse.y, false)
        endDrag(wrote)
    }

    onCanceled: endDrag(false)

    // Text is written on the object it belongs to, as a person expects of text.
    onDoubleClicked: if (!playing && drawing === "") editSelected()

    Keys.onPressed: (event) => {
        // While playing, keys are the running prototype's, and no outline
        // shows what an edit would hit.
        if (playing) {
            event.accepted = false
            return
        }
        if (event.key === Qt.Key_Escape && drawing !== "") {
            drawn()
            event.accepted = true
            return
        }
        if (toolKey(event)) {
            event.accepted = true
            return
        }
        const ctrl = (event.modifiers & Qt.ControlModifier) !== 0
        const step = (event.modifiers & Qt.ShiftModifier) !== 0 ? 10 : 1
        const has = workspace.chosen.length > 0
        event.accepted = true
        if (event.key === Qt.Key_Left && has)
            nudge(-step, 0)
        else if (event.key === Qt.Key_Right && has)
            nudge(step, 0)
        else if (event.key === Qt.Key_Up && has)
            nudge(0, -step)
        else if (event.key === Qt.Key_Down && has)
            nudge(0, step)
        else if (ctrl && event.key === Qt.Key_C && has)
            workspace.dispatch("node_arrange", {action: "copy", doc: workspace.doc, nodes: selectedHandles()})
        else if (ctrl && event.key === Qt.Key_V)
            workspace.dispatch("node_arrange", {action: "paste", doc: workspace.doc, parent: {node: pasteParent()}})
        else if (ctrl && event.key === Qt.Key_D && has)
            workspace.dispatch("node_arrange", {action: "duplicate", doc: workspace.doc, nodes: selectedHandles()})
        else if (!ctrl && (event.key === Qt.Key_Delete || event.key === Qt.Key_Backspace)
                 && has)
            workspace.dispatch("node_arrange", {action: "remove", doc: workspace.doc, nodes: selectedHandles()})
        else
            event.accepted = false
    }

    Connections {
        target: overlay.workspace
        // A list set again need not be another selection: only one that differs
        // reads again, or an idle window could feed itself and never rest.
        function onChosenChanged() {
            const key = JSON.stringify(overlay.workspace.chosen)
            // What is typed is the person's: it goes in, on what it was for.
            if (overlay.editing !== null && overlay.workspace.selected !== overlay.editing.node)
                overlay.finishEditing(true)
            if (key !== overlay.seenChosen) {
                overlay.seenChosen = key
                overlay.told = ""
                overlay.writing = false
                // Choosing is how a press on another object starts its drag,
                // and a drawing chooses the container it starts in.
                if (overlay.mode !== "selecting" && overlay.mode !== "drawing")
                    overlay.letGo()
                overlay.refetch()
            }
        }
        function onPreviewChanged() {
            const preview = overlay.workspace.preview
            const generation = preview !== null ? preview.generation : -1
            if (generation !== overlay.seenGeneration) {
                overlay.seenGeneration = generation
                if (overlay.writing)
                    settled.restart()
                else
                    overlay.refetch()
            }
        }
        function onAnswered(tool, answer) {
            if (tool === "node_select" && (overlay.mode === "selecting" || overlay.mode === "drawing")) {
                overlay.refetch()
            } else if (tool === "node_insert" && overlay.pendingDrawing(answer)) {
                return
            } else if (tool === "node_move") {
                overlay.endDrop(true)
                overlay.refetch()
            } else if (tool === "node_arrange") {
                overlay.reread = false
                if (answer.edit === undefined && answer.generation === undefined) {
                    if (answer.copied === undefined)
                        overlay.reads = Math.max(0, overlay.reads - 1)
                    if (overlay.dropWaits && answer.copied === undefined) {
                        overlay.siblings = answer.siblings !== undefined ? answer.siblings : []
                        if (overlay.siblings.length > 0 || overlay.reads === 0) {
                            overlay.dropWaits = false
                            // An agent may have emptied the selection's boxes meanwhile.
                            if (overlay.selectedOne === null || !overlay.dropReorder()) {
                                overlay.endDrop(false)
                                overlay.refetch()
                            }
                        }
                        return
                    }
                    if (answer.copied !== undefined || overlay.writing)
                        return
                    overlay.nodes = answer.boxes !== undefined ? answer.boxes : []
                    overlay.parentBox = answer.parent !== undefined ? answer.parent : null
                    overlay.siblings = answer.siblings !== undefined ? answer.siblings : []
                    overlay.ghost = overlay.dragging || overlay.dropping ? overlay.ghost : null
                    overlay.ghost3 = overlay.dragging || overlay.dropping ? overlay.ghost3 : null
                    // The hand may be ahead of the siblings it snaps to.
                    if (overlay.mode === "moving" || overlay.mode === "resizing" || overlay.mode === "drawing")
                        overlay.startSnaps()
                    if (overlay.mode === "selecting") {
                        overlay.startBox = overlay.around()
                        if (overlay.startBox !== null)
                            overlay.mode = "pressed"
                    }
                } else {
                    // The read asked now is the first that can say the new box.
                    overlay.endDrop(true)
                    if (overlay.writing)
                        settled.restart()
                    else
                        overlay.refetch()
                }
            }
        }
        function onDocChanged() {
            overlay.finishEditing(true)
        }
        function onRefused(tool, error) {
            if (tool === "node_insert") {
                overlay.writing_ = null
            } else if (tool === "node_move") {
                overlay.endDrop(false)
                overlay.refetch()
            } else if (tool === "node_arrange") {
                // A refused read leaves nothing to draw; a refused drop goes
                // back to what the preview says.
                overlay.writing = false
                overlay.endDrop(false)
                // Read again once only: when the read is what is refused
                // (a preview gone, or one that cannot say), asking on would
                // keep an idle window busy for ever.
                overlay.reads = Math.max(0, overlay.reads - 1)
                const refusedTwice = overlay.reread
                overlay.reread = !refusedTwice
                if (refusedTwice || (error.details !== undefined && error.details.kind === "no_preview")) {
                    overlay.nodes = []
                    overlay.parentBox = null
                    overlay.siblings = []
                    overlay.reread = false
                } else {
                    overlay.refetch()
                }
            }
        }
    }
    // Presses move the outline ahead of the preview, which is still showing
    // the file as it was: its boxes would take the outline back. They count
    // again once the presses and reloads have stopped.
    property bool writing: false
    property bool reread: false
    Timer {
        id: settled
        interval: 300
        onTriggered: {
            overlay.writing = false
            overlay.refetch()
        }
    }
    // What was drawn is what is selected next, whoever draws in this window.
    function pendingDrawing(answer) {
        if (answer.node === undefined)
            return false
        const text = writing_
        writing_ = null
        workspace.dispatch("node_select", {doc: workspace.doc, nodes: [{node: answer.node}]})
        if (text !== null)
            editing = {doc: workspace.doc, node: answer.node, box: text.box, size: text.size, written: text.written}
        return true
    }
    // Geometry reads asked and not answered; they answer in order.
    property int reads: 0
    property bool dropWaits: false
    property int seenGeneration: -1
    property string seenChosen: "[]"

    // One outline per object; no drawing while nothing is selected.
    Repeater {
        model: overlay.playing || overlay.mode === "drawing" ? [] : overlay.nodes
        delegate: Rectangle {
            required property var modelData
            objectName: "selectionOutline"
            visible: modelData.window !== undefined
            x: visible ? modelData.window.x * overlay.zoom : 0
            y: visible ? modelData.window.y * overlay.zoom : 0
            width: visible ? modelData.window.width * overlay.zoom : 0
            height: visible ? modelData.window.height * overlay.zoom : 0
            color: "transparent"
            border.color: Colors.accent
            border.width: 1
            opacity: overlay.ghost !== null ? 0.4 : 1
        }
    }

    Repeater {
        model: overlay.handlesShown
        delegate: Rectangle {
            required property int index
            objectName: "resizeHandle"
            readonly property var box: overlay.selectedOne !== null ? overlay.selectedOne.window : {x: 0, y: 0, width: 0, height: 0}
            width: overlay.handleSize
            height: overlay.handleSize
            x: (box.x + box.width * overlay.handleX[index]) * overlay.zoom - width / 2
            y: (box.y + box.height * overlay.handleY[index]) * overlay.zoom - height / 2
            color: Colors.surface
            border.color: Colors.accent
            border.width: 1
        }
    }

    // The arrows of an object of a 3D scene, the colours telling the axes
    // apart and each named, since a colour alone does not.
    Repeater {
        model: overlay.arrows
        delegate: Item {
            id: arrow
            required property var modelData
            objectName: "moveArrow"
            readonly property real dx: (modelData.to.x - modelData.from.x) * overlay.zoom
            readonly property real dy: (modelData.to.y - modelData.from.y) * overlay.zoom
            readonly property color tint: modelData.axis === 0 ? Colors.danger
                : (modelData.axis === 1 ? Colors.accent : Colors.text)
            // A refused axis is still shown, dimmed: it is there, it is not
            // the pointer's to move.
            opacity: modelData.refused !== "" ? 0.4 : (overlay.ghost3 !== null && overlay.ghost3.axis !== modelData.axis ? 0.4 : 1)
            x: modelData.from.x * overlay.zoom
            y: modelData.from.y * overlay.zoom
            Rectangle {
                width: Math.hypot(arrow.dx, arrow.dy)
                height: 2
                y: -1
                color: arrow.tint
                transformOrigin: Item.Left
                rotation: Math.atan2(arrow.dy, arrow.dx) * 180 / Math.PI
            }
            Rectangle {
                x: arrow.dx - width / 2
                y: arrow.dy - height / 2
                width: overlay.handleSize + 2
                height: width
                radius: width / 2
                color: Colors.surface
                border.color: arrow.tint
                border.width: 2
            }
            Label {
                x: arrow.dx + (arrow.dx >= 0 ? Spacing.xs : -width - Spacing.xs)
                y: arrow.dy - height / 2
                text: modelData.name.toUpperCase()
                color: arrow.tint
                font.pixelSize: Typography.captionSize
                font.weight: Typography.bodyStrongWeight
            }
        }
    }

    Label {
        objectName: "dragValue"
        visible: overlay.ghost3 !== null
        x: overlay.arrows.length > 0 ? overlay.arrows[0].from.x * overlay.zoom + Spacing.sm : 0
        y: overlay.arrows.length > 0 ? overlay.arrows[0].from.y * overlay.zoom + Spacing.sm : 0
        text: overlay.ghost3 !== null ? overlay.axisNames[overlay.ghost3.axis] + " " + overlay.ghost3.value : ""
        color: Colors.accent
        font.pixelSize: Typography.captionSize
    }

    Rectangle {
        objectName: "dragGhost"
        visible: overlay.ghost !== null
        x: visible ? overlay.ghost.x * overlay.zoom : 0
        y: visible ? overlay.ghost.y * overlay.zoom : 0
        width: visible ? overlay.ghost.width * overlay.zoom : 0
        height: visible ? overlay.ghost.height * overlay.zoom : 0
        color: "transparent"
        border.color: Colors.accent
        border.width: 2

        Label {
            objectName: "dragSize"
            visible: overlay.mode === "resizing" || overlay.mode === "drawing"
            y: parent.height + Spacing.xxs
            text: overlay.ghost !== null ? Math.round(overlay.ghost.width) + " × " + Math.round(overlay.ghost.height) : ""
            color: Colors.accent
            font.pixelSize: Typography.captionSize
        }
    }

    Rectangle {
        objectName: "guideX"
        visible: overlay.guideX >= 0
        x: overlay.guideX * overlay.zoom
        width: 1
        height: overlay.height
        color: Colors.danger
    }
    Rectangle {
        objectName: "guideY"
        visible: overlay.guideY >= 0
        y: overlay.guideY * overlay.zoom
        width: overlay.width
        height: 1
        color: Colors.danger
    }

    TextArea {
        id: canvasText
        objectName: "canvasText"
        visible: overlay.editing !== null
        enabled: visible
        x: visible ? overlay.editing.box.x * overlay.zoom : 0
        y: visible ? overlay.editing.box.y * overlay.zoom : 0
        width: visible ? Math.max(80, overlay.editing.box.width * overlay.zoom) : 0
        padding: 0
        font.pixelSize: visible ? overlay.editing.size * overlay.zoom : 14
        wrapMode: TextEdit.Wrap
        color: Colors.text
        background: Rectangle {
            color: Colors.surface
            border.color: Colors.accent
        }
        Accessible.name: "Text of the selected object"
        onActiveFocusChanged: {
            if (!activeFocus && visible)
                overlay.finishEditing(true)
        }
        onVisibleChanged: {
            if (visible) {
                text = overlay.editing.written
                selectAll()
                forceActiveFocus()
            }
        }
        Keys.onPressed: (event) => {
            if (event.key === Qt.Key_Escape) {
                overlay.finishEditing(false)
                event.accepted = true
            } else if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
                       && (event.modifiers & Qt.ShiftModifier) === 0) {
                overlay.finishEditing(true)
                event.accepted = true
            }
        }
    }
}
