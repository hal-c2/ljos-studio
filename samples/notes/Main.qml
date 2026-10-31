import QtQuick
import QtQuick.Controls
import Ljos.Design

Rectangle {
    id: root
    width: 390
    height: 760
    color: Colors.canvas

    // JsonListModel is read-only and loads after the first frame: the file
    // seeds a ListModel the screens edit once its rows are in.
    function copySeed() {
        notes.clear()
        for (let i = 0; i < seed.count; ++i)
            notes.append(seed.get(i))
    }

    EntryModel {
        id: seed
        onCountChanged: Qt.callLater(root.copySeed)
    }

    ListModel {
        id: notes
    }

    StackView {
        id: nav
        anchors.fill: parent
        initialItem: ListScreen {
            model: notes
            stack: nav
        }
        pushEnter: Transition {
            NumberAnimation { property: "x"; from: nav.width; to: 0; duration: Motion.enterScreen; easing.type: Easing.OutCubic }
        }
        pushExit: Transition {
            NumberAnimation { property: "opacity"; from: 1; to: 0; duration: Motion.exitScreen }
        }
        popEnter: Transition {
            NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Motion.enterScreen }
        }
        popExit: Transition {
            NumberAnimation { property: "x"; from: 0; to: nav.width; duration: Motion.exitScreen; easing.type: Easing.InCubic }
        }
    }

    Component.onCompleted: if (seed.count > 0) copySeed()
}
