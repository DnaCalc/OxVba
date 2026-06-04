//! Pure (host-free) base-library bodies: strings, math, date/time arithmetic,
//! conversion, random, financial, information, and a first-cut Collection.
//!
//! The `Format` engine lives in [`crate::format`]. The remaining `FIDELITY:`
//! markers are features absent from the legacy VM too: keyed `Collection`
//! access (awaits the vm2 object model) and `StrConv`'s CJK/encoding modes.

use crate::{
    LibContext, LibError, LibResult, as_f64, as_i32, as_i64, as_str, as_usize, need, opt, vbool,
    vf64, vi32, vstr, vunit,
};
use oxvba_runtime::{Variant, safe_array::SafeArray, variant::VarType};

// ── Strings ──────────────────────────────────────────────────────────────────

pub fn len(args: &[Variant]) -> LibResult<Variant> {
    Ok(vi32(as_str(need(args, 0)?)?.encode_utf16().count() as i32))
}

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

/// Optional trailing compare-mode argument: 0 = binary (default), 1 = text. The
/// legacy VM threaded `Option Compare` at compile time; `oxvba-temp-b2b` passes
/// that mode as a trailing argument here.
fn text_compare(args: &[Variant], index: usize) -> LibResult<bool> {
    match opt(args, index) {
        Some(v) => Ok(as_i32(v)? == 1),
        None => Ok(false),
    }
}

/// Text-mode normalization, ported from the legacy `normalize_for_compare`:
/// ASCII case-folding. FIDELITY: not Unicode/locale-aware.
fn norm_compare(s: String, text: bool) -> String {
    if text { s.to_ascii_lowercase() } else { s }
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
/// 1 = text → ASCII case-insensitive) is supplied by `oxvba-temp-b2b` from the
/// compile-time `Option Compare`. (`InStrRev`'s own `start` argument awaits the
/// b2b canonical arg layout.)
pub fn instr(args: &[Variant], rev: bool) -> LibResult<Variant> {
    let base = if !rev && args.first().is_some_and(is_numeric_typed) { 1 } else { 0 };
    let start = if base == 1 { as_i32(need(args, 0)?)? } else { 1 };
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
    let pos = if rev {
        hay.rfind(&needle)
    } else {
        hay[byte_start..].find(&needle).map(|b| byte_start + b)
    };
    Ok(vi32(match pos {
        Some(byte) => hay[..byte].encode_utf16().count() as i32 + 1,
        None => 0,
    }))
}

fn is_numeric_typed(v: &Variant) -> bool {
    matches!(
        v.vtype(),
        VarType::Integer
            | VarType::Long
            | VarType::LongLong
            | VarType::Single
            | VarType::Double
            | VarType::Currency
            | VarType::Decimal
            | VarType::Byte
    )
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

pub fn split(args: &[Variant]) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let delim = match opt(args, 1) {
        Some(v) => as_str(v)?,
        None => " ".to_string(),
    };
    let parts: Vec<Variant> = if delim.is_empty() {
        vec![vstr(s)]
    } else {
        s.split(&delim).map(vstr).collect()
    };
    Ok(Variant::from_safearray(SafeArray::from_variants(parts)))
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

/// FIDELITY: basic `Replace(expr, find, replace)`; ignores start/count/compare.
pub fn replace(args: &[Variant]) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let find = as_str(need(args, 1)?)?;
    let with = as_str(need(args, 2)?)?;
    Ok(vstr(if find.is_empty() {
        s
    } else {
        s.replace(&find, &with)
    }))
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
    let a = norm_compare(as_str(need(args, 0)?)?, text);
    let b = norm_compare(as_str(need(args, 1)?)?, text);
    Ok(vi32(match a.cmp(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }))
}

/// `Like` — supports `?`, `*`, `#` and an optional trailing compare mode
/// (0=binary, 1=text). Already richer than the legacy VM (which implemented Like
/// as plain equality). FIDELITY: `[charlist]` ranges not yet supported.
pub fn like(args: &[Variant]) -> LibResult<Variant> {
    let text = text_compare(args, 2)?;
    let s: Vec<char> = norm_compare(as_str(need(args, 0)?)?, text).chars().collect();
    let p: Vec<char> = norm_compare(as_str(need(args, 1)?)?, text).chars().collect();
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

pub fn chr(args: &[Variant]) -> LibResult<Variant> {
    let code = as_i32(need(args, 0)?)? as u32;
    let ch = char::from_u32(code).ok_or_else(|| LibError::invalid_call("invalid character code"))?;
    Ok(vstr(ch.to_string()))
}

pub fn asc(args: &[Variant]) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let ch = s.chars().next().ok_or_else(|| LibError::invalid_call("Asc of empty string"))?;
    Ok(vi32(ch as i32))
}

pub fn space(args: &[Variant]) -> LibResult<Variant> {
    Ok(vstr(" ".repeat(as_usize(need(args, 0)?)?)))
}

/// `String(number, character)` — `character` may be a code or a string.
pub fn string_repeat(args: &[Variant]) -> LibResult<Variant> {
    let count = as_usize(need(args, 0)?)?;
    let arg = need(args, 1)?;
    let ch = if let Ok(code) = as_i32(arg) {
        char::from_u32(code as u32).unwrap_or(' ')
    } else {
        as_str(arg)?.chars().next().unwrap_or(' ')
    };
    Ok(vstr(ch.to_string().repeat(count)))
}

pub fn str_reverse(args: &[Variant]) -> LibResult<Variant> {
    Ok(vstr(as_str(need(args, 0)?)?.chars().rev().collect::<String>()))
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
// worksheet 1900 leap-year quirk is not part of VBA `Date` semantics.)

const VBA_EPOCH_DAYS_FROM_UNIX: i64 = -25569; // days from 1970-01-01 back to 1899-12-30

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn ymd_to_serial(y: i64, m: i64, d: i64) -> f64 {
    (days_from_civil(y, m, d) - VBA_EPOCH_DAYS_FROM_UNIX) as f64
}

pub(crate) fn serial_to_ymd(serial: f64) -> (i64, i64, i64) {
    civil_from_days(serial.floor() as i64 + VBA_EPOCH_DAYS_FROM_UNIX)
}

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
    Ok(Variant::from_date_f64((h * 3600.0 + m * 60.0 + s) / 86400.0))
}

/// FIDELITY: parses ISO-ish `YYYY-MM-DD` and `M/D/YYYY`; locale formats later.
pub fn date_value(args: &[Variant]) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let (y, m, d) = parse_date(&s)?;
    Ok(Variant::from_date_f64(ymd_to_serial(y, m, d)))
}

