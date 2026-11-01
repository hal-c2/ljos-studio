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
    readonly property real zoom: fitted ? Math.max(0.125, fit) : chosenZoom
    property bool playing: false
    onPlayingChanged: workspace.followPicture(playing)
    // The state the inspector is recording into; null for none. What the
    // canvas sets goes there, as what the inspector sets does.
    property var recording: null
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

    // The middle of the view, in the view's own coordinates, unless the wheel
    // says otherwise: the point under the pointer stays under the pointer.
    function zoomBy(notches, atX, atY) {
        const x = atX === undefined ? flick.width / 2 : atX
        const y = atY === undefined ? flick.height / 2 : atY
        const before = zoom
        const onX = (flick.contentX + x - reach) / before
        const onY = (flick.contentY + y - reach) / before
        chosenZoom = Math.min(8, Math.max(0.125, zoom * Math.pow(1.25, notches)))
        fitted = false
        flick.contentX = onX * chosenZoom + reach - x
        flick.contentY = onY * chosenZoom + reach - y
    }

    function centre() {
        flick.contentX = reach + (frames.width - flick.width) / 2
        flick.contentY = reach + (frames.height - flick.height) / 2
    }
    onZoomChanged: if (fitted) Qt.callLater(centre)
    onReachChanged: Qt.callLater(centre)
    onFittedChanged: if (fitted) Qt.callLater(centre)
    onPreviewChanged: if (fitted) Qt.callLater(centre)

    // A step back from the panels and from the frame on it.
    fill: Colors.surfaceHover

    // Dots that travel with the canvas and grow with it, below the frame.

    // The middle button pans wherever it presses, over the frame too.
    MouseArea {
        anchors.fill: flick
        acceptedButtons: Qt.MiddleButton
        property real fromX: 0
        property real fromY: 0
        onPressed: (mouse) => {
            fromX = flick.contentX + mouse.x
            fromY = flick.contentY + mouse.y
        }
        onPositionChanged: (mouse) => {
            flick.contentX = fromX - mouse.x
            flick.contentY = fromY - mouse.y
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
        interactive: !panel.dragging
        boundsBehavior: Flickable.StopAtBounds
        contentWidth: frames.width + 2 * panel.reach
        contentHeight: frames.height + 2 * panel.reach

        WheelHandler {
            acceptedModifiers: Qt.ControlModifier
            onWheel: (event) => panel.zoomBy(event.angleDelta.y / 120, event.x, event.y)
        }

        Column {
            id: frames
            x: panel.reach
            y: panel.reach
            spacing: Spacing.xs

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
                            objectName: "overlay"
                            enabled: !panel.hand
                            anchors.fill: parent
                            workspace: panel.workspace
                            zoom: panel.zoom
                            playing: panel.playing
                            recording: panel.recording
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
        visible: alignments.count > 0

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
            checked: !panel.hand
            onClicked: panel.hand = false
        }
        IconButton {
            icon_: "hand"
            text: "Hand"
            checkable: true
            checked: panel.hand
            onClicked: panel.hand = true
        }
        IconButton {
            icon_: "play"
            text: "Play"
            checkable: true
            checked: panel.playing
            onClicked: panel.playing = checked
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
            onClicked: {
                panel.chosenZoom = 1
                panel.fitted = false
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
            onClicked: panel.fitted = true
        }
    }

    Label {
        objectName: "canvasHint"
        x: Spacing.xs
        y: parent.height - height - Spacing.md
        visible: text !== ""
        text: panel.says
        color: Colors.text
        font.pixelSize: Typography.captionSize
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    // Zoom is the panel's own: it changes no file and no agent needs it, since
    // `preview_screenshot` takes a size.
    Shortcut {
        sequence: "Ctrl+0"
        onActivated: panel.fitted = true
    }
    Shortcut {
        sequence: "Ctrl+1"
        onActivated: {
            panel.chosenZoom = 1
            panel.fitted = false
        }
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
