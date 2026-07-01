//! Pure (host-free) base-library bodies: strings, math, date/time arithmetic,
//! conversion, random, financial, information, and a first-cut Collection.
//!
//! The `Format` engine lives in [`crate::format`]. The remaining `FIDELITY:`
//! markers are features absent from the legacy VM too: keyed `Collection`
//! access (awaits the vm2 object model) and `StrConv`'s CJK/encoding modes.

use crate::{
    LibContext, LibError, LibResult, alloc_count, as_f64, as_i32, as_i64, as_str, as_usize, need,
    opt, vbool, vf64, vi32, vstr, vunit,
};
use oxvba_runtime::{
    Decimal96, Variant,
    safe_array::{SafeArray, VT_UI1_VALUE, VT_VARIANT_VALUE},
    variant::VarType,
};
// The VBA serial ↔ civil calendar math is canonical in `oxvba_runtime::vba_date`; re-export it
// at `crate::pure::*` so this module's date functions (and `format.rs`) keep their call sites.
pub(crate) use oxvba_runtime::{
    civil_from_days, days_from_civil, serial_to_hms, serial_to_ymd, ymd_to_serial,
};

const VBA_DATE_MIN_SERIAL: f64 = -657_434.0; // 0100-01-01
const VBA_DATE_MAX_EXCLUSIVE_SERIAL: f64 = 2_958_466.0; // 10000-01-01

// ── Strings ──────────────────────────────────────────────────────────────────

pub fn len(args: &[Variant]) -> LibResult<Variant> {
    let value = need(args, 0)?;
    // Count UTF-16 CODE UNITS (VBA's string length): read them directly off a String
    // Variant's BSTR so a lone/paired surrogate half counts as one unit each (a UTF-8
    // round-trip via `as_str` would fold a lone surrogate to U+FFFD). Non-strings coerce.
    let units = match value.string_units() {
        Some(units) => units.len(),
        None => as_str(value)?.encode_utf16().count(),
    };
    Ok(vi32(units as i32))
}

/// Optional trailing compare-mode argument: 0 = binary (default), 1 = text. The
/// front-end supplies that mode as a trailing argument here, resolved from the
/// source's `Option Compare` (the binder injects `1` for `Option Compare Text`
/// when the call omits an explicit compare).
fn text_compare(args: &[Variant], index: usize) -> LibResult<bool> {
    match arg_present(args, index) {
        Some(v) => Ok(as_i32(v)? == 1),
        None => Ok(false),
    }
}

/// True for the `Missing` optional-argument sentinel the VM passes for an omitted
/// optional (`CoreArg::Omitted` → vbError `DISP_E_PARAMNOTFOUND`).
fn missing_sentinel(v: &Variant) -> bool {
    const MISSING_ARG: i32 = 0x8002_0004u32 as i32;
    v.vtype() == VarType::Error && v.as_error_code() == Some(MISSING_ARG)
}

/// An optional argument that is actually present — not past the end, not `Empty`,
/// and not the `Missing` sentinel. This lets the binder pad an omitted
/// *intermediate* optional (so a synthesized trailing `compare` lands at the right
/// index) without the lib mistaking the pad for a real value: an omitted optional
/// in such a padded slot arrives as `Empty`/Missing and must read as absent.
fn arg_present(args: &[Variant], index: usize) -> Option<&Variant> {
    opt(args, index).filter(|v| !matches!(v.vtype(), VarType::Empty) && !missing_sentinel(v))
}

/// Text-mode normalization, ported from the legacy `normalize_for_compare`:
/// ASCII case-folding. FIDELITY: not Unicode/locale-aware.
fn norm_compare(s: String, text: bool) -> String {
    if text { s.to_ascii_lowercase() } else { s }
}

/// The operand's UTF-16 code units for comparison: a String reads its BSTR units VERBATIM
/// (lone surrogate halves preserved); a non-string coerces through its VBA string form
/// (which carries no surrogates). Mirrors the VM operator path (`arith::cmp_order`) so
/// `StrComp` distinguishes distinct lone surrogate halves instead of folding both to U+FFFD
/// via a lossy `as_str` round-trip — VBA strings are binary UTF-16 code-unit sequences.
fn compare_units(value: &Variant) -> LibResult<Vec<u16>> {
    match value.string_units() {
        Some(units) => Ok(units),
        None => Ok(as_str(value)?.encode_utf16().collect()),
    }
}

fn string_units(value: &Variant) -> LibResult<Vec<u16>> {
    match value.string_units() {
        Some(units) => Ok(units),
        None => Ok(as_str(value)?.encode_utf16().collect()),
    }
}

/// Case-fold UTF-16 code units the same way [`norm_compare`] folds a `String` — ASCII A–Z
/// only — but at the code-unit level so surrogate halves survive. Binary mode is verbatim.
fn norm_compare_units(units: Vec<u16>, text: bool) -> Vec<u16> {
    if text {
        units
            .into_iter()
            .map(|u| {
                if (0x41..=0x5A).contains(&u) {
                    u + 0x20
                } else {
                    u
                }
            })
            .collect()
    } else {
        units
    }
}

pub fn left(args: &[Variant]) -> LibResult<Variant> {
    let units = string_units(need(args, 0)?)?;
    let n = as_usize(need(args, 1)?)?;
    Ok(Variant::from_utf16_units(&units[..n.min(units.len())]))
}

pub fn right(args: &[Variant]) -> LibResult<Variant> {
    let units = string_units(need(args, 0)?)?;
    let n = as_usize(need(args, 1)?)?.min(units.len());
    Ok(Variant::from_utf16_units(&units[units.len() - n..]))
}

pub fn mid(args: &[Variant]) -> LibResult<Variant> {
    let units = string_units(need(args, 0)?)?;
    let start = one_based_mid_start(need(args, 1)?)?;
    let from = (start - 1).min(units.len());
    let take = match opt(args, 2) {
        Some(v) => as_usize(v)?,
        None => units.len() - from,
    };
    let to = from.saturating_add(take).min(units.len());
    Ok(Variant::from_utf16_units(&units[from..to]))
}

/// `Mid(s, start, [count]) = value` — returns the spliced string (the VM stores
/// it back into the target slot).
pub fn mid_stmt(args: &[Variant]) -> LibResult<Variant> {
    let mut units = string_units(need(args, 0)?)?;
    let start = one_based_mid_start(need(args, 1)?)? - 1;
    // args = [target, start, value] or [target, start, count, value]
    let (count, value) = if args.len() >= 4 {
        (
            Some(as_usize(need(args, 2)?)?),
            string_units(need(args, 3)?)?,
        )
    } else {
        (None, string_units(need(args, 2)?)?)
    };
    let limit = count.unwrap_or(value.len()).min(value.len());
    for (i, unit) in value.into_iter().take(limit).enumerate() {
        if let Some(index) = start.checked_add(i)
            && index < units.len()
        {
            units[index] = unit;
        }
    }
    Ok(Variant::from_utf16_units(&units))
}

pub fn lset_stmt(args: &[Variant]) -> LibResult<Variant> {
    align_stmt(args, false)
}

pub fn rset_stmt(args: &[Variant]) -> LibResult<Variant> {
    align_stmt(args, true)
}

/// `LSet s = value` / `RSet s = value` for string targets. The first argument is the
/// target's current value; its UTF-16 length is the alignment width for variable
/// strings, while fixed-length strings naturally carry their declared width.
fn align_stmt(args: &[Variant], right_align: bool) -> LibResult<Variant> {
    let width = string_units(need(args, 0)?)?.len();
    let mut value = string_units(need(args, 1)?)?;
    if value.len() > width {
        value.truncate(width);
    }
    let pad = width.saturating_sub(value.len());
    let mut out = Vec::with_capacity(width);
    if right_align {
        out.extend(std::iter::repeat(0x20).take(pad));
        out.extend(value);
    } else {
        out.extend(value);
        out.extend(std::iter::repeat(0x20).take(pad));
    }
    Ok(Variant::from_utf16_units(&out))
}

fn one_based_mid_start(value: &Variant) -> LibResult<usize> {
    let start = as_i64(value)?;
    if start < 1 {
        return Err(LibError::invalid_call(
            "Mid start must be greater than or equal to 1",
        ));
    }
    usize::try_from(start).map_err(|_| LibError::overflow("Mid start overflow"))
}

/// `InStr([start], string1, string2, [compare])` and
/// `InStrRev(string1, string2, [compare])`.
///
/// For `InStr`, an optional leading numeric-typed `start` (1-based) is detected
/// by argument type — the legacy 2-operand form passes a string-typed
/// `string1`, so this is unambiguous. The trailing compare mode (0 = binary,
/// 1 = text → ASCII case-insensitive) is supplied by the lowering from the
/// source's `Option Compare`. (`InStrRev`'s own `start` argument awaits the
/// canonical arg layout.)
pub fn instr(args: &[Variant], rev: bool) -> LibResult<Variant> {
    if rev {
        return instr_rev(args);
    }
    // `InStr([start], string1, string2, [compare])`. Per VBA `start` is detected by ARITY,
    // not by type: 2 args = `(string1, string2)`; 3–4 args = `(start, string1, string2,
    // [compare])` — `compare` is legal only when `start` is present. (The old code treated a
    // numeric first arg as `start`, mis-binding e.g. `InStr(numericVar, s)`.)
    let has_start = args.len() >= 3;
    let base = usize::from(has_start);
    let start = if has_start {
        as_i32(need(args, 0)?)?
    } else {
        1
    };
    if start < 1 {
        return Err(LibError::invalid_call("InStr start must be >= 1"));
    }
    let text = text_compare(args, base + 2)?;
    let hay = norm_compare(as_str(need(args, base)?)?, text);
    let needle = norm_compare(as_str(need(args, base + 1)?)?, text);

    let byte_start = utf16_index_to_byte(&hay, start as usize - 1);
    if byte_start > hay.len() {
        return Ok(vi32(0));
    }
    let pos = hay[byte_start..].find(&needle).map(|b| byte_start + b);
    Ok(vi32(match pos {
        Some(byte) => hay[..byte].encode_utf16().count() as i32 + 1,
        None => 0,
    }))
}

/// `InStrRev(stringcheck, stringmatch, [start = -1], [compare])` — the position (1-based, from
/// the START of the string) of the LAST occurrence of `stringmatch` whose match lies at or
/// before `start`; `-1` searches from the end. NOTE the arg layout differs from `InStr`:
/// `start` is the 3rd arg and `compare` the 4th (the old code ignored `start` and misread
/// arg 2 as the compare mode).
fn instr_rev(args: &[Variant]) -> LibResult<Variant> {
    let text = text_compare(args, 3)?;
    let hay = norm_compare(as_str(need(args, 0)?)?, text);
    let needle = norm_compare(as_str(need(args, 1)?)?, text);
    let hay_units = hay.encode_utf16().count();
    let start = match arg_present(args, 2) {
        Some(v) => as_i32(v)?,
        None => -1,
    };
    if start == 0 || start < -1 {
        return Err(LibError::invalid_call("InStrRev start must be -1 or >= 1"));
    }
    // Search the first `start` code units (the match must end at or before `start`).
    let region_units = if start == -1 {
        hay_units
    } else {
        (start as usize).min(hay_units)
    };
    let region_end = utf16_index_to_byte(&hay, region_units);
    let region = &hay[..region_end];
    Ok(vi32(match region.rfind(&needle) {
        Some(byte) => hay[..byte].encode_utf16().count() as i32 + 1,
        None => 0,
    }))
}

/// Byte offset of the char at a given UTF-16 code-unit index (VBA string index).
fn utf16_index_to_byte(s: &str, utf16_index: usize) -> usize {
    let mut units = 0;
    for (byte, ch) in s.char_indices() {
        if units >= utf16_index {
            return byte;
        }
        units += ch.len_utf16();
    }
    s.len()
}

pub fn lcase(args: &[Variant]) -> LibResult<Variant> {
    Ok(vstr(as_str(need(args, 0)?)?.to_lowercase()))
}
pub fn ucase(args: &[Variant]) -> LibResult<Variant> {
    Ok(vstr(as_str(need(args, 0)?)?.to_uppercase()))
}

/// `Split(expression, [delimiter = " "], [limit = -1], [compare])`. `limit` caps the number
/// of substrings (the last element holds the unsplit remainder); `compare` chooses how the
/// delimiter is matched (binary vs. case-insensitive text). Both were previously ignored.
pub fn split(args: &[Variant]) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let delim = match opt(args, 1) {
        Some(v) => as_str(v)?,
        None => " ".to_string(),
    };
    let limit = match opt(args, 2) {
        Some(v) => as_i32(v)?,
        None => -1,
    };
    let text = text_compare(args, 3)?;
    let parts: Vec<Variant> = if delim.is_empty() {
        // An empty delimiter never splits — the whole string is the single element.
        vec![vstr(s)]
    } else if limit == 0 {
        // A zero limit returns an empty (`LBound 0, UBound -1`) array.
        Vec::new()
    } else {
        split_with_limit(&s, &delim, limit, text)
    };
    Ok(Variant::from_safearray(SafeArray::from_variants(parts)))
}

/// Split `s` on `delim` into at most `limit` parts (negative = unlimited), matching the
/// delimiter case-insensitively when `text`. Substrings are taken from the ORIGINAL string
/// (original case preserved); only the delimiter *positions* are found case-insensitively.
fn split_with_limit(s: &str, delim: &str, limit: i32, text: bool) -> Vec<Variant> {
    // For binary compare `hay`/`needle` equal the originals, so byte offsets are exact; for
    // text compare they are the lowercased forms (byte-length-preserving for ASCII and most
    // text — the same approximation `InStr`/`norm_compare` already make).
    let hay = norm_compare(s.to_string(), text);
    let needle = norm_compare(delim.to_string(), text);
    let mut parts: Vec<Variant> = Vec::new();
    let mut cursor = 0usize;
    loop {
        // Once `limit - 1` parts are emitted, the final element holds the whole remainder.
        if limit >= 0 && parts.len() as i32 == limit - 1 {
            break;
        }
        match hay[cursor..].find(&needle) {
            Some(rel) => {
                let pos = cursor + rel;
                parts.push(vstr(s[cursor..pos].to_string()));
                cursor = pos + needle.len();
            }
            None => break,
        }
    }
    parts.push(vstr(s[cursor..].to_string()));
    parts
}

/// `Filter(source, match, [include=True], [compare=binary])` — the subset of a
/// string array whose elements contain (or, when `include` is False, omit) the
/// `match` substring, returned as a fresh 0-based array.
pub fn filter(args: &[Variant]) -> LibResult<Variant> {
    let array = need(args, 0)?
        .as_safearray()
        .ok_or_else(|| LibError::type_mismatch("Filter expects a source array"))?;
    let needle = as_str(need(args, 1)?)?;
    let include = match arg_present(args, 2) {
        Some(v) => as_bool_lenient(v),
        None => true,
    };
    let text = text_compare(args, 3)?;
    let needle_cmp = norm_compare(needle, text);
    let mut out: Vec<Variant> = Vec::new();
    for element in array.variant_elements().unwrap_or_default() {
        let s = as_str(&element)?;
        let contains = norm_compare(s, text).contains(&needle_cmp);
        if contains == include {
            out.push(element);
        }
    }
    Ok(Variant::from_safearray(SafeArray::from_variants(out)))
}

pub fn join(args: &[Variant]) -> LibResult<Variant> {
    let array = need(args, 0)?
        .as_safearray()
        .ok_or_else(|| LibError::type_mismatch("Join expects an array"))?;
    let delim = match opt(args, 1) {
        Some(v) => as_str(v)?,
        None => " ".to_string(),
    };
    let parts = array.variant_elements().unwrap_or_default();
    let texts: LibResult<Vec<String>> = parts.iter().map(as_str).collect();
    Ok(vstr(texts?.join(&delim)))
}

