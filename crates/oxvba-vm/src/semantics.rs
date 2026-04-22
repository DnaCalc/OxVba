//! Shared pure semantic functions for VBA runtime operations.
//!
//! These functions are extracted from the interpreter so they can be reused
//! by the JIT runtime helpers without duplication.

use oxvba_com::{ComCallbackToken, ComMemberToken, ComSubscriptionToken, DynamicMemberSelector};
use oxvba_compiler::bytecode::{
    RuntimeAssignmentIntent, RuntimeAssignmentTargetKind, StringCompareMode,
};
use oxvba_runtime::runtime_value::validate_date_range;
use oxvba_runtime::{
    BindingHandle, CurrencyValue, F64Subtype, F64Value, ObjectHandle, RuntimeValue, bstr::BStr,
};

const SECONDS_PER_DAY: f64 = 86_400.0;

// ── Coercion & Type Checks ────────────────────────────────────────────

pub fn either_null(lhs: &RuntimeValue, rhs: &RuntimeValue) -> bool {
    matches!(lhs, RuntimeValue::Null) || matches!(rhs, RuntimeValue::Null)
}

pub fn either_error(lhs: &RuntimeValue, rhs: &RuntimeValue) -> bool {
    matches!(lhs, RuntimeValue::ErrorCode(_)) || matches!(rhs, RuntimeValue::ErrorCode(_))
}

pub fn either_is_f64(lhs: &RuntimeValue, rhs: &RuntimeValue) -> bool {
    matches!(
        lhs,
        RuntimeValue::F64(_) | RuntimeValue::Currency(_) | RuntimeValue::Decimal(_)
    ) || matches!(
        rhs,
        RuntimeValue::F64(_) | RuntimeValue::Currency(_) | RuntimeValue::Decimal(_)
    )
}

pub fn runtime_value_as_f64(value: &RuntimeValue) -> Result<f64, String> {
    match value {
        RuntimeValue::Empty => Ok(0.0),
        RuntimeValue::I32(v) => Ok(*v as f64),
        RuntimeValue::I64(v) => Ok(*v as f64),
        RuntimeValue::F64(v) => Ok(v.as_f64()),
        RuntimeValue::Bool(v) => Ok(if *v { -1.0 } else { 0.0 }),
        RuntimeValue::Currency(c) => Ok(c.scaled_i64() as f64 / CurrencyValue::SCALE as f64),
        RuntimeValue::Decimal(d) => {
            let mag = d.magnitude_u128() as f64;
            let scale = 10f64.powi(d.scale() as i32);
            Ok(if d.is_negative() {
                -(mag / scale)
            } else {
                mag / scale
            })
        }
        other => Err(format!("cannot coerce {:?} to f64", other)),
    }
}

pub fn runtime_value_to_numeric_compat(value: &RuntimeValue, field: &str) -> Result<f64, String> {
    match value {
        RuntimeValue::String(text) => {
            let trimmed = text.as_str().trim();
            if trimmed.is_empty() {
                Ok(0.0)
            } else {
                trimmed
                    .parse::<f64>()
                    .map_err(|_| format!("{field} requires numeric-compatible text, got {text:?}"))
            }
        }
        other => runtime_value_as_f64(other),
    }
}

pub fn runtime_value_to_i32_compat(value: &RuntimeValue, field: &str) -> Result<i32, String> {
    let numeric = runtime_value_to_numeric_compat(value, field)?;
    if !numeric.is_finite() || numeric < i32::MIN as f64 || numeric > i32::MAX as f64 {
        return Err(format!("{field} exceeds i32 range: {numeric}"));
    }
    Ok(numeric.trunc() as i32)
}

pub fn runtime_value_is_explicit_zero_carrier(value: &RuntimeValue) -> bool {
    match value {
        RuntimeValue::I32(v) => *v == 0,
        RuntimeValue::I64(v) => *v == 0,
        RuntimeValue::Bool(v) => !*v,
        RuntimeValue::Object(handle) => handle.raw() == 0,
        RuntimeValue::ObjectHandle(handle) => handle.raw() == 0,
        RuntimeValue::BindingHandle(handle) => handle.raw() == 0,
        _ => false,
    }
}

pub fn runtime_value_to_text(value: &RuntimeValue, field: &str) -> Result<String, String> {
    match value {
        RuntimeValue::Empty => Ok(String::new()),
        RuntimeValue::String(text) => Ok(text.as_str().to_string()),
        RuntimeValue::I32(v) => Ok(v.to_string()),
        RuntimeValue::I64(v) => Ok(v.to_string()),
        RuntimeValue::Bool(v) => Ok(if *v {
            "-1".to_string()
        } else {
            "0".to_string()
        }),
        RuntimeValue::F64(v) => {
            if v.subtype() == F64Subtype::Date {
                format_date_serial_digits(v.as_f64()).or_else(|_| Ok(v.as_f64().to_string()))
            } else {
                Ok(v.as_f64().to_string())
            }
        }
        RuntimeValue::Currency(c) => {
            Ok((c.scaled_i64() as f64 / CurrencyValue::SCALE as f64).to_string())
        }
        RuntimeValue::Decimal(d) => {
            let mag = d.magnitude_u128() as f64;
            let scale = 10f64.powi(d.scale() as i32);
            let value = if d.is_negative() {
                -(mag / scale)
            } else {
                mag / scale
            };
            Ok(value.to_string())
        }
        RuntimeValue::Null => Err(format!("{field} requires text-compatible value, got Null")),
        RuntimeValue::ErrorCode(code) => Ok((*code as i32).to_string()),
        RuntimeValue::ArrayIntent(_) => Err(format!(
            "{field} requires scalar text-compatible value, got array"
        )),
        RuntimeValue::Object(handle) => Ok(handle.raw().to_string()),
        RuntimeValue::ObjectHandle(handle) => Ok(handle.raw().to_string()),
        RuntimeValue::BindingHandle(handle) => Ok(handle.raw().to_string()),
    }
}

pub fn runtime_split_count_bounded(
    value: &RuntimeValue,
    delimiter: &RuntimeValue,
) -> Result<RuntimeValue, String> {
    let text = runtime_value_to_text(value, "Split src")?;
    let delimiter = runtime_value_to_text(delimiter, "Split delimiter")?;
    let count = if delimiter.is_empty() {
        1
    } else {
        i32::try_from(text.split(&delimiter).count())
            .map_err(|_| "Split piece count exceeded i32 range".to_string())?
    };
    Ok(RuntimeValue::I32(count))
}

pub fn runtime_join_bounded(
    value: &RuntimeValue,
    delimiter: &RuntimeValue,
) -> Result<RuntimeValue, String> {
    let _ = runtime_value_to_text(delimiter, "Join delimiter")?;
    let out = match value {
        RuntimeValue::ArrayIntent(array) => i32::try_from(array.len)
            .map_err(|_| "Join array length exceeded i32 range".to_string())?,
        RuntimeValue::Empty => 0,
        RuntimeValue::I32(v) => {
            if oxvba_runtime::safe_array::is_array_tag(*v) {
                oxvba_runtime::safe_array::array_len_from_tag(*v)
                    .and_then(|count| i32::try_from(count).ok())
                    .unwrap_or(*v)
            } else {
                *v
            }
        }
        RuntimeValue::I64(v) => {
            i32::try_from(*v).map_err(|_| format!("Join src exceeds i32 range: {v}"))?
        }
        RuntimeValue::Bool(v) => {
            if *v {
                -1
            } else {
                0
            }
        }
        RuntimeValue::String(text) => text.as_str().parse::<i32>().unwrap_or(0),
        RuntimeValue::F64(_) | RuntimeValue::Currency(_) | RuntimeValue::Decimal(_) => {
            let numeric = runtime_value_as_f64(value)?;
            if !numeric.is_finite() || numeric < i32::MIN as f64 || numeric > i32::MAX as f64 {
                return Err(format!("Join src exceeds i32 range: {numeric}"));
            }
            numeric.trunc() as i32
        }
        other => runtime_value_to_i32_compat(other, "Join src")?,
    };
    Ok(RuntimeValue::I32(out))
}

pub fn runtime_like_bounded(
    lhs: &RuntimeValue,
    pattern: &RuntimeValue,
    mode: StringCompareMode,
) -> Result<RuntimeValue, String> {
    let lhs = normalize_for_compare(runtime_value_to_text(lhs, "Like lhs")?, mode);
    let pattern = normalize_for_compare(runtime_value_to_text(pattern, "Like pattern")?, mode);
    Ok(RuntimeValue::I32(if lhs == pattern { -1 } else { 0 }))
}

pub fn runtime_strconv_bounded(
    src: &RuntimeValue,
    conversion: &RuntimeValue,
) -> Result<RuntimeValue, String> {
    let text = runtime_value_to_text(src, "StrConv source")?;
    let conv = runtime_value_to_i32_compat(conversion, "StrConv conversion")?;
    let result = match conv {
        1 => text.to_uppercase(),
        2 => text.to_lowercase(),
        3 => proper_case(&text),
        _ => text,
    };
    Ok(RuntimeValue::String(BStr::from(result)))
}

pub fn runtime_chr_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let value = runtime_value_to_i32_compat(src, "Chr operand")?;
    let ch = char::from_u32(value as u32).unwrap_or('\0');
    Ok(RuntimeValue::String(BStr::from(ch.to_string())))
}

pub fn runtime_asc_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let text = runtime_value_to_text(src, "Asc operand")?;
    let code = if text.is_empty() {
        0
    } else {
        text.as_bytes()[0] as i32
    };
    Ok(RuntimeValue::I32(code))
}

