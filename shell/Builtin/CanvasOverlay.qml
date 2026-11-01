// What Studio draws over the preview's picture: the selection, its handles,
// and a drag. A drag touches no file and asks nothing of the running app: it
// moves a ghost, so a frame costs a few rectangles. The drop is one call of
// the tool an agent has, `node_arrange`, which is one undo step.
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

    // `node_arrange` `geometry`'s answer for the selection.
    property var nodes: []
    property var parentBox: null
    property var siblings: []

    // "": at rest. "selecting": pressed, the selection under the pointer is
    // on its way. "pressed": could still be a click. "moving", "resizing".
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

    // Past this many screen pixels a press is a drag.
    readonly property real dragStart: 3
    // Within this many screen pixels an edge snaps.
    readonly property real snapReach: 6
    readonly property real handleSize: 8

    readonly property var selectedOne: nodes.length === 1 && nodes[0].window !== undefined ? nodes[0] : null
    readonly property bool resizable: selectedOne !== null && selectedOne.drag.resize !== "none"
    readonly property int handlesShown: resizable && !playing && ghost === null ? 8 : 0
    // What a drag does, said before and during it.
    readonly property string says: {
        if (workspace.chosen.length === 0 || playing)
            return ""
        if (nodes.length > 0 && nodes.every(n => n.window === undefined))
            return "The preview does not show this object"
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
            without_siblings: mode === ""
        })
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
            workspace.dispatch("node_arrange", {action: "place", doc: workspace.doc, places: places})
            return true
        }
        return selectedOne !== null && selectedOne.drag.move === "reorder" && dropReorder()
    }

    // A layout places the object: the drop picks its place among the others.
    function dropReorder() {
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
        workspace.dispatch("node_arrange", {action: "place", doc: workspace.doc, places: [place]})
        return true
    }

    function endDrag(wrote) {
        mode = ""
        guideX = -1
        guideY = -1
        dropping = wrote
        if (!wrote)
            ghost = null
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
        workspace.dispatch("node_arrange", {action: "place", doc: workspace.doc, places: places})
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
        dropWaits = false
        dropping = false
        if (playing)
            return
        pressedAt = Qt.point(mouse.x, mouse.y)
        const shift = (mouse.modifiers & Qt.ShiftModifier) !== 0
        handle = shift ? -1 : handleAt(mouse.x, mouse.y)
        if (handle >= 0) {
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
            hoverCursor = at >= 0 ? cursors[at] : (inside(mouse.x, mouse.y) ? Qt.SizeAllCursor : Qt.ArrowCursor)
            return
        }
        if (mode === "selecting")
            return
        const free = (mouse.modifiers & Qt.ControlModifier) !== 0
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
        if (mode === "moving")
            wrote = dropMove()
        else if (mode === "resizing")
            wrote = dropResize()
        else if (mode === "pressed")
            // A click inside the selection may mean what is under it.
            selectAt(mouse.x, mouse.y, false)
        endDrag(wrote)
    }

    onCanceled: endDrag(false)

    Keys.onPressed: (event) => {
        // While playing, keys are the running prototype's, and no outline
        // shows what an edit would hit.
        if (playing) {
            event.accepted = false
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
            if (key !== overlay.seenChosen) {
                overlay.seenChosen = key
                overlay.writing = false
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
            if (tool === "node_select" && overlay.mode === "selecting") {
                overlay.refetch()
            } else if (tool === "node_move") {
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
                            // An agent may have emptied the selection meanwhile.
                            if (overlay.selectedOne === null || !overlay.dropReorder()) {
                                overlay.dropping = false
                                overlay.ghost = null
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
                    // The hand may be ahead of the siblings it snaps to.
                    if (overlay.mode === "moving" || overlay.mode === "resizing")
                        overlay.startSnaps()
                    if (overlay.mode === "selecting") {
                        overlay.startBox = overlay.around()
                        if (overlay.startBox !== null)
                            overlay.mode = "pressed"
                    }
                } else if (overlay.writing) {
                    settled.restart()
                } else {
                    // The read asked now is the first that can say the new box.
                    overlay.dropping = false
                    overlay.refetch()
                }
            }
        }
        function onRefused(tool, error) {
            if (tool === "node_move") {
                overlay.refetch()
            } else if (tool === "node_arrange") {
                // A refused read leaves nothing to draw; a refused drop goes
                // back to what the preview says.
                overlay.writing = false
                overlay.dropping = false
                overlay.ghost = null
                // Read again once only: when the read is what is refused
                // (a preview gone, or one that cannot say), asking on would
                // keep an idle window busy for ever.
                overlay.reads = Math.max(0, overlay.reads - 1)
                overlay.dropWaits = false
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
    // Geometry reads asked and not answered; they answer in order.
    property int reads: 0
    property bool dropWaits: false
    property int seenGeneration: -1
    property string seenChosen: "[]"

    // One outline per object; no drawing while nothing is selected.
    Repeater {
        model: overlay.playing ? [] : overlay.nodes
        delegate: Rectangle {
            required property var modelData
            objectName: "selectionOutline"
            visible: modelData.window !== undefined
            x: visible ? modelData.window.x * overlay.zoom : 0
            y: visible ? modelData.window.y * overlay.zoom : 0
            width: visible ? modelData.window.width * overlay.zoom : 0
            height: visible ? modelData.window.height * overlay.zoom : 0
            color: "transparent"
            border.color: Colors.primary
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
            border.color: Colors.primary
            border.width: 1
        }
    }

    Rectangle {
        objectName: "dragGhost"
        visible: overlay.ghost !== null
        x: visible ? overlay.ghost.x * overlay.zoom : 0
        y: visible ? overlay.ghost.y * overlay.zoom : 0
        width: visible ? overlay.ghost.width * overlay.zoom : 0
        height: visible ? overlay.ghost.height * overlay.zoom : 0
        color: "transparent"
        border.color: Colors.primary
        border.width: 2

        Label {
            objectName: "dragSize"
            visible: overlay.mode === "resizing"
            y: parent.height + Spacing.xxs
            text: overlay.ghost !== null ? Math.round(overlay.ghost.width) + " × " + Math.round(overlay.ghost.height) : ""
            color: Colors.primary
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
}
