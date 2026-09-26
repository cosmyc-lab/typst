//! Font metadata as Typst's own parser sees it.
//!
//! A host that stores font files can record, at upload time, the family,
//! style, weight and stretch the compiler will match `text(font: ..)` against,
//! without re-implementing any of Typst's naming rules.

use rustc_hash::FxHashMap;
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
    /// Typst's PANOSE-based serif guess.
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
    check_coverage(bytes)?;

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

/// The most codepoints one face's `cmap` subtables may declare, summed.
///
/// Reading a face lists every codepoint of its `cmap` one by one, and a
/// 12-byte range can declare four billion of them. Four times the Unicode
/// range leaves room for a real face's several subtables.
const MAX_FACE_CODEPOINTS: u64 = 4 * 0x110000;

/// The most codepoints all faces of one file may declare, summed.
const MAX_FILE_CODEPOINTS: u64 = 16 * 0x110000;

/// Rejects a file whose `cmap` tables would make reading it unbounded.
///
/// Counts, from the raw table bytes, how many codepoints each face's
/// subtables declare, before the face is read. Every encoding record is
/// counted, since several records may point at the same subtable and each
/// one is listed again; each counts as at least one, as does every step the
/// parser takes through a subtable, so the count bounds the parser's work
/// and not only its output. The count stops as soon as a limit is crossed, so
/// the check itself stays bounded. A face the font parser cannot read is
/// left to the reader, which skips it.
fn check_coverage(bytes: &[u8]) -> Result<(), String> {
    let count = collection_count(bytes).unwrap_or(1);
    let mut file_total = 0;
    for index in 0..count {
        let Ok(face) = ttf_parser::Face::parse(bytes, index) else { continue };
        let Some(cmap) = cmap_table(face.raw_face()) else { continue };
        let face_total = cmap_codepoints(cmap, MAX_FACE_CODEPOINTS)?;
        if face_total > MAX_FACE_CODEPOINTS {
            return Err(format!("font face {index} declares too many codepoints"));
        }
        file_total += face_total;
        if file_total > MAX_FILE_CODEPOINTS {
            return Err("the font file declares too many codepoints".into());
        }
    }
    Ok(())
}

/// The `cmap` table the font parser reads for a face.
///
/// Mirrors how the parser collects tables: a linear pass over the table
/// directory, which need not be sorted, where the last `cmap` record wins.
/// A record whose end overflows is passed over, and one whose range lies
/// outside the file leaves the face without a `cmap`. A binary search of the
/// directory could pick a different table, or none, than the one the face is
/// read with.
fn cmap_table<'a>(raw: &ttf_parser::RawFace<'a>) -> Option<&'a [u8]> {
    let tag = ttf_parser::Tag::from_bytes(b"cmap");
    let mut cmap = None;
    for record in raw.table_records {
        if record.tag != tag {
            continue;
        }
        let start = record.offset as usize;
        let Some(end) = start.checked_add(record.length as usize) else { continue };
        cmap = raw.data.get(start..end);
    }
    cmap
}

/// Sums the codepoints declared by every encoding record of a `cmap` table,
/// stopping once the sum passes `limit`.
///
/// Errors on a format 12 or 13 group that is reversed or ends past U+10FFFF.
/// A subtable too short to hold what it declares counts as empty, since the
/// font parser does not read it either.
///
/// Each subtable is counted once and its count charged to every record that
/// points at it, so the check costs one pass over the table's bytes plus one
/// step per record however many records share a subtable. A count cut short
/// by the limit pushes the sum past it at once, so no cut count is reused.
fn cmap_codepoints(cmap: &[u8], limit: u64) -> Result<u64, String> {
    let records = read_u16(cmap, 2).unwrap_or(0);
    let mut counted = FxHashMap::<u32, u64>::default();
    let mut total = 0;
    for record in 0..usize::from(records) {
        let Some(offset) = read_u32(cmap, 4 + 8 * record + 4) else { break };
        let count = match counted.get(&offset) {
            Some(&count) => count,
            None => {
                let count = match cmap.get(offset as usize..) {
                    Some(subtable) => {
                        subtable_codepoints(subtable, limit.saturating_sub(total))?
                    }
                    None => 0,
                };
                counted.insert(offset, count);
                count
            }
        };
        // Every record counts as at least one, even one that lists nothing,
        // so that the record count alone moves toward the limit.
        total += count.max(1);
        if total > limit {
            break;
        }
    }
    Ok(total)
}

