//! `oxvba-lib` — the native bodies of the VBA base library.
//!
//! Every `oxvba_bundle::NativeImplId` is dispatched here to a Rust body, copied
//! out of the legacy VM's intrinsic logic. Pure functions compute over
//! `oxvba_runtime::Variant`; host-sensitive functions delegate to the
//! `oxvba_hal::HostServices` facets. The interpreter calls [`invoke`] for every
//! native built-in call (COM dispatch and `Declare` are handled by the VM via the
//! host directly, not here).
//!
//! Completeness is structural: [`invoke`] is an exhaustive `match` over
//! `NativeImplId`, so a missing built-in is a compile error. The remaining
//! `// FIDELITY:` markers are features absent from the legacy VM as well: keyed
//! `Collection` access (awaits the vm2 object model) and `StrConv`'s
//! CJK/encoding modes.

mod format;
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
        Self {
            code,
            message: message.into(),
        }
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
        // Preserve the rich VBA `Err.Number` the HAL recovered from the host (e.g.
        // 429/432 for a failed COM activation, or a Declare host code); only fall
        // back to the generic invalid-call (5) when the HAL gave no host code.
        // (The COM *dispatch* path reaches the VM via `Fault::from_hal` instead,
        // which already threads `host_error_code`.)
        match err.host_error_code {
            Some(code) => LibError::new(code, err.message),
            None => LibError::invalid_call(format!("{err:?}")),
        }
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
        // VBA's `Rnd` is deterministic until `Randomize`; the 24-bit LCG starts
        // from this fixed default seed.
        Self {
            rng_state: 0x0005_0000,
        }
    }
}

// ── Argument / value helpers (shared by the family modules) ──────────────────

pub(crate) fn need(args: &[Variant], index: usize) -> LibResult<&Variant> {
    args.get(index)
        .ok_or_else(|| LibError::invalid_call(format!("missing argument {index}")))
}

pub(crate) fn opt(args: &[Variant], index: usize) -> Option<&Variant> {
    args.get(index)
}

/// Read any numeric Variant as `f64`. Reads `LongLong`/`Single`/`Currency`
/// directly (the `coerce_to` table has no path for them) and routes the rest
/// (`Integer`/`Long`/`Byte`/`Boolean`/`Date`/`Empty`) through `Double`.
pub(crate) fn as_f64(value: &Variant) -> LibResult<f64> {
    if let Some(v) = value.as_f64() {
        return Ok(v); // Double
    }
    if let Some(v) = value.as_f32() {
        return Ok(v as f64); // Single
    }
    if let Some(v) = value.as_i64() {
        return Ok(v as f64); // LongLong
    }
    if let Some(v) = value.as_currency_scaled_i64() {
        return Ok(v as f64 / 10_000.0); // Currency (fixed 4-dp scale)
    }
    coerce_to(value, VarType::Double)?
        .as_f64()
        .ok_or_else(|| LibError::type_mismatch("expected a numeric value"))
}

/// Read any numeric Variant as `i64`, using VBA's banker's rounding for
/// fractional values. (`coerce_to` has no `LongLong`/`Double→Long` path.)
pub(crate) fn as_i64(value: &Variant) -> LibResult<i64> {
    if let Some(v) = value.as_i64() {
        return Ok(v); // LongLong
    }
    if let Some(v) = value.as_i32() {
        return Ok(i64::from(v)); // Long
    }
    let d = as_f64(value)?;
    if !d.is_finite() || d.abs() >= 9.223_372_036_854_775e18 {
        return Err(LibError::overflow("integer overflow"));
    }
    Ok(d.round_ties_even() as i64)
}

pub(crate) fn as_i32(value: &Variant) -> LibResult<i32> {
    let v = as_i64(value)?;
    i32::try_from(v).map_err(|_| LibError::overflow("value does not fit in Long"))
}

pub(crate) fn as_usize(value: &Variant) -> LibResult<usize> {
    let v = as_i64(value)?;
    usize::try_from(v).map_err(|_| LibError::invalid_call("expected a non-negative count"))
}