/// `Replace(expr, find, replace, [start], [count], [compare])`. Per VBA the
/// result is the substring of `expr` FROM `start` (1-based; the prefix before
/// `start` is dropped), with up to `count` replacements (default -1 = all) of
/// `find` by `replace`, matched binary or text per `compare` (default binary).
pub fn replace(args: &[Variant]) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let find = as_str(need(args, 1)?)?;
    let with = as_str(need(args, 2)?)?;
    let start = match numeric_arg(args, 3) {
        Some(v) => as_i32(v)?,
        None => 1,
    };
    let count = match numeric_arg(args, 4) {
        Some(v) => as_i32(v)?,
        None => -1,
    };
    let text = text_compare(args, 5)?;
    if start < 1 || count < -1 {
        // VBA "Invalid procedure call or argument" (error 5).
        return Err(LibError::invalid_call("Replace start/count out of range"));
    }

    // The result begins at `start` (1-based); a start past the end yields "".
    let chars: Vec<char> = s.chars().collect();
    let from = (start - 1) as usize;
    if from >= chars.len() {
        return Ok(vstr(String::new()));
    }
    let hay: Vec<char> = chars[from..].to_vec();

    if find.is_empty() || count == 0 {
        return Ok(vstr(hay.into_iter().collect::<String>()));
    }

    let needle: Vec<char> = find.chars().collect();
    let needle_cmp: Vec<char> = norm_compare(find.clone(), text).chars().collect();
    let limit = if count < 0 {
        usize::MAX
    } else {
        count as usize
    };
    let mut out = String::new();
    let mut i = 0usize;
    let mut done = 0usize;
    while i < hay.len() {
        let window_matches = done < limit
            && i + needle.len() <= hay.len()
            && hay[i..i + needle.len()]
                .iter()
                .zip(&needle_cmp)
                .all(|(h, n)| {
                    if text {
                        h.to_ascii_lowercase() == *n
                    } else {
                        h == n
                    }
                });
        if window_matches {
            out.push_str(&with);
            i += needle.len();
            done += 1;
        } else {
            out.push(hay[i]);
            i += 1;
        }
    }
    Ok(vstr(out))
}

pub fn trim(args: &[Variant], left: bool, right: bool) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let t = match (left, right) {
        (true, true) => s.trim_matches(' '),
        (true, false) => s.trim_start_matches(' '),
        (false, true) => s.trim_end_matches(' '),
        (false, false) => &s,
    };
    Ok(vstr(t.to_string()))
}

/// `StrComp(s1, s2, [compare])` — optional trailing compare mode (0=binary, 1=text).
pub fn str_comp(args: &[Variant]) -> LibResult<Variant> {
    let text = text_compare(args, 2)?;
    let a = norm_compare_units(compare_units(need(args, 0)?)?, text);
    let b = norm_compare_units(compare_units(need(args, 1)?)?, text);
    Ok(vi32(match a.cmp(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }))
}

/// `Like` — supports `?`, `*`, `#`, `[charlist]` (with `!` negation and `a-z`
/// ranges), and an optional trailing compare mode (0=binary, 1=text).
pub fn like(args: &[Variant]) -> LibResult<Variant> {
    let text = text_compare(args, 2)?;
    let s: Vec<char> = norm_compare(as_str(need(args, 0)?)?, text)
        .chars()
        .collect();
    let p: Vec<char> = norm_compare(as_str(need(args, 1)?)?, text)
        .chars()
        .collect();
    Ok(vbool(like_match(&s, &p)))
}

fn like_match(s: &[char], p: &[char]) -> bool {
    if p.is_empty() {
        return s.is_empty();
    }
    match p[0] {
        '*' => (0..=s.len()).any(|i| like_match(&s[i..], &p[1..])),
        '?' => !s.is_empty() && like_match(&s[1..], &p[1..]),
        '#' => !s.is_empty() && s[0].is_ascii_digit() && like_match(&s[1..], &p[1..]),
        '[' => match charlist_end(&p[1..]) {
            // p[1..1+end] is the charlist body; p[1+end] is the closing ']'.
            Some(end) => {
                !s.is_empty()
                    && char_in_charlist(s[0], &p[1..1 + end])
                    && like_match(&s[1..], &p[end + 2..])
            }
            None => !s.is_empty() && s[0] == '[' && like_match(&s[1..], &p[1..]),
        },
        c => !s.is_empty() && s[0] == c && like_match(&s[1..], &p[1..]),
    }
}

