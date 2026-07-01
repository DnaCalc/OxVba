//! VBA `Date` serial ↔ civil-calendar decomposition and display formatting.
//!
//! A VBA `Date` is an OLE-Automation serial: the integer part is days since 1899-12-30 and the
//! fractional part is the time of day. The calendar is proleptic Gregorian (no Excel 1900
//! leap-year quirk). This is the single source of truth for the serial math — `oxvba-lib`'s
//! date functions re-import it, and the value-display formatters here (`variant_to_vba_string`
//! / `print_display_text` / `write_display_text`) use it so a `Date` renders as a formatted
//! date string, not its raw serial number.

/// Days from the Unix epoch (1970-01-01) back to the VBA epoch (1899-12-30).
pub const VBA_EPOCH_DAYS_FROM_UNIX: i64 = -25569;

/// Days since 1970-01-01 for a proleptic-Gregorian `(y, m, d)` (Howard Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Proleptic-Gregorian `(y, m, d)` for a days-since-1970-01-01 count (inverse of
/// [`days_from_civil`]).
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
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

/// The VBA `Date` serial for a `(y, m, d)`.
pub fn ymd_to_serial(y: i64, m: i64, d: i64) -> f64 {
    (days_from_civil(y, m, d) - VBA_EPOCH_DAYS_FROM_UNIX) as f64
}

fn serial_day_and_seconds(serial: f64) -> (i64, i64) {
    let mut day = serial.trunc() as i64;
    let frac = (serial - serial.trunc()).abs();
    let mut total = (frac * 86_400.0).round() as i64;
    if total >= 86_400 {
        day += 1;
        total -= 86_400;
    }
    (day, total)
}

/// The `(year, month, day)` of a VBA `Date` serial.
pub fn serial_to_ymd(serial: f64) -> (i64, i64, i64) {
    let (day, _) = serial_day_and_seconds(serial);
    civil_from_days(day + VBA_EPOCH_DAYS_FROM_UNIX)
}

/// The `(hour, minute, second)` of a VBA `Date` serial's time-of-day fraction.
pub fn serial_to_hms(serial: f64) -> (i64, i64, i64) {
    let (_, total) = serial_day_and_seconds(serial);
    (total / 3600, (total % 3600) / 60, total % 60)
}

/// VBA's long-time display: `h:mm:ss AM/PM` (12-hour, no leading zero on the hour).
fn format_long_time(h: i64, mi: i64, s: i64) -> String {
    let (h12, ampm) = match h {
        0 => (12, "AM"),
        1..=11 => (h, "AM"),
        12 => (12, "PM"),
        _ => (h - 12, "PM"),
    };
    format!("{h12}:{mi:02}:{s:02} {ampm}")
}

/// VBA's "General Date" display (en-US): a value with a real date and a time renders as
/// `M/D/YYYY h:mm:ss AM/PM`; a date with no time renders as `M/D/YYYY`; a value whose date part
/// is the epoch (1899-12-30, integer part 0) renders as the time only, `h:mm:ss AM/PM`.
pub fn format_general_date(serial: f64) -> String {
    let (y, m, d) = serial_to_ymd(serial);
    let (h, mi, s) = serial_to_hms(serial);
    let has_date = serial.trunc() != 0.0;
    let has_time = (h, mi, s) != (0, 0, 0);
    let date = format!("{m}/{d}/{y}");
    let time = format_long_time(h, mi, s);
    match (has_date, has_time) {
        (true, true) => format!("{date} {time}"),
        (true, false) => date,
        (false, _) => time,
    }
}

/// VBA `Write #` date literal: `#YYYY-MM-DD HH:MM:SS#` (or `#YYYY-MM-DD#` at midnight) — the
/// locale-independent, machine-readable form `Write` emits (distinct from "General Date").
pub fn format_write_date(serial: f64) -> String {
    let (y, m, d) = serial_to_ymd(serial);
    let (h, mi, s) = serial_to_hms(serial);
    if (h, mi, s) == (0, 0, 0) {
        format!("#{y:04}-{m:02}-{d:02}#")
    } else {
        format!("#{y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}#")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_date_date_only() {
        // 2020-01-15 (no time).
        let serial = ymd_to_serial(2020, 1, 15);
        assert_eq!(format_general_date(serial), "1/15/2020");
    }

    #[test]
    fn general_date_with_time() {
        // 2020-01-15 13:30:00 → 1:30:00 PM.
        let serial = ymd_to_serial(2020, 1, 15) + (13.0 * 3600.0 + 30.0 * 60.0) / 86_400.0;
        assert_eq!(format_general_date(serial), "1/15/2020 1:30:00 PM");
    }

    #[test]
    fn general_date_time_only_and_midnight() {
        // Epoch-date noon → time only.
        assert_eq!(format_general_date(0.5), "12:00:00 PM");
        // Exactly 0 (epoch midnight) → time only, 12:00:00 AM.
        assert_eq!(format_general_date(0.0), "12:00:00 AM");
    }

    #[test]
    fn negative_serial_uses_whole_number_date_part() {
        assert_eq!(format_general_date(-1.25), "12/29/1899 6:00:00 AM");
    }

    #[test]
    fn rounded_time_can_carry_to_next_date() {
        let almost_next_day = ymd_to_serial(2020, 1, 15) + 86_399.6 / 86_400.0;
        assert_eq!(format_general_date(almost_next_day), "1/16/2020");
    }

    #[test]
    fn write_date_form() {
        assert_eq!(
            format_write_date(ymd_to_serial(2020, 1, 15)),
            "#2020-01-15#"
        );
        let serial = ymd_to_serial(2020, 1, 15) + 0.5;
        assert_eq!(format_write_date(serial), "#2020-01-15 12:00:00#");
    }
}