/// Read a guest-controlled **allocation** count (`String`/`Space`/… lengths) as a
/// `usize`, bounded to VBA's `Long` range. VBA passes these counts as `Long`, so a
/// value outside `0..=2^31-1` is invalid: a negative is "Invalid procedure call" (5),
/// and an out-of-`Long` magnitude is "Overflow" (6). This prevents a garbage/huge
/// count from driving an unbounded host allocation that would abort the process —
/// guest code must never be able to crash the VM.
pub(crate) fn alloc_count(value: &Variant) -> LibResult<usize> {
    let v = as_i64(value)?;
    if v < 0 {
        return Err(LibError::invalid_call(format!(
            "negative allocation count {v}"
        )));
    }
    if v > i64::from(i32::MAX) {
        return Err(LibError::overflow(format!(
            "allocation count {v} exceeds Long range"
        )));
    }
    Ok(v as usize)
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
/// discarded — a true `Empty` Variant.
pub(crate) fn vunit() -> Variant {
    Variant::empty()
}

/// Dispatch a base-library built-in to its native body. Exhaustive over
/// `NativeImplId` — adding a variant without a body is a compile error.
/// The value-returning string functions that propagate `Null` → `Null` (a `Null` argument
/// yields a `Null` result). Excludes `Format` (returns "" for `Null`), the array-returning
/// `Split`/`Join`/`Filter`, the conversion functions (which have their own `Null` rules), and
/// the `Mid` statement form.
fn string_fn_propagates_null(id: NativeImplId) -> bool {
    use NativeImplId::*;
    matches!(
        id,
        Len | LenB
            | Left
            | Right
            | Mid
            | LCase
            | UCase
            | Trim
            | LTrim
            | RTrim
            | StrReverse
            | Space
            | StringRepeat
            | Chr
            | ChrW
            | Asc
            | AscW
            | InStr
            | InStrRev
            | Replace
            | StrComp
            | StrConv
            | Like
    )
}

pub fn invoke(
    id: NativeImplId,
    args: &[Variant],
    host: &dyn HostServices,
    ctx: &mut LibContext,
) -> LibResult<Variant> {
    use NativeImplId::*;
    // VBA propagates `Null` through the value-returning string functions: if any argument is
    // `Null`, the result is `Null` (these otherwise reach `as_str` → `variant_to_vba_string`,
    // which raises Type mismatch 13 on `Null`).
    //
    // FIDELITY: the `$`-suffixed forms (`Left$`, `UCase$`, …) raise error 94 ("Invalid use of
    // Null") instead, since a `String` cannot hold `Null` — but the binder resolves `Left` and
    // `Left$` to the same `NativeImplId`, so the suffix is not visible here. Until it is
    // threaded, both forms return `Null`. (See the builtin-library split note.)
    if string_fn_propagates_null(id) && args.iter().any(|a| a.vtype() == Vt::Null) {
        return Ok(Variant::null());
    }
    match id {
        // ── Strings ──
        Len => pure::len(args),
        LenB => pure::len_b(args),
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
        ChrW => pure::chr_w(args),
        AscW => pure::asc_w(args),
        Space => pure::space(args),
        StringRepeat => pure::string_repeat(args),
        StrReverse => pure::str_reverse(args),
        StrConv => pure::str_conv(args),
        Format => pure::format(args),
        Filter => pure::filter(args),

        // ── Math ──
        // Abs/Int/Fix preserve the argument's numeric subtype (Abs promotes on
        // overflow); Sgn always returns Integer. The transcendentals below stay
        // `math1` (Double).
        Abs => pure::abs(args),
        Int => pure::int_floor(args),
        Fix => pure::fix_trunc(args),
        Sgn => pure::sgn(args),
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
        Hour => pure::date_part(args, pure::DatePart::Hour),
        Minute => pure::date_part(args, pure::DatePart::Minute),
        Second => pure::date_part(args, pure::DatePart::Second),
        MonthName => pure::month_name(args),
        WeekdayName => pure::weekday_name(args),
        DatePart => pure::vba_datepart(args),
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
        CDbl => pure::cdbl(args),
        CSng => pure::csng(args),
        CInt => pure::cint(args),
        CLng => pure::clng(args),
        CLngLng | CLngPtr => pure::clnglng(args),
        CByte => pure::cbyte(args),
        CBool => pure::cbool(args),
        CCur => pure::ccur(args),
        CVar => pure::cvar(args),

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
        IPmt => pure::ipmt(args),
        PPmt => pure::ppmt(args),
        Sln => pure::sln(args),
        Syd => pure::syd(args),
        Ddb => pure::ddb(args),

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
        IsMissing => pure::is_missing(args),
        IIf => pure::iif(args),
        Choose => pure::choose(args),
        Switch => pure::switch(args),
        Rgb => pure::rgb(args),
        QbColor => pure::qb_color(args),

        // ── File / Console I/O ──
        FreeFile => host::free_file(args, host),
        FileOpen => host::file_open(args, host),
        FileClose => host::file_close(args, host),
        FileKill => host::file_kill(args, host),
        FileMkDir => host::file_mkdir(args, host),
        FileRmDir => host::file_rmdir(args, host),
        FileCurDir => host::file_cur_dir(args, host),
        FileChDir => host::file_ch_dir(args, host),
        FileLen => host::file_len(args, host),
        FileCopy => host::file_copy(args, host),
        FileGetAttr => host::file_get_attr(args, host),
        FileSetAttr => host::file_set_attr(args, host),
        FileChDrive => host::file_ch_drive(args, host),
        FileDateTime => host::file_date_time(args, host),
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
        FilePut => host::file_put(args, host),
        FileGetInto => host::file_get_into(args, host),
        FileWidth => host::file_width(args, host),
        FileRename => host::file_rename(args, host),
        FileLock => host::file_lock(args, host),
        FileUnlock => host::file_unlock(args, host),

        // ── Interaction / host ──
        MsgBox => host::msg_box(args, host),
        InputBox => host::input_box(args, host),
        Beep => host::beep(host),
        DoEvents => host::do_events(host),
        Shell => host::shell(args, host),
        Environ => host::environ(args, host),
        Dir => host::dir(args, host),
        CreateObject => host::create_object(args, host),
        GetObject => host::get_object(args, host),
        ComSubscribeEvent => host::com_subscribe_event(args, host),
        ComUnsubscribeEvent => host::com_unsubscribe_event(args, host),
        ComEventCallbackSubscription => host::com_event_callback_subscription(args, host),
        ComEventCallbackArg => host::com_event_callback_arg(args, host),
        ComReleaseEventCallback => host::com_release_event_callback(args, host),

        // ── Diagnostics ──
        DebugPrint => host::debug_print(args, host),
    }
}