/// Index (within `body`, the chars after the opening `[`) of the closing `]`. A
/// `]` that is the first member (optionally after a leading `!`) is literal.
fn charlist_end(body: &[char]) -> Option<usize> {
    let mut i = 0;
    if body.first() == Some(&'!') {
        i += 1;
    }
    if body.get(i) == Some(&']') {
        i += 1;
    }
    while i < body.len() {
        if body[i] == ']' {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// `[charlist]` membership: a leading `!` negates; `a-z` denotes a range.
fn char_in_charlist(c: char, body: &[char]) -> bool {
    let (negate, body) = match body.split_first() {
        Some((&'!', rest)) => (true, rest),
        _ => (false, body),
    };
    let mut i = 0;
    let mut found = false;
    while i < body.len() {
        if i + 2 < body.len() && body[i + 1] == '-' {
            let (lo, hi) = (body[i], body[i + 2]);
            if (lo <= c && c <= hi) || (hi <= c && c <= lo) {
                found = true;
            }
            i += 3;
        } else {
            if body[i] == c {
                found = true;
            }
            i += 1;
        }
    }
    found != negate
}

/// Shared code-point decode for `Chr`/`ChrW`: the argument is a VBA `Long` whose
/// low 16 bits are the (wide) character code; negatives wrap via `code as u16`.
/// Rejects the UTF-16 surrogate range and otherwise returns the `char`.
fn wide_char(code: i32) -> LibResult<char> {
    if !(-32768..=65535).contains(&code) {
        return Err(LibError::invalid_call("invalid character code"));
    }
    let value = code as u16;
    char::from_u32(u32::from(value)).ok_or_else(|| LibError::invalid_call("invalid character code"))
}

/// `ChrW(code)` — genuine WIDE: the single UTF-16 CODE UNIT whose value is the
/// argument's low 16 bits (negatives wrap). Any `0..=65535` is valid — INCLUDING a
/// lone surrogate half `0xD800..=0xDFFF` — because a VBA string is a UTF-16 code-unit
/// sequence, not Unicode scalar values. Building the string directly from the unit
/// (not via `char`) is what lets `ChrW(&HD83D) & ChrW(&HDE00)` form a real astral pair.
pub fn chr_w(args: &[Variant]) -> LibResult<Variant> {
    let code = as_i32(need(args, 0)?)?;
    if !(-32768..=65535).contains(&code) {
        return Err(LibError::invalid_call("invalid character code"));
    }
    Ok(Variant::from_utf16_units(&[code as u16]))
}

/// `Chr(code)` — genuine ANSI: codes 0..=255 decode through the live system ANSI
/// code page (`CP_ACP` on Windows, the CP-1252 fallback off-Windows); codes
/// 256..=65535 act WIDE (VBA7 `Chr` is wide above 255); negatives wrap.
pub fn chr(args: &[Variant]) -> LibResult<Variant> {
    let code = as_i32(need(args, 0)?)?;
    let value = if (-32768..=65535).contains(&code) {
        code as u16
    } else {
        return Err(LibError::invalid_call("invalid character code"));
    };
    let ch = if value <= 0xFF {
        // A single ANSI byte decodes to one Unicode char via the shared codec; fall
        // back to the byte's own code point if the host ACP leaves it undefined.
        oxvba_runtime::ansi::ansi_decode(&[value as u8])
            .chars()
            .next()
            .unwrap_or(value as u8 as char)
    } else {
        wide_char(code)?
    };
    Ok(vstr(ch.to_string()))
}

/// `AscW(s)` — genuine WIDE: the first char's Unicode code point, returned as a VBA
/// `Integer` (i16) so code points > 32767 come back negative (matching VBA AscW).
pub fn asc_w(args: &[Variant]) -> LibResult<Variant> {
    let value = need(args, 0)?;
    // Read the first UTF-16 CODE UNIT directly (so `AscW(ChrW(n))` round-trips ANY
    // 0..=65535, including a lone surrogate half a `char`-based read could not carry).
    // The i16 wrap matches VBA: code points > 32767 come back negative.
    let unit = match value.string_units() {
        Some(units) => units.first().copied(),
        None => as_str(value)?.encode_utf16().next(),
    }
    .ok_or_else(|| LibError::invalid_call("AscW of empty string"))?;
    Ok(vi32(i32::from(unit as i16)))
}

/// `Asc(s)` — genuine ANSI: the first char's byte in the live system ANSI code page
/// (`CP_ACP` on Windows, the CP-1252 fallback off-Windows). For the common
/// Western/CP-1252 SBCS case this is the 0..=255 ANSI code; a char the host ACP
/// cannot represent yields 63 (`"?"`, VBA's unmappable best-fit). (DBCS lead-byte /
/// 2-byte `Asc` on a multibyte ACP is an accepted documented edge: we return the
/// first byte only.)
pub fn asc(args: &[Variant]) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let ch = s
        .chars()
        .next()
        .ok_or_else(|| LibError::invalid_call("Asc of empty string"))?;
    let byte = oxvba_runtime::ansi::ansi_encode(&ch.to_string())
        .first()
        .copied()
        .unwrap_or(b'?');
    Ok(vi32(i32::from(byte)))
}

pub fn space(args: &[Variant]) -> LibResult<Variant> {
    Ok(vstr(" ".repeat(alloc_count(need(args, 0)?)?)))
}

/// `String(number, character)` — `character` may be a code or a string.
pub fn string_repeat(args: &[Variant]) -> LibResult<Variant> {
    let count = alloc_count(need(args, 0)?)?;
    let arg = need(args, 1)?;
    let ch = if let Ok(code) = as_i32(arg) {
        let code = if code > 255 { code % 256 } else { code };
        char::from_u32(code as u32).unwrap_or(' ')
    } else {
        as_str(arg)?.chars().next().unwrap_or(' ')
    };
    Ok(vstr(ch.to_string().repeat(count)))
}

pub fn str_reverse(args: &[Variant]) -> LibResult<Variant> {
    Ok(vstr(
        as_str(need(args, 0)?)?.chars().rev().collect::<String>(),
    ))
}

/// FIDELITY: handles case modes plus the deterministic ANSI byte conversion modes
/// through the shared runtime ANSI codec. East Asian width/kana modes still pass
/// through outside relevant locales.
pub fn str_conv(args: &[Variant]) -> LibResult<Variant> {
    let mode = as_i32(need(args, 1)?)?;
    const VB_UPPER_CASE: i32 = 1;
    const VB_LOWER_CASE: i32 = 2;
    const VB_PROPER_CASE: i32 = 3;
    const VB_UNICODE: i32 = 64;
    const VB_FROM_UNICODE: i32 = 128;

    if mode & VB_UNICODE != 0 && mode & VB_FROM_UNICODE != 0 {
        return Err(LibError::invalid_call(
            "vbUnicode and vbFromUnicode are mutually exclusive",
        ));
    }

    let mut s = if mode & VB_UNICODE != 0 {
        let input = need(args, 0)?;
        if let Some(array) = input.as_safearray() {
            let bytes = array_bytes(&array)?;
            oxvba_runtime::ansi::ansi_decode(&bytes)
        } else {
            let text = as_str(input)?;
            oxvba_runtime::ansi::ansi_decode(&oxvba_runtime::ansi::ansi_encode(&text))
        }
    } else {
        as_str(need(args, 0)?)?
    };

    s = match mode & VB_PROPER_CASE {
        VB_UPPER_CASE => s.to_uppercase(),
        VB_LOWER_CASE => s.to_lowercase(),
        VB_PROPER_CASE => proper_case(&s),
        _ => s,
    };

    if mode & VB_FROM_UNICODE != 0 {
        let bytes = oxvba_runtime::ansi::ansi_encode(&s)
            .into_iter()
            .map(Variant::from_u8)
            .collect();
        let array =
            SafeArray::from_typed_variants(VT_UI1_VALUE, bytes).map_err(LibError::type_mismatch)?;
        return Ok(Variant::from_safearray(array));
    }

    Ok(vstr(s))
}

fn proper_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut cs = w.chars();
            match cs.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn array_bytes(array: &SafeArray) -> LibResult<Vec<u8>> {
    array
        .variant_elements()
        .ok_or_else(|| LibError::type_mismatch("expected Byte array"))?
        .into_iter()
        .map(|value| {
            value
                .as_u8()
                .ok_or_else(|| LibError::type_mismatch("expected Byte array"))
        })
        .collect()
}

/// `Format(expr, [mask])` — delegates to the [`crate::format`] engine (named
/// formats, custom numeric masks, custom date/time masks). No mask (or an empty
/// mask) → string passthrough (`CStr`).
pub fn format(args: &[Variant]) -> LibResult<Variant> {
    let value = need(args, 0)?;
    let mask = match opt(args, 1) {
        Some(v) if !matches!(v.vtype(), VarType::Empty) => as_str(v)?,
        _ => return cstr(args),
    };
    if mask.is_empty() {
        return cstr(args);
    }
    Ok(vstr(crate::format::apply(value, &mask)))
}

pub fn format_number(args: &[Variant]) -> LibResult<Variant> {
    let value = need(args, 0)?;
    let options = fixed_format_options(args)?;
    Ok(vstr(crate::format::format_number(
        value,
        options.decimals,
        options.include_leading_digit,
        options.parens_for_negative,
        options.group_digits,
    )))
}

pub fn format_currency(args: &[Variant]) -> LibResult<Variant> {
    let value = need(args, 0)?;
    let options = fixed_format_options(args)?;
    Ok(vstr(crate::format::format_currency(
        value,
        options.decimals,
        options.include_leading_digit,
        options.parens_for_negative,
        options.group_digits,
    )))
}

pub fn format_percent(args: &[Variant]) -> LibResult<Variant> {
    let value = need(args, 0)?;
    let options = fixed_format_options(args)?;
    Ok(vstr(crate::format::format_percent(
        value,
        options.decimals,
        options.include_leading_digit,
        options.parens_for_negative,
        options.group_digits,
    )))
}

pub fn format_date_time(args: &[Variant]) -> LibResult<Variant> {
    let value = need(args, 0)?;
    let named_format = match opt(args, 1) {
        Some(value) if !matches!(value.vtype(), VarType::Empty) => as_i32(value)?,
        _ => 0,
    };
    crate::format::format_date_time(value, named_format)
        .map(vstr)
        .ok_or_else(|| LibError::invalid_call("FormatDateTime named format must be 0 to 4"))
}

struct FixedFormatOptions {
    decimals: usize,
    include_leading_digit: bool,
    parens_for_negative: bool,
    group_digits: bool,
}

fn fixed_format_options(args: &[Variant]) -> LibResult<FixedFormatOptions> {
    Ok(FixedFormatOptions {
        decimals: fixed_decimals(args)?,
        include_leading_digit: tri_state_arg(args, 2, true)?,
        parens_for_negative: tri_state_arg(args, 3, false)?,
        group_digits: tri_state_arg(args, 4, true)?,
    })
}

fn fixed_decimals(args: &[Variant]) -> LibResult<usize> {
    let value = match opt(args, 1) {
        Some(value) if !matches!(value.vtype(), VarType::Empty) => as_i32(value)?,
        _ => -1,
    };
    match value {
        -1 => Ok(2),
        0..=99 => Ok(value as usize),
        _ => Err(LibError::invalid_call(
            "NumDigitsAfterDecimal must be -1 or 0 to 99",
        )),
    }
}

fn tri_state_arg(args: &[Variant], index: usize, default: bool) -> LibResult<bool> {
    let value = match opt(args, index) {
        Some(value) if !matches!(value.vtype(), VarType::Empty) => as_i32(value)?,
        _ => -2,
    };
    match value {
        -2 => Ok(default),
        -1 => Ok(true),
        0 => Ok(false),
        _ => Err(LibError::invalid_call(
            "TriState argument must be -2, -1, or 0",
        )),
    }
}

// ── Math ──────────────────────────────────────────────────────────────────────

pub fn math1(args: &[Variant], f: impl Fn(f64) -> f64) -> LibResult<Variant> {
    Ok(vf64(f(as_f64(need(args, 0)?)?)))
}

pub fn sqr(args: &[Variant]) -> LibResult<Variant> {
    let x = as_f64(need(args, 0)?)?;
    if x < 0.0 {
        return Err(LibError::invalid_call("Sqr argument must be non-negative"));
    }
    Ok(vf64(x.sqrt()))
}

pub fn log(args: &[Variant]) -> LibResult<Variant> {
    let x = as_f64(need(args, 0)?)?;
    if x <= 0.0 {
        return Err(LibError::invalid_call("Log argument must be positive"));
    }
    Ok(vf64(x.ln()))
}

pub fn exp(args: &[Variant]) -> LibResult<Variant> {
    let value = as_f64(need(args, 0)?)?.exp();
    if !value.is_finite() {
        return Err(LibError::overflow("Exp overflow"));
    }
    Ok(vf64(value))
}

/// `Abs` / `Int` / `Fix` — the three "type-preserving" unary math functions.
/// Unlike the transcendentals they return the argument's own numeric subtype
/// (so `Int(CCur(3.7))` is a `Currency`, `Fix(CDate(...))` is a `Date`), and
/// `Abs` *promotes* on overflow rather than wrapping (`Abs(CInt(-32768))` is a
/// `Long`, `Abs(CLng(&H80000000))` is a `Double`). Empty → `Integer` 0, Null →
/// Null, Boolean → `Integer`, a numeric String → `Double`. Verified against
/// live Office VBA 7.1.
#[derive(Clone, Copy)]
enum Math1Op {
    Abs,
    Int,
    Fix,
}

pub fn abs(args: &[Variant]) -> LibResult<Variant> {
    math1_typed(need(args, 0)?, Math1Op::Abs)
}
pub fn int_floor(args: &[Variant]) -> LibResult<Variant> {
    math1_typed(need(args, 0)?, Math1Op::Int)
}
pub fn fix_trunc(args: &[Variant]) -> LibResult<Variant> {
    math1_typed(need(args, 0)?, Math1Op::Fix)
}

/// `Sgn` — the sign of the argument as an `Integer` (-1, 0, 1), regardless of
/// the input type. `Null` raises error 94 (the result is `Integer`, which
/// cannot hold `Null`); `Empty` is 0.
pub fn sgn(args: &[Variant]) -> LibResult<Variant> {
    let v = need(args, 0)?;
    if v.vtype() == VarType::Null {
        return Err(LibError::new(94, "invalid use of Null"));
    }
    let x = conv_f64(v)?;
    if x.is_nan() {
        return Err(LibError::invalid_call("Sgn argument must not be NaN"));
    }
    let s: i16 = if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    };
    Ok(Variant::from_i16(s))
}

fn float_op(op: Math1Op, x: f64) -> f64 {
    match op {
        Math1Op::Abs => x.abs(),
        Math1Op::Int => x.floor(),
        Math1Op::Fix => x.trunc(),
    }
}

/// `Abs` of a signed integer, promoting to the next wider type when the
/// magnitude overflows the source type (VBA never wraps or errors here):
/// SignedByte→Integer, Integer→Long, Long→Double, LongLong→Double.
fn abs_signed_promote(x: i64, kind: VarType) -> Variant {
    let mag = x.unsigned_abs();
    match kind {
        VarType::SignedByte if mag <= i8::MAX as u64 => Variant::from_i8(mag as i8),
        VarType::SignedByte => Variant::from_i16(mag as i16),
        VarType::Integer if mag <= i16::MAX as u64 => Variant::from_i16(mag as i16),
        VarType::Integer => Variant::from_i32(mag as i32),
        VarType::Long if mag <= i32::MAX as u64 => Variant::from_i32(mag as i32),
        VarType::Long => Variant::from_f64(mag as f64),
        VarType::LongLong if mag <= i64::MAX as u64 => Variant::from_i64(mag as i64),
        VarType::LongLong => Variant::from_f64(mag as f64),
        _ => Variant::from_f64(mag as f64),
    }
}

/// `Abs`/`Int`/`Fix` of a `Currency` (a fixed 4-dp scaled `i64`), keeping the
/// `Currency` type. `Int` floors toward −∞, `Fix` truncates toward zero.
fn currency_op(op: Math1Op, scaled: i64) -> LibResult<Variant> {
    let result = match op {
        Math1Op::Abs => scaled
            .checked_abs()
            .ok_or_else(|| LibError::overflow("Abs overflows Currency"))?,
        Math1Op::Fix => (scaled / 10_000) * 10_000,
        Math1Op::Int => {
            let q = scaled / 10_000;
            let rem = scaled % 10_000;
            let q = if rem != 0 && scaled < 0 { q - 1 } else { q };
            q * 10_000
        }
    };
    Ok(Variant::from_currency_scaled_i64(result))
}

/// `Abs`/`Int`/`Fix` of a `Decimal`, keeping the `Decimal` type. `Abs` clears
/// the sign; `Int`/`Fix` drop the fractional digits (scale → 0), with `Int`
/// flooring toward −∞ (a negative with a fraction rounds away from zero).
fn decimal_op(op: Math1Op, d: oxvba_runtime::Decimal96) -> Variant {
    use oxvba_runtime::Decimal96;
    match op {
        Math1Op::Abs => {
            Variant::from_decimal96(Decimal96::from_parts(d.lo, d.mid, d.hi, d.scale(), false))
        }
        Math1Op::Int | Math1Op::Fix => {
            let scale = u32::from(d.scale());
            if scale == 0 {
                return Variant::from_decimal96(d);
            }
            let magnitude = d.magnitude_u128();
            let divisor = 10u128.pow(scale.min(38));
            let mut whole = magnitude / divisor;
            let remainder = magnitude % divisor;
            if matches!(op, Math1Op::Int) && d.is_negative() && remainder != 0 {
                whole += 1; // floor of a negative rounds away from zero
            }
            let lo = (whole & 0xFFFF_FFFF) as u32;
            let mid = ((whole >> 32) & 0xFFFF_FFFF) as u32;
            let hi = ((whole >> 64) & 0xFFFF_FFFF) as u32;
            Variant::from_decimal96(Decimal96::from_parts(lo, mid, hi, 0, d.is_negative()))
        }
    }
}

fn math1_typed(v: &Variant, op: Math1Op) -> LibResult<Variant> {
    match v.vtype() {
        VarType::Null => Ok(Variant::null()),
        VarType::Empty => Ok(Variant::from_i16(0)),
        VarType::Boolean => {
            let n: i16 = if v.as_bool().unwrap_or(false) { -1 } else { 0 };
            Ok(Variant::from_i16(match op {
                Math1Op::Abs => n.abs(),
                Math1Op::Int | Math1Op::Fix => n,
            }))
        }
        // Signed integers: Int/Fix are identity; Abs may promote.
        kind @ (VarType::SignedByte | VarType::Integer | VarType::Long | VarType::LongLong) => {
            match op {
                Math1Op::Int | Math1Op::Fix => Ok(v.clone()),
                Math1Op::Abs => {
                    let x = match kind {
                        VarType::SignedByte => i64::from(v.as_i8().unwrap_or(0)),
                        VarType::Integer => i64::from(v.as_i16().unwrap_or(0)),
                        VarType::Long => i64::from(v.as_i32().unwrap_or(0)),
                        _ => v.as_i64().unwrap_or(0),
                    };
                    Ok(abs_signed_promote(x, kind))
                }
            }
        }
        // Unsigned integers are non-negative whole numbers: Abs/Int/Fix all identity.
        VarType::Byte
        | VarType::UnsignedInteger
        | VarType::UnsignedLong
        | VarType::UnsignedInt
        | VarType::UnsignedLongLong => Ok(v.clone()),
        VarType::Single => Ok(Variant::from_f32(
            float_op(op, f64::from(v.as_f32().unwrap_or(0.0))) as f32,
        )),
        VarType::Double => Ok(Variant::from_f64(float_op(op, v.as_f64().unwrap_or(0.0)))),
        VarType::Date => Ok(Variant::from_date_f64(float_op(
            op,
            v.as_date_f64().unwrap_or(0.0),
        ))),
        VarType::Currency => currency_op(op, v.as_currency_scaled_i64().unwrap_or(0)),
        VarType::Decimal => Ok(decimal_op(op, v.as_decimal96().unwrap_or_default())),
        // A numeric String coerces to Double; everything else (Object/Error/…) is a Type mismatch.
        VarType::String => Ok(vf64(float_op(op, conv_f64(v)?))),
        _ => Err(LibError::type_mismatch("Abs/Int/Fix expects a number")),
    }
}

/// Banker's rounding (round-half-to-even), like VBA `Round`.
pub fn round(args: &[Variant]) -> LibResult<Variant> {
    let x = as_f64(need(args, 0)?)?;
    let digits = match opt(args, 1) {
        Some(v) => as_i32(v)?,
        None => 0,
    };
    let factor = 10f64.powi(digits);
    let scaled = x * factor;
    let rounded = scaled.round_ties_even();
    Ok(vf64(rounded / factor))
}

// ── Date / time (serial: days since 1899-12-30) ────────────────────────────────
// Proleptic Gregorian, matching VBA's OLE-Automation `Date`. (The Excel
// worksheet 1900 leap-year quirk is not part of VBA `Date` semantics.) The serial
// ↔ civil math lives in `oxvba_runtime::vba_date` (the single source of truth, so
// the runtime's `Date` display formatters share it); imported above.

/// Sakamoto's algorithm (ported from the legacy VM): 0 = Sunday … 6 = Saturday.
pub(crate) fn day_of_week(year: i32, month: u32, day: u32) -> i32 {
    let table = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let m = month as i32;
    let y = if m < 3 { year - 1 } else { year };
    (y + y / 4 - y / 100 + y / 400 + table[(m - 1) as usize] + day as i32).rem_euclid(7)
}

fn validate_vba_date_serial(serial: f64) -> LibResult<f64> {
    if serial.is_finite() && (VBA_DATE_MIN_SERIAL..VBA_DATE_MAX_EXCLUSIVE_SERIAL).contains(&serial)
    {
        Ok(serial)
    } else {
        Err(LibError::invalid_call("date out of range"))
    }
}

fn validate_dateserial_arg(value: i64) -> LibResult<i64> {
    if (-32_768..=32_767).contains(&value) {
        Ok(value)
    } else {
        Err(LibError::invalid_call("DateSerial argument out of range"))
    }
}

fn date_day_index(serial: f64) -> i64 {
    serial.trunc() as i64
}

fn day_index_weekday(day: i64) -> i32 {
    let (y, m, d) = serial_to_ymd(day as f64);
    day_of_week(y as i32, m as u32, d as u32)
}

fn count_weekday_boundaries_forward(start_day: i64, end_day: i64, target_dow: i32) -> i64 {
    if end_day <= start_day {
        return 0;
    }
    let first_candidate = start_day + 1;
    let offset = (target_dow - day_index_weekday(first_candidate)).rem_euclid(7) as i64;
    let first_hit = first_candidate + offset;
    if first_hit > end_day {
        0
    } else {
        1 + (end_day - first_hit) / 7
    }
}

fn count_weekday_boundaries(start_day: i64, end_day: i64, target_dow: i32) -> i64 {
    if end_day >= start_day {
        count_weekday_boundaries_forward(start_day, end_day, target_dow)
    } else {
        -count_weekday_boundaries_forward(end_day, start_day, target_dow)
    }
}

pub fn date_serial(args: &[Variant]) -> LibResult<Variant> {
    let y = validate_dateserial_arg(as_i64(need(args, 0)?)?)?;
    let m = validate_dateserial_arg(as_i64(need(args, 1)?)?)?;
    let d = validate_dateserial_arg(as_i64(need(args, 2)?)?)?;
    Ok(Variant::from_date_f64(validate_vba_date_serial(
        ymd_to_serial(y, m, d),
    )?))
}

pub fn time_serial(args: &[Variant]) -> LibResult<Variant> {
    let h = validate_dateserial_arg(as_i64(need(args, 0)?)?)? as f64;
    let m = validate_dateserial_arg(as_i64(need(args, 1)?)?)? as f64;
    let s = validate_dateserial_arg(as_i64(need(args, 2)?)?)? as f64;
    Ok(Variant::from_date_f64(
        (h * 3600.0 + m * 60.0 + s) / 86400.0,
    ))
}

/// VBA `DateValue` returns the DATE-ONLY part of its argument. It accepts a Date or a numeric
/// serial (not just a string): `DateValue(Now)` is today at midnight. Strings parse through the
/// same lenient parser as `CDate`. The time-of-day is dropped by truncating the serial toward
/// zero (so a negative serial keeps its calendar day).
pub fn date_value(args: &[Variant]) -> LibResult<Variant> {
    let v = need(args, 0)?;
    let serial = match v.vtype() {
        VarType::Date => v.as_date_f64().unwrap_or(0.0),
        VarType::String => cdate_from_string(&as_str(v)?)?.as_date_f64().unwrap_or(0.0),
        _ => conv_f64(v)?,
    };
    Ok(Variant::from_date_f64(validate_vba_date_serial(
        serial.trunc(),
    )?))
}

/// VBA `TimeValue` returns the TIME-OF-DAY part of its argument. Like `DateValue` it accepts a
/// `Date` or a numeric serial (not just a string) — `TimeValue(Now)` is the current time — and
/// must NOT round-trip a `Date` through string formatting (which dropped precision and, once
/// dates render as "General Date", misparsed the formatted text). Strings parse as `h[:m[:s]]`.
pub fn time_value(args: &[Variant]) -> LibResult<Variant> {
    let v = need(args, 0)?;
    let serial = match v.vtype() {
        VarType::Date => v.as_date_f64().unwrap_or(0.0),
        VarType::String => parse_time_value_string(&as_str(v)?)?,
        VarType::Null => return Ok(Variant::null()),
        _ => conv_f64(v)?,
    };
    // Keep only the time-of-day fraction (drop the integer date part).
    Ok(Variant::from_date_f64(serial - serial.trunc()))
}

/// Whether `(y, m, d)` is a real calendar date. The month/day are normalized by
/// `days_from_civil` (Feb 30 rolls into March), so a round-trip through the serial that does
/// NOT return the same `(y, m, d)` means the input was an invalid date — matching VBA, where
/// `CDate("February 30, 2000")` raises error 13 and `IsDate(...)` of it is False.
fn ymd_is_valid(y: i64, m: i64, d: i64) -> bool {
    (1..=12).contains(&m) && d >= 1 && civil_from_days(days_from_civil(y, m, d)) == (y, m, d)
}

fn parse_date(s: &str) -> LibResult<(i64, i64, i64)> {
    let err = || LibError::type_mismatch(format!("cannot parse date `{s}`"));
    let (y, m, d) = parse_date_raw(s).ok_or_else(err)?;
    if !ymd_is_valid(y, m, d) {
        return Err(err());
    }
    Ok((y, m, d))
}

fn parse_time_value_string(s: &str) -> LibResult<f64> {
    let s = s.trim();
    let err = || LibError::type_mismatch(format!("cannot parse time `{s}`"));
    if s.is_empty() {
        return Err(err());
    }
    if let Some((date_part, time_part)) = split_trailing_time(s) {
        parse_date(date_part.trim())?;
        return parse_time_only(&time_part).ok_or_else(err);
    }
    parse_time_only(s).ok_or_else(err)
}

fn split_trailing_time(s: &str) -> Option<(&str, String)> {
    let (before, last) = s.trim().rsplit_once(' ')?;
    if last.eq_ignore_ascii_case("am") || last.eq_ignore_ascii_case("pm") {
        let (date_part, time_part) = before.trim_end().rsplit_once(' ')?;
        if time_part.contains(':') {
            return Some((date_part.trim(), format!("{time_part} {last}")));
        }
    } else if last.contains(':') {
        return Some((before.trim(), last.trim().to_string()));
    }
    None
}

fn parse_time_only(s: &str) -> Option<f64> {
    let mut text = s.trim();
    let mut meridian = None;
    let lower = text.to_ascii_lowercase();
    if lower.ends_with("am") || lower.ends_with("pm") {
        meridian = Some(&lower[lower.len() - 2..]);
        text = text[..text.len() - 2].trim_end();
    }

    let parts: Vec<&str> = text.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let mut h = parts[0].trim().parse::<i64>().ok()?;
    let m = parts[1].trim().parse::<i64>().ok()?;
    let sec = match parts.get(2) {
        Some(value) => value.trim().parse::<f64>().ok()?,
        None => 0.0,
    };
    if !(0..=59).contains(&m) || !(0.0..60.0).contains(&sec) {
        return None;
    }
    match meridian {
        Some("am") => {
            if !(1..=12).contains(&h) {
                return None;
            }
            if h == 12 {
                h = 0;
            }
        }
        Some("pm") => {
            if !(1..=12).contains(&h) {
                return None;
            }
            if h != 12 {
                h += 12;
            }
        }
        _ if !(0..=23).contains(&h) => return None,
        _ => {}
    }
    Some((h as f64 * 3600.0 + m as f64 * 60.0 + sec) / 86400.0)
}

/// Parse a date string into `(year, month, day)` WITHOUT validating the calendar (see
/// [`ymd_is_valid`]). Accepts ISO `YYYY-MM-DD`, US `M/D/YYYY`, and month-name forms.
fn parse_date_raw(s: &str) -> Option<(i64, i64, i64)> {
    let s = s.trim();
    // ISO `YYYY-MM-DD` (all-numeric; falls through to the month-name path otherwise).
    if let Some((y, rest)) = s.split_once('-')
        && let Some((m, d)) = rest.split_once('-')
        && let (Ok(y), Ok(m), Ok(d)) = (
            y.trim().parse::<i64>(),
            m.trim().parse::<i64>(),
            d.trim().parse::<i64>(),
        )
    {
        return Some((y, m, d));
    }
    // US `M/D/YYYY`.
    if let Some((m, rest)) = s.split_once('/')
        && let Some((d, y)) = rest.split_once('/')
        && let (Ok(m), Ok(d), Ok(y)) = (
            m.trim().parse::<i64>(),
            d.trim().parse::<i64>(),
            y.trim().parse::<i64>(),
        )
    {
        return Some((y, m, d));
    }
    // Month-name forms: `d mmm yyyy`, `mmm d, yyyy`, `dd mmmm yyyy`, … — a month name
    // plus a day and a year, in either order, space/comma separated.
    let mut month = None;
    let mut nums: Vec<i64> = Vec::new();
    for tok in s.split([' ', ',', '\t']).filter(|t| !t.is_empty()) {
        if let Some(m) = month_from_name(tok) {
            month = Some(m);
        } else if let Ok(n) = tok.trim().parse::<i64>() {
            nums.push(n);
        } else {
            return None;
        }
    }
    if let (Some(m), [a, b]) = (month, nums.as_slice()) {
        // The year is the value that can't be a day-of-month (> 31); the other is the
        // day. A 2-digit year uses VBA's window: 0-29 → 2000s, 30-99 → 1900s.
        let (day, year) = if *a > 31 { (*b, *a) } else { (*a, *b) };
        let year = match year {
            0..=29 => year + 2000,
            30..=99 => year + 1900,
            other => other,
        };
        return Some((year, m, day));
    }
    None
}

/// The 1-based month number for a month name or its 3-letter prefix (`Jan`,
/// `January`, `jan.`, …). Each English month's 3-letter prefix is unique.
fn month_from_name(tok: &str) -> Option<i64> {
    const ABBR: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let t = tok.trim().to_ascii_lowercase();
    ABBR.iter()
        .position(|a| t.starts_with(a))
        .map(|i| i as i64 + 1)
}

/// FIDELITY: supports day/week/hour/minute/second additions exactly; month/year
/// additions are calendar-correct but do not clamp end-of-month edge cases.
pub fn date_add(args: &[Variant]) -> LibResult<Variant> {
    let interval = as_str(need(args, 0)?)?.to_lowercase();
    let number = as_f64(need(args, 1)?)?;
    let serial = as_f64(need(args, 2)?)?;
    let result = match interval.as_str() {
        "yyyy" | "m" | "q" => {
            let (y, m, d) = serial_to_ymd(serial);
            let months = m - 1
                + match interval.as_str() {
                    "yyyy" => number as i64 * 12,
                    "q" => number as i64 * 3,
                    _ => number as i64,
                };
            let ny = y + months.div_euclid(12);
            let nm = months.rem_euclid(12) + 1;
            ymd_to_serial(ny, nm, d)
        }
        "d" | "y" | "w" => serial + number,
        "ww" => serial + number * 7.0,
        "h" => serial + number / 24.0,
        "n" => serial + number / 1440.0,
        "s" => serial + number / 86400.0,
        _ => {
            return Err(LibError::invalid_call(format!(
                "unknown DateAdd interval `{interval}`"
            )));
        }
    };
    Ok(Variant::from_date_f64(validate_vba_date_serial(result)?))
}

/// FIDELITY: difference in the unit; calendar-unit diffs are approximate.
pub fn date_diff(args: &[Variant]) -> LibResult<Variant> {
    let interval = as_str(need(args, 0)?)?.to_lowercase();
    let a = as_f64(need(args, 1)?)?;
    let b = as_f64(need(args, 2)?)?;
    let a_day = date_day_index(a);
    let b_day = date_day_index(b);
    let days = (b_day - a_day) as f64;
    let result = match interval.as_str() {
        "d" | "y" => days,
        "w" => {
            let start_weekday = day_index_weekday(a_day);
            count_weekday_boundaries(a_day, b_day, start_weekday) as f64
        }
        "ww" => {
            let first = first_day_of_week(args, 3)?;
            count_weekday_boundaries(a_day, b_day, first - 1) as f64
        }
        "h" => (b - a) * 24.0,
        "n" => (b - a) * 1440.0,
        "s" => (b - a) * 86400.0,
        "m" | "q" | "yyyy" => {
            let (ya, ma, _) = serial_to_ymd(a);
            let (yb, mb, _) = serial_to_ymd(b);
            let months = (yb - ya) * 12 + (mb - ma);
            match interval.as_str() {
                "yyyy" => (yb - ya) as f64,
                "q" => (months / 3) as f64,
                _ => months as f64,
            }
        }
        _ => {
            return Err(LibError::invalid_call(format!(
                "unknown DateDiff interval `{interval}`"
            )));
        }
    };
    Ok(vf64(result.trunc()))
}

#[derive(Clone, Copy)]
pub enum DatePart {
    Year,
    Month,
    Day,
    Weekday,
    Hour,
    Minute,
    Second,
}

pub fn date_part(args: &[Variant], part: DatePart) -> LibResult<Variant> {
    let serial = as_f64(need(args, 0)?)?;
    Ok(vi32(match part {
        DatePart::Year | DatePart::Month | DatePart::Day | DatePart::Weekday => {
            let (y, m, d) = serial_to_ymd(serial);
            match part {
                DatePart::Year => y as i32,
                DatePart::Month => m as i32,
                DatePart::Day => d as i32,
                DatePart::Weekday => {
                    let first = first_day_of_week(args, 1)?;
                    weekday_number(y as i32, m as u32, d as u32, first)
                }
                _ => unreachable!(),
            }
        }
        DatePart::Hour | DatePart::Minute | DatePart::Second => {
            let (h, mi, s) = serial_to_hms(serial);
            match part {
                DatePart::Hour => h as i32,
                DatePart::Minute => mi as i32,
                DatePart::Second => s as i32,
                _ => unreachable!(),
            }
        }
    }))
}

fn first_day_of_week(args: &[Variant], index: usize) -> LibResult<i32> {
    let first = match opt(args, index) {
        Some(v) => as_i32(v)?,
        None => 1,
    };
    let first = if first == 0 { 1 } else { first };
    if !(1..=7).contains(&first) {
        return Err(LibError::invalid_call("firstdayofweek out of range"));
    }
    Ok(first)
}

fn weekday_number(year: i32, month: u32, day: u32, first: i32) -> i32 {
    let dow0 = day_of_week(year, month, day);
    (dow0 - (first - 1)).rem_euclid(7) + 1
}

pub fn month_name(args: &[Variant]) -> LibResult<Variant> {
    const NAMES: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let n = as_usize(need(args, 0)?)?;
    let abbreviate = matches!(opt(args, 1), Some(v) if as_bool_lenient(v));
    NAMES
        .get(n.wrapping_sub(1))
        .map(|s| vstr(if abbreviate { &s[..3] } else { s }))
        .ok_or_else(|| LibError::invalid_call("MonthName index out of range"))
}

/// `WeekdayName(weekday, [abbreviate], [firstdayofweek])` — the weekday is
/// relative to `firstdayofweek` (default vbSunday = 1), 1-based.
pub fn weekday_name(args: &[Variant]) -> LibResult<Variant> {
    const NAMES: [&str; 7] = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    let weekday = as_i32(need(args, 0)?)?;
    let abbreviate = matches!(opt(args, 1), Some(v) if as_bool_lenient(v));
    // firstdayofweek: 0 = vbUseSystemDayOfWeek (treat as Sunday), default vbSunday.
    let first = match opt(args, 2) {
        Some(v) => as_i32(v)?,
        None => 1,
    };
    if !(1..=7).contains(&weekday) {
        return Err(LibError::invalid_call("WeekdayName weekday out of range"));
    }
    let first = if first == 0 { 1 } else { first };
    let idx = ((weekday - 1) + (first - 1)).rem_euclid(7) as usize;
    let name = NAMES[idx];
    Ok(vstr(if abbreviate { &name[..3] } else { name }))
}

/// `LenB(value)` — byte count. For strings, VBA reports UTF-16 payload bytes;
/// for scalar variables it reports the storage width of the value's VarType.
pub fn len_b(args: &[Variant]) -> LibResult<Variant> {
    let value = need(args, 0)?;
    let bytes = match value.vtype() {
        VarType::String => match value.string_units() {
            Some(units) => units.len() * 2,
            None => as_str(value)?.encode_utf16().count() * 2,
        },
        VarType::Boolean | VarType::Integer | VarType::UnsignedInteger => 2,
        VarType::Byte | VarType::SignedByte => 1,
        VarType::Long | VarType::UnsignedLong | VarType::UnsignedInt | VarType::Error => 4,
        VarType::LongLong | VarType::UnsignedLongLong => 8,
        VarType::Single => 4,
        VarType::Double | VarType::Currency | VarType::Date => 8,
        VarType::Empty | VarType::Null => 0,
        _ => as_str(value)?.encode_utf16().count() * 2,
    };
    Ok(vi32(bytes as i32))
}

/// `DatePart(interval, date, [firstdayofweek], [firstweekofyear])` — extract a
/// component named by `interval` (`yyyy`/`q`/`m`/`y`/`d`/`w`/`ww`/`h`/`n`/`s`).
pub fn vba_datepart(args: &[Variant]) -> LibResult<Variant> {
    let interval = as_str(need(args, 0)?)?.trim().to_ascii_lowercase();
    let serial = as_f64(need(args, 1)?)?;
    let (y, m, d) = serial_to_ymd(serial);
    let (h, mi, s) = serial_to_hms(serial);
    let value = match interval.as_str() {
        "yyyy" => y as i32,
        "q" => (((m - 1) / 3) + 1) as i32,
        "m" => m as i32,
        "d" => d as i32,
        "w" => day_of_week(y as i32, m as u32, d as u32) + 1,
        "y" => {
            // Day of the year: this date's ordinal minus Jan 1's.
            (days_from_civil(y, m, d) - days_from_civil(y, 1, 1) + 1) as i32
        }
        "ww" => {
            // Week of year (default Sunday-first, week 1 = the week of Jan 1).
            let doy = days_from_civil(y, m, d) - days_from_civil(y, 1, 1);
            let jan1_dow = day_of_week(y as i32, 1, 1); // 0 = Sunday
            ((doy + jan1_dow as i64) / 7 + 1) as i32
        }
        "h" => h as i32,
        "n" => mi as i32,
        "s" => s as i32,
        other => {
            return Err(LibError::invalid_call(format!(
                "unknown DatePart interval `{other}`"
            )));
        }
    };
    Ok(vi32(value))
}

/// `IsMissing(arg)` — True only for an omitted optional `Variant` argument,
/// which the VM materializes as vbError `&H80020004` (DISP_E_PARAMNOTFOUND).
pub fn is_missing(args: &[Variant]) -> LibResult<Variant> {
    const MISSING_ARG: i32 = 0x8002_0004u32 as i32;
    let v = need(args, 0)?;
    let missing = v.vtype() == VarType::Error && v.as_error_code() == Some(MISSING_ARG);
    Ok(vbool(missing))
}

pub fn error_text(args: &[Variant]) -> LibResult<Variant> {
    let Some(value) = opt(args, 0) else {
        return Ok(vstr(""));
    };
    if value.vtype() == VarType::Empty {
        return Ok(vstr(""));
    }
    let code = as_i32(value)?;
    if code == 0 {
        return Ok(vstr(""));
    }
    if code < 0 {
        return Err(LibError::invalid_call("Error number must be non-negative"));
    }
    Ok(vstr(oxvba_runtime::default_error_message(code)))
}

/// Lenient truthiness for an optional Boolean-ish flag argument.
fn as_bool_lenient(v: &Variant) -> bool {
    v.as_bool()
        .or_else(|| as_i32(v).ok().map(|n| n != 0))
        .unwrap_or(false)
}

// ── Conversion ─────────────────────────────────────────────────────────────────

fn radix_digits(n: u64, radix: u32) -> String {
    match radix {
        16 => format!("{n:X}"),
        8 => format!("{n:o}"),
        _ => unreachable!("unsupported radix"),
    }
}

fn integer_width_radix(value: &Variant, radix: u32) -> LibResult<String> {
    let digits = match value.vtype() {
        VarType::Boolean => {
            let n: u16 = if value.as_bool().unwrap_or(false) {
                u16::MAX
            } else {
                0
            };
            radix_digits(u64::from(n), radix)
        }
        VarType::SignedByte => radix_digits(value.as_i8().unwrap_or(0) as u8 as u64, radix),
        VarType::Byte => radix_digits(u64::from(value.as_u8().unwrap_or(0)), radix),
        VarType::Integer => radix_digits(value.as_i16().unwrap_or(0) as u16 as u64, radix),
        VarType::UnsignedInteger => radix_digits(u64::from(value.as_u16().unwrap_or(0)), radix),
        VarType::Long => radix_digits(value.as_i32().unwrap_or(0) as u32 as u64, radix),
        VarType::UnsignedLong | VarType::UnsignedInt => {
            radix_digits(u64::from(value.as_u32().unwrap_or(0)), radix)
        }
        VarType::LongLong => radix_digits(value.as_i64().unwrap_or(0) as u64, radix),
        VarType::UnsignedLongLong => radix_digits(value.as_u64().unwrap_or(0), radix),
        _ => radix_digits(as_i64(value)? as u64, radix),
    };
    Ok(digits)
}

pub fn hex(args: &[Variant]) -> LibResult<Variant> {
    Ok(vstr(integer_width_radix(need(args, 0)?, 16)?))
}
pub fn oct(args: &[Variant]) -> LibResult<Variant> {
    Ok(vstr(integer_width_radix(need(args, 0)?, 8)?))
}
pub fn cstr(args: &[Variant]) -> LibResult<Variant> {
    Ok(vstr(as_str(need(args, 0)?)?))
}
/// `Str(n)` — leading space for non-negative numbers.
pub fn str_fn(args: &[Variant]) -> LibResult<Variant> {
    let n = as_f64(need(args, 0)?)?;
    let body = as_str(need(args, 0)?)?;
    Ok(vstr(if n >= 0.0 { format!(" {body}") } else { body }))
}
/// FIDELITY: parses a leading numeric prefix.
pub fn val(args: &[Variant]) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let compact: String = s
        .bytes()
        .filter(|byte| !is_val_ignored_space(*byte))
        .map(char::from)
        .collect();
    let text = compact.as_str();
    if let Some((value, _rest)) = parse_vba_prefixed_integer_token(text, true) {
        return Ok(vf64(value as f64));
    }
    Ok(vf64(val_decimal_prefix(text).unwrap_or(0.0)))
}

