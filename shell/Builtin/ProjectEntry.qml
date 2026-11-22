// A project opens on its entry: Main.qml, the document `ljos run` and the
// export panel start from. The documents panel opens a lone document itself;
// this covers a project of several. Only the window's first look: an agent
// that switches documents later has its own plans.
import QtQuick
import Studio

QtObject {
    id: entry
    required property Workspace workspace
    property bool looked: false

    function look() {
        const docs = workspace.documents || []
        if (looked || docs.length === 0)
            return
        looked = true
        if (workspace.doc !== "" || docs.length < 2 || docs.indexOf("Main.qml") < 0)
            return
        workspace.doc = "Main.qml"
        if (workspace.previews)
            workspace.dispatch("preview_open", {doc: "Main.qml"})
    }

    Component.onCompleted: look()

    property Connections watch: Connections {
        target: entry.workspace
        function onDocumentsChanged() { entry.look() }
    }
}
