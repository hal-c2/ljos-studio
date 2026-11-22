// What is wrong in the running preview: a count, and on a click the warnings
// and errors its last reload (or the app since) logged, each on the object
// written on its line. Choosing one shows its document and selects that
// object, the `node_select` an agent makes. The list is the workspace's
// `problems`, kept by Studio: it empties as each problem is fixed, so the
// panel keeps only whether the person opened it. It comes and goes as they
// type, so place it where that covers nothing and moves neither the code
// nor the frame. A skin may restyle or move it freely.
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

    // And why the open document doesn't parse, where no error names it: one
    // broken when it was opened has had no preview to say so, and an error
    // of no file says nothing of it.
    readonly property var problems: {
        const said = workspace.problems ?? []
        const doc = workspace.doc
        if (workspace.problem === "" || said.some(p => p.severity === "error" && p.file === doc))
            return said
        return said.concat([{severity: "error", message: workspace.problem, file: doc, line: 0, node: ""}])
    }
    readonly property int count: problems.length
    readonly property int errors: problems.filter(p => p.severity === "error").length
    // The person's: neither opened by a problem nor closed when the last goes.
    property bool open: false

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

    visible: count > 0
    padding: 1
    implicitWidth: header.implicitWidth + 2 * padding
    implicitHeight: column.implicitHeight + 2 * padding
    Accessible.role: Accessible.Pane
    Accessible.name: count === 0 ? "No problems in the preview" : count === 1 ? "1 problem in the preview" : count + " problems in the preview"
    background: Rectangle {
        radius: 8
        color: Colors.surfaceRaised
        border.width: 1
        border.color: Colors.border
    }

    ColumnLayout {
        id: column
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
            Accessible.name: panel.open ? "Hide the problems" : "Show the problems"
            background: Rectangle {
                radius: 7
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
                    Accessible.role: Accessible.StaticText
                    Accessible.name: "Problem count: " + panel.count
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
            // Less where the panel is given less, and it scrolls.
            Layout.fillHeight: true
            Layout.minimumHeight: 0
            Layout.preferredHeight: Math.min(contentHeight, panel.rows * 44)
            Layout.maximumHeight: Math.min(contentHeight, panel.rows * 44)
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
