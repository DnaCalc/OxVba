//! Host-sensitive base-library bodies: file/console I/O, interaction, process,
//! time, COM activation and events, diagnostics. Each delegates to an
//! `oxvba_hal` `HostServices` facet; `HalError` maps to `LibError` via `?`.
//!
//! The COM event-callback subsystem (subscribe/unsubscribe/callback) converts
//! Variants to the `oxvba-com` token types and calls the `com()` facet, ported
//! from the legacy VM's `semantics::variant_to_com_*` helpers.

use crate::{LibError, LibResult, as_f64, as_i32, as_str, need, opt, vunit};
use oxvba_com::{ComCallbackToken, ComMemberToken, ComSubscriptionToken};
use oxvba_hal::HostServices;
use oxvba_runtime::Variant;
use oxvba_runtime::object_ref::ObjectRef;
use oxvba_runtime::variant::VarType;

fn req(args: &[Variant], index: usize) -> LibResult<Variant> {
    Ok(need(args, index)?.clone())
}

fn arg_or_empty(args: &[Variant], index: usize) -> Variant {
    opt(args, index).cloned().unwrap_or_else(vunit)
}

fn string_arg(args: &[Variant], index: usize) -> LibResult<Variant> {
    Ok(Variant::from_string(as_str(need(args, index)?)?))
}

fn optional_string_arg(args: &[Variant], index: usize) -> LibResult<Variant> {
    match opt(args, index) {
        Some(value) if matches!(value.vtype(), VarType::Empty) => Ok(Variant::empty()),
        Some(value) => Ok(Variant::from_string(as_str(value)?)),
        None => Ok(Variant::empty()),
    }
}

// ── Time / locale ──
pub fn date_now(host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.time_locale().date_serial_now_variant()?)
}
pub fn time_now(host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.time_locale().time_serial_now_variant()?)
}
pub fn now(host: &dyn HostServices) -> LibResult<Variant> {
    let date = host.time_locale().date_serial_now_variant()?;
    let time = host.time_locale().time_serial_now_variant()?;
    Ok(Variant::from_date_f64(as_f64(&date)? + as_f64(&time)?))
}
pub fn timer(host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.time_locale().timer_ticks_variant()?)
}

// ── File / console I/O ──
pub fn free_file(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().free_file_variant(arg_or_empty(args, 0))?)
}
pub fn file_open(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    // args: path, packed mode ((file_number << 16) | mode_code), record length.
    Ok(host
        .fs()
        .open_with_record_len(req(args, 0)?, req(args, 1)?, arg_or_empty(args, 2))?)
}
pub fn file_close(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().close_variant(req(args, 0)?)?)
}
pub fn file_kill(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().kill_variant(req(args, 0)?)?)
}
pub fn file_mkdir(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().mkdir_variant(req(args, 0)?)?)
}
pub fn file_rmdir(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().rmdir_variant(req(args, 0)?)?)
}
pub fn file_cur_dir(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host
        .fs()
        .cur_dir_variant(opt(args, 0).cloned().unwrap_or_else(Variant::empty))?)
}
pub fn file_ch_dir(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().ch_dir_variant(req(args, 0)?)?)
}
pub fn file_len(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().file_len_variant(req(args, 0)?)?)
}
pub fn file_copy(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().file_copy_variant(req(args, 0)?, req(args, 1)?)?)
}
pub fn file_get_attr(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().get_attr_variant(req(args, 0)?)?)
}
pub fn file_set_attr(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().set_attr_variant(req(args, 0)?, req(args, 1)?)?)
}
pub fn file_ch_drive(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().ch_drive_variant(req(args, 0)?)?)
}
pub fn file_date_time(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().file_date_time_variant(req(args, 0)?)?)
}
pub fn file_read(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().read_bytes_variant(req(args, 0)?, req(args, 1)?)?)
}
/// `Print #handle, item0 <sep> item1 …`. The binder passes `args[0]` = handle,
/// `args[1]` = the separator spec (one char per field — the separator that
/// *follows* that field: `,` print-zone / `;` adjacent / `n` none), and
/// `args[2..]` = the field values. We assemble the complete record here (so no
/// field is dropped) and write it verbatim through the file sink.
pub fn file_print(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    let handle = req(args, 0)?;
    let start_col = as_i32(&host.fs().print_column_variant(handle.clone())?)?.max(0) as usize;
    let width = as_i32(&host.fs().print_width_variant(handle.clone())?)?.max(0) as usize;
    let record = assemble_print_record_with_column(
        &separator_spec(args),
        &print_item_spec(args),
        args.get(3..).unwrap_or(&[]),
        start_col,
        width,
    )?;
    Ok(host
        .fs()
        .print_line_variant(handle, Variant::from_string(record))?)
}

