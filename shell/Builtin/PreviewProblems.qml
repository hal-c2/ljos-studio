// What is wrong in the running preview: a count, and the warnings and
// errors its last reload (or the app since) logged, each on the object
// written on its line. Choosing one shows its document and selects that
// object, the `node_select` an agent makes. The list is the workspace's
// `problems`, kept by Studio: it empties as each problem is fixed, so the
// panel keeps no state about them. A skin may restyle or move it freely.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design
import Studio

Pane {
    id: panel
    required property Workspace workspace
    // How many rows show before the list scrolls.
    property int rows: 4

    readonly property var problems: workspace.problems ?? []
    readonly property int count: problems.length
    readonly property int errors: problems.filter(p => p.severity === "error").length
    property bool open: true

    function where(p) {
        if (p.file === undefined || p.file === "")
            return ""
        return p.line > 0 ? p.file + ":" + p.line : p.file
    }

    function go(p) {
        const doc = p.file ?? ""
        if (doc !== "" && doc !== workspace.doc && workspace.documents.indexOf(doc) >= 0)
            workspace.doc = doc
        if ((p.node ?? "") !== "" && doc !== "")
            workspace.dispatch("node_select", {doc: doc, nodes: [{node: p.node}]})
    }

    padding: 0
    Accessible.role: Accessible.Pane
    Accessible.name: count === 0 ? "No problems in the preview" : count === 1 ? "1 problem in the preview" : count + " problems in the preview"
    background: Rectangle {
        color: Colors.surface
        Rectangle {
            width: parent.width
            height: 1
            color: Colors.border
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        ItemDelegate {
            id: header
            Layout.fillWidth: true
            padding: Spacing.xs
            leftPadding: Spacing.sm
            rightPadding: Spacing.sm
            enabled: panel.count > 0
            onClicked: panel.open = !panel.open
            Accessible.name: panel.Accessible.name
            background: Rectangle {
                color: header.down ? Colors.surfacePressed : header.hovered ? Colors.surfaceHover : "transparent"
            }
            contentItem: RowLayout {
                spacing: Spacing.xs
                Label {
                    text: panel.count === 0 ? "No problems" : "Problems"
                    font.pixelSize: Typography.captionSize
                    font.weight: Typography.bodyStrongWeight
                    color: panel.count === 0 ? Colors.textMuted : Colors.text
                    Layout.fillWidth: true
                }
                // The count, coloured as the worst of them.
                Rectangle {
                    visible: panel.count > 0
                    radius: height / 2
                    color: panel.errors > 0 ? Colors.danger : Colors.warning
                    implicitHeight: badge.implicitHeight + Spacing.xxs * 2
                    implicitWidth: Math.max(implicitHeight, badge.implicitWidth + Spacing.xs * 2)
                    Label {
                        id: badge
                        anchors.centerIn: parent
                        text: panel.count
                        font.pixelSize: Typography.captionSize
                        font.weight: Typography.bodyStrongWeight
                        color: panel.errors > 0 ? Colors.textOnDanger : Colors.textOnWarning
                    }
                }
            }
        }

        ListView {
            id: list
            visible: panel.open && panel.count > 0
            clip: true
            model: panel.problems
            boundsBehavior: Flickable.StopAtBounds
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentHeight, panel.rows * 44)
            ScrollBar.vertical: ScrollBar {}

            delegate: ItemDelegate {
                id: row
                required property var modelData
                width: ListView.view.width
                padding: Spacing.xs
                leftPadding: Spacing.sm
                rightPadding: Spacing.sm
                enabled: (modelData.node ?? "") !== "" || (modelData.file ?? "") !== ""
                onClicked: panel.go(modelData)
                Accessible.name: modelData.severity + ": " + modelData.message + (panel.where(modelData) === "" ? "" : ", " + panel.where(modelData))
                ToolTip.visible: hovered && (modelData.node ?? "") !== ""
                ToolTip.text: "Go to " + modelData.node
                ToolTip.delay: 600
                background: Rectangle {
                    color: row.down ? Colors.surfacePressed : row.hovered ? Colors.surfaceHover : "transparent"
                }
                contentItem: RowLayout {
                    spacing: Spacing.xs
                    Rectangle {
                        implicitWidth: Spacing.xs
                        implicitHeight: Spacing.xs
                        radius: width / 2
                        color: row.modelData.severity === "error" ? Colors.danger : Colors.warning
                        Layout.alignment: Qt.AlignTop
                        Layout.topMargin: Spacing.xxs * 2
                    }
                    ColumnLayout {
                        spacing: 0
                        Layout.fillWidth: true
                        Label {
                            text: row.modelData.message
                            wrapMode: Text.Wrap
                            maximumLineCount: 2
                            elide: Text.ElideRight
                            font.pixelSize: Typography.captionSize
                            color: Colors.text
                            Layout.fillWidth: true
                        }
                        Label {
                            visible: text !== ""
                            text: panel.where(row.modelData)
                            elide: Text.ElideMiddle
                            font.pixelSize: Typography.captionSize
                            color: Colors.textMuted
                            Layout.fillWidth: true
                        }
                    }
                }
            }
        }
    }
}
