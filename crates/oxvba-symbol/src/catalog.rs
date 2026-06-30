//! The exhaustive intrinsic catalog: every `NativeImplId` gets a name set, an
//! arity-shaped signature, and a [`CallShape`] (the quirky-syntax classification).
//!
//! `intrinsic_entry` is a `match` over `NativeImplId` **with no `_` arm** — adding
//! a variant to `oxvba-bundle` fails to compile here until it is classified. That
//! is the no-partial guarantee.

use oxvba_bundle::NativeImplId;

use crate::signature::CallShape;

/// An arity-shaped intrinsic signature. Intrinsics are Variant-flexible, so the
/// binding-relevant facts are arity + variadic + call shape (full per-param types
/// are not needed to bind a library call).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntrinsicSig {
    pub min_args: u8,
    /// `None` = no fixed upper bound (variadic / `ParamArray`-like).
    pub max_args: Option<u8>,
    pub param_array: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct IntrinsicEntry {
    pub id: NativeImplId,
    /// User-facing names (with `$` variants). Empty = not resolvable by bare name
    /// (statement-form, predeclared-member, or internal machinery).
    pub names: &'static [&'static str],
    pub sig: IntrinsicSig,
    pub param_names: &'static [&'static str],
    pub call_shape: CallShape,
}

const fn sig(min: u8, max: u8) -> IntrinsicSig {
    IntrinsicSig {
        min_args: min,
        max_args: Some(max),
        param_array: false,
    }
}
const fn variadic(min: u8) -> IntrinsicSig {
    IntrinsicSig {
        min_args: min,
        max_args: None,
        param_array: true,
    }
}
const fn e(
    id: NativeImplId,
    names: &'static [&'static str],
    sig: IntrinsicSig,
    call_shape: CallShape,
) -> IntrinsicEntry {
    IntrinsicEntry {
        id,
        names,
        sig,
        param_names: &[],
        call_shape,
    }
}
const fn e_params(
    id: NativeImplId,
    names: &'static [&'static str],
    sig: IntrinsicSig,
    param_names: &'static [&'static str],
    call_shape: CallShape,
) -> IntrinsicEntry {
    IntrinsicEntry {
        id,
        names,
        sig,
        param_names,
        call_shape,
    }
}

