// The line a field keeps under it for what was refused there. It is its
// height whether it says anything or not, so a refusal moves nothing below
// it, and the person's next click lands where they aimed. The words are cut
// to the line and read whole as its name; what the refusal offers instead
// sits at its end as links, named in full for a reader ("Bind width to
// cube") and short for the eye.
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
    signal bindAsked()
    signal textAsked()

    // A caption's line, and no more: the rows keep their pitch.
    implicitHeight: 14

    // Small, in the link's ink: an offer, quieter than the field's own buttons.
    component Link: AbstractButton {
        id: link
        hoverEnabled: true
        implicitWidth: words.implicitWidth
        implicitHeight: 14
        padding: 0
        contentItem: Text {
            id: words
            text: link.text
            color: Colors.link
            font.pixelSize: Typography.captionSize
            font.weight: Typography.bodyStrongWeight
            font.underline: link.hovered || link.visualFocus
            verticalAlignment: Text.AlignVCenter
            Accessible.ignored: true
        }
        background: null
    }

    RowLayout {
        anchors.fill: parent
        spacing: Spacing.xs

        Label {
            visible: note.text !== ""
            text: note.text
            color: Colors.danger
            font.pixelSize: Typography.captionSize
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            Layout.fillWidth: true
            Layout.fillHeight: true
            Accessible.role: Accessible.StaticText
            Accessible.name: note.text
        }
        Link {
            visible: note.bindSays !== ""
            text: "Bind"
            Accessible.name: note.bindSays
            onClicked: note.bindAsked()
        }
        Link {
            visible: note.textSays !== ""
            text: "As text"
            Accessible.name: note.textSays
            onClicked: note.textAsked()
        }
    }
}