fn val_decimal_prefix(text: &str) -> Option<f64> {
    let bytes = text.as_bytes();
    let mut pos = 0usize;
    if matches!(bytes.get(pos), Some(b'+' | b'-')) {
        pos += 1;
    }

    let mut saw_digit = false;
    while matches!(bytes.get(pos), Some(b'0'..=b'9')) {
        saw_digit = true;
        pos += 1;
    }
    if matches!(bytes.get(pos), Some(b'.')) {
        pos += 1;
        while matches!(bytes.get(pos), Some(b'0'..=b'9')) {
            saw_digit = true;
            pos += 1;
        }
    }
    if !saw_digit {
        return None;
    }

    let mantissa_end = pos;
    if matches!(bytes.get(pos), Some(b'e' | b'E' | b'd' | b'D')) {
        pos += 1;
        if matches!(bytes.get(pos), Some(b'+' | b'-')) {
            pos += 1;
        }
        let exp_digits_start = pos;
        while matches!(bytes.get(pos), Some(b'0'..=b'9')) {
            pos += 1;
        }
        if exp_digits_start == pos {
            pos = mantissa_end;
        }
    }

    let token = text[..pos].replace(['d', 'D'], "E");
    token.parse::<f64>().ok()
}
pub fn cdate(args: &[Variant]) -> LibResult<Variant> {
    let v = need(args, 0)?;
    match v.vtype() {
        VarType::Date => Ok(v.clone()),
        // A string parses as a date and/or time; everything else numeric IS the date
        // serial (the integer part is the day, the fraction the time of day).
        VarType::String => cdate_from_string(&as_str(v)?),
        _ => {
            let serial = validate_vba_date_serial(conv_f64(v)?)?;
            Ok(Variant::from_date_f64(serial))
        }
    }
}

