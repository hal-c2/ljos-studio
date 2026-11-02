//! A project's pictures and fonts, kept in `assets/`.
//!
//! They are not documents: no history holds them. Bringing one in writes a
//! name nothing uses yet, and taking one out is refused while a document uses
//! it, so neither can break what is on the canvas.

use std::path::Path;

use base64::Engine as _;
use ljos_mcp_core::{ErrorCode, SurfaceError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use studio_core::Store;

use crate::{Studio, place_tools::refused, store_error};

const DIR: &str = "assets";
/// Far above a picture a window shows; a mistaken path stops here, before
/// it is read into memory.
const MOST_BYTES: u64 = 64 * 1024 * 1024;
/// Where a file's pixel size is looked for. A JPEG with more metadata in
/// front of its size than this just lists no size.
const HEAD_BYTES: usize = 256 * 1024;
const TAKEN: &str = "png, jpg, webp, svg, gif, ttf, otf, woff2";

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AssetAction {
    Import,
    List,
    Remove,
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct AssetInfo {
    pub(crate) name: String,
    /// Relative to the project: what a document at its root writes.
    pub(crate) url: String,
    /// `image` or `font`.
    pub(crate) kind: &'static str,
    pub(crate) bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) height: Option<u32>,
}

fn kind_of(extension: &str) -> Option<&'static str> {
    match extension {
        "png" | "jpg" | "jpeg" | "webp" | "svg" | "gif" => Some("image"),
        "ttf" | "otf" | "woff2" => Some("font"),
        _ => None,
    }
}

fn extension_of(name: &str) -> Option<String> {
    let (_, extension) = name.rsplit_once('.')?;
    Some(extension.to_ascii_lowercase())
}

/// The file names in `assets/` of a kind it holds.
pub(crate) fn names(store: &dyn Store) -> Vec<(String, &'static str)> {
    let below = format!("{DIR}/");
    store
        .list_below(DIR, "")
        .into_iter()
        .filter_map(|path| {
            let name = path.strip_prefix(&below)?;
            // A file in a directory of its own is that directory's business.
            let kind = kind_of(&extension_of(name)?).filter(|_| !name.contains('/'))?;
            Some((name.to_owned(), kind))
        })
        .collect()
}

pub(crate) fn list(store: &dyn Store) -> Vec<AssetInfo> {
    names(store)
        .into_iter()
        .filter_map(|(name, kind)| {
            let url = format!("{DIR}/{name}");
            // A link that leads out is not read, and so not listed.
            let (bytes, head) = store.peek(&url, HEAD_BYTES).ok()?;
            let (width, height) = pixel_size(&name, &head).unzip();
            Some(AssetInfo {
                name,
                url,
                kind,
                bytes,
                width,
                height,
            })
        })
        .collect()
}

/// `file` (or `data`, base64) copied into `assets/` under a plain unused name.
pub(crate) fn import(
    studio: &Studio,
    file: Option<&str>,
    data: Option<&str>,
    name: Option<&str>,
) -> Result<AssetInfo, SurfaceError> {
    let (bytes, given) = match (file, data) {
        (Some(file), None) => {
            let bytes = read_source(studio, file)?;
            (bytes, name.unwrap_or(file).to_owned())
        }
        (None, Some(data)) => {
            let name = name.ok_or_else(|| {
                SurfaceError::invalid_argument("`data` needs a `name`".to_owned())
            })?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|e| SurfaceError::invalid_argument(format!("`data`: {e}")))?;
            (bytes, name.to_owned())
        }
        _ => {
            return Err(SurfaceError::invalid_argument(
                "`import` needs `file` or `data`, one of them".to_owned(),
            ));
        }
    };
    if bytes.len() as u64 > MOST_BYTES {
        return Err(refused(
            "asset_size",
            format!(
                "an asset is at most {MOST_BYTES} bytes; this is {}",
                bytes.len()
            ),
        ));
    }
    let plain = plain_name(&given);
    let extension = extension_of(&plain).unwrap_or_default();
    let Some(kind) = kind_of(&extension) else {
        return Err(refused(
            "asset_kind",
            format!("`{given}` is not an asset: {TAKEN} are taken"),
        ));
    };
    if !holds(&extension, &bytes) {
        return Err(refused(
            "asset_kind",
            format!("`{given}` does not hold what a .{extension} file holds"),
        ));
    }
    let store = studio.session.store();
    let name = unused(store, &plain);
    let url = format!("{DIR}/{name}");
    store.write_bytes(&url, &bytes).map_err(store_error)?;
    let (width, height) = pixel_size(&name, &bytes).unzip();
    Ok(AssetInfo {
        name,
        url,
        kind,
        bytes: bytes.len() as u64,
        width,
        height,
    })
}

