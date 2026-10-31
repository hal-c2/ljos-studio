//! The toolkit's types a document can place: what the palette offers beside
//! the project's blocks and what `blocks_list` answers for a document.

use schemars::JsonSchema;
use serde::Serialize;
use studio_core::Import;

const TEXT: &str = include_str!(concat!(env!("OUT_DIR"), "/toolkit.txt"));

/// Offered whatever the document imports: most objects come from these, and
/// `node_insert` adds the import of the one placed.
const ALWAYS: [&str; 5] = [
    "QtQuick",
    "QtQuick.Layouts",
    "QtQuick.Controls",
    "QtQml",
    "QtQuick3D",
];

/// One module's types.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct Group {
    pub module: String,
    /// The document imports it; otherwise placing a type adds the import.
    pub imported: bool,
    pub types: Vec<String>,
}

fn names(uri: &str) -> Option<Vec<String>> {
    TEXT.lines().find_map(|line| {
        let mut words = line.split(' ');
        (words.next() == Some(uri)).then(|| words.map(str::to_owned).collect())
    })
}

/// The modules `source` imports and those always offered, imported ones
/// first as it wrote them, keeping the types whose name holds `search`
/// (any case). A module left with no type is not listed. A document that
/// does not parse has no imports to read.
pub(crate) fn offered(source: &str, search: &str) -> Vec<Group> {
    let imported: Vec<String> = studio_core::outline::imports(source)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| match i {
            Import::Module { uri, .. } => Some(uri),
            Import::Path { .. } => None,
        })
        .collect();
    let search = search.to_lowercase();
    let mut uris: Vec<&str> = Vec::new();
    for uri in imported.iter().map(String::as_str).chain(ALWAYS) {
        if !uris.contains(&uri) {
            uris.push(uri);
        }
    }
    uris.into_iter()
        .filter_map(|uri| {
            let mut types = names(uri)?;
            types.retain(|t| t.to_lowercase().contains(&search));
            (!types.is_empty()).then(|| Group {
                module: uri.to_owned(),
                imported: imported.iter().any(|i| i == uri),
                types,
            })
        })
        .collect()
}
