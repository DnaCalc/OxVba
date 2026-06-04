//! `oxvba-lib` — the native bodies of the VBA base library.
//!
//! Every `oxvba_bundle::NativeImplId` is dispatched here to a Rust body, copied
//! out of the legacy VM's intrinsic logic. Pure functions compute over
//! `oxvba_runtime::Variant`; host-sensitive functions delegate to the
//! `oxvba_hal::HostServices` facets. `oxvba-vm2` calls [`invoke`] for every
//! `Op::CallNative { callee: Builtin(..) }` (COM dispatch and `Declare` are
//! handled by the VM via the host directly, not here).
//!
//! Completeness is structural: [`invoke`] is an exhaustive `match` over
//! `NativeImplId`, so a missing built-in is a compile error. A handful of the
//! richest bodies (Format, IRR/Rate solvers, full date arithmetic, Collection)
//! are first-cut and marked `// FIDELITY:` for refinement against the reference.

mod host;
mod pure;

use oxvba_bundle::NativeImplId;
use oxvba_hal::{HalError, HostServices};
use oxvba_runtime::variant::VarType as Vt;
use oxvba_runtime::{Variant, coerce::coerce_to, variant::VarType, variant_to_vba_string};

/// A VBA run-time error raised by a library body (number + message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibError {
    pub code: i32,
    pub message: String,
}

pub type LibResult<T> = Result<T, LibError>;

impl LibError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
    /// Run-time error 13 — Type mismatch.
    pub fn type_mismatch(message: impl Into<String>) -> Self {
        Self::new(13, message)
    }
    /// Run-time error 6 — Overflow.
    pub fn overflow(message: impl Into<String>) -> Self {
        Self::new(6, message)
    }
    /// Run-time error 5 — Invalid procedure call or argument.
    pub fn invalid_call(message: impl Into<String>) -> Self {
        Self::new(5, message)
    }
}

impl From<String> for LibError {
    fn from(message: String) -> Self {
        LibError::type_mismatch(message)
    }
}

impl From<HalError> for LibError {
    fn from(err: HalError) -> Self {
        // Host failures surface as a generic invalid-call until the HAL error
        // taxonomy is mapped onto VBA run-time codes.
        LibError::invalid_call(format!("{err:?}"))
    }
}

/// Mutable per-execution library state (e.g. the `Rnd` generator). Owned by the
/// VM and threaded through every [`invoke`]; the host (read-only facets) is
/// passed separately.
#[derive(Debug, Clone)]
pub struct LibContext {
    /// `Rnd`/`Randomize` LCG state.
    pub rng_state: u64,
}

impl Default for LibContext {
    fn default() -> Self {
        // VBA's default Rnd seed produces a fixed sequence; this LCG seed mirrors
        // "deterministic unless Randomize" behavior.
        Self { rng_state: 0x2545_F491_4F6C_DD1D }
    }
}

// ── Argument / value helpers (shared by the family modules) ──────────────────

pub(crate) fn need<'a>(args: &'a [Variant], index: usize) -> LibResult<&'a Variant> {
    args.get(index)
        .ok_or_else(|| LibError::invalid_call(format!("missing argument {index}")))
}

pub(crate) fn opt(args: &[Variant], index: usize) -> Option<&Variant> {
    args.get(index)
}

pub(crate) fn as_f64(value: &Variant) -> LibResult<f64> {
    coerce_to(value, VarType::Double)?
        .as_f64()
        .ok_or_else(|| LibError::type_mismatch("expected a numeric value"))
}

pub(crate) fn as_i64(value: &Variant) -> LibResult<i64> {
    coerce_to(value, VarType::LongLong)?
        .as_i64()
        .ok_or_else(|| LibError::type_mismatch("expected an integer value"))
}

pub(crate) fn as_i32(value: &Variant) -> LibResult<i32> {
    let v = as_i64(value)?;
    i32::try_from(v).map_err(|_| LibError::overflow("value does not fit in Long"))
}

pub(crate) fn as_usize(value: &Variant) -> LibResult<usize> {
    let v = as_i64(value)?;
    usize::try_from(v).map_err(|_| LibError::invalid_call("expected a non-negative count"))
}

pub(crate) fn as_str(value: &Variant) -> LibResult<String> {
    Ok(variant_to_vba_string(value)?.as_str())
}

pub(crate) fn vstr(text: impl Into<String>) -> Variant {
    Variant::from_string(text.into())
}
pub(crate) fn vf64(value: f64) -> Variant {
    Variant::from_f64(value)
}
pub(crate) fn vi32(value: i32) -> Variant {
    Variant::from_i32(value)
}
pub(crate) fn vbool(value: bool) -> Variant {
    Variant::from_bool(value)
}
/// The empty/void return for statement-form library calls whose result is
/// discarded. FIDELITY: should be a true `Empty` Variant once exposed.
pub(crate) fn vunit() -> Variant {
    Variant::from_i32(0)
}

