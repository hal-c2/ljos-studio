// What was refused in the row above it. It takes the gap between two rows
// and no more, whether it says anything or not, so a refusal moves nothing
// and the person's next click lands where they aimed. The words hang from
// the gap over the top of what follows, cut to a line and read whole as
// their name. What the refusal offers instead is a button as high as any
// other, centred on the gap: it reaches the middle of neither row, so each
// of those is still clicked where it is aimed at. Given a button's height,
// beside buttons, it holds both and hangs over nothing.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Design

Item {
    id: note
    property string text: ""
    // An offer's whole name, empty for none.
    property string bindSays: ""
    property string textSays: ""
    // What holds one lifts itself by this: the words lie over the next row.
    readonly property bool saying: text !== "" || bindSays !== "" || textSays !== ""
    signal bindAsked()
    signal textAsked()

    implicitHeight: Spacing.xxs
    z: saying ? 1 : 0

    component Offer: AbstractButton {
        id: offer
        hoverEnabled: true
        implicitWidth: Math.max(28, words.implicitWidth + 2 * Spacing.xs)
        implicitHeight: 28
        padding: 0
        contentItem: Text {
            id: words
            text: offer.text
            color: Colors.link
            font.pixelSize: Typography.captionSize
            font.weight: Typography.bodyStrongWeight
            font.underline: offer.hovered || offer.visualFocus
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            Accessible.ignored: true
        }
        background: Rectangle {
            radius: 6
            color: Colors.surfaceRaised
            border.width: offer.visualFocus ? 2 : 1
            border.color: offer.visualFocus ? Colors.focusRing : Colors.border
        }
    }

    Rectangle {
        visible: note.text !== ""
        width: note.width - (offers.width > 0 ? offers.width + Spacing.xxs : 0)
        height: 14
        radius: 3
        color: Colors.surface

        Label {
            anchors.fill: parent
            text: note.text
            color: Colors.danger
            font.pixelSize: Typography.captionSize
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            Accessible.role: Accessible.StaticText
            Accessible.name: note.text
        }
    }
    Row {
        id: offers
        visible: note.bindSays !== "" || note.textSays !== ""
        anchors.right: parent.right
        y: (note.height - 28) / 2
        width: visible ? implicitWidth : 0
        spacing: 2

        Offer {
            visible: note.bindSays !== ""
            text: "Bind"
            Accessible.name: note.bindSays
            onClicked: note.bindAsked()
        }
        Offer {
            visible: note.textSays !== ""
            text: "As text"
            Accessible.name: note.textSays
            onClicked: note.textAsked()
        }
    }
}
