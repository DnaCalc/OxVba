//! Resolves names against a *referenced project's* synthesized export surface
//! ([`crate::surface::ProjectExportSurface`]) — the complete published contract a
//! referrer binds against, exactly as if the referenced project were a separate
//! compiled assembly (its own bundle).
//!
//! Every published member resolves to one uniform [`DispatchRoute::ExternMember`]
//! that the binder lowers to a cross-bundle extern by context:
//!   * a **coclass** member (resolved on a typed object receiver) →
//!     `has_receiver: true`; the binder emits a `LateDispatch` by name, routed in
//!     the object's own bundle.
//!   * a **hidden-module** / global-namespace function (no receiver) →
//!     `has_receiver: false`; the binder registers a `BundleImport` and emits an
//!     `ExternProc` call.
//!   * a `Public Enum` member / `Public Const` → [`DispatchRoute::ConstValue`]
//!     carrying the published folded literal.
//!
//! `New Lib.Widget` resolves via [`Provider::resolve_extern_coclass`] → a
//! cross-bundle `NewExtern` against the referenced project's bundle.

use oxvba_bundle::ProjectMemberKind;

use crate::binding::{Binding, DispatchRoute};
use crate::model::fold_identifier;
use crate::provider::Provider;
use crate::signature::VarTypeRef;
use crate::surface::{
    MemberOrigin, ProjectExportSurface, SurfaceConst, SurfaceMember, SurfaceType, SurfaceTypeKind,
};

pub struct SurfaceProvider {
    surface: ProjectExportSurface,
    project_folded: String,
}

impl SurfaceProvider {
    pub fn new(surface: ProjectExportSurface) -> Self {
        let project_folded = fold_identifier(&surface.project_name);
        Self {
            surface,
            project_folded,
        }
    }

    fn type_by_name(&self, name: &str) -> Option<&SurfaceType> {
        let folded = fold_identifier(name);
        self.surface
            .types
            .iter()
            .find(|t| fold_identifier(&t.name) == folded)
    }

    fn is_coclass(ty: &SurfaceType) -> bool {
        matches!(ty.kind, SurfaceTypeKind::Coclass { .. })
    }

