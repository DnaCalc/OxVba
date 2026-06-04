//! Host-sensitive base-library bodies: file/console I/O, interaction, process,
//! time, COM activation, diagnostics. Each delegates to an `oxvba_hal`
//! `HostServices` facet; `HalError` maps to `LibError` via `?`.
//!
//! FIDELITY: the COM event-callback subsystem (subscribe/unsubscribe/callback)
//! is typed in terms of `oxvba-com` tokens and is a first-cut gap here — it
//! returns a clear error until those token types are wired through.

use crate::{LibError, LibResult, as_f64, need, opt, vunit};
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
    Ok(host.fs().open_variant(req(args, 0)?, req(args, 1)?)?)
}
pub fn file_close(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().close_variant(req(args, 0)?)?)
}
pub fn file_kill(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().kill_variant(req(args, 0)?)?)
}
pub fn file_read(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().read_bytes_variant(req(args, 0)?, req(args, 1)?)?)
}
pub fn file_write(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().write_bytes_variant(req(args, 0)?, req(args, 1)?)?)
}
pub fn file_print(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().print_line_variant(req(args, 0)?, req(args, 1)?)?)
}
pub fn console_print(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.console().print_line_variant(req(args, 0)?)?)
}
pub fn file_input(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().input_fields_variant(req(args, 0)?, req(args, 1)?)?)
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
    Ok(host.fs().seek_variant(req(args, 0)?, arg_or_empty(args, 1))?)
}
pub fn file_loc(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.fs().loc_variant(req(args, 0)?)?)
}

// ── Interaction / process ──
pub fn msg_box(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.ui().msg_box_variant(req(args, 0)?, arg_or_empty(args, 1))?)
}
pub fn input_box(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.ui().input_box_variant(req(args, 0)?, arg_or_empty(args, 1))?)
}
pub fn beep(_host: &dyn HostServices) -> LibResult<Variant> {
    // FIDELITY: no HAL beep facet yet; a no-op until one is added.
    Ok(vunit())
}
pub fn do_events(host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.events().do_events_variant()?)
}
pub fn shell(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.process().shell_variant(req(args, 0)?, arg_or_empty(args, 1))?)
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
    Err(LibError::type_mismatch(format!("{field} requires an integer handle")))
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
pub fn com_release_event_callback(
    args: &[Variant],
    host: &dyn HostServices,
) -> LibResult<Variant> {
    let callback = ComCallbackToken::new(to_i32_handle(need(args, 0)?, "callback")?);
    Ok(host.com().release_event_callback_variant(callback)?)
}

// ── Diagnostics ──
pub fn debug_print(args: &[Variant], host: &dyn HostServices) -> LibResult<Variant> {
    Ok(host.diag().debug_print_variant(req(args, 0)?)?)
}