pub fn runtime_space_bounded(count: &RuntimeValue) -> Result<RuntimeValue, String> {
    let count = runtime_value_to_i32_compat(count, "Space count")?;
    Ok(RuntimeValue::String(BStr::from(
        " ".repeat(count.max(0) as usize),
    )))
}

pub fn runtime_string_repeat_bounded(
    count: &RuntimeValue,
    ch: &RuntimeValue,
) -> Result<RuntimeValue, String> {
    let count = runtime_value_to_i32_compat(count, "String$ count")?;
    let ch = match ch {
        RuntimeValue::String(s) => {
            if s.is_empty() {
                '\0'
            } else {
                s.as_str().chars().next().unwrap_or('\0')
            }
        }
        other => char::from_u32(runtime_value_to_i32_compat(other, "String$ char")? as u32)
            .unwrap_or('\0'),
    };
    Ok(RuntimeValue::String(BStr::from(
        ch.to_string().repeat(count.max(0) as usize),
    )))
}

pub fn runtime_hex_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let value = runtime_value_to_i32_compat(src, "Hex operand")?;
    Ok(RuntimeValue::String(BStr::from(format!("{:X}", value as u32))))
}

pub fn runtime_oct_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let value = runtime_value_to_i32_compat(src, "Oct operand")?;
    Ok(RuntimeValue::String(BStr::from(format!("{:o}", value as u32))))
}

pub fn runtime_val_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let result = match src {
        RuntimeValue::String(s) => {
            let trimmed = s.as_str().trim();
            if trimmed.is_empty() {
                RuntimeValue::I32(0)
            } else if let Ok(n) = trimmed.parse::<i64>() {
                if n >= i32::MIN as i64 && n <= i32::MAX as i64 {
                    RuntimeValue::I32(n as i32)
                } else {
                    RuntimeValue::F64(F64Value::from_f64(n as f64))
                }
            } else if let Ok(f) = trimmed.parse::<f64>() {
                if f == f.trunc() && f >= i32::MIN as f64 && f <= i32::MAX as f64 {
                    RuntimeValue::I32(f as i32)
                } else {
                    RuntimeValue::F64(F64Value::from_f64(f))
                }
            } else {
                let mut end = 0usize;
                let bytes = trimmed.as_bytes();
                if !bytes.is_empty()
                    && (bytes[0] == b'-' || bytes[0] == b'+' || bytes[0].is_ascii_digit())
                {
                    end = 1;
                    let mut has_dot = false;
                    while end < bytes.len() {
                        if bytes[end].is_ascii_digit() {
                            end += 1;
                        } else if bytes[end] == b'.' && !has_dot {
                            has_dot = true;
                            end += 1;
                        } else {
                            break;
                        }
                    }
                }
                if end > 0 {
                    if let Ok(f) = trimmed[..end].parse::<f64>() {
                        if f == f.trunc() && f >= i32::MIN as f64 && f <= i32::MAX as f64 {
                            RuntimeValue::I32(f as i32)
                        } else {
                            RuntimeValue::F64(F64Value::from_f64(f))
                        }
                    } else {
                        RuntimeValue::I32(0)
                    }
                } else {
                    RuntimeValue::I32(0)
                }
            }
        }
        RuntimeValue::I32(_) | RuntimeValue::I64(_) | RuntimeValue::F64(_) => src.clone(),
        RuntimeValue::Empty => RuntimeValue::I32(0),
        other => {
            let numeric = runtime_value_to_numeric_compat(other, "Val src")?;
            if numeric == numeric.trunc()
                && numeric >= i32::MIN as f64
                && numeric <= i32::MAX as f64
            {
                RuntimeValue::I32(numeric as i32)
            } else {
                RuntimeValue::F64(F64Value::from_f64(numeric))
            }
        }
    };
    Ok(result)
}

pub fn runtime_abs_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    match src {
        RuntimeValue::Null => Ok(RuntimeValue::Null),
        RuntimeValue::F64(v) => Ok(RuntimeValue::F64(F64Value::from_f64(v.as_f64().abs()))),
        other => {
            let value = runtime_value_to_i32_compat(other, "Abs operand")?;
            Ok(RuntimeValue::I32(if value == i32::MIN {
                i32::MAX
            } else {
                value.abs()
            }))
        }
    }
}

pub fn runtime_sgn_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    match src {
        RuntimeValue::Null => Ok(RuntimeValue::Null),
        RuntimeValue::F64(v) => {
            let value = v.as_f64();
            Ok(RuntimeValue::I32(if value > 0.0 {
                1
            } else if value < 0.0 {
                -1
            } else {
                0
            }))
        }
        other => Ok(RuntimeValue::I32(
            runtime_value_to_i32_compat(other, "Sgn operand")?.signum(),
        )),
    }
}

fn runtime_round_i32_bounded(value: i32, digits: i32) -> i32 {
    if digits >= 0 {
        return value;
    }
    let factor = 10_i32.saturating_pow((-digits) as u32).max(1);
    let half = factor / 2;
    if value >= 0 {
        ((value + half) / factor) * factor
    } else {
        ((value - half) / factor) * factor
    }
}

pub fn runtime_round_bounded(
    src: &RuntimeValue,
    digits: Option<&RuntimeValue>,
) -> Result<RuntimeValue, String> {
    let value = runtime_value_to_i32_compat(src, "Round operand")?;
    let digits = match digits {
        Some(value) => runtime_value_to_i32_compat(value, "Round digits")?,
        None => 0,
    };
    Ok(RuntimeValue::I32(runtime_round_i32_bounded(value, digits)))
}

pub fn runtime_sqr_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let value = runtime_value_to_i32_compat(src, "Sqr operand")?;
    Ok(RuntimeValue::I32(
        (value.saturating_abs() as f64).sqrt() as i32
    ))
}

pub fn runtime_sin_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let value = runtime_value_to_i32_compat(src, "Sin operand")?;
    Ok(RuntimeValue::I32((value as f64).sin().round() as i32))
}

pub fn runtime_cos_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let value = runtime_value_to_i32_compat(src, "Cos operand")?;
    Ok(RuntimeValue::I32((value as f64).cos().round() as i32))
}

pub fn runtime_log_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let value = runtime_value_to_i32_compat(src, "Log operand")?;
    Ok(RuntimeValue::I32(if value > 0 {
        (value as f64).ln().round() as i32
    } else {
        0
    }))
}

pub fn runtime_exp_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let value = runtime_value_to_i32_compat(src, "Exp operand")?;
    Ok(RuntimeValue::I32((value as f64).exp().round() as i32))
}

pub fn runtime_atn_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let value = runtime_value_to_i32_compat(src, "Atn operand")?;
    Ok(RuntimeValue::I32((value as f64).atan().round() as i32))
}

pub fn runtime_tan_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let value = runtime_value_to_i32_compat(src, "Tan operand")?;
    Ok(RuntimeValue::I32((value as f64).tan().round() as i32))
}

pub fn runtime_month_name_bounded(src: &RuntimeValue) -> Result<RuntimeValue, String> {
    let month = runtime_value_to_i32_compat(src, "MonthName operand")?;
    let name = match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    };
    Ok(RuntimeValue::String(BStr::from(name.to_string())))
}

pub fn runtime_date_serial_bounded(
    year: &RuntimeValue,
    month: &RuntimeValue,
    day: &RuntimeValue,
) -> Result<RuntimeValue, String> {
    runtime_date_serial_value(
        runtime_value_to_i32_compat(year, "DateSerial year")?,
        runtime_value_to_i32_compat(month, "DateSerial month")?,
        runtime_value_to_i32_compat(day, "DateSerial day")?,
    )
}

pub fn runtime_time_serial_bounded(
    hour: &RuntimeValue,
    minute: &RuntimeValue,
    second: &RuntimeValue,
) -> Result<RuntimeValue, String> {
    runtime_time_serial_value(
        runtime_value_to_i32_compat(hour, "TimeSerial hour")?,
        runtime_value_to_i32_compat(minute, "TimeSerial minute")?,
        runtime_value_to_i32_compat(second, "TimeSerial second")?,
    )
}

pub fn runtime_date_add_bounded(
    interval: &RuntimeValue,
    number: &RuntimeValue,
    date: &RuntimeValue,
) -> Result<RuntimeValue, String> {
    runtime_date_add_value(
        runtime_value_to_i32_compat(interval, "DateAdd interval")?,
        runtime_value_to_i32_compat(number, "DateAdd number")?,
        date,
    )
}

pub fn runtime_date_diff_bounded(
    interval: &RuntimeValue,
    date1: &RuntimeValue,
    date2: &RuntimeValue,
) -> Result<i32, String> {
    runtime_date_diff_value(
        runtime_value_to_i32_compat(interval, "DateDiff interval")?,
        date1,
        date2,
    )
}

