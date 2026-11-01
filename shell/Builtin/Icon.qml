// An icon of `icons.js`, stroked in a role of the theme so it follows light,
// dark and high contrast. Decoration: the control holding it says the name.
import QtQuick
import QtQuick.Shapes
import Ljos.Design
import "icons.js" as Icons

Item {
    id: icon
    required property string name
    property color color: Colors.text
    // Paths are on a 16 pixel grid, so any other size scales them.
    property real size: 16
    implicitWidth: size
    implicitHeight: size
    Accessible.ignored: true

    Shape {
        width: 16
        height: 16
        transformOrigin: Item.TopLeft
        scale: icon.size / 16
        ShapePath {
            strokeColor: icon.color
            strokeWidth: 1.25
            fillColor: "transparent"
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathSvg {
                path: Icons.paths[icon.name] || ""
            }
        }
    }
}
