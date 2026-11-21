// Studio's own shell: panels that skin the one block Studio binds,
// Studio.Workspace. Every gesture is a `dispatch` of a Studio tool, the
// call an agent makes over MCP. The window (../Window.qml) shows it when
// the person has no shell of their own, or when theirs does not work; a
// person's shell starts as a copy of this directory.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Item {
    id: shell
    // What the window's title says.
    readonly property string title: workspace.doc === "" ? "Ljós Studio" : workspace.doc + " · Ljós Studio"

    Workspace {
        id: workspace
    }

    Shortcut {
        sequences: [StandardKey.Undo]
        onActivated: workspace.dispatch("history", {action: "undo"})
    }
    Shortcut {
        sequences: [StandardKey.Redo]
        onActivated: workspace.dispatch("history", {action: "redo"})
    }

    Shortcut {
        sequence: "Ctrl+E"
        onActivated: exporter.show()
    }

    ExportPanel {
        id: exporter
        workspace: workspace
    }
    ProjectEntry {
        workspace: workspace
    }

    // The canvas's ground, under the panels that float on it.
    Rectangle {
        anchors.fill: parent
        color: Colors.canvas
    }

    Shortcut {
        sequences: ["Ctrl+K", "Meta+K", "Ctrl+Shift+P"]
        onActivated: commands.show()
    }
    CommandPalette {
        id: commands
        workspace: workspace
        canvas: canvas
        toolbar: toolbar
        exporter: exporter
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Toolbar {
            id: toolbar
            workspace: workspace
            problems: previewProblems.count
            onExportRequested: exporter.show()
            Layout.fillWidth: true
        }

        SplitView {
            orientation: Qt.Horizontal
            // A gutter of the canvas's own ground: the panels float beside it.
            handle: Item {
                implicitWidth: Spacing.xs
                Rectangle {
                    anchors.centerIn: parent
                    width: 2
                    height: parent.height / 3
                    radius: 1
                    color: Colors.border
                    opacity: SplitHandle.hovered || SplitHandle.pressed ? 1 : 0
                }
            }
            Layout.fillWidth: true
            Layout.fillHeight: true

            // What the document is made of, and what can go in it.
            Item {
                visible: toolbar.leftOpen
                SplitView.preferredWidth: 268
                SplitView.minimumWidth: 208

                Rectangle {
                    id: leftCard
                    anchors.fill: parent
                    anchors.margins: Spacing.xs
                    anchors.rightMargin: 0
                    radius: 12
                    color: Colors.surface
                    border.width: 1
                    border.color: Colors.surfacePressed
                }
                ColumnLayout {
                    anchors.fill: leftCard
                    anchors.margins: 1
                    spacing: 0

                    TabStrip {
                        id: leftTabs
                        tabs: ["Layers", "Insert"]
                        Layout.fillWidth: true
                    }
                    ColumnLayout {
                        visible: leftTabs.current === 0
                        spacing: 0
                        Layout.fillWidth: true
                        Layout.fillHeight: true

                        Section {
                            title: "Documents"
                            ruled: false
                            inset: Spacing.xxs
                            Layout.fillWidth: true
                            DocumentsPanel {
                                id: documentsPanel
                                workspace: workspace
                                Layout.fillWidth: true
                                Layout.preferredHeight: documentsPanel.wants
                                Layout.minimumHeight: documentsPanel.least
                            }
                        }
                        // Beside the documents: what the open one offers when it is a block.
                        Section {
                            title: "Block"
                            visible: blockActions.block !== null
                            inset: Spacing.xxs
                            Layout.fillWidth: true
                            BlockActions {
                                id: blockActions
                                workspace: workspace
                                Layout.fillWidth: true
                            }
                        }
                        Section {
                            id: layersSection
                            title: "Layers"
                            inset: Spacing.xxs
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            LayersPanel {
                                workspace: workspace
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                            }
                        }
                        // A closed Layers leaves its room empty below, so the
                        // card keeps its tabs at the top.
                        Item {
                            visible: !layersSection.open
                            Layout.fillHeight: true
                        }
                    }
                    PalettePanel {
                        visible: leftTabs.current === 1
                        workspace: workspace
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                    }
                    // Under whichever fills: its room comes out of that, so it
                    // covers nothing and the code and the frame stay put.
                    PreviewProblems {
                        id: previewProblems
                        workspace: workspace
                        Layout.fillWidth: true
                        Layout.margins: Spacing.xs
                    }
                }
            }

            SplitView {
                orientation: Qt.Vertical
                SplitView.fillWidth: true
                handle: Item {
                    implicitHeight: Spacing.xs
                    Rectangle {
                        anchors.centerIn: parent
                        width: parent.width / 3
                        height: 2
                        radius: 1
                        color: Colors.border
                        opacity: SplitHandle.hovered || SplitHandle.pressed ? 1 : 0
                    }
                }

                CanvasPanel {
                    id: canvas
                    workspace: workspace
                    recording: inspector.recording
                    // The window's own ground, so the gutters round the
                    // panels are the canvas going on behind them.
                    fill: Colors.canvas
                    onDoneRecording: inspector.pick("", "")
                    onLayoutAsked: (target, state) => inspector.editLayout(target, state)
                    SplitView.fillHeight: true
                }
                Item {
                    visible: toolbar.codeOpen
                    SplitView.preferredHeight: 188
                    SplitView.minimumHeight: 88

                    Rectangle {
                        id: codeCard
                        anchors.fill: parent
                        anchors.bottomMargin: Spacing.xs
                        radius: 12
                        color: Colors.surface
                        border.width: 1
                        border.color: Colors.surfacePressed
                    }
                    CodePanel {
                        workspace: workspace
                        anchors.fill: codeCard
                        anchors.margins: 1
                    }
                }
            }

            // What the selection is, and what the document's look and flow are.
            Item {
                visible: toolbar.rightOpen
                SplitView.preferredWidth: 300
                SplitView.minimumWidth: 240

                Rectangle {
                    id: rightCard
                    anchors.fill: parent
                    anchors.margins: Spacing.xs
                    anchors.leftMargin: 0
                    radius: 12
                    color: Colors.surface
                    border.width: 1
                    border.color: Colors.surfacePressed
                }
                ColumnLayout {
                    anchors.fill: rightCard
                    anchors.margins: 1
                    spacing: 0

                    TabStrip {
                        id: rightTabs
                        tabs: ["Design", "Tokens", "Flow"]
                        Layout.fillWidth: true
                    }
                    InspectorPanel {
                        id: inspector
                        visible: rightTabs.current === 0
                        workspace: workspace
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                    }
                    TokensPanel {
                        visible: rightTabs.current === 1
                        workspace: workspace
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                    }
                    FlowPanel {
                        visible: rightTabs.current === 2
                        workspace: workspace
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                    }
                }
            }
        }

        // The person's own last refusal, until the next gesture is answered.
        // Text that doesn't parse is the problems' to say: as a row here it
        // would move the code view on every half-typed word.
        Label {
            objectName: "status"
            Accessible.role: Accessible.StatusBar
            Accessible.name: "Status"
            padding: Spacing.xs
            leftPadding: Spacing.xs + 14 + Spacing.xs
            text: workspace.error
            color: Colors.text
            font.pixelSize: Typography.captionSize
            elide: Text.ElideRight
            // A slim bar on the canvas's ground, its mark saying it is a
            // refusal and not news.
            background: Rectangle {
                color: Colors.surface
                Rectangle {
                    width: parent.width
                    height: 1
                    color: Colors.surfacePressed
                }
                Icon {
                    x: Spacing.xs
                    anchors.verticalCenter: parent.verticalCenter
                    name: "close"
                    size: 14
                    color: Colors.danger
                }
            }
            visible: text !== ""
            Layout.fillWidth: true
        }
    }
}
