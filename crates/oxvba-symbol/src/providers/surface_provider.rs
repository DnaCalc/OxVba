//! Resolves names against a *referenced project's* synthesized COM export
//! surface ([`crate::surface::ProjectExportSurface`]). This is how a referencing
//! project binds to a referenced one: through the same COM contract it would see
//! if the referenced project were a compiled COM component.
//!
//! Route choice mirrors the COM shape:
//!   * a **coclass** member (resolved on a typed object receiver) → an early-bound
//!     [`DispatchRoute::ComMember`] carrying the surface dispid; the VM dispatches
//!     it in-image and dispid-faithfully (WS-H).
//!   * a **hidden-module** / global-namespace function (no receiver — COM has no
//!     free functions) → [`DispatchRoute::ProjectMember`] carrying the originating
//!     symbol, which the binder devirtualizes to an in-image `CallProc`.
//!   * a `Public Enum` member / `Public Const` → [`DispatchRoute::Value`].
//!
//! `New <coclass>` is **not** resolved here: a referenced class is lowered into the
//! same bundle, so it instantiates via the binder's `class_of_sym` → `New(ClassId)`
//! (WS-D), not via COM activation.

use oxvba_bundle::ProjectMemberKind;

use crate::binding::{Binding, DispatchRoute};
use crate::model::fold_identifier;
use crate::provider::Provider;
use crate::signature::VarTypeRef;
use crate::surface::{ProjectExportSurface, SurfaceMember, SurfaceType, SurfaceTypeKind};

pub struct SurfaceProvider {
    surface: ProjectExportSurface,
    project_folded: String,
}

impl SurfaceProvider {
    pub fn new(surface: ProjectExportSurface) -> Self {
        let project_folded = fold_identifier(&surface.project_name);
        Self { surface, project_folded }
    }

    fn type_by_name(&self, name: &str) -> Option<&SurfaceType> {
        let folded = fold_identifier(name);
        self.surface.types.iter().find(|t| fold_identifier(&t.name) == folded)
    }

    fn is_coclass(ty: &SurfaceType) -> bool {
        matches!(ty.kind, SurfaceTypeKind::Coclass { .. })
    }

    /// A member of `ty` matching `name` and (optionally) `want` kind.
    fn find_member<'a>(
        ty: &'a SurfaceType,
        name: &str,
        want: Option<ProjectMemberKind>,
    ) -> Option<&'a SurfaceMember> {
        let folded = fold_identifier(name);
        // Prefer a member whose kind matches `want`; otherwise the first by name
        // (Get precedes Let/Set in synthesis order, so a read context picks Get).
        ty.members
            .iter()
            .find(|m| fold_identifier(&m.name) == folded && want.is_none_or(|k| k == m.member_kind))
            .or_else(|| ty.members.iter().find(|m| fold_identifier(&m.name) == folded))
    }

    fn member_binding(ty: &SurfaceType, m: &SurfaceMember) -> Binding {
        if Self::is_coclass(ty) {
            // Early-bound COM member — the VM dispatches the dispid in-image (WS-H).
            Binding {
                symbol: None,
                is_default: m.is_default,
                route: DispatchRoute::ComMember {
                    member_name: m.name.clone(),
                    dispid: m.dispid,
                    vtable_slot: m.vtable_slot,
                    invoke_kind: m.invoke_kind,
                    member_kind: m.member_kind,
                    is_default_member: m.is_default,
                    param_by_ref: m.parameter_types.iter().map(|t| t.is_by_ref()).collect(),
                },
            }
        } else {
            // Hidden-module / global function — devirtualizes to an in-image call.
            Binding {
                symbol: Some(m.symbol),
                is_default: m.is_default,
                route: DispatchRoute::ProjectMember { kind: m.member_kind },
            }
        }
    }

    fn const_binding(&self, name: &str) -> Option<Binding> {
        let folded = fold_identifier(name);
        self.surface
            .consts
            .iter()
            .find(|c| fold_identifier(&c.name) == folded)
            .map(|c| Binding::new(Some(c.symbol), DispatchRoute::Value))
    }

    /// Unqualified resolution: a global-namespace (hidden-module) member, then a
    /// global-namespace constant.
    fn resolve_global(&self, name: &str) -> Option<Binding> {
        for ty in self.surface.types.iter().filter(|t| t.global_namespace && !Self::is_coclass(t)) {
            if let Some(m) = Self::find_member(ty, name, None) {
                return Some(Self::member_binding(ty, m));
            }
        }
        self.const_binding(name)
    }

    fn resolve_type_member(&self, owner: &str, member: &str) -> Option<Binding> {
        let ty = self.type_by_name(owner)?;
        Self::find_member(ty, member, None).map(|m| Self::member_binding(ty, m))
    }
}

impl Provider for SurfaceProvider {
    fn resolve(&self, name: &str) -> Option<Binding> {
        self.resolve_global(name)
    }

    fn resolve_member(
        &self,
        recv: &VarTypeRef,
        name: &str,
        want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        let VarTypeRef::Object(type_name) = recv else {
            return None;
        };
        let ty = self.type_by_name(type_name)?;
        Self::find_member(ty, name, want).map(|m| Self::member_binding(ty, m))
    }

    fn resolve_qualified(&self, parts: &[&str]) -> Option<Binding> {
        match parts {
            [member] => self.resolve(member),
            [owner, member] => self.resolve_type_member(owner, member),
            [project, owner, member] => {
                if fold_identifier(project) == self.project_folded {
                    self.resolve_type_member(owner, member)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn resolve_default_member(&self, recv: &VarTypeRef) -> Option<Binding> {
        let VarTypeRef::Object(type_name) = recv else {
            return None;
        };
        let ty = self.type_by_name(type_name)?;
        ty.members
            .iter()
            .find(|m| m.is_default)
            .map(|m| Self::member_binding(ty, m))
    }
}
