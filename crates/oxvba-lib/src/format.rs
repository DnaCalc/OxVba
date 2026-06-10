//! VBA `Format(expr, format)` engine.
//!
//! Covers the documented surface:
//! - **Named numeric:** General Number, Currency, Fixed, Standard, Percent,
//!   Scientific; **Boolean:** Yes/No, True/False, On/Off.
//! - **Named date:** General/Long/Medium/Short Date and Long/Medium/Short Time.
//! - **Custom numeric masks:** `0` `#` `.` `,` `%`, `E+`/`E-` exponential, and
//!   `;`-separated positive;negative;zero sections.
//! - **Custom date/time masks:** `y`/`m`/`d`/`h`/`n`/`s` tokens (1, 2, and named
//!   widths), `AM/PM`, with the VBA rule that `m`/`mm` after an hour token means
//!   minutes.
//!
//! FIDELITY: string-format characters (`@ & < >`), quoted literal runs in masks,
//! and locale-specific currency symbols / separators are first-cut. Rounding
//! follows Rust's formatter (round-half-to-even), matching VBA's bankers' rule
//! for the common cases.

use oxvba_runtime::coerce::coerce_to;
use oxvba_runtime::variant::VarType;
use oxvba_runtime::{Variant, variant_to_vba_string};

use crate::pure::{day_of_week, serial_to_ymd};

const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August",
    "September", "October", "November", "December",
];
const MONTHS_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEKDAYS: [&str; 7] = [
    "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday",
];
const WEEKDAYS_ABBR: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// Entry point: `Format(value, mask)`. `mask` is non-empty (the empty/omitted
/// case is handled by the caller as a plain `CStr`).
pub fn apply(value: &Variant, mask: &str) -> String {
    let trimmed = mask.trim();
    if trimmed.is_empty() {
        return general_string(value);
    }
    if let Some(out) = named(value, trimmed) {
        return out;
    }
    if is_date_mask(trimmed) {
        format_date(value, trimmed)
    } else {
        format_numeric(value, trimmed)
    }
}

fn general_string(value: &Variant) -> String {
    variant_to_vba_string(value).map(|b| b.as_str()).unwrap_or_default()
}

