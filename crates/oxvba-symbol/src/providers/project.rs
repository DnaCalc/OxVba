//! Cross-module / referenced-project resolution. Builds an index over the
//! scanned modules (sibling + referenced) and resolves unqualified public names
//! and 1/2/3-part qualified names (`Module.Member`, `Project.Module.Member`).
//! A clean reimplementation of the legacy `frontend_project_symbols` index.

use std::collections::BTreeMap;

use oxvba_bundle::ProjectMemberKind;

use crate::binding::{Binding, DispatchRoute};
use crate::model::{fold_identifier, SymbolId, SymbolKind, Visibility};
use crate::provider::Provider;
use crate::scanner::ModuleScan;
use crate::signature::VarTypeRef;

#[derive(Debug, Clone, Copy)]
struct MemberEntry {
    symbol: SymbolId,
    kind: SymbolKind,
    visibility: Visibility,
}

#[derive(Default)]
pub struct ProjectProvider {
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
        _symbols: &crate::model::SymbolTable,
        active: &[ModuleScan],
        referenced: &[ModuleScan],
    ) -> Self {
        let mut provider = ProjectProvider::default();
        for scan in active.iter().chain(referenced.iter()) {
            let module_key = fold_identifier(&scan.module_name);
            let module_entry = provider.modules.entry(module_key.clone()).or_default();
            for member in &scan.members {
                let entry = MemberEntry {
                    symbol: member.symbol,
                    kind: member.kind,
                    visibility: member.visibility,
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
            [owner, member] => self.resolve_public_owner_member(owner, member),
            [_project, owner, member] => self.resolve_public_owner_member(owner, member),
            _ => None,
        }
    }

    fn resolve_owner_member(&self, owner: &str, member: &str) -> Option<Binding> {
        let owner = fold_identifier(owner);
        let member = fold_identifier(member);
        let candidates = self
            .modules
            .get(&owner)
            .and_then(|module| module.get(&member))
            .or_else(|| self.enums.get(&owner).and_then(|enum_| enum_.get(&member)))?;
        candidates.first().map(|entry| binding_for(*entry))
    }

    fn resolve_public_owner_member(&self, owner: &str, member: &str) -> Option<Binding> {
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
            .find(|entry| is_project_public_member(*entry))
            .map(binding_for)
    }
}

impl Provider for ProjectProvider {
    fn resolve(&self, name: &str) -> Option<Binding> {
        let candidates = self.public.get(&fold_identifier(name))?;
        candidates.first().map(|entry| binding_for(*entry))
    }

    fn resolve_qualified(&self, parts: &[&str]) -> Option<Binding> {
        ProjectProvider::resolve_qualified(self, parts)
    }

    fn resolve_member(
        &self,
        recv: &VarTypeRef,
        name: &str,
        _want: Option<ProjectMemberKind>,
    ) -> Option<Binding> {
        // A typed project-class/module receiver: resolve `recv.member`.
        let VarTypeRef::Object(type_name) = recv else {
            return None;
        };
        self.resolve_owner_member(type_name, name)
    }

    fn resolve_default_member(&self, recv: &VarTypeRef) -> Option<Binding> {
        let VarTypeRef::Object(type_name) = recv else {
            return None;
        };
        let entry = self.default_members.get(&fold_identifier(type_name))?;
        let mut binding = binding_for(*entry);
        binding.is_default = true;
        Some(binding)
    }
}

fn is_project_public_member(entry: MemberEntry) -> bool {
    entry.visibility == Visibility::Public && is_public_member_kind(entry.kind)
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
