//! ID allocation: map symbol-model `SymbolId`s to symbol-free coreir ids
//! (`GlobalId`/`ProcId`/`LocalId`) and build each `CoreProc` frame skeleton.
//!
//! This replays the scanner's deterministic scope/symbol order. The key link:
//! the scanner creates one `Procedure` scope per top-level proc decl, in source
//! order, so the i-th top-level proc decl in a module's CST corresponds to the
//! i-th `Procedure` scope under that module's scope. We zip them to attach a
//! `ProcId` + frame to each decl, then the binder fills the body afterward.

use std::collections::HashMap;

use oxvba_bundle::coreir::{CoreGlobal, CoreLocal, CoreParam, GlobalId, LocalId, ProcId};
use oxvba_bundle::{ProcedureKind, ProjectMemberKind};
use oxvba_symbol::manifest::{ModuleKind, SymbolProjectManifest};
use oxvba_symbol::model::{
    fold_identifier, ScopeId, ScopeKind, SymbolId, SymbolImpl, SymbolKind, SymbolNamespace,
};
use oxvba_symbol::provider::ResolutionEnvironment;
use oxvba_symbol::signature::{PassingMode, Signature, VarTypeRef};
use oxvba_syntax::{SyntaxKind, SyntaxNode};

use crate::error::BindError;
use crate::types;

/// One procedure's frame skeleton + the `SymbolId`→`LocalId` map for its scope.
pub struct ProcInfo {
    pub proc_id: ProcId,
    pub name: String,
    pub kind: ProcedureKind,
    /// The symbol-model `Procedure` scope holding this proc's params + locals.
    pub proc_scope: ScopeId,
    pub params: Vec<CoreParam>,
    pub locals: Vec<CoreLocal>,
    pub return_local: Option<LocalId>,
    /// The function/property-get return type (for coercing the return assignment).
    pub return_type: VarTypeRef,
    /// Parameter/Local `SymbolId` → its frame slot.
    pub local_of: HashMap<SymbolId, LocalId>,
    /// The class module this proc belongs to (for `Me`), if any. Consumed by the
    /// objects/COM phase.
    #[allow(dead_code)]
    pub class_name: Option<String>,
}

/// The whole project's id allocation: globals, procs, and the symbol→id maps the
/// expression/call binders consult.
pub struct IdAllocator {
    pub globals: Vec<CoreGlobal>,
    pub procs: Vec<ProcInfo>,
    pub global_of: HashMap<SymbolId, GlobalId>,
    pub proc_of: HashMap<SymbolId, ProcId>,
    pub prop_accessor_of: HashMap<(SymbolId, ProjectMemberKind), ProcId>,
}

impl IdAllocator {
    pub fn build(
        env: &ResolutionEnvironment,
        manifest: &SymbolProjectManifest,
    ) -> Result<Self, BindError> {
        let symbols = &env.symbols;
        let mut alloc = IdAllocator {
            globals: Vec::new(),
            procs: Vec::new(),
            global_of: HashMap::new(),
            proc_of: HashMap::new(),
            prop_accessor_of: HashMap::new(),
        };

        // 1) Globals — active-module `Field`/`WithEventsField`, in declaration order.
        for module in env.modules() {
            for sym_id in symbols.symbols_in_scope(module.module_scope)? {
                let sym = symbols.symbol(sym_id).expect("symbol in scope");
                if matches!(sym.kind, SymbolKind::Field | SymbolKind::WithEventsField) {
                    let gid = GlobalId(alloc.globals.len());
                    let array_element = match &sym.imp {
                        SymbolImpl::DeclaredType(t) => types::array_element(t),
                        _ => None,
                    };
                    alloc.globals.push(CoreGlobal { name: alloc_name(env, sym.name), array_element });
                    alloc.global_of.insert(sym_id, gid);
                }
            }
        }

        // 2) Procs — per active module, zip top-level proc decls with the module's
        //    `Procedure` scopes (both in source order).
        for module in env.modules() {
            let class_name = class_name_for(manifest, module.module_name);
            let proc_scopes: Vec<ScopeId> = symbols
                .scopes()
                .iter()
                .filter(|s| s.kind == ScopeKind::Procedure && s.parent == Some(module.module_scope))
                .map(|s| s.id)
                .collect();
            let decls: Vec<SyntaxNode> = top_level_proc_decls(module.syntax);
            // The scanner creates one Procedure scope per named proc decl, in source
            // order (both gated on `proc_name_token`). If these ever desync, a body
            // would be bound against the wrong frame — fail loudly instead.
            if decls.len() != proc_scopes.len() {
                return Err(BindError::Malformed(format!(
                    "module `{}`: {} proc decls but {} procedure scopes",
                    module.module_name,
                    decls.len(),
                    proc_scopes.len()
                )));
            }
            for (decl, &proc_scope) in decls.iter().zip(proc_scopes.iter()) {
                alloc.alloc_proc(env, module.module_scope, *decl, proc_scope, class_name.clone())?;
            }
        }

        Ok(alloc)
    }