/// Takes the asset out, and answers its url; refused while a document uses it.
pub(crate) fn remove(studio: &Studio, name: &str) -> Result<String, SurfaceError> {
    let bare = name.strip_prefix("assets/").unwrap_or(name);
    if bare.is_empty() || bare.contains(['/', '\\']) || bare.starts_with('.') {
        return Err(SurfaceError::new(
            ErrorCode::PathDenied,
            format!("`{name}` is not a file of `{DIR}/`"),
        ));
    }
    let url = format!("{DIR}/{bare}");
    let store = studio.session.store();
    if !store.exists(&url) {
        return Err(SurfaceError::not_found(format!("`{url}` does not exist")));
    }
    // No history holds assets: what is taken out by mistake stays out.
    if extension_of(bare).and_then(|e| kind_of(&e)).is_none() {
        return Err(refused(
            "asset_kind",
            format!("`{url}` is not an asset: {TAKEN} are taken out"),
        ));
    }
    // A script a document imports says urls as well as a document does.
    let mut files = studio.session.documents();
    files.extend(SCRIPTS.iter().flat_map(|extension| store.list(extension)));
    let users: Vec<String> = files
        .into_iter()
        .filter(|file| store.read(file).is_ok_and(|text| names_url(&text, &url)))
        .collect();
    if !users.is_empty() {
        return Err(refused(
            "asset_in_use",
            format!("`{url}` is used by {}", users.join(", ")),
        )
        .with_details(serde_json::json!({ "kind": "asset_in_use", "documents": users })));
    }
    store.remove(&url).map_err(store_error)?;
    Ok(url)
}

const SCRIPTS: [&str; 2] = ["js", "mjs"];

/// Whether `text` says `url` as a whole path, however many `../` lead to it.
/// A url a document builds at run time from parts is not seen here.
fn names_url(text: &str, url: &str) -> bool {
    let part = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.');
    text.match_indices(url).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(part)
            && !text[at + url.len()..].chars().next().is_some_and(part)
    })
}

/// A file named by a tool's caller: judged where it really is, below the
/// workspace, as a project is, and only a regular file, since reading a pipe
/// would hold the call forever. What is not there answers as what is out of
/// reach does, so a refusal does not tell what exists outside.
fn read_source(studio: &Studio, file: &str) -> Result<Vec<u8>, SurfaceError> {
    let denied = || {
        SurfaceError::new(
            ErrorCode::PathDenied,
            format!("`{file}` is not a file in the workspace; send it as `data` with a `name`"),
        )
    };
    let dir = studio.dir().ok_or_else(denied)?;
    let real = dir
        .join(Path::new(file))
        .canonicalize()
        .map_err(|_| denied())?;
    let workspace = studio.workspace.canonicalize().map_err(|_| denied())?;
    if !real.starts_with(&workspace) {
        return Err(denied());
    }
    let meta = std::fs::metadata(&real).map_err(|_| denied())?;
    if !meta.is_file() {
        return Err(refused(
            "asset_file",
            format!("`{file}` is not a regular file"),
        ));
    }
    if meta.len() > MOST_BYTES {
        return Err(refused(
            "asset_size",
            format!(
                "an asset is at most {MOST_BYTES} bytes; `{file}` is {}",
                meta.len()
            ),
        ));
    }
    std::fs::read(&real)
        .map_err(|e| SurfaceError::new(ErrorCode::Internal, format!("`{file}`: {e}")))
}

