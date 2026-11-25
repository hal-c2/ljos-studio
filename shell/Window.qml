// Studio's window: the person's own shell when they have one, Studio's
// (Builtin/Shell.qml) when they do not or theirs does not work. A banner
// says why theirs is not shown, or why their last edit of it did not
// apply, with the file and line. A folder is picked with the system's own
// dialog, and the window asks the shell before it closes over edits held.
import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
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

    // What the shell offers of files, when it is Studio's or a copy of it:
    // a person's own shell may have none of it.
    readonly property var shell: loader.item
    readonly property bool shellHasFiles: shell !== null && shell.askClose !== undefined
    // The shell agreed: what it held is saved or dropped.
    property bool closeAgreed: false

    onShellHasFilesChanged: if (shellHasFiles) shell.choosesFolders = true
    onClosing: (close) => {
        if (closeAgreed || !shellHasFiles || shell.unsaved.length === 0)
            return
        close.accepted = false
        shell.askClose()
    }
    Connections {
        target: window.shellHasFiles ? window.shell : null
        function onFolderAsked() { folders.open() }
        function onCloseAgreed() {
            window.closeAgreed = true
            window.close()
        }
    }

    // A file URL as the path it names: `file:///C:/x` is `C:/x`, and a
    // network folder's `file://server/share/x` is `//server/share/x`.
    function pathOf(url) {
        const text = url.toString()
        if (/^file:\/\/[^/]/.test(text))
            return decodeURIComponent("//" + text.slice("file://".length))
        const path = decodeURIComponent(text.replace(/^file:\/\//, ""))
        return /^\/[A-Za-z]:/.test(path) ? path.slice(1) : path
    }
    FolderDialog {
        id: folders
        title: "Open folder"
        onAccepted: window.shell.openFolder(window.pathOf(selectedFolder))
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
