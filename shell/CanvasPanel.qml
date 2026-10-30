// The document running on its target: a frame of the preview's size with
// the preview's latest picture in it. Ctrl+wheel zooms; a click selects
// the object drawn there, as `node_select` with a point does for an agent.
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
    readonly property var preview: workspace.preview

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
                            onTapped: (point) => panel.workspace.dispatch("node_select", {
                                doc: panel.workspace.doc,
                                at: {
                                    x: Math.floor(point.position.x / panel.zoom),
                                    y: Math.floor(point.position.y / panel.zoom)
                                }
                            })
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
}
