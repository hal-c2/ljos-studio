import QtQuick
import Ljos.Design

// A link: on the web an `a` with this `href`, which the browser follows and
// a crawler or a screen reader reads as a link; in an app, a tap opens the
// address in the person's browser.
Text {
    id: link
    property url href
    text: "Link"
    color: Colors.link
    font.underline: hover.hovered
    Accessible.role: Accessible.Link
    Accessible.name: link.text
    HoverHandler {
        id: hover
        cursorShape: Qt.PointingHandCursor
    }
    TapHandler {
        // On the web the page's `a` is followed by the browser itself.
        enabled: Qt.platform.os !== "wasm"
        onTapped: Qt.openUrlExternally(link.href)
    }
}
