//! ANSI (system code page) ↔ Unicode conversion for byte-exact VBA string I/O.
//!
//! VBA 7.1 stores strings on disk and marshals them across `Declare` 'A'
//! boundaries as **ANSI** — the active system code page (`CP_ACP`), via
//! `WideCharToMultiByte`/`MultiByteToWideChar`. We do exactly that on Windows.
//!
//! On non-Windows targets (where VBA itself does not run — these builds exist only
//! for cross-platform development and CI) we use a built-in **Windows-1252** codec,
//! the predominant Western ANSI code page, so the dev lane is correct for Latin
//! text. DBCS / non-1252 system code pages are only reproduced on real Windows.

/// Encode a Unicode string to ANSI (system code page) bytes. No terminator.
pub fn ansi_encode(text: &str) -> Vec<u8> {
    #[cfg(target_os = "windows")]
    {
        windows_codec::encode(text)
    }
    #[cfg(not(target_os = "windows"))]
    {
        cp1252::encode(text)
    }
}

/// Decode ANSI (system code page) bytes to a Unicode string.
pub fn ansi_decode(bytes: &[u8]) -> String {
    #[cfg(target_os = "windows")]
    {
        windows_codec::decode(bytes)
    }
    #[cfg(not(target_os = "windows"))]
    {
        cp1252::decode(bytes)
    }
}

#[cfg(target_os = "windows")]
mod windows_codec {
    const CP_ACP: u32 = 0;

    unsafe extern "system" {
        fn WideCharToMultiByte(
            code_page: u32,
            flags: u32,
            wide: *const u16,
            wide_len: i32,
            multi_byte: *mut u8,
            multi_byte_len: i32,
            default_char: *const u8,
            used_default: *mut i32,
        ) -> i32;
        fn MultiByteToWideChar(
            code_page: u32,
            flags: u32,
            multi_byte: *const u8,
            multi_byte_len: i32,
            wide: *mut u16,
            wide_len: i32,
        ) -> i32;
    }

    pub fn encode(text: &str) -> Vec<u8> {
        if text.is_empty() {
            return Vec::new();
        }
        let wide: Vec<u16> = text.encode_utf16().collect();
        let needed = unsafe {
            WideCharToMultiByte(
                CP_ACP,
                0,
                wide.as_ptr(),
                wide.len() as i32,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        };
        if needed <= 0 {
            return text.as_bytes().to_vec();
        }
        let mut out = vec![0u8; needed as usize];
        let written = unsafe {
            WideCharToMultiByte(
                CP_ACP,
                0,
                wide.as_ptr(),
                wide.len() as i32,
                out.as_mut_ptr(),
                needed,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        };
        out.truncate(written.max(0) as usize);
        out
    }

    pub fn decode(bytes: &[u8]) -> String {
        if bytes.is_empty() {
            return String::new();
        }
        let needed = unsafe {
            MultiByteToWideChar(CP_ACP, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0)
        };
        if needed <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut wide = vec![0u16; needed as usize];
        let written = unsafe {
            MultiByteToWideChar(
                CP_ACP,
                0,
                bytes.as_ptr(),
                bytes.len() as i32,
                wide.as_mut_ptr(),
                needed,
            )
        };
        String::from_utf16_lossy(&wide[..written.max(0) as usize])
    }
}

#[cfg(not(target_os = "windows"))]
mod cp1252 {
    /// Windows-1252 mapping for the 0x80..=0x9F range (the rest of 0x00..=0xFF is
    /// Latin-1 identity). `U+FFFD` marks the five undefined CP-1252 positions.
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{FFFD}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{FFFD}',
        '\u{017D}', '\u{FFFD}', '\u{FFFD}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
        '\u{0153}', '\u{FFFD}', '\u{017E}', '\u{0178}',
    ];

    pub fn encode(text: &str) -> Vec<u8> {
        text.chars()
            .map(|c| {
                let u = c as u32;
                if u < 0x80 || (0xA0..=0xFF).contains(&u) {
                    u as u8
                } else if let Some(i) = HIGH.iter().position(|&h| h == c) {
                    0x80 + i as u8
                } else {
                    b'?'
                }
            })
            .collect()
    }

    pub fn decode(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|&b| if b < 0x80 || b >= 0xA0 { b as char } else { HIGH[(b - 0x80) as usize] })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_round_trips_and_is_one_byte_per_char() {
        let bytes = ansi_encode("Hello");
        assert_eq!(bytes, b"Hello");
        assert_eq!(ansi_decode(&bytes), "Hello");
    }

    // On non-Windows the CP-1252 fallback maps é (U+00E9) to the single byte 0xE9.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn latin1_high_char_is_single_ansi_byte() {
        let bytes = ansi_encode("é");
        assert_eq!(bytes, vec![0xE9]);
        assert_eq!(ansi_decode(&bytes), "é");
    }
}
