// The document running on its target: a frame of the preview's size with
// the preview's latest picture in it, and the state its breakpoint shows. A
// click selects the object drawn there, as `node_select` with a point does
// for an agent, and CanvasOverlay moves and resizes it. In play mode a click
// is the app's own: `preview_state` clicks the running app there and reads
// the root's state, which the flow panel shows.
//
// The canvas has no edges. The view is a point of it (`viewX`, `viewY`, in
// document pixels, at the view's top left) and a `zoom`: a document pixel
// at `p` is drawn at `(p - view) * zoom`. Only the frames scale; the lines
// above them, the handles and the bars keep their size on the screen. The
// wheel, Space and a drag, the middle button and the hand move the view;
// Ctrl or ⌘ with the wheel, and a pinch, zoom about the point under the
// hand and follow it. A zoom asked for by a key, a button or the zoom field
// glides there, keeping one point of the canvas still on the screen. A new
// frame is fitted to the view until the person moves or zooms it.
//
// The targets' frames can sit side by side. A session runs one preview, so
// one frame is live; the others show the last picture their target drew
// (`preview.kept`) and a click on one runs the document there.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Shapes
import Ljos.Design
import Studio

Panel {
    id: panel
    required property Workspace workspace
    // Whether the selection can be lined up; the overlay is a frame's, in a
    // delegate the toolbar cannot name.
    // How many frames' overlays say their selection has no box: each frame
    // has its own, and one changing does not speak for the others.
    property int framesWithoutBoxes: 0
    readonly property bool hasBoxes: framesWithoutBoxes === 0
    // What only holds its children can be taken away from around them.
    readonly property var wrappers: ["Row", "Column", "Grid", "Flow", "Item", "RowLayout", "ColumnLayout", "GridLayout"]
    readonly property string chosenType: {
        const all = workspace.layers
        for (let i = 0; i < all.length; i++) {
            if (all[i].node === workspace.selected)
                return all[i].type
        }
        return ""
    }
    title: "Canvas"
    clip: true

    // ------------------------------------------------------------ the view

    property real viewX: 0
    property real viewY: 0
    property real zoom: 1
    // 2 % shows a wall of frames; 6400 % a pixel as a tile to place by.
    readonly property real minZoom: 0.02
    readonly property real maxZoom: 64
    // Until the person moves or zooms, the view keeps `fitting` in sight
    // as the panel and the frames change size.
    property bool fitted: true
    // "live": the frame the preview runs in; "all": every frame shown.
    property string fitting: "live"
    // Whether a drag pans, whatever is under it, and a click selects nothing.
    property bool hand: false
    // In play a press is the app's, whichever tool waits for after.
    readonly property bool panning: hand && !playing
    // Space held is the hand for as long as it is held, then back to what
    // was in use and where the keys were.
    property bool handHeld: false
    property bool handWas: false
    property var keysWere: null
    // What a drag on the frame draws; "" selects and moves.
    property string drawing: ""
    readonly property bool playing: workspace.playing
    onPlayingChanged: workspace.followPicture(playing)
    // The state the inspector is recording into; null for none. What the
    // canvas sets goes there, as what the inspector sets does.
    property var recording: null
    signal layoutAsked(string target, string state)
    signal doneRecording()
    property bool dragging: false
    property string says: ""
    readonly property var preview: workspace.preview
    // The targets' frames beside the live one.
    property bool sideBySide: false
    // Who holds what of the document shown: the frame says so, as the
    // picture cannot mark an object it does not know the place of.
    readonly property string held: {
        const out = []
        const leases = workspace.leases
        const layers = workspace.layers
        for (let i = 0; i < leases.length; ++i) {
            if (leases[i].doc !== workspace.doc)
                continue
            let name = leases[i].node
            for (let j = 0; j < layers.length; ++j) {
                if (layers[j].node === leases[i].node)
                    name = layers[j].label
            }
            out.push(name + " held by " + leases[i].holder)
        }
        return out.join(", ")
    }
    // The lines above a frame, on the screen.
    readonly property real frameLabels: (held === "" ? 1 : 2) * (Typography.captionSize * 1.5 + Spacing.xxs) + Spacing.xxs

    // ----------------------------------------------------------- the frames

    // Studio's targets, as `preview_open` knows them; a preview without the
    // list (an older Studio) has the same three.
    readonly property var targets: preview !== null && preview.targets !== undefined ? preview.targets : [
        {name: "Desktop", width: 1280, height: 800},
        {name: "Tablet", width: 820, height: 1180},
        {name: "Phone", width: 390, height: 844}
    ]
    readonly property string liveName: preview === null ? "" : (preview.target !== "" ? preview.target : "Custom")
    // Room between frames side by side, in document pixels: enough for the
    // label of the next not to run into the one before at any zoom shown.
    readonly property real gap: 160
    // Where each frame sits on the canvas, left to right, tops in a line. A
    // string, which changes only when the arrangement does: `preview` is a
    // new value at every publish, and a new model would build the frames
    // again.
    readonly property string arrangement: {
        if (preview === null)
            return "[]"
        const out = []
        let x = 0
        const live = {name: liveName, width: preview.width, height: preview.height, live: true}
        if (!sideBySide) {
            out.push(Object.assign({x: 0}, live))
        } else {
            if (!targets.some(t => t.name === liveName))
                out.push(live)
            for (const t of targets)
                out.push(t.name === liveName ? live : {name: t.name, width: t.width, height: t.height, live: false})
            for (const f of out) {
                f.x = x
                x += f.width + gap
            }
        }
        return JSON.stringify(out)
    }
    readonly property var frames: JSON.parse(arrangement)
    readonly property var liveFrame: frames.find(f => f.live) || {name: "", x: 0, width: 0, height: 0}
    readonly property var besideFrames: JSON.parse(JSON.stringify(frames.filter(f => !f.live)))

    // ------------------------------------------------------------- motion

    // A glide from one view to another: the zoom changes at an even rate
    // of scale, and each axis keeps the point of the canvas still that is
    // in the same place on the screen at both ends. A zoom about a point
    // keeps that point; a fit lands where it has to.
    property real glideT: 0
    property real fromZoom: 1
    property real fromX: 0
    property real fromY: 0
    property real toZoom: 1
    property real toX: 0
    property real toY: 0
    NumberAnimation {
        id: glider
        target: panel
        property: "glideT"
        from: 0
        to: 1
        easing.type: Easing.BezierSpline
        easing.bezierCurve: Motion.reduced ? Motion.easeOutSoft : Motion.easeInOut
    }
    onGlideTChanged: {
        const z = fromZoom * Math.pow(toZoom / fromZoom, glideT)
        const d = 1 / fromZoom - 1 / toZoom
        const along = Math.abs(d) < 1e-9 ? glideT : (1 / fromZoom - 1 / z) / d
        zoom = z
        viewX = fromX + (toX - fromX) * along
        viewY = fromY + (toY - fromY) * along
    }
    // A step of zoom is met many times a day; a fit or a jump to 100 % now
    // and then. Less motion asked for is a shorter glide, never none: the
    // move is what says where the view went.
    readonly property int stepTime: Motion.reduced ? Motion.exitSmallFrequent : Motion.moveScreenFrequent
    readonly property int jumpTime: Motion.reduced ? Motion.exitSmallFrequent : Motion.moveScreen
    function glideTo(z, x, y, time) {
        glider.stop()
        if (time <= 0 || (Math.abs(z - zoom) < 1e-9 && Math.abs(x - viewX) < 1e-6 && Math.abs(y - viewY) < 1e-6)) {
            zoom = z
            viewX = x
            viewY = y
            toZoom = z
            return
        }
        fromZoom = zoom
        fromX = viewX
        fromY = viewY
        toZoom = z
        toX = x
        toY = y
        glider.duration = time
        glideT = 0
        glider.start()
    }
    // Where a glide under way is going, or the zoom now.
    readonly property real goalZoom: glider.running ? toZoom : zoom

    function clampZoom(z) {
        return Math.min(maxZoom, Math.max(minZoom, z))
    }
    // The point under (`atX`, `atY`) of the view, the middle unless said,
    // stays there. `time` 0 follows the hand.
    function zoomTo(wanted, atX, atY, time) {
        const sx = atX === undefined ? surface.width / 2 : atX
        const sy = atY === undefined ? surface.height / 2 : atY
        const z = clampZoom(wanted)
        const px = viewX + sx / zoom
        const py = viewY + sy / zoom
        fitted = false
        glideTo(z, px - sx / z, py - sy / z, time === undefined ? stepTime : time)
    }
    function zoomBy(notches, atX, atY, time) {
        zoomTo(goalZoom * Math.pow(1.25, notches), atX, atY, time)
    }
    function panBy(dx, dy) {
        glider.stop()
        fitted = false
        viewX -= dx / zoom
        viewY -= dy / zoom
    }
    // From a fitted frame its corner stays put, so a frame too big for the
    // view at its real size shows its start rather than its middle.
    function actualSize() {
        if (fitted && preview !== null)
            zoomTo(1, (liveFrame.x - viewX) * zoom, (0 - viewY) * zoom, jumpTime)
        else
            zoomTo(1, undefined, undefined, jumpTime)
    }

    // The box of document pixels `what` names on the canvas.
    function boxOf(what) {
        const shown = what === "all" ? frames : frames.filter(f => f.live)
        if (shown.length === 0)
            return null
        let x0 = Infinity, x1 = -Infinity, y1 = 0
        for (const f of shown) {
            x0 = Math.min(x0, f.x)
            x1 = Math.max(x1, f.x + f.width)
            y1 = Math.max(y1, f.height)
        }
        return {x: x0, y: 0, width: x1 - x0, height: y1}
    }
    // The whole of `what` in the view with its labels, no larger than its
    // real size: a phone's frame in a wide panel shows as a phone.
    function fit(what, time) {
        const box = boxOf(what)
        if (box === null || surface.width <= 0 || surface.height <= 0)
            return
        const room = Spacing.xxl
        const z = clampZoom(Math.min(1,
            (surface.width - 2 * room) / Math.max(1, box.width),
            (surface.height - 2 * room - frameLabels) / Math.max(1, box.height)))
        const x = box.x + box.width / 2 - surface.width / 2 / z
        const y = box.y - ((surface.height - frameLabels - box.height * z) / 2 + frameLabels) / z
        fitting = what
        glideTo(z, x, y, time)
        fitted = true
    }
    function fitNow() {
        fit("live", jumpTime)
    }
    function fitAll() {
        fit(sideBySide ? "all" : "live", jumpTime)
    }
    // A fitted view follows what it fits without moving by itself: a new
    // size is a new arrangement, not a move of the person's.
    readonly property string fitKey: arrangement + " " + surface.width + "x" + surface.height + " " + frameLabels
    onFitKeyChanged: Qt.callLater(refit)
    function refit() {
        if (fitted)
            fit(fitting, 0)
    }
    onSideBySideChanged: Qt.callLater(() => fit("all", jumpTime))

    // A step back from the panels and from the frame on it.
    fill: Colors.surfaceHover

    Item {
        id: surface
        objectName: "canvas"
        anchors.fill: parent
        clip: true

        // Lines that travel with the canvas and keep a useful spacing as it
        // zooms.
        Shape {
            id: grid
            readonly property real step: {
                let at = 48 * panel.zoom
                while (at < 24)
                    at *= 2
                while (at > 96)
                    at /= 2
                return at
            }
            readonly property real originX: -panel.viewX * panel.zoom
            readonly property real originY: -panel.viewY * panel.zoom
            x: ((originX % step) + step) % step - step
            y: ((originY % step) + step) % step - step
            width: surface.width + 2 * step
            height: surface.height + 2 * step
            Accessible.ignored: true
            ShapePath {
                strokeColor: Colors.surfacePressed
                strokeWidth: 1
                fillColor: "transparent"
                PathSvg {
                    path: {
                        let out = ""
                        for (let at = 0; at <= grid.width; at += grid.step)
                            out += "M" + at + " 0V" + grid.height
                        for (let at = 0; at <= grid.height; at += grid.step)
                            out += "M0 " + at + "H" + grid.width
                        return out
                    }
                }
            }
        }

        // A drag on the canvas around the frames, the middle button and the
        // hand pan wherever they press, over a frame too. Nothing here to
        // name for assistive technology: the keys move the view by Ctrl+0,
        // Ctrl+1, Shift+1 and the zoom field.
        MouseArea {
            anchors.fill: parent
            Accessible.ignored: true
            acceptedButtons: Qt.LeftButton | Qt.MiddleButton
            cursorShape: panel.panning || pressed ? (pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor) : Qt.ArrowCursor
            property real lastX: 0
            property real lastY: 0
            onPressed: (mouse) => {
                lastX = mouse.x
                lastY = mouse.y
            }
            onPositionChanged: (mouse) => {
                panel.panBy(mouse.x - lastX, mouse.y - lastY)
                lastX = mouse.x
                lastY = mouse.y
            }
        }

        // A notch of the wheel moves the view as far as a few lines of text;
        // a trackpad says its own distance.
        function wheelDelta(event, horizontal) {
            const px = event.pixelDelta
            const ang = event.angleDelta
            let dx = px.x !== 0 || px.y !== 0 ? px.x : ang.x / 120 * 64
            let dy = px.x !== 0 || px.y !== 0 ? px.y : ang.y / 120 * 64
            if (horizontal && dx === 0)
                return Qt.point(dy, 0)
            return Qt.point(dx, dy)
        }
        WheelHandler {
            acceptedModifiers: Qt.NoModifier
            onWheel: (event) => {
                const d = surface.wheelDelta(event, false)
                panel.panBy(d.x, d.y)
            }
        }
        WheelHandler {
            acceptedModifiers: Qt.ShiftModifier
            onWheel: (event) => {
                const d = surface.wheelDelta(event, true)
                panel.panBy(d.x, d.y)
            }
        }
        // Ctrl on Linux and Windows, ⌘ on a Mac. The zoom follows the wheel
        // as the hand turns it, a trackpad's small steps as smoothly.
        WheelHandler {
            acceptedModifiers: Qt.ControlModifier
            onWheel: (event) => panel.zoomBy(event.angleDelta.y / 120, event.x, event.y, 0)
        }
        WheelHandler {
            acceptedModifiers: Qt.MetaModifier
            onWheel: (event) => panel.zoomBy(event.angleDelta.y / 120, event.x, event.y, 0)
        }
        // Two fingers on a touch screen; a trackpad's pinch too, once the
        // window hands it on (CORE-GAPS: egui's Zoom reaches no handler).
        PinchHandler {
            target: null
            property real scaleWas: 1
            property point centreWas: Qt.point(0, 0)
            onActiveChanged: {
                scaleWas = 1
                centreWas = centroid.position
            }
            onActiveScaleChanged: {
                if (!active || activeScale <= 0)
                    return
                const at = centroid.position
                panel.panBy(at.x - centreWas.x, at.y - centreWas.y)
                panel.zoomTo(panel.zoom * activeScale / scaleWas, at.x, at.y, 0)
                scaleWas = activeScale
                centreWas = at
            }
        }

        // The frames the preview does not run in, side by side with it: the
        // picture their target last drew, and a click to run it there.
        Repeater {
            model: panel.besideFrames
            delegate: Item {
                id: beside
                required property var modelData
                readonly property var kept: panel.preview !== null && panel.preview.kept !== undefined
                    ? panel.preview.kept[modelData.name] : undefined
                objectName: "beside " + modelData.name
                x: (modelData.x - panel.viewX) * panel.zoom
                y: -panel.viewY * panel.zoom
                width: besideBody.width
                height: besideBody.height

                Item {
                    anchors.left: parent.left
                    anchors.bottom: besideBody.top
                    anchors.bottomMargin: Spacing.xxs
                    width: 1
                    height: Typography.captionSize * 1.5
                    Label {
                        objectName: "frameLabel"
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.min(implicitWidth, Math.max(Typography.captionSize, besideBody.width))
                        elide: Text.ElideRight
                        text: beside.modelData.name + " · " + beside.modelData.width + " × " + beside.modelData.height
                        color: Colors.textMuted
                        font.pixelSize: Typography.captionSize
                        font.weight: Typography.bodyStrongWeight
                        Accessible.role: Accessible.StaticText
                        Accessible.name: text
                    }
                }
                Rectangle {
                    id: besideBody
                    width: beside.modelData.width * panel.zoom
                    height: beside.modelData.height * panel.zoom
                    color: Colors.surface
                    border.color: pick.containsMouse ? Colors.focusRing : Colors.border
                    border.width: pick.containsMouse ? 2 : 1
                    Accessible.role: Accessible.Button
                    Accessible.name: "Show " + beside.modelData.name
                    Accessible.onPressAction: pick.show()
                    Rectangle {
                        x: 2
                        y: 4
                        z: -1
                        width: parent.width
                        height: parent.height
                        color: Colors.shadow
                        opacity: 0.25
                        Accessible.ignored: true
                    }
                    PreviewPicture {
                        id: keptPicture
                        objectName: "keptPicture"
                        objectName: "picture"
                        anchors.fill: parent
                        anchors.margins: 1
                        workspace: panel.workspace
                        target: beside.modelData.name
                        visible: beside.kept !== undefined
                        // Behind the document's latest: shown, but quieter.
                        opacity: beside.kept !== undefined && panel.preview !== null
                            && beside.kept < panel.workspace.generation ? 0.6 : 1
                    }
                    Label {
                        anchors.centerIn: parent
                        visible: beside.kept === undefined && parent.width >= 10 * Typography.captionSize
                            && parent.height >= 4 * Typography.captionSize
                        width: parent.width - 2 * Spacing.xs
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        text: "Click to run it on " + beside.modelData.name.toLowerCase()
                        color: Colors.textMuted
                        font.pixelSize: Typography.captionSize
                        Accessible.ignored: true
                    }
                    MouseArea {
                        id: pick
                        anchors.fill: parent
                        enabled: !panel.panning
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        function show() {
                            panel.workspace.dispatch("preview_open", {
                                doc: panel.workspace.doc,
                                target: beside.modelData.name.toLowerCase(),
                                state: panel.preview !== null ? panel.preview.state : ""
                            })
                        }
                        onClicked: show()
                    }
                }
            }
        }

        // The frame the preview runs in. One delegate for as long as there
        // is a preview: `preview` is a new value at every publish, and a new
        // model would build the frame again, taking the keyboard focus and a
        // drag with it.
        Repeater {
            model: panel.preview !== null ? 1 : 0
            delegate: Item {
                id: frame
                readonly property var modelData: panel.preview !== null ? panel.preview
                    : ({target: "", width: 0, height: 0, variant: ""})
                readonly property string name: panel.liveName
                objectName: "frame " + name
                x: (panel.liveFrame.x - panel.viewX) * panel.zoom
                y: -panel.viewY * panel.zoom
                width: body.width
                height: body.height

                // Above the frame at their own size, whatever the zoom.
                Column {
                    anchors.left: parent.left
                    anchors.bottom: body.top
                    anchors.bottomMargin: Spacing.xxs
                    spacing: Spacing.xxs
                    Row {
                        spacing: Spacing.xs
                        height: Typography.captionSize * 1.5
                        Label {
                            id: liveLabel
                            objectName: "frameLabel"
                            anchors.verticalCenter: parent.verticalCenter
                            // A frame zoomed out to a sliver names itself in
                            // as much as it has, not over its neighbour's.
                            width: Math.min(implicitWidth, Math.max(Typography.captionSize, frame.width))
                            elide: Text.ElideRight
                            text: frame.name + " · " + frame.modelData.width + " × " + frame.modelData.height
                            color: Colors.text
                            font.pixelSize: Typography.captionSize
                            font.weight: Typography.bodyStrongWeight
                            Accessible.role: Accessible.StaticText
                            Accessible.name: text
                        }
                        // The state the frame's breakpoint turns on, as the
                        // file's `when` says.
                        Label {
                            objectName: "frameState"
                            anchors.verticalCenter: parent.verticalCenter
                            visible: frame.width >= liveLabel.implicitWidth
                            text: frame.modelData.variant !== "" ? "State " + frame.modelData.variant : "No state"
                            color: Colors.textMuted
                            font.pixelSize: Typography.captionSize
                            Accessible.role: Accessible.StaticText
                            Accessible.name: text
                        }
                        // The state this frame's width turns on, to draw in; a
                        // layout for a width is that state, not a second file.
                        TextButton {
                            readonly property string target: frame.modelData.target.toLowerCase()
                            anchors.verticalCenter: parent.verticalCenter
                            visible: target !== "" && panel.recording === null && frame.width >= liveLabel.implicitWidth
                            text: frame.modelData.variant !== ""
                                  ? "Edit " + target + " layout"
                                  : target.charAt(0).toUpperCase() + target.slice(1) + " layout"
                            onClicked: panel.layoutAsked(target, frame.modelData.variant)
                        }
                    }
                    Label {
                        objectName: "frameHeld"
                        visible: panel.held !== ""
                        text: panel.held
                        color: Colors.text
                        font.pixelSize: Typography.captionSize
                        Accessible.role: Accessible.StaticText
                        Accessible.name: text
                    }
                }
                Rectangle {
                    id: body
                    width: frame.modelData.width * panel.zoom
                    height: frame.modelData.height * panel.zoom
                    color: Colors.surface
                    border.color: Colors.border
                    // Lifted off the canvas by its shadow.
                    Rectangle {
                        x: 2
                        y: 4
                        z: -1
                        width: parent.width
                        height: parent.height
                        color: Colors.shadow
                        opacity: 0.25
                        Accessible.ignored: true
                    }
                    Accessible.role: Accessible.Graphic
                    Accessible.name: frame.name

                    PreviewPicture {
                        objectName: "picture"
                        anchors.fill: parent
                        workspace: panel.workspace
                    }
                    CanvasOverlay {
                        onHasBoxesChanged: panel.framesWithoutBoxes += hasBoxes ? -1 : 1
                        Component.onDestruction: if (!hasBoxes) panel.framesWithoutBoxes -= 1
                        id: overlayHere
                        objectName: "overlay"
                        enabled: !panel.panning
                        anchors.fill: parent
                        workspace: panel.workspace
                        zoom: panel.zoom
                        playing: panel.playing
                        recording: panel.recording
                        drawing: panel.drawing
                        onDrawn: panel.drawing = ""
                        toolKey: (event) => panel.toolKey(event, overlayHere)
                        onDraggingChanged: panel.dragging = dragging
                        onSaysChanged: panel.says = says
                        Component.onCompleted: refetch()
                    }
                }
            }
        }

        Label {
            visible: panel.preview === null
            anchors.centerIn: parent
            text: panel.workspace.previews ? "Open a document to preview it" : "Previews are off"
            color: Colors.textMuted
        }
    }

    // Only after a click, and it stops by itself: an idle window asks for no
    // pictures.
    Timer {
        interval: 100
        repeat: true
        running: panel.workspace.settling
        onTriggered: panel.workspace.lookAgain()
    }

    // Above the canvas, not in it, so they stay put as the frame moves.
    FloatBar {
        anchors.horizontalCenter: parent.horizontalCenter
        y: Spacing.xs
        visible: alignments.count > 0 || wraps.count > 0 || unwrapping.visible

        Repeater {
            id: alignments
            // A selection of two lines up; three or more can be evened out.
            model: panel.workspace.chosen.length === 0 || panel.playing || !panel.hasBoxes ? [] : [
                {label: "Left", align: "left", icon: "alignLeft"},
                {label: "Centre", align: "hcenter", icon: "alignHCenter"},
                {label: "Right", align: "right", icon: "alignRight"},
                {label: "Top", align: "top", icon: "alignTop"},
                {label: "Middle", align: "vcenter", icon: "alignVCenter"},
                {label: "Bottom", align: "bottom", icon: "alignBottom"},
                {label: "Even across", align: "distribute_h", icon: "distributeH", least: 3},
                {label: "Even down", align: "distribute_v", icon: "distributeV", least: 3}
            ]
            delegate: IconButton {
                required property var modelData
                icon_: modelData.icon
                text: "Align " + modelData.label
                visible: panel.workspace.chosen.length >= (modelData.least || 1)
                onClicked: panel.arrange("align", {align: modelData.align})
            }
        }

        // A wrap needs no boxes: without them it goes by what the file says.
        Repeater {
            id: wraps
            model: panel.workspace.chosen.length === 0 || panel.playing ? [] : [
                {kind: "Row", icon: "row"},
                {kind: "Column", icon: "column"},
                {kind: "Grid", icon: "grid"},
                {kind: "Item", icon: "frame"}
            ]
            delegate: IconButton {
                required property var modelData
                icon_: modelData.icon
                text: "Wrap in " + modelData.kind
                onClicked: panel.arrange("wrap", {in: modelData.kind})
            }
        }

        IconButton {
            id: unwrapping
            icon_: "unwrap"
            text: "Unwrap"
            visible: !panel.playing && panel.workspace.chosen.length === 1
                && panel.wrappers.indexOf(panel.chosenType) >= 0
            onClicked: panel.arrange("unwrap", {})
        }
    }
    // `node_arrange` on the selection, as the bar above and the command
    // palette ask for it.
    function arrange(action, more) {
        const args = Object.assign({
            action: action,
            doc: workspace.doc,
            nodes: workspace.chosen.map(h => ({node: h}))
        }, more)
        if (action === "align" && recording !== null)
            args.record = recording
        workspace.dispatch("node_arrange", args)
    }

    // Here and not in the inspector: the inspector can be folded or behind a
    // tab, and a drag still goes into the state. This is where it takes effect.
    FloatBar {
        id: recordingBar
        objectName: "recordingBar"
        anchors.left: parent.left
        anchors.bottom: parent.bottom
        anchors.margins: Spacing.xs
        visible: panel.recording !== null

        Label {
            text: panel.recording !== null ? "Editing state " + panel.recording.state : ""
            color: Colors.text
            font.weight: Typography.bodyStrongWeight
            leftPadding: Spacing.xs
            anchors.verticalCenter: parent.verticalCenter
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
        TextButton {
            text: "Done"
            Accessible.name: "Done editing state"
            onClicked: panel.doneRecording()
        }
    }

    FloatBar {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Spacing.xs
        visible: panel.preview !== null

        IconButton {
            icon_: "select"
            text: "Select"
            checkable: true
            checked: !panel.hand && panel.drawing === ""
            // A click toggles a checkable button and ends its binding, and
            // clicking the tool in use is not putting it down.
            onClicked: {
                panel.useTool("select")
                checked = Qt.binding(() => !panel.hand && panel.drawing === "")
            }
        }
        IconButton {
            icon_: "hand"
            text: "Hand"
            checkable: true
            checked: panel.hand
            onClicked: {
                panel.useTool("hand")
                checked = Qt.binding(() => panel.hand)
            }
        }
        Hairline {}
        Repeater {
            model: [
                {label: "Draw rectangle", icon: "frame", kind: "Rectangle"},
                {label: "Draw text", icon: "text", kind: "Text"}
            ]
            delegate: IconButton {
                required property var modelData
                icon_: modelData.icon
                text: modelData.label
                checkable: true
                checked: panel.drawing === modelData.kind
                onClicked: {
                    panel.useTool(panel.drawing === modelData.kind ? "select" : modelData.kind)
                    checked = Qt.binding(() => panel.drawing === modelData.kind)
                }
            }
        }
        Hairline {}
        IconButton {
            icon_: "play"
            text: "Play"
            checkable: true
            checked: panel.playing
            onClicked: {
                panel.useTool("play")
                checked = Qt.binding(() => panel.playing)
            }
        }
    }
    // The tools by name, as the bar and the command palette pick them:
    // "select", "hand", "Rectangle", "Text" or "play" (which toggles).
    function useTool(tool) {
        if (tool === "play") {
            workspace.playing = !playing
            drawing = ""
            return
        }
        hand = tool === "hand"
        drawing = tool === "select" || tool === "hand" ? "" : tool
        if (drawing !== "")
            workspace.playing = false
        if (tool !== "select")
            canvasFocus.forceActiveFocus()
    }

    FloatBar {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: Spacing.xs
        visible: panel.preview !== null

        IconButton {
            icon_: "row"
            text: "Side by side"
            checkable: true
            checked: panel.sideBySide
            onClicked: {
                panel.sideBySide = !panel.sideBySide
                checked = Qt.binding(() => panel.sideBySide)
            }
        }
        Hairline {}
        IconButton {
            icon_: "minus"
            text: "Zoom out"
            onClicked: panel.zoomBy(-1)
        }
        // The zoom, and a place to type one: "250", "250%" or "2.5x".
        Field {
            id: zoomField
            objectName: "zoomField"
            implicitWidth: 64
            horizontalAlignment: TextInput.AlignHCenter
            Accessible.name: "Zoom"
            readonly property string shown: panel.zoomText(panel.goalZoom)
            text: shown
            onActiveFocusChanged: if (activeFocus) selectAll(); else text = Qt.binding(() => shown)
            onAccepted: {
                const typed = text.trim().toLowerCase()
                const n = parseFloat(typed)
                if (!isNaN(n) && n > 0)
                    panel.zoomTo(typed.endsWith("x") ? n : n / 100, undefined, undefined, panel.jumpTime)
                text = Qt.binding(() => shown)
                canvasFocus.forceActiveFocus()
            }
            Keys.onEscapePressed: {
                text = Qt.binding(() => shown)
                canvasFocus.forceActiveFocus()
            }
        }
        IconButton {
            icon_: "plus"
            text: "Zoom in"
            onClicked: panel.zoomBy(1)
        }
        IconButton {
            icon_: "fit"
            text: "Fit"
            onClicked: panel.fitNow()
        }
    }
    // Whole per cents but where they would all round the same: 2.5 % and 6400 %.
    function zoomText(z) {
        const p = z * 100
        return (p < 10 ? Math.round(p * 10) / 10 : Math.round(p)) + "%"
    }

    Label {
        objectName: "canvasHint"
        x: Spacing.xs
        // Above the bar when there is one: a drag's text comes just when the
        // bar saying where the drag goes matters.
        y: (recordingBar.visible ? recordingBar.y : parent.height - Spacing.md + Spacing.xs) - height - Spacing.xs
        visible: text !== ""
        text: panel.says
        color: Colors.text
        font.pixelSize: Typography.captionSize
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    // The tool is picked with the pointer on a button: Escape still puts it down.
    Item {
        id: canvasFocus
        Keys.onEscapePressed: panel.drawing = ""
        Keys.onPressed: (event) => {
            event.accepted = panel.toolKey(event, panel.keysWere)
        }
        Keys.onReleased: (event) => {
            if (event.key !== Qt.Key_Space || event.isAutoRepeat || !panel.handHeld)
                return
            panel.handHeld = false
            panel.hand = panel.handWas
            if (!panel.hand && panel.keysWere !== null)
                panel.keysWere.forceActiveFocus()
            event.accepted = true
        }
    }

    function toolKey(event, from) {
        if ((event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) !== 0)
            return false
        const tool = {[Qt.Key_V]: "", [Qt.Key_H]: "", [Qt.Key_R]: "Rectangle", [Qt.Key_T]: "Text"}[event.key]
        if (event.key === Qt.Key_Space) {
            if (handHeld || event.isAutoRepeat)
                return true
            handHeld = true
            handWas = hand
            keysWere = from
            hand = true
            // The overlay is disabled under the hand, and the key's release
            // has to reach someone.
            canvasFocus.forceActiveFocus()
            return true
        }
        if (tool === undefined)
            return false
        hand = event.key === Qt.Key_H
        // The tool is the person's choice now, and Space's release, which goes
        // where the keys are, has nothing to give back.
        handHeld = false
        drawing = tool
        workspace.playing = false
        // The overlay is disabled under the hand, and the next letter has to
        // reach someone; it takes the keys back when the hand goes.
        if (hand) {
            keysWere = from
            canvasFocus.forceActiveFocus()
        } else if (canvasFocus.activeFocus && keysWere !== null) {
            keysWere.forceActiveFocus()
        }
        return true
    }

    // Zoom is the panel's own: it changes no file and no agent needs it, since
    // `preview_screenshot` takes a size.
    Shortcut {
        sequence: "Ctrl+0"
        onActivated: panel.fitNow()
    }
    Shortcut {
        sequence: "Ctrl+1"
        onActivated: panel.actualSize()
    }
    Shortcut {
        // A US keyboard sends Shift+1 as "!" with Shift held, so both
        // spellings are listed.
        sequences: ["Shift+1", "Shift+!"]
        onActivated: panel.fitAll()
    }
    Shortcut {
        sequences: ["Ctrl+=", "Ctrl++"]
        onActivated: panel.zoomBy(1)
    }
    Shortcut {
        sequence: "Ctrl+-"
        onActivated: panel.zoomBy(-1)
    }
}
