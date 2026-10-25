//! System fonts for interface languages whose glyph shapes the embedded font does not
//! have, such as Japanese kanji.

use std::{fs, path::PathBuf};

/// Faces to try for each language tag, most preferred first, as file names in the system
/// font folder and the full name of the face inside the file.
fn candidates(language: &str) -> &'static [(&'static str, &'static str)] {
    match language {
        "ja" => &[
            ("YuGothM.ttc", "Yu Gothic UI Regular"),
            ("meiryo.ttc", "Meiryo UI"),
            ("msgothic.ttc", "MS UI Gothic"),
        ],
        _ => &[],
    }
}

/// The system's interface font for the BCP 47 tag `language`, as the bytes of its font file
/// and the index of the face in it; `None` when the language uses the embedded font or the
/// system has none of the faces.
pub fn ui_font(language: &str) -> Option<(Vec<u8>, u32)> {
    let folder = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("WINDIR"))
        .map(|root| PathBuf::from(root).join("Fonts"))?;
    candidates(language).iter().find_map(|(file, face)| {
        let bytes = fs::read(folder.join(file)).ok()?;
        let index = face_index(&bytes, face)?;
        Some((bytes, index))
    })
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// Index of the face whose full name, in US English, is `name` in a font file or a font
/// collection.
fn face_index(data: &[u8], name: &str) -> Option<u32> {
    let faces: Vec<usize> = if data.get(..4)? == b"ttcf" {
        let count = u32_at(data, 8)? as usize;
        (0..count)
            .map(|i| u32_at(data, 12 + 4 * i).map(|offset| offset as usize))
            .collect::<Option<_>>()?
    } else {
        vec![0]
    };
    faces
        .iter()
        .position(|&offset| full_name(data, offset).as_deref() == Some(name))
        .map(|index| index as u32)
}

/// Full name of the face whose table directory starts at `offset`, from its `name` table.
fn full_name(data: &[u8], offset: usize) -> Option<String> {
    let tables = u16_at(data, offset + 4)? as usize;
    let table = (0..tables)
        .map(|i| offset + 12 + 16 * i)
        .find(|&record| data.get(record..record + 4) == Some(b"name"))?;
    let start = u32_at(data, table + 8)? as usize;
    let count = u16_at(data, start + 2)? as usize;
    let strings = start + u16_at(data, start + 4)? as usize;
    (0..count).find_map(|i| {
        let record = start + 6 + 12 * i;
        // Windows platform, US English, full name.
        let wanted = u16_at(data, record)? == 3
            && u16_at(data, record + 4)? == 0x409
            && u16_at(data, record + 6)? == 4;
        if !wanted {
            return None;
        }
        let length = u16_at(data, record + 8)? as usize;
        let at = strings + u16_at(data, record + 10)? as usize;
        let units: Vec<u16> = data
            .get(at..at + length)?
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes(*pair))
            .collect();
        String::from_utf16(&units).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A face with only a `name` table holding its full name.
    fn face(name: &str) -> Vec<u8> {
        let text: Vec<u8> = name.encode_utf16().flat_map(u16::to_be_bytes).collect();
        let mut table = Vec::new();
        for value in [0, 1, 18, 3, 1, 0x409, 4, text.len() as u16, 0] {
            table.extend_from_slice(&u16::to_be_bytes(value));
        }
        table.extend_from_slice(&text);
        let mut face = Vec::new();
        face.extend_from_slice(&0x0001_0000_u32.to_be_bytes());
        for value in [1_u16, 16, 0, 0] {
            face.extend_from_slice(&value.to_be_bytes());
        }
        face.extend_from_slice(b"name");
        face.extend_from_slice(&0_u32.to_be_bytes());
        face.extend_from_slice(&28_u32.to_be_bytes());
        face.extend_from_slice(&(table.len() as u32).to_be_bytes());
        face.extend_from_slice(&table);
        face
    }

    #[test]
    fn faces_are_found_by_full_name_in_a_collection() {
        let first = face("Yu Gothic Medium");
        let second = face("Yu Gothic UI Regular");
        // The name table offsets of each face count from the start of the collection.
        let shift = |face: &[u8], by: usize| {
            let mut face = face.to_vec();
            let offset = u32::from_be_bytes(face[20..24].try_into().unwrap()) as usize + by;
            face[20..24].copy_from_slice(&(offset as u32).to_be_bytes());
            face
        };
        let header = 20;
        let mut collection = b"ttcf".to_vec();
        collection.extend_from_slice(&0x0001_0000_u32.to_be_bytes());
        collection.extend_from_slice(&2_u32.to_be_bytes());
        collection.extend_from_slice(&(header as u32).to_be_bytes());
        collection.extend_from_slice(&((header + first.len()) as u32).to_be_bytes());
        collection.extend_from_slice(&shift(&first, header));
        collection.extend_from_slice(&shift(&second, header + first.len()));
        assert_eq!(face_index(&collection, "Yu Gothic UI Regular"), Some(1));
        assert_eq!(face_index(&collection, "Yu Gothic Medium"), Some(0));
        assert_eq!(face_index(&collection, "Meiryo UI"), None);
        assert_eq!(face_index(&face("Meiryo UI"), "Meiryo UI"), Some(0));
        assert_eq!(face_index(b"ttcf\0\0", "Meiryo UI"), None);
        assert!(ui_font("en").is_none());
    }
}
