//! The typed COM model — the [`crate::ty::IfaceId`]-indexed interface table and the
//! early-bound call-site key.
//!
//! First-class typed COM is a primary OxVBA goal, and the enabling fact is that the
//! full typed signature **already exists upstream**: `oxvba_com::TypeLibMemberMetadata`
//! carries per-param semantic types + ABI wire shapes + interface IIDs, the
//! `QueryInterface` target, the x64 vtable slot (+ AV-safety bound), dual-vs-dispinterface
//! kind, return type/wire, and optional/`[lcid]` synthesis rules;
//! `oxvba_com::TypeLibInterfaceMetadata` groups those members under an interface name +
//! IID. **OxIR reuses those types verbatim** (they carry `serde`, added on the canonical
//! definitions) — it does not mirror them. The legacy pipeline discarded ~95% of this,
//! keeping only a `(dispid, name)` selector and re-resolving the live object at run
//! time; the Core IR now carries the descriptor on `CoreCallee::EarlyCom` and the
//! elaboration interns it here, so an early-bound call is fully typed with no typelib
//! re-resolution. (The typed *receiver* identity — typing a `Dim r As Excel.Range` local
//! as [`crate::ty::ObjClass::ComIface`] rather than `Untyped` — is a later refinement;
//! each call already names its own typed descriptor.)
//!
//! The only structure OxIR **adds** is the [`crate::ty::IfaceId`]-indexed table
//! ([`ComInterface`]) — which has no upstream equivalent because it unifies referenced/
//! served COM interfaces (the [`crate::ty::ObjClass::ComIface`] case) with project
//! `Implements` interfaces (the [`crate::ty::ObjClass::Iface`] case) into one id space —
//! and the [`ComMethodRef`] call-site key naming a member within an interface.
//!
//! The dynamic path ([`crate::inst::OxInst::ComCallLate`]) needs no descriptor: a
//! genuinely `Object`/`Variant` receiver is dispatched by name with Variant args —
//! correct VBA late-binding, not erasure.

use serde::{Deserialize, Serialize};

use oxvba_bundle::ProjectMemberKind;
use oxvba_com::{TypeLibInterfaceMetadata, TypeLibMemberMetadata};

use crate::ty::IfaceId;

/// A reference to one early-bound COM member: the [`IfaceId`] of its owning interface
/// and the member's position within that interface's member list. The payload of
/// [`crate::inst::OxInst::ComCallEarly`]; resolve it to the typed
/// [`TypeLibMemberMetadata`] with [`crate::program::OxProgram::com_method`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComMethodRef {
    pub iface: IfaceId,
    pub member: usize,
}

/// One entry in the program's interface table (indexed by [`IfaceId`]): either a
/// referenced/served COM interface (the canonical `oxvba_com` grouping, reused
/// verbatim) or a project `Implements` interface (typed internal dispatch). A typed
/// object value's [`crate::ty::ObjClass`] names an entry here — which is what makes a
/// typed receiver a typed interface identity rather than a name string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComInterface {
    /// A referenced or served COM interface — the canonical typelib grouping (name +
    /// IID + the full typed member descriptors). Named by
    /// [`crate::ty::ObjClass::ComIface`].
    Com(TypeLibInterfaceMetadata),
    /// A project `Implements` interface — typed internal dispatch. Named by
    /// [`crate::ty::ObjClass::Iface`]. The per-implementing-class member→proc
    /// resolution is attached during elaboration.
    Project(ProjectInterface),
}

impl ComInterface {
    /// The interface display name.
    pub fn name(&self) -> &str {
        match self {
            ComInterface::Com(meta) => &meta.name,
            ComInterface::Project(p) => &p.name,
        }
    }

    /// The typed member descriptors of a COM interface (`None` for a project interface).
    pub fn com_members(&self) -> Option<&[TypeLibMemberMetadata]> {
        match self {
            ComInterface::Com(meta) => Some(&meta.members),
            ComInterface::Project(_) => None,
        }
    }
}

/// A project `Implements` interface: its name and ordered member signatures. The typed
/// signature of each member is the implementing class's own [`OxFunc`]; this carries
/// only the name+kind needed to resolve `Dim x As IFoo : x.M` to the right member.
///
/// [`OxFunc`]: crate::program::OxFunc
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInterface {
    pub name: String,
    pub methods: Vec<ProjectIfaceMethod>,
}

/// One member of a project `Implements` interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIfaceMethod {
    pub name: String,
    pub kind: ProjectMemberKind,
}
