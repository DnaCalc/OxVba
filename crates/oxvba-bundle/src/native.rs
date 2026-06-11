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

/// Declares the [`NativeImplId`] enum and [`NativeImplId::ALL`] from a single
/// variant list, so the set of native ops and the slice enumerating them are one
/// source of truth and cannot drift apart.
macro_rules! native_impl_ids {
    ( $( $(#[$attr:meta])* $variant:ident ),+ $(,)? ) => {
        /// A single natively-implemented VBA library function.
        ///
        /// Generated together with [`NativeImplId::ALL`] (see [`native_impl_ids!`]).
        /// The exhaustive `intrinsic_entry` match in `oxvba-symbol` and the dispatch
        /// match in `oxvba-lib` then make each variant's name/signature/body a
        /// compile-time obligation — there is no hand-maintained roster to keep in sync.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum NativeImplId {
            $( $(#[$attr])* $variant, )+
        }

        impl NativeImplId {
            /// Every variant, in declaration order — generated alongside the enum,
            /// so adding a variant extends this slice automatically.
            pub const ALL: &'static [NativeImplId] = &[ $( NativeImplId::$variant, )+ ];
        }
    };
}

native_impl_ids! {
    // ── Strings ──────────────────────────────────────────────
    Len,
    LenB,
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
    Filter, // `Filter(arr, match, [include], [compare])`

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
    Hour,
    Minute,
    Second,
    MonthName,
    WeekdayName,
    DatePart, // `DatePart(interval, date, …)`
    DateNow,  // `Date`
    TimeNow,  // `Time`
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
    IsMissing, // `IsMissing(optionalArg)` — True for an omitted optional Variant
    IIf,       // `IIf(cond, t, f)` — eager (both arms evaluated)
    Choose,    // `Choose(idx, v1, …)` — 1-based, eager
    Switch,    // `Switch(c1, v1, c2, v2, …)` — eager

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
    FileMkDir,
    FileRmDir,
    FileCurDir, // `CurDir([drive])`
    FileChDir,  // `ChDir path`
    FileLen,    // `FileLen(path)` — file size
    FileCopy,    // `FileCopy source, dest`
    FileGetAttr, // `GetAttr(path)` — file attribute bits (vbReadOnly|vbHidden|…)
    FileSetAttr, // `SetAttr path, attributes`
    FileChDrive,  // `ChDrive drive`
    FileDateTime, // `FileDateTime(path)` — last-modified as a Date serial
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
            Len | LenB | Left | Right | Mid | MidStmt | InStr | InStrRev | LCase | UCase
            | Split | Join | Replace | Trim | LTrim | RTrim | StrComp | Like | Chr | Asc
            | Space | StringRepeat | StrReverse | StrConv | Format | Filter => M::Strings,
            Abs | Int | Fix | Sgn | Round | Sqr | Sin | Cos | Log | Exp | Atn | Tan => M::Math,
            DateSerial | TimeSerial | DateValue | TimeValue | DateAdd | DateDiff | Year | Month
            | Day | Weekday | Hour | Minute | Second | MonthName | WeekdayName | DatePart
            | DateNow | TimeNow | Now | Timer => M::DateTime,
            Hex | Oct | CStr | Str | Val | CDate | CVErr | CBool | CByte | CInt | CLng
            | CLngLng | CLngPtr | CSng | CDbl | CCur | CVar => M::Conversion,
            Rnd | Randomize => M::Random,
            Fv | Pv | Pmt | Npv | Irr | Mirr | Rate | NPer => M::Financial,
            IsArray | VarType | TypeName | IsNumeric | IsError | IsDate | IsObject | IsNull
            | IsEmpty | IsMissing | IIf | Choose | Switch => M::Information,
            CollectionAdd | CollectionItem | CollectionRemove | CollectionCount => M::Collection,
            FreeFile | FileOpen | FileClose | FileKill | FileMkDir | FileRmDir | FileCurDir
            | FileChDir | FileLen | FileCopy | FileGetAttr | FileSetAttr | FileChDrive
            | FileDateTime | FileRead | FileWrite | FilePrint | ConsolePrint | FileInput
            | ConsoleInput | FileLineInput | ConsoleLineInput | FileEof | FileLof | FileSeek
            | FileLoc | FilePut | FileGetInto | FileWidth | FileRename | FileLock | FileUnlock => {
                M::FileIo
            }
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

    /// The canonical `Strings`-module member name this id is exported under in the
    /// synthetic `VBA` library bundle (e.g. `StringRepeat` → `"String"`, `LenB` →
    /// `"LenB"`), or `None` for an id outside [`LibraryModule::Strings`]. This is
    /// the single source of truth shared by the bundle's export tokens and the
    /// binder's `ExternMember` resolution, so the two match exactly (an import is
    /// linked to its export by case-insensitive name). It mirrors the primary
    /// (first) user-facing name in the `oxvba-symbol` intrinsic catalog, but lives
    /// here because the bundle cannot depend on the catalog.
    ///
    /// `MidStmt` (the `Mid(…) = …` statement form) and `Like` (the operator) are
    /// intentionally `None`: neither is an ordinary library function resolvable by
    /// bare name, so neither routes through the `VBA` bundle.
    pub fn strings_member_name(self) -> Option<&'static str> {
        use NativeImplId::*;
        Some(match self {
            Len => "Len",
            LenB => "LenB",
            Left => "Left",
            Right => "Right",
            Mid => "Mid",
            InStr => "InStr",
            InStrRev => "InStrRev",
            LCase => "LCase",
            UCase => "UCase",
            Split => "Split",
            Join => "Join",
            Replace => "Replace",
            Trim => "Trim",
            LTrim => "LTrim",
            RTrim => "RTrim",
            StrComp => "StrComp",
            Chr => "Chr",
            Asc => "Asc",
            Space => "Space",
            StringRepeat => "String",
            StrReverse => "StrReverse",
            StrConv => "StrConv",
            Format => "Format",
            Filter => "Filter",
            // `MidStmt` is the assignment-statement form (not a value function) and
            // `Like` is an operator (empty catalog name) — neither is a bundle
            // member. Every other `Strings` id is listed above.
            MidStmt | Like => return None,
            _ => return None,
        })
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

/// The native body of a [`ProcedureDescriptor`](crate::ProcedureDescriptor):
/// either a pure/host **library function** ([`NativeImplId`], run through
/// `oxvba-lib` with no VM access) or a **built-in object method**
/// ([`NativeMethodId`], run inside the VM with `&mut self` access). The two are
/// dispatched differently by the VM — a `Library` body is invoked exactly like
/// `Op::CallNative { NativeCallee::Builtin(..) }` (its arguments arrive already
/// assembled, no frame is pushed), whereas a `Method` body runs via the VM's
/// `run_native_method` so it can mutate VM-held instance state.
///
/// This is what lets every ordinary built-in surface — both library functions
/// (Strings/Math/…) and built-in classes (`Collection`, …) — appear as members
/// of the synthetic `VBA` library bundle and dispatch through the ordinary
/// cross-bundle machinery, instead of via a bespoke `CoreCallee::Native` route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeBody {
    /// A pure/host library function (`oxvba-lib`); the VM runs it with no frame,
    /// exactly as `Op::CallNative` runs a `NativeCallee::Builtin`.
    Library(NativeImplId),
    /// A built-in object's method, run inside the VM (`&mut self`) so it can reach
    /// VM-held instance state.
    Method(NativeMethodId),
}

/// The body of a built-in object's method that is implemented as native VM code
/// rather than bytecode. A `NativeMethodId` body runs inside the VM with
/// `&mut self` access — needed for built-in objects (`Collection`, …) whose
/// methods mutate VM-held instance state. A
/// [`ProcedureDescriptor`](crate::ProcedureDescriptor) carrying
/// `Some(NativeBody::Method(..))` is invoked by the VM directly via
/// `run_native_method` instead of pushing a bytecode frame; this is how the
/// synthetic `VBA` library bundle's classes get native method bodies while
/// dispatching through the ordinary class machinery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeMethodId {
    /// `Collection.Add(item, [key], [before], [after])`.
    CollectionAdd,
    /// `Collection.Item(indexOrKey)` (also the default member).
    CollectionItem,
    /// `Collection.Count`.
    CollectionCount,
    /// `Collection.Remove(indexOrKey)`.
    CollectionRemove,
}
