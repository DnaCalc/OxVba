//! Shared pure semantic functions for VBA runtime operations.
//!
//! These functions are extracted from the interpreter so they can be reused
//! by the JIT runtime helpers without duplication.
//!
#![allow(clippy::items_after_test_module)]

use oxvba_com::{ComCallbackToken, ComMemberToken, ComSubscriptionToken, DynamicMemberSelector};
use oxvba_compiler::bytecode::{
    NumericCoerceTarget, RuntimeAssignmentIntent, RuntimeAssignmentTargetKind, StringCompareMode,
};
use oxvba_runtime::value_types::validate_date_range;
use oxvba_runtime::{BindingHandle, CurrencyValue, ObjectRef, VarType, Variant, bstr::BStr};

const SECONDS_PER_DAY: f64 = 86_400.0;

// ── Coercion & Type Checks ────────────────────────────────────────────

pub fn runtime_variant_as_f64(value: &Variant) -> Result<f64, String> {
    match value.vtype() {
        VarType::Empty => Ok(0.0),
        VarType::Integer => Ok(value.as_i16().unwrap_or(0) as f64),
        VarType::Long => Ok(value.as_i32().unwrap_or(0) as f64),
        VarType::LongLong => Ok(value.as_i64().unwrap_or(0) as f64),
        VarType::Byte => Ok(value.as_u8().unwrap_or(0) as f64),
        VarType::Single => Ok(f64::from(value.as_f32().unwrap_or(0.0))),
        VarType::Double => Ok(value.as_f64().unwrap_or(0.0)),
        VarType::Date => Ok(value.as_date_f64().unwrap_or(0.0)),
        VarType::Boolean => Ok(if value.as_bool().unwrap_or(false) {
            -1.0
        } else {
            0.0
        }),
        VarType::Currency => {
            Ok(value.as_currency_scaled_i64().unwrap_or(0) as f64 / CurrencyValue::SCALE as f64)
        }
        VarType::Decimal => {
            let decimal = value
                .as_decimal96()
                .ok_or_else(|| "invalid Decimal variant payload".to_string())?;
            let mag = decimal.magnitude_u128() as f64;
            let scale = 10f64.powi(decimal.scale() as i32);
            Ok(if decimal.is_negative() {
                -(mag / scale)
            } else {
                mag / scale
            })
        }
        other => Err(format!("cannot coerce {:?} Variant to f64", other)),
    }
}

pub fn runtime_variant_to_text(value: &Variant, field: &str) -> Result<String, String> {
    match value.vtype() {
        VarType::Empty => Ok(String::new()),
        VarType::Integer => Ok(value.as_i16().unwrap_or(0).to_string()),
        VarType::Long => Ok(value.as_i32().unwrap_or(0).to_string()),
        VarType::LongLong => Ok(value.as_i64().unwrap_or(0).to_string()),
        VarType::SignedByte => Ok(value.as_i8().unwrap_or(0).to_string()),
        VarType::Byte => Ok(value.as_u8().unwrap_or(0).to_string()),
        VarType::UnsignedInteger => Ok(value.as_u16().unwrap_or(0).to_string()),
        VarType::UnsignedLong | VarType::UnsignedInt => Ok(value.as_u32().unwrap_or(0).to_string()),
        VarType::UnsignedLongLong => Ok(value.as_u64().unwrap_or(0).to_string()),
        VarType::String => value
            .as_bstr()
            .map(|text| text.as_str().to_string())
            .ok_or_else(|| "invalid String variant payload".to_string()),
        VarType::Boolean => Ok(if value.as_bool().unwrap_or(false) {
            "-1".to_string()
        } else {
            "0".to_string()
        }),
        VarType::Single => Ok(f64::from(value.as_f32().unwrap_or(0.0)).to_string()),
        VarType::Double => Ok(value.as_f64().unwrap_or(0.0).to_string()),
        VarType::Date => format_date_serial_digits(value.as_date_f64().unwrap_or(0.0))
            .or_else(|_| Ok(value.as_date_f64().unwrap_or(0.0).to_string())),
        VarType::Currency => Ok((value.as_currency_scaled_i64().unwrap_or(0) as f64
            / CurrencyValue::SCALE as f64)
            .to_string()),
        VarType::Decimal => value
            .as_decimal96()
            .map(|decimal| decimal.to_string())
            .ok_or_else(|| "invalid Decimal variant payload".to_string()),
        VarType::Null => Err(format!("{field} requires text-compatible value, got Null")),
        VarType::Error => Ok(value.as_error_code().unwrap_or(0).to_string()),
        VarType::ArrayVariant => Err(format!(
            "{field} requires scalar text-compatible value, got array"
        )),
        VarType::Object => Ok(value
            .as_object_ref()
            .map(|handle| handle.raw().to_string())
            .unwrap_or_else(|| "0".to_string())),
    }
}

pub fn runtime_split_count_variant_bounded(
    value: &Variant,
    delimiter: &Variant,
) -> Result<Variant, String> {
    let text = runtime_variant_to_text(value, "Split src")?;
    let delimiter = runtime_variant_to_text(delimiter, "Split delimiter")?;
    let count = if delimiter.is_empty() {
        1
    } else {
        i32::try_from(text.split(&delimiter).count())
            .map_err(|_| "Split piece count exceeded i32 range".to_string())?
    };
    Ok(Variant::from_i32(count))
}

pub fn runtime_join_variant_bounded(
    value: &Variant,
    delimiter: &Variant,
) -> Result<Variant, String> {
    let _ = runtime_variant_to_text(delimiter, "Join delimiter")?;
    let out = match value.vtype() {
        VarType::ArrayVariant => {
            let array = value
                .as_safearray()
                .ok_or_else(|| "Join src has invalid array payload".to_string())?;
            i32::try_from(array.len())
                .map_err(|_| "Join array length exceeded i32 range".to_string())?
        }
        VarType::Empty => 0,
        VarType::String => value
            .as_bstr()
            .and_then(|text| text.as_str().parse::<i32>().ok())
            .unwrap_or(0),
        _ => runtime_variant_to_i32_compat(value, "Join src")?,
    };
    Ok(Variant::from_i32(out))
}

pub fn runtime_like_variant_bounded(
    lhs: &Variant,
    pattern: &Variant,
    mode: StringCompareMode,
) -> Result<Variant, String> {
    let lhs = normalize_for_compare(runtime_variant_to_text(lhs, "Like lhs")?, mode);
    let pattern = normalize_for_compare(runtime_variant_to_text(pattern, "Like pattern")?, mode);
    Ok(Variant::from_i32(if lhs == pattern { -1 } else { 0 }))
}

pub fn runtime_strconv_variant_bounded(
    src: &Variant,
    conversion: &Variant,
) -> Result<Variant, String> {
    let text = runtime_variant_to_text(src, "StrConv source")?;
    let conv = runtime_variant_to_i32_compat(conversion, "StrConv conversion")?;
    let result = match conv {
        1 => text.to_uppercase(),
        2 => text.to_lowercase(),
        3 => proper_case(&text),
        _ => text,
    };
    Ok(Variant::from_string(BStr::from(result)))
}

pub fn runtime_chr_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let value = runtime_variant_to_i32_compat(src, "Chr operand")?;
    let ch = char::from_u32(value as u32).unwrap_or('\0');
    Ok(Variant::from_string(BStr::from(ch.to_string())))
}

pub fn runtime_asc_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let text = runtime_variant_to_text(src, "Asc operand")?;
    let code = if text.is_empty() {
        0
    } else {
        text.as_bytes()[0] as i32
    };
    Ok(Variant::from_i32(code))
}

pub fn runtime_space_variant_bounded(count: &Variant) -> Result<Variant, String> {
    let count = runtime_variant_to_i32_compat(count, "Space count")?;
    Ok(Variant::from_string(BStr::from(
        " ".repeat(count.max(0) as usize),
    )))
}

pub fn runtime_string_repeat_variant_bounded(
    count: &Variant,
    ch: &Variant,
) -> Result<Variant, String> {
    let count = runtime_variant_to_i32_compat(count, "String$ count")?;
    let ch = if let Some(text) = ch.as_bstr() {
        if text.is_empty() {
            '\0'
        } else {
            text.as_str().chars().next().unwrap_or('\0')
        }
    } else {
        char::from_u32(runtime_variant_to_i32_compat(ch, "String$ char")? as u32).unwrap_or('\0')
    };
    Ok(Variant::from_string(BStr::from(
        ch.to_string().repeat(count.max(0) as usize),
    )))
}

