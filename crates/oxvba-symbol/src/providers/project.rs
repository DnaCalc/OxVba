//! Cross-module / referenced-project resolution. Builds an index over the
//! scanned modules (sibling + referenced) and resolves unqualified public names
//! and 1/2/3-part qualified names (`Module.Member`, `Project.Module.Member`).
//! A clean reimplementation of the legacy `frontend_project_symbols` index.

use std::collections::BTreeMap;

use oxvba_bundle::ProjectMemberKind;

use crate::binding::{Binding, DispatchRoute};
use crate::model::{SymbolId, SymbolImpl, SymbolKind, SymbolTable, Visibility, fold_identifier};
use crate::provider::Provider;
use crate::scanner::ModuleScan;
use crate::signature::VarTypeRef;

#[derive(Debug, Clone, Copy)]
struct MemberEntry {
    symbol: SymbolId,
    owner: SymbolId,
    kind: SymbolKind,
    visibility: Visibility,
    accessors: PropertyAccessors,
}

#[derive(Debug, Clone, Copy, Default)]
struct PropertyAccessors {
    get: bool,
    let_: bool,
    set: bool,
}

#[derive(Default)]
pub struct ProjectProvider {
    /// Folded active project name accepted by `Project.Module.Member` lookup.
    project_folded: String,
    /// Public module-level names across all modules → candidates.
    public: BTreeMap<String, Vec<MemberEntry>>,
    /// module(folded) → (member(folded) → candidates).
    modules: BTreeMap<String, BTreeMap<String, Vec<MemberEntry>>>,
    /// enum(folded) → (member(folded) → candidates).
    enums: BTreeMap<String, BTreeMap<String, Vec<MemberEntry>>>,
    /// module(folded) → its default member (`VB_UserMemId = 0`).
    default_members: BTreeMap<String, MemberEntry>,
}

impl ProjectProvider {
    pub fn build(
        project_name: &str,
        symbols: &SymbolTable,
        active: &[ModuleScan],
        referenced: &[ModuleScan],
    ) -> Self {
        let mut provider = ProjectProvider {
            project_folded: fold_identifier(project_name),
            ..ProjectProvider::default()
        };
        for scan in active.iter().chain(referenced.iter()) {
            let module_key = fold_identifier(&scan.module_name);
            let module_entry = provider.modules.entry(module_key.clone()).or_default();
            for member in &scan.members {
                let entry = MemberEntry {
                    symbol: member.symbol,
                    owner: scan.module_symbol,
                    kind: member.kind,
                    visibility: member.visibility,
                    accessors: property_accessors(symbols, member.symbol, member.kind),
                };
                module_entry
                    .entry(member.name_folded.clone())
                    .or_default()
                    .push(entry);
                if scan.exposes_unqualified_members && is_project_public_member(entry) {
                    provider
                        .public
                        .entry(member.name_folded.clone())
                        .or_default()
                        .push(entry);
                }
                if member.is_default {
                    provider
                        .default_members
                        .entry(module_key.clone())
                        .or_insert(entry);
                }
                if let Some(enum_name) = &member.enum_name {
                    provider
                        .enums
                        .entry(fold_identifier(enum_name))
                        .or_default()
                        .entry(member.name_folded.clone())
                        .or_default()
                        .push(entry);
                }
            }
        }
        provider
    }

    /// Resolve a 1/2/3-part qualified name.
    pub fn resolve_qualified(&self, parts: &[&str]) -> Option<Binding> {
        match parts {
            [member] => self.resolve(member),
            [owner, member] => self.resolve_public_owner_member(owner, member, None),
            [project, owner, member] if fold_identifier(project) == self.project_folded => {
                self.resolve_public_owner_member(owner, member, None)
            }
            _ => None,
        }
    }

    fn resolve_owner_member(
        &self,
        owner: &str,
        member: &str,
        want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        let owner = fold_identifier(owner);
        let member = fold_identifier(member);
        let candidates = self
            .modules
            .get(&owner)
            .and_then(|module| module.get(&member))
            .or_else(|| self.enums.get(&owner).and_then(|enum_| enum_.get(&member)))?;
        candidates
            .iter()
            .copied()
            .find(|entry| supports_want(*entry, want))
            .map(|entry| binding_for_want(entry, want))
    }

    fn resolve_public_owner_member(
        &self,
        owner: &str,
        member: &str,
        want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        let owner = fold_identifier(owner);
        let member = fold_identifier(member);
        let candidates = self
            .modules
            .get(&owner)
            .and_then(|module| module.get(&member))
            .or_else(|| self.enums.get(&owner).and_then(|enum_| enum_.get(&member)))?;
        candidates
            .iter()
            .copied()
            .find(|entry| is_project_public_member(*entry) && supports_want(*entry, want))
            .map(|entry| binding_for_want(entry, want))
    }
}

