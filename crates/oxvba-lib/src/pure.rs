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
use oxvba_runtime::{Variant, safe_array::SafeArray, variant::VarType};
// The VBA serial ↔ civil calendar math is canonical in `oxvba_runtime::vba_date`; re-export it
// at `crate::pure::*` so this module's date functions (and `format.rs`) keep their call sites.
pub(crate) use oxvba_runtime::{
    civil_from_days, days_from_civil, serial_to_hms, serial_to_ymd, ymd_to_serial,
};

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

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
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
    let s = as_str(need(args, 0)?)?;
    let n = as_usize(need(args, 1)?)?;
    Ok(vstr(chars(&s).into_iter().take(n).collect::<String>()))
}

pub fn right(args: &[Variant]) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let c = chars(&s);
    let n = as_usize(need(args, 1)?)?.min(c.len());
    Ok(vstr(c[c.len() - n..].iter().collect::<String>()))
}

pub fn mid(args: &[Variant]) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let start = as_usize(need(args, 1)?)?.max(1);
    let c = chars(&s);
    let from = (start - 1).min(c.len());
    let take = match opt(args, 2) {
        Some(v) => as_usize(v)?,
        None => c.len() - from,
    };
    Ok(vstr(c[from..].iter().take(take).collect::<String>()))
}

/// `Mid(s, start, [count]) = value` — returns the spliced string (the VM stores
/// it back into the target slot).
pub fn mid_stmt(args: &[Variant]) -> LibResult<Variant> {
    let mut c = chars(&as_str(need(args, 0)?)?);
    let start = as_usize(need(args, 1)?)?.max(1) - 1;
    // args = [target, start, value] or [target, start, count, value]
    let (count, value) = if args.len() >= 4 {
        (Some(as_usize(need(args, 2)?)?), as_str(need(args, 3)?)?)
    } else {
        (None, as_str(need(args, 2)?)?)
    };
    let repl: Vec<char> = value.chars().collect();
    let limit = count.unwrap_or(repl.len()).min(repl.len());
    for (i, ch) in repl.into_iter().take(limit).enumerate() {
        if start + i < c.len() {
            c[start + i] = ch;
        }
    }
    Ok(vstr(c.into_iter().collect::<String>()))
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
        (true, true) => s.trim(),
        (true, false) => s.trim_start(),
        (false, true) => s.trim_end(),
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

/// FIDELITY: handles vbUpperCase/vbLowerCase/vbProperCase; other modes pass through.
pub fn str_conv(args: &[Variant]) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let mode = as_i32(need(args, 1)?)?;
    Ok(vstr(match mode {
        1 => s.to_uppercase(),
        2 => s.to_lowercase(),
        3 => proper_case(&s),
        _ => s,
    }))
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

// ── Math ──────────────────────────────────────────────────────────────────────

pub fn math1(args: &[Variant], f: impl Fn(f64) -> f64) -> LibResult<Variant> {
    Ok(vf64(f(as_f64(need(args, 0)?)?)))
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
        Some(v) => as_i32(v)?.max(0) as u32,
        None => 0,
    };
    let factor = 10f64.powi(digits as i32);
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

pub fn date_serial(args: &[Variant]) -> LibResult<Variant> {
    let y = as_i64(need(args, 0)?)?;
    let m = as_i64(need(args, 1)?)?;
    let d = as_i64(need(args, 2)?)?;
    Ok(Variant::from_date_f64(ymd_to_serial(y, m, d)))
}

pub fn time_serial(args: &[Variant]) -> LibResult<Variant> {
    let h = as_f64(need(args, 0)?)?;
    let m = as_f64(need(args, 1)?)?;
    let s = as_f64(need(args, 2)?)?;
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
    Ok(Variant::from_date_f64(serial.trunc()))
}

/// VBA `TimeValue` returns the TIME-OF-DAY part of its argument. Like `DateValue` it accepts a
/// `Date` or a numeric serial (not just a string) — `TimeValue(Now)` is the current time — and
/// must NOT round-trip a `Date` through string formatting (which dropped precision and, once
/// dates render as "General Date", misparsed the formatted text). Strings parse as `h[:m[:s]]`.
pub fn time_value(args: &[Variant]) -> LibResult<Variant> {
    let v = need(args, 0)?;
    let serial = match v.vtype() {
        VarType::Date => v.as_date_f64().unwrap_or(0.0),
        VarType::String => {
            let s = as_str(v)?;
            let parts: Vec<f64> = s
                .split(':')
                .map(|p| p.trim().parse::<f64>().unwrap_or(0.0))
                .collect();
            let h = parts.first().copied().unwrap_or(0.0);
            let m = parts.get(1).copied().unwrap_or(0.0);
            let sec = parts.get(2).copied().unwrap_or(0.0);
            (h * 3600.0 + m * 60.0 + sec) / 86400.0
        }
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
    Ok(Variant::from_date_f64(result))
}

/// FIDELITY: difference in the unit; calendar-unit diffs are approximate.
pub fn date_diff(args: &[Variant]) -> LibResult<Variant> {
    let interval = as_str(need(args, 0)?)?.to_lowercase();
    let a = as_f64(need(args, 1)?)?;
    let b = as_f64(need(args, 2)?)?;
    let days = b.floor() - a.floor();
    let result = match interval.as_str() {
        "d" | "y" | "w" => days,
        "ww" => (days / 7.0).trunc(),
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
                // VBA Weekday: 1 = Sunday. Sakamoto returns 0 = Sunday, so add 1.
                DatePart::Weekday => day_of_week(y as i32, m as u32, d as u32) + 1,
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

/// Lenient truthiness for an optional Boolean-ish flag argument.
fn as_bool_lenient(v: &Variant) -> bool {
    v.as_bool()
        .or_else(|| as_i32(v).ok().map(|n| n != 0))
        .unwrap_or(false)
}

// ── Conversion ─────────────────────────────────────────────────────────────────

pub fn hex(args: &[Variant]) -> LibResult<Variant> {
    Ok(vstr(format!("{:X}", as_i64(need(args, 0)?)?)))
}
pub fn oct(args: &[Variant]) -> LibResult<Variant> {
    Ok(vstr(format!("{:o}", as_i64(need(args, 0)?)?)))
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
    let trimmed = s.trim_start();
    let mut end = 0;
    for (i, c) in trimmed.char_indices() {
        if c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E') {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    Ok(vf64(trimmed[..end].parse::<f64>().unwrap_or(0.0)))
}
pub fn cdate(args: &[Variant]) -> LibResult<Variant> {
    let v = need(args, 0)?;
    match v.vtype() {
        VarType::Date => Ok(v.clone()),
        // A string parses as a date and/or time; everything else numeric IS the date
        // serial (the integer part is the day, the fraction the time of day).
        VarType::String => cdate_from_string(&as_str(v)?),
        _ => Ok(Variant::from_date_f64(conv_f64(v)?)),
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
    if let Some((date_part, time_part)) = s.rsplit_once(' ')
        && time_part.contains(':')
    {
        let (y, m, d) = parse_date(date_part.trim())?;
        let time = time_value(&[vstr(time_part.trim())])?
            .as_date_f64()
            .unwrap_or(0.0);
        return Ok(Variant::from_date_f64(ymd_to_serial(y, m, d) + time));
    }
    let (y, m, d) = parse_date(s)?;
    Ok(Variant::from_date_f64(ymd_to_serial(y, m, d)))
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
    let t = if opt_f64(args, 5, 0.0)? != 0.0 { 1.0 } else { 0.0 };
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
    let t = if opt_f64(args, 5, 0.0)? != 0.0 { 1.0 } else { 0.0 };
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
    let component = |index: usize| -> LibResult<i32> {
        Ok(as_i32(need(args, index)?)?.clamp(0, 255))
    };
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
        0, 8_388_608, 32_768, 8_421_376, 128, 8_388_736, 32_896, 12_632_256, 8_421_504,
        16_711_680, 65_280, 16_776_960, 255, 16_711_935, 65_535, 16_777_215,
    ];
    let index = as_i32(need(args, 0)?)?;
    if !(0..=15).contains(&index) {
        return Err(LibError::invalid_call("QBColor index must be 0 to 15"));
    }
    Ok(vi32(PALETTE[index as usize]))
}

// ── Information ──────────────────────────────────────────────────────────────────

pub fn is_vtype(args: &[Variant], pred: impl Fn(VarType) -> bool) -> LibResult<Variant> {
    Ok(vbool(pred(need(args, 0)?.vtype())))
}

pub fn var_type(args: &[Variant]) -> LibResult<Variant> {
    Ok(vi32(need(args, 0)?.vtype() as i32))
}

pub fn type_name(args: &[Variant]) -> LibResult<Variant> {
    let name = match need(args, 0)?.vtype() {
        VarType::Empty => "Empty",
        VarType::Null => "Null",
        VarType::Integer => "Integer",
        VarType::Long => "Long",
        VarType::Single => "Single",
        VarType::Double => "Double",
        VarType::Currency => "Currency",
        VarType::Date => "Date",
        VarType::String => "String",
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
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(value) = parse_vba_prefixed_integer(t) {
        return Some(value as f64);
    }
    // Decimal / sign / exponent. Require the first significant byte to be a
    // sign/dot/digit so we reject Rust's "inf"/"nan"/"infinity" (which start with a
    // letter) that VBA does not treat as numeric, while still accepting "42", "+1.5",
    // ".5", "-3", "1.5e3", "1E3".
    if matches!(t.as_bytes()[0], b'+' | b'-' | b'.' | b'0'..=b'9') {
        return t.parse::<f64>().ok().filter(|value| value.is_finite());
    }
    None
}

fn parse_vba_prefixed_integer(t: &str) -> Option<i64> {
    let (sign, rest) = if let Some(rest) = t.strip_prefix('-') {
        (-1_i64, rest)
    } else if let Some(rest) = t.strip_prefix('+') {
        (1_i64, rest)
    } else {
        (1_i64, t)
    };
    // VBA hex (`&H…`) / octal (`&O…` or bare `&` + octal digits) integer literals,
    // with an optional trailing `&` Long-type suffix.
    let rest = rest.strip_prefix('&')?;
    let (radix, digits) = match rest.as_bytes().first() {
        Some(b'H' | b'h') => (16, &rest[1..]),
        Some(b'O' | b'o') => (8, &rest[1..]),
        // Bare `&` followed by octal digits (VBA's terse octal form).
        _ => (8, rest),
    };
    // The width-based two's-complement sign rule applies to `Val`/`CInt`/`CLng`
    // of a `&H…`/`&O…` *string* exactly as to a literal — `Val("&HFFFF")` is -1
    // (verified live). Honour an optional `%`/`&`/`^` type character too.
    let suffix = match digits.as_bytes().last() {
        Some(&b @ (b'%' | b'&' | b'^')) => Some(b),
        _ => None,
    };
    let digits = digits.trim_end_matches(['&', '%', '^']);
    if digits.is_empty() {
        return None;
    }
    let magnitude = u64::from_str_radix(digits, radix).ok()?;
    let width = oxvba_runtime::vba_radix_width(magnitude, suffix)?;
    oxvba_runtime::vba_radix_signed_value(magnitude, width).checked_mul(sign)
}

pub fn is_date(args: &[Variant]) -> LibResult<Variant> {
    let v = need(args, 0)?;
    // VBA `IsDate` is True for a Date value or a STRING recognizable as a valid date/time
    // (using the same lenient parser as `CDate`); a raw numeric, Empty, Null, Boolean, or
    // object is NOT a date to `IsDate` (even though `CDate` would convert a number).
    let ok = match v.vtype() {
        VarType::Date => true,
        VarType::String => as_str(v).ok().is_some_and(|s| cdate_from_string(&s).is_ok()),
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