/// `Write #handle, item0, item1 …`. Same arg shape as [`file_print`]; `Write #`
/// always comma-delimits and formats each field machine-readably (strings quoted,
/// `#TRUE#`/`#FALSE#`/`#NULL#`/`#ERROR n#`), so the source `;`/`,` separators only
/// decide whether the trailing `\r\n` is suppressed.
pub fn file_write(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    let handle = req(args, 0)?;
    let record = assemble_write_record(&separator_spec(args), args.get(2..).unwrap_or(&[]));
    Ok(host
        .fs()
        .print_line_variant(handle, Variant::from_string(record))?)
}

/// VBA `Print #` print-zone width: a `,` separator advances to the next 14-column
/// zone boundary.
const PRINT_ZONE_WIDTH: usize = 14;

/// The per-field separator spec the binder packs into `args[1]` (one char per
/// field). Empty when there are no fields.
fn separator_spec(args: &[Variant]) -> String {
    opt(args, 1)
        .and_then(|value| value.as_bstr())
        .map(|spec| spec.as_str().to_string())
        .unwrap_or_default()
}

fn print_item_spec(args: &[Variant]) -> String {
    opt(args, 2)
        .and_then(|value| value.as_bstr())
        .map(|spec| spec.as_str().to_string())
        .unwrap_or_default()
}

/// Whether a trailing `,`/`;` after the last item suppresses the record's `\r\n`
/// terminator (so the next `Print`/`Write` continues the same line).
fn trailing_separator_suppresses_newline(seps: &str) -> bool {
    matches!(seps.chars().last(), Some(',') | Some(';'))
}

fn is_print_numeric(value: &Variant) -> bool {
    matches!(
        value.vtype(),
        VarType::Integer
            | VarType::Long
            | VarType::LongLong
            | VarType::SignedByte
            | VarType::Byte
            | VarType::UnsignedInteger
            | VarType::UnsignedLong
            | VarType::UnsignedInt
            | VarType::UnsignedLongLong
            | VarType::Single
            | VarType::Double
            | VarType::Decimal
            | VarType::Currency
    )
}

fn print_field_text(value: &Variant) -> String {
    let text = oxvba_runtime::print_display_text(value);
    if !is_print_numeric(value) {
        return text;
    }
    if text.starts_with('-') {
        format!("{text} ")
    } else {
        format!(" {text} ")
    }
}

fn non_negative_count(value: &Variant, name: &str) -> LibResult<usize> {
    let count = as_i32(value)?;
    if count < 0 {
        return Err(LibError::invalid_call(format!(
            "{name} requires a non-negative count"
        )));
    }
    Ok(count as usize)
}

fn active_width(width: usize) -> Option<usize> {
    if width == 0 { None } else { Some(width) }
}

fn append_raw_spaces(out: &mut String, col: &mut usize, count: usize) {
    for _ in 0..count {
        out.push(' ');
        *col = col.saturating_add(1);
    }
}

fn append_line_break(out: &mut String, col: &mut usize) {
    out.push_str("\r\n");
    *col = 0;
}

fn append_spaces(out: &mut String, col: &mut usize, count: usize, width: Option<usize>) {
    let Some(width) = width else {
        append_raw_spaces(out, col, count);
        return;
    };
    let target_col = col.saturating_add(count) % width;
    if target_col < *col {
        append_line_break(out, col);
    }
    append_raw_spaces(out, col, target_col.saturating_sub(*col));
}