/// `CDate`/`CVDate` of a string: `"date"`, `"time"`, or `"date time"` — reusing the
/// `DateValue`/`TimeValue` parsers, summing the day serial and time-of-day fraction.
fn cdate_from_string(s: &str) -> LibResult<Variant> {
    let s = s.trim();
    let has_date = s.contains('/')
        || s.contains('-')
        || s.split([' ', ',', '\t'])
            .any(|t| month_from_name(t).is_some());
    // A pure time ("HH:MM[:SS]") with no date component.
    if s.contains(':') && !has_date {
        return time_value(&[vstr(s)]);
    }
    // "date time" — split a trailing time off the date.
    if let Some((date_part, time_part)) = split_trailing_time(s) {
        let (y, m, d) = parse_date(date_part.trim())?;
        let time = time_value(&[vstr(&time_part)])?
            .as_date_f64()
            .unwrap_or(0.0);
        return Ok(Variant::from_date_f64(validate_vba_date_serial(
            ymd_to_serial(y, m, d) + time,
        )?));
    }
    let (y, m, d) = parse_date(s)?;
    Ok(Variant::from_date_f64(validate_vba_date_serial(
        ymd_to_serial(y, m, d),
    )?))
}
pub fn cverr(args: &[Variant]) -> LibResult<Variant> {
    Ok(Variant::from_error_code(as_i32(need(args, 0)?)?))
}

// VBA numeric / type conversions. Each coerces its single argument to the named
// type with VBA banker's rounding and raises Overflow (6) when the rounded value is
// out of the target's range — matching `CLng`/`CInt`/etc.

/// Read a conversion argument as `f64`, parsing a numeric string (VBA `CDbl("3.5")`,
/// `CInt("&H20")`, etc.). `as_f64` alone does not parse strings — only numeric
/// Variants.
fn conv_f64(value: &Variant) -> LibResult<f64> {
    if value.vtype() == VarType::String {
        return parse_vba_numeric_string(&as_str(value)?)
            .map(|n| n as f64)
            .ok_or_else(|| LibError::type_mismatch("expected a numeric value"));
    }
    as_f64(value)
}
/// Read a conversion argument as `i64` with VBA banker's rounding, parsing a numeric
/// string first. A non-string numeric keeps its exact value via `as_i64`.
fn conv_i64(value: &Variant) -> LibResult<i64> {
    if value.vtype() == VarType::String {
        let d = conv_f64(value)?;
        if !d.is_finite() || d.abs() >= 9.223_372_036_854_775e18 {
            return Err(LibError::overflow("integer overflow"));
        }
        return Ok(d.round_ties_even() as i64);
    }
    as_i64(value)
}

const DECIMAL96_MAX_MAGNITUDE: u128 = (1u128 << 96) - 1;

fn decimal96_from_magnitude(magnitude: u128, scale: u8, negative: bool) -> Decimal96 {
    Decimal96::from_parts(
        magnitude as u32,
        (magnitude >> 32) as u32,
        (magnitude >> 64) as u32,
        scale,
        negative && magnitude != 0,
    )
}

fn decimal96_zero() -> Decimal96 {
    Decimal96::from_parts(0, 0, 0, 0, false)
}

fn decimal96_from_i64(value: i64) -> Decimal96 {
    decimal96_from_magnitude(u128::from(value.unsigned_abs()), 0, value < 0)
}

fn decimal96_from_currency_scaled(scaled: i64) -> Decimal96 {
    decimal96_from_magnitude(u128::from(scaled.unsigned_abs()), 4, scaled < 0)
}

fn round_decimal_digits(digits: &mut Vec<u8>, discard: usize) {
    if discard == 0 {
        return;
    }
    let keep_len = digits.len().saturating_sub(discard);
    let first_dropped = digits.get(keep_len).copied().unwrap_or(0);
    let rest_nonzero = digits.iter().skip(keep_len + 1).any(|d| *d != 0);
    digits.truncate(keep_len);

    let kept_last_odd = digits.last().is_some_and(|d| d % 2 == 1);
    let round_up = first_dropped > 5 || (first_dropped == 5 && (rest_nonzero || kept_last_odd));
    if !round_up {
        return;
    }

    if digits.is_empty() {
        digits.push(1);
        return;
    }
    for digit in digits.iter_mut().rev() {
        if *digit < 9 {
            *digit += 1;
            return;
        }
        *digit = 0;
    }
    digits.insert(0, 1);
}

fn decimal96_from_digits(
    mut digits: Vec<u8>,
    mut scale: usize,
    negative: bool,
) -> Option<Decimal96> {
    while digits.first() == Some(&0) {
        digits.remove(0);
    }
    if digits.is_empty() {
        return Some(decimal96_zero());
    }

    if scale > 28 {
        round_decimal_digits(&mut digits, scale - 28);
        scale = 28;
        while digits.first() == Some(&0) {
            digits.remove(0);
        }
        if digits.is_empty() {
            return Some(decimal96_zero());
        }
    }

    while scale > 0 && digits.last() == Some(&0) {
        digits.pop();
        scale -= 1;
    }

    let mut magnitude = 0u128;
    for digit in digits {
        magnitude = magnitude.checked_mul(10)?.checked_add(u128::from(digit))?;
        if magnitude > DECIMAL96_MAX_MAGNITUDE {
            return None;
        }
    }
    Some(decimal96_from_magnitude(magnitude, scale as u8, negative))
}

fn parse_decimal96_text(text: &str) -> Option<Decimal96> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(value) = parse_vba_prefixed_integer(t) {
        return Some(decimal96_from_i64(value));
    }

    let bytes = t.as_bytes();
    let mut pos = 0;
    let negative = match bytes.get(pos).copied() {
        Some(b'-') => {
            pos += 1;
            true
        }
        Some(b'+') => {
            pos += 1;
            false
        }
        _ => false,
    };

    let mut digits = Vec::new();
    let mut fractional_digits = 0usize;
    let mut saw_digit = false;
    let mut saw_dot = false;
    while let Some(byte) = bytes.get(pos).copied() {
        match byte {
            b'0'..=b'9' => {
                saw_digit = true;
                digits.push(byte - b'0');
                if saw_dot {
                    fractional_digits += 1;
                }
                pos += 1;
            }
            b'.' if !saw_dot => {
                saw_dot = true;
                pos += 1;
            }
            _ => break,
        }
    }
    if !saw_digit {
        return None;
    }

    let mut exponent = 0i32;
    if matches!(bytes.get(pos), Some(b'e' | b'E')) {
        pos += 1;
        let exp_negative = match bytes.get(pos).copied() {
            Some(b'-') => {
                pos += 1;
                true
            }
            Some(b'+') => {
                pos += 1;
                false
            }
            _ => false,
        };
        let exp_start = pos;
        let mut value = 0i32;
        while let Some(byte @ b'0'..=b'9') = bytes.get(pos).copied() {
            value = value.checked_mul(10)?.checked_add(i32::from(byte - b'0'))?;
            pos += 1;
        }
        if pos == exp_start {
            return None;
        }
        exponent = if exp_negative { -value } else { value };
    }
    if pos != bytes.len() {
        return None;
    }

    let scale = i64::try_from(fractional_digits).ok()? - i64::from(exponent);
    if scale < 0 {
        let zero_count = usize::try_from(-scale).ok()?;
        if zero_count > 60 {
            return None;
        }
        digits.extend(std::iter::repeat(0).take(zero_count));
        decimal96_from_digits(digits, 0, negative)
    } else {
        decimal96_from_digits(digits, usize::try_from(scale).ok()?, negative)
    }
}

fn decimal96_from_f64(value: f64) -> LibResult<Decimal96> {
    if !value.is_finite() {
        return Err(LibError::overflow("value does not fit in Decimal"));
    }
    if value == 0.0 {
        return Ok(decimal96_zero());
    }
    let text = format!("{value:.15}");
    parse_decimal96_text(&text).ok_or_else(|| LibError::overflow("value does not fit in Decimal"))
}

fn decimal96_from_variant(value: &Variant) -> LibResult<Decimal96> {
    match value.vtype() {
        VarType::Decimal => value
            .as_decimal96()
            .ok_or_else(|| LibError::type_mismatch("invalid Decimal variant payload")),
        VarType::String => {
            let text = as_str(value)?;
            match parse_decimal96_text(&text) {
                Some(value) => Ok(value),
                None if parse_vba_numeric_string(&text).is_some() => {
                    Err(LibError::overflow("value does not fit in Decimal"))
                }
                None => Err(LibError::type_mismatch("expected a numeric value")),
            }
        }
        VarType::Currency => Ok(decimal96_from_currency_scaled(
            value.as_currency_scaled_i64().unwrap_or(0),
        )),
        VarType::LongLong => Ok(decimal96_from_i64(value.as_i64().unwrap_or(0))),
        _ => decimal96_from_f64(as_f64(value)?),
    }
}

/// `CDbl` — to `Double` (no overflow; the full f64 range).
pub fn cdbl(args: &[Variant]) -> LibResult<Variant> {
    Ok(vf64(conv_f64(need(args, 0)?)?))
}
/// `CSng` — to `Single`; a value outside the `Single` range overflows.
pub fn csng(args: &[Variant]) -> LibResult<Variant> {
    let value = conv_f64(need(args, 0)?)?;
    if value.is_finite() && value.abs() > f64::from(f32::MAX) {
        return Err(LibError::overflow("value does not fit in Single"));
    }
    Ok(Variant::from_f32(value as f32))
}
/// `CInt` — to `Integer` (banker's rounding, range −32768..32767).
pub fn cint(args: &[Variant]) -> LibResult<Variant> {
    let value = conv_i64(need(args, 0)?)?;
    let narrowed =
        i16::try_from(value).map_err(|_| LibError::overflow("value does not fit in Integer"))?;
    Ok(Variant::from_i16(narrowed))
}
/// `CLng` — to `Long` (banker's rounding, range −2^31..2^31−1).
pub fn clng(args: &[Variant]) -> LibResult<Variant> {
    let value = conv_i64(need(args, 0)?)?;
    let narrowed =
        i32::try_from(value).map_err(|_| LibError::overflow("value does not fit in Long"))?;
    Ok(vi32(narrowed))
}
/// `CLngLng` / `CLngPtr` (on the 64-bit runtime) — to `LongLong` (banker's rounding).
pub fn clnglng(args: &[Variant]) -> LibResult<Variant> {
    Ok(Variant::from_i64(conv_i64(need(args, 0)?)?))
}
/// `CByte` — to `Byte` (banker's rounding, range 0..255).
pub fn cbyte(args: &[Variant]) -> LibResult<Variant> {
    let value = conv_i64(need(args, 0)?)?;
    let narrowed =
        u8::try_from(value).map_err(|_| LibError::overflow("value does not fit in Byte"))?;
    Ok(Variant::from_u8(narrowed))
}
/// `CBool` — to `Boolean` (0 → False, any non-zero → True; `"True"`/`"False"` too).
pub fn cbool(args: &[Variant]) -> LibResult<Variant> {
    let value = need(args, 0)?;
    if value.vtype() == VarType::String {
        // The `"True"`/`"False"` literals share one recognizer with the implicit Boolean
        // Let-coercion (`arith::coerce_numeric`) so explicit `CBool` and implicit assignment
        // agree; any other string falls through to the numeric path (non-zero → True).
        if let Some(b) = oxvba_runtime::coerce::parse_bool_text(&as_str(value)?) {
            return Ok(vbool(b));
        }
    }
    Ok(vbool(conv_f64(value)? != 0.0))
}
/// `CCur` — to `Currency` (a fixed 4-decimal scaled i64); out of range overflows.
pub fn ccur(args: &[Variant]) -> LibResult<Variant> {
    let scale = oxvba_runtime::CurrencyValue::SCALE as f64;
    let scaled = (conv_f64(need(args, 0)?)? * scale).round_ties_even();
    if !scaled.is_finite() || scaled.abs() >= 9.223_372_036_854_775e18 {
        return Err(LibError::overflow("value does not fit in Currency"));
    }
    Ok(Variant::from_currency_scaled_i64(scaled as i64))
}
/// `CDec` — to a Variant carrying the Decimal subtype.
pub fn cdec(args: &[Variant]) -> LibResult<Variant> {
    let value = decimal96_from_variant(need(args, 0)?)?;
    Ok(Variant::from_decimal96(value))
}
/// `CVar` — to `Variant`: the value unchanged (everything is already a Variant).
pub fn cvar(args: &[Variant]) -> LibResult<Variant> {
    Ok(need(args, 0)?.clone())
}

// ── Random ───────────────────────────────────────────────────────────────────
//
// VBA's `Rnd` is the VB6/VBA 24-bit linear-congruential generator:
//   x = (x * 0x43FD43FD + 0xC39EC3) mod 2^24,  result = x / 2^24  (as Single).
// `Rnd(0)` repeats the last number; `Rnd(n<0)` reseeds deterministically from
// the Single bit-pattern of `n`; `Randomize [n]` reseeds the high word. State
// is the 24-bit value in `LibContext::rng_state`.

const RND_MULT: u64 = 0x43FD_43FD;
const RND_INC: u64 = 0x00C3_9EC3;
const RND_MASK: u64 = 0x00FF_FFFF; // 2^24 - 1
const RND_SCALE: f64 = 16_777_216.0; // 2^24

fn rng_step(ctx: &mut LibContext) -> f64 {
    ctx.rng_state = ctx.rng_state.wrapping_mul(RND_MULT).wrapping_add(RND_INC) & RND_MASK;
    ctx.rng_state as f64 / RND_SCALE
}

/// An optional numeric argument, treating an `Empty` placeholder as omitted.
fn numeric_arg(args: &[Variant], index: usize) -> Option<&Variant> {
    opt(args, index).filter(|v| !matches!(v.vtype(), VarType::Empty) && !missing_sentinel(v))
}

