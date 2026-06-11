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
    ///   name) stay `None`;
    /// - the `FileIo` by-**name** members, exported under the canonical VBA typelib
    ///   module **`FileSystem`** (member = catalog primary). These split into the
    ///   catalog-`Ordinary` FUNCTION forms (`FreeFile`/`CurDir`/`FileLen`/`GetAttr`/
    ///   `FileDateTime`/`EOF`/`LOF`/`Seek`/`Loc`) and the by-name `FileStatement` forms
    ///   that are *not lexer keywords* (`Kill`/`MkDir`/`RmDir`/`ChDir`/`ChDrive`/
    ///   `SetAttr`/`FileCopy`), which parse as ordinary statement-calls and resolve via
    ///   `name_to_intrinsic` just like a function — the binder never consults the
    ///   catalog `CallShape`, so both kinds migrate identically. Like `Interaction`,
    ///   they reach host filesystem services through `invoke_native_lib`, so rerouting
    ///   changes only the dispatch route, not behaviour. Their name-LESS `FileIo`
    ///   STATEMENT-form siblings (`FileOpen`/`FileClose`/`Print #`/`Put`/`Get`/`Name`/
    ///   `Lock`/… — parsed to dedicated CST nodes and lowered in `oxvba-bind/stmt.rs`,
    ///   not resolved by name) live in [`NativeImplId::library_statement_member`]
    ///   instead (they have no catalog name for the primary-name drift-guard).
    ///
    /// `None` for every other id: the `Information`/`Interaction` exceptions above, the
    /// special forms `IIf`/`Choose`/`Switch`/`CreateObject`, the name-LESS `FileIo`
    /// statement forms (see `library_statement_member`) and the name-less `FileRead`,
    /// `Diagnostics` (`Debug.Print`/`Debug.Assert`, internal), and the `Collection`
    /// members (a class, not a free function). The special forms and `FileRead` keep
    /// the bespoke `DispatchRoute::Native(id)` route; the parser-bound file statements
    /// are emitted as `ExternProc` calls to their `library_statement_member` location.
    pub fn library_member(self) -> Option<(&'static str, &'static str)> {
        use NativeImplId::*;
        // The owning module name for the migrated id; `None` for any module/id that
        // does not route through the bundle. `Information`, `Interaction`, and `FileIo`
        // are partially migrated (predicates / host functions / by-name file functions
        // only), so the per-id `match` below — not the module alone — decides their
        // membership; the excluded ids of those modules fall through to the final
        // `_ => return None`.
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
            // ── FileSystem (the FileIo by-NAME members — non-empty catalog `names`) ──
            // Module is "FileSystem" (the canonical VBA typelib module name), not the
            // `LibraryModule::FileIo` enum name; see the module-name lookup below.
            //
            // These split into the `Ordinary` FUNCTION forms and the `FileStatement`
            // forms that are *still resolved by name* (`Kill`/`MkDir`/… are not lexer
            // keywords, so they parse as ordinary statement-calls and bind through
            // `name_to_intrinsic`). Both kinds resolve identically — the catalog's
            // `CallShape` is informational and not consulted by the binder — so both
            // migrate here, with the member name = catalog primary (covered by the
            // `migrated_library_member_name_matches_catalog_primary` drift-guard). The
            // name-LESS `FileStatement` forms (`FileOpen`/`Print #`/`Put`/… — parsed to
            // dedicated CST nodes, lowered in `oxvba-bind/stmt.rs`) instead live in
            // `library_statement_member` (they have no catalog name to guard against).
            FreeFile => "FreeFile",
            FileCurDir => "CurDir",
            FileLen => "FileLen",
            FileGetAttr => "GetAttr",
            FileDateTime => "FileDateTime",
            FileEof => "EOF",
            FileLof => "LOF",
            FileSeek => "Seek",
            FileLoc => "Loc",
            // The by-name `FileStatement` forms (not parser-bound — they resolve by
            // name like a function): member = catalog primary.
            FileKill => "Kill",
            FileMkDir => "MkDir",
            FileRmDir => "RmDir",
            FileChDir => "ChDir",
            FileChDrive => "ChDrive",
            FileSetAttr => "SetAttr",
            FileCopy => "FileCopy",
            // Everything else is NOT a by-name `library_member`: the name-less
            // `MidStmt`/`Like`; the `Information` special forms `IIf`/`Choose`/`Switch`;
            // the `Interaction` `CreateObject` + `Com*` machinery; the name-LESS
            // `FileIo` STATEMENT forms (now `library_statement_member` — parser-bound,
            // emitted as `ExternProc` from `stmt.rs`) and the name-less `FileRead`;
            // `Diagnostics` (`DebugPrint`); and the `Collection` members. The special
            // forms / `FileRead` keep the `Native(id)` route.
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
            // The `FileIo` by-name FUNCTIONS export under the canonical VBA typelib
            // module name "FileSystem" (not the `LibraryModule::FileIo` enum name); the
            // partially migrated module's STATEMENT/name-less ids were already excluded
            // by the per-id `match` above, so only a migrated function reaches here.
            LibraryModule::FileIo => "FileSystem",
            // `Collection`/`Diagnostics` have no migrated members, so no id that reaches
            // here belongs to them; if one ever did it would be a bug to surface loudly
            // rather than silently mis-route.
            LibraryModule::Collection | LibraryModule::Diagnostics => {
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
            // ── FileSystem by-name members ──
            // The single-argument forms (functions + the 1-arg name-statements).
            FreeFile | FileCurDir | FileLen | FileGetAttr | FileDateTime | FileEof | FileLof
            | FileSeek | FileLoc | FileKill | FileMkDir | FileRmDir | FileChDir | FileChDrive => 1,
            // The two-argument name-statements.
            FileSetAttr | FileCopy => 2,
            // Not a migrated bundle member (the parser-bound statements use
            // `library_statement_param_count`).
            _ => 0,
        }
    }

    /// The synthetic `VBA`-bundle location `(module, member)` for a **name-less**
    /// file STATEMENT — the funny-syntax forms parsed to dedicated CST nodes and
    /// lowered in `oxvba-bind/stmt.rs` (`Open`/`Close`/`Print #`/`Write #`/`Input #`/
    /// `Line Input #`/`Width #`/`Name … As`/`Lock`/`Unlock`/`Put`/`Get`). Returns
    /// `None` for any other id.
    ///
    /// This is the statement-form analogue of [`NativeImplId::library_member`]. It is a
    /// SEPARATE table because these ids have **empty** catalog `names` (they are not
    /// user-resolvable by name — `name_to_intrinsic` never returns them), so they
    /// cannot be guarded by the catalog-primary-name parity check that the by-name
    /// members use. Instead they are exported under fixed INTERNAL member names of the
    /// `FileSystem` module (chosen to read like the VBA keyword), and the binder
    /// emits a cross-bundle `ExportToken::ModuleFunc` import + `Op::CallExtern` to
    /// them — replacing the bespoke `CoreCallee::Native(File*)` lowering that
    /// `stmt.rs` used. A drift-guard test (`statement_members_are_native_library_procs`
    /// in `oxvba-bundle/vba_library`) asserts the bundle exports each of these.
    ///
    /// `FileSeek` is **not** here: its function form `Seek(n)` is a by-name member
    /// (`library_member` → `FileSystem.Seek`), and the `Seek #n, pos` STATEMENT reuses
    /// that same member (the `oxvba-lib` body dispatches on the argument count), so
    /// `stmt.rs` routes the statement through `library_member(FileSeek)`.
    pub fn library_statement_member(self) -> Option<(&'static str, &'static str)> {
        use NativeImplId::*;
        let member = match self {
            FileOpen => "Open",
            FileClose => "Close",
            FilePrint => "Print",
            FileWrite => "Write",
            FileInput => "Input",
            FileLineInput => "LineInput",
            FileWidth => "Width",
            FileRename => "Name",
            FileLock => "Lock",
            FileUnlock => "Unlock",
            FilePut => "Put",
            FileGetInto => "Get",
            _ => return None,
        };
        Some(("FileSystem", member))
    }

    /// The informational parameter count recorded on the bundle's
    /// [`ProcedureDescriptor`](crate::ProcedureDescriptor) for a name-less file
    /// statement member (see [`NativeImplId::library_statement_member`]) — its
    /// maximum fixed arity. The native body reads its arguments positionally and
    /// ignores this; variadic print/input forms cap their own args, so `0` stands in
    /// for them. `0` for any id that is not a statement member.
    pub fn library_statement_param_count(self) -> usize {
        use NativeImplId::*;
        match self {
            // `FileOpen(path, packed-mode, reclen)` — see `bind_open` in `stmt.rs`.
            FileOpen => 3,
            // `FileGetInto(handle, rec, type-code, str-len)` / `FilePut(handle, rec,
            // value, fixed-flag)` — fixed 4-tuples assembled by `bind_get`/`bind_put`.
            FileGetInto | FilePut => 4,
            // `FileClose([handle…])`, the `Print #`/`Write #`/`Input #` families and
            // `Width #`/`Name … As`/`Lock`/`Unlock` are variadic; cap at 0.
            FileClose | FilePrint | FileWrite | FileInput | FileLineInput | FileWidth
            | FileRename | FileLock | FileUnlock => 0,
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
