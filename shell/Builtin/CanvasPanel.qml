// The document running on its target: a frame of the preview's size with
// the preview's latest picture in it, and the state its breakpoint shows. Ctrl+wheel zooms; a click selects
// the object drawn there, as `node_select` with a point does for an agent,
// and CanvasOverlay moves and resizes it. A new frame is fitted to the panel
// until the person picks a zoom.
// In play mode a click is the app's own: `preview_state` clicks the
// running app there and reads the root's state, which the flow panel shows.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
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

    // Until the person zooms, the frame fits: a desktop frame in a panel
    // narrower than it would otherwise show only its corner.
    property bool fitted: true
    property real chosenZoom: 1
    readonly property real fit: preview === null ? 1
        : Math.min(1, (flick.width - 2 * Spacing.lg) / Math.max(1, preview.width),
                   (flick.height - 2 * Spacing.lg - frameLabels) / Math.max(1, preview.height))
    // The two lines above the frame.
    readonly property real frameLabels: 2 * (Typography.captionSize * 1.5 + Spacing.xxs)
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

    function zoomBy(notches) {
        chosenZoom = Math.min(8, Math.max(0.125, zoom * Math.pow(1.25, notches)))
        fitted = false
    }

    Flickable {
        id: flick
        objectName: "canvas"
        anchors.fill: parent
        // Below the buttons: the frame's own labels stay readable.
        anchors.topMargin: tools.visible ? tools.height + Spacing.xs : 0
        clip: true
        interactive: !panel.dragging
        contentWidth: frames.width + 2 * Spacing.lg
        contentHeight: frames.height + 2 * Spacing.lg

        WheelHandler {
            acceptedModifiers: Qt.ControlModifier
            onWheel: (event) => panel.zoomBy(event.angleDelta.y / 120)
        }

        Column {
            id: frames
            x: Spacing.lg
            y: Spacing.lg
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

                    Label {
                        objectName: "frameLabel"
                        text: frame.name + " · " + frame.modelData.width + " × " + frame.modelData.height
                        color: Colors.text
                        font.pixelSize: Typography.captionSize
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
                    // The state the frame's breakpoint turns on, as the
                    // file's `when` says.
                    Label {
                        objectName: "frameState"
                        text: frame.modelData.variant !== "" ? "State " + frame.modelData.variant : "No state"
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
            x: Spacing.lg
            y: Spacing.lg
            text: panel.workspace.previews ? "Open a document to preview it" : "Previews are off"
            color: Colors.text
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

    // Above the canvas, not in it, so it stays put as the frames scroll.
    Flow {
        id: tools
        x: Spacing.xs
        y: Spacing.xs
        width: flick.width - 2 * Spacing.xs
        layoutDirection: Qt.RightToLeft
        spacing: Spacing.xxs
        visible: panel.preview !== null

        Button {
            text: "Play"
            checkable: true
            checked: panel.playing
            Accessible.name: text
            onClicked: panel.playing = checked
        }
        Button {
            text: "Fit"
            Accessible.name: text
            onClicked: panel.fitted = true
        }
        Button {
            text: "100%"
            Accessible.name: "Actual size"
            onClicked: {
                panel.chosenZoom = 1
                panel.fitted = false
            }
        }
        Repeater {
            // A selection of two lines up; three or more can be evened out.
            model: panel.workspace.chosen.length === 0 || panel.playing || !panel.hasBoxes ? [] : [
                {label: "Left", align: "left"}, {label: "Centre", align: "hcenter"},
                {label: "Right", align: "right"}, {label: "Top", align: "top"},
                {label: "Middle", align: "vcenter"}, {label: "Bottom", align: "bottom"},
                {label: "Even across", align: "distribute_h", least: 3},
                {label: "Even down", align: "distribute_v", least: 3}
            ]
            delegate: Button {
                required property var modelData
                text: modelData.label
                Accessible.name: "Align " + modelData.label
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

    Label {
        objectName: "canvasHint"
        x: Spacing.xs
        y: parent.height - height - Spacing.xs
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