/// Total classification of every intrinsic. No wildcard arm — exhaustiveness is
/// the coverage guarantee.
pub const fn intrinsic_entry(id: NativeImplId) -> IntrinsicEntry {
    use CallShape::{AssignmentTarget, DebugMember, FileStatement, Ordinary, SpecialForm};
    use NativeImplId::*;
    match id {
        // ── Strings ──
        Len => e(Len, &["Len"], sig(1, 1), Ordinary),
        LenB => e(LenB, &["LenB"], sig(1, 1), Ordinary),
        Left => e(Left, &["Left", "Left$"], sig(2, 2), Ordinary),
        Right => e(Right, &["Right", "Right$"], sig(2, 2), Ordinary),
        Mid => e(Mid, &["Mid", "Mid$"], sig(2, 3), Ordinary),
        MidStmt => e(MidStmt, &[], sig(2, 3), AssignmentTarget),
        InStr => e(InStr, &["InStr"], sig(2, 4), Ordinary),
        InStrRev => e(InStrRev, &["InStrRev"], sig(2, 4), Ordinary),
        LCase => e(LCase, &["LCase", "LCase$"], sig(1, 1), Ordinary),
        UCase => e(UCase, &["UCase", "UCase$"], sig(1, 1), Ordinary),
        Split => e(Split, &["Split"], sig(1, 4), Ordinary),
        Join => e(Join, &["Join"], sig(1, 2), Ordinary),
        Replace => e_params(
            Replace,
            &["Replace"],
            sig(3, 6),
            &["Expression", "Find", "Replace", "Start", "Count", "Compare"],
            Ordinary,
        ),
        Trim => e(Trim, &["Trim", "Trim$"], sig(1, 1), Ordinary),
        LTrim => e(LTrim, &["LTrim", "LTrim$"], sig(1, 1), Ordinary),
        RTrim => e(RTrim, &["RTrim", "RTrim$"], sig(1, 1), Ordinary),
        StrComp => e(StrComp, &["StrComp"], sig(2, 3), Ordinary),
        Like => e(Like, &[], sig(2, 2), Ordinary), // the `Like` operator
        Chr => e(Chr, &["Chr", "Chr$"], sig(1, 1), Ordinary),
        Asc => e(Asc, &["Asc"], sig(1, 1), Ordinary),
        ChrW => e(ChrW, &["ChrW", "ChrW$"], sig(1, 1), Ordinary),
        AscW => e(AscW, &["AscW"], sig(1, 1), Ordinary),
        Space => e(Space, &["Space", "Space$"], sig(1, 1), Ordinary),
        StringRepeat => e(StringRepeat, &["String", "String$"], sig(2, 2), Ordinary),
        StrReverse => e(StrReverse, &["StrReverse"], sig(1, 1), Ordinary),
        StrConv => e(StrConv, &["StrConv"], sig(2, 3), Ordinary),
        Format => e(Format, &["Format", "Format$"], sig(1, 4), Ordinary),
        Filter => e(Filter, &["Filter"], sig(2, 4), Ordinary),

        // ── Math ──
        Abs => e(Abs, &["Abs"], sig(1, 1), Ordinary),
        Int => e(Int, &["Int"], sig(1, 1), Ordinary),
        Fix => e(Fix, &["Fix"], sig(1, 1), Ordinary),
        Sgn => e(Sgn, &["Sgn"], sig(1, 1), Ordinary),
        Round => e(Round, &["Round"], sig(1, 2), Ordinary),
        Sqr => e(Sqr, &["Sqr"], sig(1, 1), Ordinary),
        Sin => e(Sin, &["Sin"], sig(1, 1), Ordinary),
        Cos => e(Cos, &["Cos"], sig(1, 1), Ordinary),
        Log => e(Log, &["Log"], sig(1, 1), Ordinary),
        Exp => e(Exp, &["Exp"], sig(1, 1), Ordinary),
        Atn => e(Atn, &["Atn"], sig(1, 1), Ordinary),
        Tan => e(Tan, &["Tan"], sig(1, 1), Ordinary),

        // ── DateTime ──
        DateSerial => e(DateSerial, &["DateSerial"], sig(3, 3), Ordinary),
        TimeSerial => e(TimeSerial, &["TimeSerial"], sig(3, 3), Ordinary),
        DateValue => e(DateValue, &["DateValue"], sig(1, 1), Ordinary),
        TimeValue => e(TimeValue, &["TimeValue"], sig(1, 1), Ordinary),
        DateAdd => e(DateAdd, &["DateAdd"], sig(3, 3), Ordinary),
        DateDiff => e(DateDiff, &["DateDiff"], sig(3, 5), Ordinary),
        Year => e(Year, &["Year"], sig(1, 1), Ordinary),
        Month => e(Month, &["Month"], sig(1, 1), Ordinary),
        Day => e(Day, &["Day"], sig(1, 1), Ordinary),
        Weekday => e(Weekday, &["Weekday"], sig(1, 2), Ordinary),
        Hour => e(Hour, &["Hour"], sig(1, 1), Ordinary),
        Minute => e(Minute, &["Minute"], sig(1, 1), Ordinary),
        Second => e(Second, &["Second"], sig(1, 1), Ordinary),
        MonthName => e(MonthName, &["MonthName"], sig(1, 2), Ordinary),
        WeekdayName => e(WeekdayName, &["WeekdayName"], sig(1, 3), Ordinary),
        DatePart => e(DatePart, &["DatePart"], sig(2, 4), Ordinary),
        DateNow => e(DateNow, &["Date", "Date$"], sig(0, 0), Ordinary),
        TimeNow => e(TimeNow, &["Time", "Time$"], sig(0, 0), Ordinary),
        Now => e(Now, &["Now"], sig(0, 0), Ordinary),
        Timer => e(Timer, &["Timer"], sig(0, 0), Ordinary),

        // ── Conversion ──
        Hex => e(Hex, &["Hex", "Hex$"], sig(1, 1), Ordinary),
        Oct => e(Oct, &["Oct", "Oct$"], sig(1, 1), Ordinary),
        CStr => e(CStr, &["CStr"], sig(1, 1), Ordinary),
        Str => e(Str, &["Str", "Str$"], sig(1, 1), Ordinary),
        Val => e(Val, &["Val"], sig(1, 1), Ordinary),
        CDate => e(CDate, &["CDate", "CVDate"], sig(1, 1), Ordinary),
        CVErr => e(CVErr, &["CVErr"], sig(1, 1), Ordinary),
        CBool => e(CBool, &["CBool"], sig(1, 1), Ordinary),
        CByte => e(CByte, &["CByte"], sig(1, 1), Ordinary),
        CInt => e(CInt, &["CInt"], sig(1, 1), Ordinary),
        CLng => e(CLng, &["CLng"], sig(1, 1), Ordinary),
        CLngLng => e(CLngLng, &["CLngLng"], sig(1, 1), Ordinary),
        CLngPtr => e(CLngPtr, &["CLngPtr"], sig(1, 1), Ordinary),
        CSng => e(CSng, &["CSng"], sig(1, 1), Ordinary),
        CDbl => e(CDbl, &["CDbl"], sig(1, 1), Ordinary),
        CCur => e(CCur, &["CCur"], sig(1, 1), Ordinary),
        CVar => e(CVar, &["CVar"], sig(1, 1), Ordinary),

        // ── Random ──
        Rnd => e(Rnd, &["Rnd"], sig(0, 1), Ordinary),
        Randomize => e(Randomize, &["Randomize"], sig(0, 1), Ordinary),

        // ── Financial ──
        Fv => e(Fv, &["FV"], sig(3, 5), Ordinary),
        Pv => e(Pv, &["PV"], sig(3, 5), Ordinary),
        Pmt => e(Pmt, &["Pmt"], sig(3, 5), Ordinary),
        Npv => e(Npv, &["NPV"], sig(2, 2), Ordinary),
        Irr => e(Irr, &["IRR"], sig(1, 2), Ordinary),
        Mirr => e(Mirr, &["MIRR"], sig(3, 3), Ordinary),
        Rate => e(Rate, &["Rate"], sig(3, 6), Ordinary),
        NPer => e(NPer, &["NPer"], sig(3, 5), Ordinary),

        // ── Information ──
        IsArray => e(IsArray, &["IsArray"], sig(1, 1), Ordinary),
        VarType => e(VarType, &["VarType"], sig(1, 1), Ordinary),
        TypeName => e(TypeName, &["TypeName"], sig(1, 1), Ordinary),
        IsNumeric => e(IsNumeric, &["IsNumeric"], sig(1, 1), Ordinary),
        IsError => e(IsError, &["IsError"], sig(1, 1), Ordinary),
        IsDate => e(IsDate, &["IsDate"], sig(1, 1), Ordinary),
        IsObject => e(IsObject, &["IsObject"], sig(1, 1), Ordinary),
        IsNull => e(IsNull, &["IsNull"], sig(1, 1), Ordinary),
        IsEmpty => e(IsEmpty, &["IsEmpty"], sig(1, 1), Ordinary),
        IsMissing => e(IsMissing, &["IsMissing"], sig(1, 1), Ordinary),
        // IIf/Choose/Switch resolve by name through the `vba_library` special-form
        // route, not the intrinsic-name map — so they carry no names here; the
        // entry exists only to give the lowering target a shape.
        IIf => e(IIf, &[], sig(3, 3), SpecialForm),
        Choose => e(Choose, &[], variadic(1), SpecialForm),
        Switch => e(Switch, &[], variadic(2), SpecialForm),
        Rgb => e_params(Rgb, &["RGB"], sig(3, 3), &["Red", "Green", "Blue"], Ordinary),
        QbColor => e(QbColor, &["QBColor"], sig(1, 1), Ordinary),

        // ── File / Console I/O ──
        FreeFile => e(FreeFile, &["FreeFile"], sig(0, 1), Ordinary),
        FileOpen => e(FileOpen, &[], variadic(0), FileStatement),
        FileClose => e(FileClose, &[], variadic(0), FileStatement),
        // `Kill pathname` deletes files. Unlike Open/Close/Print#/Name/Lock/…, `Kill`
        // is not a lexer keyword, so it parses as an ordinary statement-call and must
        // resolve by name (a 1-argument native).
        FileKill => e(FileKill, &["Kill"], sig(1, 1), FileStatement),
        // Like `Kill`, the directory statements are not lexer keywords — they
        // resolve by name as 1-argument statement-calls.
        FileMkDir => e(FileMkDir, &["MkDir"], sig(1, 1), FileStatement),
        FileRmDir => e(FileRmDir, &["RmDir"], sig(1, 1), FileStatement),
        FileCurDir => e(FileCurDir, &["CurDir"], sig(0, 1), Ordinary),
        FileChDir => e(FileChDir, &["ChDir"], sig(1, 1), FileStatement),
        FileLen => e(FileLen, &["FileLen"], sig(1, 1), Ordinary),
        FileCopy => e(FileCopy, &["FileCopy"], sig(2, 2), FileStatement),
        FileGetAttr => e(FileGetAttr, &["GetAttr"], sig(1, 1), Ordinary),
        FileSetAttr => e(FileSetAttr, &["SetAttr"], sig(2, 2), FileStatement),
        FileChDrive => e(FileChDrive, &["ChDrive"], sig(1, 1), FileStatement),
        FileDateTime => e(FileDateTime, &["FileDateTime"], sig(1, 1), Ordinary),
        FileRead => e(FileRead, &[], variadic(0), Ordinary),
        FileWrite => e(FileWrite, &[], variadic(0), FileStatement),
        FilePrint => e(FilePrint, &[], variadic(0), FileStatement),
        ConsolePrint => e(ConsolePrint, &["Print"], variadic(0), FileStatement),
        FileInput => e(FileInput, &[], variadic(0), FileStatement),
        ConsoleInput => e(ConsoleInput, &[], variadic(0), FileStatement),
        FileLineInput => e(FileLineInput, &[], variadic(0), FileStatement),
        ConsoleLineInput => e(ConsoleLineInput, &[], variadic(0), FileStatement),
        FileEof => e(FileEof, &["EOF"], sig(1, 1), Ordinary),
        FileLof => e(FileLof, &["LOF"], sig(1, 1), Ordinary),
        FileSeek => e(FileSeek, &["Seek"], sig(1, 1), Ordinary),
        // Random-access / misc file statements (routed from the parser, not by name).
        FilePut => e(FilePut, &[], variadic(0), FileStatement),
        FileGetInto => e(FileGetInto, &[], variadic(0), FileStatement),
        FileWidth => e(FileWidth, &[], variadic(0), FileStatement),
        FileRename => e(FileRename, &[], variadic(0), FileStatement),
        FileLock => e(FileLock, &[], variadic(0), FileStatement),
        FileUnlock => e(FileUnlock, &[], variadic(0), FileStatement),
        FileLoc => e(FileLoc, &["Loc"], sig(1, 1), Ordinary),

        // ── Interaction / host ──
        MsgBox => e(MsgBox, &["MsgBox"], sig(1, 5), Ordinary),
        InputBox => e(InputBox, &["InputBox"], sig(1, 7), Ordinary),
        Beep => e(Beep, &["Beep"], sig(0, 0), Ordinary),
        DoEvents => e(DoEvents, &["DoEvents"], sig(0, 0), Ordinary),
        Shell => e(Shell, &["Shell"], sig(1, 2), Ordinary),
        Environ => e(Environ, &["Environ", "Environ$"], sig(1, 1), Ordinary),
        Dir => e(Dir, &["Dir", "Dir$"], sig(0, 2), Ordinary),
        CreateObject => e(CreateObject, &["CreateObject"], sig(1, 2), SpecialForm),
        GetObject => e(GetObject, &["GetObject"], sig(1, 2), SpecialForm),
        ComSubscribeEvent => e(ComSubscribeEvent, &[], variadic(0), SpecialForm),
        ComUnsubscribeEvent => e(ComUnsubscribeEvent, &[], variadic(0), SpecialForm),
        ComEventCallbackSubscription => {
            e(ComEventCallbackSubscription, &[], variadic(0), SpecialForm)
        }
        ComEventCallbackArg => e(ComEventCallbackArg, &[], variadic(0), SpecialForm),
        ComReleaseEventCallback => e(ComReleaseEventCallback, &[], variadic(0), SpecialForm),

        // ── Diagnostics ──
        DebugPrint => e(DebugPrint, &[], variadic(0), DebugMember),
    }
}