    /// A member of `ty` matching `name` and (optionally) `want` kind, restricted to
    /// members that are actually bindable across a bundle boundary.
    fn find_member<'a>(
        ty: &'a SurfaceType,
        name: &str,
        want: Option<ProjectMemberKind>,
    ) -> Option<&'a SurfaceMember> {
        let folded = fold_identifier(name);
        let bindable = |m: &&SurfaceMember| Self::is_bindable_cross_project(m);
        // Prefer a member whose kind matches `want`; otherwise the first by name
        // (Get precedes Let/Set in synthesis order, so a read context picks Get).
        ty.members
            .iter()
            .filter(bindable)
            .find(|m| fold_identifier(&m.name) == folded && want.is_none_or(|k| k == m.member_kind))
            .or_else(|| {
                ty.members
                    .iter()
                    .filter(bindable)
                    .find(|m| fold_identifier(&m.name) == folded)
            })
    }

    /// A field-backed surface member (a `Public` module variable, or a class field
    /// surfaced as a property pair) has no callable export — a module variable is a
    /// global, a class field is an instance slot — so it cannot be reached across a
    /// bundle boundary (an `ExternProc` import would find no export; a `LateDispatch`
    /// by name would find no method). Exclude it so a cross-project reference fails
    /// cleanly at bind time ("unresolved") rather than opaquely at link/run time.
    /// (Cross-bundle field/variable access needs synthesized accessor procs — TODO.)
    fn is_bindable_cross_project(m: &SurfaceMember) -> bool {
        m.origin != MemberOrigin::Field
    }

    /// One uniform cross-bundle route. A coclass member dispatches on its receiver
    /// (`has_receiver: true`); a hidden-module function is an import-backed call.
    fn member_binding(&self, ty: &SurfaceType, m: &SurfaceMember) -> Binding {
        Binding {
            symbol: None,
            is_default: m.is_default,
            route: DispatchRoute::ExternMember {
                unit: self.surface.project_name.clone(),
                owner: ty.name.clone(),
                member: m.name.clone(),
                kind: m.member_kind,
                param_types: m.parameter_types.clone(),
                param_names: m.parameter_names.clone(),
                has_receiver: Self::is_coclass(ty),
            },
        }
    }

    fn const_binding_of(c: &SurfaceConst) -> Binding {
        Binding::new(Some(c.symbol), DispatchRoute::ConstValue(c.value.clone()))
    }

    /// A `Public Const` / `Enum` member by bare name (unqualified).
    fn const_binding(&self, name: &str) -> Option<Binding> {
        let folded = fold_identifier(name);
        self.surface
            .consts
            .iter()
            .find(|c| fold_identifier(&c.name) == folded)
            .map(Self::const_binding_of)
    }

    /// An `Enum` member qualified by its enum name (`Color.Red`).
    fn enum_member_binding(&self, enum_name: &str, member: &str) -> Option<Binding> {
        let (enum_folded, member_folded) = (fold_identifier(enum_name), fold_identifier(member));
        self.surface
            .consts
            .iter()
            .find(|c| {
                fold_identifier(&c.name) == member_folded
                    && c.enum_name
                        .as_deref()
                        .is_some_and(|e| fold_identifier(e) == enum_folded)
            })
            .map(Self::const_binding_of)
    }

    /// Unqualified resolution: a global-namespace (hidden-module) member, then a
    /// global-namespace constant.
    fn resolve_global(&self, name: &str) -> Option<Binding> {
        for ty in self
            .surface
            .types
            .iter()
            .filter(|t| t.global_namespace && !Self::is_coclass(t))
        {
            if let Some(m) = Self::find_member(ty, name, None) {
                return Some(self.member_binding(ty, m));
            }
        }
        self.const_binding(name)
    }

    fn resolve_type_member(&self, owner: &str, member: &str) -> Option<Binding> {
        if let Some(ty) = self.type_by_name(owner)
            && let Some(m) = Self::find_member(ty, member, None)
        {
            return Some(self.member_binding(ty, m));
        }
        // `Owner` may be an enum name rather than a coclass/module type.
        self.enum_member_binding(owner, member)
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
        Self::find_member(ty, name, want).map(|m| self.member_binding(ty, m))
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
            .find(|m| m.is_default && Self::is_bindable_cross_project(m))
            .map(|m| self.member_binding(ty, m))
    }

    fn resolve_extern_coclass(&self, name: &str) -> Option<(String, String)> {
        // `New <name>` where `<name>` is a bare class (`Widget`) or project-qualified
        // (`Lib.Widget`). A leading segment matching this unit's name is stripped.
        let class = match name.split_once('.') {
            Some((project, class)) if fold_identifier(project) == self.project_folded => class,
            Some(_) => return None,
            None => name,
        };
        let ty = self.type_by_name(class)?;
        match ty.kind {
            SurfaceTypeKind::Coclass {
                creatable: true, ..
            } => Some((self.surface.project_name.clone(), ty.name.clone())),
            _ => None,
        }
    }

    fn resolve_extern_predeclared(&self, name: &str) -> Option<(String, String)> {
        // A bare class name (`ThisWorkbook`) or project-qualified (`Host.ThisWorkbook`)
        // naming a `VB_PredeclaredId` coclass → its singleton in this project's bundle.
        // `creatable` is irrelevant: a predeclared document class is typically
        // `VB_Creatable = False` yet still has a global instance.
        let class = match name.split_once('.') {
            Some((project, class)) if fold_identifier(project) == self.project_folded => class,
            Some(_) => return None,
            None => name,
        };
        let ty = self.type_by_name(class)?;
        match ty.kind {
            SurfaceTypeKind::Coclass { .. } if ty.predeclared => {
                Some((self.surface.project_name.clone(), ty.name.clone()))
            }
            _ => None,
        }
    }
}
