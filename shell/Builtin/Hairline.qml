// A line between groups of tools: decoration, so a quiet role rather than the
// 3:1 border a control's edge needs.
import QtQuick
import Ljos.Design

Rectangle {
    implicitWidth: 1
    implicitHeight: 20
    color: Colors.surfacePressed
    Accessible.ignored: true
}
