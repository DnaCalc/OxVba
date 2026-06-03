//! VBA base-library descriptor — the seed of the HIR resolution environment.
//!
//! Per `docs/spec/HIR_RESOLUTION_ENVIRONMENT_V1.md`, the always-available VBA
//! library is modeled as an implicitly-referenced descriptor resolved through the
//! same path as project/COM references, rather than as scattered parser/compiler
//! special-cases. This module is the **single source of truth** for that surface:
//! the HIR front-end resolves bare library names against it (with user symbols
//! shadowing), and the legacy resolver delegates to it.
//!
//! This first slice covers the `vb*` value constants. Later phases extend the
//! descriptor with callables (carrying native impl ids) and the predeclared
//! `Debug`/`Err`/`Collection` objects with their members.

/// A neutral, IR-independent value for a base-library constant. Consumers map it
/// to their own literal representation (HIR `HirLiteral`, legacy `BoundExpr`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryConstantValue {
    Str(&'static str),
    Int(i32),
}

/// Resolve a bare VBA library value constant (`vbCrLf`, `vbYesNo`, …) by
/// case-insensitive name.
///
/// `vbNullString`, `Empty`, `Null`, and `Nothing` are intentionally absent: they
/// are modeled as dedicated structural intrinsics, not value constants.
pub fn vba_library_constant(name: &str) -> Option<LibraryConstantValue> {
    use LibraryConstantValue::{Int, Str};
    Some(match name.to_ascii_lowercase().as_str() {
        // String control characters
        "vbcr" => Str("\r"),
        "vblf" => Str("\n"),
        "vbcrlf" | "vbnewline" => Str("\r\n"),
        "vbtab" => Str("\t"),
        "vbnullchar" => Str("\0"),
        "vbback" => Str("\u{0008}"),
        "vbformfeed" => Str("\u{000C}"),
        "vbverticaltab" => Str("\u{000B}"),
        // Comparison (VbCompareMethod)
        "vbbinarycompare" => Int(0),
        "vbtextcompare" => Int(1),
        "vbdatabasecompare" => Int(2),
        // String conversion (VbStrConv)
        "vbuppercase" => Int(1),
        "vblowercase" => Int(2),
        "vbpropercase" => Int(3),
        "vbwide" => Int(4),
        "vbnarrow" => Int(8),
        "vbkatakana" => Int(16),
        "vbhiragana" => Int(32),
        "vbunicode" => Int(64),
        "vbfromunicode" => Int(128),
        // VarType (VbVarType)
        "vbempty" => Int(0),
        "vbnull" => Int(1),
        "vbinteger" => Int(2),
        "vblong" => Int(3),
        "vbsingle" => Int(4),
        "vbdouble" => Int(5),
        "vbcurrency" => Int(6),
        "vbdate" => Int(7),
        "vbstring" => Int(8),
        "vbobject" => Int(9),
        "vberror" => Int(10),
        "vbboolean" => Int(11),
        "vbvariant" => Int(12),
        "vbdataobject" => Int(13),
        "vbdecimal" => Int(14),
        "vbbyte" => Int(17),
        "vblonglong" => Int(20),
        "vbuserdefinedtype" => Int(36),
        "vbarray" => Int(8192),
        // Tristate / boolean-ish
        "vbtrue" => Int(-1),
        "vbfalse" => Int(0),
        "vbusedefault" => Int(-2),
        // MsgBox buttons / icons / defaults / modality (VbMsgBoxStyle)
        "vbokonly" => Int(0),
        "vbokcancel" => Int(1),
        "vbabortretryignore" => Int(2),
        "vbyesnocancel" => Int(3),
        "vbyesno" => Int(4),
        "vbretrycancel" => Int(5),
        "vbcritical" => Int(16),
        "vbquestion" => Int(32),
        "vbexclamation" => Int(48),
        "vbinformation" => Int(64),
        "vbdefaultbutton1" => Int(0),
        "vbdefaultbutton2" => Int(256),
        "vbdefaultbutton3" => Int(512),
        "vbdefaultbutton4" => Int(768),
        "vbapplicationmodal" => Int(0),
        "vbsystemmodal" => Int(4096),
        "vbmsgboxhelpbutton" => Int(16384),
        "vbmsgboxsetforeground" => Int(65536),
        "vbmsgboxright" => Int(524288),
        "vbmsgboxrtlreading" => Int(1048576),
        // MsgBox results (VbMsgBoxResult)
        "vbok" => Int(1),
        "vbcancel" => Int(2),
        "vbabort" => Int(3),
        "vbretry" => Int(4),
        "vbignore" => Int(5),
        "vbyes" => Int(6),
        "vbno" => Int(7),
        // Colors (VbColorConstants), RGB-packed
        "vbblack" => Int(0),
        "vbred" => Int(255),
        "vbgreen" => Int(65280),
        "vbyellow" => Int(65535),
        "vbblue" => Int(16711680),
        "vbmagenta" => Int(16711935),
        "vbcyan" => Int(16776960),
        "vbwhite" => Int(16777215),
        // Date format (VbDateTimeFormat)
        "vbgeneraldate" => Int(0),
        "vblongdate" => Int(1),
        "vbshortdate" => Int(2),
        "vblongtime" => Int(3),
        "vbshorttime" => Int(4),
        // Day of week / first week (VbDayOfWeek / VbFirstWeekOfYear)
        "vbusesystemdayofweek" => Int(0),
        "vbsunday" => Int(1),
        "vbmonday" => Int(2),
        "vbtuesday" => Int(3),
        "vbwednesday" => Int(4),
        "vbthursday" => Int(5),
        "vbfriday" => Int(6),
        "vbsaturday" => Int(7),
        "vbusesystem" => Int(0),
        "vbfirstjan1" => Int(1),
        "vbfirstfourdays" => Int(2),
        "vbfirstfullweek" => Int(3),
        // File attributes (VbFileAttribute)
        "vbnormal" => Int(0),
        "vbreadonly" => Int(1),
        "vbhidden" => Int(2),
        "vbsystem" => Int(4),
        "vbvolume" => Int(8),
        "vbdirectory" => Int(16),
        "vbarchive" => Int(32),
        // Automation/object error base
        "vbobjecterror" => Int(-2147221504),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_string_and_integer_constants_case_insensitively() {
        assert_eq!(vba_library_constant("vbCrLf"), Some(LibraryConstantValue::Str("\r\n")));
        assert_eq!(vba_library_constant("VBCRLF"), Some(LibraryConstantValue::Str("\r\n")));
        assert_eq!(vba_library_constant("vbYesNo"), Some(LibraryConstantValue::Int(4)));
        assert_eq!(vba_library_constant("vbObjectError"), Some(LibraryConstantValue::Int(-2147221504)));
    }

    #[test]
    fn unknown_and_structural_intrinsic_names_are_absent() {
        assert_eq!(vba_library_constant("notaconstant"), None);
        // Structural intrinsics are modeled elsewhere, not as value constants.
        assert_eq!(vba_library_constant("vbNullString"), None);
        assert_eq!(vba_library_constant("Nothing"), None);
        assert_eq!(vba_library_constant("Empty"), None);
        assert_eq!(vba_library_constant("Null"), None);
    }
}