pub fn time_value(args: &[Variant]) -> LibResult<Variant> {
    let s = as_str(need(args, 0)?)?;
    let parts: Vec<f64> = s
        .split(':')
        .map(|p| p.trim().parse::<f64>().unwrap_or(0.0))
        .collect();
    let h = parts.first().copied().unwrap_or(0.0);
    let m = parts.get(1).copied().unwrap_or(0.0);
    let sec = parts.get(2).copied().unwrap_or(0.0);
    Ok(Variant::from_date_f64((h * 3600.0 + m * 60.0 + sec) / 86400.0))
}

fn parse_date(s: &str) -> LibResult<(i64, i64, i64)> {
    let err = || LibError::type_mismatch(format!("cannot parse date `{s}`"));
    if let Some((y, rest)) = s.split_once('-') {
        let (m, d) = rest.split_once('-').ok_or_else(err)?;
        return Ok((y.trim().parse().map_err(|_| err())?, m.trim().parse().map_err(|_| err())?, d.trim().parse().map_err(|_| err())?));
    }
    if let Some((m, rest)) = s.split_once('/') {
        let (d, y) = rest.split_once('/').ok_or_else(err)?;
        return Ok((y.trim().parse().map_err(|_| err())?, m.trim().parse().map_err(|_| err())?, d.trim().parse().map_err(|_| err())?));
    }
    Err(err())
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
        _ => return Err(LibError::invalid_call(format!("unknown DateAdd interval `{interval}`"))),
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
        _ => return Err(LibError::invalid_call(format!("unknown DateDiff interval `{interval}`"))),
    };
    Ok(vf64(result.trunc()))
}

#[derive(Clone, Copy)]
pub enum DatePart {
    Year,
    Month,
    Day,
    Weekday,
}

pub fn date_part(args: &[Variant], part: DatePart) -> LibResult<Variant> {
    let serial = as_f64(need(args, 0)?)?;
    let (y, m, d) = serial_to_ymd(serial);
    Ok(vi32(match part {
        DatePart::Year => y as i32,
        DatePart::Month => m as i32,
        DatePart::Day => d as i32,
        // VBA Weekday: 1 = Sunday. Sakamoto returns 0 = Sunday, so add 1.
        DatePart::Weekday => day_of_week(y as i32, m as u32, d as u32) + 1,
    }))
}