pub fn rnd(args: &[Variant], ctx: &mut LibContext) -> LibResult<Variant> {
    let value = match numeric_arg(args, 0) {
        Some(v) => as_f64(v)?,
        None => 1.0, // omitted ⇒ next in sequence
    };
    let result = if value == 0.0 {
        // Rnd(0): return the most recently generated number, unchanged.
        ctx.rng_state as f64 / RND_SCALE
    } else if value < 0.0 {
        // Rnd(n<0): deterministic reseed from the Single bit pattern of n.
        ctx.rng_state = (value as f32).to_bits() as u64 & RND_MASK;
        rng_step(ctx)
    } else {
        rng_step(ctx)
    };
    Ok(Variant::from_f32(result as f32))
}

pub fn randomize(args: &[Variant], ctx: &mut LibContext) -> LibResult<Variant> {
    let seed = match numeric_arg(args, 0) {
        Some(v) => (as_f64(v)? as f32).to_bits() as u64,
        // No argument: VBA seeds from the system timer. Without host access here,
        // fold the current state forward deterministically.
        None => ctx.rng_state.wrapping_mul(RND_MULT).wrapping_add(RND_INC),
    };
    // VBA replaces the high 16 bits of the 24-bit state, preserving the low byte.
    ctx.rng_state = ((ctx.rng_state & 0xFF) | ((seed & 0xFFFF) << 8)) & RND_MASK;
    Ok(vunit())
}

// ── Financial ──────────────────────────────────────────────────────────────────

fn opt_f64(args: &[Variant], index: usize, default: f64) -> LibResult<f64> {
    match opt(args, index) {
        Some(v) => as_f64(v),
        None => Ok(default),
    }
}

fn cashflows(v: &Variant) -> LibResult<Vec<f64>> {
    match v.as_safearray() {
        Some(arr) => arr
            .variant_elements()
            .unwrap_or_default()
            .iter()
            .map(as_f64)
            .collect(),
        None => Ok(vec![as_f64(v)?]),
    }
}

/// Future value of `pv` plus `nper` payments `pmt` at periodic rate `i`
/// (`t` = 0 ordinary / end-of-period, 1 annuity-due / begin-of-period). Shared by
/// `FV` and the `IPmt`/`PPmt` balance computation.
fn fv_value(i: f64, nper: f64, pmt: f64, pv: f64, t: f64) -> f64 {
    if i == 0.0 {
        -(pv + pmt * nper)
    } else {
        let g = (1.0 + i).powf(nper);
        -(pv * g + pmt * (1.0 + i * t) * (g - 1.0) / i)
    }
}

/// Periodic payment for present value `pv` reaching future value `fv` over `nper`
/// periods at rate `i` (`t` as in [`fv_value`]). Shared by `Pmt` and `IPmt`/`PPmt`.
fn pmt_value(i: f64, nper: f64, pv: f64, fv: f64, t: f64) -> f64 {
    if i == 0.0 {
        -(pv + fv) / nper
    } else {
        let g = (1.0 + i).powf(nper);
        -(fv + pv * g) / ((1.0 + i * t) * (g - 1.0) / i)
    }
}

/// The interest portion of the `per`-th payment (the basis for `IPmt`/`PPmt`). The
/// interest is the balance carried into the period times the rate; for an annuity
/// due (`t = 1`) the first period carries no interest and later periods discount by
/// one period — the numpy-financial/Excel convention, live-verified against VBA
/// (`IPmt(0.1/12, 2, 36, 10000, 0, 1) = -80.6667`).
fn ipmt_value(rate: f64, per: f64, nper: f64, pv: f64, fv: f64, t: f64) -> f64 {
    let pmt = pmt_value(rate, nper, pv, fv, t);
    let mut interest = fv_value(rate, per - 1.0, pmt, pv, t) * rate;
    if t == 1.0 {
        if per == 1.0 {
            interest = 0.0;
        } else {
            interest /= 1.0 + rate;
        }
    }
    interest
}

pub fn fv(args: &[Variant]) -> LibResult<Variant> {
    let (i, n, pmt) = (
        as_f64(need(args, 0)?)?,
        as_f64(need(args, 1)?)?,
        as_f64(need(args, 2)?)?,
    );
    let pv = opt_f64(args, 3, 0.0)?;
    let t = opt_f64(args, 4, 0.0)?;
    Ok(vf64(fv_value(i, n, pmt, pv, t)))
}

pub fn pv(args: &[Variant]) -> LibResult<Variant> {
    let (i, n, pmt) = (
        as_f64(need(args, 0)?)?,
        as_f64(need(args, 1)?)?,
        as_f64(need(args, 2)?)?,
    );
    let fv = opt_f64(args, 3, 0.0)?;
    let t = opt_f64(args, 4, 0.0)?;
    Ok(vf64(if i == 0.0 {
        -(fv + pmt * n)
    } else {
        let g = (1.0 + i).powf(n);
        -(fv + pmt * (1.0 + i * t) * (g - 1.0) / i) / g
    }))
}

pub fn pmt(args: &[Variant]) -> LibResult<Variant> {
    let (i, n, pv) = (
        as_f64(need(args, 0)?)?,
        as_f64(need(args, 1)?)?,
        as_f64(need(args, 2)?)?,
    );
    let fv = opt_f64(args, 3, 0.0)?;
    let t = opt_f64(args, 4, 0.0)?;
    Ok(vf64(pmt_value(i, n, pv, fv, t)))
}

/// `IPmt(rate, per, nper, pv, [fv = 0], [type = 0])` → the interest portion of the
/// `per`-th payment.
pub fn ipmt(args: &[Variant]) -> LibResult<Variant> {
    let rate = as_f64(need(args, 0)?)?;
    let per = as_f64(need(args, 1)?)?;
    let nper = as_f64(need(args, 2)?)?;
    let pv = as_f64(need(args, 3)?)?;
    let fv = opt_f64(args, 4, 0.0)?;
    let t = if opt_f64(args, 5, 0.0)? != 0.0 {
        1.0
    } else {
        0.0
    };
    Ok(vf64(ipmt_value(rate, per, nper, pv, fv, t)))
}

/// `PPmt(rate, per, nper, pv, [fv = 0], [type = 0])` → the principal portion of the
/// `per`-th payment, i.e. the period payment minus its interest portion.
pub fn ppmt(args: &[Variant]) -> LibResult<Variant> {
    let rate = as_f64(need(args, 0)?)?;
    let per = as_f64(need(args, 1)?)?;
    let nper = as_f64(need(args, 2)?)?;
    let pv = as_f64(need(args, 3)?)?;
    let fv = opt_f64(args, 4, 0.0)?;
    let t = if opt_f64(args, 5, 0.0)? != 0.0 {
        1.0
    } else {
        0.0
    };
    let payment = pmt_value(rate, nper, pv, fv, t);
    Ok(vf64(payment - ipmt_value(rate, per, nper, pv, fv, t)))
}

/// `SLN(cost, salvage, life)` → straight-line depreciation per period.
pub fn sln(args: &[Variant]) -> LibResult<Variant> {
    let cost = as_f64(need(args, 0)?)?;
    let salvage = as_f64(need(args, 1)?)?;
    let life = as_f64(need(args, 2)?)?;
    Ok(vf64((cost - salvage) / life))
}

/// `SYD(cost, salvage, life, period)` → sum-of-years'-digits depreciation for
/// `period`.
pub fn syd(args: &[Variant]) -> LibResult<Variant> {
    let cost = as_f64(need(args, 0)?)?;
    let salvage = as_f64(need(args, 1)?)?;
    let life = as_f64(need(args, 2)?)?;
    let period = as_f64(need(args, 3)?)?;
    Ok(vf64(
        (cost - salvage) * (life - period + 1.0) * 2.0 / (life * (life + 1.0)),
    ))
}

/// `DDB(cost, salvage, life, period, [factor = 2])` → double-declining-balance
/// depreciation for `period`. Uses the closed form `book(start) - book(end)` with
/// the salvage floor (depreciation never takes book value below `salvage`) —
/// live-verified against VBA (`DDB(10000,1000,5,5) = 296`, the salvage-capped final
/// period; `DDB(…, factor:=3)` honours the rate override).
pub fn ddb(args: &[Variant]) -> LibResult<Variant> {
    let cost = as_f64(need(args, 0)?)?;
    let salvage = as_f64(need(args, 1)?)?;
    let life = as_f64(need(args, 2)?)?;
    let period = as_f64(need(args, 3)?)?;
    let factor = opt_f64(args, 4, 2.0)?;
    let rate = factor / life;
    let (book_start, book_end) = if rate >= 1.0 {
        // A rate at/above 1 fully depreciates in the first period.
        (if period == 1.0 { cost } else { 0.0 }, 0.0)
    } else {
        (
            cost * (1.0 - rate).powf(period - 1.0),
            cost * (1.0 - rate).powf(period),
        )
    };
    let depreciation = if book_end < salvage {
        book_start - salvage
    } else {
        book_start - book_end
    };
    Ok(vf64(depreciation.max(0.0)))
}

pub fn nper(args: &[Variant]) -> LibResult<Variant> {
    let (i, pmt, pv) = (
        as_f64(need(args, 0)?)?,
        as_f64(need(args, 1)?)?,
        as_f64(need(args, 2)?)?,
    );
    let fv = opt_f64(args, 3, 0.0)?;
    let t = opt_f64(args, 4, 0.0)?;
    Ok(vf64(if i == 0.0 {
        -(pv + fv) / pmt
    } else {
        let a = pmt * (1.0 + i * t);
        ((a - fv * i) / (a + pv * i)).ln() / (1.0 + i).ln()
    }))
}

pub fn npv(args: &[Variant]) -> LibResult<Variant> {
    let rate = as_f64(need(args, 0)?)?;
    let flows: Vec<f64> = match opt(args, 1) {
        Some(v) if v.as_safearray().is_some() => cashflows(v)?,
        _ => {
            let mut out = Vec::new();
            for v in &args[1..] {
                out.push(as_f64(v)?);
            }
            out
        }
    };
    let mut acc = 0.0;
    for (k, cf) in flows.iter().enumerate() {
        acc += cf / (1.0 + rate).powi(k as i32 + 1);
    }
    Ok(vf64(acc))
}

fn npv_at(rate: f64, flows: &[f64]) -> f64 {
    flows
        .iter()
        .enumerate()
        .map(|(k, cf)| cf / (1.0 + rate).powi(k as i32))
        .sum()
}

/// FIDELITY: Newton/​bisection hybrid first-cut.
pub fn irr(args: &[Variant]) -> LibResult<Variant> {
    let flows = cashflows(need(args, 0)?)?;
    let mut rate = opt_f64(args, 1, 0.1)?;
    for _ in 0..100 {
        let f = npv_at(rate, &flows);
        let df = flows
            .iter()
            .enumerate()
            .skip(1)
            .map(|(k, cf)| -(k as f64) * cf / (1.0 + rate).powi(k as i32 + 1))
            .sum::<f64>();
        if df.abs() < 1e-12 {
            break;
        }
        let next = rate - f / df;
        if (next - rate).abs() < 1e-9 {
            rate = next;
            break;
        }
        rate = next;
    }
    Ok(vf64(rate))
}

pub fn mirr(args: &[Variant]) -> LibResult<Variant> {
    let flows = cashflows(need(args, 0)?)?;
    let finance = as_f64(need(args, 1)?)?;
    let reinvest = as_f64(need(args, 2)?)?;
    let n = flows.len() as i32;
    let neg: f64 = flows
        .iter()
        .enumerate()
        .filter(|(_, cf)| **cf < 0.0)
        .map(|(k, cf)| cf / (1.0 + finance).powi(k as i32))
        .sum();
    let pos: f64 = flows
        .iter()
        .enumerate()
        .filter(|(_, cf)| **cf > 0.0)
        .map(|(k, cf)| cf * (1.0 + reinvest).powi(n - 1 - k as i32))
        .sum();
    if neg == 0.0 || n <= 1 {
        return Err(LibError::invalid_call(
            "MIRR requires positive and negative flows",
        ));
    }
    Ok(vf64((-pos / neg).powf(1.0 / (n as f64 - 1.0)) - 1.0))
}

fn rate_func(r: f64, nper: f64, pmt: f64, pv: f64, fv: f64, due: f64) -> f64 {
    if r.abs() < 1e-9 {
        pv + pmt * nper + fv
    } else {
        let growth = (1.0 + r).powf(nper);
        pv * growth + pmt * (1.0 + r * due) * ((growth - 1.0) / r) + fv
    }
}

fn rate_func_derivative(r: f64, nper: f64, pmt: f64, pv: f64, due: f64) -> f64 {
    if r.abs() < 1e-8 {
        let step = 1e-7;
        (rate_func(r + step, nper, pmt, pv, 0.0, due)
            - rate_func(r - step, nper, pmt, pv, 0.0, due))
            / (2.0 * step)
    } else {
        let base = 1.0 + r;
        if base <= 0.0 {
            return f64::NAN;
        }
        let growth = base.powf(nper);
        let growth_p = nper * base.powf(nper - 1.0);
        let c = (growth - 1.0) / r;
        let c_p = (growth_p * r - (growth - 1.0)) / (r * r);
        pv * growth_p + pmt * (due * c + (1.0 + r * due) * c_p)
    }
}

/// Ported from the legacy VM's `rate_i32`: Newton iteration with the analytic
/// annuity derivative. Switched to f64 (the legacy variant scaled by percent).
pub fn rate(args: &[Variant]) -> LibResult<Variant> {
    let nper = as_f64(need(args, 0)?)?;
    let pmt = as_f64(need(args, 1)?)?;
    let pv = as_f64(need(args, 2)?)?;
    let fv = opt_f64(args, 3, 0.0)?;
    let due = if opt_f64(args, 4, 0.0)? != 0.0 {
        1.0
    } else {
        0.0
    };
    if nper == 0.0 {
        return Err(LibError::invalid_call("Rate requires nper <> 0"));
    }
    let mut r = opt_f64(args, 5, 0.1)?.clamp(-0.99, 10.0);
    for _ in 0..60 {
        let f = rate_func(r, nper, pmt, pv, fv, due);
        let fp = rate_func_derivative(r, nper, pmt, pv, due);
        if fp.abs() < 1e-12 {
            return Err(LibError::invalid_call("Rate failed to converge"));
        }
        let next = (r - f / fp).clamp(-0.99, 10.0);
        if !next.is_finite() {
            return Err(LibError::invalid_call("Rate diverged"));
        }
        if (next - r).abs() < 1e-10 {
            return Ok(vf64(next));
        }
        r = next;
    }
    Err(LibError::invalid_call("Rate failed to converge"))
}

// ── Colour ───────────────────────────────────────────────────────────────────────

/// `RGB(red, green, blue)` → a `Long` packed colour. Each component is coerced to a
/// number and CLAMPED to `0..=255` (a value above 255 becomes 255 — live-verified:
/// `RGB(256, 300, 1000) = 16777215`), then packed as `red + green*256 + blue*65536`
/// (the maximum, `RGB(255,255,255)`, is `16_777_215`, well within `Long`).
pub fn rgb(args: &[Variant]) -> LibResult<Variant> {
    let component =
        |index: usize| -> LibResult<i32> { Ok(as_i32(need(args, index)?)?.clamp(0, 255)) };
    let (red, green, blue) = (component(0)?, component(1)?, component(2)?);
    Ok(vi32(red + green * 256 + blue * 65_536))
}

