use serde::{Deserialize, Serialize};
use std::fmt;

/// Filename encoding hint for non-UTF-8 archives (primarily ZIP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilenameEncoding {
    /// Keep the raw bytes as-is (default if UTF-8 is valid).
    Utf8,
    /// Auto-detect with `chardetng` (only used when UTF-8 validation fails).
    Auto,
    /// Japanese Shift-JIS (Windows codepage 932).
    ShiftJis,
    /// Simplified Chinese GBK/GB 18030 (Windows codepage 936).
    Gbk,
    /// Korean EUC-KR (Windows codepage 949).
    EucKr,
    /// Cyrillic Windows-1251.
    Windows1251,
}

impl FilenameEncoding {
    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Auto => "Auto-detect",
            Self::ShiftJis => "Shift-JIS (CP932)",
            Self::Gbk => "GBK (CP936)",
            Self::EucKr => "EUC-KR (CP949)",
            Self::Windows1251 => "Windows-1251",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "utf-8" | "utf8" => Some(Self::Utf8),
            "auto" => Some(Self::Auto),
            "932" | "shift-jis" | "shift_jis" | "sjis" => Some(Self::ShiftJis),
            "936" | "gbk" | "gb2312" | "gb18030" => Some(Self::Gbk),
            "949" | "euc-kr" | "euckr" => Some(Self::EucKr),
            "1251" | "windows-1251" | "win1251" => Some(Self::Windows1251),
            _ => None,
        }
    }
}

impl fmt::Display for FilenameEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Decode a raw ZIP filename according to the given encoding hint.
///
/// Strategy:
/// 1. Try UTF-8 first — if valid, return it regardless of hint.
/// 2. Otherwise, use the hint to decode.
pub fn decode_zip_filename(raw: &[u8], encoding: FilenameEncoding) -> String {
    if raw.is_empty() {
        return String::new();
    }

    // Always try UTF-8 first.
    if let Ok(utf8) = std::str::from_utf8(raw) {
        return utf8.to_string();
    }

    match encoding {
        FilenameEncoding::Utf8 => {
            // UTF-8 validation failed; return lossy representation.
            String::from_utf8_lossy(raw).into_owned()
        }
        FilenameEncoding::Auto => auto_decode(raw),
        FilenameEncoding::ShiftJis => decode_with(raw, encoding_rs::SHIFT_JIS),
        FilenameEncoding::Gbk => decode_with(raw, encoding_rs::GBK),
        FilenameEncoding::EucKr => decode_with(raw, encoding_rs::EUC_KR),
        FilenameEncoding::Windows1251 => decode_with(raw, encoding_rs::WINDOWS_1251),
    }
}

fn decode_with(raw: &[u8], encoding: &'static encoding_rs::Encoding) -> String {
    let (decoded, _encoding_used, had_errors) = encoding.decode(raw);
    if had_errors {
        // Fall back to lossy UTF-8 if the specified encoding also fails.
        String::from_utf8_lossy(raw).into_owned()
    } else {
        decoded.into_owned()
    }
}

fn auto_decode(raw: &[u8]) -> String {
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(raw, true);
    let (encoding, confident) = detector.guess_assess(None, true);

    let (decoded, _encoding_used, had_errors) = encoding.decode(raw);

    // If not confident or had errors, try the lossy UTF-8 approach as fallback.
    if !confident || had_errors {
        let lossy = String::from_utf8_lossy(raw);
        // If both results are similar, prefer the detected one.
        if had_errors {
            return lossy.into_owned();
        }
    }

    decoded.into_owned()
}