fn num(value: &Variant) -> f64 {
    coerce_to(value, VarType::Double).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn serial_of(value: &Variant) -> f64 {
    value.as_date_f64().unwrap_or_else(|| num(value))
}

// ── Named formats ─────────────────────────────────────────────────────────────

fn named(value: &Variant, mask: &str) -> Option<String> {
    match mask.to_ascii_lowercase().as_str() {
        "general number" => Some(general_number(num(value))),
        "currency" => Some(format_numeric(value, "$#,##0.00")),
        "fixed" => Some(format_numeric(value, "0.00")),
        "standard" => Some(format_numeric(value, "#,##0.00")),
        "percent" => Some(format_numeric(value, "0.00%")),
        "scientific" => Some(scientific(num(value), 2)),
        "yes/no" => Some(bool_word(value, "Yes", "No")),
        "true/false" => Some(bool_word(value, "True", "False")),
        "on/off" => Some(bool_word(value, "On", "Off")),
        "general date" => Some(format_date(value, "general date")),
        "long date" => Some(format_date(value, "dddd, mmmm d, yyyy")),
        "medium date" => Some(format_date(value, "dd-mmm-yy")),
        "short date" => Some(format_date(value, "m/d/yyyy")),
        "long time" => Some(format_date(value, "h:nn:ss AM/PM")),
        "medium time" => Some(format_date(value, "h:nn AM/PM")),
        "short time" => Some(format_date(value, "hh:nn")),
        _ => None,
    }
}

fn bool_word(value: &Variant, yes: &str, no: &str) -> String {
    if num(value) != 0.0 { yes } else { no }.to_string()
}

/// VBA "General Number": full precision, no thousands separator, trailing zeros
/// trimmed.
fn general_number(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

// ── Numeric masks ───────────────────────────────────────────────────────────

fn is_date_mask(mask: &str) -> bool {
    let lower = mask.to_ascii_lowercase();
    // Date masks never use `0`/`#` digit placeholders; numeric masks never use
    // the calendar tokens. This split is unambiguous for the common surface.
    let has_placeholder = lower.contains('0') || lower.contains('#');
    let has_calendar = lower.contains(['y', 'm', 'd', 'h', 'n', 's', ':']);
    has_calendar && !has_placeholder
}

fn format_numeric(value: &Variant, mask: &str) -> String {
    let n = num(value);
    let sections: Vec<&str> = mask.split(';').collect();
    let (section, force_minus) = if n < 0.0 {
        if sections.len() >= 2 && !sections[1].is_empty() {
            (sections[1], false) // the negative section carries its own sign
        } else {
            (sections[0], true)
        }
    } else if n == 0.0 && sections.len() >= 3 {
        (sections[2], false)
    } else {
        (sections[0], false)
    };
    let body = render_section(section, n.abs());
    if force_minus { format!("-{body}") } else { body }
}

fn render_section(section: &str, mut value: f64) -> String {
    let chars: Vec<char> = section.chars().collect();
    if let Some(out) = try_scientific(&chars, value) {
        return out;
    }
    if section.contains('%') {
        value *= 100.0;
    }
    let first = chars.iter().position(|c| matches!(c, '0' | '#'));
    let last = chars.iter().rposition(|c| matches!(c, '0' | '#'));
    let (first, last) = match (first, last) {
        (Some(f), Some(l)) => (f, l),
        _ => return section.to_string(), // no placeholders → pure literal
    };

    let prefix: String = chars[..first].iter().collect();
    let core = &chars[first..=last];
    let suffix: String = chars[last + 1..].iter().collect();

    let dot = core.iter().position(|c| *c == '.');
    let (int_pat, frac_pat): (&[char], &[char]) = match dot {
        Some(i) => (&core[..i], &core[i + 1..]),
        None => (core, &[]),
    };
    let grouping = int_pat.contains(&',');
    let int_zeros = int_pat.iter().filter(|c| **c == '0').count();
    let frac_total = frac_pat.iter().filter(|c| matches!(c, '0' | '#')).count();
    let frac_forced = frac_pat.iter().filter(|c| **c == '0').count();

    let rendered = format!("{value:.frac_total$}");
    let (int_s, frac_s) = match rendered.split_once('.') {
        Some((i, f)) => (i.to_string(), f.to_string()),
        None => (rendered, String::new()),
    };

    let mut int_s = int_s;
    while int_s.len() < int_zeros {
        int_s.insert(0, '0');
    }
    if grouping {
        int_s = group_thousands(&int_s);
    }

    let mut frac_s = frac_s;
    while frac_s.len() > frac_forced && frac_s.ends_with('0') {
        frac_s.pop();
    }
    let frac_out = if frac_s.is_empty() { String::new() } else { format!(".{frac_s}") };

    format!("{prefix}{int_s}{frac_out}{suffix}")
}

fn group_thousands(digits: &str) -> String {
    let n = digits.len();
    let mut out = String::with_capacity(n + n / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (n - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn try_scientific(chars: &[char], value: f64) -> Option<String> {
    let e = chars.iter().position(|c| matches!(c, 'e' | 'E'))?;
    if !matches!(chars.get(e + 1), Some('+') | Some('-')) {
        return None;
    }
    let decimals = match chars.iter().position(|c| *c == '.') {
        Some(d) if d < e => chars[d + 1..e].iter().filter(|c| matches!(c, '0' | '#')).count(),
        _ => 0,
    };
    Some(scientific(value, decimals))
}

fn scientific(value: f64, decimals: usize) -> String {
    let raw = format!("{value:.decimals$E}");
    match raw.split_once('E') {
        Some((mantissa, exp)) => {
            let (sign, digits) = match exp.strip_prefix('-') {
                Some(d) => ('-', d),
                None => ('+', exp.trim_start_matches('+')),
            };
            format!("{mantissa}E{sign}{digits:0>2}")
        }
        None => raw,
    }
}

// ── Date / time masks ─────────────────────────────────────────────────────────

struct DateParts {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    dow: usize, // 0 = Sunday … 6 = Saturday
}

fn decompose(value: &Variant) -> DateParts {
    let serial = serial_of(value);
    let (year, month, day) = serial_to_ymd(serial);
    let frac = serial - serial.floor();
    let total = ((frac * 86_400.0).round() as i64).rem_euclid(86_400);
    let dow = day_of_week(year as i32, month as u32, day as u32).rem_euclid(7) as usize;
    DateParts {
        year,
        month,
        day,
        hour: total / 3600,
        minute: (total % 3600) / 60,
        second: total % 60,
        dow,
    }
}

fn format_date(value: &Variant, mask: &str) -> String {
    let p = decompose(value);
    // "General Date" routes here with a non-token mask; render date + present
    // time components like VBA's default.
    if mask.eq_ignore_ascii_case("general date") {
        let serial = serial_of(value);
        let has_time = serial.fract() != 0.0;
        let has_date = serial.trunc() != 0.0;
        return match (has_date, has_time) {
            (true, true) => format!(
                "{} {}",
                format_date(value, "m/d/yyyy"),
                format_date(value, "h:nn:ss AM/PM")
            ),
            (false, true) => format_date(value, "h:nn:ss AM/PM"),
            _ => format_date(value, "m/d/yyyy"),
        };
    }

    let lower = mask.to_ascii_lowercase();
    let use_12h = lower.contains("am/pm") || lower.contains("ampm") || lower.contains("a/p");
    let chars: Vec<char> = mask.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    // VBA: `m`/`mm` mean minutes when they follow an hour token (across literal
    // separators), month otherwise.
    let mut after_hour = false;
    while i < chars.len() {
        let rest = &chars[i..];
        let (text, advance, hour_token, calendar_token) = if starts_ci(rest, "yyyy") {
            (format!("{:04}", p.year), 4, false, true)
        } else if starts_ci(rest, "yy") {
            (format!("{:02}", p.year.rem_euclid(100)), 2, false, true)
        } else if starts_ci(rest, "mmmm") {
            (month_name(p.month, false), 4, false, true)
        } else if starts_ci(rest, "mmm") {
            (month_name(p.month, true), 3, false, true)
        } else if starts_ci(rest, "mm") {
            let v = if after_hour { p.minute } else { p.month };
            (format!("{v:02}"), 2, false, true)
        } else if starts_ci(rest, "m") {
            let v = if after_hour { p.minute } else { p.month };
            (v.to_string(), 1, false, true)
        } else if starts_ci(rest, "dddd") {
            (WEEKDAYS[p.dow].to_string(), 4, false, true)
        } else if starts_ci(rest, "ddd") {
            (WEEKDAYS_ABBR[p.dow].to_string(), 3, false, true)
        } else if starts_ci(rest, "dd") {
            (format!("{:02}", p.day), 2, false, true)
        } else if starts_ci(rest, "d") {
            (p.day.to_string(), 1, false, true)
        } else if starts_ci(rest, "hh") {
            (format!("{:02}", hour_display(p.hour, use_12h)), 2, true, true)
        } else if starts_ci(rest, "h") {
            (hour_display(p.hour, use_12h).to_string(), 1, true, true)
        } else if starts_ci(rest, "nn") {
            (format!("{:02}", p.minute), 2, false, true)
        } else if starts_ci(rest, "n") {
            (p.minute.to_string(), 1, false, true)
        } else if starts_ci(rest, "ss") {
            (format!("{:02}", p.second), 2, false, true)
        } else if starts_ci(rest, "s") {
            (p.second.to_string(), 1, false, true)
        } else if starts_ci(rest, "am/pm") {
            (if p.hour < 12 { "AM" } else { "PM" }.to_string(), 5, false, true)
        } else if starts_ci(rest, "ampm") {
            (if p.hour < 12 { "AM" } else { "PM" }.to_string(), 4, false, true)
        } else if starts_ci(rest, "a/p") {
            (if p.hour < 12 { "A" } else { "P" }.to_string(), 3, false, true)
        } else {
            (chars[i].to_string(), 1, false, false)
        };
        out.push_str(&text);
        i += advance;
        if calendar_token {
            after_hour = hour_token;
        }
        // literal separators leave `after_hour` unchanged so "hh:mm" reads mm as
        // minutes.
    }
    out
}

fn month_name(month: i64, abbr: bool) -> String {
    let idx = (month.clamp(1, 12) - 1) as usize;
    if abbr { MONTHS_ABBR[idx] } else { MONTHS[idx] }.to_string()
}

fn hour_display(hour24: i64, use_12h: bool) -> i64 {
    if use_12h {
        let h = hour24 % 12;
        if h == 0 { 12 } else { h }
    } else {
        hour24
    }
}

fn starts_ci(rest: &[char], token: &str) -> bool {
    let t: Vec<char> = token.chars().collect();
    rest.len() >= t.len() && rest[..t.len()].iter().zip(&t).all(|(a, b)| a.eq_ignore_ascii_case(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(n: f64, mask: &str) -> String {
        apply(&Variant::from_f64(n), mask)
    }
    fn fmt_date(serial: f64, mask: &str) -> String {
        apply(&Variant::from_date_f64(serial), mask)
    }

    #[test]
    fn numeric_masks() {
        assert_eq!(fmt(1234.5, "#,##0.00"), "1,234.50");
        assert_eq!(fmt(-1234.5, "#,##0.00"), "-1,234.50");
        assert_eq!(fmt(1234.567, "0.00"), "1234.57");
        assert_eq!(fmt(5.0, "00000"), "00005");
        assert_eq!(fmt(0.5, "0%"), "50%");
        assert_eq!(fmt(0.5, "0.0%"), "50.0%");
        assert_eq!(fmt(12345.0, "0.00E+00"), "1.23E+04");
    }

    #[test]
    fn named_formats() {
        assert_eq!(fmt(1234.5, "Currency"), "$1,234.50");
        assert_eq!(fmt(1234.5, "Standard"), "1,234.50");
        assert_eq!(fmt(0.5, "Percent"), "50.00%");
        assert_eq!(fmt(0.0, "Yes/No"), "No");
        assert_eq!(fmt(1.0, "True/False"), "True");
        assert_eq!(fmt(42.0, "General Number"), "42");
    }

    #[test]
    fn date_masks() {
        // Serial 0 = 1899-12-30 (the VBA Date epoch).
        assert_eq!(fmt_date(0.0, "yyyy-mm-dd"), "1899-12-30");
        // Half a day = noon.
        assert_eq!(fmt_date(0.5, "hh:nn:ss"), "12:00:00");
        assert_eq!(fmt_date(0.5, "h:nn AM/PM"), "12:00 PM");
        // `mm` after an hour token means minutes, across the `:` separator.
        assert_eq!(fmt_date(0.5, "hh:mm"), "12:00");
    }
}