/// Dispatch a base-library built-in to its native body. Exhaustive over
/// `NativeImplId` — adding a variant without a body is a compile error.
pub fn invoke(
    id: NativeImplId,
    args: &[Variant],
    host: &dyn HostServices,
    ctx: &mut LibContext,
) -> LibResult<Variant> {
    use NativeImplId::*;
    match id {
        // ── Strings ──
        Len => pure::len(args),
        Left => pure::left(args),
        Right => pure::right(args),
        Mid => pure::mid(args),
        MidStmt => pure::mid_stmt(args),
        InStr => pure::instr(args, false),
        InStrRev => pure::instr(args, true),
        LCase => pure::lcase(args),
        UCase => pure::ucase(args),
        Split => pure::split(args),
        Join => pure::join(args),
        Replace => pure::replace(args),
        Trim => pure::trim(args, true, true),
        LTrim => pure::trim(args, true, false),
        RTrim => pure::trim(args, false, true),
        StrComp => pure::str_comp(args),
        Like => pure::like(args),
        Chr => pure::chr(args),
        Asc => pure::asc(args),
        Space => pure::space(args),
        StringRepeat => pure::string_repeat(args),
        StrReverse => pure::str_reverse(args),
        StrConv => pure::str_conv(args),
        Format => pure::format(args),

        // ── Math ──
        Abs => pure::math1(args, f64::abs),
        Int => pure::math1(args, f64::floor),
        Fix => pure::math1(args, f64::trunc),
        Sgn => pure::math1(args, |x| x.signum() * (x != 0.0) as i64 as f64),
        Round => pure::round(args),
        Sqr => pure::math1(args, f64::sqrt),
        Sin => pure::math1(args, f64::sin),
        Cos => pure::math1(args, f64::cos),
        Log => pure::math1(args, f64::ln),
        Exp => pure::math1(args, f64::exp),
        Atn => pure::math1(args, f64::atan),
        Tan => pure::math1(args, f64::tan),

        // ── DateTime ──
        DateSerial => pure::date_serial(args),
        TimeSerial => pure::time_serial(args),
        DateValue => pure::date_value(args),
        TimeValue => pure::time_value(args),
        DateAdd => pure::date_add(args),
        DateDiff => pure::date_diff(args),
        Year => pure::date_part(args, pure::DatePart::Year),
        Month => pure::date_part(args, pure::DatePart::Month),
        Day => pure::date_part(args, pure::DatePart::Day),
        Weekday => pure::date_part(args, pure::DatePart::Weekday),
        MonthName => pure::month_name(args),
        DateNow => host::date_now(host),
        TimeNow => host::time_now(host),
        Now => host::now(host),
        Timer => host::timer(host),

        // ── Conversion ──
        Hex => pure::hex(args),
        Oct => pure::oct(args),
        CStr => pure::cstr(args),
        Str => pure::str_fn(args),
        Val => pure::val(args),
        CDate => pure::cdate(args),
        CVErr => pure::cverr(args),

        // ── Random ──
        Rnd => pure::rnd(args, ctx),
        Randomize => pure::randomize(args, ctx),

        // ── Financial ──
        Fv => pure::fv(args),
        Pv => pure::pv(args),
        Pmt => pure::pmt(args),
        Npv => pure::npv(args),
        Irr => pure::irr(args),
        Mirr => pure::mirr(args),
        Rate => pure::rate(args),
        NPer => pure::nper(args),

        // ── Information ──
        IsArray => pure::is_vtype(args, |t| matches!(t, Vt::ArrayVariant)),
        VarType => pure::var_type(args),
        TypeName => pure::type_name(args),
        IsNumeric => pure::is_numeric(args),
        IsError => pure::is_vtype(args, |t| matches!(t, Vt::Error)),
        IsDate => pure::is_date(args),
        IsObject => pure::is_vtype(args, |t| matches!(t, Vt::Object)),
        IsNull => pure::is_vtype(args, |t| matches!(t, Vt::Null)),
        IsEmpty => pure::is_vtype(args, |t| matches!(t, Vt::Empty)),

        // ── Collection (FIDELITY: SafeArray-backed first-cut; no keyed access) ──
        CollectionAdd => pure::collection_add(args),
        CollectionItem => pure::collection_item(args),
        CollectionRemove => pure::collection_remove(args),
        CollectionCount => pure::collection_count(args),

        // ── File / Console I/O ──
        FreeFile => host::free_file(args, host),
        FileOpen => host::file_open(args, host),
        FileClose => host::file_close(args, host),
        FileKill => host::file_kill(args, host),
        FileRead => host::file_read(args, host),
        FileWrite => host::file_write(args, host),
        FilePrint => host::file_print(args, host),
        ConsolePrint => host::console_print(args, host),
        FileInput => host::file_input(args, host),
        ConsoleInput => host::console_input(args, host),
        FileLineInput => host::file_line_input(args, host),
        ConsoleLineInput => host::console_line_input(host),
        FileEof => host::file_eof(args, host),
        FileLof => host::file_lof(args, host),
        FileSeek => host::file_seek(args, host),
        FileLoc => host::file_loc(args, host),

        // ── Interaction / host ──
        MsgBox => host::msg_box(args, host),
        InputBox => host::input_box(args, host),
        Beep => host::beep(host),
        DoEvents => host::do_events(host),
        Shell => host::shell(args, host),
        Environ => host::environ(args, host),
        Dir => host::dir(args, host),
        CreateObject => host::create_object(args, host),
        ComSubscribeEvent => host::com_subscribe_event(args, host),
        ComUnsubscribeEvent => host::com_unsubscribe_event(args, host),
        ComEventCallbackSubscription => host::com_event_callback_subscription(args, host),
        ComEventCallbackArg => host::com_event_callback_arg(args, host),
        ComReleaseEventCallback => host::com_release_event_callback(args, host),

        // ── Diagnostics ──
        DebugPrint => host::debug_print(args, host),
    }
}