pub fn month_name(args: &[Variant]) -> LibResult<Variant> {
    const NAMES: [&str; 12] = [
        "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ];
    let n = as_usize(need(args, 0)?)?;
    NAMES
        .get(n.wrapping_sub(1))
        .map(|s| vstr(*s))
        .ok_or_else(|| LibError::invalid_call("MonthName index out of range"))
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
    Ok(oxvba_runtime::coerce::coerce_to(need(args, 0)?, VarType::Date)?)
}
pub fn cverr(args: &[Variant]) -> LibResult<Variant> {
    Ok(Variant::from_error_code(as_i32(need(args, 0)?)?))
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
fn numeric_arg<'a>(args: &'a [Variant], index: usize) -> Option<&'a Variant> {
    opt(args, index).filter(|v| !matches!(v.vtype(), VarType::Empty))
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

pub fn fv(args: &[Variant]) -> LibResult<Variant> {
    let (i, n, pmt) = (as_f64(need(args, 0)?)?, as_f64(need(args, 1)?)?, as_f64(need(args, 2)?)?);
    let pv = opt_f64(args, 3, 0.0)?;
    let t = opt_f64(args, 4, 0.0)?;
    Ok(vf64(if i == 0.0 {
        -(pv + pmt * n)
    } else {
        let g = (1.0 + i).powf(n);
        -(pv * g + pmt * (1.0 + i * t) * (g - 1.0) / i)
    }))
}

pub fn pv(args: &[Variant]) -> LibResult<Variant> {
    let (i, n, pmt) = (as_f64(need(args, 0)?)?, as_f64(need(args, 1)?)?, as_f64(need(args, 2)?)?);
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
    let (i, n, pv) = (as_f64(need(args, 0)?)?, as_f64(need(args, 1)?)?, as_f64(need(args, 2)?)?);
    let fv = opt_f64(args, 3, 0.0)?;
    let t = opt_f64(args, 4, 0.0)?;
    Ok(vf64(if i == 0.0 {
        -(pv + fv) / n
    } else {
        let g = (1.0 + i).powf(n);
        -(fv + pv * g) / ((1.0 + i * t) * (g - 1.0) / i)
    }))
}

pub fn nper(args: &[Variant]) -> LibResult<Variant> {
    let (i, pmt, pv) = (as_f64(need(args, 0)?)?, as_f64(need(args, 1)?)?, as_f64(need(args, 2)?)?);
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
        return Err(LibError::invalid_call("MIRR requires positive and negative flows"));
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
        (rate_func(r + step, nper, pmt, pv, 0.0, due) - rate_func(r - step, nper, pmt, pv, 0.0, due))
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
    let due = if opt_f64(args, 4, 0.0)? != 0.0 { 1.0 } else { 0.0 };
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
    Ok(vbool(oxvba_runtime::coerce::coerce_to(need(args, 0)?, VarType::Double).is_ok()))
}

pub fn is_date(args: &[Variant]) -> LibResult<Variant> {
    let v = need(args, 0)?;
    Ok(vbool(
        matches!(v.vtype(), VarType::Date)
            || oxvba_runtime::coerce::coerce_to(v, VarType::Date).is_ok(),
    ))
}

// ── Collection (FIDELITY: SafeArray-backed, index-only; no keys) ─────────────────

fn collection_elems(v: &Variant) -> Vec<Variant> {
    v.as_safearray()
        .and_then(|a| a.variant_elements())
        .unwrap_or_default()
}

pub fn collection_add(args: &[Variant]) -> LibResult<Variant> {
    let mut items = collection_elems(need(args, 0)?);
    items.push(need(args, 1)?.clone());
    Ok(Variant::from_safearray(SafeArray::from_variants(items)))
}

pub fn collection_item(args: &[Variant]) -> LibResult<Variant> {
    let items = collection_elems(need(args, 0)?);
    let index = as_usize(need(args, 1)?)?;
    items
        .get(index.wrapping_sub(1))
        .cloned()
        .ok_or_else(|| LibError::new(9, "subscript out of range"))
}

pub fn collection_remove(args: &[Variant]) -> LibResult<Variant> {
    let mut items = collection_elems(need(args, 0)?);
    let index = as_usize(need(args, 1)?)?;
    if index >= 1 && index <= items.len() {
        items.remove(index - 1);
    }
    Ok(Variant::from_safearray(SafeArray::from_variants(items)))
}

pub fn collection_count(args: &[Variant]) -> LibResult<Variant> {
    Ok(vi32(collection_elems(need(args, 0)?).len() as i32))
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
        assert_eq!(instr_(&[Variant::from_i32(6), vs("hello world"), vs("o")], false), 8);
        // InStrRev finds the last 'o'.
        assert_eq!(instr_(&[vs("hello world"), vs("o")], true), 8);
        // Text compare (mode 1): case-insensitive.
        assert_eq!(instr_(&[vs("ABC"), vs("b"), Variant::from_i32(1)], false), 2);
        // Binary compare (default): no match for differing case.
        assert_eq!(instr_(&[vs("ABC"), vs("b")], false), 0);
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