pub fn runtime_mid_stmt_bounded(
    target: &RuntimeValue,
    start: &RuntimeValue,
    count: Option<&RuntimeValue>,
    value: &RuntimeValue,
) -> Result<RuntimeValue, String> {
    let base = runtime_value_to_text(target, "MidStmt target")?;
    let repl = runtime_value_to_text(value, "MidStmt value")?;
    let start = runtime_value_to_usize(start)?;
    let start_idx = if start == 0 {
        0
    } else {
        (start - 1).min(base.len())
    };
    if start_idx >= base.len() {
        return Ok(target.clone());
    }
    let end_idx = match count {
        Some(count) => {
            let count = runtime_value_to_usize(count)?;
            (start_idx + count).min(base.len())
        }
        None => base.len(),
    };
    let replace_len = end_idx.saturating_sub(start_idx);
    let replace_text = if replace_len >= repl.len() {
        repl.as_str()
    } else {
        &repl[..replace_len]
    };
    let mut out = String::with_capacity(base.len() - replace_len + replace_text.len());
    out.push_str(&base[..start_idx]);
    out.push_str(replace_text);
    out.push_str(&base[end_idx..]);

    if matches!(target, RuntimeValue::String(_)) || matches!(value, RuntimeValue::String(_)) {
        return Ok(RuntimeValue::String(BStr::from(out)));
    }
    if let Ok(parsed) = out.parse::<i32>() {
        return Ok(RuntimeValue::I32(parsed));
    }
    Ok(RuntimeValue::String(BStr::from(out)))
}

fn parse_month_token(token: &str) -> Option<i32> {
    match token
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase()
        .as_str()
    {
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

fn parse_string_date_to_packed(text: &str) -> Option<i32> {
    let normalized = text
        .trim()
        .replace(',', " ")
        .replace('.', " ")
        .replace('-', " ")
        .replace('/', " ");
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    match parts.as_slice() {
        [year, month, day] if year.len() == 4 => {
            let year = year.parse::<i32>().ok()?;
            let month = parse_month_token(month).or_else(|| month.parse::<i32>().ok())?;
            let day = day.parse::<i32>().ok()?;
            Some(year.saturating_mul(10_000) + month.saturating_mul(100) + day)
        }
        [month, day, year] if parse_month_token(month).is_some() => {
            let month = parse_month_token(month)?;
            let day = day.parse::<i32>().ok()?;
            let year = year.parse::<i32>().ok()?;
            Some(year.saturating_mul(10_000) + month.saturating_mul(100) + day)
        }
        [day, month, year] => {
            let day = day.parse::<i32>().ok()?;
            let month = parse_month_token(month).or_else(|| month.parse::<i32>().ok())?;
            let year = year.parse::<i32>().ok()?;
            Some(year.saturating_mul(10_000) + month.saturating_mul(100) + day)
        }
        _ => None,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn packed_date_components(packed: i32) -> Option<(i32, u32, u32)> {
    let year = packed / 10_000;
    let month = ((packed / 100) % 100) as u32;
    let day = (packed % 100) as u32;
    if !(1..=12).contains(&month) {
        return None;
    }
    let max_day = days_in_month(year, month);
    if max_day == 0 || day == 0 || day > max_day {
        return None;
    }
    Some((year, month, day))
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

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let days = days_since_unix_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = (year_of_era + era * 400) as i32;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_piece = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_piece + 2) / 5 + 1) as u32;
    let month = (month_piece + if month_piece < 10 { 3 } else { -9 }) as u32;
    year += i32::from(month <= 2);
    (year, month, day)
}

pub fn packed_date_to_ole_serial(packed: i32) -> Result<f64, String> {
    let Some((year, month, day)) = packed_date_components(packed) else {
        return Err(format!("invalid packed date digits: {packed}"));
    };
    let serial = days_from_civil(year, month, day) + 25_569;
    validate_date_range(serial as f64)
}

fn maybe_packed_date_to_ole_serial(value: i32) -> Option<f64> {
    packed_date_to_ole_serial(value).ok()
}

pub fn format_date_serial_digits(serial: f64) -> Result<String, String> {
    let serial = validate_date_range(serial)?;
    let whole_days = serial.floor() as i64;
    let (year, month, day) = civil_from_days(whole_days - 25_569);
    Ok(format!("{year:04}{month:02}{day:02}"))
}

pub fn runtime_value_to_date_value_digits(value: &RuntimeValue) -> Result<i32, String> {
    match value {
        RuntimeValue::String(text) => parse_string_date_to_packed(text.as_str())
            .ok_or_else(|| format!("DateValue string format is not yet supported: `{text}`")),
        other => runtime_value_to_i32_compat(other, "DateValue src"),
    }
}

pub fn runtime_value_to_cdate(value: &RuntimeValue) -> Result<RuntimeValue, String> {
    let serial = match value {
        RuntimeValue::String(text) => {
            let packed = parse_string_date_to_packed(text.as_str())
                .ok_or_else(|| format!("CDate string format is not yet supported: `{text}`"))?;
            packed_date_to_ole_serial(packed)?
        }
        RuntimeValue::I32(value) => match maybe_packed_date_to_ole_serial(*value) {
            Some(serial) => serial,
            None => validate_date_range(*value as f64)?,
        },
        RuntimeValue::I64(value) => {
            if let Ok(raw) = i32::try_from(*value) {
                match maybe_packed_date_to_ole_serial(raw) {
                    Some(serial) => serial,
                    None => validate_date_range(*value as f64)?,
                }
            } else {
                validate_date_range(*value as f64)?
            }
        }
        RuntimeValue::F64(value) => validate_date_range(value.as_f64())?,
        other => {
            let legacy = runtime_value_to_i32_compat(other, "CDate src")?;
            match maybe_packed_date_to_ole_serial(legacy) {
                Some(serial) => serial,
                None => validate_date_range(legacy as f64)?,
            }
        }
    };
    Ok(RuntimeValue::F64(F64Value::from_date_f64(serial)))
}

pub fn runtime_value_to_datevalue(value: &RuntimeValue) -> Result<RuntimeValue, String> {
    let RuntimeValue::F64(date_value) = runtime_value_to_cdate(value)? else {
        unreachable!("CDate conversion must return an F64 date subtype");
    };
    Ok(RuntimeValue::F64(F64Value::from_date_f64(
        date_value.as_f64().floor(),
    )))
}

fn date_serial_from_value(value: &RuntimeValue) -> Result<f64, String> {
    let RuntimeValue::F64(date_value) = runtime_value_to_cdate(value)? else {
        unreachable!("CDate conversion must return an F64 date subtype");
    };
    validate_date_range(date_value.as_f64())
}

fn date_components_from_serial(serial: f64) -> Result<(i32, u32, u32), String> {
    let serial = validate_date_range(serial)?;
    Ok(civil_from_days(serial.floor() as i64 - 25_569))
}

fn checked_packed_date(year: i32, month: i32, day: i32) -> Result<i32, String> {
    year.checked_mul(10_000)
        .and_then(|value| value.checked_add(month.checked_mul(100)?))
        .and_then(|value| value.checked_add(day))
        .ok_or_else(|| {
            format!("date components overflowed packed-date conversion: {year}-{month}-{day}")
        })
}

fn day_of_week(year: i32, month: u32, day: u32) -> i32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let month_i32 = month as i32;
    let day_i32 = day as i32;
    let y = if month_i32 < 3 { year - 1 } else { year };
    (y + y / 4 - y / 100 + y / 400 + t[(month_i32 - 1) as usize] + day_i32) % 7
}

pub fn runtime_date_serial_value(year: i32, month: i32, day: i32) -> Result<RuntimeValue, String> {
    let packed = checked_packed_date(year, month, day)?;
    let serial = packed_date_to_ole_serial(packed)?;
    Ok(RuntimeValue::F64(F64Value::from_date_f64(serial)))
}

pub fn runtime_time_serial_value(
    hour: i32,
    minute: i32,
    second: i32,
) -> Result<RuntimeValue, String> {
    let total_seconds = i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second);
    let fraction = total_seconds as f64 / SECONDS_PER_DAY;
    Ok(RuntimeValue::F64(F64Value::from_date_f64(fraction)))
}

pub fn runtime_value_to_timevalue(value: &RuntimeValue) -> Result<RuntimeValue, String> {
    if let RuntimeValue::I32(raw) = value
        && maybe_packed_date_to_ole_serial(*raw).is_none()
    {
        let seconds = f64::from(*raw).rem_euclid(SECONDS_PER_DAY);
        return Ok(RuntimeValue::F64(F64Value::from_date_f64(
            seconds / SECONDS_PER_DAY,
        )));
    }
    if let RuntimeValue::I64(raw) = value
        && let Ok(narrow) = i32::try_from(*raw)
        && maybe_packed_date_to_ole_serial(narrow).is_none()
    {
        let seconds = (*raw as f64).rem_euclid(SECONDS_PER_DAY);
        return Ok(RuntimeValue::F64(F64Value::from_date_f64(
            seconds / SECONDS_PER_DAY,
        )));
    }
    let RuntimeValue::F64(date_value) = runtime_value_to_cdate(value)? else {
        unreachable!("CDate conversion must return an F64 date subtype");
    };
    let fraction = date_value.as_f64().rem_euclid(1.0);
    Ok(RuntimeValue::F64(F64Value::from_date_f64(fraction)))
}

pub fn runtime_host_now_value(
    date: &RuntimeValue,
    time: &RuntimeValue,
) -> Result<RuntimeValue, String> {
    let date_serial = date_serial_from_value(date)?.floor();
    let RuntimeValue::F64(time_value) = runtime_value_to_timevalue(time)? else {
        unreachable!("TimeValue conversion must return an F64 date subtype");
    };
    let serial = validate_date_range(date_serial + time_value.as_f64())?;
    Ok(RuntimeValue::F64(F64Value::from_date_f64(serial)))
}

pub fn runtime_date_add_value(
    _interval: i32,
    number: i32,
    date: &RuntimeValue,
) -> Result<RuntimeValue, String> {
    let serial = date_serial_from_value(date)?;
    let out = validate_date_range(serial + f64::from(number))?;
    Ok(RuntimeValue::F64(F64Value::from_date_f64(out)))
}