fn append_tab(
    out: &mut String,
    col: &mut usize,
    value: &Variant,
    width: Option<usize>,
) -> LibResult<()> {
    let target = as_i32(value)?;
    if target < 1 {
        return Err(LibError::invalid_call("Tab requires a positive column"));
    }
    let mut target_col = (target - 1) as usize;
    if let Some(width) = width {
        target_col %= width;
    }
    if *col > target_col {
        append_line_break(out, col);
    }
    append_raw_spaces(out, col, target_col.saturating_sub(*col));
    Ok(())
}

fn append_next_print_zone(out: &mut String, col: &mut usize, width: Option<usize>) {
    let next_zone = ((*col / PRINT_ZONE_WIDTH) + 1) * PRINT_ZONE_WIDTH;
    if width.is_some_and(|width| next_zone >= width) {
        append_line_break(out, col);
        return;
    }
    append_raw_spaces(out, col, next_zone.saturating_sub(*col));
}

fn append_print_text(out: &mut String, col: &mut usize, text: &str, width: Option<usize>) {
    let text_width = text.chars().count();
    if let Some(width) = width {
        if *col > 0 && col.saturating_add(text_width) > width {
            append_line_break(out, col);
        }
    }
    *col = col.saturating_add(text_width);
    out.push_str(text);
}

#[cfg(test)]
fn default_print_item_spec(field_count: usize) -> String {
    std::iter::repeat('v').take(field_count).collect()
}

/// Assemble a `Print #` record: each field rendered with VBA display semantics,
/// `;` placing the next field adjacently, `,` padding to the next print zone, and
/// `Spc`/`Tab` print-clause controls moving the current output column.
#[cfg(test)]
fn assemble_print_record(seps: &str, fields: &[Variant]) -> String {
    let kinds = default_print_item_spec(fields.len());
    assemble_print_record_with_column(seps, &kinds, fields, 0, 0).unwrap_or_default()
}

fn assemble_print_record_with_column(
    seps: &str,
    kinds: &str,
    fields: &[Variant],
    start_col: usize,
    width: usize,
) -> LibResult<String> {
    let seps: Vec<char> = seps.chars().collect();
    let kinds: Vec<char> = kinds.chars().collect();
    let mut out = String::new();
    let mut col = start_col;
    let width = active_width(width);
    for (index, field) in fields.iter().enumerate() {
        match kinds.get(index).copied().unwrap_or('v') {
            's' => {
                let count = non_negative_count(field, "Spc")?;
                append_spaces(&mut out, &mut col, count, width);
            }
            't' => append_tab(&mut out, &mut col, field, width)?,
            'z' => append_next_print_zone(&mut out, &mut col, width),
            _ => {
                let text = print_field_text(field);
                append_print_text(&mut out, &mut col, &text, width);
            }
        }
        if let Some(',') = seps.get(index).copied() {
            let target = ((col / PRINT_ZONE_WIDTH) + 1) * PRINT_ZONE_WIDTH;
            if width.is_some_and(|width| target >= width) {
                append_line_break(&mut out, &mut col);
            } else {
                let count = target.saturating_sub(col);
                append_raw_spaces(&mut out, &mut col, count);
            }
        }
    }
    if !matches!(seps.last().copied(), Some(',') | Some(';')) {
        out.push_str("\r\n");
    }
    Ok(out)
}

