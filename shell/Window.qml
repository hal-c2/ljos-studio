// Studio's window: the person's own shell when they have one, Studio's
// (Builtin/Shell.qml) when they do not or theirs does not work. A banner
// says why theirs is not shown, or why their last edit of it did not
// apply, with the file and line.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Ljos.Blocks
import Ljos.Design
import Studio

ApplicationWindow {
    id: window
    // The person's `Shell.qml`, which Studio's host sets; empty for none.
    property url userShell
    title: loader.item && loader.item.title ? loader.item.title : "Ljós Studio"
    width: 1440
    height: 900
    visible: true
    color: Colors.canvas

    // Why the last reload of the person's shell did not apply: only the
    // host that watches the files knows.
    ShellStatus {
        id: status
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Label {
            objectName: "shellBanner"
            text: loader.error !== "" ? loader.error : status.reloadError
            visible: text !== ""
            Accessible.role: Accessible.AlertMessage
            Accessible.name: "Shell error"
            Accessible.description: text
            color: Colors.textOnDanger
            wrapMode: Text.Wrap
            padding: Spacing.sm
            background: Rectangle {
                color: Colors.danger
            }
            Layout.fillWidth: true
        }

        ShellLoader {
            id: loader
            objectName: "shellLoader"
            source: window.userShell
            fallback: "Builtin/Shell.qml"
            Layout.fillWidth: true
            Layout.fillHeight: true
        }
    }
}