pub fn runtime_date_diff_value(
    _interval: i32,
    date1: &RuntimeValue,
    date2: &RuntimeValue,
) -> Result<i32, String> {
    let serial1 = date_serial_from_value(date1)?.floor() as i64;
    let serial2 = date_serial_from_value(date2)?.floor() as i64;
    i32::try_from(serial2 - serial1)
        .map_err(|_| format!("DateDiff result overflowed i32 span: {serial1}..{serial2}"))
}

pub fn runtime_date_year(value: &RuntimeValue) -> Result<i32, String> {
    let (year, _, _) = date_components_from_serial(date_serial_from_value(value)?)?;
    Ok(year)
}

pub fn runtime_date_month(value: &RuntimeValue) -> Result<i32, String> {
    let (_, month, _) = date_components_from_serial(date_serial_from_value(value)?)?;
    Ok(month as i32)
}

pub fn runtime_date_day(value: &RuntimeValue) -> Result<i32, String> {
    let (_, _, day) = date_components_from_serial(date_serial_from_value(value)?)?;
    Ok(day as i32)
}

pub fn runtime_date_weekday(value: &RuntimeValue) -> Result<i32, String> {
    let (year, month, day) = date_components_from_serial(date_serial_from_value(value)?)?;
    Ok(day_of_week(year, month, day) + 1)
}

pub fn runtime_value_is_date(value: &RuntimeValue) -> bool {
    match value {
        RuntimeValue::String(_) => runtime_value_to_cdate(value).is_ok(),
        RuntimeValue::I32(_)
        | RuntimeValue::I64(_)
        | RuntimeValue::F64(_)
        | RuntimeValue::Currency(_)
        | RuntimeValue::Decimal(_) => runtime_value_to_cdate(value).is_ok(),
        _ => false,
    }
}

pub fn runtime_value_to_usize(value: &RuntimeValue) -> Result<usize, String> {
    let index = runtime_value_to_i32_compat(value, "usize operand")?;
    if index < 0 {
        return Err(format!("usize operand cannot be negative: {index}"));
    }
    usize::try_from(index).map_err(|_| format!("usize operand exceeds usize range: {index}"))
}

pub fn legacy_truthy_value(value: &RuntimeValue) -> Result<bool, String> {
    if matches!(value, RuntimeValue::Null) {
        return Ok(false);
    }
    Ok(runtime_value_to_numeric_compat(value, "boolean operand")? != 0.0)
}

pub fn runtime_value_is_object(value: &RuntimeValue) -> bool {
    matches!(
        value,
        RuntimeValue::Object(_) | RuntimeValue::ObjectHandle(_) | RuntimeValue::BindingHandle(_)
    )
}

pub fn runtime_value_is_array_compat(value: &RuntimeValue) -> bool {
    match value {
        RuntimeValue::ArrayIntent(_) => true,
        RuntimeValue::I32(v) => oxvba_runtime::safe_array::is_array_tag(*v),
        _ => false,
    }
}

pub fn runtime_vartype_tag_bounded(value: &RuntimeValue) -> i32 {
    match value {
        RuntimeValue::Empty => 0,
        RuntimeValue::Null => 1,
        RuntimeValue::ErrorCode(_) => 10,
        RuntimeValue::ArrayIntent(_) => 8192 + 12,
        RuntimeValue::I32(v) if runtime_value_is_array_compat(value) => {
            let _ = v;
            8192 + 12
        }
        RuntimeValue::I32(v) if *v == oxvba_runtime::value_tags::EMPTY_TAG => 0,
        RuntimeValue::I32(v) if *v == oxvba_runtime::value_tags::NULL_TAG => 1,
        RuntimeValue::I32(v) if oxvba_runtime::value_tags::is_error_tag(*v) => 10,
        _ => 3,
    }
}

pub fn runtime_typename_tag_bounded(value: &RuntimeValue) -> i32 {
    if runtime_value_is_array_compat(value) {
        1001
    } else {
        1002
    }
}

pub fn runtime_is_numeric_tag_bounded(value: &RuntimeValue) -> i32 {
    match value {
        RuntimeValue::Empty
        | RuntimeValue::Null
        | RuntimeValue::ErrorCode(_)
        | RuntimeValue::ArrayIntent(_) => 0,
        RuntimeValue::I32(v)
            if runtime_value_is_array_compat(value)
                || *v == oxvba_runtime::value_tags::EMPTY_TAG
                || *v == oxvba_runtime::value_tags::NULL_TAG
                || oxvba_runtime::value_tags::is_error_tag(*v) =>
        {
            0
        }
        _ => 1,
    }
}

pub fn runtime_random_seed_bounded(value: &RuntimeValue, field: &str) -> Result<i32, String> {
    runtime_value_to_i32_compat(value, field)
}

// ── Arithmetic ────────────────────────────────────────────────────────

pub fn legacy_add_const_value(
    value: &RuntimeValue,
    delta: i32,
    field: &str,
) -> Result<RuntimeValue, String> {
    if matches!(value, RuntimeValue::Null) {
        return Ok(RuntimeValue::Null);
    }
    if matches!(value, RuntimeValue::ErrorCode(_)) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    if let RuntimeValue::F64(v) = value {
        return Ok(RuntimeValue::F64(F64Value::from_f64(
            v.as_f64() + delta as f64,
        )));
    }
    let value = runtime_value_to_i32_compat(value, field)?;
    Ok(RuntimeValue::I32(value + delta))
}

pub fn legacy_add_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> Result<RuntimeValue, String> {
    if either_null(lhs, rhs) {
        return Ok(RuntimeValue::Null);
    }
    if either_error(lhs, rhs) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    if matches!(lhs, RuntimeValue::String(_)) && matches!(rhs, RuntimeValue::String(_)) {
        return Ok(legacy_concat_values(lhs, rhs));
    }
    if either_is_f64(lhs, rhs) {
        let l = runtime_value_as_f64(lhs)?;
        let r = runtime_value_as_f64(rhs)?;
        return Ok(RuntimeValue::F64(F64Value::from_f64(l + r)));
    }
    let lhs = runtime_value_to_i32_compat(lhs, "add lhs")?;
    let rhs = runtime_value_to_i32_compat(rhs, "add rhs")?;
    Ok(RuntimeValue::I32(lhs + rhs))
}

pub fn legacy_sub_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> Result<RuntimeValue, String> {
    if either_null(lhs, rhs) {
        return Ok(RuntimeValue::Null);
    }
    if either_error(lhs, rhs) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    if either_is_f64(lhs, rhs) {
        let l = runtime_value_as_f64(lhs)?;
        let r = runtime_value_as_f64(rhs)?;
        return Ok(RuntimeValue::F64(F64Value::from_f64(l - r)));
    }
    let lhs = runtime_value_to_i32_compat(lhs, "sub lhs")?;
    let rhs = runtime_value_to_i32_compat(rhs, "sub rhs")?;
    Ok(RuntimeValue::I32(lhs - rhs))
}

pub fn legacy_mul_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> Result<RuntimeValue, String> {
    if either_null(lhs, rhs) {
        return Ok(RuntimeValue::Null);
    }
    if either_error(lhs, rhs) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    if either_is_f64(lhs, rhs) {
        let l = runtime_value_as_f64(lhs)?;
        let r = runtime_value_as_f64(rhs)?;
        return Ok(RuntimeValue::F64(F64Value::from_f64(l * r)));
    }
    let lhs = runtime_value_to_i32_compat(lhs, "mul lhs")?;
    let rhs = runtime_value_to_i32_compat(rhs, "mul rhs")?;
    let result = (lhs as i64) * (rhs as i64);
    let truncated = result as i32;
    Ok(RuntimeValue::I32(truncated))
}

pub fn legacy_pow_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> Result<RuntimeValue, String> {
    if either_null(lhs, rhs) {
        return Ok(RuntimeValue::Null);
    }
    if either_error(lhs, rhs) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    let base = runtime_value_to_numeric_compat(lhs, "pow base")?;
    let exp = runtime_value_to_numeric_compat(rhs, "pow exponent")?;
    let result = base.powf(exp);
    Ok(RuntimeValue::F64(F64Value::from_f64(result)))
}

pub fn legacy_concat_values(lhs: &RuntimeValue, rhs: &RuntimeValue) -> RuntimeValue {
    let lhs_str = if matches!(lhs, RuntimeValue::Null) {
        String::new()
    } else {
        runtime_value_to_text(lhs, "concat lhs").unwrap_or_default()
    };
    let rhs_str = if matches!(rhs, RuntimeValue::Null) {
        String::new()
    } else {
        runtime_value_to_text(rhs, "concat rhs").unwrap_or_default()
    };
    RuntimeValue::String(BStr::from(format!("{lhs_str}{rhs_str}")))
}

pub fn legacy_neg_value(val: &RuntimeValue) -> Result<RuntimeValue, String> {
    if matches!(val, RuntimeValue::Null) {
        return Ok(RuntimeValue::Null);
    }
    if let RuntimeValue::F64(v) = val {
        return Ok(RuntimeValue::F64(F64Value::from_f64(-v.as_f64())));
    }
    let v = runtime_value_to_i32_compat(val, "neg operand")?;
    Ok(RuntimeValue::I32(-v))
}

pub fn legacy_increment_value(value: &RuntimeValue) -> Result<RuntimeValue, String> {
    if let RuntimeValue::F64(v) = value {
        return Ok(RuntimeValue::F64(F64Value::from_f64(v.as_f64() + 1.0)));
    }
    let value = runtime_value_to_i32_compat(value, "increment operand")?;
    Ok(RuntimeValue::I32(value + 1))
}