/// Assemble a `Write #` record: comma-delimited machine-readable fields.
fn assemble_write_record(seps: &str, fields: &[Variant]) -> String {
    let mut out = String::new();
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&oxvba_runtime::write_display_text(field));
    }
    if !trailing_separator_suppresses_newline(seps) {
        out.push_str("\r\n");
    }
    out
}
pub fn console_print(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.console().print_line_variant(print_joined(args))?)
}
pub fn file_input(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host
        .fs()
        .input_fields_variant(req(args, 0)?, req(args, 1)?)?)
}
pub fn console_input(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.console().input_fields_variant(req(args, 0)?)?)
}
pub fn file_line_input(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().line_input_variant(req(args, 0)?)?)
}
pub fn console_line_input(host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.console().line_input_variant()?)
}
pub fn file_eof(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().eof_variant(req(args, 0)?)?)
}
pub fn file_lof(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().lof_variant(req(args, 0)?)?)
}
pub fn file_seek(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host
        .fs()
        .seek_variant(req(args, 0)?, arg_or_empty(args, 1))?)
}
pub fn file_loc(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().loc_variant(req(args, 0)?)?)
}
pub fn file_put(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    // args: handle, record-number (or empty), value, fixed-length-string flag.
    Ok(host.fs().put_record_variant(
        req(args, 0)?,
        arg_or_empty(args, 1),
        req(args, 2)?,
        arg_or_empty(args, 3),
    )?)
}
pub fn file_get_into(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    // args: handle, record-number (or empty), target VBA type code, string-length spec.
    Ok(host.fs().get_record_variant(
        req(args, 0)?,
        arg_or_empty(args, 1),
        req(args, 2)?,
        arg_or_empty(args, 3),
    )?)
}
pub fn file_width(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host
        .fs()
        .width_variant(req(args, 0)?, arg_or_empty(args, 1))?)
}
pub fn file_rename(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().name_variant(req(args, 0)?, req(args, 1)?)?)
}
pub fn file_lock(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host
        .fs()
        .lock_variant(req(args, 0)?, arg_or_empty(args, 1), arg_or_empty(args, 2))?)
}
pub fn file_unlock(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host
        .fs()
        .unlock_variant(req(args, 0)?, arg_or_empty(args, 1), arg_or_empty(args, 2))?)
}

// ── Interaction / process ──
pub fn msg_box(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host
        .ui()
        .msg_box_variant(req(args, 0)?, arg_or_empty(args, 1))?)
}
pub fn input_box(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host
        .ui()
        .input_box_variant(req(args, 0)?, arg_or_empty(args, 1))?)
}
pub fn beep(_host: &dyn HostServices) -> LibResult<Variant> {
    // FIDELITY: no HAL beep facet yet; a no-op until one is added.
    Ok(vunit())
}
pub fn do_events(host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.events().do_events_variant()?)
}
pub fn shell(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host
        .process()
        .shell_variant(req(args, 0)?, arg_or_empty(args, 1))?)
}
pub fn command(host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.process().command_variant()?)
}
pub fn get_setting(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.process().get_setting_variant(
        string_arg(args, 0)?,
        string_arg(args, 1)?,
        string_arg(args, 2)?,
        optional_string_arg(args, 3)?,
    )?)
}
pub fn get_all_settings(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host
        .process()
        .get_all_settings_variant(string_arg(args, 0)?, string_arg(args, 1)?)?)
}
pub fn save_setting(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.process().save_setting_variant(
        string_arg(args, 0)?,
        string_arg(args, 1)?,
        string_arg(args, 2)?,
        string_arg(args, 3)?,
    )?)
}
pub fn delete_setting(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.process().delete_setting_variant(
        string_arg(args, 0)?,
        string_arg(args, 1)?,
        optional_string_arg(args, 2)?,
    )?)
}
pub fn environ(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.process().environ_variant(req(args, 0)?)?)
}
pub fn dir(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host
        .process()
        .dir_variant(arg_or_empty(args, 0), arg_or_empty(args, 1))?)
}

// ── COM ──
pub fn create_object(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.com().create_object_variant(req(args, 0)?)?)
}

/// `GetObject([pathname], [class])`. The mode (running instance / new instance / file
/// bind) is decided by the argument shape, which the HAL resolves: an omitted `pathname`
/// arrives as `Empty`, distinguishing it from a present `""`.
pub fn get_object(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host
        .com()
        .get_object_variant(arg_or_empty(args, 0), arg_or_empty(args, 1))?)
}

// Variant → typed-handle conversions, ported from the legacy VM
// (`semantics::variant_to_com_*`): COM handles/tokens are `i32` newtypes that a
// Variant may carry as an object ref, an `i32`, or a range-checked `i64`.
fn to_object(value: &Variant, field: &str) -> LibResult<ObjectRef> {
    if let Some(object) = value.as_object_ref() {
        return Ok(object);
    }
    let raw = to_i32_handle(value, field)?;
    Ok(ObjectRef::from_compat_identity(raw))
}

fn to_i32_handle(value: &Variant, field: &str) -> LibResult<i32> {
    if let Some(raw) = value.as_i32() {
        return Ok(raw);
    }
    if let Some(raw) = value.as_i64() {
        return i32::try_from(raw)
            .map_err(|_| LibError::type_mismatch(format!("{field} exceeds i32 handle range")));
    }
    Err(LibError::type_mismatch(format!(
        "{field} requires an integer handle"
    )))
}

