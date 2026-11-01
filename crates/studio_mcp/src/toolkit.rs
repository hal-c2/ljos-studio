//! The toolkit's types a document can place: what the palette offers beside
//! the project's blocks and what `blocks_list` answers for a document.

use std::collections::HashMap;
use std::sync::OnceLock;

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

const OFFERS: &str = include_str!(concat!(env!("OUT_DIR"), "/offers.txt"));

#[derive(Default)]
struct Offers {
    /// A module and a type's name, to its class.
    exports: HashMap<&'static str, HashMap<&'static str, &'static str>>,
    /// A class, to its base and what it adds.
    classes: HashMap<&'static str, (&'static str, Vec<&'static str>)>,
}

fn table() -> &'static Offers {
    static TABLE: OnceLock<Offers> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = Offers::default();
        let mut class = "";
        for line in OFFERS.lines() {
            let mut parts = line.split('\t');
            match parts.next() {
                Some("E") => {
                    if let (Some(uri), Some(name), Some(of)) =
                        (parts.next(), parts.next(), parts.next())
                    {
                        table.exports.entry(uri).or_default().insert(name, of);
                    }
                }
                Some("C") => {
                    class = parts.next().unwrap_or("");
                    let base = parts.next().unwrap_or("");
                    table.classes.insert(class, (base, Vec::new()));
                }
                Some("P") => {
                    if let Some(entry) = table.classes.get_mut(class) {
                        entry.1.push(line);
                    }
                }
                _ => {}
            }
        }
        table
    })
}

/// What the type `name` of the module `uri` can be given, its own first.
pub(crate) fn offers(uri: &str, name: &str) -> Vec<crate::signals::Offer> {
    let table = table();
    let mut out = Vec::new();
    let Some(mut class) = table
        .exports
        .get(uri)
        .and_then(|names| names.get(name))
        .copied()
    else {
        return out;
    };
    while let Some((base, lines)) = table.classes.get(class) {
        for line in lines {
            let mut parts = line.split('\t').skip(1);
            let (Some(name), Some(ty)) = (parts.next(), parts.next()) else {
                continue;
            };
            out.push(crate::signals::Offer {
                name: name.to_owned(),
                ty: ty.to_owned(),
                values: parts
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect(),
            });
        }
        class = base;
    }
    out
}
