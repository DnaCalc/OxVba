//! VBA hex (`&H…`) / octal (`&O…`) integer-literal interpretation (MS-VBAL §3.3.2).
//!
//! The digits give an **unsigned magnitude**; the literal's value is that
//! magnitude reinterpreted as a two's-complement signed integer of the
//! literal's *type width*. The width comes from a trailing type character
//! (`%`=Integer/16-bit, `&`=Long/32-bit, `^`=LongLong/64-bit) or, when absent,
//! from the narrowest width that holds the magnitude.
//!
//! This is why (verified against live Office VBA 7.1):
//! `&HFFFF`=-1, `&H8000`=-32768, `&H7FFF`=32767 (all Integer width),
//! `&HFFFF&`=65535 and `&H10000`=65536 (Long width),
//! `&HFFFFFFFF`=-1 (Long), `&O37777777777`=-1 (Long, octal of 0xFFFFFFFF).
//!
//! The same rule governs `Val`/`CInt`/`CLng` of a `&H…`/`&O…` *string*, so the
//! runtime string path shares these helpers.

/// Bit-width that fixes the two's-complement sign interpretation of a literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VbaRadixWidth {
    /// 16-bit (`Integer`).
    Integer,
    /// 32-bit (`Long`).
    Long,
    /// 64-bit (`LongLong`).
    LongLong,
}

/// The literal's width from its trailing type character (`%`/`&`/`^`), or —
/// absent one — the narrowest width that holds `magnitude`. Returns `None` when
/// a type character's width cannot hold the magnitude (a compile-time overflow
/// in VBA).
pub fn vba_radix_width(magnitude: u64, suffix: Option<u8>) -> Option<VbaRadixWidth> {
    Some(match suffix {
        Some(b'%') => {
            if magnitude > u16::MAX as u64 {
                return None;
            }
            VbaRadixWidth::Integer
        }
        Some(b'&') => {
            if magnitude > u32::MAX as u64 {
                return None;
            }
            VbaRadixWidth::Long
        }
        Some(b'^') => VbaRadixWidth::LongLong,
        _ => {
            if magnitude <= u16::MAX as u64 {
                VbaRadixWidth::Integer
            } else if magnitude <= u32::MAX as u64 {
                VbaRadixWidth::Long
            } else {
                VbaRadixWidth::LongLong
            }
        }
    })
}

/// Reinterpret `magnitude` as a signed value of `width` bits (two's complement).
pub fn vba_radix_signed_value(magnitude: u64, width: VbaRadixWidth) -> i64 {
    match width {
        VbaRadixWidth::Integer => (magnitude as u16) as i16 as i64,
        VbaRadixWidth::Long => (magnitude as u32) as i32 as i64,
        VbaRadixWidth::LongLong => magnitude as i64,
    }
}

/// Parse a `&H…`/`&O…` token of the given `radix` (16 or 8) into its signed
/// value. The text may carry the `&H`/`&O` prefix and/or a trailing `%`/`&`/`^`
/// type character; both are stripped here. Returns `None` on malformed digits
/// or a type-character width overflow.
pub fn parse_vba_radix(text: &str, radix: u32) -> Option<i64> {
    let trimmed = text.trim();
    let suffix = match trimmed.as_bytes().last() {
        Some(&b @ (b'%' | b'&' | b'^')) => Some(b),
        _ => None,
    };
    let body = trimmed
        .trim_start_matches('&')
        .trim_start_matches(['h', 'H', 'o', 'O'])
        .trim_end_matches(['&', '%', '^']);
    if body.is_empty() {
        return None;
    }
    let magnitude = u64::from_str_radix(body, radix).ok()?;
    let width = vba_radix_width(magnitude, suffix)?;
    Some(vba_radix_signed_value(magnitude, width))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsuffixed_hex_uses_two_s_complement_at_the_magnitude_width() {
        // Integer width (≤ 16 bits): high bit set → negative.
        assert_eq!(parse_vba_radix("&HFFFF", 16), Some(-1));
        assert_eq!(parse_vba_radix("&H8000", 16), Some(-32768));
        assert_eq!(parse_vba_radix("&H7FFF", 16), Some(32767));
        assert_eq!(parse_vba_radix("&HFF", 16), Some(255));
        // Long width (17..=32 bits).
        assert_eq!(parse_vba_radix("&H10000", 16), Some(65536));
        assert_eq!(parse_vba_radix("&HFFFFFFFF", 16), Some(-1));
        assert_eq!(parse_vba_radix("&H80000000", 16), Some(-2147483648));
    }

    #[test]
    fn type_character_fixes_the_width() {
        // `&` forces Long even when the magnitude fits Integer.
        assert_eq!(parse_vba_radix("&HFFFF&", 16), Some(65535));
        assert_eq!(parse_vba_radix("&HFFFFFFFF&", 16), Some(-1));
        // `%` forces Integer.
        assert_eq!(parse_vba_radix("&HFFFF%", 16), Some(-1));
        // `^` forces LongLong (no narrowing).
        assert_eq!(parse_vba_radix("&HFFFFFFFF^", 16), Some(0xFFFF_FFFF));
        assert_eq!(parse_vba_radix("&HFFFFFFFFFFFFFFFF^", 16), Some(-1));
    }

    #[test]
    fn octal_shares_the_magnitude_width_rule() {
        // 0xFFFF as octal is 177777 → Integer width → -1.
        assert_eq!(parse_vba_radix("&O177777", 8), Some(-1));
        // 0xFFFFFFFF as octal is 37777777777 → Long width → -1.
        assert_eq!(parse_vba_radix("&O37777777777", 8), Some(-1));
        assert_eq!(parse_vba_radix("&O17", 8), Some(15));
    }

    #[test]
    fn unsuffixed_value_promotes_past_long_to_longlong() {
        // 33-bit magnitude: too wide for Long, so LongLong width (no sign flip).
        assert_eq!(parse_vba_radix("&H1FFFFFFFF", 16), Some(0x1_FFFF_FFFF));
    }

    #[test]
    fn type_character_overflow_is_rejected() {
        // `%` (Integer) cannot hold a 17-bit magnitude.
        assert_eq!(parse_vba_radix("&H10000%", 16), None);
        // `&` (Long) cannot hold a 33-bit magnitude.
        assert_eq!(parse_vba_radix("&H1FFFFFFFF&", 16), None);
    }

    #[test]
    fn malformed_digits_are_rejected() {
        assert_eq!(parse_vba_radix("&HZZ", 16), None);
        assert_eq!(parse_vba_radix("&H", 16), None);
    }
}
