//! The VBA base-library provider: `vb*` value constants, the intrinsic catalog,
//! the user-facing structural intrinsics (`VarPtr`/`StrPtr`/`ObjPtr`), the special
//! forms (`Array`/`IIf`/…), and the predeclared `Debug`/`Err` objects. Constants
//! are ported verbatim from the legacy `frontend_library`.
//!
//! The built-in `Collection` is *not* a predeclared object: it is a class of the
//! `VBA` library bundle, resolved here as a creatable extern coclass
//! (`resolve_extern_coclass`) whose members route cross-bundle to that unit
//! (`resolve_member` → `ExternMember`), exactly like a referenced project's class.

use oxvba_bundle::ProjectMemberKind;
use oxvba_bundle::coreir::CoreConst;

use crate::binding::{Binding, DispatchRoute, SpecialForm};
use crate::catalog::name_to_intrinsic;
use crate::model::{PredeclaredObjectId, fold_identifier};
use crate::predeclared::{predeclared_member, predeclared_object};
use crate::provider::Provider;
use crate::signature::VarTypeRef;
use crate::structural::StructuralIntrinsic;

/// The unit name of the built-in library bundle (see `oxvba-bundle/vba_library`).
const VBA_UNIT: &str = "VBA";

pub struct VbaLibraryProvider;

impl Provider for VbaLibraryProvider {
    fn resolve(&self, name: &str) -> Option<Binding> {
        // A `vb*` value constant resolves through the SAME shared route a referenced
        // project's `Public Const`/`Enum` member uses: the folded literal carried in
        // `ConstValue`. The binder inlines it identically (see `expr::const_type`),
        // so this is a pure routing change — the values are unchanged.
        if let Some(value) = library_constant(name) {
            return Some(Binding::new(None, DispatchRoute::ConstValue(value)));
        }
        if let Some(object) = predeclared_object(name) {
            return Some(Binding::new(None, DispatchRoute::PredeclaredObject(object)));
        }
        if let Some(id) = name_to_intrinsic(name) {
            // A migrated library function (`Strings`/`Math`/`DateTime`/`Conversion`/
            // `Random`/`Financial`) resolves like a referenced project's
            // hidden-module free function: a cross-bundle `ExternMember` (no receiver)
            // against the synthetic `VBA` unit's owning module. The binder lowers it
            // to a `ModuleFunc` import + `ExternProc` call, and the VM runs the linked
            // native-bodied proc through `oxvba-lib` — the same `oxvba-lib` body the
            // bespoke `Native` route used, so behaviour is unchanged. The `(owner,
            // member)` location is `NativeImplId::library_member`, the single source of
            // truth shared with the bundle's export tokens, so the import links to the
            // export by construction. Every non-migrated intrinsic (Information special
            // forms + predicates, Interaction, FileIo, Diagnostics) keeps the
            // `Native(id)` route.
            if let Some((owner, member)) = id.library_member() {
                return Some(Binding::new(
                    None,
                    DispatchRoute::ExternMember {
                        unit: VBA_UNIT.to_string(),
                        owner: owner.to_string(),
                        member: member.to_string(),
                        kind: ProjectMemberKind::Method,
                        param_types: Vec::new(),
                        param_names: Vec::new(),
                        has_receiver: false,
                    },
                ));
            }
            return Some(Binding::new(None, DispatchRoute::Native(id)));
        }
        if let Some(structural) = user_structural_intrinsic(name) {
            return Some(Binding::new(None, DispatchRoute::Structural(structural)));
        }
        if let Some(form) = special_form(name) {
            return Some(Binding::new(None, DispatchRoute::SpecialForm(form)));
        }
        None
    }

    fn resolve_member(
        &self,
        recv: &VarTypeRef,
        name: &str,
        _want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        let VarTypeRef::Object(type_name) = recv else {
            return None;
        };
        // The built-in `Collection` resolves like a referenced coclass: its members
        // bind cross-bundle to the `VBA` library class (late dispatch by name), not
        // via a predeclared/`NativeImplId` route.
        if fold_identifier(type_name) == "collection" {
            return collection_member(name);
        }
        let object = predeclared_object(type_name)?;
        let route = predeclared_member(object, name)?;
        Some(Binding::new(None, route))
    }

