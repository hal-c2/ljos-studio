// A line between groups of tools: decoration, so a quiet role rather than the
// 3:1 border a control's edge needs. Upright between tools by default; set
// `implicitHeight: 1` and fill the width for one across a panel.
import QtQuick
import Ljos.Design

Rectangle {
    implicitWidth: 1
    implicitHeight: 16
    color: Colors.surfacePressed
    Accessible.ignored: true
}