    fn alloc_proc(
        &mut self,
        env: &ResolutionEnvironment,
        module_scope: ScopeId,
        decl: SyntaxNode<'_>,
        proc_scope: ScopeId,
        class_name: Option<String>,
    ) -> Result<(), BindError> {
        let symbols = &env.symbols;
        let proc_id = ProcId(self.procs.len());
        let logical = match decl.proc_name_token() {
            // Strip brackets to match the scanner's `normalize_identifier_token`
            // (find_in_scope folds case internally); preserve original case.
            Some(t) => t.text.trim_start_matches('[').trim_end_matches(']').to_string(),
            None => return Ok(()),
        };
        let member_kind = match decl.kind() {
            SyntaxKind::PropertyDecl => Some(property_accessor_kind(decl)),
            _ => None,
        };
        let kind = match (decl.kind(), member_kind) {
            (SyntaxKind::FunctionDecl, _) => ProcedureKind::Function,
            (SyntaxKind::PropertyDecl, Some(ProjectMemberKind::PropertyGet)) => ProcedureKind::PropertyGet,
            (SyntaxKind::PropertyDecl, Some(ProjectMemberKind::PropertyLet)) => ProcedureKind::PropertyLet,
            (SyntaxKind::PropertyDecl, Some(ProjectMemberKind::PropertySet)) => ProcedureKind::PropertySet,
            _ => ProcedureKind::Sub,
        };

        // Map proc symbol → proc id (project-member resolution targets this).
        let proc_sym = symbols.find_in_scope(module_scope, SymbolNamespace::Procedure, &logical)?;
        if let Some(sym) = proc_sym {
            match member_kind {
                Some(mk) => {
                    self.prop_accessor_of.insert((sym, mk), proc_id);
                }
                None => {
                    self.proc_of.insert(sym, proc_id);
                }
            }
        }

        // The signature carries the parameter passing modes for the frame.
        let signature = proc_signature(env, proc_sym, member_kind);
        let return_type = signature
            .as_ref()
            .and_then(|s| s.return_type.clone())
            .unwrap_or(VarTypeRef::Variant);
        let (params, locals, return_local, local_of) =
            build_frame(env, signature.as_ref(), proc_scope, kind, &logical);

        self.procs.push(ProcInfo {
            proc_id,
            name: logical,
            kind,
            proc_scope,
            params,
            locals,
            return_local,
            return_type,
            local_of,
            class_name,
        });
        Ok(())
    }

    /// The `ProcId` of the entry point: case-insensitive `Main`, if present.
    pub fn entry(&self) -> Option<ProcId> {
        let main = fold_identifier("Main");
        self.procs
            .iter()
            .find(|p| fold_identifier(&p.name) == main)
            .map(|p| p.proc_id)
    }
}

fn build_frame(
    env: &ResolutionEnvironment,
    signature: Option<&Signature>,
    proc_scope: ScopeId,
    kind: ProcedureKind,
    logical: &str,
) -> (Vec<CoreParam>, Vec<CoreLocal>, Option<LocalId>, HashMap<SymbolId, LocalId>) {
    let symbols = &env.symbols;
    let scope_syms = symbols.symbols_in_scope(proc_scope).unwrap_or_default();
    let mut params = Vec::new();
    let mut locals = Vec::new();
    let mut local_of = HashMap::new();
    let mut next = 0usize;

    // Parameters first (declaration order), pairing with the signature for `by_ref`.
    let mut param_index = 0usize;
    for &sym_id in &scope_syms {
        let sym = symbols.symbol(sym_id).expect("symbol in scope");
        if sym.namespace == SymbolNamespace::Parameter {
            let by_ref = signature
                .and_then(|s| s.params.get(param_index))
                .map(|p| p.mode == PassingMode::ByRef)
                .unwrap_or(true);
            params.push(CoreParam { name: alloc_name(env, sym.name), by_ref });
            local_of.insert(sym_id, LocalId(next));
            next += 1;
            param_index += 1;
        }
    }

    // Then locals (declaration order, block scoping already flattened).
    for &sym_id in &scope_syms {
        let sym = symbols.symbol(sym_id).expect("symbol in scope");
        if sym.namespace == SymbolNamespace::Local {
            let array_element = match &sym.imp {
                SymbolImpl::DeclaredType(t) => types::array_element(t),
                _ => None,
            };
            locals.push(CoreLocal { name: alloc_name(env, sym.name), array_element });
            local_of.insert(sym_id, LocalId(next));
            next += 1;
        }
    }

    // The synthetic return local for a Function / Property Get.
    let return_local = if matches!(kind, ProcedureKind::Function | ProcedureKind::PropertyGet) {
        let id = LocalId(next);
        locals.push(CoreLocal { name: logical.to_string(), array_element: None });
        Some(id)
    } else {
        None
    };

    (params, locals, return_local, local_of)
}

