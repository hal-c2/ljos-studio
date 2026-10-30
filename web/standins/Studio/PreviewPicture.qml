// What ljos-web's compiler knows of the native `Studio.PreviewPicture`
// (`crates/studio_blocks/ljostypes.json`). A native type that paints has
// no face on the web before ljos-web's W4, so a page shows this one: an
// empty item, as a preview page cannot take a picture yet either.
import QtQuick

Item {
    property Workspace workspace
}
