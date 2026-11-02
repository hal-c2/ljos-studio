import QtQuick
import Ljos.Design

Rectangle {
    id: quote
    property bool compact
    property real gutter

    width: parent.width
    height: body.height + 2 * Spacing.xxl
    color: Colors.surface

    Column {
        id: body
        anchors.centerIn: parent
        width: Math.min(parent.width - 2 * quote.gutter, 640)
        spacing: Spacing.md

        Text {
            width: parent.width
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            text: "“The fog came down on Fimmvörðuháls and the map just kept going. We walked out on the cairns it showed us.”"
            color: Colors.text
            font.pixelSize: quote.compact ? Typography.subtitleSize : Typography.titleSize
            font.italic: true
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: "Sigrún, hut warden"
            color: Colors.textMuted
            font.pixelSize: Typography.bodySize
        }
    }
}