fn proc_signature(
    env: &ResolutionEnvironment,
    proc_sym: Option<SymbolId>,
    member_kind: Option<ProjectMemberKind>,
) -> Option<Signature> {
    let sym = proc_sym?;
    let imp = &env.symbols.symbol(sym)?.imp;
    let sig_id = match (imp, member_kind) {
        (SymbolImpl::Signature(id), None) => Some(*id),
        (SymbolImpl::Property(group), Some(mk)) => match mk {
            ProjectMemberKind::PropertyGet => group.get,
            ProjectMemberKind::PropertyLet => group.let_,
            ProjectMemberKind::PropertySet => group.set,
            ProjectMemberKind::Method => None,
        },
        _ => None,
    }?;
    env.signatures.get(sig_id).cloned()
}

fn alloc_name(env: &ResolutionEnvironment, name: oxvba_symbol::model::InternedNameId) -> String {
    env.symbols
        .name(name)
        .map(|n| n.first_spelling.clone())
        .unwrap_or_default()
}

fn top_level_proc_decls(root: SyntaxNode<'_>) -> Vec<SyntaxNode<'_>> {
    root.child_nodes()
        .into_iter()
        .filter(|n| {
            matches!(
                n.kind(),
                SyntaxKind::SubDecl | SyntaxKind::FunctionDecl | SyntaxKind::PropertyDecl
            ) && n.proc_name_token().is_some()
        })
        .collect()
}

fn property_accessor_kind(decl: SyntaxNode<'_>) -> ProjectMemberKind {
    for t in decl.child_tokens() {
        match t.kind {
            SyntaxKind::KwGet => return ProjectMemberKind::PropertyGet,
            SyntaxKind::KwLet => return ProjectMemberKind::PropertyLet,
            SyntaxKind::KwSet => return ProjectMemberKind::PropertySet,
            _ => {}
        }
    }
    ProjectMemberKind::PropertyGet
}

fn class_name_for(manifest: &SymbolProjectManifest, module_name: &str) -> Option<String> {
    let folded = fold_identifier(module_name);
    manifest.modules.iter().find_map(|m| {
        let name = if m.attributes.vb_name.is_empty() { &m.module_name } else { &m.attributes.vb_name };
        (fold_identifier(name) == folded && m.module_kind == ModuleKind::Class).then(|| name.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_symbol::build_resolution_environment;
    use oxvba_symbol::manifest::{ModuleAttributes, ModuleUnit, ProjectKind};
    use oxvba_symbol::provider::TypeLibResolver;
    use std::collections::BTreeMap;

    struct NullTypeLibs;
    impl TypeLibResolver for NullTypeLibs {
        fn resolve(
            &self,
            _request: &oxvba_com::TypeLibResolveRequest,
        ) -> Option<oxvba_com::TypeLibMetadataBlob> {
            None
        }
    }

    fn manifest(source: &str) -> SymbolProjectManifest {
        SymbolProjectManifest {
            project_name: "Proj".into(),
            project_kind: ProjectKind::Source,
            modules: vec![ModuleUnit {
                module_name: "Mod1".into(),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes::named("Mod1"),
                source: source.into(),
            }],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        }
    }

    #[test]
    fn allocates_globals_procs_and_frames() {
        let src = "Public total As Long\n\n\
                   Sub Main()\n    Dim x As Long\n    x = Add(2, 3)\nEnd Sub\n\n\
                   Function Add(a As Long, b As Long) As Long\n    Add = a + b\nEnd Function\n";
        let env = build_resolution_environment(&manifest(src), &NullTypeLibs).unwrap();
        let alloc = IdAllocator::build(&env, &manifest(src)).unwrap();

        assert_eq!(alloc.globals.len(), 1);
        assert_eq!(alloc.globals[0].name, "total");
        assert_eq!(alloc.global_of.len(), 1);

        assert_eq!(alloc.procs.len(), 2);
        let main = &alloc.procs[0];
        assert_eq!(main.name, "Main");
        assert_eq!(main.kind, ProcedureKind::Sub);
        assert!(main.params.is_empty());
        assert_eq!(main.locals.len(), 1); // x
        assert_eq!(main.locals[0].name, "x");
        assert!(main.return_local.is_none());

        let add = &alloc.procs[1];
        assert_eq!(add.name, "Add");
        assert_eq!(add.kind, ProcedureKind::Function);
        assert_eq!(add.params.len(), 2);
        assert!(add.params[0].by_ref); // VBA params default to ByRef
        // params occupy slots 0,1; the synthetic return local is slot 2.
        assert_eq!(add.return_local, Some(LocalId(2)));
        assert_eq!(add.locals.len(), 1); // just the return local
        assert_eq!(add.locals[0].name, "Add");

        assert_eq!(alloc.entry(), Some(ProcId(0)));
        assert_eq!(alloc.proc_of.len(), 2);
    }
}