/// Every `NativeImplId`, generated from the enum declaration itself
/// ([`NativeImplId::ALL`]). The exhaustive `intrinsic_entry` match is the other
/// half of the no-partial guarantee: a new variant fails to compile until it has
/// an entry, and it joins this slice automatically — nothing to hand-maintain.
pub const ALL_INTRINSICS: &[NativeImplId] = NativeImplId::ALL;

/// Resolve a bare intrinsic name (case-insensitive) to its `NativeImplId`.
pub fn name_to_intrinsic(name: &str) -> Option<NativeImplId> {
    ALL_INTRINSICS.iter().copied().find(|id| {
        intrinsic_entry(*id)
            .names
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(name))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_every_native_impl_id() {
        // `intrinsic_entry` is a wildcard-free match, so name/signature/kind
        // coverage is enforced at compile time and there is no count to maintain.
        // This guards the one property the compiler can't: no callable name is
        // claimed by two variants, which would make `name_to_intrinsic` (a
        // first-match lookup) silently shadow one of them. Statement/assignment
        // forms such as `MidStmt` carry no name and are dispatched structurally.
        let mut seen: Vec<String> = Vec::new();
        for id in ALL_INTRINSICS.iter().copied() {
            for name in intrinsic_entry(id).names {
                let lower = name.to_ascii_lowercase();
                assert!(
                    !seen.contains(&lower),
                    "name {name:?} is claimed by more than one NativeImplId"
                );
                seen.push(lower);
            }
        }
    }

    #[test]
    fn names_round_trip_through_reverse_map() {
        for id in ALL_INTRINSICS.iter().copied() {
            for name in intrinsic_entry(id).names {
                assert_eq!(name_to_intrinsic(name), Some(id), "name {name}");
            }
        }
    }

    #[test]
    fn migrated_library_member_name_matches_catalog_primary() {
        // The `VBA`-bundle export/import name for a migrated id
        // (`NativeImplId::library_member`) must equal the catalog's *primary* (first)
        // user-facing name — the name `name_to_intrinsic` maps a source identifier to.
        // If they diverged, a user-typed call (`Abs(x)`) would resolve to the id but
        // the binder would import a member name the bundle never exported, breaking
        // the cross-bundle link. This binds the two hand-written name sources (which
        // live in different crates) together.
        for id in ALL_INTRINSICS.iter().copied() {
            let Some((_module, member)) = id.library_member() else {
                continue;
            };
            let primary = intrinsic_entry(id)
                .names
                .first()
                .unwrap_or_else(|| panic!("migrated id {id:?} has no catalog name"));
            assert!(
                member.eq_ignore_ascii_case(primary),
                "library_member name {member:?} != catalog primary {primary:?} for {id:?}",
            );
        }
    }

    #[test]
    fn quirky_call_shapes_are_classified() {
        assert_eq!(
            intrinsic_entry(NativeImplId::MidStmt).call_shape,
            CallShape::AssignmentTarget
        );
        assert_eq!(
            intrinsic_entry(NativeImplId::FileOpen).call_shape,
            CallShape::FileStatement
        );
        assert_eq!(
            intrinsic_entry(NativeImplId::FilePrint).call_shape,
            CallShape::FileStatement
        );
        assert_eq!(
            intrinsic_entry(NativeImplId::DebugPrint).call_shape,
            CallShape::DebugMember
        );
        assert_eq!(
            intrinsic_entry(NativeImplId::CreateObject).call_shape,
            CallShape::SpecialForm
        );
    }
}