// ── Division (with error codes) ───────────────────────────────────────

/// Returns Ok(value) or Err(error_code) for division by zero (code 11).
pub fn legacy_div_values(
    lhs: &RuntimeValue,
    rhs: &RuntimeValue,
) -> Result<Result<RuntimeValue, i32>, String> {
    if either_null(lhs, rhs) {
        return Ok(Ok(RuntimeValue::Null));
    }
    let r = runtime_value_to_numeric_compat(rhs, "div rhs")?;
    if r == 0.0 {
        return Ok(Err(11));
    }
    let l = runtime_value_to_numeric_compat(lhs, "div lhs")?;
    Ok(Ok(RuntimeValue::F64(F64Value::from_f64(l / r))))
}

pub fn legacy_intdiv_values(
    lhs: &RuntimeValue,
    rhs: &RuntimeValue,
) -> Result<Result<RuntimeValue, i32>, String> {
    if either_null(lhs, rhs) {
        return Ok(Ok(RuntimeValue::Null));
    }
    let r = runtime_value_to_numeric_compat(rhs, "intdiv rhs")?;
    let r_trunc = r as i32;
    if r_trunc == 0 {
        return Ok(Err(11));
    }
    let l = runtime_value_to_numeric_compat(lhs, "intdiv lhs")?;
    Ok(Ok(RuntimeValue::I32((l / r).trunc() as i32)))
}

pub fn legacy_mod_values(
    lhs: &RuntimeValue,
    rhs: &RuntimeValue,
) -> Result<Result<RuntimeValue, i32>, String> {
    if either_null(lhs, rhs) {
        return Ok(Ok(RuntimeValue::Null));
    }
    let r = runtime_value_to_numeric_compat(rhs, "mod rhs")?;
    let r_int = r as i32;
    if r_int == 0 {
        return Ok(Err(11));
    }
    let l = runtime_value_to_numeric_compat(lhs, "mod lhs")?;
    Ok(Ok(RuntimeValue::I32((l as i32) % r_int)))
}

// ── Comparison ────────────────────────────────────────────────────────

pub fn normalize_for_compare(text: String, mode: StringCompareMode) -> String {
    match mode {
        StringCompareMode::Binary => text,
        StringCompareMode::Text => text.to_ascii_lowercase(),
    }
}

pub fn typed_compare_values(
    lhs: &RuntimeValue,
    rhs: &RuntimeValue,
    mode: StringCompareMode,
    pred: fn(std::cmp::Ordering) -> bool,
) -> Result<bool, String> {
    if either_null(lhs, rhs) {
        return Ok(false);
    }
    match (lhs, rhs) {
        (RuntimeValue::String(a), RuntimeValue::String(b)) => {
            let a = normalize_for_compare(a.as_str().to_string(), mode);
            let b = normalize_for_compare(b.as_str().to_string(), mode);
            Ok(pred(a.cmp(&b)))
        }
        (RuntimeValue::String(a), RuntimeValue::Empty) => {
            let a = normalize_for_compare(a.as_str().to_string(), mode);
            Ok(pred(a.cmp(&String::new())))
        }
        (RuntimeValue::Empty, RuntimeValue::String(b)) => {
            let b = normalize_for_compare(b.as_str().to_string(), mode);
            Ok(pred(String::new().cmp(&b)))
        }
        (RuntimeValue::F64(a), RuntimeValue::F64(b)) => {
            let ord = a
                .as_f64()
                .partial_cmp(&b.as_f64())
                .unwrap_or(std::cmp::Ordering::Equal);
            Ok(pred(ord))
        }
        (RuntimeValue::I32(a), RuntimeValue::F64(b)) => {
            let ord = (*a as f64)
                .partial_cmp(&b.as_f64())
                .unwrap_or(std::cmp::Ordering::Equal);
            Ok(pred(ord))
        }
        (RuntimeValue::I64(a), RuntimeValue::I64(b)) => Ok(pred(a.cmp(b))),
        (RuntimeValue::I64(a), RuntimeValue::I32(b)) => Ok(pred(a.cmp(&(*b as i64)))),
        (RuntimeValue::I32(a), RuntimeValue::I64(b)) => Ok(pred((*a as i64).cmp(b))),
        (RuntimeValue::F64(a), RuntimeValue::I32(b)) => {
            let ord = a
                .as_f64()
                .partial_cmp(&(*b as f64))
                .unwrap_or(std::cmp::Ordering::Equal);
            Ok(pred(ord))
        }
        (RuntimeValue::I64(a), RuntimeValue::F64(b)) => {
            let ord = (*a as f64)
                .partial_cmp(&b.as_f64())
                .unwrap_or(std::cmp::Ordering::Equal);
            Ok(pred(ord))
        }
        (RuntimeValue::F64(a), RuntimeValue::I64(b)) => {
            let ord = a
                .as_f64()
                .partial_cmp(&(*b as f64))
                .unwrap_or(std::cmp::Ordering::Equal);
            Ok(pred(ord))
        }
        _ => {
            if let (Ok(l), Ok(r)) = (
                runtime_value_to_numeric_compat(lhs, "comparison lhs"),
                runtime_value_to_numeric_compat(rhs, "comparison rhs"),
            ) {
                let ord = l.partial_cmp(&r).unwrap_or(std::cmp::Ordering::Equal);
                return Ok(pred(ord));
            }
            Err("comparison operands are not compatible for numeric comparison".to_string())
        }
    }
}

// ── Assignment Validation ─────────────────────────────────────────────

pub fn runtime_assignment_value_label(value: &RuntimeValue) -> &'static str {
    match value {
        RuntimeValue::Empty => "Empty",
        RuntimeValue::Null => "Null",
        RuntimeValue::ErrorCode(_) => "Error",
        RuntimeValue::I32(_) => "Long",
        RuntimeValue::I64(_) => "LongLong",
        RuntimeValue::F64(value) => match value.subtype() {
            oxvba_runtime::F64Subtype::Single => "Single",
            oxvba_runtime::F64Subtype::Double => "Double",
            oxvba_runtime::F64Subtype::Date => "Date",
        },
        RuntimeValue::Decimal(_) => "Decimal",
        RuntimeValue::Currency(_) => "Currency",
        RuntimeValue::Bool(_) => "Boolean",
        RuntimeValue::String(_) => "String",
        RuntimeValue::ArrayIntent(_) => "Array",
        RuntimeValue::Object(_) | RuntimeValue::ObjectHandle(_) => "Object",
        RuntimeValue::BindingHandle(_) => "Binding",
    }
}

