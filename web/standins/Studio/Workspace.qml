// What ljos-web's compiler (`qmlc --with`) knows of the native
// `Studio.Workspace` until it reads `ljostypes.json` (ljos-web's W1): its
// members as `crates/studio_blocks/ljostypes.json` has them. Nothing runs
// it; a page is given `web/Workspace.js` where this file is asked for.
import QtQml

QtObject {
    readonly property var documents
    property string doc
    readonly property real generation
    readonly property string text
    readonly property var layers
    readonly property string selected
    readonly property var selection
    readonly property var inspector
    readonly property var states
    readonly property var roles
    readonly property var signals
    readonly property var flow
    readonly property var palette
    readonly property var done
    readonly property int looks
    readonly property var undone
    readonly property var preview
    readonly property string error
    readonly property string problem
    readonly property bool previews
    readonly property string theme
    readonly property var tokens
    readonly property var comments
    readonly property var leases
    readonly property bool settling
    readonly property bool blank
    readonly property var chosen
    readonly property var toolkit
    readonly property var rootStates

    signal answered(string tool, var answer)
    signal refused(string tool, var error)

    function dispatch(tool: string, args: var) {
    }

    function follow(tool: string, args: var) {
    }

    function followPicture(on: bool) {
    }

    function lookAgain() {
    }
}