    fn resolve_extern_coclass(&self, name: &str) -> Option<(String, String)> {
        // `New Collection` (or `New VBA.Collection`) instantiates the built-in
        // `VBA.Collection` class via the same cross-bundle `NewExtern` path a
        // referenced project's coclass uses.
        let folded = fold_identifier(name);
        let bare = folded.strip_prefix("vba.").unwrap_or(&folded);
        (bare == "collection").then(|| (VBA_UNIT.to_string(), "Collection".to_string()))
    }
}

/// Resolve a `Collection` member to its cross-bundle [`DispatchRoute::ExternMember`]
/// route against the `VBA` unit. The accessor kinds must match the VBA bundle's
/// `ClassMethod` kinds (`Count` is a property-get; the rest are methods) so the
/// late-dispatch kind check matches; they are fixed by member name, not by the
/// caller's `want` hint.
fn collection_member(name: &str) -> Option<Binding> {
    let (member, kind) = match fold_identifier(name).as_str() {
        "add" => ("Add", ProjectMemberKind::Method),
        "item" => ("Item", ProjectMemberKind::Method),
        "count" => ("Count", ProjectMemberKind::PropertyGet),
        "remove" => ("Remove", ProjectMemberKind::Method),
        _ => return None,
    };
    Some(Binding::new(
        None,
        DispatchRoute::ExternMember {
            unit: VBA_UNIT.to_string(),
            owner: "Collection".to_string(),
            member: member.to_string(),
            kind,
            param_types: Vec::new(),
            param_names: Vec::new(),
            has_receiver: true,
        },
    ))
}

/// The user-facing structural pointer intrinsics (the rest of the structural set
/// is compiler-internal and not resolvable by source name).
fn user_structural_intrinsic(name: &str) -> Option<StructuralIntrinsic> {
    Some(match name.to_ascii_lowercase().as_str() {
        "varptr" => StructuralIntrinsic::VarPtr,
        "strptr" => StructuralIntrinsic::StrPtr,
        "objptr" => StructuralIntrinsic::ObjPtr,
        _ => return None,
    })
}

fn special_form(name: &str) -> Option<SpecialForm> {
    Some(match name.to_ascii_lowercase().as_str() {
        "array" => SpecialForm::Array,
        "iif" => SpecialForm::IIf,
        "choose" => SpecialForm::Choose,
        "switch" => SpecialForm::Switch,
        "callbyname" => SpecialForm::CallByName,
        "typeof" => SpecialForm::TypeOf,
        "addressof" => SpecialForm::AddressOf,
        "ubound" => SpecialForm::UBound,
        "lbound" => SpecialForm::LBound,
        _ => return None,
    })
}

/// The predeclared-object lookup helper (also used by `build`).
pub fn library_predeclared_object(name: &str) -> Option<PredeclaredObjectId> {
    predeclared_object(name)
}

