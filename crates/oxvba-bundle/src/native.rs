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

    /// The synthetic `VBA`-bundle location `(module, member)` this id is exported
    /// under, or `None` for an id that does **not** route through the bundle.
    ///
    /// This is the single source of truth shared by the bundle's export tokens
    /// (`oxvba-bundle/vba_library`) and the binder's `ExternMember` resolution
    /// (`oxvba-symbol/providers/vba_library`), so a cross-bundle import is linked to
    /// its export by construction (case-insensitive name match). The member name
    /// mirrors the primary (first) user-facing name in the `oxvba-symbol` intrinsic
    /// catalog (e.g. `StringRepeat` → `"String"`, `DateNow` → `"Date"`, `Fv` →
    /// `"FV"`), and the module name is the [`LibraryModule`] the id belongs to; both
    /// live here because the bundle cannot depend on the catalog.
    ///
    /// `Some` exactly for the **migrated** ids:
    /// - the whole `Strings`, `Math`, `DateTime`, `Conversion`, `Random`,
    ///   `Financial` modules — minus their name-less members (`MidStmt`, the
    ///   `Mid(…) = …` statement form, and `Like`, the operator, which are not
    ///   ordinary by-name library functions);
    /// - the `Information` **predicate** functions (`IsArray`/`VarType`/`TypeName`/
    ///   `IsNumeric`/`IsError`/`IsDate`/`IsObject`/`IsNull`/`IsEmpty`/`IsMissing`) —
    ///   ordinary by-name functions; their `Information`-module siblings `IIf`,
    ///   `Choose`, `Switch` are **special forms** (eager but with dedicated binder
    ///   lowering, resolved by `special_form`, not `name_to_intrinsic`) and stay
    ///   `None`;
    /// - the `Interaction` **host** functions (`MsgBox`/`InputBox`/`Beep`/`DoEvents`/
    ///   `Shell`/`Environ`/`Dir`) — ordinary by-name functions that reach host
    ///   services (the native body already receives the host via `invoke_native_lib`,
    ///   so rerouting changes only the dispatch route, not behaviour). Their
    ///   `Interaction`-module siblings `CreateObject` (object activation / `New`
    ///   lowering target) and the `Com*` event-machinery ids (not user-callable by
    ///   name) stay `None`.
    ///
    /// `None` for every other id: those `Information`/`Interaction` exceptions above,
    /// all `FileIo` (a later slice), `Diagnostics` (`Debug.Print`/`Debug.Assert`,
    /// internal), and the `Collection` members (a class, not a free function). Those
    /// keep the bespoke `DispatchRoute::Native(id)` route.
    pub fn library_member(self) -> Option<(&'static str, &'static str)> {
        use NativeImplId::*;
        // The owning module name for the migrated id; `None` for any module/id that
        // does not route through the bundle. `Information` and `Interaction` are
        // partially migrated (predicates / host functions only), so the per-id `match`
        // below — not the module alone — decides their membership; the excluded ids of
        // those modules fall through to the final `_ => return None`.
        let member = match self {
            // ── Strings ──
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
            // `MidStmt` (assignment form) and `Like` (operator) are name-less — not
            // bundle members, even though their module is `Strings`.
            MidStmt | Like => return None,
            // ── Math ──
            Abs => "Abs",
            Int => "Int",
            Fix => "Fix",
            Sgn => "Sgn",
            Round => "Round",
            Sqr => "Sqr",
            Sin => "Sin",
            Cos => "Cos",
            Log => "Log",
            Exp => "Exp",
            Atn => "Atn",
            Tan => "Tan",
            // ── DateTime ──
            DateSerial => "DateSerial",
            TimeSerial => "TimeSerial",
            DateValue => "DateValue",
            TimeValue => "TimeValue",
            DateAdd => "DateAdd",
            DateDiff => "DateDiff",
            Year => "Year",
            Month => "Month",
            Day => "Day",
            Weekday => "Weekday",
            Hour => "Hour",
            Minute => "Minute",
            Second => "Second",
            MonthName => "MonthName",
            WeekdayName => "WeekdayName",
            DatePart => "DatePart",
            DateNow => "Date",
            TimeNow => "Time",
            Now => "Now",
            Timer => "Timer",
            // ── Conversion ──
            Hex => "Hex",
            Oct => "Oct",
            CStr => "CStr",
            Str => "Str",
            Val => "Val",
            CDate => "CDate",
            CVErr => "CVErr",
            CBool => "CBool",
            CByte => "CByte",
            CInt => "CInt",
            CLng => "CLng",
            CLngLng => "CLngLng",
            CLngPtr => "CLngPtr",
            CSng => "CSng",
            CDbl => "CDbl",
            CCur => "CCur",
            CVar => "CVar",
            // ── Random ──
            Rnd => "Rnd",
            Randomize => "Randomize",
            // ── Financial ── (catalog primaries are upper-cased)
            Fv => "FV",
            Pv => "PV",
            Pmt => "Pmt",
            Npv => "NPV",
            Irr => "IRR",
            Mirr => "MIRR",
            Rate => "Rate",
            NPer => "NPer",
            // ── Information (predicates only — IIf/Choose/Switch stay special forms) ──
            IsArray => "IsArray",
            VarType => "VarType",
            TypeName => "TypeName",
            IsNumeric => "IsNumeric",
            IsError => "IsError",
            IsDate => "IsDate",
            IsObject => "IsObject",
            IsNull => "IsNull",
            IsEmpty => "IsEmpty",
            IsMissing => "IsMissing",
            // ── Interaction (host functions only) ──
            MsgBox => "MsgBox",
            InputBox => "InputBox",
            Beep => "Beep",
            DoEvents => "DoEvents",
            Shell => "Shell",
            Environ => "Environ",
            Dir => "Dir",
            // Everything else stays on the `Native(id)` route: the name-less
            // `MidStmt`/`Like`; the `Information` special forms `IIf`/`Choose`/`Switch`;
            // the `Interaction` `CreateObject` + `Com*` machinery; all `FileIo`;
            // `Diagnostics` (`DebugPrint`); and the `Collection` members.
            _ => return None,
        };
        // The owning module name. `module()` is the authoritative grouping; the
        // per-id `match` above already excluded the non-migrated ids of the partially
        // migrated `Information`/`Interaction` modules, so this only ever runs for a
        // migrated id (every migrated module maps to a name here).
        let module = match self.module() {
            LibraryModule::Strings => "Strings",
            LibraryModule::Math => "Math",
            LibraryModule::DateTime => "DateTime",
            LibraryModule::Conversion => "Conversion",
            LibraryModule::Random => "Random",
            LibraryModule::Financial => "Financial",
            LibraryModule::Information => "Information",
            LibraryModule::Interaction => "Interaction",
            // `Collection`/`FileIo`/`Diagnostics` have no migrated members, so no id
            // that reaches here belongs to them; if one ever did it would be a bug to
            // surface loudly rather than silently mis-route.
            LibraryModule::Collection | LibraryModule::FileIo | LibraryModule::Diagnostics => {
                unreachable!(
                    "non-migrated module {:?} yielded a member name",
                    self.module()
                )
            }
        };
        Some((module, member))
    }

    /// The informational parameter count recorded on the bundle's
    /// [`ProcedureDescriptor`] for this migrated library function — its
    /// **maximum** arity (the native body reads its arguments positionally and
    /// ignores this; it is only a descriptor field). `0` for any id that is not a
    /// migrated bundle member. Mirrors the catalog's `max_args` (the bundle cannot
    /// depend on the catalog), with `0` standing in for the variadic forms here
    /// (`Filter`/`Split`/etc. cap their own args).
    pub fn library_param_count(self) -> usize {
        use NativeImplId::*;
        match self {
            // ── Strings ──
            Len | LenB | LCase | UCase | Trim | LTrim | RTrim | Chr | Asc | Space | StrReverse => 1,
            Left | Right | Join | StringRepeat => 2,
            Mid | StrComp | StrConv => 3,
            InStr | InStrRev | Split | Format | Filter => 4,
            Replace => 6,
            // ── Math ──
            Abs | Int | Fix | Sgn | Sqr | Sin | Cos | Log | Exp | Atn | Tan => 1,
            Round => 2,
            // ── DateTime ──
            Year | Month | Day | Hour | Minute | Second | DateValue | TimeValue => 1,
            Weekday | MonthName => 2,
            DateSerial | TimeSerial | DateAdd => 3,
            DatePart | WeekdayName => 4,
            DateDiff => 5,
            DateNow | TimeNow | Now | Timer => 0,
            // ── Conversion ── (all single-argument)
            Hex | Oct | CStr | Str | Val | CDate | CVErr | CBool | CByte | CInt | CLng
            | CLngLng | CLngPtr | CSng | CDbl | CCur | CVar => 1,
            // ── Random ──
            Rnd | Randomize => 1,
            // ── Financial ──
            Npv | Irr => 2,
            Mirr => 3,
            Fv | Pv | Pmt | NPer => 5,
            Rate => 6,
            // ── Information predicates ── (all single-argument)
            IsArray | VarType | TypeName | IsNumeric | IsError | IsDate | IsObject | IsNull
            | IsEmpty | IsMissing => 1,
            // ── Interaction host functions ──
            Beep | DoEvents => 0,
            Environ => 1,
            Shell | Dir => 2,
            MsgBox => 5,
            InputBox => 7,
            // Not a migrated bundle member.
            _ => 0,
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