/// The file's own name in letters, digits, `.`, `_` and `-`: it goes into a
/// path and into QML strings, and leads nowhere.
fn plain_name(given: &str) -> String {
    let file = given.rsplit(['/', '\\']).next().unwrap_or_default();
    let (stem, extension) = file.rsplit_once('.').unwrap_or((file, ""));
    let mut plain = String::new();
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '.') {
            plain.push(c);
        } else if !plain.ends_with('-') {
            plain.push('-');
        }
    }
    let plain = plain.trim_matches(['-', '.']);
    let plain: String = plain.chars().take(100).collect();
    let stem = if plain.is_empty() { "asset" } else { &plain };
    if file.contains('.') {
        format!("{stem}.{}", extension.to_ascii_lowercase())
    } else {
        stem.to_owned()
    }
}

/// `name`, or with a number, so no two differ by case alone: on some file
/// systems they would be one file.
fn unused(store: &dyn Store, name: &str) -> String {
    let taken: Vec<String> = store
        .list_below(DIR, "")
        .iter()
        .map(|p| p.to_ascii_lowercase())
        .collect();
    let free = |n: &str| !taken.contains(&format!("{DIR}/{n}").to_ascii_lowercase());
    if free(name) {
        return name.to_owned();
    }
    let (stem, extension) = name.rsplit_once('.').unwrap_or((name, ""));
    (1u32..)
        .map(|n| format!("{stem}-{n}.{extension}"))
        .find(|n| free(n))
        .expect("some number is free")
}

/// Whether the bytes begin as such a file begins: an extension alone would
/// let anything in under a picture's name.
fn holds(extension: &str, bytes: &[u8]) -> bool {
    match extension {
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" | "jpeg" => bytes.starts_with(b"\xff\xd8\xff"),
        "gif" => bytes.starts_with(b"GIF8"),
        "webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"),
        "ttf" => bytes.starts_with(b"\0\x01\0\0") || bytes.starts_with(b"true"),
        "otf" => bytes.starts_with(b"OTTO"),
        "woff2" => bytes.starts_with(b"wOF2"),
        "svg" => std::str::from_utf8(bytes).is_ok_and(|t| t.contains("<svg")),
        _ => false,
    }
}

fn pixel_size(name: &str, head: &[u8]) -> Option<(u32, u32)> {
    let extension = extension_of(name)?;
    // A file dropped in by hand may not be what its name says; an SVG's own
    // reader looks for its tag, in a head that may end inside a character.
    if extension != "svg" && !holds(&extension, head) {
        return None;
    }
    match extension.as_str() {
        "png" => png_size(head),
        "jpg" | "jpeg" => jpeg_size(head),
        "gif" => gif_size(head),
        "webp" => webp_size(head),
        "svg" => svg_size(&String::from_utf8_lossy(head)),
        _ => None,
    }
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn be16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_be_bytes(
        b.get(at..at + 2)?.try_into().ok()?,
    )))
}
fn le16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_le_bytes(
        b.get(at..at + 2)?.try_into().ok()?,
    )))
}
fn le24(b: &[u8], at: usize) -> Option<u32> {
    let p = b.get(at..at + 3)?;
    Some(u32::from(p[0]) | u32::from(p[1]) << 8 | u32::from(p[2]) << 16)
}

fn png_size(b: &[u8]) -> Option<(u32, u32)> {
    Some((be32(b, 16)?, be32(b, 20)?))
}

fn gif_size(b: &[u8]) -> Option<(u32, u32)> {
    Some((le16(b, 6)?, le16(b, 8)?))
}

/// Segments up to the frame header, which says the size.
fn jpeg_size(b: &[u8]) -> Option<(u32, u32)> {
    let mut at = 2;
    loop {
        if *b.get(at)? != 0xff {
            return None;
        }
        let marker = *b.get(at + 1)?;
        // Padding before a marker.
        if marker == 0xff {
            at += 1;
            continue;
        }
        // Frame headers, without the DHT, JPG and DAC that share the range.
        if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            return Some((be16(b, at + 7)?, be16(b, at + 5)?));
        }
        at += 2 + be16(b, at + 2)? as usize;
    }
}

