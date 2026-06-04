//! Pure (host-free) base-library bodies: strings, math, date/time arithmetic,
//! conversion, random, financial, information, and a first-cut Collection.
//!
//! Where VBA's exact semantics are intricate (Option Compare modes on string
//! search, `Format` masks, iterative `IRR`/`Rate`, the 1900 leap-year quirk,
//! keyed Collections) the body is a functional first-cut marked `FIDELITY:` —
//! it computes a correct result for the common case and is refined against the
//! reference later.

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

/// `InStr`/`InStrRev`. FIDELITY: binary compare only (no `Option Compare Text`).
pub fn instr(args: &[Variant], rev: bool) -> LibResult<Variant> {
    let hay = as_str(need(args, 0)?)?;
    let needle = as_str(need(args, 1)?)?;
    let pos = if rev { hay.rfind(&needle) } else { hay.find(&needle) };
    Ok(vi32(match pos {
        Some(byte) => hay[..byte].encode_utf16().count() as i32 + 1,
        None => 0,
    }))
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

/// FIDELITY: binary compare only.
pub fn str_comp(args: &[Variant]) -> LibResult<Variant> {
    let a = as_str(need(args, 0)?)?;
    let b = as_str(need(args, 1)?)?;
    Ok(vi32(match a.cmp(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }))
}

/// FIDELITY: supports `?`, `*`, `#`; does not yet support `[charlist]` ranges.
pub fn like(args: &[Variant]) -> LibResult<Variant> {
    let s: Vec<char> = as_str(need(args, 0)?)?.chars().collect();
    let p: Vec<char> = as_str(need(args, 1)?)?.chars().collect();
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
        c => !s.is_empty() && s[0] == c && like_match(&s[1..], &p[1..]),
    }
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

/// FIDELITY: heavy first-cut — no-format → CStr; numeric/general masks approximate.
pub fn format(args: &[Variant]) -> LibResult<Variant> {
    let value = need(args, 0)?;
    let mask = match opt(args, 1) {
        Some(v) => as_str(v)?,
        None => return cstr(args),
    };
    // Minimal: numeric fixed-decimal masks like "0.00"; else CStr.
    if let Some(dot) = mask.find('.') {
        let decimals = mask[dot + 1..].chars().filter(|c| *c == '0' || *c == '#').count();
        if let Ok(n) = as_f64(value) {
            return Ok(vstr(format!("{n:.*}", decimals)));
        }
    }
    cstr(args)
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
// FIDELITY: proleptic Gregorian; does not reproduce the Excel 1900 leap-year bug.

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

fn serial_to_ymd(serial: f64) -> (i64, i64, i64) {
    civil_from_days(serial.floor() as i64 + VBA_EPOCH_DAYS_FROM_UNIX)
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
        // VBA Weekday: 1 = Sunday. Day 1 (1899-12-31) was a Sunday.
        DatePart::Weekday => (((serial.floor() as i64 % 7) + 7) % 7 + 1) as i32,
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

fn next_rng(ctx: &mut LibContext) -> f64 {
    // SplitMix64 step → [0,1). FIDELITY: not VBA's exact LCG sequence.
    ctx.rng_state = ctx.rng_state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = ctx.rng_state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

pub fn rnd(args: &[Variant], ctx: &mut LibContext) -> LibResult<Variant> {
    if let Some(v) = opt(args, 0) {
        let n = as_f64(v)?;
        if n < 0.0 {
            ctx.rng_state = (n.to_bits()) | 1;
        }
    }
    Ok(Variant::from_f32(next_rng(ctx) as f32))
}

pub fn randomize(args: &[Variant], ctx: &mut LibContext) -> LibResult<Variant> {
    let seed = match opt(args, 0) {
        Some(v) => as_f64(v)?.to_bits(),
        None => ctx.rng_state.rotate_left(17),
    };
    ctx.rng_state = seed | 1;
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

/// FIDELITY: Newton first-cut on the annuity equation.
pub fn rate(args: &[Variant]) -> LibResult<Variant> {
    let (n, pmt, pv) = (as_f64(need(args, 0)?)?, as_f64(need(args, 1)?)?, as_f64(need(args, 2)?)?);
    let fv = opt_f64(args, 3, 0.0)?;
    let t = opt_f64(args, 4, 0.0)?;
    let mut r = opt_f64(args, 5, 0.1)?;
    for _ in 0..100 {
        let g = (1.0 + r).powf(n);
        let f = pv * g + pmt * (1.0 + r * t) * (g - 1.0) / r + fv;
        let dr = 1e-6;
        let g2 = (1.0 + r + dr).powf(n);
        let f2 = pv * g2 + pmt * (1.0 + (r + dr) * t) * (g2 - 1.0) / (r + dr) + fv;
        let df = (f2 - f) / dr;
        if df.abs() < 1e-12 {
            break;
        }
        let next = r - f / df;
        if (next - r).abs() < 1e-9 {
            r = next;
            break;
        }
        r = next;
    }
    Ok(vf64(r))
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
