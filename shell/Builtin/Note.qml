// The line a field keeps under it for what was refused there. It is its
// height whether it says anything or not, so a refusal moves nothing below
// it and covers none of it: the person's next click lands where they aimed.
// The words are cut to the line and read whole as their name. What the
// refusal offers instead sits at the line's end as links in the sentence,
// named in full for a reader ("Bind width to cube") and short for the eye.
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

    // A caption's line: the rows keep this much apart, said or not.
    implicitHeight: 16

    // In the link's ink, as high as the line: an offer read as part of what
    // the refusal says, quieter than the field's own buttons.
    component Link: AbstractButton {
        id: link
        hoverEnabled: true
        implicitWidth: words.implicitWidth + 2 * Spacing.xxs
        implicitHeight: note.height
        padding: 0
        contentItem: Text {
            id: words
            text: link.text
            color: Colors.link
            font.pixelSize: Typography.captionSize
            font.weight: Typography.bodyStrongWeight
            font.underline: link.hovered || link.visualFocus
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            Accessible.ignored: true
        }
        background: Rectangle {
            radius: 3
            color: "transparent"
            border.width: link.visualFocus ? 2 : 0
            border.color: Colors.focusRing
        }
    }

    RowLayout {
        anchors.fill: parent
        spacing: Spacing.xxs

        Label {
            text: note.text
            color: Colors.danger
            font.pixelSize: Typography.captionSize
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            Layout.fillWidth: true
            Layout.fillHeight: true
            Accessible.role: Accessible.StaticText
            Accessible.name: note.text
            Accessible.ignored: note.text === ""
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
