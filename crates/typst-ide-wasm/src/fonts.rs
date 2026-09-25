//! Font metadata as Typst's own parser sees it.
//!
//! A host that stores font files can record, at upload time, the family,
//! style, weight and stretch the compiler will match `text(font: ..)` against,
//! without re-implementing any of Typst's naming rules.

use typst::foundations::Bytes;
use typst::text::{Font, FontFlags, FontStyle};

/// The most faces read from a single file.
///
/// Guards against a collection header that claims billions of faces.
const MAX_FACES: u32 = 256;

/// One face of a font file.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontFace {
    /// The face's index in its file (0 unless the file is a collection).
    pub index: u32,
    /// The family name `text(font: ..)` matches.
    pub family: String,
    /// `"normal"`, `"italic"` or `"oblique"`.
    pub style: FontStyle,
    /// 100 (thin) to 900 (black).
    pub weight: u16,
    /// Width as a ratio of the normal width (1.0).
    pub stretch: f64,
    /// All glyphs have the same width.
    pub monospace: bool,
    /// Glyphs have serifs.
    pub serif: bool,
    /// The face has a math table.
    pub math: bool,
    /// The face is a variable font.
    pub variable: bool,
}

/// Reads every face of a font file (`.ttf`, `.otf`, or a `.ttc`/`.otc`
/// collection).
///
/// Returns an error when no face can be read — including for formats Typst
/// does not load, such as WOFF2.
pub fn font_faces(bytes: &[u8]) -> Result<Vec<FontFace>, String> {
    if let Some(count) = collection_count(bytes) {
        // Every face needs a 4-byte offset in the header, so a count the
        // header cannot hold is forged.
        if count > MAX_FACES || count as usize > bytes.len().saturating_sub(12) / 4 {
            return Err(format!("the font collection claims {count} faces"));
        }
    }

    let faces: Vec<FontFace> = Font::iter(Bytes::new(bytes.to_vec()))
        .take(MAX_FACES as usize)
        .map(|font| {
            let info = font.info();
            FontFace {
                index: font.index(),
                family: info.family.clone(),
                style: info.variant.style,
                weight: info.variant.weight.to_number(),
                stretch: info.variant.stretch.to_ratio().get(),
                monospace: info.flags.contains(FontFlags::MONOSPACE),
                serif: info.flags.contains(FontFlags::SERIF),
                math: info.flags.contains(FontFlags::MATH),
                variable: info.flags.contains(FontFlags::VARIABLE),
            }
        })
        .collect();

    if faces.is_empty() {
        return Err("no font face could be read from these bytes".into());
    }
    Ok(faces)
}

/// The face count a collection header declares, if `bytes` is a collection.
fn collection_count(bytes: &[u8]) -> Option<u32> {
    if !bytes.starts_with(b"ttcf") {
        return None;
    }
    let count = bytes.get(8..12)?;
    Some(u32::from_be_bytes(count.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Embedded fonts shipped with Typst: real files, no network.
    fn some_font() -> &'static [u8] {
        typst_assets::fonts().next().expect("typst-assets ships fonts")
    }

    /// Builds a TrueType collection from standalone font files.
    ///
    /// A collection is a `ttcf` header followed by the fonts; each font's
    /// table records hold absolute offsets, so they are shifted by where the
    /// font lands in the collection.
    fn collection(fonts: &[&[u8]]) -> Vec<u8> {
        let header_len = 12 + 4 * fonts.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"ttcf");
        out.extend_from_slice(&[0, 1, 0, 0]);
        out.extend_from_slice(&(fonts.len() as u32).to_be_bytes());
        let mut base = header_len;
        let mut bodies = Vec::new();
        for font in fonts {
            out.extend_from_slice(&(base as u32).to_be_bytes());
            let mut body = font.to_vec();
            let tables = u16::from_be_bytes([body[4], body[5]]) as usize;
            for i in 0..tables {
                let at = 12 + 16 * i + 8;
                let offset = u32::from_be_bytes(body[at..at + 4].try_into().unwrap());
                body[at..at + 4].copy_from_slice(&(offset + base as u32).to_be_bytes());
            }
            // Keep every font 4-byte aligned, as table offsets require.
            while body.len() % 4 != 0 {
                body.push(0);
            }
            base += body.len();
            bodies.push(body);
        }
        for body in bodies {
            out.extend_from_slice(&body);
        }
        out
    }

    #[test]
    fn a_font_file_yields_one_face_with_its_metadata() {
        let faces = font_faces(some_font()).expect("a font");
        assert_eq!(faces.len(), 1);
        let face = &faces[0];
        assert_eq!(face.index, 0);
        assert!(!face.family.is_empty());
        assert!((100..=900).contains(&face.weight));
        assert!(face.stretch > 0.4 && face.stretch < 2.1);
    }

    #[test]
    fn metadata_matches_what_the_compiler_sees() {
        use typst::foundations::Bytes;
        use typst::text::Font;
        let data = some_font();
        let font = Font::iter(Bytes::new(data.to_vec())).next().expect("font");
        let face = &font_faces(data).expect("a font")[0];
        assert_eq!(face.family, font.info().family);
        assert_eq!(face.weight, font.info().variant.weight.to_number());
    }

    #[test]
    fn a_collection_yields_every_face_with_its_index() {
        let fonts: Vec<&[u8]> = typst_assets::fonts().take(2).collect();
        let faces = font_faces(&collection(&fonts)).expect("a collection");
        assert_eq!(faces.len(), 2);
        assert_eq!(faces[0].index, 0);
        assert_eq!(faces[1].index, 1);
        assert_eq!(faces[0].family, font_faces(fonts[0]).unwrap()[0].family);
        assert_eq!(faces[1].family, font_faces(fonts[1]).unwrap()[0].family);
        // The first two bundled fonts share a family, so the weights are
        // what pins each face to its place in the file.
        assert_eq!(faces[0].weight, font_faces(fonts[0]).unwrap()[0].weight);
        assert_eq!(faces[1].weight, font_faces(fonts[1]).unwrap()[0].weight);
        assert_ne!(faces[0].weight, faces[1].weight);
    }

    #[test]
    fn serializes_to_the_documented_shape() {
        let json = serde_json::to_value(&font_faces(some_font()).unwrap()[0]).unwrap();
        let keys: Vec<&str> =
            json.as_object().unwrap().keys().map(String::as_str).collect();
        for key in [
            "index",
            "family",
            "style",
            "weight",
            "stretch",
            "monospace",
            "serif",
            "math",
            "variable",
        ] {
            assert!(keys.contains(&key), "missing {key} in {json}");
        }
        assert!(
            ["normal", "italic", "oblique"].contains(&json["style"].as_str().unwrap())
        );
    }

    #[test]
    fn bytes_that_are_not_a_font_are_an_error() {
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
        assert!(font_faces(&png).is_err());
        assert!(font_faces(&[]).is_err());
        assert!(font_faces(&some_font()[..64]).is_err());
    }

    #[test]
    fn a_forged_collection_header_is_rejected_quickly() {
        let mut forged = b"ttcf\0\x01\0\0".to_vec();
        forged.extend_from_slice(&u32::MAX.to_be_bytes());
        forged.extend_from_slice(&[0; 16]);
        let started = std::time::Instant::now();
        assert!(font_faces(&forged).is_err());
        assert!(started.elapsed() < std::time::Duration::from_millis(200));
    }
}