pub fn runtime_hex_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let value = runtime_variant_to_i32_compat(src, "Hex operand")?;
    Ok(Variant::from_string(BStr::from(format!(
        "{:X}",
        value as u32
    ))))
}

pub fn runtime_oct_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let value = runtime_variant_to_i32_compat(src, "Oct operand")?;
    Ok(Variant::from_string(BStr::from(format!(
        "{:o}",
        value as u32
    ))))
}

pub fn runtime_format_variant_bounded(
    value: &Variant,
    format_string: Option<&Variant>,
) -> Result<Variant, String> {
    let number = runtime_variant_as_f64(value)?;
    let format_string = match format_string {
        Some(format) => Some(runtime_variant_to_text(format, "Format string")?),
        None => None,
    };
    Ok(Variant::from_string(BStr::from(format_number(
        number,
        format_string.as_deref(),
    ))))
}

pub fn runtime_val_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let result = match src.vtype() {
        VarType::String => {
            let text = src
                .as_bstr()
                .ok_or_else(|| "Val src has invalid String payload".to_string())?;
            let text = text.as_str();
            let trimmed = text.trim();
            if trimmed.is_empty() {
                Variant::from_i32(0)
            } else if let Ok(n) = trimmed.parse::<i64>() {
                if n >= i32::MIN as i64 && n <= i32::MAX as i64 {
                    Variant::from_i32(n as i32)
                } else {
                    Variant::from_f64(n as f64)
                }
            } else if let Ok(f) = trimmed.parse::<f64>() {
                if f == f.trunc() && f >= i32::MIN as f64 && f <= i32::MAX as f64 {
                    Variant::from_i32(f as i32)
                } else {
                    Variant::from_f64(f)
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
                            Variant::from_i32(f as i32)
                        } else {
                            Variant::from_f64(f)
                        }
                    } else {
                        Variant::from_i32(0)
                    }
                } else {
                    Variant::from_i32(0)
                }
            }
        }
        VarType::Long | VarType::LongLong | VarType::Double => src.clone(),
        VarType::Empty => Variant::from_i32(0),
        _ => {
            let numeric = runtime_variant_to_numeric_compat(src, "Val src")?;
            if numeric == numeric.trunc()
                && numeric >= i32::MIN as f64
                && numeric <= i32::MAX as f64
            {
                Variant::from_i32(numeric as i32)
            } else {
                Variant::from_f64(numeric)
            }
        }
    };
    Ok(result)
}

pub fn runtime_variant_to_vba_str(src: &Variant) -> Result<Variant, String> {
    let text = oxvba_runtime::variant_to_vba_string(src)?;
    let is_numeric = matches!(
        src.vtype(),
        VarType::Integer
            | VarType::Long
            | VarType::LongLong
            | VarType::Byte
            | VarType::Single
            | VarType::Double
            | VarType::Date
            | VarType::Currency
            | VarType::Decimal
    );
    if is_numeric && !text.is_empty() && !text.as_str().starts_with('-') {
        Ok(Variant::from_string(BStr::from(format!(
            " {}",
            text.as_str()
        ))))
    } else {
        Ok(Variant::from_string(text))
    }
}

pub fn runtime_abs_variant_bounded(src: &Variant) -> Result<Variant, String> {
    match src.vtype() {
        VarType::Null => Ok(Variant::null()),
        VarType::Single | VarType::Double | VarType::Date => {
            Ok(Variant::from_f64(runtime_variant_as_f64(src)?.abs()))
        }
        _ => {
            let value = runtime_variant_to_i32_compat(src, "Abs operand")?;
            Ok(Variant::from_i32(if value == i32::MIN {
                i32::MAX
            } else {
                value.abs()
            }))
        }
    }
}

pub fn runtime_sgn_variant_bounded(src: &Variant) -> Result<Variant, String> {
    match src.vtype() {
        VarType::Null => Ok(Variant::null()),
        VarType::Single | VarType::Double | VarType::Date => {
            let value = runtime_variant_as_f64(src)?;
            Ok(Variant::from_i32(if value > 0.0 {
                1
            } else if value < 0.0 {
                -1
            } else {
                0
            }))
        }
        _ => Ok(Variant::from_i32(
            runtime_variant_to_i32_compat(src, "Sgn operand")?.signum(),
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

pub fn runtime_round_variant_bounded(
    src: &Variant,
    digits: Option<&Variant>,
) -> Result<Variant, String> {
    let value = runtime_variant_to_i32_compat(src, "Round operand")?;
    let digits = match digits {
        Some(value) => runtime_variant_to_i32_compat(value, "Round digits")?,
        None => 0,
    };
    Ok(Variant::from_i32(runtime_round_i32_bounded(value, digits)))
}

pub fn runtime_sqr_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let value = runtime_variant_to_i32_compat(src, "Sqr operand")?;
    Ok(Variant::from_i32(
        (value.saturating_abs() as f64).sqrt() as i32
    ))
}

pub fn runtime_sin_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let value = runtime_variant_to_i32_compat(src, "Sin operand")?;
    Ok(Variant::from_i32((value as f64).sin().round() as i32))
}

pub fn runtime_cos_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let value = runtime_variant_to_i32_compat(src, "Cos operand")?;
    Ok(Variant::from_i32((value as f64).cos().round() as i32))
}

pub fn runtime_log_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let value = runtime_variant_to_i32_compat(src, "Log operand")?;
    Ok(Variant::from_i32(if value > 0 {
        (value as f64).ln().round() as i32
    } else {
        0
    }))
}

pub fn runtime_exp_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let value = runtime_variant_to_i32_compat(src, "Exp operand")?;
    Ok(Variant::from_i32((value as f64).exp().round() as i32))
}

pub fn runtime_atn_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let value = runtime_variant_to_i32_compat(src, "Atn operand")?;
    Ok(Variant::from_i32((value as f64).atan().round() as i32))
}

pub fn runtime_tan_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let value = runtime_variant_to_i32_compat(src, "Tan operand")?;
    Ok(Variant::from_i32((value as f64).tan().round() as i32))
}

pub fn runtime_month_name_variant_bounded(src: &Variant) -> Result<Variant, String> {
    let month = runtime_variant_to_i32_compat(src, "MonthName operand")?;
    Ok(Variant::from_string(BStr::from(
        month_name(month).to_string(),
    )))
}

fn month_name(month: i32) -> &'static str {
    match month {
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
    }
}

pub fn runtime_date_serial_variant_bounded(
    year: &Variant,
    month: &Variant,
    day: &Variant,
) -> Result<Variant, String> {
    let packed = checked_packed_date(
        runtime_variant_to_i32_compat(year, "DateSerial year")?,
        runtime_variant_to_i32_compat(month, "DateSerial month")?,
        runtime_variant_to_i32_compat(day, "DateSerial day")?,
    )?;
    Ok(Variant::from_date_f64(packed_date_to_ole_serial(packed)?))
}

pub fn runtime_time_serial_variant_bounded(
    hour: &Variant,
    minute: &Variant,
    second: &Variant,
) -> Result<Variant, String> {
    let total_seconds = i64::from(runtime_variant_to_i32_compat(hour, "TimeSerial hour")?) * 3600
        + i64::from(runtime_variant_to_i32_compat(minute, "TimeSerial minute")?) * 60
        + i64::from(runtime_variant_to_i32_compat(second, "TimeSerial second")?);
    Ok(Variant::from_date_f64(
        total_seconds as f64 / SECONDS_PER_DAY,
    ))
}

pub fn runtime_date_add_variant_bounded(
    interval: &Variant,
    number: &Variant,
    date: &Variant,
) -> Result<Variant, String> {
    let _interval = runtime_variant_to_i32_compat(interval, "DateAdd interval")?;
    let number = runtime_variant_to_i32_compat(number, "DateAdd number")?;
    let serial = date_serial_from_variant(date)?;
    Ok(Variant::from_date_f64(validate_date_range(
        serial + f64::from(number),
    )?))
}

