//! Explicit COM compatibility adapters for scalar slot-token projections.

use crate::ComValue;

pub fn com_value_from_runtime_token(value: i32) -> ComValue {
    ComValue::I32(value)
}

pub fn com_value_to_runtime_token(value: &ComValue) -> Result<i32, String> {
    match value {
        ComValue::Empty | ComValue::Null => Ok(0),
        ComValue::ErrorCode(code) => Ok(*code),
        ComValue::Bool(value) => Ok(i32::from(*value)),
        ComValue::I32(value) => Ok(*value),
        ComValue::I64(value) => i32::try_from(*value)
            .map_err(|_| format!("COM i64 value {value} is outside i32 token range")),
        ComValue::U64(value) => i32::try_from(*value)
            .map_err(|_| format!("COM u64 value {value} is outside i32 token range")),
        ComValue::F64(value) => Ok(value.as_f64() as i32),
        ComValue::Decimal(value) => value
            .to_string()
            .parse::<i32>()
            .map_err(|err| format!("COM decimal value cannot be represented as i32: {err}")),
        ComValue::Currency(value) => Ok((value.scaled_i64() / 10_000) as i32),
        ComValue::String(value) => value
            .as_str()
            .trim()
            .parse::<i32>()
            .map_err(|err| format!("COM string value cannot be represented as i32: {err}")),
        ComValue::ArrayIntent(array) => Ok(array.len().min(i32::MAX as usize) as i32),
        ComValue::Object(object) => Ok(object.raw()),
    }
}
