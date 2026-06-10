//! `NativeImplId` — the complete enumeration of the VBA base library / built-in
//! surface whose bodies are native (the "DLL side"). Each variant names one
//! library function; `oxvba-lib` provides the body, `oxvba-vm2` dispatches it.
//!
//! This list is a *total* partition of the genuine library functions in the
//! legacy `Instruction` set (the structural primitives — arrays, with-events,
//! pointers, object identity — stay as core `Op`s in `isa.rs`; they are not
//! library surface). COM member invocation and `Declare Lib` are not here: they
//! are `NativeCallee::ComDispatch` / `NativeCallee::Declare` (see `isa.rs`),
//! because their target is data-driven, not a fixed built-in.
//!
//! The grouping mirrors the real `VBA` type library's modules.

/// A single natively-implemented VBA library function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeImplId {
    // ── Strings ──────────────────────────────────────────────
    Len,
    Left,
    Right,
    Mid,
    MidStmt, // `Mid(s, i, n) = ...` statement form
    InStr,
    InStrRev,
    LCase,
    UCase,
    Split,
    Join,
    Replace,
    Trim,
    LTrim,
    RTrim,
    StrComp,
    Like,
    Chr,
    Asc,
    Space,
    StringRepeat, // `String(n, ch)`
    StrReverse,
    StrConv,
    Format,

    // ── Math ─────────────────────────────────────────────────
    Abs,
    Int,
    Fix,
    Sgn,
    Round,
    Sqr,
    Sin,
    Cos,
    Log,
    Exp,
    Atn,
    Tan,

    // ── DateTime ─────────────────────────────────────────────
    DateSerial,
    TimeSerial,
    DateValue,
    TimeValue,
    DateAdd,
    DateDiff,
    Year,
    Month,
    Day,
    Weekday,
    MonthName,
    DateNow, // `Date`
    TimeNow, // `Time`
    Now,
    Timer,

    // ── Conversion ───────────────────────────────────────────
    Hex,
    Oct,
    CStr,
    Str,
    Val,
    CDate,
    CVErr,
    // Numeric / type conversions (`CDbl`/`CLng`/… — coerce the argument to the named
    // type with VBA banker's rounding + overflow). `CDec` is not yet supported.
    CBool,
    CByte,
    CInt,
    CLng,
    CLngLng,
    CLngPtr,
    CSng,
    CDbl,
    CCur,
    CVar,

    // ── Random ───────────────────────────────────────────────
    Rnd,
    Randomize,

    // ── Financial ────────────────────────────────────────────
    Fv,
    Pv,
    Pmt,
    Npv,
    Irr,
    Mirr,
    Rate,
    NPer,

    // ── Information ──────────────────────────────────────────
    IsArray,
    VarType,
    TypeName,
    IsNumeric,
    IsError,
    IsDate,
    IsObject,
    IsNull,
    IsEmpty,
    IIf,    // `IIf(cond, t, f)` — eager (both arms evaluated)
    Choose, // `Choose(idx, v1, …)` — 1-based, eager
    Switch, // `Switch(c1, v1, c2, v2, …)` — eager

    // ── Collection ───────────────────────────────────────────
    CollectionAdd,
    CollectionItem,
    CollectionRemove,
    CollectionCount,

    // ── File / Console I/O ───────────────────────────────────
    FreeFile,
    FileOpen,
    FileClose,
    FileKill,
    FileRead,
    FileWrite,
    FilePrint,
    ConsolePrint,
    FileInput,
    ConsoleInput,
    FileLineInput,
    ConsoleLineInput,
    FileEof,
    FileLof,
    FileSeek,
    FileLoc,
    FilePut,     // `Put #n, [rec], value` — write a record
    FileGetInto, // `Get #n, [rec], var` — read a record (lowered as an assignment)
    FileWidth,   // `Width #n, width`
    FileRename,  // `Name old As new`
    FileLock,    // `Lock #n [, range]`
    FileUnlock,  // `Unlock #n [, range]`

    // ── Interaction / host ───────────────────────────────────
    MsgBox,
    InputBox,
    Beep,
    DoEvents,
    Shell,
    Environ,
    Dir,
    CreateObject,
    ComSubscribeEvent,
    ComUnsubscribeEvent,
    ComEventCallbackSubscription,
    ComEventCallbackArg,
    ComReleaseEventCallback,

    // ── Diagnostics ──────────────────────────────────────────
    DebugPrint,
}

/// The `VBA`-typelib-style module a library function belongs to. Provenance for
/// the descriptor; also drives host-sensitivity (Interaction / File I/O are
/// host-sensitive, the rest deterministic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LibraryModule {
    Strings,
    Math,
    DateTime,
    Conversion,
    Random,
    Financial,
    Information,
    Collection,
    FileIo,
    Interaction,
    Diagnostics,
}

impl NativeImplId {
    /// The owning library module (provenance + host-sensitivity grouping).
    pub fn module(self) -> LibraryModule {
        use LibraryModule as M;
        use NativeImplId::*;
        match self {
            Len | Left | Right | Mid | MidStmt | InStr | InStrRev | LCase | UCase | Split
            | Join | Replace | Trim | LTrim | RTrim | StrComp | Like | Chr | Asc | Space
            | StringRepeat | StrReverse | StrConv | Format => M::Strings,
            Abs | Int | Fix | Sgn | Round | Sqr | Sin | Cos | Log | Exp | Atn | Tan => M::Math,
            DateSerial | TimeSerial | DateValue | TimeValue | DateAdd | DateDiff | Year | Month
            | Day | Weekday | MonthName | DateNow | TimeNow | Now | Timer => M::DateTime,
            Hex | Oct | CStr | Str | Val | CDate | CVErr | CBool | CByte | CInt | CLng
            | CLngLng | CLngPtr | CSng | CDbl | CCur | CVar => M::Conversion,
            Rnd | Randomize => M::Random,
            Fv | Pv | Pmt | Npv | Irr | Mirr | Rate | NPer => M::Financial,
            IsArray | VarType | TypeName | IsNumeric | IsError | IsDate | IsObject | IsNull
            | IsEmpty | IIf | Choose | Switch => M::Information,
            CollectionAdd | CollectionItem | CollectionRemove | CollectionCount => M::Collection,
            FreeFile | FileOpen | FileClose | FileKill | FileRead | FileWrite | FilePrint
            | ConsolePrint | FileInput | ConsoleInput | FileLineInput | ConsoleLineInput
            | FileEof | FileLof | FileSeek | FileLoc | FilePut | FileGetInto | FileWidth
            | FileRename | FileLock | FileUnlock => M::FileIo,
            MsgBox
            | InputBox
            | Beep
            | DoEvents
            | Shell
            | Environ
            | Dir
            | CreateObject
            | ComSubscribeEvent
            | ComUnsubscribeEvent
            | ComEventCallbackSubscription
            | ComEventCallbackArg
            | ComReleaseEventCallback => M::Interaction,
            DebugPrint => M::Diagnostics,
        }
    }

    /// True when the body must reach host services (UI, filesystem, process,
    /// time, COM activation) rather than being a pure computation.
    pub fn is_host_sensitive(self) -> bool {
        matches!(
            self.module(),
            LibraryModule::FileIo | LibraryModule::Interaction | LibraryModule::Diagnostics
        ) || matches!(
            self,
            NativeImplId::DateNow | NativeImplId::TimeNow | NativeImplId::Now | NativeImplId::Timer
        )
    }
}
