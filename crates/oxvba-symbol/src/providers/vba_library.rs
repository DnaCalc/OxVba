//! The VBA base-library provider: `vb*` value constants, the intrinsic catalog,
//! the user-facing structural intrinsics (`VarPtr`/`StrPtr`/`ObjPtr`), the special
//! forms (`Array`/`IIf`/…), and the predeclared `Debug`/`Err`/`Collection` objects.
//! Constants are ported verbatim from the legacy `frontend_library`.

use oxvba_bundle::ProjectMemberKind;

use crate::binding::{Binding, DispatchRoute, SpecialForm};
use crate::catalog::name_to_intrinsic;
use crate::model::{LibraryConstValue, PredeclaredObjectId};
use crate::predeclared::{predeclared_member, predeclared_object};
use crate::provider::Provider;
use crate::signature::VarTypeRef;
use crate::structural::StructuralIntrinsic;

pub struct VbaLibraryProvider;

impl Provider for VbaLibraryProvider {
    fn resolve(&self, name: &str) -> Option<Binding> {
        if let Some(value) = library_constant(name) {
            return Some(Binding::new(None, DispatchRoute::LibraryConst(value)));
        }
        if let Some(object) = predeclared_object(name) {
            return Some(Binding::new(None, DispatchRoute::PredeclaredObject(object)));
        }
        if let Some(id) = name_to_intrinsic(name) {
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
        let object = predeclared_object(type_name)?;
        let route = predeclared_member(object, name)?;
        Some(Binding::new(None, route))
    }
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
        _ => return None,
    })
}

/// The predeclared-object lookup helper (also used by `build`).
pub fn library_predeclared_object(name: &str) -> Option<PredeclaredObjectId> {
    predeclared_object(name)
}

/// Resolve a `vb*` base-library value constant by case-insensitive name. Ported
/// verbatim from `frontend_library`. `vbNullString`/`Nothing`/`Empty`/`Null` are
/// intentionally absent — they are structural intrinsics, not value constants.
pub fn library_constant(name: &str) -> Option<LibraryConstValue> {
    use LibraryConstValue::{Int, Str};
    Some(match name.to_ascii_lowercase().as_str() {
        "vbcr" => Str("\r".into()),
        "vblf" => Str("\n".into()),
        "vbcrlf" | "vbnewline" => Str("\r\n".into()),
        "vbtab" => Str("\t".into()),
        "vbnullchar" => Str("\0".into()),
        "vbback" => Str("\u{0008}".into()),
        "vbformfeed" => Str("\u{000C}".into()),
        "vbverticaltab" => Str("\u{000B}".into()),
        "vbbinarycompare" => Int(0),
        "vbtextcompare" => Int(1),
        "vbdatabasecompare" => Int(2),
        "vbuppercase" => Int(1),
        "vblowercase" => Int(2),
        "vbpropercase" => Int(3),
        "vbwide" => Int(4),
        "vbnarrow" => Int(8),
        "vbkatakana" => Int(16),
        "vbhiragana" => Int(32),
        "vbunicode" => Int(64),
        "vbfromunicode" => Int(128),
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
        "vbtrue" => Int(-1),
        "vbfalse" => Int(0),
        "vbusedefault" => Int(-2),
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
        "vbok" => Int(1),
        "vbcancel" => Int(2),
        "vbabort" => Int(3),
        "vbretry" => Int(4),
        "vbignore" => Int(5),
        "vbyes" => Int(6),
        "vbno" => Int(7),
        "vbblack" => Int(0),
        "vbred" => Int(255),
        "vbgreen" => Int(65280),
        "vbyellow" => Int(65535),
        "vbblue" => Int(16711680),
        "vbmagenta" => Int(16711935),
        "vbcyan" => Int(16776960),
        "vbwhite" => Int(16777215),
        "vbgeneraldate" => Int(0),
        "vblongdate" => Int(1),
        "vbshortdate" => Int(2),
        "vblongtime" => Int(3),
        "vbshorttime" => Int(4),
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
        "vbnormal" => Int(0),
        "vbreadonly" => Int(1),
        "vbhidden" => Int(2),
        "vbsystem" => Int(4),
        "vbvolume" => Int(8),
        "vbdirectory" => Int(16),
        "vbarchive" => Int(32),
        "vbobjecterror" => Int(-2147221504),
        _ => return None,
    })
}
