// The page's window: Studio's built-in shell (Builtin/Shell.qml, copied
// beside this file by `mise run web:shell`). The native window
// (shell/Window.qml) loads a person's shell through `Ljos.Blocks`, which
// ljos-web does not have yet, so a page shows the built-in one.
import QtQuick
import QtQuick.Controls
import Ljos.Design

ApplicationWindow {
    title: shell.title
    width: 1440
    height: 900
    visible: true
    color: Colors.canvas

    Shell {
        id: shell
        anchors.fill: parent
    }
}