impl Provider for ProjectProvider {
    fn resolve(&self, name: &str) -> Option<Binding> {
        let candidates = self.public.get(&fold_identifier(name))?;
        if has_competing_owners(candidates) {
            return None;
        }
        candidates
            .iter()
            .copied()
            .find(|entry| supports_want(*entry, None))
            .map(|entry| binding_for_want(entry, None))
    }

    fn has_ambiguous_unqualified_name(&self, name: &str) -> bool {
        self.public
            .get(&fold_identifier(name))
            .is_some_and(|candidates| has_competing_owners(candidates))
    }

    fn resolve_qualified(&self, parts: &[&str]) -> Option<Binding> {
        ProjectProvider::resolve_qualified(self, parts)
    }

    fn resolve_member(
        &self,
        recv: &VarTypeRef,
        name: &str,
        want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        // A typed project-class/module receiver: resolve `recv.member`.
        let VarTypeRef::Object(type_name) = recv else {
            return None;
        };
        self.resolve_owner_member(type_name, name, want)
    }

    fn resolve_default_member(&self, recv: &VarTypeRef) -> Option<Binding> {
        self.resolve_default_member_kind(recv, None)
    }

    fn resolve_default_member_kind(
        &self,
        recv: &VarTypeRef,
        want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        let VarTypeRef::Object(type_name) = recv else {
            return None;
        };
        let entry = *self.default_members.get(&fold_identifier(type_name))?;
        if !supports_want(entry, want) {
            return None;
        }
        let mut binding = binding_for_want(entry, want);
        binding.is_default = true;
        Some(binding)
    }
}

fn is_project_public_member(entry: MemberEntry) -> bool {
    entry.visibility == Visibility::Public && is_public_member_kind(entry.kind)
}

fn has_competing_owners(candidates: &[MemberEntry]) -> bool {
    let Some(first) = candidates.first() else {
        return false;
    };
    candidates
        .iter()
        .skip(1)
        .any(|candidate| candidate.owner != first.owner)
}

fn is_public_member_kind(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Procedure
            | SymbolKind::Function
            | SymbolKind::Property
            | SymbolKind::Type
            | SymbolKind::Enum
            | SymbolKind::EnumMember
            | SymbolKind::Const
            | SymbolKind::Field
            | SymbolKind::WithEventsField
            | SymbolKind::Event
    )
}

fn binding_for(entry: MemberEntry) -> Binding {
    let route = match entry.kind {
        SymbolKind::Procedure | SymbolKind::Function | SymbolKind::Event => {
            DispatchRoute::ProjectMember {
                kind: ProjectMemberKind::Method,
            }
        }
        // Default (read) accessor; the binder picks Let/Set from the group.
        SymbolKind::Property => DispatchRoute::ProjectMember {
            kind: ProjectMemberKind::PropertyGet,
        },
        _ => DispatchRoute::Value,
    };
    Binding::new(Some(entry.symbol), route)
}

fn binding_for_want(entry: MemberEntry, want: Option<ProjectMemberKind>) -> Binding {
    let mut binding = binding_for(entry);
    if entry.kind == SymbolKind::Property
        && let Some(kind) = want
        && let DispatchRoute::ProjectMember { kind: have } = &mut binding.route
    {
        *have = kind;
    }
    binding
}

fn supports_want(entry: MemberEntry, want: Option<ProjectMemberKind>) -> bool {
    match (entry.kind, want) {
        (SymbolKind::Property, None) => {
            entry.accessors.get || entry.accessors.let_ || entry.accessors.set
        }
        (SymbolKind::Property, Some(ProjectMemberKind::PropertyGet)) => entry.accessors.get,
        (SymbolKind::Property, Some(ProjectMemberKind::PropertyLet)) => entry.accessors.let_,
        (SymbolKind::Property, Some(ProjectMemberKind::PropertySet)) => entry.accessors.set,
        (SymbolKind::Property, Some(ProjectMemberKind::Method)) => false,
        (_, _) => true,
    }
}

fn property_accessors(
    symbols: &SymbolTable,
    symbol: SymbolId,
    kind: SymbolKind,
) -> PropertyAccessors {
    if kind != SymbolKind::Property {
        return PropertyAccessors::default();
    }
    match symbols.symbol(symbol).map(|sym| &sym.imp) {
        Some(SymbolImpl::Property(group)) => PropertyAccessors {
            get: group.get.is_some(),
            let_: group.let_.is_some(),
            set: group.set.is_some(),
        },
        _ => PropertyAccessors::default(),
    }
}
