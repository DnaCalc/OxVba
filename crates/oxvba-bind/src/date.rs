//! Date-literal parsing — converts a `#…#` date literal's text to an OLE
//! automation date serial (the `f64` bit pattern stored in `CoreConst::Date`).
//!
//! The grammar accepts the common VBA date forms: `#YYYY/MM/DD#`, `#MM/DD/YYYY#`,
//! `#Mon DD YYYY#`, and `#DD Mon YYYY#`, with `/`, `-`, `.`, `,` or whitespace
//! separators. Time-of-day components are not supported (a literal that splits
//! into more than three parts is rejected); this matches the legacy front end.

/// Parse a `#…#` date literal's full text (including the delimiters) to the
/// OLE-serial `f64` bit pattern used by `CoreConst::Date`. Returns `None` for a
/// malformed or out-of-range date.
pub(crate) fn parse_date_literal_serial_bits(text: &str) -> Option<u64> {
    let inner = text.strip_prefix('#')?.strip_suffix('#')?.trim();
    let packed = parse_date_literal_to_packed(inner)?;
    Some(packed_date_to_ole_serial(packed)?.to_bits())
}

fn parse_date_literal_to_packed(text: &str) -> Option<i32> {
    let normalized = text.trim().replace([',', '.', '-', '/'], " ");
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    let packed = match parts.as_slice() {
        [year, month, day] if year.len() == 4 => {
            let year = year.parse::<i32>().ok()?;
            let month = parse_month_token(month).or_else(|| month.parse::<i32>().ok())?;
            let day = day.parse::<i32>().ok()?;
            year.saturating_mul(10_000) + month.saturating_mul(100) + day
        }
        [month, day, year] if parse_month_token(month).is_some() => {
            let month = parse_month_token(month)?;
            let day = day.parse::<i32>().ok()?;
            let year = year.parse::<i32>().ok()?;
            year.saturating_mul(10_000) + month.saturating_mul(100) + day
        }
        [month, day, year] if is_unambiguous_numeric_month_day(month, day) => {
            let month = month.parse::<i32>().ok()?;
            let day = day.parse::<i32>().ok()?;
            let year = year.parse::<i32>().ok()?;
            year.saturating_mul(10_000) + month.saturating_mul(100) + day
        }
        [day, month, year] => {
            let day = day.parse::<i32>().ok()?;
            let month = parse_month_token(month).or_else(|| month.parse::<i32>().ok())?;
            let year = year.parse::<i32>().ok()?;
            year.saturating_mul(10_000) + month.saturating_mul(100) + day
        }
        _ => return None,
    };
    packed_date_components(packed)?;
    Some(packed)
}

fn is_unambiguous_numeric_month_day(month: &str, day: &str) -> bool {
    let Ok(month) = month.parse::<i32>() else {
        return false;
    };
    let Ok(day) = day.parse::<i32>() else {
        return false;
    };
    (1..=12).contains(&month) && day > 12
}

fn parse_month_token(text: &str) -> Option<i32> {
    match text.trim().to_ascii_lowercase().as_str() {
        "jan" | "january" => Some(1),
        "feb" | "february" => Some(2),
        "mar" | "march" => Some(3),
        "apr" | "april" => Some(4),
        "may" => Some(5),
        "jun" | "june" => Some(6),
        "jul" | "july" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "sept" | "september" => Some(9),
        "oct" | "october" => Some(10),
        "nov" | "november" => Some(11),
        "dec" | "december" => Some(12),
        _ => None,
    }
}

fn packed_date_components(packed: i32) -> Option<(i32, u32, u32)> {
    let year = packed / 10_000;
    let month = ((packed / 100) % 100) as u32;
    let day = (packed % 100) as u32;
    if !(1..=12).contains(&month) {
        return None;
    }
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    };
    if max_day == 0 || day == 0 || day > max_day {
        return None;
    }
    Some((year, month, day))
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn packed_date_to_ole_serial(packed: i32) -> Option<f64> {
    let (year, month, day) = packed_date_components(packed)?;
    let serial = days_from_civil(year, month, day) + 25_569;
    Some(serial as f64)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = i64::from(year) - i64::from((month <= 2) as i32);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_index = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_index + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso_and_us_forms() {
        // 2020-01-01 is OLE serial 43831.
        assert_eq!(
            parse_date_literal_serial_bits("#1/1/2020#").map(f64::from_bits),
            Some(43831.0)
        );
        assert_eq!(
            parse_date_literal_serial_bits("#2020/01/01#").map(f64::from_bits),
            Some(43831.0)
        );
        assert_eq!(
            parse_date_literal_serial_bits("#Jan 1 2020#").map(f64::from_bits),
            Some(43831.0)
        );
    }

    #[test]
    fn rejects_invalid_and_timed() {
        assert_eq!(parse_date_literal_serial_bits("#2/30/2020#"), None);
        assert_eq!(parse_date_literal_serial_bits("#1/1/2020 12:00#"), None);
    }
}