pub fn runtime_date_diff_variant_bounded(
    interval: &Variant,
    date1: &Variant,
    date2: &Variant,
) -> Result<Variant, String> {
    let _interval = runtime_variant_to_i32_compat(interval, "DateDiff interval")?;
    let serial1 = date_serial_from_variant(date1)?.floor() as i64;
    let serial2 = date_serial_from_variant(date2)?.floor() as i64;
    let delta = i32::try_from(serial2 - serial1)
        .map_err(|_| format!("DateDiff result overflowed i32 span: {serial1}..{serial2}"))?;
    Ok(Variant::from_i32(delta))
}

pub fn runtime_mid_stmt_variant_bounded(
    target: &Variant,
    start: &Variant,
    count: Option<&Variant>,
    value: &Variant,
) -> Result<Variant, String> {
    let base = runtime_variant_to_text(target, "MidStmt target")?;
    let repl = runtime_variant_to_text(value, "MidStmt value")?;
    let start = runtime_variant_to_i32_compat(start, "MidStmt start")?.max(0) as usize;
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
            let count = runtime_variant_to_i32_compat(count, "MidStmt count")?.max(0) as usize;
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

    if matches!(target.vtype(), VarType::String) || matches!(value.vtype(), VarType::String) {
        return Ok(Variant::from_string(BStr::from(out)));
    }
    if let Ok(parsed) = out.parse::<i32>() {
        return Ok(Variant::from_i32(parsed));
    }
    Ok(Variant::from_string(BStr::from(out)))
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

pub fn runtime_variant_to_cdate(value: &Variant) -> Result<Variant, String> {
    let serial = match value.vtype() {
        VarType::String => {
            let text = value
                .as_bstr()
                .ok_or_else(|| "CDate src has invalid String payload".to_string())?;
            let packed = parse_string_date_to_packed(&text.as_str())
                .ok_or_else(|| format!("CDate string format is not yet supported: `{text}`"))?;
            packed_date_to_ole_serial(packed)?
        }
        VarType::Long => {
            let raw = value
                .as_i32()
                .ok_or_else(|| "CDate src has invalid Long payload".to_string())?;
            maybe_packed_date_to_ole_serial(raw).unwrap_or(validate_date_range(raw as f64)?)
        }
        VarType::LongLong => {
            let raw = value
                .as_i64()
                .ok_or_else(|| "CDate src has invalid LongLong payload".to_string())?;
            if let Ok(narrow) = i32::try_from(raw) {
                maybe_packed_date_to_ole_serial(narrow).unwrap_or(validate_date_range(raw as f64)?)
            } else {
                validate_date_range(raw as f64)?
            }
        }
        _ => validate_date_range(runtime_variant_to_numeric_compat(value, "CDate src")?)?,
    };
    Ok(Variant::from_date_f64(serial))
}

pub fn runtime_variant_to_datevalue(value: &Variant) -> Result<Variant, String> {
    Ok(Variant::from_date_f64(
        date_serial_from_variant(value)?.floor(),
    ))
}

pub fn runtime_variant_to_timevalue(value: &Variant) -> Result<Variant, String> {
    if let VarType::Long = value.vtype()
        && let Some(raw) = value.as_i32()
        && maybe_packed_date_to_ole_serial(raw).is_none()
    {
        let seconds = f64::from(raw).rem_euclid(SECONDS_PER_DAY);
        return Ok(Variant::from_date_f64(seconds / SECONDS_PER_DAY));
    }
    if let VarType::LongLong = value.vtype()
        && let Some(raw) = value.as_i64()
        && let Ok(narrow) = i32::try_from(raw)
        && maybe_packed_date_to_ole_serial(narrow).is_none()
    {
        let seconds = (raw as f64).rem_euclid(SECONDS_PER_DAY);
        return Ok(Variant::from_date_f64(seconds / SECONDS_PER_DAY));
    }
    Ok(Variant::from_date_f64(
        date_serial_from_variant(value)?.rem_euclid(1.0),
    ))
}

fn date_serial_from_variant(value: &Variant) -> Result<f64, String> {
    let date = runtime_variant_to_cdate(value)?;
    validate_date_range(
        date.as_date_f64()
            .ok_or_else(|| "CDate conversion returned non-Date Variant".to_string())?,
    )
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

pub fn variant_host_now_value(date: &Variant, time: &Variant) -> Result<Variant, String> {
    fn serial_number(value: &Variant, field: &str) -> Result<f64, String> {
        if let Some(value) = value.as_date_f64() {
            return Ok(value);
        }
        if let Some(value) = value.as_f64() {
            return Ok(value);
        }
        if let Some(value) = value.as_i32() {
            return Ok(f64::from(value));
        }
        if let Some(value) = value.as_i64() {
            return Ok(value as f64);
        }
        Err(format!(
            "Now {field} requires date/numeric Variant, got {:?}",
            value.vtype()
        ))
    }

    let date_serial = serial_number(date, "date")?.floor();
    let time_serial = serial_number(time, "time")?;
    let serial = validate_date_range(date_serial + time_serial)?;
    Ok(Variant::from_date_f64(serial))
}

pub fn runtime_variant_date_year(value: &Variant) -> Result<Variant, String> {
    let (year, _, _) = date_components_from_serial(date_serial_from_variant(value)?)?;
    Ok(Variant::from_i32(year))
}

pub fn runtime_variant_date_month(value: &Variant) -> Result<Variant, String> {
    let (_, month, _) = date_components_from_serial(date_serial_from_variant(value)?)?;
    Ok(Variant::from_i32(month as i32))
}

pub fn runtime_variant_date_day(value: &Variant) -> Result<Variant, String> {
    let (_, _, day) = date_components_from_serial(date_serial_from_variant(value)?)?;
    Ok(Variant::from_i32(day as i32))
}

pub fn runtime_variant_date_weekday(value: &Variant) -> Result<Variant, String> {
    let (year, month, day) = date_components_from_serial(date_serial_from_variant(value)?)?;
    Ok(Variant::from_i32(day_of_week(year, month, day) + 1))
}

pub fn runtime_variant_to_i32_compat(
    value: &oxvba_runtime::Variant,
    field: &str,
) -> Result<i32, String> {
    let numeric = runtime_variant_to_numeric_compat(value, field)?;

    if !numeric.is_finite() || numeric < i32::MIN as f64 || numeric > i32::MAX as f64 {
        return Err(format!("{field} exceeds i32 range: {numeric}"));
    }
    Ok(numeric.trunc() as i32)
}

pub fn runtime_variant_to_numeric_compat(
    value: &oxvba_runtime::Variant,
    field: &str,
) -> Result<f64, String> {
    Ok(match value.vtype() {
        oxvba_runtime::VarType::Empty => 0.0,
        oxvba_runtime::VarType::Integer => value
            .as_i16()
            .map(f64::from)
            .ok_or_else(|| format!("{field} has invalid Integer payload"))?,
        oxvba_runtime::VarType::Long => value
            .as_i32()
            .map(f64::from)
            .ok_or_else(|| format!("{field} has invalid Long payload"))?,
        oxvba_runtime::VarType::LongLong => value
            .as_i64()
            .map(|v| v as f64)
            .ok_or_else(|| format!("{field} has invalid LongLong payload"))?,
        oxvba_runtime::VarType::Single => value
            .as_f32()
            .map(f64::from)
            .ok_or_else(|| format!("{field} has invalid Single payload"))?,
        oxvba_runtime::VarType::Double => value
            .as_f64()
            .ok_or_else(|| format!("{field} has invalid Double payload"))?,
        oxvba_runtime::VarType::Date => value
            .as_date_f64()
            .ok_or_else(|| format!("{field} has invalid Date payload"))?,
        oxvba_runtime::VarType::Boolean => value
            .as_bool()
            .map(|v| if v { -1.0 } else { 0.0 })
            .ok_or_else(|| format!("{field} has invalid Boolean payload"))?,
        oxvba_runtime::VarType::Currency => value
            .as_currency_scaled_i64()
            .map(|v| v as f64 / CurrencyValue::SCALE as f64)
            .ok_or_else(|| format!("{field} has invalid Currency payload"))?,
        oxvba_runtime::VarType::Decimal => value
            .as_decimal96()
            .map(|d| {
                let mag = d.magnitude_u128() as f64;
                let scale = 10f64.powi(d.scale() as i32);
                if d.is_negative() {
                    -(mag / scale)
                } else {
                    mag / scale
                }
            })
            .ok_or_else(|| format!("{field} has invalid Decimal payload"))?,
        oxvba_runtime::VarType::Byte => value
            .as_u8()
            .map(f64::from)
            .ok_or_else(|| format!("{field} has invalid Byte payload"))?,
        oxvba_runtime::VarType::String => {
            let Some(text) = value.as_bstr() else {
                return Err(format!("{field} has invalid String payload"));
            };
            let text = text.as_str();
            let trimmed = text.trim();
            if trimmed.is_empty() {
                0.0
            } else {
                trimmed.parse::<f64>().map_err(|_| {
                    format!("{field} requires numeric-compatible text, got {text:?}")
                })?
            }
        }
        other => {
            return Err(format!(
                "{field} requires numeric-compatible Variant, got {:?}",
                other
            ));
        }
    })
}

pub fn variant_to_i32_compat(value: &Variant, field: &str) -> Result<i32, String> {
    runtime_variant_to_i32_compat(value, field)
}

fn runtime_i32_compat_is_date(value: i32) -> bool {
    maybe_packed_date_to_ole_serial(value).is_some() || validate_date_range(value as f64).is_ok()
}

pub fn runtime_variant_is_date(value: &oxvba_runtime::Variant) -> bool {
    match value.vtype() {
        oxvba_runtime::VarType::String => value
            .as_bstr()
            .map(|text| parse_string_date_to_packed(&text.as_str()).is_some())
            .unwrap_or(false),
        oxvba_runtime::VarType::Integer => value
            .as_i16()
            .map(|v| runtime_i32_compat_is_date(i32::from(v)))
            .unwrap_or(false),
        oxvba_runtime::VarType::Long => value
            .as_i32()
            .map(runtime_i32_compat_is_date)
            .unwrap_or(false),
        oxvba_runtime::VarType::LongLong => value
            .as_i64()
            .map(|v| {
                if let Ok(raw) = i32::try_from(v) {
                    runtime_i32_compat_is_date(raw)
                } else {
                    validate_date_range(v as f64).is_ok()
                }
            })
            .unwrap_or(false),
        oxvba_runtime::VarType::Single => value
            .as_f32()
            .map(|v| validate_date_range(f64::from(v)).is_ok())
            .unwrap_or(false),
        oxvba_runtime::VarType::Double => value
            .as_f64()
            .map(|v| validate_date_range(v).is_ok())
            .unwrap_or(false),
        oxvba_runtime::VarType::Date => value
            .as_date_f64()
            .map(|v| validate_date_range(v).is_ok())
            .unwrap_or(false),
        oxvba_runtime::VarType::Currency
        | oxvba_runtime::VarType::Decimal
        | oxvba_runtime::VarType::Byte => runtime_variant_to_i32_compat(value, "CDate src")
            .map(runtime_i32_compat_is_date)
            .unwrap_or(false),
        _ => false,
    }
}

pub fn variant_truthy_value(value: &Variant) -> Result<bool, String> {
    if matches!(value.vtype(), VarType::Null) {
        return Ok(false);
    }
    Ok(runtime_variant_to_numeric_compat(value, "boolean operand")? != 0.0)
}

pub fn variant_int_value(value: &Variant) -> Result<Variant, String> {
    if matches!(value.vtype(), VarType::Null) {
        return Ok(Variant::null());
    }
    if let Some(value) = value.as_f64() {
        return Ok(Variant::from_i32(value.floor() as i32));
    }
    if let Some(value) = value.as_date_f64() {
        return Ok(Variant::from_i32(value.floor() as i32));
    }
    Ok(Variant::from_i32(runtime_variant_to_i32_compat(
        value, "Int",
    )?))
}

pub fn variant_fix_value(value: &Variant) -> Result<Variant, String> {
    if matches!(value.vtype(), VarType::Null) {
        return Ok(Variant::null());
    }
    Ok(Variant::from_i32(runtime_variant_to_i32_compat(
        value, "Fix",
    )?))
}

pub fn runtime_variant_is_object(value: &oxvba_runtime::Variant) -> bool {
    matches!(value.vtype(), oxvba_runtime::VarType::Object)
}

/// True for the `Nothing` null-object reference, which lowers to integer 0 (an empty object
/// slot). `Set obj = Nothing` clears the reference, so this is an accepted `Set` source for an
/// object target alongside a real object. Non-zero non-object values (e.g. `Set obj = 5`) are
/// still rejected.
pub fn runtime_variant_is_nothing(value: &oxvba_runtime::Variant) -> bool {
    matches!(value.vtype(), oxvba_runtime::VarType::Empty) || value.as_i32() == Some(0)
}

pub fn runtime_object_identity_is(lhs: &Variant, rhs: &Variant) -> bool {
    match (lhs.as_object_ref(), rhs.as_object_ref()) {
        (Some(lhs), Some(rhs)) => lhs == rhs,
        (Some(lhs), None) => lhs.raw() == 0 && runtime_variant_is_nothing(rhs),
        (None, Some(rhs)) => runtime_variant_is_nothing(lhs) && rhs.raw() == 0,
        (None, None) => runtime_variant_is_nothing(lhs) && runtime_variant_is_nothing(rhs),
    }
}

pub fn runtime_vartype_tag_bounded_variant(value: &oxvba_runtime::Variant) -> i32 {
    match value.vtype() {
        oxvba_runtime::VarType::Empty => 0,
        oxvba_runtime::VarType::Null => 1,
        oxvba_runtime::VarType::Error => 10,
        oxvba_runtime::VarType::ArrayVariant => value
            .as_safearray()
            .map(|array| 8192 + i32::from(array.element_vartype()))
            .unwrap_or(8192 + 12),
        _ => 3,
    }
}

pub fn runtime_vartype_compat_bounded_variant(value: &oxvba_runtime::Variant) -> i32 {
    match value.vtype() {
        oxvba_runtime::VarType::Empty => 0,
        oxvba_runtime::VarType::Null => 1,
        oxvba_runtime::VarType::Integer => 2,
        // Preserve the current retained-value heuristic until declared type
        // tracking is available for exact VBA VarType parity.
        oxvba_runtime::VarType::Long => value
            .as_i32()
            .filter(|v| (-32768..=32767).contains(v))
            .map(|_| 2)
            .unwrap_or(3),
        oxvba_runtime::VarType::LongLong => 20,
        oxvba_runtime::VarType::SignedByte => 16,
        oxvba_runtime::VarType::UnsignedInteger => 18,
        oxvba_runtime::VarType::UnsignedLong => 19,
        oxvba_runtime::VarType::UnsignedLongLong => 21,
        oxvba_runtime::VarType::UnsignedInt => 23,
        oxvba_runtime::VarType::Single => 4,
        oxvba_runtime::VarType::Double => 5,
        oxvba_runtime::VarType::Currency => 6,
        oxvba_runtime::VarType::Date => 7,
        oxvba_runtime::VarType::String => 8,
        oxvba_runtime::VarType::Object => 9,
        oxvba_runtime::VarType::Error => 10,
        oxvba_runtime::VarType::Boolean => 11,
        oxvba_runtime::VarType::Decimal => 14,
        oxvba_runtime::VarType::Byte => 2,
        oxvba_runtime::VarType::ArrayVariant => value
            .as_safearray()
            .map(|array| 8192 + i32::from(array.element_vartype()))
            .unwrap_or(8192 + 12),
    }
}

pub fn runtime_typename_tag_bounded_variant(value: &oxvba_runtime::Variant) -> i32 {
    if matches!(value.vtype(), oxvba_runtime::VarType::ArrayVariant) {
        1001
    } else {
        1002
    }
}

pub fn runtime_is_numeric_tag_bounded_variant(value: &oxvba_runtime::Variant) -> i32 {
    match value.vtype() {
        oxvba_runtime::VarType::Empty
        | oxvba_runtime::VarType::Null
        | oxvba_runtime::VarType::Error
        | oxvba_runtime::VarType::ArrayVariant => 0,
        _ => 1,
    }
}

pub fn runtime_random_seed_variant_bounded(value: &Variant, field: &str) -> Result<i32, String> {
    runtime_variant_to_i32_compat(value, field)
}

// ── Arithmetic ────────────────────────────────────────────────────────

fn variant_is_null(value: &Variant) -> bool {
    matches!(value.vtype(), VarType::Null)
}

fn variant_is_error(value: &Variant) -> bool {
    matches!(value.vtype(), VarType::Error)
}

fn either_variant_null(lhs: &Variant, rhs: &Variant) -> bool {
    variant_is_null(lhs) || variant_is_null(rhs)
}

fn either_variant_error(lhs: &Variant, rhs: &Variant) -> bool {
    variant_is_error(lhs) || variant_is_error(rhs)
}

fn variant_is_f64_arithmetic(value: &Variant) -> bool {
    matches!(
        value.vtype(),
        VarType::Single | VarType::Double | VarType::Date | VarType::Currency | VarType::Decimal
    )
}

fn variant_is_f64_scalar(value: &Variant) -> bool {
    matches!(
        value.vtype(),
        VarType::Single | VarType::Double | VarType::Date
    )
}

pub fn variant_add_const_value(
    value: &Variant,
    delta: i32,
    field: &str,
) -> Result<Variant, String> {
    if variant_is_null(value) {
        return Ok(Variant::null());
    }
    if variant_is_error(value) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    if variant_is_f64_scalar(value) {
        return Ok(Variant::from_f64(
            runtime_variant_as_f64(value)? + delta as f64,
        ));
    }
    let value = runtime_variant_to_i32_compat(value, field)?;
    // VBA promotes Long overflow to Double rather than wrapping.
    match value.checked_add(delta) {
        Some(sum) => Ok(Variant::from_i32(sum)),
        None => Ok(Variant::from_f64(value as f64 + delta as f64)),
    }
}

pub fn variant_add_values(lhs: &Variant, rhs: &Variant) -> Result<Variant, String> {
    if either_variant_null(lhs, rhs) {
        return Ok(Variant::null());
    }
    if either_variant_error(lhs, rhs) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    if matches!(lhs.vtype(), VarType::String) && matches!(rhs.vtype(), VarType::String) {
        return Ok(variant_concat_values(lhs, rhs));
    }
    if variant_is_f64_arithmetic(lhs) || variant_is_f64_arithmetic(rhs) {
        let l = runtime_variant_as_f64(lhs)?;
        let r = runtime_variant_as_f64(rhs)?;
        return Ok(Variant::from_f64(l + r));
    }
    let lhs = runtime_variant_to_i32_compat(lhs, "add lhs")?;
    let rhs = runtime_variant_to_i32_compat(rhs, "add rhs")?;
    match lhs.checked_add(rhs) {
        Some(sum) => Ok(Variant::from_i32(sum)),
        None => Ok(Variant::from_f64(lhs as f64 + rhs as f64)),
    }
}

pub fn variant_sub_values(lhs: &Variant, rhs: &Variant) -> Result<Variant, String> {
    if either_variant_null(lhs, rhs) {
        return Ok(Variant::null());
    }
    if either_variant_error(lhs, rhs) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    if variant_is_f64_arithmetic(lhs) || variant_is_f64_arithmetic(rhs) {
        let l = runtime_variant_as_f64(lhs)?;
        let r = runtime_variant_as_f64(rhs)?;
        return Ok(Variant::from_f64(l - r));
    }
    let lhs = runtime_variant_to_i32_compat(lhs, "sub lhs")?;
    let rhs = runtime_variant_to_i32_compat(rhs, "sub rhs")?;
    match lhs.checked_sub(rhs) {
        Some(diff) => Ok(Variant::from_i32(diff)),
        None => Ok(Variant::from_f64(lhs as f64 - rhs as f64)),
    }
}

pub fn variant_mul_values(lhs: &Variant, rhs: &Variant) -> Result<Variant, String> {
    if either_variant_null(lhs, rhs) {
        return Ok(Variant::null());
    }
    if either_variant_error(lhs, rhs) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    if variant_is_f64_arithmetic(lhs) || variant_is_f64_arithmetic(rhs) {
        let l = runtime_variant_as_f64(lhs)?;
        let r = runtime_variant_as_f64(rhs)?;
        return Ok(Variant::from_f64(l * r));
    }
    let lhs = runtime_variant_to_i32_compat(lhs, "mul lhs")?;
    let rhs = runtime_variant_to_i32_compat(rhs, "mul rhs")?;
    let result = (lhs as i64) * (rhs as i64);
    if (i32::MIN as i64..=i32::MAX as i64).contains(&result) {
        Ok(Variant::from_i32(result as i32))
    } else {
        // VBA promotes Long overflow to Double rather than truncating.
        Ok(Variant::from_f64(result as f64))
    }
}

pub fn variant_pow_values(lhs: &Variant, rhs: &Variant) -> Result<Variant, String> {
    if either_variant_null(lhs, rhs) {
        return Ok(Variant::null());
    }
    if either_variant_error(lhs, rhs) {
        return Err("type mismatch: CVErr value in arithmetic".to_string());
    }
    let base = runtime_variant_to_numeric_compat(lhs, "pow base")?;
    let exp = runtime_variant_to_numeric_compat(rhs, "pow exponent")?;
    Ok(Variant::from_f64(base.powf(exp)))
}

pub fn variant_concat_values(lhs: &Variant, rhs: &Variant) -> Variant {
    let lhs_str = if variant_is_null(lhs) {
        String::new()
    } else {
        runtime_variant_to_text(lhs, "concat lhs").unwrap_or_default()
    };
    let rhs_str = if variant_is_null(rhs) {
        String::new()
    } else {
        runtime_variant_to_text(rhs, "concat rhs").unwrap_or_default()
    };
    Variant::from_string(BStr::from(format!("{lhs_str}{rhs_str}")))
}

pub fn variant_neg_value(value: &Variant) -> Result<Variant, String> {
    if variant_is_null(value) {
        return Ok(Variant::null());
    }
    if variant_is_f64_scalar(value) {
        return Ok(Variant::from_f64(-runtime_variant_as_f64(value)?));
    }
    let value = runtime_variant_to_i32_compat(value, "neg operand")?;
    match value.checked_neg() {
        Some(neg) => Ok(Variant::from_i32(neg)),
        None => Ok(Variant::from_f64(-(value as f64))),
    }
}

pub fn variant_increment_value(value: &Variant) -> Result<Variant, String> {
    if variant_is_f64_scalar(value) {
        return Ok(Variant::from_f64(runtime_variant_as_f64(value)? + 1.0));
    }
    let value = runtime_variant_to_i32_compat(value, "increment operand")?;
    match value.checked_add(1) {
        Some(sum) => Ok(Variant::from_i32(sum)),
        None => Ok(Variant::from_f64(value as f64 + 1.0)),
    }
}

/// Rounds half-to-even ("banker's rounding"), matching VBA `CLng`/`CInt` narrowing.
fn round_half_to_even(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let floor = x.floor();
    let diff = x - floor;
    if diff < 0.5 {
        floor
    } else if diff > 0.5 {
        floor + 1.0
    } else if (floor as i64) % 2 == 0 {
        floor
    } else {
        floor + 1.0
    }
}

/// Range-checks a numeric value against a fixed-width integer type, reporting overflow
/// (`Err(())`, surfaced as VBA run-time error 6) when it does not fit. Fractional values round
/// half-to-even (as `CInt`/`CLng` do) before the check, so e.g. `32767.4` fits `Integer` but
/// `32767.6` does not. In-range values are accepted unchanged — this is an overflow *guard*,
/// not a retag: VBA arithmetic on fixed integer types and assignment into a declared fixed
/// integer target error on overflow rather than widening. (Result subtype tagging — `Integer`
/// vs `Long` — is a separate fidelity concern, tracked apart from this overflow gate.)
pub fn variant_overflow_check(value: &Variant, target: NumericCoerceTarget) -> Result<(), ()> {
    let rounded = round_half_to_even(runtime_variant_as_f64(value).map_err(|_| ())?);
    let (lo, hi) = match target {
        NumericCoerceTarget::Byte => (0.0, 255.0),
        NumericCoerceTarget::Integer => (f64::from(i16::MIN), f64::from(i16::MAX)),
        NumericCoerceTarget::Long => (f64::from(i32::MIN), f64::from(i32::MAX)),
        NumericCoerceTarget::LongLong => (i64::MIN as f64, i64::MAX as f64),
    };
    if rounded < lo || rounded > hi {
        Err(())
    } else {
        Ok(())
    }
}

// ── Division (with error codes) ───────────────────────────────────────

/// Returns Ok(value) or Err(error_code) for division by zero (code 11).
pub fn variant_div_values(lhs: &Variant, rhs: &Variant) -> Result<Result<Variant, i32>, String> {
    if either_variant_null(lhs, rhs) {
        return Ok(Ok(Variant::null()));
    }
    let r = runtime_variant_to_numeric_compat(rhs, "div rhs")?;
    if r == 0.0 {
        return Ok(Err(11));
    }
    let l = runtime_variant_to_numeric_compat(lhs, "div lhs")?;
    Ok(Ok(Variant::from_f64(l / r)))
}

pub fn variant_intdiv_values(lhs: &Variant, rhs: &Variant) -> Result<Result<Variant, i32>, String> {
    if either_variant_null(lhs, rhs) {
        return Ok(Ok(Variant::null()));
    }
    let r = runtime_variant_to_numeric_compat(rhs, "intdiv rhs")?;
    let r_trunc = r as i32;
    if r_trunc == 0 {
        return Ok(Err(11));
    }
    let l = runtime_variant_to_numeric_compat(lhs, "intdiv lhs")?;
    Ok(Ok(Variant::from_i32((l / r).trunc() as i32)))
}

pub fn variant_mod_values(lhs: &Variant, rhs: &Variant) -> Result<Result<Variant, i32>, String> {
    if either_variant_null(lhs, rhs) {
        return Ok(Ok(Variant::null()));
    }
    let r = runtime_variant_to_numeric_compat(rhs, "mod rhs")?;
    let r_int = r as i32;
    if r_int == 0 {
        return Ok(Err(11));
    }
    let l = runtime_variant_to_numeric_compat(lhs, "mod lhs")?;
    // checked_rem guards the i32::MIN % -1 overflow edge (result is 0).
    Ok(Ok(Variant::from_i32(
        (l as i32).checked_rem(r_int).unwrap_or(0),
    )))
}

// ── Comparison ────────────────────────────────────────────────────────

pub fn normalize_for_compare(text: String, mode: StringCompareMode) -> String {
    match mode {
        StringCompareMode::Binary => text,
        StringCompareMode::Text => text.to_ascii_lowercase(),
    }
}

fn variant_exact_i64(value: &Variant) -> Option<i64> {
    match value.vtype() {
        VarType::Integer => value.as_i16().map(i64::from),
        VarType::Long => value.as_i32().map(i64::from),
        VarType::LongLong => value.as_i64(),
        VarType::Byte => value.as_u8().map(i64::from),
        _ => None,
    }
}

pub fn typed_compare_variants(
    lhs: &Variant,
    rhs: &Variant,
    mode: StringCompareMode,
    pred: fn(std::cmp::Ordering) -> bool,
) -> Result<bool, String> {
    if either_variant_null(lhs, rhs) {
        return Ok(false);
    }
    // Object comparison is VBA `Is` — identity by IUnknown pointer, uniform for internal,
    // referenced-project, and COM objects. `Variant`'s object equality compares the raw
    // IUnknown pointers (safe; no deref), so this works for real COM pointers too.
    if matches!(
        (lhs.vtype(), rhs.vtype()),
        (VarType::Object, VarType::Object)
    ) {
        let ordering = if lhs == rhs {
            std::cmp::Ordering::Equal
        } else {
            std::cmp::Ordering::Greater
        };
        return Ok(pred(ordering));
    }
    match (lhs.vtype(), rhs.vtype()) {
        (VarType::String, VarType::String) => {
            let a = normalize_for_compare(runtime_variant_to_text(lhs, "comparison lhs")?, mode);
            let b = normalize_for_compare(runtime_variant_to_text(rhs, "comparison rhs")?, mode);
            Ok(pred(a.cmp(&b)))
        }
        (VarType::String, VarType::Empty) => {
            let a = normalize_for_compare(runtime_variant_to_text(lhs, "comparison lhs")?, mode);
            Ok(pred(a.cmp(&String::new())))
        }
        (VarType::Empty, VarType::String) => {
            let b = normalize_for_compare(runtime_variant_to_text(rhs, "comparison rhs")?, mode);
            Ok(pred(String::new().cmp(&b)))
        }
        _ => {
            if let (Some(l), Some(r)) = (variant_exact_i64(lhs), variant_exact_i64(rhs)) {
                return Ok(pred(l.cmp(&r)));
            }
            if let (Ok(l), Ok(r)) = (
                runtime_variant_to_numeric_compat(lhs, "comparison lhs"),
                runtime_variant_to_numeric_compat(rhs, "comparison rhs"),
            ) {
                let ord = l.partial_cmp(&r).unwrap_or(std::cmp::Ordering::Equal);
                return Ok(pred(ord));
            }
            Err("comparison operands are not compatible for numeric comparison".to_string())
        }
    }
}

// ── Assignment Validation ─────────────────────────────────────────────

pub fn variant_assignment_value_label(value: &Variant) -> &'static str {
    match value.vtype() {
        VarType::Empty => "Empty",
        VarType::Null => "Null",
        VarType::Error => "Error",
        VarType::Integer => "Integer",
        VarType::Long => "Long",
        VarType::LongLong => "LongLong",
        VarType::SignedByte => "SignedByte",
        VarType::UnsignedInteger => "UnsignedInteger",
        VarType::UnsignedLong => "UnsignedLong",
        VarType::UnsignedLongLong => "UnsignedLongLong",
        VarType::UnsignedInt => "UnsignedInt",
        VarType::Single => "Single",
        VarType::Double => "Double",
        VarType::Date => "Date",
        VarType::Decimal => "Decimal",
        VarType::Currency => "Currency",
        VarType::Boolean => "Boolean",
        VarType::String => "String",
        VarType::ArrayVariant => "Array",
        VarType::Object => "Object",
        VarType::Byte => "Byte",
    }
}

pub fn validate_runtime_assignment_variant(
    value: &Variant,
    intent: RuntimeAssignmentIntent,
    target_kind: RuntimeAssignmentTargetKind,
    target_name: &str,
    target_type_name: &str,
) -> Result<(), String> {
    match (intent, target_kind) {
        (RuntimeAssignmentIntent::Set, RuntimeAssignmentTargetKind::Variant)
        | (RuntimeAssignmentIntent::Set, RuntimeAssignmentTargetKind::Object) => {
            if runtime_variant_is_object(value) || runtime_variant_is_nothing(value) {
                // A real object reference, or `Nothing` (clear the reference).
                Ok(())
            } else {
                Err(format!(
                    "Set requires object value for variable {target_name}"
                ))
            }
        }
        (RuntimeAssignmentIntent::Implicit, RuntimeAssignmentTargetKind::Object) => {
            if runtime_variant_is_object(value) {
                Err(format!("Set required for Object variable {target_name}"))
            } else {
                Err(format!(
                    "cannot assign {} to Object variable {target_name}",
                    variant_assignment_value_label(value)
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
            if runtime_variant_is_object(value) {
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

pub fn variant_to_com_object(value: &Variant, field: &str) -> Result<ObjectRef, String> {
    if let Some(object) = value.as_object_ref() {
        return Ok(object);
    }
    if let Some(raw) = value.as_i32() {
        return Ok(ObjectRef::from_compat_identity(raw));
    }
    if let Some(raw) = value.as_i64() {
        return i32::try_from(raw)
            .map(ObjectRef::from_compat_identity)
            .map_err(|_| format!("{field} exceeds i32 handle range: {raw}"));
    }
    Err(format!(
        "{field} requires object-compatible Variant, got {:?}",
        value.vtype()
    ))
}

pub fn variant_to_com_member_token(value: &Variant, field: &str) -> Result<ComMemberToken, String> {
    if let Some(raw) = value.as_i32() {
        return Ok(ComMemberToken::new(raw));
    }
    if let Some(raw) = value.as_i64() {
        return i32::try_from(raw)
            .map(ComMemberToken::new)
            .map_err(|_| format!("{field} exceeds i32 member-token range: {raw}"));
    }
    Err(format!(
        "{field} requires member-token-compatible Variant, got {:?}",
        value.vtype()
    ))
}

pub fn variant_to_com_subscription_token(
    value: &Variant,
    field: &str,
) -> Result<ComSubscriptionToken, String> {
    if let Some(raw) = value.as_i32() {
        return Ok(ComSubscriptionToken::new(raw));
    }
    if let Some(raw) = value.as_i64() {
        return i32::try_from(raw)
            .map(ComSubscriptionToken::new)
            .map_err(|_| format!("{field} exceeds i32 subscription-token range: {raw}"));
    }
    Err(format!(
        "{field} requires subscription-token-compatible Variant, got {:?}",
        value.vtype()
    ))
}

pub fn variant_to_com_callback_token(
    value: &Variant,
    field: &str,
) -> Result<ComCallbackToken, String> {
    if let Some(raw) = value.as_i32() {
        return Ok(ComCallbackToken::new(raw));
    }
    if let Some(raw) = value.as_i64() {
        return i32::try_from(raw)
            .map(ComCallbackToken::new)
            .map_err(|_| format!("{field} exceeds i32 callback-token range: {raw}"));
    }
    Err(format!(
        "{field} requires callback-token-compatible Variant, got {:?}",
        value.vtype()
    ))
}

pub fn variant_to_dynamic_member_selector(
    value: &Variant,
    field: &str,
) -> Result<DynamicMemberSelector, String> {
    if let Some(text) = value.as_bstr() {
        return Ok(DynamicMemberSelector::Name(text.as_str().to_string()));
    }
    if matches!(value.vtype(), VarType::Empty) {
        return Ok(DynamicMemberSelector::DefaultMember);
    }
    if let Some(token) = value.as_i32() {
        return Ok(if token == 0 {
            DynamicMemberSelector::DefaultMember
        } else {
            DynamicMemberSelector::Token(token)
        });
    }
    if let Some(token) = value.as_i64() {
        let token = i32::try_from(token)
            .map_err(|_| format!("{field} exceeds i32 selector-token range: {token}"))?;
        return Ok(if token == 0 {
            DynamicMemberSelector::DefaultMember
        } else {
            DynamicMemberSelector::Token(token)
        });
    }
    Err(format!(
        "{field} requires string-or-token selector Variant, got {:?}",
        value.vtype()
    ))
}

pub fn variant_to_usize_index(value: &Variant, field: &str) -> Result<usize, String> {
    let index = if let Some(raw) = value.as_i32() {
        raw
    } else if let Some(raw) = value.as_i64() {
        i32::try_from(raw).map_err(|_| format!("{field} exceeds i32 index range: {raw}"))?
    } else {
        return Err(format!(
            "{field} requires integer index Variant, got {:?}",
            value.vtype()
        ));
    };
    if index < 0 {
        return Err(format!("{field} cannot be negative: {index}"));
    }
    usize::try_from(index).map_err(|_| format!("{field} exceeds usize range: {index}"))
}

fn runtime_array_bounds(
    array: &oxvba_runtime::safe_array::SafeArray,
) -> Vec<oxvba_runtime::safe_array::SafeArrayBound> {
    array.bounds().unwrap_or_else(|| {
        vec![oxvba_runtime::safe_array::SafeArrayBound {
            lower: 0,
            count: u32::try_from(array.len()).unwrap_or(u32::MAX),
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

fn runtime_array_variant_indices(
    index_values: &[oxvba_runtime::Variant],
    field: &str,
) -> Result<Vec<i32>, String> {
    index_values
        .iter()
        .map(|value| runtime_variant_to_i32_compat(value, field))
        .collect()
}

pub fn runtime_array_get_variant(
    array_value: &oxvba_runtime::Variant,
    index_values: &[oxvba_runtime::Variant],
    field: &str,
) -> Result<oxvba_runtime::Variant, String> {
    let Some(array) = array_value.as_safearray() else {
        return Err(format!(
            "{field} requires a runtime array value, got {array_value:?}"
        ));
    };
    let elements_binding = array.variant_elements();
    let elements = elements_binding
        .as_ref()
        .ok_or_else(|| format!("{field} array payload is missing element storage"))?;
    let indices = runtime_array_variant_indices(index_values, field)?;
    let bounds = runtime_array_bounds(&array);
    let offset = runtime_array_offset(&bounds, &indices, field)?;
    elements
        .get(offset)
        .cloned()
        .ok_or_else(|| format!("{field} index {:?} is out of range", indices))
}

pub fn runtime_array_set_variant(
    array_value: &oxvba_runtime::Variant,
    index_values: &[oxvba_runtime::Variant],
    new_value: &oxvba_runtime::Variant,
    field: &str,
) -> Result<oxvba_runtime::Variant, String> {
    let Some(array) = array_value.as_safearray() else {
        return Err(format!(
            "{field} requires a runtime array value, got {array_value:?}"
        ));
    };
    let updated = array.clone();
    let bounds = runtime_array_bounds(&updated);
    let indices = runtime_array_variant_indices(index_values, field)?;
    let offset = runtime_array_offset(&bounds, &indices, field)?;
    let mut elements = updated
        .variant_elements()
        .ok_or_else(|| format!("{field} array payload is missing element storage"))?;
    let Some(slot) = elements.get_mut(offset) else {
        return Err(format!("{field} index {:?} is out of range", indices));
    };
    *slot = new_value.clone();
    Ok(oxvba_runtime::Variant::from_safearray(
        updated.replace_variant_elements(elements)?,
    ))
}

pub fn runtime_array_lbound_variant(
    array_value: &oxvba_runtime::Variant,
    field: &str,
) -> Result<i32, String> {
    let Some(array) = array_value.as_safearray() else {
        return Err(format!(
            "{field} requires a SAFEARRAY-backed Variant carrier, got {array_value:?}"
        ));
    };
    Ok(array
        .bounds()
        .as_ref()
        .and_then(|bounds| bounds.first())
        .map(|bound| bound.lower)
        .unwrap_or(0))
}

pub fn runtime_array_ubound_variant(
    array_value: &oxvba_runtime::Variant,
    field: &str,
) -> Result<i32, String> {
    let Some(array) = array_value.as_safearray() else {
        return Err(format!(
            "{field} requires a SAFEARRAY-backed Variant carrier, got {array_value:?}"
        ));
    };
    let bounds_binding = array.bounds();
    let Some(bound) = bounds_binding.as_ref().and_then(|bounds| bounds.first()) else {
        return i32::try_from(array.len())
            .map(|len| len - 1)
            .map_err(|_| format!("{field} array length exceeds i32 range"));
    };
    let count =
        i32::try_from(bound.count).map_err(|_| format!("{field} bound metadata overflowed"))?;
    Ok(bound.lower + count - 1)
}

// ── WithEvents Key Functions ──────────────────────────────────────────

pub fn withevents_binding_key(owner: &ObjectRef, binding: BindingHandle) -> i64 {
    ((owner.raw() as i64) << 32) | (binding.raw() as u32 as i64)
}

pub fn withevents_binding_from_key(key: i64) -> BindingHandle {
    BindingHandle::new((key as u32) as i32)
}

pub fn withevents_owner_from_key(key: i64) -> ObjectRef {
    ObjectRef::from_compat_identity((key >> 32) as i32)
}

pub fn variant_to_withevents_binding_handle(
    value: &Variant,
    field: &str,
) -> Result<BindingHandle, String> {
    if let Some(raw) = value.as_i32() {
        return Ok(BindingHandle::new(raw));
    }
    if let Some(raw) = value.as_i64() {
        return i32::try_from(raw)
            .map(BindingHandle::new)
            .map_err(|_| format!("WithEvents {field} exceeds i32 handle range: {raw}"));
    }
    Err(format!(
        "WithEvents {field} requires binding-handle-compatible Variant, got {:?}",
        value.vtype()
    ))
}

pub fn variant_to_withevents_owner_handle(
    value: &Variant,
    field: &str,
) -> Result<ObjectRef, String> {
    if matches!(value.vtype(), VarType::Empty) {
        return Ok(ObjectRef::from_compat_identity(0));
    }
    if let Some(object) = value.as_object_ref() {
        return Ok(object);
    }
    if let Some(raw) = value.as_i32() {
        return Ok(ObjectRef::from_compat_identity(raw));
    }
    if let Some(raw) = value.as_i64() {
        return i32::try_from(raw)
            .map(ObjectRef::from_compat_identity)
            .map_err(|_| format!("WithEvents {field} exceeds i32 handle range: {raw}"));
    }
    Err(format!(
        "WithEvents {field} requires object-handle-compatible Variant, got {:?}",
        value.vtype()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_compiler::bytecode::{RuntimeAssignmentIntent, RuntimeAssignmentTargetKind};
    use oxvba_runtime::{Decimal96, VarType, Variant};

    fn assert_double(value: &Variant, expected: f64) {
        assert_eq!(value.vtype(), VarType::Double, "value={value:?}");
        let actual = value.as_f64().expect("Double payload");
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual} from {value:?}"
        );
    }

    #[test]
    fn mixed_numeric_matrix_current_variant_results() {
        let integer_long = variant_add_values(&Variant::from_i16(2), &Variant::from_i32(3))
            .expect("Integer + Long");
        assert_eq!(integer_long.vtype(), VarType::Long);
        assert_eq!(integer_long.as_i32(), Some(5));

        let long_double = variant_add_values(&Variant::from_i32(3), &Variant::from_f64(0.5))
            .expect("Long + Double");
        assert_double(&long_double, 3.5);

        let currency_long = variant_add_values(
            &Variant::from_currency_scaled_i64(12_500),
            &Variant::from_i32(2),
        )
        .expect("Currency + Long");
        assert_double(&currency_long, 3.25);

        let decimal_long = variant_add_values(
            &Variant::from_decimal96(Decimal96::from_parts(123_450, 0, 0, 3, false)),
            &Variant::from_i16(1),
        )
        .expect("Decimal + Integer");
        assert_double(&decimal_long, 124.45);

        let date_long = variant_add_values(&Variant::from_date_f64(10.0), &Variant::from_i32(2))
            .expect("Date + Long");
        assert_double(&date_long, 12.0);

        let boolean_long = variant_add_values(&Variant::from_bool(true), &Variant::from_i32(2))
            .expect("Boolean + Long");
        assert_eq!(boolean_long.vtype(), VarType::Long);
        assert_eq!(boolean_long.as_i32(), Some(1));

        let string_long = variant_add_values(&Variant::from_string("7"), &Variant::from_i32(5))
            .expect("numeric String + Long");
        assert_eq!(string_long.vtype(), VarType::Long);
        assert_eq!(string_long.as_i32(), Some(12));

        let null_result =
            variant_add_values(&Variant::null(), &Variant::from_i32(5)).expect("Null + Long");
        assert_eq!(null_result.vtype(), VarType::Null);

        let error_result = variant_add_values(&Variant::from_error_code(13), &Variant::from_i32(5));
        assert!(
            error_result
                .expect_err("CVErr arithmetic should fail")
                .contains("CVErr")
        );

        let quotient = variant_div_values(&Variant::from_i32(7), &Variant::from_i32(2))
            .expect("division should evaluate")
            .expect("division should not fault");
        assert_double(&quotient, 3.5);

        let divide_by_zero = variant_div_values(&Variant::from_i32(7), &Variant::from_i32(0))
            .expect("division should classify runtime fault");
        assert_eq!(divide_by_zero, Err(11));
    }

    #[test]
    fn exact_scalar_carrier_expectations_preserve_variant_tags() {
        let currency = Variant::from_currency_scaled_i64(-42_500);
        assert_eq!(currency.vtype(), VarType::Currency);
        assert_eq!(currency.as_currency_scaled_i64(), Some(-42_500));

        let decimal = Decimal96::from_parts(0x5566_7788, 0x1122_3344, 0x99AA_BBCC, 4, true);
        let decimal_variant = Variant::from_decimal96(decimal);
        assert_eq!(decimal_variant.vtype(), VarType::Decimal);
        assert_eq!(decimal_variant.as_decimal96(), Some(decimal));

        let date = Variant::from_date_f64(46_081.25);
        assert_eq!(date.vtype(), VarType::Date);
        assert_eq!(date.as_date_f64(), Some(46_081.25));

        let boolean = Variant::from_bool(true);
        assert_eq!(boolean.vtype(), VarType::Boolean);
        assert_eq!(boolean.as_bool(), Some(true));
        assert_eq!(runtime_variant_to_i32_compat(&boolean, "Boolean"), Ok(-1));
    }

    #[test]
    fn numeric_stress_rounding_overflow_truncation_edges() {
        let rounded =
            runtime_round_variant_bounded(&Variant::from_i32(19), Some(&Variant::from_i32(-1)))
                .expect("Round should succeed");
        assert_eq!(rounded.as_i32(), Some(20));

        let overflow = variant_mul_values(&Variant::from_i32(50_000), &Variant::from_i32(50_000))
            .expect("Long multiplication promotes to Double on overflow");
        assert_eq!(overflow.vtype(), VarType::Double);
        assert_eq!(overflow.as_f64(), Some(2_500_000_000.0));

        let intdiv = variant_intdiv_values(&Variant::from_i32(7), &Variant::from_i32(2))
            .expect("integer division evaluates")
            .expect("integer division should not fault");
        assert_eq!(intdiv.as_i32(), Some(3));

        let modulo = variant_mod_values(&Variant::from_i32(7), &Variant::from_i32(2))
            .expect("mod evaluates")
            .expect("mod should not fault");
        assert_eq!(modulo.as_i32(), Some(1));

        let exponent = variant_pow_values(&Variant::from_i32(2), &Variant::from_i32(10))
            .expect("power evaluates");
        assert_double(&exponent, 1024.0);

        let null_neg = variant_neg_value(&Variant::null()).expect("Null negation propagates");
        assert_eq!(null_neg.vtype(), VarType::Null);

        let single_add = variant_add_const_value(&Variant::from_f32(1.5), 2, "Single add const")
            .expect("Single add const");
        assert_double(&single_add, 3.5);
    }

    #[test]
    fn coercion_error_stress_rows_cover_empty_null_cverr_and_assignment_timing() {
        assert_eq!(
            runtime_variant_to_numeric_compat(&Variant::from_string("   "), "blank numeric"),
            Ok(0.0)
        );

        let empty_add =
            variant_add_values(&Variant::empty(), &Variant::from_i32(5)).expect("Empty + Long");
        assert_eq!(empty_add.as_i32(), Some(5));

        let null_div = variant_div_values(&Variant::null(), &Variant::from_i32(0))
            .expect("Null division should evaluate before divide-by-zero fault")
            .expect("Null division should propagate Null");
        assert_eq!(null_div.vtype(), VarType::Null);

        let cverr_add = variant_add_values(&Variant::from_error_code(13), &Variant::from_i32(1));
        assert!(
            cverr_add
                .expect_err("CVErr arithmetic should report type mismatch")
                .contains("CVErr")
        );

        let null_eq = typed_compare_variants(
            &Variant::null(),
            &Variant::null(),
            StringCompareMode::Binary,
            |ord| ord == std::cmp::Ordering::Equal,
        )
        .expect("Null comparison should be deterministic");
        assert!(!null_eq);

        let let_object = validate_runtime_assignment_variant(
            &Variant::from_i32(1),
            RuntimeAssignmentIntent::Let,
            RuntimeAssignmentTargetKind::Object,
            "target",
            "Object",
        );
        assert!(
            let_object
                .expect_err("Let assignment into Object should fail")
                .contains("Let cannot assign to Object")
        );
    }

    #[test]
    fn object_identity_is_distinguishes_objects_and_nothing() {
        let object = ObjectRef::from_compat_identity(41);
        let first = Variant::from_object_ref(object.clone());
        let same = Variant::from_object_ref(object);
        let different = Variant::from_object_ref(ObjectRef::from_compat_identity(42));
        let nothing = Variant::from_i32(0);

        assert!(runtime_object_identity_is(&first, &same));
        assert!(!runtime_object_identity_is(&first, &different));
        assert!(!runtime_object_identity_is(&first, &nothing));
        assert!(runtime_object_identity_is(&nothing, &Variant::empty()));
    }
}