/// `QBColor(colorIndex)` → the `Long` packed colour for one of the 16 legacy
/// QuickBasic console colours. The table is live-verified against VBA; an index
/// outside `0..=15` raises "Invalid procedure call or argument" (error 5).
pub fn qb_color(args: &[Variant]) -> LibResult<Variant> {
    // colour-index → packed RGB `Long`, exactly as VBA's `QBColor` returns
    // (live-probed: 0..15 → these values).
    const PALETTE: [i32; 16] = [
        0, 8_388_608, 32_768, 8_421_376, 128, 8_388_736, 32_896, 12_632_256, 8_421_504, 16_711_680,
        65_280, 16_776_960, 255, 16_711_935, 65_535, 16_777_215,
    ];
    let index = as_i32(need(args, 0)?)?;
    if !(0..=15).contains(&index) {
        return Err(LibError::invalid_call("QBColor index must be 0 to 15"));
    }
    Ok(vi32(PALETTE[index as usize]))
}

// ── Interaction ──────────────────────────────────────────────────────────────

/// `Partition(number, start, stop, interval)` identifies the range containing
/// `number`. Integer arguments use VBA banker's rounding; a `Null` argument
/// propagates to a `Null` result.
pub fn partition(args: &[Variant]) -> LibResult<Variant> {
    let number_arg = need(args, 0)?;
    let start_arg = need(args, 1)?;
    let stop_arg = need(args, 2)?;
    let interval_arg = need(args, 3)?;

    if [number_arg, start_arg, stop_arg, interval_arg]
        .iter()
        .any(|value| value.vtype() == VarType::Null)
    {
        return Ok(Variant::null());
    }
    let number = as_i64(number_arg)?;
    let start = as_i64(start_arg)?;
    let stop = as_i64(stop_arg)?;
    let interval = as_i64(interval_arg)?;

    if start < 0 {
        return Err(LibError::invalid_call(
            "Partition start must be greater than or equal to 0",
        ));
    }
    if stop <= start {
        return Err(LibError::invalid_call(
            "Partition stop must be greater than start",
        ));
    }
    if interval < 1 {
        return Err(LibError::invalid_call(
            "Partition interval must be greater than or equal to 1",
        ));
    }

    let after_stop = stop
        .checked_add(1)
        .ok_or_else(|| LibError::overflow("Partition stop overflow"))?;
    let width = after_stop.to_string().len();

    let (lower, upper) = if interval == 1 {
        (Some(number), Some(number))
    } else if number < start {
        (None, Some(start - 1))
    } else if number > stop {
        (Some(after_stop), None)
    } else {
        let offset = number - start;
        let lower = start + (offset / interval) * interval;
        let upper = lower
            .checked_add(interval - 1)
            .unwrap_or(i64::MAX)
            .min(stop);
        (Some(lower), Some(upper))
    };

    Ok(vstr(format!(
        "{}:{}",
        partition_bound(lower, width),
        partition_bound(upper, width)
    )))
}

fn partition_bound(value: Option<i64>, width: usize) -> String {
    match value {
        Some(value) => format!("{value:>width$}"),
        None => " ".repeat(width),
    }
}

// ── Information ──────────────────────────────────────────────────────────────────

pub fn is_vtype(args: &[Variant], pred: impl Fn(VarType) -> bool) -> LibResult<Variant> {
    Ok(vbool(pred(need(args, 0)?.vtype())))
}

pub fn var_type(args: &[Variant]) -> LibResult<Variant> {
    let value = need(args, 0)?;
    let code = match value.as_safearray() {
        Some(array) => 0x2000 | i32::from(array.element_vartype()),
        None => value.vtype() as i32,
    };
    Ok(vi32(code))
}

pub fn type_name(args: &[Variant]) -> LibResult<Variant> {
    let value = need(args, 0)?;
    if let Some(array) = value.as_safearray() {
        return Ok(vstr(format!(
            "{}()",
            type_name_for_vartype(array.element_vartype())
        )));
    }
    let name = match value.vtype() {
        VarType::Empty => "Empty",
        VarType::Null => "Null",
        VarType::Integer => "Integer",
        VarType::Long => "Long",
        VarType::Single => "Single",
        VarType::Double => "Double",
        VarType::Currency => "Currency",
        VarType::Date => "Date",
        VarType::String => "String",
        VarType::Object
            if value
                .as_object_ref()
                .map(|object| object.raw())
                .unwrap_or(0)
                == 0 =>
        {
            "Nothing"
        }
        VarType::Object => "Object",
        VarType::Error => "Error",
        VarType::Boolean => "Boolean",
        VarType::Decimal => "Decimal",
        VarType::Byte => "Byte",
        VarType::LongLong => "LongLong",
        VarType::ArrayVariant => "Variant()",
        _ => "Variant",
    };
    Ok(vstr(name))
}

fn type_name_for_vartype(vartype: u16) -> &'static str {
    if vartype == VT_VARIANT_VALUE {
        return "Variant";
    }
    match VarType::from_u16(vartype) {
        Some(VarType::Empty) => "Empty",
        Some(VarType::Null) => "Null",
        Some(VarType::Integer) => "Integer",
        Some(VarType::Long) => "Long",
        Some(VarType::Single) => "Single",
        Some(VarType::Double) => "Double",
        Some(VarType::Currency) => "Currency",
        Some(VarType::Date) => "Date",
        Some(VarType::String) => "String",
        Some(VarType::Object) => "Object",
        Some(VarType::Error) => "Error",
        Some(VarType::Boolean) => "Boolean",
        Some(VarType::Decimal) => "Decimal",
        Some(VarType::Byte) => "Byte",
        Some(VarType::LongLong) => "LongLong",
        _ => "Variant",
    }
}

pub fn is_numeric(args: &[Variant]) -> LibResult<Variant> {
    let v = need(args, 0)?;
    // A *String* is numeric iff its text represents a VBA number — `coerce_to(..,
    // Double)` does not parse strings, so a numeric string like "42" needs the
    // dedicated grammar check. Every other subtype is judged by its `VarType`:
    // VBA's `IsNumeric` is True only for the genuine numeric subtypes and `Empty`,
    // and False for `Boolean`/`Date`/`Null`/`Object`/`Error`/arrays/`Nothing`
    // (`coerce_to(.., Double)` would wrongly accept Boolean and Date).
    let result = match v.vtype() {
        VarType::String => is_numeric_string(&as_str(v)?),
        VarType::Empty
        | VarType::Integer
        | VarType::Long
        | VarType::Single
        | VarType::Double
        | VarType::Currency
        | VarType::Byte
        | VarType::SignedByte
        | VarType::Decimal
        | VarType::LongLong
        | VarType::UnsignedInteger
        | VarType::UnsignedLong
        | VarType::UnsignedLongLong
        | VarType::UnsignedInt => true,
        VarType::Boolean
        | VarType::Date
        | VarType::Null
        | VarType::Object
        | VarType::Error
        | VarType::ProcRef
        | VarType::Record
        | VarType::ArrayVariant => false,
    };
    Ok(vbool(result))
}

/// True when `s` represents a VBA number (the `IsNumeric` string grammar). VBA
/// recognises a decimal/sign/exponent literal (optionally surrounded by
/// whitespace) and `&H`/`&O` (or bare `&`) integer literals; it does NOT accept
/// thousands separators, currency symbols (locale-dependent — out of scope), or
/// Rust's `inf`/`nan` spellings.
///
/// This needs a FULL-parse check, not Val's leading-prefix parse: `IsNumeric("12a")`
/// must be False.
fn is_numeric_string(s: &str) -> bool {
    parse_vba_numeric_string(s).is_some()
}

fn parse_vba_numeric_string(s: &str) -> Option<f64> {
    oxvba_runtime::coerce::parse_vba_numeric_string(s)
}

fn parse_vba_prefixed_integer(t: &str) -> Option<i64> {
    let (value, rest) = parse_vba_prefixed_integer_token(t, false)?;
    if rest.trim().is_empty() {
        Some(value)
    } else {
        None
    }
}

// `Val` reads a leading token and ignores spaces/tabs/newlines inside it; the
// conversion functions and `IsNumeric` require the whole string to be numeric.
fn parse_vba_prefixed_integer_token(t: &str, skip_embedded_space: bool) -> Option<(i64, &str)> {
    let bytes = t.as_bytes();
    let mut pos = 0;
    let (sign, rest) = if let Some(rest) = t.strip_prefix('-') {
        pos = 1;
        (-1_i64, rest)
    } else if let Some(rest) = t.strip_prefix('+') {
        pos = 1;
        (1_i64, rest)
    } else {
        (1_i64, t)
    };
    // VBA hex (`&H…`) / octal (`&O…` or bare `&` + octal digits) integer literals,
    // with an optional trailing `&` Long-type suffix.
    rest.strip_prefix('&')?;
    pos += 1;
    let (radix, digit_start) = match bytes.get(pos) {
        Some(b'H' | b'h') => (16, pos + 1),
        Some(b'O' | b'o') => (8, pos + 1),
        // Bare `&` followed by octal digits (VBA's terse octal form).
        _ => (8, pos),
    };
    pos = digit_start;

    let mut digits = String::new();
    while let Some(&byte) = bytes.get(pos) {
        if skip_embedded_space && is_val_ignored_space(byte) {
            pos += 1;
            continue;
        }
        let is_digit = match radix {
            16 => byte.is_ascii_hexdigit(),
            8 => (b'0'..=b'7').contains(&byte),
            _ => false,
        };
        if !is_digit {
            break;
        }
        digits.push(char::from(byte));
        pos += 1;
    }
    if digits.is_empty() {
        return None;
    }

    if skip_embedded_space {
        while let Some(&byte) = bytes.get(pos) {
            if !is_val_ignored_space(byte) {
                break;
            }
            pos += 1;
        }
    }
    // The width-based two's-complement sign rule applies to `Val`/`CInt`/`CLng`
    // of a `&H…`/`&O…` *string* exactly as to a literal — `Val("&HFFFF")` is -1
    // (verified live). Honour an optional `%`/`&`/`^` type character too.
    let suffix = match bytes.get(pos) {
        Some(&b @ (b'%' | b'&' | b'^')) => Some(b),
        _ => None,
    };
    if suffix.is_some() {
        pos += 1;
    }
    let magnitude = u64::from_str_radix(&digits, radix).ok()?;
    let width = oxvba_runtime::vba_radix_width(magnitude, suffix)?;
    let value = oxvba_runtime::vba_radix_signed_value(magnitude, width).checked_mul(sign)?;
    Some((value, &t[pos..]))
}

fn is_val_ignored_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

pub fn is_date(args: &[Variant]) -> LibResult<Variant> {
    let v = need(args, 0)?;
    // VBA `IsDate` is True for a Date value or a STRING recognizable as a valid date/time
    // (using the same lenient parser as `CDate`); a raw numeric, Empty, Null, Boolean, or
    // object is NOT a date to `IsDate` (even though `CDate` would convert a number).
    let ok = match v.vtype() {
        VarType::Date => true,
        VarType::String => as_str(v)
            .ok()
            .is_some_and(|s| cdate_from_string(&s).is_ok()),
        _ => false,
    };
    Ok(vbool(ok))
}

/// VBA truthiness for the special forms: `Null`/`Empty` are false, otherwise a
/// nonzero numeric coercion is true (`True` is -1, any nonzero is true).
fn truthy(v: &Variant) -> LibResult<bool> {
    match v.vtype() {
        VarType::Null | VarType::Empty => Ok(false),
        _ => Ok(as_f64(v)? != 0.0),
    }
}

/// `IIf(cond, truepart, falsepart)` — EAGER: both parts are already evaluated
/// by the caller; this just selects one.
pub fn iif(args: &[Variant]) -> LibResult<Variant> {
    if truthy(need(args, 0)?)? {
        Ok(need(args, 1)?.clone())
    } else {
        Ok(need(args, 2)?.clone())
    }
}

/// `Choose(index, v1, v2, …)` — 1-based selection; `Null` when out of range.
pub fn choose(args: &[Variant]) -> LibResult<Variant> {
    let idx = as_i32(need(args, 0)?)?;
    if idx < 1 {
        return Ok(Variant::null());
    }
    Ok(args
        .get(idx as usize)
        .cloned()
        .unwrap_or_else(Variant::null))
}