pub fn validate_runtime_assignment(
    value: &RuntimeValue,
    intent: RuntimeAssignmentIntent,
    target_kind: RuntimeAssignmentTargetKind,
    target_name: &str,
    target_type_name: &str,
) -> Result<(), String> {
    match (intent, target_kind) {
        (RuntimeAssignmentIntent::Set, RuntimeAssignmentTargetKind::Variant)
        | (RuntimeAssignmentIntent::Set, RuntimeAssignmentTargetKind::Object) => {
            if runtime_value_is_object(value) {
                Ok(())
            } else {
                Err(format!(
                    "Set requires object value for variable {target_name}"
                ))
            }
        }
        (RuntimeAssignmentIntent::Implicit, RuntimeAssignmentTargetKind::Object) => {
            if runtime_value_is_object(value) {
                Err(format!("Set required for Object variable {target_name}"))
            } else {
                Err(format!(
                    "cannot assign {} to Object variable {target_name}",
                    runtime_assignment_value_label(value)
                ))
            }
        }
        (RuntimeAssignmentIntent::Let, RuntimeAssignmentTargetKind::Object) => Err(format!(
            "Let cannot assign to Object variable {target_name}"
        )),
        (
            RuntimeAssignmentIntent::Implicit | RuntimeAssignmentIntent::Let,
            RuntimeAssignmentTargetKind::Scalar,
        ) => {
            if runtime_value_is_object(value) {
                Err(format!(
                    "cannot assign Object to {target_type_name} variable {target_name}"
                ))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SECONDS_PER_DAY, format_date_serial_digits, runtime_value_is_date, runtime_value_to_cdate,
        runtime_value_to_date_value_digits, runtime_value_to_datevalue, runtime_value_to_text,
        runtime_value_to_timevalue, typed_compare_values,
    };
    use oxvba_compiler::bytecode::StringCompareMode;
    use oxvba_runtime::{F64Value, RuntimeValue, bstr::BStr};

    #[test]
    fn string_equals_empty_coerces_without_legacy_token_failure() {
        let out = typed_compare_values(
            &RuntimeValue::String(BStr(String::new())),
            &RuntimeValue::Empty,
            StringCompareMode::Binary,
            |ord| ord == std::cmp::Ordering::Equal,
        )
        .expect("string-empty comparison should coerce cleanly");
        assert!(out);
    }

    #[test]
    fn nonempty_string_not_equal_to_empty_after_string_coercion() {
        let out = typed_compare_values(
            &RuntimeValue::String(BStr("main".to_string())),
            &RuntimeValue::Empty,
            StringCompareMode::Binary,
            |ord| ord == std::cmp::Ordering::Equal,
        )
        .expect("string-empty comparison should coerce cleanly");
        assert!(!out);
    }

    #[test]
    fn datevalue_parses_day_month_name_year_string() {
        let out = runtime_value_to_date_value_digits(&RuntimeValue::String(BStr(
            "1 Jan 2000".to_string(),
        )))
        .expect("DateValue string should parse");
        assert_eq!(out, 20000101);
    }

    #[test]
    fn datevalue_parses_month_name_day_year_string() {
        let out = runtime_value_to_date_value_digits(&RuntimeValue::String(BStr(
            "January 1, 2000".to_string(),
        )))
        .expect("DateValue month-first string should parse");
        assert_eq!(out, 20000101);
    }

    #[test]
    fn datevalue_preserves_existing_numeric_passthrough_lane() {
        let out = runtime_value_to_date_value_digits(&RuntimeValue::I32(20260228))
            .expect("numeric DateValue passthrough should still work");
        assert_eq!(out, 20260228);
    }

    #[test]
    fn datevalue_runtime_promotes_string_input_to_date_subtype() {
        let out = runtime_value_to_datevalue(&RuntimeValue::String(BStr("1 Jan 2000".to_string())))
            .expect("DateValue runtime conversion should succeed");
        assert_eq!(out, RuntimeValue::F64(F64Value::from_date_f64(36526.0)));
    }

    #[test]
    fn cdate_runtime_accepts_packed_digits_for_compatibility() {
        let out = runtime_value_to_cdate(&RuntimeValue::I32(20260228))
            .expect("CDate packed-digit compatibility conversion should succeed");
        assert_eq!(out, RuntimeValue::F64(F64Value::from_date_f64(46081.0)));
    }

    #[test]
    fn cdate_runtime_accepts_dotted_month_name_string() {
        let out = runtime_value_to_cdate(&RuntimeValue::String(BStr("Jan. 1, 2000".to_string())))
            .expect("CDate dotted month-name string should parse");
        assert_eq!(out, RuntimeValue::F64(F64Value::from_date_f64(36526.0)));
    }

    #[test]
    fn timevalue_runtime_promotes_seconds_carrier_to_date_subtype() {
        let out = runtime_value_to_timevalue(&RuntimeValue::I32(3723))
            .expect("TimeValue integer-seconds compatibility conversion should succeed");
        assert_eq!(
            out,
            RuntimeValue::F64(F64Value::from_date_f64(3723.0 / SECONDS_PER_DAY))
        );
    }

    #[test]
    fn is_date_accepts_real_date_subtype_and_rejects_plain_string() {
        assert!(runtime_value_is_date(&RuntimeValue::F64(
            F64Value::from_date_f64(46081.5)
        )));
        assert!(!runtime_value_is_date(&RuntimeValue::String(BStr(
            "not a date".to_string()
        ))));
    }

    #[test]
    fn date_subtype_formats_to_stable_digits() {
        let out = format_date_serial_digits(40348.0).expect("date formatting should succeed");
        assert_eq!(out, "20100619");
    }

    #[test]
    fn runtime_value_to_text_coerces_common_typed_values_without_legacy_projection() {
        assert_eq!(
            runtime_value_to_text(&RuntimeValue::I32(42), "text operand")
                .expect("i32 string coercion should succeed"),
            "42"
        );
        assert_eq!(
            runtime_value_to_text(
                &RuntimeValue::F64(F64Value::from_date_f64(40348.0)),
                "text operand"
            )
            .expect("date subtype string coercion should succeed"),
            "20100619"
        );
    }

    #[test]
    fn split_count_bounded_uses_typed_text_coercion() {
        let out = super::runtime_split_count_bounded(
            &RuntimeValue::String(BStr("123231".to_string())),
            &RuntimeValue::I32(23),
        )
        .expect("Split should succeed");
        assert_eq!(out, RuntimeValue::I32(3));
    }

    #[test]
    fn join_bounded_keeps_array_count_and_scalar_passthrough_shape() {
        let array =
            RuntimeValue::ArrayIntent(oxvba_runtime::safe_array::SafeArray::from_values(vec![
                RuntimeValue::I32(1),
                RuntimeValue::I32(2),
                RuntimeValue::I32(3),
            ]));
        assert_eq!(
            super::runtime_join_bounded(&array, &RuntimeValue::I32(0))
                .expect("Join over array should succeed"),
            RuntimeValue::I32(3)
        );
        assert_eq!(
            super::runtime_join_bounded(&RuntimeValue::I32(789), &RuntimeValue::I32(0))
                .expect("Join over scalar should preserve bounded passthrough"),
            RuntimeValue::I32(789)
        );
    }

    #[test]
    fn runtime_value_to_usize_and_array_indices_use_numeric_compatibility() {
        assert_eq!(
            super::runtime_value_to_usize(&RuntimeValue::String(BStr("12".to_string())))
                .expect("numeric text usize coercion should succeed"),
            12
        );
        assert_eq!(
            super::runtime_array_indices(
                &[
                    RuntimeValue::String(BStr("1".to_string())),
                    RuntimeValue::I32(2),
                ],
                "array index"
            )
            .expect("numeric-compatible array indices should succeed"),
            vec![1, 2]
        );
    }

    #[test]
    fn array_bounds_accept_explicit_array_tag_carriers_without_generic_token_projection() {
        let tag = oxvba_runtime::safe_array::array_tag_from_safe_array(
            &oxvba_runtime::safe_array::SafeArray::from_values(vec![
                RuntimeValue::I32(1),
                RuntimeValue::I32(2),
                RuntimeValue::I32(3),
                RuntimeValue::I32(4),
            ]),
        )
        .expect("array tag");
        assert_eq!(
            super::runtime_array_lbound(&RuntimeValue::I32(tag), "LBound operand")
                .expect("array-tag lbound should succeed"),
            0
        );
        assert_eq!(
            super::runtime_array_ubound(&RuntimeValue::I32(tag), "UBound operand")
                .expect("array-tag ubound should succeed"),
            3
        );
    }

    #[test]
    fn like_bounded_uses_typed_text_coercion() {
        assert_eq!(
            super::runtime_like_bounded(
                &RuntimeValue::String(BStr("ABC".to_string())),
                &RuntimeValue::String(BStr("abc".to_string())),
                StringCompareMode::Text,
            )
            .expect("Like should succeed"),
            RuntimeValue::I32(-1)
        );
        assert_eq!(
            super::runtime_like_bounded(
                &RuntimeValue::I32(123),
                &RuntimeValue::String(BStr("456".to_string())),
                StringCompareMode::Binary,
            )
            .expect("Like should succeed"),
            RuntimeValue::I32(0)
        );
    }

    #[test]
    fn mid_stmt_bounded_preserves_numeric_subset_without_legacy_projection() {
        assert_eq!(
            super::runtime_mid_stmt_bounded(
                &RuntimeValue::I32(12345),
                &RuntimeValue::I32(2),
                Some(&RuntimeValue::I32(2)),
                &RuntimeValue::I32(99),
            )
            .expect("MidStmt should succeed"),
            RuntimeValue::I32(19945)
        );
    }

    #[test]
    fn mid_stmt_bounded_supports_string_target_mutation() {
        assert_eq!(
            super::runtime_mid_stmt_bounded(
                &RuntimeValue::String(BStr("ABCDE".to_string())),
                &RuntimeValue::I32(2),
                Some(&RuntimeValue::I32(2)),
                &RuntimeValue::String(BStr("99".to_string())),
            )
            .expect("MidStmt string mutation should succeed"),
            RuntimeValue::String(BStr("A99DE".to_string()))
        );
    }

    #[test]
    fn numeric_compat_accepts_numeric_text_for_truthy_and_division_lanes() {
        assert!(
            super::legacy_truthy_value(&RuntimeValue::String(BStr("12".to_string())))
                .expect("truthy coercion should succeed")
        );
        assert_eq!(
            super::legacy_div_values(
                &RuntimeValue::String(BStr("12".to_string())),
                &RuntimeValue::I32(3),
            )
            .expect("division coercion should succeed"),
            Ok(RuntimeValue::F64(F64Value::from_f64(4.0)))
        );
    }

    #[test]
    fn char_and_format_helpers_use_typed_text_numeric_coercion() {
        assert_eq!(
            super::runtime_chr_bounded(&RuntimeValue::String(BStr("65".to_string())))
                .expect("Chr should coerce numeric text"),
            RuntimeValue::String(BStr("A".to_string()))
        );
        assert_eq!(
            super::runtime_asc_bounded(&RuntimeValue::I32(123))
                .expect("Asc should coerce through typed text"),
            RuntimeValue::I32('1' as i32)
        );
        assert_eq!(
            super::runtime_space_bounded(&RuntimeValue::String(BStr("3".to_string())))
                .expect("Space should coerce numeric text"),
            RuntimeValue::String(BStr("   ".to_string()))
        );
        assert_eq!(
            super::runtime_string_repeat_bounded(
                &RuntimeValue::I32(3),
                &RuntimeValue::String(BStr("Z".to_string())),
            )
            .expect("String$ should succeed"),
            RuntimeValue::String(BStr("ZZZ".to_string()))
        );
        assert_eq!(
            super::runtime_hex_bounded(&RuntimeValue::String(BStr("255".to_string())))
                .expect("Hex should coerce numeric text"),
            RuntimeValue::String(BStr("FF".to_string()))
        );
        assert_eq!(
            super::runtime_oct_bounded(&RuntimeValue::I32(8)).expect("Oct should succeed"),
            RuntimeValue::String(BStr("10".to_string()))
        );
        assert_eq!(
            super::runtime_strconv_bounded(
                &RuntimeValue::String(BStr("ab".to_string())),
                &RuntimeValue::String(BStr("1".to_string())),
            )
            .expect("StrConv should coerce numeric text conversion"),
            RuntimeValue::String(BStr("AB".to_string()))
        );
        assert_eq!(
            super::runtime_val_bounded(&RuntimeValue::Bool(true)).expect("Val should coerce bool"),
            RuntimeValue::I32(-1)
        );
    }

    #[test]
    fn math_and_date_helpers_use_typed_numeric_coercion() {
        assert_eq!(
            super::runtime_abs_bounded(&RuntimeValue::String(BStr("-7".to_string())))
                .expect("Abs should coerce numeric text"),
            RuntimeValue::I32(7)
        );
        assert_eq!(
            super::runtime_round_bounded(
                &RuntimeValue::String(BStr("19".to_string())),
                Some(&RuntimeValue::String(BStr("-1".to_string()))),
            )
            .expect("Round should coerce numeric text"),
            RuntimeValue::I32(20)
        );
        assert_eq!(
            super::runtime_month_name_bounded(&RuntimeValue::String(BStr("3".to_string())))
                .expect("MonthName should coerce numeric text"),
            RuntimeValue::String(BStr("March".to_string()))
        );
        assert_eq!(
            super::runtime_date_serial_bounded(
                &RuntimeValue::String(BStr("2026".to_string())),
                &RuntimeValue::String(BStr("2".to_string())),
                &RuntimeValue::String(BStr("28".to_string())),
            )
            .expect("DateSerial should coerce numeric text"),
            RuntimeValue::F64(F64Value::from_date_f64(46081.0))
        );
        assert_eq!(
            super::runtime_date_add_bounded(
                &RuntimeValue::String(BStr("1".to_string())),
                &RuntimeValue::String(BStr("3".to_string())),
                &RuntimeValue::F64(F64Value::from_date_f64(46081.0)),
            )
            .expect("DateAdd should coerce numeric text"),
            RuntimeValue::F64(F64Value::from_date_f64(46084.0))
        );
    }

    #[test]
    fn tag_and_seed_helpers_use_runtime_shape_and_typed_numeric_coercion() {
        let array =
            RuntimeValue::ArrayIntent(oxvba_runtime::safe_array::SafeArray::from_values(vec![
                RuntimeValue::I32(1),
                RuntimeValue::I32(2),
            ]));
        assert_eq!(super::runtime_vartype_tag_bounded(&array), 8204);
        assert_eq!(super::runtime_typename_tag_bounded(&array), 1001);
        assert_eq!(
            super::runtime_is_numeric_tag_bounded(&RuntimeValue::ErrorCode(7)),
            0
        );
        assert_eq!(
            super::runtime_random_seed_bounded(
                &RuntimeValue::String(BStr("1".to_string())),
                "Randomize seed"
            )
            .expect("Randomize seed should coerce numeric text"),
            1
        );
    }

    #[test]
    fn arithmetic_and_comparison_helpers_use_typed_numeric_coercion() {
        assert_eq!(
            super::legacy_add_values(
                &RuntimeValue::String(BStr("12".to_string())),
                &RuntimeValue::I32(3)
            )
            .expect("add should coerce numeric text"),
            RuntimeValue::I32(15)
        );
        assert_eq!(
            super::legacy_sub_values(
                &RuntimeValue::String(BStr("12".to_string())),
                &RuntimeValue::I32(5)
            )
            .expect("sub should coerce numeric text"),
            RuntimeValue::I32(7)
        );
        assert_eq!(
            super::legacy_mul_values(
                &RuntimeValue::String(BStr("3".to_string())),
                &RuntimeValue::I32(4)
            )
            .expect("mul should coerce numeric text"),
            RuntimeValue::I32(12)
        );
        assert_eq!(
            super::legacy_pow_values(
                &RuntimeValue::String(BStr("2".to_string())),
                &RuntimeValue::I32(3)
            )
            .expect("pow should coerce numeric text"),
            RuntimeValue::F64(F64Value::from_f64(8.0))
        );
        assert!(
            super::typed_compare_values(
                &RuntimeValue::String(BStr("12".to_string())),
                &RuntimeValue::I32(12),
                StringCompareMode::Binary,
                |ord| ord == std::cmp::Ordering::Equal,
            )
            .expect("comparison should coerce numeric text")
        );
    }

    #[test]
    fn compatibility_carriers_use_explicit_typed_or_tagged_behavior() {
        let array = oxvba_runtime::safe_array::SafeArray::from_values(vec![
            RuntimeValue::I32(1),
            RuntimeValue::I32(2),
            RuntimeValue::I32(3),
        ]);
        let tag = oxvba_runtime::safe_array::array_tag_from_safe_array(&array)
            .expect("array tag should materialize");
        assert_eq!(
            super::runtime_join_bounded(&RuntimeValue::I32(tag), &RuntimeValue::I32(0))
                .expect("Join should project array tag cardinality"),
            RuntimeValue::I32(3)
        );
        assert_eq!(
            super::runtime_value_to_date_value_digits(&RuntimeValue::Bool(true))
                .expect("DateValue digits should use typed numeric compatibility"),
            -1
        );
        assert_eq!(
            super::runtime_value_to_cdate(&RuntimeValue::Bool(true))
                .expect("CDate should use typed numeric compatibility"),
            RuntimeValue::F64(F64Value::from_date_f64(-1.0))
        );
    }
}

// ── Formatting ────────────────────────────────────────────────────────

pub fn format_number(n: f64, fmt: Option<&str>) -> String {
    match fmt {
        None => {
            if n == (n as i64) as f64 && n.abs() < i64::MAX as f64 {
                format!("{}", n as i64)
            } else {
                format!("{}", n)
            }
        }
        Some("0") => format!("{}", n.round() as i64),
        Some(pat) if pat.starts_with("0.") && pat[2..].chars().all(|c| c == '0') => {
            let decimals = pat.len() - 2;
            format!("{:.prec$}", n, prec = decimals)
        }
        Some("0%") => format!("{}%", (n * 100.0).round() as i64),
        Some("#,##0") => {
            let i = n.round() as i64;
            let negative = i < 0;
            let abs_str = (i.unsigned_abs()).to_string();
            let mut grouped = String::new();
            for (idx, ch) in abs_str.chars().rev().enumerate() {
                if idx > 0 && idx % 3 == 0 {
                    grouped.push(',');
                }
                grouped.push(ch);
            }
            let grouped: String = grouped.chars().rev().collect();
            if negative {
                format!("-{}", grouped)
            } else {
                grouped
            }
        }
        Some(_) => {
            if n == (n as i64) as f64 && n.abs() < i64::MAX as f64 {
                format!("{}", n as i64)
            } else {
                format!("{}", n)
            }
        }
    }
}

pub fn proper_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for c in s.chars() {
        if c.is_whitespace() {
            capitalize_next = true;
            result.push(c);
        } else if capitalize_next {
            for uc in c.to_uppercase() {
                result.push(uc);
            }
            capitalize_next = false;
        } else {
            for lc in c.to_lowercase() {
                result.push(lc);
            }
        }
    }
    result
}

// ── COM Token Conversions ─────────────────────────────────────────────

pub fn runtime_value_to_com_object(
    value: &RuntimeValue,
    field: &str,
) -> Result<ObjectHandle, String> {
    match value {
        RuntimeValue::Object(handle) => Ok(ObjectHandle::new(handle.raw())),
        RuntimeValue::ObjectHandle(handle) => Ok(*handle),
        RuntimeValue::I32(raw) => Ok(ObjectHandle::new(*raw)),
        RuntimeValue::I64(raw) => i32::try_from(*raw)
            .map(ObjectHandle::new)
            .map_err(|_| format!("{field} exceeds i32 handle range: {raw}")),
        other => Err(format!(
            "{field} requires object handle-compatible carrier, got {other:?}"
        )),
    }
}

pub fn runtime_value_to_com_member_token(
    value: &RuntimeValue,
    field: &str,
) -> Result<ComMemberToken, String> {
    match value {
        RuntimeValue::I32(raw) => Ok(ComMemberToken::new(*raw)),
        RuntimeValue::I64(raw) => i32::try_from(*raw)
            .map(ComMemberToken::new)
            .map_err(|_| format!("{field} exceeds i32 member-token range: {raw}")),
        other => Err(format!(
            "{field} requires member-token-compatible carrier, got {other:?}"
        )),
    }
}

pub fn runtime_value_to_com_subscription_token(
    value: &RuntimeValue,
    field: &str,
) -> Result<ComSubscriptionToken, String> {
    match value {
        RuntimeValue::I32(raw) => Ok(ComSubscriptionToken::new(*raw)),
        RuntimeValue::I64(raw) => i32::try_from(*raw)
            .map(ComSubscriptionToken::new)
            .map_err(|_| format!("{field} exceeds i32 subscription-token range: {raw}")),
        other => Err(format!(
            "{field} requires subscription-token-compatible carrier, got {other:?}"
        )),
    }
}

pub fn runtime_value_to_com_callback_token(
    value: &RuntimeValue,
    field: &str,
) -> Result<ComCallbackToken, String> {
    match value {
        RuntimeValue::I32(raw) => Ok(ComCallbackToken::new(*raw)),
        RuntimeValue::I64(raw) => i32::try_from(*raw)
            .map(ComCallbackToken::new)
            .map_err(|_| format!("{field} exceeds i32 callback-token range: {raw}")),
        other => Err(format!(
            "{field} requires callback-token-compatible carrier, got {other:?}"
        )),
    }
}

pub fn runtime_value_to_dynamic_member_selector(
    value: &RuntimeValue,
    field: &str,
) -> Result<DynamicMemberSelector, String> {
    match value {
        RuntimeValue::String(text) => Ok(DynamicMemberSelector::Name(text.as_str().to_string())),
        RuntimeValue::Empty => Ok(DynamicMemberSelector::DefaultMember),
        RuntimeValue::I32(token) => {
            if *token == 0 {
                Ok(DynamicMemberSelector::DefaultMember)
            } else {
                Ok(DynamicMemberSelector::Token(*token))
            }
        }
        RuntimeValue::I64(token) => {
            let token = i32::try_from(*token)
                .map_err(|_| format!("{field} exceeds i32 selector-token range: {token}"))?;
            if token == 0 {
                Ok(DynamicMemberSelector::DefaultMember)
            } else {
                Ok(DynamicMemberSelector::Token(token))
            }
        }
        other => Err(format!(
            "{field} requires string-or-token selector carrier, got {other:?}"
        )),
    }
}

pub fn runtime_value_to_usize_index(value: &RuntimeValue, field: &str) -> Result<usize, String> {
    let index = runtime_value_to_i32_compat(value, field)?;
    if index < 0 {
        return Err(format!("{field} cannot be negative: {index}"));
    }
    usize::try_from(index).map_err(|_| format!("{field} exceeds usize range: {index}"))
}

fn runtime_array_bounds(
    array: &oxvba_runtime::safe_array::SafeArray,
) -> Vec<oxvba_runtime::safe_array::SafeArrayBound> {
    array.bounds.clone().unwrap_or_else(|| {
        vec![oxvba_runtime::safe_array::SafeArrayBound {
            lower: 0,
            count: u32::try_from(array.len).unwrap_or(u32::MAX),
        }]
    })
}

fn runtime_array_offset(
    bounds: &[oxvba_runtime::safe_array::SafeArrayBound],
    indices: &[i32],
    field: &str,
) -> Result<usize, String> {
    if bounds.len() != indices.len() {
        return Err(format!(
            "{field} expects {} dimensions, got {}",
            bounds.len(),
            indices.len()
        ));
    }
    let mut offset = 0usize;
    let mut stride = 1usize;
    for dim in (0..bounds.len()).rev() {
        let bound = &bounds[dim];
        let upper = bound
            .lower
            .checked_add(i32::try_from(bound.count).map_err(|_| {
                format!("{field} bound metadata overflowed while computing the upper bound")
            })?)
            .and_then(|value| value.checked_sub(1))
            .ok_or_else(|| format!("{field} bound metadata overflowed"))?;
        let index = indices[dim];
        if index < bound.lower || index > upper {
            return Err(format!(
                "{field} index {} is out of range for dimension {} ({} to {})",
                index,
                dim + 1,
                bound.lower,
                upper
            ));
        }
        let normalized = usize::try_from(index - bound.lower)
            .map_err(|_| format!("{field} index {index} exceeds host index capacity"))?;
        offset =
            offset
                .checked_add(normalized.checked_mul(stride).ok_or_else(|| {
                    format!("{field} index computation overflowed the host stride")
                })?)
                .ok_or_else(|| format!("{field} index computation overflowed the host offset"))?;
        stride = stride
            .checked_mul(bound.count as usize)
            .ok_or_else(|| format!("{field} index computation overflowed the host stride"))?;
    }
    Ok(offset)
}

fn runtime_array_indices(index_values: &[RuntimeValue], field: &str) -> Result<Vec<i32>, String> {
    index_values
        .iter()
        .map(|value| runtime_value_to_i32_compat(value, field))
        .collect()
}

pub fn runtime_array_get(
    array_value: &RuntimeValue,
    index_values: &[RuntimeValue],
    field: &str,
) -> Result<RuntimeValue, String> {
    let RuntimeValue::ArrayIntent(array) = array_value else {
        return Err(format!(
            "{field} requires a runtime array value, got {array_value:?}"
        ));
    };
    let elements = array
        .elements
        .as_ref()
        .ok_or_else(|| format!("{field} array payload is missing element storage"))?;
    let indices = runtime_array_indices(index_values, field)?;
    let bounds = runtime_array_bounds(array);
    let offset = runtime_array_offset(&bounds, &indices, field)?;
    elements
        .get(offset)
        .cloned()
        .ok_or_else(|| format!("{field} index {:?} is out of range", indices))
}

pub fn runtime_array_set(
    array_value: &RuntimeValue,
    index_values: &[RuntimeValue],
    new_value: &RuntimeValue,
    field: &str,
) -> Result<RuntimeValue, String> {
    let RuntimeValue::ArrayIntent(array) = array_value else {
        return Err(format!(
            "{field} requires a runtime array value, got {array_value:?}"
        ));
    };
    let mut updated = array.clone();
    let bounds = runtime_array_bounds(&updated);
    let indices = runtime_array_indices(index_values, field)?;
    let offset = runtime_array_offset(&bounds, &indices, field)?;
    let elements = updated
        .elements
        .as_mut()
        .ok_or_else(|| format!("{field} array payload is missing element storage"))?;
    let Some(slot) = elements.get_mut(offset) else {
        return Err(format!("{field} index {:?} is out of range", indices));
    };
    *slot = new_value.clone();
    Ok(RuntimeValue::ArrayIntent(updated))
}

pub fn runtime_array_lbound(array_value: &RuntimeValue, field: &str) -> Result<i32, String> {
    match array_value {
        RuntimeValue::ArrayIntent(array) => Ok(array
            .bounds
            .as_ref()
            .and_then(|bounds| bounds.first())
            .map(|bound| bound.lower)
            .unwrap_or(0)),
        RuntimeValue::I32(legacy) if oxvba_runtime::safe_array::is_array_tag(*legacy) => Ok(0),
        RuntimeValue::I64(legacy)
            if i32::try_from(*legacy)
                .ok()
                .is_some_and(oxvba_runtime::safe_array::is_array_tag) =>
        {
            Ok(0)
        }
        other => Err(format!(
            "{field} requires runtime array or array-tag carrier, got {other:?}"
        )),
    }
}

pub fn runtime_array_ubound(array_value: &RuntimeValue, field: &str) -> Result<i32, String> {
    match array_value {
        RuntimeValue::ArrayIntent(array) => {
            let Some(bound) = array.bounds.as_ref().and_then(|bounds| bounds.first()) else {
                return i32::try_from(array.len)
                    .map(|len| len - 1)
                    .map_err(|_| format!("{field} array length exceeds i32 range"));
            };
            let count = i32::try_from(bound.count)
                .map_err(|_| format!("{field} bound metadata overflowed"))?;
            Ok(bound.lower + count - 1)
        }
        RuntimeValue::I32(legacy) if oxvba_runtime::safe_array::is_array_tag(*legacy) => {
            Ok(oxvba_runtime::safe_array::array_len_from_tag(*legacy)
                .and_then(|count| i32::try_from(count).ok())
                .unwrap_or(0)
                - 1)
        }
        RuntimeValue::I64(legacy)
            if i32::try_from(*legacy)
                .ok()
                .is_some_and(oxvba_runtime::safe_array::is_array_tag) =>
        {
            let legacy = i32::try_from(*legacy)
                .map_err(|_| format!("{field} array-tag carrier exceeds i32 range: {legacy}"))?;
            Ok(oxvba_runtime::safe_array::array_len_from_tag(legacy)
                .and_then(|count| i32::try_from(count).ok())
                .unwrap_or(0)
                - 1)
        }
        other => Err(format!(
            "{field} requires runtime array or array-tag carrier, got {other:?}"
        )),
    }
}

// ── WithEvents Key Functions ──────────────────────────────────────────

pub fn withevents_binding_key(owner: ObjectHandle, binding: BindingHandle) -> i64 {
    ((owner.raw() as i64) << 32) | (binding.raw() as u32 as i64)
}

pub fn withevents_binding_from_key(key: i64) -> BindingHandle {
    BindingHandle::new((key as u32) as i32)
}

pub fn withevents_owner_from_key(key: i64) -> ObjectHandle {
    ObjectHandle::new((key >> 32) as i32)
}

pub fn withevents_binding_handle(
    value: &RuntimeValue,
    field: &str,
) -> Result<BindingHandle, String> {
    match value {
        RuntimeValue::BindingHandle(handle) => Ok(*handle),
        RuntimeValue::I32(raw) => Ok(BindingHandle::new(*raw)),
        RuntimeValue::I64(raw) => i32::try_from(*raw)
            .map(BindingHandle::new)
            .map_err(|_| format!("WithEvents {field} exceeds i32 handle range: {raw}")),
        other => Err(format!(
            "WithEvents {field} requires binding-handle-compatible carrier, got {other:?}"
        )),
    }
}

pub fn withevents_owner_handle(value: &RuntimeValue, field: &str) -> Result<ObjectHandle, String> {
    match value {
        RuntimeValue::Object(handle) => Ok(ObjectHandle::new(handle.raw())),
        RuntimeValue::ObjectHandle(handle) => Ok(*handle),
        // Project-lowered implicit root calls can omit the hidden owner carrier on
        // statement-context paths. Treat that the same as the established root-zero
        // owner so the generated WithEvents backing store remains addressable.
        RuntimeValue::Empty => Ok(ObjectHandle::new(0)),
        RuntimeValue::I32(raw) => Ok(ObjectHandle::new(*raw)),
        RuntimeValue::I64(raw) => i32::try_from(*raw)
            .map(ObjectHandle::new)
            .map_err(|_| format!("WithEvents {field} exceeds i32 handle range: {raw}")),
        other => Err(format!(
            "WithEvents {field} requires object-handle-compatible carrier, got {other:?}"
        )),
    }
}
