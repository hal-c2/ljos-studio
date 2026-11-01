// The document running on its target: a frame of the preview's size with
// the preview's latest picture in it, and the state its breakpoint shows. Ctrl+wheel zooms; a click selects
// the object drawn there, as `node_select` with a point does for an agent,
// and CanvasOverlay moves and resizes it. A new frame is fitted to the panel
// until the person picks a zoom. Round the frame the canvas goes on a window's
// worth in every direction: it pans by the wheel, the middle button or the hand.
// In play mode a click is the app's own: `preview_state` clicks the
// running app there and reads the root's state, which the flow panel shows.
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

    // Until the person zooms, the frame fits: a desktop frame in a panel
    // narrower than it would otherwise show only its corner.
    property bool fitted: true
    property real chosenZoom: 1
    readonly property real fit: preview === null ? 1
        : Math.min(1, (flick.width - 2 * Spacing.xxl) / Math.max(1, preview.width),
                   (flick.height - 2 * Spacing.xxl - frameLabels) / Math.max(1, preview.height))
    // The lines above the frame.
    readonly property real frameLabels: (held === "" ? 1 : 2) * (Typography.captionSize * 1.5 + Spacing.xxs)
    // How far the canvas goes on past the frame, each way.
    readonly property real reach: Math.max(flick.width, flick.height)
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
    readonly property real zoom: fitted ? Math.max(0.125, fit) : chosenZoom
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

    // Whether the person has moved the view since it was last centred: then
    // nothing but Fit takes it back.
    property bool panned: false
    // The view is being moved by this panel, not by the person.
    property bool placing: false
    function moveView(x, y) {
        placing = true
        flick.contentX = x
        flick.contentY = y
        placing = false
    }

    // The middle of the view, in the view's own coordinates, unless the wheel
    // says otherwise: the point under the pointer stays under the pointer. Only
    // the frame scales; the lines above it stay their size.
    function zoomTo(wanted, atX, atY) {
        const x = atX === undefined ? flick.width / 2 : atX
        const y = atY === undefined ? flick.height / 2 : atY
        const before = zoom
        const above = frames.height - (preview === null ? 0 : preview.height * before)
        const onX = (flick.contentX + x - reach) / before
        const onY = (flick.contentY + y - reach - above) / before
        chosenZoom = Math.min(8, Math.max(0.125, wanted))
        fitted = false
        moveView(onX * chosenZoom + reach - x, onY * chosenZoom + reach + above - y)
    }
    // From a centred frame the corner stays put, so a frame too big for the
    // view at its real size shows its start rather than its middle.
    function actualSize() {
        if (fitted && !panned) {
            const above = frames.height - (preview === null ? 0 : preview.height * zoom)
            zoomTo(1, panel.reach - flick.contentX, panel.reach + above - flick.contentY)
        } else {
            zoomTo(1)
        }
    }
    function zoomBy(notches, atX, atY) {
        zoomTo(zoom * Math.pow(1.25, notches), atX, atY)
    }

    function centre() {
        moveView(reach + (frames.width - flick.width) / 2, reach + (frames.height - flick.height) / 2)
    }
    function fitNow() {
        fitted = true
        panned = false
        centre()
    }
    function centreIfUntouched() {
        if (fitted && !panned)
            centre()
    }
    // A new picture is a new `preview` whose frame has not changed: only a
    // frame of another size is a reason to centre.
    readonly property string frameSize: preview === null ? "" : preview.width + "x" + preview.height
    onFrameSizeChanged: Qt.callLater(centreIfUntouched)
    onZoomChanged: Qt.callLater(centreIfUntouched)
    onFittedChanged: if (fitted) Qt.callLater(centreIfUntouched)
    // The canvas grew or shrank round the frame: what is in view stays there.
    property real reachWas: 0
    onReachChanged: {
        const by = reach - reachWas
        reachWas = reach
        if (fitted && !panned)
            Qt.callLater(centre)
        else
            moveView(flick.contentX + by, flick.contentY + by)
    }

    // A step back from the panels and from the frame on it.
    fill: Colors.surfaceHover

    // The middle button and the hand pan wherever they press, over the frame too.
    MouseArea {
        anchors.fill: flick
        acceptedButtons: panel.panning ? Qt.LeftButton | Qt.MiddleButton : Qt.MiddleButton
        cursorShape: panel.panning ? (pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor) : Qt.ArrowCursor
        property real fromX: 0
        property real fromY: 0
        onPressed: (mouse) => {
            fromX = flick.contentX + mouse.x
            fromY = flick.contentY + mouse.y
        }
        onPositionChanged: (mouse) => {
            panel.panned = true
            panel.moveView(fromX - mouse.x, fromY - mouse.y)
        }
    }

    // Lines that travel with the canvas and keep a useful spacing as it zooms.
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
        x: -(flick.contentX % step) - step
        y: -(flick.contentY % step) - step
        width: flick.width + 2 * step
        height: flick.height + 2 * step
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

    Flickable {
        id: flick
        objectName: "canvas"
        anchors.fill: parent
        clip: true
        // The hand pans in the area under this, as the middle button does.
        interactive: !panel.dragging && !panel.panning
        boundsBehavior: Flickable.StopAtBounds
        contentWidth: frames.width + 2 * panel.reach
        contentHeight: frames.height + 2 * panel.reach
        onContentXChanged: if (!panel.placing) panel.panned = true
        onContentYChanged: if (!panel.placing) panel.panned = true

        WheelHandler {
            acceptedModifiers: Qt.ControlModifier
            onWheel: (event) => panel.zoomBy(event.angleDelta.y / 120, event.x, event.y)
        }
        // A view that is not interactive takes no wheel of its own, and the
        // hand must not cost the wheel its pan.
        WheelHandler {
            enabled: panel.panning
            acceptedModifiers: Qt.NoModifier
            onWheel: (event) => {
                panel.panned = true
                panel.moveView(
                    Math.max(0, Math.min(flick.contentX - event.angleDelta.x, flick.contentWidth - flick.width)),
                    Math.max(0, Math.min(flick.contentY - event.angleDelta.y, flick.contentHeight - flick.height)))
            }
        }

        Column {
            id: frames
            x: panel.reach
            y: panel.reach
            spacing: Spacing.xs
            // The frame arrives after the preview that asks for it.
            onWidthChanged: panel.centreIfUntouched()
            onHeightChanged: panel.centreIfUntouched()

            // One frame per target the document runs on; a session runs
            // one preview at a time. A count, not the preview: `preview` is a
            // new value at every publish, and a new model would build the
            // frame again, taking the keyboard focus and a drag with it.
            Repeater {
                model: panel.preview !== null ? 1 : 0
                delegate: Column {
                    id: frame
                    readonly property var modelData: panel.preview !== null ? panel.preview
                        : ({target: "", width: 0, height: 0, variant: ""})
                    readonly property string name: modelData.target !== "" ? modelData.target : "Custom"
                    objectName: "frame " + name
                    spacing: Spacing.xxs

                    Row {
                        spacing: Spacing.xs
                        Label {
                            objectName: "frameLabel"
                            text: frame.name + " · " + frame.modelData.width + " × " + frame.modelData.height
                            color: Colors.text
                            font.pixelSize: Typography.captionSize
                            font.weight: Typography.bodyStrongWeight
                        }
                        // The state the frame's breakpoint turns on, as the
                        // file's `when` says.
                        Label {
                            objectName: "frameState"
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
                            visible: target !== "" && panel.recording === null
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
                    Rectangle {
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
        }

        Label {
            visible: panel.preview === null
            parent: panel
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
                onClicked: {
                    const args = {
                        action: "align",
                        doc: panel.workspace.doc,
                        nodes: panel.workspace.chosen.map(h => ({node: h})),
                        align: modelData.align
                    }
                    if (panel.recording !== null)
                        args.record = panel.recording
                    panel.workspace.dispatch("node_arrange", args)
                }
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
                onClicked: panel.workspace.dispatch("node_arrange", {
                    action: "wrap",
                    doc: panel.workspace.doc,
                    nodes: panel.workspace.chosen.map(h => ({node: h})),
                    in: modelData.kind
                })
            }
        }

        IconButton {
            id: unwrapping
            icon_: "unwrap"
            text: "Unwrap"
            visible: !panel.playing && panel.workspace.chosen.length === 1
                && panel.wrappers.indexOf(panel.chosenType) >= 0
            onClicked: panel.workspace.dispatch("node_arrange", {
                action: "unwrap",
                doc: panel.workspace.doc,
                nodes: [{node: panel.workspace.chosen[0]}]
            })
        }
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
                panel.hand = false
                panel.drawing = ""
                checked = Qt.binding(() => !panel.hand && panel.drawing === "")
            }
        }
        IconButton {
            icon_: "hand"
            text: "Hand"
            checkable: true
            checked: panel.hand
            onClicked: {
                panel.hand = true
                panel.drawing = ""
                checked = Qt.binding(() => panel.hand)
                canvasFocus.forceActiveFocus()
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
                    panel.hand = false
                    panel.workspace.playing = false
                    panel.drawing = panel.drawing === modelData.kind ? "" : modelData.kind
                    checked = Qt.binding(() => panel.drawing === modelData.kind)
                    canvasFocus.forceActiveFocus()
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
                panel.workspace.playing = !panel.playing
                panel.drawing = ""
                checked = Qt.binding(() => panel.playing)
            }
        }
    }

    FloatBar {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: Spacing.xs
        visible: panel.preview !== null

        IconButton {
            icon_: "minus"
            text: "Zoom out"
            onClicked: panel.zoomBy(-1)
        }
        Button {
            id: actual
            flat: true
            implicitHeight: 28
            implicitWidth: 48
            padding: 0
            text: Math.round(panel.zoom * 100) + "%"
            font.pixelSize: Typography.captionSize
            Accessible.name: "Actual size"
            onClicked: panel.actualSize()
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
        sequences: ["Ctrl+=", "Ctrl++"]
        onActivated: panel.zoomBy(1)
    }
    Shortcut {
        sequence: "Ctrl+-"
        onActivated: panel.zoomBy(-1)
    }
}