/// `Switch(c1, v1, c2, v2, …)` — returns the first `vi` whose `ci` is truthy,
/// else `Null`.
pub fn switch(args: &[Variant]) -> LibResult<Variant> {
    let mut i = 0;
    while i + 1 < args.len() {
        if truthy(&args[i])? {
            return Ok(args[i + 1].clone());
        }
        i += 2;
    }
    Ok(Variant::null())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vs(s: &str) -> Variant {
        Variant::from_string(s.to_string())
    }
    fn like_(s: &str, p: &str) -> bool {
        like(&[vs(s), vs(p)]).unwrap().as_bool().unwrap()
    }
    fn instr_(args: &[Variant], rev: bool) -> i32 {
        instr(args, rev).unwrap().as_i32().unwrap()
    }
    fn split_(args: &[Variant]) -> Vec<String> {
        split(args)
            .unwrap()
            .as_safearray()
            .unwrap()
            .variant_elements()
            .unwrap_or_default()
            .iter()
            .map(|v| v.as_bstr().map(|b| b.as_str()).unwrap_or_default())
            .collect()
    }
    fn val_(s: &str) -> f64 {
        val(&[vs(s)]).unwrap().as_f64().unwrap()
    }
    fn partition_(args: &[Variant]) -> String {
        partition(args).unwrap().as_bstr().unwrap().as_str()
    }
    fn bstr(result: LibResult<Variant>) -> String {
        result.unwrap().as_bstr().unwrap().as_str()
    }

    #[test]
    fn val_parses_vba_radix_prefixes() {
        assert_eq!(val_("&HFFFFFFFF"), -1.0);
        assert_eq!(val_("&HFFFF&"), 65_535.0);
        assert_eq!(val_("&O37777777777"), -1.0);
        assert_eq!(val_("+&H7F"), 127.0);
        assert_eq!(val_("-&O10"), -8.0);
        assert_eq!(val_("&H F F"), 255.0);
        assert_eq!(val_("&H10 trailing"), 16.0);
    }

    #[test]
    fn val_parses_longest_complete_decimal_prefix() {
        assert_eq!(val_("123abc"), 123.0);
        assert_eq!(val_("12-3"), 12.0);
        assert_eq!(val_("1.2.3"), 1.2);
        assert_eq!(val_("1e2"), 100.0);
        assert_eq!(val_("1e"), 1.0);
        assert_eq!(val_("1e+"), 1.0);
        assert_eq!(val_("1e-2"), 0.01);
        assert_eq!(val_("1D2"), 100.0);
        assert_eq!(val_("1 2"), 12.0);
        assert_eq!(val_("- .5"), -0.5);
        assert_eq!(val_("$1"), 0.0);
    }

    #[test]
    fn sgn_rejects_nan_double() {
        assert_eq!(sgn(&[Variant::from_f64(f64::NAN)]).unwrap_err().code, 5);
    }

    #[test]
    fn mid_rejects_start_less_than_one() {
        assert_eq!(
            mid(&[vs("abcdef"), Variant::from_i32(0), Variant::from_i32(2)])
                .unwrap_err()
                .code,
            5
        );
        assert_eq!(
            mid_stmt(&[
                vs("abcdef"),
                Variant::from_i32(0),
                Variant::from_i32(2),
                vs("ZZ"),
            ])
            .unwrap_err()
            .code,
            5
        );
        assert_eq!(
            mid(&[vs("abcdef"), Variant::from_i32(1), Variant::from_i32(2)])
                .unwrap()
                .as_bstr()
                .unwrap()
                .as_str(),
            "ab"
        );
    }

    #[test]
    fn format_number_family_honours_defaults_and_tristates() {
        assert_eq!(
            bstr(format_number(&[Variant::from_f64(1234.567)])),
            "1,234.57"
        );
        assert_eq!(
            bstr(format_number(&[
                Variant::from_f64(1234.567),
                Variant::from_i32(0),
                Variant::from_i32(-2),
                Variant::from_i32(0),
                Variant::from_i32(0),
            ])),
            "1235"
        );
        assert_eq!(
            bstr(format_number(&[
                Variant::from_f64(0.5),
                Variant::from_i32(2),
                Variant::from_i32(0),
                Variant::from_i32(0),
                Variant::from_i32(0),
            ])),
            ".50"
        );
        assert_eq!(
            bstr(format_number(&[
                Variant::from_f64(-1234.5),
                Variant::from_i32(2),
                Variant::from_i32(-1),
                Variant::from_i32(-1),
                Variant::from_i32(-1),
            ])),
            "(1,234.50)"
        );
        assert_eq!(
            bstr(format_currency(&[
                Variant::from_f64(-1234.5),
                Variant::from_i32(2),
                Variant::from_i32(-1),
                Variant::from_i32(-1),
                Variant::from_i32(-1),
            ])),
            "($1,234.50)"
        );
        assert_eq!(
            bstr(format_percent(&[
                Variant::from_f64(0.1234),
                Variant::from_i32(1),
                Variant::from_i32(-1),
                Variant::from_i32(0),
                Variant::from_i32(0),
            ])),
            "12.3%"
        );
        assert_eq!(
            bstr(format_percent(&[
                Variant::from_f64(0.005),
                Variant::from_i32(2),
                Variant::from_i32(0),
                Variant::from_i32(0),
                Variant::from_i32(0),
            ])),
            ".50%"
        );
    }

    #[test]
    fn format_date_time_honours_named_formats() {
        let date = Variant::from_date_f64(ymd_to_serial(2020, 1, 15) + 13.5 / 24.0);
        assert_eq!(
            bstr(format_date_time(&[date.clone(), Variant::from_i32(0)])),
            "1/15/2020 1:30:00 PM"
        );
        assert_eq!(
            bstr(format_date_time(&[date.clone(), Variant::from_i32(1)])),
            "Wednesday, January 15, 2020"
        );
        assert_eq!(
            bstr(format_date_time(&[date.clone(), Variant::from_i32(2)])),
            "1/15/2020"
        );
        assert_eq!(
            bstr(format_date_time(&[date.clone(), Variant::from_i32(3)])),
            "1:30:00 PM"
        );
        assert_eq!(
            bstr(format_date_time(&[date, Variant::from_i32(4)])),
            "13:30"
        );
    }

    #[test]
    fn format_family_rejects_invalid_option_values() {
        assert_eq!(
            format_number(&[Variant::from_f64(1.2), Variant::from_i32(-2)])
                .unwrap_err()
                .code,
            5
        );
        assert_eq!(
            format_number(&[
                Variant::from_f64(1.2),
                Variant::from_i32(2),
                Variant::from_i32(1),
            ])
            .unwrap_err()
            .code,
            5
        );
        assert_eq!(
            format_date_time(&[Variant::from_f64(1.2), Variant::from_i32(5)])
                .unwrap_err()
                .code,
            5
        );
    }

    #[test]
    fn error_text_returns_default_messages_and_fallbacks() {
        assert_eq!(bstr(error_text(&[])), "");
        assert_eq!(bstr(error_text(&[Variant::from_i32(0)])), "");
        assert_eq!(
            bstr(error_text(&[Variant::from_i32(11)])),
            "Division by zero"
        );
        assert_eq!(
            bstr(error_text(&[Variant::from_i32(12345)])),
            "Application-defined or object-defined error"
        );
        assert_eq!(error_text(&[Variant::from_i32(-1)]).unwrap_err().code, 5);
    }

    #[test]
    fn split_honours_limit_and_compare() {
        // No limit → all substrings.
        assert_eq!(split_(&[vs("a,b,c"), vs(",")]), ["a", "b", "c"]);
        // limit=2 → the last element holds the unsplit remainder.
        assert_eq!(
            split_(&[vs("a,b,c"), vs(","), Variant::from_i32(2)]),
            ["a", "b,c"]
        );
        // limit=1 → the whole string.
        assert_eq!(
            split_(&[vs("a,b,c"), vs(","), Variant::from_i32(1)]),
            ["a,b,c"]
        );
        // limit=0 → an empty array.
        assert!(split_(&[vs("a,b,c"), vs(","), Variant::from_i32(0)]).is_empty());
        // Text compare (mode 1): split on "x" case-insensitively, original case preserved.
        assert_eq!(
            split_(&[
                vs("aXbxC"),
                vs("x"),
                Variant::from_i32(-1),
                Variant::from_i32(1)
            ]),
            ["a", "b", "C"]
        );
        // Binary compare (default): only the lowercase "x" splits.
        assert_eq!(split_(&[vs("aXbxC"), vs("x")]), ["aXb", "C"]);
    }

    #[test]
    fn like_charlist() {
        assert!(like_("abc", "[a-c]bc"));
        assert!(like_("Abc", "[A-Z]*"));
        assert!(like_("x", "[!a-c]"));
        assert!(!like_("b", "[!a-c]"));
        assert!(like_("a]b", "a[]]b")); // literal ']' as first list member
        assert!(like_("3", "#"));
        assert!(!like_("z", "[0-9]"));
    }

    #[test]
    fn instr_start_and_compare() {
        assert_eq!(instr_(&[vs("hello world"), vs("o")], false), 5);
        // Leading numeric `start` = 6 → first 'o' at or after position 6.
        assert_eq!(
            instr_(&[Variant::from_i32(6), vs("hello world"), vs("o")], false),
            8
        );
        // InStrRev finds the last 'o'.
        assert_eq!(instr_(&[vs("hello world"), vs("o")], true), 8);
        // Text compare: per VBA `compare` requires `start`, so the 4-arg form
        // `InStr(1, "ABC", "b", 1)` is case-insensitive → 2.
        assert_eq!(
            instr_(
                &[
                    Variant::from_i32(1),
                    vs("ABC"),
                    vs("b"),
                    Variant::from_i32(1)
                ],
                false
            ),
            2
        );
        // Binary compare (default): no match for differing case.
        assert_eq!(instr_(&[vs("ABC"), vs("b")], false), 0);
    }

    #[test]
    fn instr_start_is_arity_not_type() {
        // 2 args is always `(string1, string2)` — even when `string1` is a numeric string,
        // it is NOT a leading `start`. `InStr("12", "2")` finds '2' at position 2.
        assert_eq!(instr_(&[vs("12"), vs("2")], false), 2);
    }

    #[test]
    fn instr_rev_honours_start_and_compare() {
        // Last 'a' overall is at 7; with start=4 the search is limited to "abca" → 4.
        assert_eq!(instr_(&[vs("abcabca"), vs("a")], true), 7);
        assert_eq!(
            instr_(&[vs("abcabca"), vs("a"), Variant::from_i32(4)], true),
            4
        );
        // compare is arg 4 (after start): text compare finds 'B' case-insensitively.
        assert_eq!(
            instr_(
                &[
                    vs("aBcaBc"),
                    vs("b"),
                    Variant::from_i32(-1),
                    Variant::from_i32(1)
                ],
                true
            ),
            5
        );
    }

    fn isnum(v: Variant) -> bool {
        is_numeric(&[v]).unwrap().as_bool().unwrap()
    }

    #[test]
    fn is_numeric_of_strings_and_values() {
        // Numeric strings (the bug fix): decimal / sign / dot / exponent / whitespace.
        assert!(isnum(vs("42")));
        assert!(isnum(vs("3.14")));
        assert!(isnum(vs("-1.5e3")));
        assert!(isnum(vs("  7  ")));
        assert!(isnum(vs("+1.5")));
        assert!(isnum(vs(".5")));
        // VBA hex/octal literals.
        assert!(isnum(vs("&HFF")));
        assert!(isnum(vs("&H1F&"))); // trailing Long-type suffix
        assert!(isnum(vs("&O17")));
        assert!(isnum(vs("&777"))); // bare-& octal
        // Non-numeric strings.
        assert!(!isnum(vs("12a")));
        assert!(!isnum(vs("")));
        assert!(!isnum(vs("abc")));
        assert!(!isnum(vs("nan"))); // Rust spelling VBA rejects
        assert!(!isnum(vs("inf")));
        assert!(!isnum(vs("&HZZ"))); // bad hex digits
        assert!(!isnum(vs("&H"))); // empty hex body
        // Genuine numeric subtypes and Empty are numeric (the VarType check).
        assert!(isnum(Variant::from_i32(42)));
        assert!(isnum(Variant::from_f64(3.5)));
        assert!(isnum(Variant::empty()));
        // Boolean, Date and Null are NOT numeric in VBA — even though Boolean/Date
        // would coerce to Double, `IsNumeric` rejects them by subtype.
        assert!(!isnum(Variant::from_bool(true)));
        assert!(!isnum(Variant::from_bool(false)));
        assert!(!isnum(Variant::from_date_f64(45000.0))); // a Date-typed Variant
        assert!(!isnum(Variant::null()));
    }

    #[test]
    fn partition_formats_documented_ranges() {
        assert_eq!(
            partition_(&[
                Variant::from_i32(-1),
                Variant::from_i32(0),
                Variant::from_i32(99),
                Variant::from_i32(5),
            ]),
            "   : -1"
        );
        assert_eq!(
            partition_(&[
                Variant::from_i32(0),
                Variant::from_i32(0),
                Variant::from_i32(99),
                Variant::from_i32(5),
            ]),
            "  0:  4"
        );
        assert_eq!(
            partition_(&[
                Variant::from_i32(99),
                Variant::from_i32(0),
                Variant::from_i32(99),
                Variant::from_i32(5),
            ]),
            " 95: 99"
        );
        assert_eq!(
            partition_(&[
                Variant::from_i32(100),
                Variant::from_i32(0),
                Variant::from_i32(99),
                Variant::from_i32(5),
            ]),
            "100:   "
        );
        assert_eq!(
            partition_(&[
                Variant::from_i32(1000),
                Variant::from_i32(100),
                Variant::from_i32(1010),
                Variant::from_i32(20),
            ]),
            "1000:1010"
        );
        assert_eq!(
            partition_(&[
                Variant::from_i32(100),
                Variant::from_i32(0),
                Variant::from_i32(1000),
                Variant::from_i32(1),
            ]),
            " 100: 100"
        );
    }

    #[test]
    fn partition_rounds_nulls_and_rejects_invalid_ranges() {
        assert_eq!(
            partition(&[
                Variant::from_f64(2.5),
                Variant::from_i32(0),
                Variant::from_i32(10),
                Variant::from_i32(2),
            ])
            .unwrap()
            .as_bstr()
            .unwrap()
            .as_str(),
            " 2: 3"
        );
        assert_eq!(
            partition(&[
                Variant::from_f64(3.5),
                Variant::from_i32(0),
                Variant::from_i32(10),
                Variant::from_i32(2),
            ])
            .unwrap()
            .as_bstr()
            .unwrap()
            .as_str(),
            " 4: 5"
        );
        assert_eq!(
            partition(&[
                Variant::null(),
                Variant::from_i32(0),
                Variant::from_i32(10),
                Variant::from_i32(2),
            ])
            .unwrap()
            .vtype(),
            VarType::Null
        );
        assert_eq!(
            partition(&[
                Variant::from_i32(1),
                Variant::from_i32(-1),
                Variant::from_i32(10),
                Variant::from_i32(2),
            ])
            .unwrap_err()
            .code,
            5
        );
        assert_eq!(
            partition(&[
                Variant::from_i32(1),
                Variant::from_i32(0),
                Variant::from_i32(0),
                Variant::from_i32(2),
            ])
            .unwrap_err()
            .code,
            5
        );
        assert_eq!(
            partition(&[
                Variant::from_i32(1),
                Variant::from_i32(0),
                Variant::from_i32(10),
                Variant::from_i32(0),
            ])
            .unwrap_err()
            .code,
            5
        );
        assert_eq!(
            partition(&[Variant::null()]).unwrap_err().code,
            5,
            "missing arguments are not masked by an earlier Null"
        );
    }

    fn chr_(id_args: &[Variant], wide: bool) -> String {
        let r = if wide { chr_w(id_args) } else { chr(id_args) };
        as_str(&r.unwrap()).unwrap()
    }
    fn asc_(s: &str, wide: bool) -> i32 {
        let r = if wide { asc_w(&[vs(s)]) } else { asc(&[vs(s)]) };
        r.unwrap().as_i32().unwrap()
    }

    #[test]
    fn chr_asc_ansi_vs_wide() {
        // ASCII (the common case) is identical for both variants.
        assert_eq!(chr_(&[Variant::from_i32(65)], false), "A");
        assert_eq!(chr_(&[Variant::from_i32(65)], true), "A");
        assert_eq!(asc_("A", false), 65);
        assert_eq!(asc_("A", true), 65);

        // 128..255: Chr decodes through the system ANSI codec — on the dev/CI host
        // (CP-1252 Windows ACP, or the off-Windows CP-1252 fallback) 150 → en-dash —
        // while ChrW is always the raw code point.
        assert_eq!(chr_(&[Variant::from_i32(150)], false), "\u{2013}"); // –
        assert_eq!(chr_(&[Variant::from_i32(150)], true), "\u{0096}"); // U+0096
        assert_ne!(
            chr_(&[Variant::from_i32(150)], false),
            chr_(&[Variant::from_i32(150)], true)
        );
        // Asc is the system-ANSI byte (150 on a CP-1252 host); AscW is the code point.
        assert_eq!(asc_("\u{2013}", false), 150); // en-dash → ANSI byte 150
        assert_eq!(asc_("\u{2013}", true), 8211); // en-dash → U+2013

        // The wide range (>255) agrees: Chr acts wide above 255.
        assert_eq!(chr_(&[Variant::from_i32(8364)], false), "\u{20AC}"); // €
        assert_eq!(chr_(&[Variant::from_i32(8364)], true), "\u{20AC}");
        // € round-trips: AscW is the code point, Asc is the CP-1252 byte 128.
        assert_eq!(asc_("\u{20AC}", true), 8364);
        assert_eq!(asc_("\u{20AC}", false), 128);

        // AscW returns an Integer (i16): code points > 32767 come back negative.
        assert_eq!(asc_("\u{8000}", true), (0x8000_u16 as i16) as i32);
        // Asc of an unmappable char is 63 ("?").
        assert_eq!(asc_("\u{4E2D}", false), 63);
    }

    #[test]
    fn str_comp_distinguishes_lone_surrogates() {
        // VBA strings are binary UTF-16 code-unit sequences; StrComp must NOT fold distinct
        // lone surrogate halves to U+FFFD (the lossy `as_str` path did, making them compare
        // equal). Binary mode (no third arg).
        let high = Variant::from_utf16_units(&[0xD800]);
        let low = Variant::from_utf16_units(&[0xDC00]);
        let same = Variant::from_utf16_units(&[0xD800]);
        // Distinct halves are unequal; 0xD800 < 0xDC00 by code unit, equal halves compare 0.
        assert_eq!(
            str_comp(&[high.clone(), low.clone()])
                .unwrap()
                .as_i32()
                .unwrap(),
            -1
        );
        assert_eq!(str_comp(&[low, high.clone()]).unwrap().as_i32().unwrap(), 1);
        assert_eq!(str_comp(&[high, same]).unwrap().as_i32().unwrap(), 0);
        // Text mode (compare = 1) still ASCII case-folds without disturbing surrogates.
        let upper = Variant::from_string("A".to_string());
        let lower = Variant::from_string("a".to_string());
        assert_eq!(
            str_comp(&[upper, lower, Variant::from_i32(1)])
                .unwrap()
                .as_i32()
                .unwrap(),
            0
        );
    }

    #[test]
    fn rnd_deterministic_bounded_and_repeatable() {
        let mut a = LibContext::default();
        let mut b = LibContext::default();
        // `Rnd` returns a Single; use the coercing helper to read it.
        let x = as_f64(&rnd(&[], &mut a).unwrap()).unwrap();
        let y = as_f64(&rnd(&[], &mut b).unwrap()).unwrap();
        assert_eq!(x, y, "default seed is deterministic");
        assert!((0.0..1.0).contains(&x), "result in [0,1): {x}");
        // Rnd(0) repeats the most recent value without advancing.
        let again = as_f64(&rnd(&[Variant::from_i32(0)], &mut a).unwrap()).unwrap();
        assert_eq!(x, again);
    }
}
