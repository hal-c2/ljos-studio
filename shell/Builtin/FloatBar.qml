// A group of tools floating over the canvas, opaque so what is under it does
// not fight its icons. Given less room than its tools take side by side, it
// goes on in another line rather than past the canvas's edge.
import QtQuick
import Ljos.Design

Rectangle {
    id: bar
    default property alias content: tools.data
    // How wide it may be, edge to edge.
    property real maxWidth: Infinity
    // Its tools shown side by side, as wide as they are.
    readonly property real lineWidth: {
        let w = 0
        let n = 0
        for (let i = 0; i < tools.children.length; i++) {
            const c = tools.children[i]
            if (c.visible && c.implicitWidth > 0) {
                w += c.implicitWidth
                n += 1
            }
        }
        return w + Math.max(0, n - 1) * tools.spacing
    }
    implicitWidth: Math.max(28, Math.min(lineWidth, maxWidth - 2 * Spacing.xxs)) + 2 * Spacing.xxs
    implicitHeight: Math.max(28, tools.implicitHeight) + 2 * Spacing.xxs
    radius: 8
    color: Colors.surfaceRaised
    border.width: 1
    border.color: Colors.border

    Flow {
        id: tools
        x: Spacing.xxs
        y: Spacing.xxs
        width: bar.width - 2 * Spacing.xxs
        spacing: 2
    }
}