/// The codepoints one `cmap` subtable declares (an upper bound of what the
/// font parser lists), stopping once past `limit`.
fn subtable_codepoints(data: &[u8], limit: u64) -> Result<u64, String> {
    let Some(format) = read_u16(data, 0) else { return Ok(0) };
    let count = match format {
        0 => 256,
        2 => {
            // 256 sub-header keys, then sub-headers of 8 bytes each. The
            // parser visits every key, so each counts as at least one, even
            // one whose sub-header lists nothing.
            let mut total = 0;
            for byte in 0..256 {
                let Some(key) = read_u16(data, 6 + 2 * byte) else { return Ok(0) };
                let sub = usize::from(key / 8);
                total += match sub {
                    0 => 1,
                    _ => u64::from(read_u16(data, 518 + 8 * sub + 2).unwrap_or(0)).max(1),
                };
            }
            total
        }
        4 => {
            let Some(seg_x2) = read_u16(data, 6) else { return Ok(0) };
            let segs = usize::from(seg_x2 / 2);
            let mut total = 0;
            for seg in 0..segs {
                let (Some(end), Some(start)) = (
                    read_u16(data, 14 + 2 * seg),
                    read_u16(data, 16 + usize::from(seg_x2) + 2 * seg),
                ) else {
                    return Ok(0);
                };
                // A reversed segment lists nothing, but counts as one so
                // that every segment moves the count toward the limit.
                total += u64::from(end.saturating_sub(start)) + 1;
                if total > limit {
                    break;
                }
            }
            total
        }
        6 => {
            let entries = u64::from(read_u16(data, 8).unwrap_or(0));
            entries.min((data.len().saturating_sub(10) / 2) as u64)
        }
        10 => {
            let chars = u64::from(read_u32(data, 16).unwrap_or(0));
            chars.min((data.len().saturating_sub(20) / 2) as u64)
        }
        12 | 13 => {
            let Some(groups) = read_u32(data, 12) else { return Ok(0) };
            if groups as usize > data.len().saturating_sub(16) / 12 {
                return Ok(0);
            }
            let mut total = 0;
            for group in 0..groups as usize {
                let at = 16 + 12 * group;
                let (Some(start), Some(end)) =
                    (read_u32(data, at), read_u32(data, at + 4))
                else {
                    return Ok(0);
                };
                if start > end || end > 0x10FFFF {
                    return Err(format!(
                        "a font cmap maps the invalid range {start:#X}..={end:#X}"
                    ));
                }
                total += u64::from(end - start) + 1;
                if total > limit {
                    break;
                }
            }
            total
        }
        _ => 0,
    };
    Ok(count)
}

fn read_u16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at.checked_add(2)?)?.try_into().ok()?))
}