fn to_index(value: &Variant) -> LibResult<usize> {
    if matches!(value.vtype(), VarType::Empty) {
        return Ok(0);
    }
    let raw = to_i32_handle(value, "callback index")?;
    usize::try_from(raw).map_err(|_| LibError::invalid_call("negative callback index"))
}

pub fn com_subscribe_event(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    let object = to_object(need(args, 0)?, "event source")?;
    let event = ComMemberToken::new(to_i32_handle(need(args, 1)?, "event token")?);
    let token = host.com().subscribe_event(object, event)?;
    Ok(Variant::from_i32(token.raw()))
}
pub fn com_unsubscribe_event(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    let subscription = ComSubscriptionToken::new(to_i32_handle(need(args, 0)?, "subscription")?);
    Ok(host.com().unsubscribe_event_variant(subscription)?)
}
pub fn com_event_callback_subscription(
    args: &[Variant],
    host: &dyn HostServices,
) -> LibResult<Variant> {
    let callback = ComCallbackToken::new(to_i32_handle(need(args, 0)?, "callback")?);
    let token = host.com().event_callback_subscription(callback)?;
    Ok(Variant::from_i32(token.raw()))
}
pub fn com_event_callback_arg(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    let callback = ComCallbackToken::new(to_i32_handle(need(args, 0)?, "callback")?);
    let index = to_index(need(args, 1)?)?;
    Ok(host.com().event_callback_variant(callback, index)?)
}
pub fn com_release_event_callback(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    let callback = ComCallbackToken::new(to_i32_handle(need(args, 0)?, "callback")?);
    Ok(host.com().release_event_callback_variant(callback)?)
}

// ── Diagnostics ──
pub fn debug_print(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.diag().debug_print_variant(print_joined(args))?)
}

/// Join `Print`/`Debug.Print` arguments into the single display string the host
/// emits. VBA's `,` separator advances to the next print zone; we render it as a
/// tab (the `;` no-space separator and `Tab()`/`Spc()` positioning are a deferred
/// fidelity refinement — see POST_CLEANUP.md). An empty arg list prints a blank
/// line.
fn print_joined(args: &[Variant]) -> Variant {
    let text = args
        .iter()
        .map(oxvba_runtime::print_display_text)
        .collect::<Vec<_>>()
        .join("\t");
    Variant::from_string(text)
}

#[cfg(test)]
mod tests {
    use super::{assemble_print_record, assemble_print_record_with_column, assemble_write_record};
    use oxvba_runtime::Variant;

    #[test]
    fn print_record_emits_every_field_adjacent_for_semicolons() {
        let fields = [
            Variant::from_string("a"),
            Variant::from_string("b"),
            Variant::from_string("c"),
        ];
        assert_eq!(assemble_print_record(";;n", &fields), "abc\r\n");
    }

    #[test]
    fn print_record_comma_pads_to_next_14_column_zone() {
        let fields = [Variant::from_string("a"), Variant::from_string("b")];
        // "a" (1 col) + comma → pad to column 14 (13 spaces) → "b" → terminator.
        assert_eq!(
            assemble_print_record(",n", &fields),
            format!("a{}b\r\n", " ".repeat(13))
        );
    }

    #[test]
    fn print_record_trailing_separator_suppresses_the_newline() {
        let fields = [Variant::from_string("a")];
        assert_eq!(assemble_print_record(";", &fields), "a");
    }

    #[test]
    fn print_record_with_no_fields_is_a_blank_line() {
        assert_eq!(assemble_print_record("", &[]), "\r\n");
    }

    #[test]
    fn print_record_pads_numeric_fields_with_sign_and_trailing_space() {
        let fields = [
            Variant::from_i32(1),
            Variant::from_i32(-2),
            Variant::from_i32(0),
        ];
        assert_eq!(assemble_print_record(";;n", &fields), " 1 -2  0 \r\n");
    }

