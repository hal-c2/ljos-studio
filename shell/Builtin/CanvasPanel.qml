// The document running on its target: a frame of the preview's size with
// the preview's latest picture in it, and the state its breakpoint shows. Ctrl+wheel zooms; a click selects
// the object drawn there, as `node_select` with a point does for an agent.
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
    title: "Canvas"

    property real zoom: 1
    property bool playing: false
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
        zoom = Math.min(8, Math.max(0.125, zoom * Math.pow(1.25, notches)))
    }

    Flickable {
        id: flick
        objectName: "canvas"
        anchors.fill: parent
        clip: true
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
            // one preview at a time.
            Repeater {
                model: panel.preview !== null ? [panel.preview] : []
                delegate: Column {
                    id: frame
                    required property var modelData
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
                        TapHandler {
                            onTapped: (point) => {
                                const x = point.position.x / panel.zoom
                                const y = point.position.y / panel.zoom
                                if (panel.playing) {
                                    // The app is clicked at the very point, as a person's
                                    // pointer would be. The root's state says where the
                                    // flow is, when the preview can find the root: by its
                                    // `id`.
                                    const layers = panel.workspace.layers
                                    const args = {click_at: {x: x, y: y}}
                                    if (layers.length > 0 && layers[0].id !== "")
                                        args.node = {path: "root"}
                                    panel.workspace.dispatch("preview_state", args)
                                }
                                else {
                                    panel.workspace.dispatch("node_select", {
                                        doc: panel.workspace.doc,
                                        at: {x: Math.floor(x), y: Math.floor(y)}
                                    })
                                }
                            }
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

    // Over the canvas, not in it, so it stays put as the frames scroll.
    Button {
        x: flick.width - width - Spacing.xs
        y: Spacing.xs
        visible: panel.preview !== null
        text: "Play"
        checkable: true
        checked: panel.playing
        Accessible.name: text
        onClicked: panel.playing = checked
    }
}
