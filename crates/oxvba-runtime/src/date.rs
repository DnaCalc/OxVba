//! Deterministic VBA date text parsing shared by compile-time constants and VM
//! coercion. Locale-sensitive numeric forms such as `2/3/2026` intentionally
//! return `None`; callers that need host-locale parsing must provide an explicit
//! host collation/date service.

pub fn parse_date_literal_serial_bits(text: &str) -> Option<u64> {
    let inner = text.strip_prefix('#')?.strip_suffix('#')?.trim();
    parse_date_text_serial_bits(inner)
}

pub fn parse_date_text_serial_bits(text: &str) -> Option<u64> {
    Some(parse_date_text_serial(text)?.to_bits())
}

pub fn parse_date_text_serial(text: &str) -> Option<f64> {
    let packed = parse_date_text_to_packed(text)?;
    packed_date_to_ole_serial(packed)
}

fn parse_date_text_to_packed(text: &str) -> Option<i32> {
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
        [day, month, year] if parse_month_token(month).is_some() => {
            let day = day.parse::<i32>().ok()?;
            let month = parse_month_token(month)?;
            let year = year.parse::<i32>().ok()?;
            year.saturating_mul(10_000) + month.saturating_mul(100) + day
        }
        [first, second, year] => {
            let first = first.parse::<i32>().ok()?;
            let second = second.parse::<i32>().ok()?;
            let year = year.parse::<i32>().ok()?;
            let (month, day) = match unambiguous_numeric_date_order(first, second)? {
                NumericDateOrder::MonthDayYear => (first, second),
                NumericDateOrder::DayMonthYear => (second, first),
            };
            year.saturating_mul(10_000) + month.saturating_mul(100) + day
        }
        _ => return None,
    };
    packed_date_components(packed)?;
    Some(packed)
}

enum NumericDateOrder {
    MonthDayYear,
    DayMonthYear,
}

fn unambiguous_numeric_date_order(first: i32, second: i32) -> Option<NumericDateOrder> {
    let first_can_be_month = (1..=12).contains(&first);
    let second_can_be_month = (1..=12).contains(&second);
    match (first_can_be_month, second_can_be_month) {
        // `2/3/2026` is locale-sensitive; `2/2/2026` has the same result in either
        // numeric order, so it remains deterministic.
        (true, true) if first == second => Some(NumericDateOrder::MonthDayYear),
        (true, true) => None,
        (true, false) => Some(NumericDateOrder::MonthDayYear),
        (false, true) => Some(NumericDateOrder::DayMonthYear),
        (false, false) => None,
    }
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
    Some((days_from_civil(year, month, day) + 25_569) as f64)
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
    use super::{parse_date_literal_serial_bits, parse_date_text_serial_bits};

    #[test]
    fn date_literals_accept_unambiguous_numeric_orders() {
        let expected = Some(46_081.0f64.to_bits());
        assert_eq!(parse_date_literal_serial_bits("#2026-02-28#"), expected);
        assert_eq!(parse_date_literal_serial_bits("#2/28/2026#"), expected);
        assert_eq!(parse_date_literal_serial_bits("#28/2/2026#"), expected);
    }

    #[test]
    fn date_literals_reject_ambiguous_numeric_orders() {
        assert_eq!(parse_date_literal_serial_bits("#2/3/2026#"), None);
        assert_eq!(parse_date_literal_serial_bits("#12/11/2026#"), None);
        assert!(parse_date_literal_serial_bits("#1/1/2026#").is_some());
    }

    #[test]
    fn date_text_accepts_same_deterministic_orders_as_literals() {
        let expected = Some(46_081.0f64.to_bits());
        assert_eq!(parse_date_text_serial_bits("2026-02-28"), expected);
        assert_eq!(parse_date_text_serial_bits("February 28, 2026"), expected);
        assert_eq!(parse_date_text_serial_bits("28/2/2026"), expected);
        assert_eq!(parse_date_text_serial_bits("2/3/2026"), None);
    }
}