    #[test]
    fn print_record_honors_spc_tab_and_bare_tab_controls() {
        let fields = [
            Variant::from_string("A"),
            Variant::from_i32(3),
            Variant::from_string("B"),
            Variant::from_i32(10),
            Variant::from_string("C"),
            Variant::from_i32(0),
            Variant::from_string("D"),
        ];
        let record =
            assemble_print_record_with_column(";;;;;;n", "vsvtvzv", &fields, 0, 0).expect("record");
        assert_eq!(
            record,
            format!(
                "A{}B{}C{}D\r\n",
                " ".repeat(3),
                " ".repeat(4),
                " ".repeat(4)
            )
        );
    }

    #[test]
    fn print_record_starts_comma_zones_from_existing_column() {
        let fields = [Variant::from_string("e"), Variant::from_string("z")];
        let record = assemble_print_record_with_column(",n", "vv", &fields, 4, 0).expect("record");
        assert_eq!(record, format!("e{}z\r\n", " ".repeat(9)));
    }

    #[test]
    fn print_record_rejects_nonpositive_tab_column() {
        let fields = [Variant::from_i32(0)];
        assert!(assemble_print_record_with_column("n", "t", &fields, 0, 0).is_err());
    }

    #[test]
    fn print_record_width_wraps_before_next_overflowing_value() {
        let fields = [
            Variant::from_string("ab"),
            Variant::from_string("cd"),
            Variant::from_string("ef"),
        ];
        assert_eq!(
            assemble_print_record_with_column(";;n", "vvv", &fields, 0, 5).expect("record"),
            "abcd\r\nef\r\n"
        );

        let long_field = [Variant::from_string("abcdef")];
        assert_eq!(
            assemble_print_record_with_column("n", "v", &long_field, 0, 5).expect("record"),
            "abcdef\r\n"
        );
    }

    #[test]
    fn print_record_width_wraps_numeric_fields_and_comma_zones() {
        let numeric = [Variant::from_i32(12), Variant::from_i32(34)];
        assert_eq!(
            assemble_print_record_with_column(";n", "vv", &numeric, 0, 5).expect("record"),
            " 12 \r\n 34 \r\n"
        );

        let comma = [Variant::from_string("a"), Variant::from_string("b")];
        assert_eq!(
            assemble_print_record_with_column(",n", "vv", &comma, 0, 10).expect("record"),
            "a\r\nb\r\n"
        );
    }

    #[test]
    fn print_record_width_wraps_print_controls_like_vba() {
        let fields = [
            Variant::from_string("A"),
            Variant::from_i32(3),
            Variant::from_string("B"),
            Variant::from_i32(3),
            Variant::from_string("C"),
            Variant::from_i32(0),
            Variant::from_string("D"),
        ];
        assert_eq!(
            assemble_print_record_with_column(";;;;;;n", "vsvtvzv", &fields, 0, 5).expect("record"),
            "A   B\r\n  C\r\nD\r\n"
        );

        let spc_long = [Variant::from_i32(6), Variant::from_string("A")];
        assert_eq!(
            assemble_print_record_with_column(";n", "sv", &spc_long, 0, 5).expect("record"),
            " A\r\n"
        );

        let tab_far = [Variant::from_i32(10), Variant::from_string("A")];
        assert_eq!(
            assemble_print_record_with_column(";n", "tv", &tab_far, 0, 5).expect("record"),
            "    A\r\n"
        );
    }

    #[test]
    fn write_record_quotes_strings_and_marks_boolean() {
        let fields = [
            Variant::from_string("a"),
            Variant::from_i32(1),
            Variant::from_bool(true),
        ];
        assert_eq!(assemble_write_record(",,n", &fields), "\"a\",1,#TRUE#\r\n");
    }

    #[test]
    fn write_record_doubles_embedded_quotes() {
        let fields = [Variant::from_string("x\"y")];
        assert_eq!(assemble_write_record("n", &fields), "\"x\"\"y\"\r\n");
    }

    #[test]
    fn write_record_empty_field_is_blank_and_joins_with_commas() {
        // `Write #` emits nothing for an Empty field (just the delimiting comma).
        let fields = [Variant::empty(), Variant::from_string("z")];
        assert_eq!(assemble_write_record(",n", &fields), ",\"z\"\r\n");
    }
}