fn read_u32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at.checked_add(4)?)?.try_into().ok()?))
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

    #[test]
    fn a_collection_header_longer_than_its_bytes_is_rejected() {
        let mut forged = b"ttcf\0\x01\0\0".to_vec();
        forged.extend_from_slice(&200_u32.to_be_bytes());
        forged.resize(30, 0);
        // Under the face limit, but 30 bytes cannot hold 200 offsets.
        let err = font_faces(&forged).expect_err("a forged header");
        assert!(err.contains("claims 200 faces"), "{err}");
    }

    /// Replaces a font's `cmap` table with `cmap`, appended at the end.
    fn with_cmap(font: &[u8], cmap: &[u8]) -> Vec<u8> {
        let mut out = font.to_vec();
        let record = record_of(&out, *b"cmap");
        repoint(&mut out, record, cmap);
        out
    }

    /// The position of the first table record tagged `tag`.
    fn record_of(font: &[u8], tag: [u8; 4]) -> usize {
        let tables = u16::from_be_bytes([font[4], font[5]]) as usize;
        (0..tables)
            .map(|i| 12 + 16 * i)
            .find(|&at| font[at..at + 4] == tag)
            .expect("the font has the table")
    }

    /// Appends `table` and points the table record at `record` to it.
    fn repoint(font: &mut Vec<u8>, record: usize, table: &[u8]) {
        while font.len() % 4 != 0 {
            font.push(0);
        }
        let offset = font.len() as u32;
        font[record + 8..record + 12].copy_from_slice(&offset.to_be_bytes());
        font[record + 12..record + 16]
            .copy_from_slice(&(table.len() as u32).to_be_bytes());
        font.extend_from_slice(table);
    }

    /// Moves the `cmap` record to the front of the table directory, which
    /// leaves the directory unsorted.
    fn cmap_first(mut font: Vec<u8>) -> Vec<u8> {
        let record = record_of(&font, *b"cmap");
        font[12..record + 16].rotate_right(16);
        font
    }

    /// Gives the font two `cmap` records: the original one points at
    /// `first`, and the later `post` record, retagged, at `last`.
    fn two_cmaps(first: &[u8], last: &[u8]) -> Vec<u8> {
        let mut font = some_font().to_vec();
        let original = record_of(&font, *b"cmap");
        let later = record_of(&font, *b"post");
        assert!(later > original);
        font[later..later + 4].copy_from_slice(b"cmap");
        repoint(&mut font, original, first);
        repoint(&mut font, later, last);
        font
    }

    /// A `cmap` whose `records` encoding records all point at one subtable.
    fn cmap(records: u16, subtable: &[u8]) -> Vec<u8> {
        let mut out = vec![0, 0];
        out.extend_from_slice(&records.to_be_bytes());
        for _ in 0..records {
            out.extend_from_slice(&[0, 3, 0, 10]);
            out.extend_from_slice(&(4 + 8 * u32::from(records)).to_be_bytes());
        }
        out.extend_from_slice(subtable);
        out
    }

    /// A format 12 subtable with the given `(start, end)` groups.
    fn format12(groups: &[(u32, u32)]) -> Vec<u8> {
        let mut out = vec![0, 12, 0, 0];
        out.extend_from_slice(&(16 + 12 * groups.len() as u32).to_be_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&(groups.len() as u32).to_be_bytes());
        for &(start, end) in groups {
            out.extend_from_slice(&start.to_be_bytes());
            out.extend_from_slice(&end.to_be_bytes());
            out.extend_from_slice(&1_u32.to_be_bytes());
        }
        out
    }

    /// A format 4 subtable with one `start..=end` segment and the final one.
    fn format4(start: u16, end: u16) -> Vec<u8> {
        let mut out = vec![0, 4, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0];
        for value in [end, 0xFFFF, 0, start, 0xFFFF, 0, 1, 0, 0] {
            out.extend_from_slice(&value.to_be_bytes());
        }
        let len = out.len() as u16;
        out[2..4].copy_from_slice(&len.to_be_bytes());
        out
    }

    fn quickly_rejected(font: &[u8]) -> String {
        let started = std::time::Instant::now();
        let err = font_faces(font).expect_err("the font is rejected");
        assert!(started.elapsed() < std::time::Duration::from_millis(200));
        err
    }

    #[test]
    fn a_sane_replacement_cmap_still_reads() {
        // Proves the fixtures below build a readable font, so their errors
        // come from the guard.
        let font = with_cmap(some_font(), &cmap(1, &format12(&[(0x20, 0x7E)])));
        assert_eq!(font_faces(&font).expect("a font").len(), 1);
        let font = with_cmap(some_font(), &cmap(1, &format4(0x20, 0x7E)));
        assert_eq!(font_faces(&font).expect("a font").len(), 1);
    }

    #[test]
    fn a_cmap_group_past_unicode_is_rejected_quickly() {
        let font = with_cmap(some_font(), &cmap(1, &format12(&[(0, u32::MAX)])));
        assert!(quickly_rejected(&font).contains("cmap"));
    }

    #[test]
    fn a_reversed_cmap_group_is_rejected() {
        let font = with_cmap(some_font(), &cmap(1, &format12(&[(0x7E, 0x20)])));
        assert!(quickly_rejected(&font).contains("cmap"));
    }

    #[test]
    fn a_face_declaring_too_many_codepoints_is_rejected_quickly() {
        // Each group is valid, but together they list the Unicode range five
        // times over.
        let groups = [(0, 0x10FFFF); 5];
        let font = with_cmap(some_font(), &cmap(1, &format12(&groups)));
        quickly_rejected(&font);
    }

    #[test]
    fn records_sharing_one_subtable_count_once_each() {
        // A single full-range subtable is fine; listed by 100 records, it is
        // read 100 times.
        let font = with_cmap(some_font(), &cmap(100, &format12(&[(0, 0x10FFFF)])));
        quickly_rejected(&font);
        let font = with_cmap(some_font(), &cmap(100, &format4(0, 0xFFFE)));
        quickly_rejected(&font);
    }

    #[test]
    fn a_cmap_first_in_an_unsorted_directory_is_still_checked() {
        let benign = cmap(1, &format12(&[(0x20, 0x7E)]));
        let font = cmap_first(with_cmap(some_font(), &benign));
        assert_eq!(font_faces(&font).expect("a font").len(), 1);

        let reversed = cmap(1, &format12(&[(0x7E, 0x20)]));
        let font = cmap_first(with_cmap(some_font(), &reversed));
        assert!(quickly_rejected(&font).contains("cmap"));

        let wide = cmap(1, &format12(&[(0, 0x10FFFF); 5]));
        quickly_rejected(&cmap_first(with_cmap(some_font(), &wide)));
    }

    #[test]
    fn of_two_cmap_records_the_last_one_is_checked() {
        let benign = cmap(1, &format12(&[(0x20, 0x7E)]));
        let reversed = cmap(1, &format12(&[(0x7E, 0x20)]));
        assert!(quickly_rejected(&two_cmaps(&benign, &reversed)).contains("cmap"));
        let faces = font_faces(&two_cmaps(&reversed, &benign)).expect("a font");
        assert_eq!(faces.len(), 1);
    }

    /// A format 2 subtable whose 256 keys all point at one empty sub-header.
    fn format2_empty() -> Vec<u8> {
        let mut out = vec![0, 2, 0, 0, 0, 0];
        for _ in 0..256 {
            out.extend_from_slice(&8_u16.to_be_bytes());
        }
        out.extend_from_slice(&[0; 16]);
        let len = out.len() as u16;
        out[2..4].copy_from_slice(&len.to_be_bytes());
        out
    }

    #[test]
    fn empty_format2_subtables_still_count_their_work() {
        // Each record lists nothing, but the parser walks all 256 keys of its
        // subtable, so 65,535 records are over 16 million steps.
        let font = with_cmap(some_font(), &cmap(u16::MAX, &format2_empty()));
        quickly_rejected(&font);
    }
}