/// Resolve a `vb*` base-library value constant by case-insensitive name, as the
/// folded `CoreConst` literal the binder inlines. Ported verbatim from
/// `frontend_library`. Integer constants are `I32` (VBA `Long`/`Integer`), string
/// constants are `Str`; this is exactly what a referenced project's `Public Const`
/// folds to, so a `vb*` constant binds identically to a published one.
/// `vbNullString`/`Nothing`/`Empty`/`Null` are intentionally absent — they are
/// structural intrinsics, not value constants.
pub fn library_constant(name: &str) -> Option<CoreConst> {
    // `int(i32)` → `CoreConst::I32` (VBA `Long`); `str(&str)` → `CoreConst::Str`.
    let int = CoreConst::I32;
    let str = |s: &str| CoreConst::Str(s.to_string());
    Some(match name.to_ascii_lowercase().as_str() {
        "vbcr" => str("\r"),
        "vblf" => str("\n"),
        "vbcrlf" | "vbnewline" => str("\r\n"),
        "vbtab" => str("\t"),
        // VbCallType (for CallByName's third argument).
        "vbmethod" => int(1),
        "vbget" => int(2),
        "vblet" => int(4),
        "vbset" => int(8),
        "vbnullchar" => str("\0"),
        // An empty string at the VBA level (compares/assigns as ""); the
        // null-pointer distinction only matters at the `Declare` boundary.
        "vbnullstring" => str(""),
        "vbback" => str("\u{0008}"),
        "vbformfeed" => str("\u{000C}"),
        "vbverticaltab" => str("\u{000B}"),
        "vbbinarycompare" => int(0),
        "vbtextcompare" => int(1),
        "vbdatabasecompare" => int(2),
        "vbuppercase" => int(1),
        "vblowercase" => int(2),
        "vbpropercase" => int(3),
        "vbwide" => int(4),
        "vbnarrow" => int(8),
        "vbkatakana" => int(16),
        "vbhiragana" => int(32),
        "vbunicode" => int(64),
        "vbfromunicode" => int(128),
        "vbempty" => int(0),
        "vbnull" => int(1),
        "vbinteger" => int(2),
        "vblong" => int(3),
        "vbsingle" => int(4),
        "vbdouble" => int(5),
        "vbcurrency" => int(6),
        "vbdate" => int(7),
        "vbstring" => int(8),
        "vbobject" => int(9),
        "vberror" => int(10),
        "vbboolean" => int(11),
        "vbvariant" => int(12),
        "vbdataobject" => int(13),
        "vbdecimal" => int(14),
        "vbbyte" => int(17),
        "vblonglong" => int(20),
        "vbuserdefinedtype" => int(36),
        "vbarray" => int(8192),
        "vbtrue" => int(-1),
        "vbfalse" => int(0),
        "vbusedefault" => int(-2),
        "vbokonly" => int(0),
        "vbokcancel" => int(1),
        "vbabortretryignore" => int(2),
        "vbyesnocancel" => int(3),
        "vbyesno" => int(4),
        "vbretrycancel" => int(5),
        "vbcritical" => int(16),
        "vbquestion" => int(32),
        "vbexclamation" => int(48),
        "vbinformation" => int(64),
        "vbdefaultbutton1" => int(0),
        "vbdefaultbutton2" => int(256),
        "vbdefaultbutton3" => int(512),
        "vbdefaultbutton4" => int(768),
        "vbapplicationmodal" => int(0),
        "vbsystemmodal" => int(4096),
        "vbmsgboxhelpbutton" => int(16384),
        "vbmsgboxsetforeground" => int(65536),
        "vbmsgboxright" => int(524288),
        "vbmsgboxrtlreading" => int(1048576),
        "vbok" => int(1),
        "vbcancel" => int(2),
        "vbabort" => int(3),
        "vbretry" => int(4),
        "vbignore" => int(5),
        "vbyes" => int(6),
        "vbno" => int(7),
        "vbblack" => int(0),
        "vbred" => int(255),
        "vbgreen" => int(65280),
        "vbyellow" => int(65535),
        "vbblue" => int(16711680),
        "vbmagenta" => int(16711935),
        "vbcyan" => int(16776960),
        "vbwhite" => int(16777215),
        "vbgeneraldate" => int(0),
        "vblongdate" => int(1),
        "vbshortdate" => int(2),
        "vblongtime" => int(3),
        "vbshorttime" => int(4),
        "vbusesystemdayofweek" => int(0),
        "vbsunday" => int(1),
        "vbmonday" => int(2),
        "vbtuesday" => int(3),
        "vbwednesday" => int(4),
        "vbthursday" => int(5),
        "vbfriday" => int(6),
        "vbsaturday" => int(7),
        "vbusesystem" => int(0),
        "vbfirstjan1" => int(1),
        "vbfirstfourdays" => int(2),
        "vbfirstfullweek" => int(3),
        "vbnormal" => int(0),
        "vbreadonly" => int(1),
        "vbhidden" => int(2),
        "vbsystem" => int(4),
        "vbvolume" => int(8),
        "vbdirectory" => int(16),
        "vbarchive" => int(32),
        "vbobjecterror" => int(-2147221504),
        _ => return None,
    })
}