fn webp_size(b: &[u8]) -> Option<(u32, u32)> {
    match b.get(12..16)? {
        b"VP8 " => Some((le16(b, 26)? & 0x3fff, le16(b, 28)? & 0x3fff)),
        b"VP8L" => {
            let bits = u32::from_le_bytes(b.get(21..25)?.try_into().ok()?);
            Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
        }
        b"VP8X" => Some((le24(b, 24)? + 1, le24(b, 27)? + 1)),
        _ => None,
    }
}

/// `width` and `height` in plain numbers, else the `viewBox`: a size in
/// percent says nothing without the page around it.
fn svg_size(text: &str) -> Option<(u32, u32)> {
    let start = text.find("<svg")?;
    let tag = &text[start..];
    let tag = &tag[..tag.find('>')?];
    let attribute = |name: &str| {
        tag.match_indices(name).find_map(|(at, _)| {
            if !tag[..at].ends_with(char::is_whitespace) {
                return None;
            }
            let rest = tag[at + name.len()..]
                .trim_start()
                .strip_prefix('=')?
                .trim_start();
            let quote = rest.chars().next().filter(|c| matches!(c, '"' | '\''))?;
            let rest = &rest[1..];
            Some(&rest[..rest.find(quote)?])
        })
    };
    let pixels = |value: &str| {
        let n: f64 = value.trim().trim_end_matches("px").parse().ok()?;
        (n.is_finite() && n >= 0.0 && n <= f64::from(u32::MAX)).then(|| n.round() as u32)
    };
    if let (Some(w), Some(h)) = (attribute("width"), attribute("height"))
        && let (Some(w), Some(h)) = (pixels(w), pixels(h))
    {
        return Some((w, h));
    }
    let box_ = attribute("viewBox")?;
    let mut parts = box_.split([' ', ',']).filter(|p| !p.is_empty()).skip(2);
    Some((pixels(parts.next()?)?, pixels(parts.next()?)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_picture_kind_says_its_size() {
        let mut gif = b"GIF89a".to_vec();
        gif.extend([3, 0, 2, 0]);
        assert_eq!(pixel_size("a.gif", &gif), Some((3, 2)));

        // An APP0 segment before the frame header, which a reader has to skip.
        let jpeg = [
            0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 17, 8, 0, 5, 0, 7,
        ];
        assert_eq!(pixel_size("a.jpg", &jpeg), Some((7, 5)));

        let mut vp8x = b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0".to_vec();
        vp8x.extend([9, 0, 0, 4, 0, 0]);
        assert_eq!(pixel_size("a.webp", &vp8x), Some((10, 5)));

        let mut lossless = b"RIFF\0\0\0\0WEBPVP8L\0\0\0\0\x2f".to_vec();
        lossless.extend(((3u32) | (1u32 << 14)).to_le_bytes());
        assert_eq!(pixel_size("a.webp", &lossless), Some((4, 2)));
    }

    #[test]
    fn a_short_or_strange_file_has_no_size_and_does_not_panic() {
        for name in ["a.png", "a.jpg", "a.gif", "a.webp", "a.svg"] {
            assert_eq!(pixel_size(name, b""), None);
            assert_eq!(pixel_size(name, b"\xff\xd8RIFF\0\0\0\0WEBPVP8X"), None);
        }
        assert_eq!(
            pixel_size("a.svg", b"<svg width='100%' height='1em'>"),
            None
        );
    }

    #[test]
    fn an_svg_prefers_its_width_and_height_to_its_view_box() {
        assert_eq!(
            svg_size(r#"<svg viewBox="0 0 9 9" width="4px" height='3.4'>"#),
            Some((4, 3))
        );
        assert_eq!(
            svg_size(r#"<svg stroke-width="1" viewBox="0,0,9,8">"#),
            Some((9, 8))
        );
    }

    #[test]
    fn a_name_comes_out_plain() {
        assert_eq!(plain_name("C:\\pics\\A b.PNG"), "A-b.png");
        assert_eq!(plain_name("..."), "asset.");
        assert_eq!(plain_name("noextension"), "noextension");
    }
}
