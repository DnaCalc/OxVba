use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::MutexGuard,
};

use crate::{
    error::{HalError, HalResult},
    model::CapabilityId,
    traits::ConsoleHal,
};
use oxvba_runtime::{F64Value, RuntimeValue, Variant, bstr::BStr};

use super::StandardHostServices;

#[derive(Debug, Default)]
pub(super) struct ConsoleState {
    pub(super) pending_fields: VecDeque<String>,
}

fn parse_console_field(raw: &str) -> RuntimeValue {
    let field = raw.trim();
    if field.eq_ignore_ascii_case("#TRUE#") {
        return RuntimeValue::Bool(true);
    }
    if field.eq_ignore_ascii_case("#FALSE#") {
        return RuntimeValue::Bool(false);
    }
    if field.eq_ignore_ascii_case("#NULL#") {
        return RuntimeValue::Empty;
    }
    if let Ok(value) = field.parse::<i32>() {
        return RuntimeValue::I32(value);
    }
    if let Ok(value) = field.parse::<f64>() {
        return RuntimeValue::F64(F64Value::from_f64(value));
    }
    RuntimeValue::String(BStr::from(field))
}

fn parse_console_field_variant(raw: &str) -> Variant {
    let field = raw.trim();
    if field.eq_ignore_ascii_case("#TRUE#") {
        return Variant::from_bool(true);
    }
    if field.eq_ignore_ascii_case("#FALSE#") {
        return Variant::from_bool(false);
    }
    if field.eq_ignore_ascii_case("#NULL#") {
        return Variant::null();
    }
    if let Ok(value) = field.parse::<i32>() {
        return Variant::from_i32(value);
    }
    if let Ok(value) = field.parse::<f64>() {
        return Variant::from_f64(value);
    }
    Variant::from_string(field)
}

fn split_console_fields(line: &str) -> VecDeque<String> {
    if line.is_empty() {
        return VecDeque::from([String::new()]);
    }
    line.split(',')
        .map(|field| field.trim().to_string())
        .collect::<VecDeque<_>>()
}

impl StandardHostServices {
    fn console_lock(&self, op: &'static str) -> HalResult<MutexGuard<'_, ConsoleState>> {
        self.console_state.lock().map_err(|_| {
            HalError::adapter_fault(
                self.profile,
                CapabilityId::ConsoleIo,
                op,
                "console state lock poisoned",
            )
        })
    }

    fn console_read_line(&self, op: &'static str) -> HalResult<String> {
        if let Some(callbacks) = self.callbacks.as_ref()
            && let Some(line) = callbacks.on_console_input_line()
        {
            return Ok(line);
        }
        if !self.policy.allow_interaction {
            return Err(self.denied(CapabilityId::ConsoleIo, op));
        }
        if !self.native_console_enabled() {
            return Err(self.denied(CapabilityId::ConsoleIo, op));
        }
        let mut line = String::new();
        io::stdin().read_line(&mut line).map_err(|err| {
            HalError::adapter_fault(
                self.profile,
                CapabilityId::ConsoleIo,
                op,
                format!("failed to read stdin: {err}"),
            )
        })?;
        Ok(line.trim_end_matches(['\r', '\n']).to_string())
    }
}

impl ConsoleHal for StandardHostServices {
    fn print_line(&self, data: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ConsoleIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "print_line"));
        }
        let text = self.runtime_value_to_display_text(&data);
        if let Some(callbacks) = self.callbacks.as_ref()
            && callbacks.on_console_print(&text)
        {
            return Ok(RuntimeValue::I32(0));
        }
        if !self.native_console_enabled() {
            return Err(self.denied(capability, "print_line"));
        }
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "{text}").map_err(|err| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "print_line",
                format!("failed to write stdout: {err}"),
            )
        })?;
        stdout.flush().map_err(|err| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "print_line",
                format!("failed to flush stdout: {err}"),
            )
        })?;
        Ok(RuntimeValue::I32(0))
    }

    fn print_line_variant(&self, data: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::ConsoleIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "print_line"));
        }
        let text = self.variant_to_display_text(&data);
        if let Some(callbacks) = self.callbacks.as_ref()
            && callbacks.on_console_print(&text)
        {
            return Ok(Variant::from_i32(0));
        }
        if !self.native_console_enabled() {
            return Err(self.denied(capability, "print_line"));
        }
        let mut stdout = io::stdout().lock();
        writeln!(stdout, "{text}").map_err(|err| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "print_line",
                format!("failed to write stdout: {err}"),
            )
        })?;
        stdout.flush().map_err(|err| {
            HalError::adapter_fault(
                self.profile,
                capability,
                "print_line",
                format!("failed to flush stdout: {err}"),
            )
        })?;
        Ok(Variant::from_i32(0))
    }

    fn input_fields(&self, count: RuntimeValue) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ConsoleIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "input_fields"));
        }
        let count = self.runtime_value_project_compat_slot_i32(
            &count,
            capability,
            "input_fields",
            "count",
        )?;
        let count = count.max(1) as usize;
        let mut state = self.console_lock("input_fields")?;
        while state.pending_fields.len() < count {
            let line = self.console_read_line("input_fields")?;
            state.pending_fields.extend(split_console_fields(&line));
        }
        let mut fields = Vec::new();
        while fields.len() < count {
            fields.push(state.pending_fields.pop_front().unwrap_or_default());
        }
        if count == 1 {
            return Ok(parse_console_field(
                fields.first().map(String::as_str).unwrap_or(""),
            ));
        }
        Ok(RuntimeValue::String(BStr::from(fields.join(","))))
    }

    fn input_fields_variant(&self, count: Variant) -> HalResult<Variant> {
        let capability = CapabilityId::ConsoleIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "input_fields"));
        }
        let count =
            self.variant_project_compat_slot_i32(&count, capability, "input_fields", "count")?;
        let count = count.max(1) as usize;
        let mut state = self.console_lock("input_fields")?;
        while state.pending_fields.len() < count {
            let line = self.console_read_line("input_fields")?;
            state.pending_fields.extend(split_console_fields(&line));
        }
        let mut fields = Vec::new();
        while fields.len() < count {
            fields.push(state.pending_fields.pop_front().unwrap_or_default());
        }
        if count == 1 {
            return Ok(parse_console_field_variant(
                fields.first().map(String::as_str).unwrap_or(""),
            ));
        }
        Ok(Variant::from_string(fields.join(",")))
    }

    fn line_input(&self) -> HalResult<RuntimeValue> {
        let capability = CapabilityId::ConsoleIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "line_input"));
        }
        let line = self.console_read_line("line_input")?;
        Ok(RuntimeValue::String(BStr::from(line)))
    }

    fn line_input_variant(&self) -> HalResult<Variant> {
        let capability = CapabilityId::ConsoleIo;
        if !self.supports(capability) {
            return Err(self.unsupported(capability, "line_input"));
        }
        let line = self.console_read_line("line_input")?;
        Ok(Variant::from_string(line))
    }
}
