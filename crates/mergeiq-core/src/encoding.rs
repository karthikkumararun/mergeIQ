use crate::error::{MergeError, Side};

/// How many leading bytes of unmarked (no-BOM) input to scan for a NUL byte when
/// deciding whether it's binary.
pub const BINARY_SCAN_LIMIT: usize = 8000;

/// A text encoding [`decode`]/[`encode`] can round-trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Encoding {
    /// UTF-8, with or without a BOM.
    Utf8,
    /// UTF-16, little-endian (always BOM-tagged on decode).
    Utf16Le,
    /// UTF-16, big-endian (always BOM-tagged on decode).
    Utf16Be,
}

/// An encoding plus whether a byte-order mark was present, as detected by [`decode`]
/// and reused by [`encode`] to round-trip byte-exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct EncodingInfo {
    /// The detected encoding.
    pub encoding: Encoding,
    /// Whether the input had a byte-order mark.
    pub bom: bool,
}

/// Detects encoding from a BOM, falling back to a NUL-byte binary heuristic for
/// unmarked input, then decodes to UTF-8 text.
///
/// A BOM is treated as a declaration of encoding: BOM-tagged input that fails to
/// decode is `MergeError::Decode` (known encoding, corrupt bytes). Unmarked input with
/// a NUL byte in its first [`BINARY_SCAN_LIMIT`] bytes, or that isn't valid UTF-8, is
/// `MergeError::Binary` (unknown encoding, looks non-textual).
pub fn decode(bytes: &[u8], side: Side) -> Result<(String, EncodingInfo), MergeError> {
    if let Some((enc, bom_len)) = encoding_rs::Encoding::for_bom(bytes) {
        let (cow, had_errors) = enc.decode_without_bom_handling(&bytes[bom_len..]);
        if had_errors {
            return Err(MergeError::Decode {
                side,
                reason: format!("invalid {} bytes", enc.name()),
            });
        }
        let encoding = match enc.name() {
            "UTF-8" => Encoding::Utf8,
            "UTF-16LE" => Encoding::Utf16Le,
            "UTF-16BE" => Encoding::Utf16Be,
            other => unreachable!("unexpected BOM encoding {other}"),
        };
        return Ok((
            cow.into_owned(),
            EncodingInfo {
                encoding,
                bom: true,
            },
        ));
    }

    let scan_len = bytes.len().min(BINARY_SCAN_LIMIT);
    if bytes[..scan_len].contains(&0) {
        return Err(MergeError::Binary { side });
    }

    let (cow, had_errors) = encoding_rs::UTF_8.decode_without_bom_handling(bytes);
    if had_errors {
        return Err(MergeError::Binary { side });
    }
    Ok((
        cow.into_owned(),
        EncodingInfo {
            encoding: Encoding::Utf8,
            bom: false,
        },
    ))
}

/// Re-encodes `text` using the given encoding/BOM, inverse of [`decode`].
pub fn encode(text: &str, info: &EncodingInfo) -> Vec<u8> {
    match info.encoding {
        Encoding::Utf8 => {
            let mut out = Vec::with_capacity(text.len() + 3);
            if info.bom {
                out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            }
            out.extend_from_slice(text.as_bytes());
            out
        }
        Encoding::Utf16Le => encode_utf16(text, false, info.bom),
        Encoding::Utf16Be => encode_utf16(text, true, info.bom),
    }
}

fn encode_utf16(text: &str, big_endian: bool, bom: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2 + 2);
    if bom {
        out.extend_from_slice(if big_endian {
            &[0xFE, 0xFF]
        } else {
            &[0xFF, 0xFE]
        });
    }
    for unit in text.encode_utf16() {
        let bytes = if big_endian {
            unit.to_be_bytes()
        } else {
            unit.to_le_bytes()
        };
        out.extend_from_slice(&bytes);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_no_bom_round_trips() {
        let input = "hello\nworld\n".as_bytes();
        let (text, info) = decode(input, Side::Base).unwrap();
        assert_eq!(text, "hello\nworld\n");
        assert_eq!(info.encoding, Encoding::Utf8);
        assert!(!info.bom);
        assert_eq!(encode(&text, &info), input);
    }

    #[test]
    fn utf8_bom_round_trips() {
        let mut input = vec![0xEF, 0xBB, 0xBF];
        input.extend_from_slice("hello".as_bytes());
        let (text, info) = decode(&input, Side::Base).unwrap();
        assert_eq!(text, "hello");
        assert_eq!(info.encoding, Encoding::Utf8);
        assert!(info.bom);
        assert_eq!(encode(&text, &info), input);
    }

    #[test]
    fn utf16_le_bom_round_trips() {
        let mut input = vec![0xFF, 0xFE];
        for unit in "hello".encode_utf16() {
            input.extend_from_slice(&unit.to_le_bytes());
        }
        let (text, info) = decode(&input, Side::Ours).unwrap();
        assert_eq!(text, "hello");
        assert_eq!(info.encoding, Encoding::Utf16Le);
        assert!(info.bom);
        assert_eq!(encode(&text, &info), input);
    }

    #[test]
    fn utf16_be_bom_round_trips() {
        let mut input = vec![0xFE, 0xFF];
        for unit in "hello".encode_utf16() {
            input.extend_from_slice(&unit.to_be_bytes());
        }
        let (text, info) = decode(&input, Side::Theirs).unwrap();
        assert_eq!(text, "hello");
        assert_eq!(info.encoding, Encoding::Utf16Be);
        assert!(info.bom);
        assert_eq!(encode(&text, &info), input);
    }

    #[test]
    fn nul_byte_is_binary() {
        let input = b"hello\0world";
        let err = decode(input, Side::Base).unwrap_err();
        assert_eq!(err, MergeError::Binary { side: Side::Base });
    }

    #[test]
    fn invalid_utf8_without_bom_is_binary() {
        let input = &[0x61, 0x80, 0x62][..]; // "a" + lone continuation byte + "b"
        let err = decode(input, Side::Ours).unwrap_err();
        assert_eq!(err, MergeError::Binary { side: Side::Ours });
    }

    #[test]
    fn corrupt_utf16_bom_body_is_decode_error() {
        // UTF-16 LE BOM followed by an odd trailing byte.
        let input = vec![0xFF, 0xFE, 0x41, 0x00, 0x42];
        let err = decode(&input, Side::Theirs).unwrap_err();
        assert!(matches!(
            err,
            MergeError::Decode {
                side: Side::Theirs,
                ..
            }
        ));
    }
}
