//! `ModuleScanner` — walk a module's lossless CST and declare its symbols
//! (procedures with signatures, properties, events, `Declare`s, types/enums,
//! fields/consts) into the symbol table. One code path, used for the active
//! module and every sibling / referenced-project module. Ported from the legacy
//! `frontend_symbols` CST collection, signature-aware (no text-parsing fallback).

use std::collections::BTreeSet;

use oxvba_bundle::{DeclareParamType, coreir::CoreConst};
use oxvba_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

use crate::cond_comp::ConditionalCompilationTarget;
use crate::manifest::{ModuleKind, ModuleUnit};
use crate::model::{
    PropertyGroup, ScopeId, SourceProvenance, SourceSpan, SymbolId, SymbolImpl, SymbolKind,
    SymbolModelError, SymbolNamespace, SymbolTable, Visibility, fold_identifier,
};
use crate::providers::declare::{CallConv, DeclareSymbol};
use crate::signature::SignatureId;
use crate::signature::{
    BuiltinType, CallShape, DefaultValue, Param, PassingMode, Signature, SignatureTable, VarTypeRef,
};

/// One module's declared surface, for the project index.
#[derive(Debug, Clone)]
pub struct ModuleScan {
    pub module_name: String,
    pub module_scope: ScopeId,
    /// The module's own `SymbolKind::Module` symbol (declared in the project
    /// scope). Used as a class's stable identity in the export surface, so the
    /// binder keys `New <coclass>` off the same symbol.
    pub module_symbol: SymbolId,
    pub members: Vec<ScannedMember>,
    /// Interface display names from this module's `Implements` clauses (bare type
    /// names, source order). Published per coclass in the export surface so a
    /// referrer mangles dispatch on an interface-typed receiver.
    pub implements: Vec<String>,
    /// Source-owned `Option Private Module`. The project loader also projects this
    /// into `ModuleAttributes`; keeping it here makes direct symbol/binder
    /// manifests obey the directive without relying on loader preprocessing.
    pub option_private_module: bool,
    /// Source-owned exported module attributes. The project loader also projects
    /// these into `ModuleAttributes`; keeping the source facts here makes direct
    /// symbol/binder manifests follow exported `.bas`/`.cls` headers.
    pub source_attributes: ScannedModuleAttributes,
    /// Public members from standard modules, or modules explicitly marked as a
    /// global namespace, participate in bare project-name lookup. Public class,
    /// document, and form members otherwise require a receiver.
    pub exposes_unqualified_members: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ScannedModuleAttributes {
    pub vb_name: Option<String>,
    pub vb_global_namespace: Option<bool>,
    pub vb_creatable: Option<bool>,
    pub vb_predeclared_id: Option<bool>,
    pub vb_exposed: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ScannedMember {
    /// The logical member name, folded.
    pub name_folded: String,
    pub symbol: SymbolId,
    pub kind: SymbolKind,
    pub namespace: SymbolNamespace,
    /// `Attribute VB_UserMemId = 0` — this is the type's default member.
    pub is_default: bool,
    /// `Attribute VB_UserMemId = -4` — this is the type's NewEnum member.
    pub is_enumerator: bool,
    /// Declared visibility (drives cross-project / COM export-surface exposure).
    pub visibility: Visibility,
    /// For a `SymbolKind::EnumMember`, the display name of its containing `Enum`
    /// (so the surface can publish `EnumName.Member` qualified resolution). `None`
    /// for every other member kind.
    pub enum_name: Option<String>,
}

/// Scan `module`'s pre-parsed CST into a fresh module scope under `project_scope`.
///
/// The caller parses (once) and checks `Parse::errors()` before calling — the CST
/// is parsed a single time and shared with the binder, never re-parsed here.
pub fn scan_module(
    symbols: &mut SymbolTable,
    signatures: &mut SignatureTable,
    next_descriptor_id: &mut u32,
    module: &ModuleUnit,
    module_syntax: SyntaxNode<'_>,
    project_scope: ScopeId,
    target: ConditionalCompilationTarget,
) -> Result<ModuleScan, SymbolModelError> {
    let source_attributes = source_module_attributes(module_syntax);
    reject_unsupported_declared_decimal_storage(module_syntax)?;
    reject_unsupported_option_compare_database(module_syntax)?;
    let default_types = module_default_types(module_syntax)?;
    let module_name = source_attributes
        .vb_name
        .clone()
        .or_else(|| {
            (!module.attributes.vb_name.is_empty()).then(|| module.attributes.vb_name.clone())
        })
        .unwrap_or_else(|| module.module_name.clone());
    let exposes_unqualified_members = module.module_kind == ModuleKind::Procedural
        || source_attributes
            .vb_global_namespace
            .unwrap_or(module.attributes.vb_global_namespace);
    let module_symbol = symbols.declare_symbol(
        project_scope,
        SymbolNamespace::Module,
        SymbolKind::Module,
        &module_name,
        SourceProvenance {
            module_name: Some(module_name.clone()),
            span: None,
        },
        SymbolImpl::None,
    )?;
    let module_scope = symbols.add_scope(
        crate::model::ScopeKind::Module,
        project_scope,
        Some(&module_name),
    )?;

    let mut scan = ModuleScan {
        module_name: module_name.clone(),
        module_scope,
        module_symbol,
        members: Vec::new(),
        implements: Vec::new(),
        option_private_module: module.attributes.option_private_module
            || option_private_module(module_syntax),
        source_attributes,
        exposes_unqualified_members,
    };
    let default_member_attrs = member_attributes_with_user_mem_id(module_syntax, 0);
    let enumerator_member_attrs = member_attributes_with_user_mem_id(module_syntax, -4);
    let mut ctx = ScanCtx {
        symbols,
        signatures,
        next_descriptor_id,
        scan: &mut scan,
        module_name: &module_name,
        module_kind: module.module_kind,
        default_member_attrs,
        enumerator_member_attrs,
        default_types,
        target,
        proc_is_static: false,
    };
    ctx.walk(module_scope, module_syntax, true)?;
    Ok(scan)
}

struct ScanCtx<'a> {
    symbols: &'a mut SymbolTable,
    signatures: &'a mut SignatureTable,
    next_descriptor_id: &'a mut u32,
    scan: &'a mut ModuleScan,
    module_name: &'a str,
    module_kind: ModuleKind,
    default_member_attrs: BTreeSet<String>,
    enumerator_member_attrs: BTreeSet<String>,
    default_types: DefaultTypeTable,
    target: ConditionalCompilationTarget,
    /// Set while walking the body of a `Static Sub/Function/Property`, so every
    /// proc-local declarator becomes a `StaticLocal` even without its own
    /// `Static` keyword. VBA has no nested procedures, so a single flag (no
    /// stack) suffices.
    proc_is_static: bool,
}

impl ScanCtx<'_> {
    fn walk(
        &mut self,
        scope: ScopeId,
        node: SyntaxNode<'_>,
        module_level: bool,
    ) -> Result<(), SymbolModelError> {
        match node.kind() {
            SyntaxKind::SubDecl | SyntaxKind::FunctionDecl | SyntaxKind::PropertyDecl => {
                self.scan_procedure(scope, node, module_level)?;
                return Ok(());
            }
            SyntaxKind::DeclareStmt => {
                self.scan_declare(scope, node, module_level)?;
            }
            SyntaxKind::EventDecl => {
                if let Some(token) = first_identifier_token(node) {
                    let name = normalize_identifier_token(token.text);
                    self.validate_event_declaration(node, name)?;
                    self.validate_parameter_declaration(node, name)?;
                    let sig = self
                        .signatures
                        .alloc(self.build_signature(node, CallShape::EventRaise));
                    self.declare(
                        scope,
                        SymbolNamespace::Member,
                        SymbolKind::Event,
                        name,
                        token,
                        SymbolImpl::Signature(sig),
                        module_level,
                        decl_visibility(node, Visibility::Public),
                        None,
                    )?;
                }
            }
            SyntaxKind::TypeBlock | SyntaxKind::EnumBlock => {
                // `Type`/`Enum` default to Public; the members of an enum inherit the
                // enum's visibility.
                let vis = decl_visibility(node, Visibility::Public);
                if let Some(token) = first_identifier_token(node) {
                    let name = normalize_identifier_token(token.text);
                    let kind = if node.kind() == SyntaxKind::EnumBlock {
                        SymbolKind::Enum
                    } else {
                        SymbolKind::Type
                    };
                    self.declare(
                        scope,
                        SymbolNamespace::Type,
                        kind,
                        name,
                        token,
                        SymbolImpl::None,
                        module_level,
                        vis,
                        None,
                    )?;
                }
                if node.kind() == SyntaxKind::EnumBlock {
                    let enum_name = first_identifier_token(node)
                        .map(|t| normalize_identifier_token(t.text).to_string());
                    for member in node.enum_members() {
                        if let Some(token) = member.declarator_name() {
                            let name = normalize_identifier_token(token.text);
                            self.declare(
                                scope,
                                SymbolNamespace::Local,
                                SymbolKind::EnumMember,
                                name,
                                token,
                                SymbolImpl::None,
                                module_level,
                                vis,
                                enum_name.as_deref(),
                            )?;
                        }
                    }
                }
            }
            SyntaxKind::DimStmt | SyntaxKind::ConstStmt => {
                let is_const = node.kind() == SyntaxKind::ConstStmt;
                // Module-level `Dim`/module variables and `Const`s default to Private;
                // an explicit `Public` exposes them. The modifier sits on the statement,
                // so it applies to every declarator.
                let vis = decl_visibility(node, Visibility::Private);
                for declarator in node.declarators() {
                    let Some(token) = declarator.declarator_name() else {
                        continue;
                    };
                    let name = normalize_identifier_token(token.text);
                    let declared_type =
                        declared_var_type_with_default(declarator, &self.default_types, !is_const);
                    if is_const && declarator.is_with_events() {
                        return Err(SymbolModelError::InvalidWithEventsConstDeclaration {
                            name: name.to_string(),
                        });
                    }
                    if !module_level && !is_const && declarator.is_with_events() {
                        return Err(SymbolModelError::WithEventsNotValidInLocalScope {
                            name: name.to_string(),
                        });
                    }
                    if module_level
                        && !is_const
                        && declarator.is_with_events()
                        && self.module_kind == ModuleKind::Procedural
                    {
                        return Err(SymbolModelError::WithEventsNotValidInStandardModule {
                            name: name.to_string(),
                        });
                    }
                    if module_level
                        && !is_const
                        && declarator.is_with_events()
                        && declarator.is_new()
                    {
                        return Err(SymbolModelError::InvalidWithEventsAutoInstantiation {
                            name: name.to_string(),
                        });
                    }
                    if module_level
                        && !is_const
                        && declarator.is_with_events()
                        && !withevents_field_type_compatible(&declared_type)
                    {
                        return Err(SymbolModelError::InvalidWithEventsFieldType {
                            name: name.to_string(),
                        });
                    }
                    let (ns, kind) = if is_const {
                        // A `Const` is a constant at any scope (module- or proc-level):
                        // namespace `Local`, kind `Const`. Its value is folded by the
                        // symbol layer, so it gets no runtime slot (see the frame
                        // builder, which skips `Const`).
                        (SymbolNamespace::Local, SymbolKind::Const)
                    } else if !module_level {
                        // `Static n` (or any local of a `Static` procedure) persists
                        // across calls — a distinct kind the binder lowers to a
                        // mangled global rather than a frame slot.
                        let is_static = node.is_static() || self.proc_is_static;
                        let local_kind = if is_static {
                            SymbolKind::StaticLocal
                        } else {
                            SymbolKind::Local
                        };
                        (SymbolNamespace::Local, local_kind)
                    } else if declarator.is_with_events() {
                        (SymbolNamespace::Member, SymbolKind::WithEventsField)
                    } else {
                        (SymbolNamespace::Member, SymbolKind::Field)
                    };
                    self.declare(
                        scope,
                        ns,
                        kind,
                        name,
                        token,
                        SymbolImpl::DeclaredType(declared_type),
                        module_level,
                        vis,
                        None,
                    )?;
                }
            }
            SyntaxKind::ImplementsStmt => {
                if let Some(type_ref) = node.child_node(SyntaxKind::TypeRef) {
                    let name = type_ref.text().trim().to_string();
                    if !name.is_empty() {
                        if self.module_kind == ModuleKind::Procedural {
                            return Err(SymbolModelError::ImplementsNotValidInStandardModule {
                                name,
                            });
                        }
                        self.scan.implements.push(name);
                    }
                }
            }
            _ => {}
        }
        for child in node.child_nodes() {
            self.walk(scope, child, module_level)?;
        }
        Ok(())
    }

    fn scan_procedure(
        &mut self,
        parent: ScopeId,
        node: SyntaxNode<'_>,
        module_level: bool,
    ) -> Result<(), SymbolModelError> {
        // Use the canonical proc-name extractor (shared with the binder) so the set
        // of procs the scanner gives a scope to is exactly the set the binder lowers.
        let Some(name_token) = node.proc_name_token() else {
            return Ok(());
        };
        let logical = normalize_identifier_token(name_token.text).to_string();
        let is_default = has_user_mem_id_node(node, 0)
            || self
                .default_member_attrs
                .contains(&fold_identifier(&logical));
        let is_enumerator = has_user_mem_id_node(node, -4)
            || self
                .enumerator_member_attrs
                .contains(&fold_identifier(&logical));
        // Sub/Function/Property default to Public; `Private`/`Friend` override.
        let visibility = decl_visibility(node, Visibility::Public);
        if module_level
            && visibility == Visibility::Friend
            && self.module_kind == ModuleKind::Procedural
        {
            return Err(SymbolModelError::FriendNotValidInStandardModule { name: logical });
        }
        self.validate_parameter_declaration(node, &logical)?;
        let sig = self
            .signatures
            .alloc(self.build_signature(node, CallShape::Ordinary));

        if node.kind() == SyntaxKind::PropertyDecl {
            // One logical member; merge this accessor into its property group.
            let accessor = property_accessor(node);
            let existing = self
                .symbols
                .find_in_scope(parent, SymbolNamespace::Procedure, &logical)?
                .filter(|id| {
                    matches!(
                        self.symbols.symbol(*id).map(|s| s.kind),
                        Some(SymbolKind::Property)
                    )
                });
            match existing {
                Some(id) => {
                    let mut group = match &self.symbols.symbol(id).expect("symbol").imp {
                        SymbolImpl::Property(group) => group.clone(),
                        _ => PropertyGroup::default(),
                    };
                    self.validate_property_accessor_pairing(&group, accessor, sig, &logical)?;
                    set_accessor(&mut group, accessor, sig);
                    self.symbols.update_impl(id, SymbolImpl::Property(group));
                }
                None => {
                    let mut group = PropertyGroup::default();
                    set_accessor(&mut group, accessor, sig);
                    let symbol = self.symbols.declare_symbol(
                        parent,
                        SymbolNamespace::Procedure,
                        SymbolKind::Property,
                        &logical,
                        provenance(self.module_name, name_token),
                        SymbolImpl::Property(group),
                    )?;
                    self.symbols
                        .update_visibility(symbol, module_level.then_some(visibility));
                    if module_level {
                        self.scan.members.push(ScannedMember {
                            name_folded: fold_identifier(&logical),
                            enum_name: None,
                            symbol,
                            kind: SymbolKind::Property,
                            namespace: SymbolNamespace::Procedure,
                            is_default,
                            is_enumerator,
                            visibility,
                        });
                    }
                }
            }
            self.scan_proc_body(node, parent, &format!("{logical} {accessor:?}"))?;
            return Ok(());
        }

        let kind = if node.kind() == SyntaxKind::FunctionDecl {
            SymbolKind::Function
        } else {
            SymbolKind::Procedure
        };
        let symbol = self.symbols.declare_symbol(
            parent,
            SymbolNamespace::Procedure,
            kind,
            &logical,
            provenance(self.module_name, name_token),
            SymbolImpl::Signature(sig),
        )?;
        self.symbols
            .update_visibility(symbol, module_level.then_some(visibility));
        if module_level {
            self.scan.members.push(ScannedMember {
                name_folded: fold_identifier(&logical),
                enum_name: None,
                symbol,
                kind,
                namespace: SymbolNamespace::Procedure,
                is_default,
                is_enumerator,
                visibility,
            });
        }
        self.scan_proc_body(node, parent, &logical)?;
        Ok(())
    }

    /// Open a procedure scope, declare the parameters, and walk the body for locals.
    fn scan_proc_body(
        &mut self,
        node: SyntaxNode<'_>,
        parent: ScopeId,
        scope_name: &str,
    ) -> Result<(), SymbolModelError> {
        let proc_scope =
            self.symbols
                .add_scope(crate::model::ScopeKind::Procedure, parent, Some(scope_name))?;
        if let Some(param_list) = node.param_list() {
            for param in param_list.params() {
                if let Some(param_token) = parameter_name_token(param) {
                    let name = normalize_identifier_token(param_token.text);
                    self.symbols.declare_symbol(
                        proc_scope,
                        SymbolNamespace::Parameter,
                        SymbolKind::Parameter,
                        name,
                        provenance(self.module_name, param_token),
                        SymbolImpl::DeclaredType(self.param_type(param)),
                    )?;
                }
            }
        }
        if let Some(body) = node.body_block() {
            // A `Static` procedure makes all of its locals static; restore the
            // flag afterward (procs don't nest, but keep it lexically scoped).
            let outer = self.proc_is_static;
            self.proc_is_static = node.is_static();
            let result = self.walk(proc_scope, body, false);
            self.proc_is_static = outer;
            result?;
            self.scan_implicit_redim_locals(proc_scope, body, node.is_static())?;
        }
        Ok(())
    }

    fn validate_property_accessor_pairing(
        &self,
        group: &PropertyGroup,
        accessor: PropertyAccessor,
        sig: SignatureId,
        property: &str,
    ) -> Result<(), SymbolModelError> {
        if property_accessor_signature(group, accessor).is_some() {
            return Err(SymbolModelError::DuplicatePropertyAccessor {
                property: property.to_string(),
                accessor: property_accessor_label(accessor),
            });
        }

        match accessor {
            PropertyAccessor::Get => {
                if let Some(let_sig) = group.let_ {
                    self.validate_property_get_let_pair(sig, let_sig, property, accessor)?;
                }
                if let Some(set_sig) = group.set {
                    self.validate_property_get_set_pair(sig, set_sig, property, accessor)?;
                }
            }
            PropertyAccessor::Let => {
                if let Some(get_sig) = group.get {
                    self.validate_property_get_let_pair(get_sig, sig, property, accessor)?;
                }
                if let Some(set_sig) = group.set {
                    self.validate_property_let_set_pair(sig, set_sig, property, accessor)?;
                }
            }
            PropertyAccessor::Set => {
                if let Some(get_sig) = group.get {
                    self.validate_property_get_set_pair(get_sig, sig, property, accessor)?;
                }
                if let Some(let_sig) = group.let_ {
                    self.validate_property_let_set_pair(let_sig, sig, property, accessor)?;
                }
            }
        }
        Ok(())
    }

    fn validate_property_get_let_pair(
        &self,
        get_id: SignatureId,
        let_id: SignatureId,
        property: &str,
        new_accessor: PropertyAccessor,
    ) -> Result<(), SymbolModelError> {
        let Some(get_sig) = self.signatures.get(get_id) else {
            return incompatible_property_accessor(
                property,
                new_accessor,
                "Property Get signature metadata is missing",
            );
        };
        let Some(let_sig) = self.signatures.get(let_id) else {
            return incompatible_property_accessor(
                property,
                new_accessor,
                "Property Let signature metadata is missing",
            );
        };
        if let_sig.params.len() != get_sig.params.len() + 1 {
            return incompatible_property_accessor(
                property,
                new_accessor,
                "Property Let must have the Property Get index parameters plus one final value parameter",
            );
        }
        for (get_param, let_param) in get_sig.params.iter().zip(let_sig.params.iter()) {
            if fold_identifier(&get_param.name) != fold_identifier(&let_param.name) {
                return incompatible_property_accessor(
                    property,
                    new_accessor,
                    "Property Let index parameter names must match Property Get",
                );
            }
            if get_param.ty != let_param.ty {
                return incompatible_property_accessor(
                    property,
                    new_accessor,
                    "Property Let index parameter types must match Property Get",
                );
            }
        }
        let Some(get_return) = get_sig.return_type.as_ref() else {
            return incompatible_property_accessor(
                property,
                new_accessor,
                "Property Get return type metadata is missing",
            );
        };
        let Some(value_param) = let_sig.params.last() else {
            return incompatible_property_accessor(
                property,
                new_accessor,
                "Property Let value parameter metadata is missing",
            );
        };
        if &value_param.ty != get_return {
            return incompatible_property_accessor(
                property,
                new_accessor,
                "Property Let value parameter type must match Property Get return type",
            );
        }
        Ok(())
    }

    fn validate_property_get_set_pair(
        &self,
        get_id: SignatureId,
        set_id: SignatureId,
        property: &str,
        new_accessor: PropertyAccessor,
    ) -> Result<(), SymbolModelError> {
        let Some(get_sig) = self.signatures.get(get_id) else {
            return incompatible_property_accessor(
                property,
                new_accessor,
                "Property Get signature metadata is missing",
            );
        };
        let Some(set_sig) = self.signatures.get(set_id) else {
            return incompatible_property_accessor(
                property,
                new_accessor,
                "Property Set signature metadata is missing",
            );
        };
        if set_sig.params.len() != get_sig.params.len() + 1 {
            return incompatible_property_accessor(
                property,
                new_accessor,
                "Property Set must have the Property Get index parameters plus one final reference parameter",
            );
        }
        for (get_param, set_param) in get_sig.params.iter().zip(set_sig.params.iter()) {
            if get_param.ty != set_param.ty {
                return incompatible_property_accessor(
                    property,
                    new_accessor,
                    "Property Set index parameter types must match Property Get",
                );
            }
        }
        Ok(())
    }

    fn validate_property_let_set_pair(
        &self,
        let_id: SignatureId,
        set_id: SignatureId,
        property: &str,
        new_accessor: PropertyAccessor,
    ) -> Result<(), SymbolModelError> {
        let Some(let_sig) = self.signatures.get(let_id) else {
            return incompatible_property_accessor(
                property,
                new_accessor,
                "Property Let signature metadata is missing",
            );
        };
        let Some(set_sig) = self.signatures.get(set_id) else {
            return incompatible_property_accessor(
                property,
                new_accessor,
                "Property Set signature metadata is missing",
            );
        };
        let let_index_len = let_sig.params.len().saturating_sub(1);
        let set_index_len = set_sig.params.len().saturating_sub(1);
        if let_index_len != set_index_len {
            return incompatible_property_accessor(
                property,
                new_accessor,
                "Property Let and Property Set index parameter counts must match",
            );
        }
        for (let_param, set_param) in let_sig
            .params
            .iter()
            .take(let_index_len)
            .zip(set_sig.params.iter().take(set_index_len))
        {
            if let_param.ty != set_param.ty {
                return incompatible_property_accessor(
                    property,
                    new_accessor,
                    "Property Let and Property Set index parameter types must match",
                );
            }
        }
        Ok(())
    }

    fn scan_implicit_redim_locals(
        &mut self,
        proc_scope: ScopeId,
        body: SyntaxNode<'_>,
        proc_is_static: bool,
    ) -> Result<(), SymbolModelError> {
        for redim in redim_stmt_nodes(body) {
            if redim_is_preserve(redim) {
                continue;
            }
            for token in simple_redim_target_tokens(redim) {
                let name = normalize_identifier_token(token.text);
                if self.source_name_exists(proc_scope, name)? {
                    continue;
                }
                self.symbols.declare_symbol(
                    proc_scope,
                    SymbolNamespace::Local,
                    if proc_is_static {
                        SymbolKind::StaticLocal
                    } else {
                        SymbolKind::Local
                    },
                    name,
                    provenance(self.module_name, token),
                    SymbolImpl::DeclaredType(VarTypeRef::Array(Box::new(VarTypeRef::Variant))),
                )?;
            }
        }
        Ok(())
    }

    fn source_name_exists(&self, scope: ScopeId, name: &str) -> Result<bool, SymbolModelError> {
        for namespace in [
            SymbolNamespace::Local,
            SymbolNamespace::Parameter,
            SymbolNamespace::Procedure,
            SymbolNamespace::Member,
            SymbolNamespace::Type,
            SymbolNamespace::Module,
            SymbolNamespace::Project,
        ] {
            if self
                .symbols
                .resolve_in_scope_chain(scope, namespace, name)?
                .is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn scan_declare(
        &mut self,
        scope: ScopeId,
        node: SyntaxNode<'_>,
        module_level: bool,
    ) -> Result<(), SymbolModelError> {
        let tokens = node.child_tokens();
        let is_function = tokens.iter().any(|t| t.kind == SyntaxKind::KwFunction);
        let Some(name_token) = declare_name_token(node) else {
            return Ok(());
        };
        let declared_name = normalize_identifier_token(name_token.text).to_string();
        self.validate_parameter_declaration(node, &declared_name)?;
        let library = node.lib_string().unwrap_or_default();
        let alias_raw = node.alias_string();
        let (alias, ordinal_alias) = match alias_raw {
            Some(value) => {
                let ordinal = value.starts_with('#');
                (value, ordinal)
            }
            None => (declared_name.clone(), false),
        };

        let mut param_names = Vec::new();
        let mut param_types = Vec::new();
        let mut param_by_ref = Vec::new();
        let mut param_optional = Vec::new();
        let mut param_param_array = false;
        if let Some(param_list) = node.param_list() {
            for param in param_list.params() {
                let name = parameter_name_token(param)
                    .map(|t| normalize_identifier_token(t.text).to_string())
                    .unwrap_or_default();
                param_names.push(name);
                param_types.push(declare_param_type(&self.param_type(param)));
                param_by_ref.push(parameter_passing_mode(param) == PassingMode::ByRef);
                param_optional.push(parameter_has_modifier(param, SyntaxKind::KwOptional));
                if parameter_has_modifier(param, SyntaxKind::KwParamArray) {
                    param_param_array = true;
                }
            }
        }
        let return_type = if is_function {
            Some(declare_param_type(&declare_return_type(
                node,
                name_token,
                &self.default_types,
            )))
        } else {
            None
        };

        let descriptor_id = *self.next_descriptor_id;
        *self.next_descriptor_id += 1;
        let declare = DeclareSymbol {
            descriptor_id,
            declared_name: declared_name.clone(),
            library,
            alias,
            ordinal_alias,
            calling_convention: CallConv::Stdcall,
            is_function,
            param_names,
            param_types,
            param_by_ref,
            param_optional,
            param_param_array,
            return_type,
        };
        self.declare(
            scope,
            SymbolNamespace::Procedure,
            if is_function {
                SymbolKind::Function
            } else {
                SymbolKind::Procedure
            },
            &declared_name,
            name_token,
            SymbolImpl::Declare(declare),
            module_level,
            decl_visibility(node, Visibility::Private),
            None,
        )?;
        Ok(())
    }

    fn build_signature(&self, node: SyntaxNode<'_>, call_shape: CallShape) -> Signature {
        let mut params = Vec::new();
        if let Some(param_list) = node.param_list() {
            let param_nodes = param_list.params();
            let param_count = param_nodes.len();
            for (index, param) in param_nodes.into_iter().enumerate() {
                let name = parameter_name_token(param)
                    .map(|t| normalize_identifier_token(t.text).to_string())
                    .unwrap_or_default();
                let optional = parameter_has_modifier(param, SyntaxKind::KwOptional);
                let ty = self.param_type(param);
                let default = match default_from_param(param, &ty, self.target) {
                    ParsedParamDefault::Value(value) => Some(value),
                    ParsedParamDefault::Invalid => None,
                    ParsedParamDefault::Absent | ParsedParamDefault::Unparsed => {
                        optional.then_some(DefaultValue::VariantMissing)
                    }
                };
                params.push(Param {
                    name,
                    ty,
                    mode: procedure_parameter_passing_mode(node, param, index, param_count),
                    optional,
                    param_array: parameter_has_modifier(param, SyntaxKind::KwParamArray),
                    default,
                });
            }
        }
        // An array return (`Function F() As Byte()`) is typed `Array(element)`, so a
        // whole-array `F = arr` assignment is a copy, not a scalar coercion (the proc
        // decl carries the return's `ArrayBounds` as a direct child).
        let return_type = proc_return_type(node, &self.default_types);
        Signature {
            params,
            return_type,
            call_shape,
        }
    }

    fn validate_parameter_declaration(
        &self,
        proc_node: SyntaxNode<'_>,
        procedure: &str,
    ) -> Result<(), SymbolModelError> {
        self.validate_property_writer_value_parameter_present(proc_node, procedure)?;
        self.validate_optional_parameter_declaration(proc_node, procedure)?;
        self.validate_param_array_declaration(proc_node, procedure)?;
        self.validate_property_set_reference_parameter(proc_node, procedure)
    }

    fn validate_event_declaration(
        &self,
        event_node: SyntaxNode<'_>,
        event: &str,
    ) -> Result<(), SymbolModelError> {
        if self.module_kind == ModuleKind::Procedural {
            return Err(SymbolModelError::EventNotValidInStandardModule {
                name: event.to_string(),
            });
        }
        let Some(param_list) = event_node.param_list() else {
            return Ok(());
        };
        for (index, param) in param_list.params().iter().enumerate() {
            let parameter = parameter_name_token(*param)
                .map(|t| normalize_identifier_token(t.text).to_string())
                .unwrap_or_else(|| format!("arg{}", index + 1));
            if parameter_has_modifier(*param, SyntaxKind::KwOptional) {
                return Err(SymbolModelError::InvalidOptionalParameterDeclaration {
                    procedure: event.to_string(),
                    parameter,
                    reason: "Event arguments cannot be Optional",
                });
            }
            if parameter_has_modifier(*param, SyntaxKind::KwParamArray) {
                return Err(SymbolModelError::InvalidParamArrayDeclaration {
                    procedure: event.to_string(),
                    parameter,
                    reason: "Event arguments cannot be ParamArray",
                });
            }
        }
        Ok(())
    }

    fn validate_property_writer_value_parameter_present(
        &self,
        proc_node: SyntaxNode<'_>,
        procedure: &str,
    ) -> Result<(), SymbolModelError> {
        if proc_node.kind() != SyntaxKind::PropertyDecl {
            return Ok(());
        }
        let accessor = property_accessor(proc_node);
        if !matches!(accessor, PropertyAccessor::Let | PropertyAccessor::Set) {
            return Ok(());
        }
        let has_value_parameter = proc_node
            .param_list()
            .is_some_and(|param_list| !param_list.params().is_empty());
        if has_value_parameter {
            return Ok(());
        }
        let accessor = match accessor {
            PropertyAccessor::Let => "Let",
            PropertyAccessor::Set => "Set",
            PropertyAccessor::Get => unreachable!(),
        };
        Err(SymbolModelError::MissingPropertyWriterParameter {
            procedure: procedure.to_string(),
            accessor,
        })
    }

    fn validate_optional_parameter_declaration(
        &self,
        proc_node: SyntaxNode<'_>,
        procedure: &str,
    ) -> Result<(), SymbolModelError> {
        let Some(param_list) = proc_node.param_list() else {
            return Ok(());
        };
        let params = param_list.params();
        let property_put_accessor = (proc_node.kind() == SyntaxKind::PropertyDecl)
            .then(|| property_accessor(proc_node))
            .filter(|accessor| matches!(accessor, PropertyAccessor::Let | PropertyAccessor::Set));
        if let Some(accessor) = property_put_accessor
            && let Some(value_param) = params.last()
        {
            if parameter_has_modifier(*value_param, SyntaxKind::KwParamArray) {
                let parameter = parameter_name_token(*value_param)
                    .map(|t| normalize_identifier_token(t.text).to_string())
                    .unwrap_or_else(|| format!("arg{}", params.len()));
                let reason = match accessor {
                    PropertyAccessor::Let => "Property Let value parameter cannot be ParamArray",
                    PropertyAccessor::Set => {
                        "Property Set reference parameter cannot be ParamArray"
                    }
                    PropertyAccessor::Get => unreachable!(),
                };
                return Err(SymbolModelError::InvalidParamArrayDeclaration {
                    procedure: procedure.to_string(),
                    parameter,
                    reason,
                });
            }
            if parameter_has_modifier(*value_param, SyntaxKind::KwOptional) {
                let parameter = parameter_name_token(*value_param)
                    .map(|t| normalize_identifier_token(t.text).to_string())
                    .unwrap_or_else(|| format!("arg{}", params.len()));
                let reason = match accessor {
                    PropertyAccessor::Let => "Property Let value parameter cannot be Optional",
                    PropertyAccessor::Set => "Property Set reference parameter cannot be Optional",
                    PropertyAccessor::Get => unreachable!(),
                };
                return Err(SymbolModelError::InvalidOptionalParameterDeclaration {
                    procedure: procedure.to_string(),
                    parameter,
                    reason,
                });
            }
        }
        let ordering_len = if property_put_accessor.is_some() {
            params.len().saturating_sub(1)
        } else {
            params.len()
        };
        let mut seen_optional = false;
        for (index, param) in params.iter().take(ordering_len).enumerate() {
            let optional = parameter_has_modifier(*param, SyntaxKind::KwOptional);
            if optional {
                seen_optional = true;
                continue;
            }
            if seen_optional {
                let parameter = parameter_name_token(*param)
                    .map(|t| normalize_identifier_token(t.text).to_string())
                    .unwrap_or_else(|| format!("arg{}", index + 1));
                return Err(SymbolModelError::InvalidOptionalParameterDeclaration {
                    procedure: procedure.to_string(),
                    parameter,
                    reason: "required parameters cannot follow Optional parameters",
                });
            }
        }
        Ok(())
    }

    fn validate_param_array_declaration(
        &self,
        proc_node: SyntaxNode<'_>,
        procedure: &str,
    ) -> Result<(), SymbolModelError> {
        let Some(param_list) = proc_node.param_list() else {
            return Ok(());
        };
        let params = param_list.params();
        for (index, param) in params.iter().enumerate() {
            if !parameter_has_modifier(*param, SyntaxKind::KwParamArray) {
                continue;
            }
            let parameter = parameter_name_token(*param)
                .map(|t| normalize_identifier_token(t.text).to_string())
                .unwrap_or_else(|| format!("arg{}", index + 1));
            let reason = if parameter_has_modifier(*param, SyntaxKind::KwOptional) {
                "ParamArray cannot be combined with Optional"
            } else if parameter_has_modifier(*param, SyntaxKind::KwByVal) {
                "ParamArray cannot be combined with ByVal"
            } else if parameter_has_modifier(*param, SyntaxKind::KwByRef) {
                "ParamArray cannot be combined with ByRef"
            } else if index + 1 != params.len() {
                "ParamArray must be the final parameter"
            } else if !matches!(
                self.param_type(*param),
                VarTypeRef::Array(inner) if matches!(inner.as_ref(), VarTypeRef::Variant)
            ) {
                "ParamArray must be an array of Variant elements"
            } else {
                continue;
            };
            return Err(SymbolModelError::InvalidParamArrayDeclaration {
                procedure: procedure.to_string(),
                parameter,
                reason,
            });
        }
        Ok(())
    }

    fn validate_property_set_reference_parameter(
        &self,
        proc_node: SyntaxNode<'_>,
        procedure: &str,
    ) -> Result<(), SymbolModelError> {
        if proc_node.kind() != SyntaxKind::PropertyDecl
            || property_accessor(proc_node) != PropertyAccessor::Set
        {
            return Ok(());
        }
        let Some(param_list) = proc_node.param_list() else {
            return Ok(());
        };
        let params = param_list.params();
        let Some(reference_param) = params.last() else {
            return Ok(());
        };
        if property_set_reference_type_compatible(&self.param_type(*reference_param)) {
            return Ok(());
        }
        let parameter = parameter_name_token(*reference_param)
            .map(|t| normalize_identifier_token(t.text).to_string())
            .unwrap_or_else(|| format!("arg{}", params.len()));
        Err(SymbolModelError::InvalidPropertySetReferenceParameter {
            procedure: procedure.to_string(),
            parameter,
        })
    }

    fn param_type(&self, node: SyntaxNode<'_>) -> VarTypeRef {
        let explicit = node
            .child_nodes()
            .into_iter()
            .find(|child| child.kind() == SyntaxKind::TypeRef)
            .map(type_ref_node)
            .or_else(|| type_suffix_type(node));
        let base = if parameter_has_modifier(node, SyntaxKind::KwParamArray) {
            explicit.unwrap_or(VarTypeRef::Variant)
        } else {
            explicit
                .or_else(|| {
                    parameter_name_token(node)
                        .and_then(|token| self.default_types.type_for(token.text))
                })
                .unwrap_or(VarTypeRef::Variant)
        };
        let element = fixed_string_refine(base, node);
        if node.array_bounds().is_some() {
            return VarTypeRef::Array(Box::new(element));
        }
        element
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn declare(
        &mut self,
        scope: ScopeId,
        namespace: SymbolNamespace,
        kind: SymbolKind,
        name: &str,
        token: SyntaxToken<'_>,
        imp: SymbolImpl,
        module_level: bool,
        visibility: Visibility,
        enum_name: Option<&str>,
    ) -> Result<(), SymbolModelError> {
        let symbol = self.symbols.declare_symbol(
            scope,
            namespace,
            kind,
            name,
            provenance(self.module_name, token),
            imp,
        )?;
        self.symbols
            .update_visibility(symbol, module_level.then_some(visibility));
        if module_level {
            self.scan.members.push(ScannedMember {
                name_folded: fold_identifier(name),
                symbol,
                kind,
                namespace,
                is_default: false,
                is_enumerator: false,
                visibility,
                enum_name: enum_name.map(str::to_string),
            });
        }
        Ok(())
    }
}

fn property_set_reference_type_compatible(ty: &VarTypeRef) -> bool {
    matches!(ty, VarTypeRef::Variant | VarTypeRef::Object(_))
}

fn withevents_field_type_compatible(ty: &VarTypeRef) -> bool {
    matches!(ty, VarTypeRef::Object(_))
}

fn property_accessor_signature(
    group: &PropertyGroup,
    accessor: PropertyAccessor,
) -> Option<SignatureId> {
    match accessor {
        PropertyAccessor::Get => group.get,
        PropertyAccessor::Let => group.let_,
        PropertyAccessor::Set => group.set,
    }
}

fn property_accessor_label(accessor: PropertyAccessor) -> &'static str {
    match accessor {
        PropertyAccessor::Get => "Get",
        PropertyAccessor::Let => "Let",
        PropertyAccessor::Set => "Set",
    }
}

fn incompatible_property_accessor<T>(
    property: &str,
    accessor: PropertyAccessor,
    reason: &'static str,
) -> Result<T, SymbolModelError> {
    Err(SymbolModelError::IncompatiblePropertyAccessor {
        property: property.to_string(),
        accessor: property_accessor_label(accessor),
        reason,
    })
}

fn procedure_parameter_passing_mode(
    proc_node: SyntaxNode<'_>,
    param: SyntaxNode<'_>,
    index: usize,
    param_count: usize,
) -> PassingMode {
    if proc_node.kind() == SyntaxKind::PropertyDecl
        && index + 1 == param_count
        && matches!(
            property_accessor(proc_node),
            PropertyAccessor::Let | PropertyAccessor::Set
        )
    {
        PassingMode::ByVal
    } else {
        parameter_passing_mode(param)
    }
}

// ── CST helpers (ported) ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropertyAccessor {
    Get,
    Let,
    Set,
}

fn property_accessor(node: SyntaxNode<'_>) -> PropertyAccessor {
    let tokens = node.child_tokens();
    if tokens.iter().any(|t| t.kind == SyntaxKind::KwLet) {
        PropertyAccessor::Let
    } else if tokens.iter().any(|t| t.kind == SyntaxKind::KwSet) {
        PropertyAccessor::Set
    } else {
        PropertyAccessor::Get
    }
}

fn set_accessor(group: &mut PropertyGroup, accessor: PropertyAccessor, sig: SignatureId) {
    match accessor {
        PropertyAccessor::Get => group.get = Some(sig),
        PropertyAccessor::Let => group.let_ = Some(sig),
        PropertyAccessor::Set => group.set = Some(sig),
    }
}

/// The declared visibility of a top-level declaration: an explicit
/// `Public`/`Private`/`Friend` modifier token wins; otherwise `default` (the VBA
/// default for that declaration kind). The modifier is a direct child token of
/// the decl node — read the same way [`property_accessor`] reads Get/Let/Set.
fn decl_visibility(node: SyntaxNode<'_>, default: Visibility) -> Visibility {
    for t in node.child_tokens() {
        match t.kind {
            SyntaxKind::KwPublic => return Visibility::Public,
            SyntaxKind::KwPrivate => return Visibility::Private,
            SyntaxKind::KwFriend => return Visibility::Friend,
            _ => {}
        }
    }
    default
}

/// `Attribute VB_UserMemId = <id>` inside the procedure.
fn has_user_mem_id_node(node: SyntaxNode<'_>, id: i32) -> bool {
    node.text()
        .lines()
        .any(|line| line_has_user_mem_id(line, id))
}

/// Exported `.cls` files place member attributes after the member body:
/// `Attribute Value.VB_UserMemId = 0`. The parser keeps those as top-level
/// `AttributeStmt` nodes, so associate them with the logical member during scan.
fn member_attributes_with_user_mem_id(root: SyntaxNode<'_>, id: i32) -> BTreeSet<String> {
    let mut attrs = BTreeSet::new();
    collect_member_attributes_with_user_mem_id(root, id, &mut attrs);
    attrs
}

fn option_private_module(root: SyntaxNode<'_>) -> bool {
    root.child_nodes().into_iter().any(|node| {
        if node.kind() != SyntaxKind::OptionStmt {
            return false;
        }
        let toks = node.child_tokens();
        toks.iter().any(|t| t.kind == SyntaxKind::KwPrivate)
            && toks
                .iter()
                .any(|t| t.kind == SyntaxKind::Ident && t.text.eq_ignore_ascii_case("Module"))
    })
}

fn reject_unsupported_option_compare_database(
    root: SyntaxNode<'_>,
) -> Result<(), SymbolModelError> {
    for node in root.child_nodes() {
        if node.kind() != SyntaxKind::OptionStmt {
            continue;
        }
        let toks = node.child_tokens();
        let is_compare = toks.iter().any(|t| t.kind == SyntaxKind::KwCompare);
        let is_database = toks
            .iter()
            .any(|t| t.kind == SyntaxKind::Ident && t.text.eq_ignore_ascii_case("Database"));
        if is_compare && is_database {
            // Microsoft Learn "Option Compare statement" documents Database as
            // Microsoft Access-only and dependent on database locale/collation:
            // learn.microsoft.com/office/vba/language/reference/user-interface-help/option-compare-statement
            // Do not silently approximate it with Binary/Text semantics.
            return Err(SymbolModelError::UnsupportedOptionCompareDatabase);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
struct DefaultTypeTable {
    ascii: [Option<VarTypeRef>; 26],
    extended_alpha: Option<VarTypeRef>,
}

impl DefaultTypeTable {
    fn set_range(
        &mut self,
        start: char,
        end: char,
        ty: &VarTypeRef,
    ) -> Result<(), SymbolModelError> {
        let Some(start) = ascii_letter_index(start) else {
            return Ok(());
        };
        let Some(end) = ascii_letter_index(end) else {
            return Ok(());
        };
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        // Microsoft Learn "Deftype statements" documents any later range that
        // includes a previously defined letter as an error; do this before
        // mutating the table so a failing range cannot partly apply.
        for idx in lo..=hi {
            if self.ascii[idx].is_some() {
                return Err(SymbolModelError::DuplicateDefTypeLetter {
                    letter: (b'A' + idx as u8) as char,
                });
            }
        }
        for idx in lo..=hi {
            self.ascii[idx] = Some(ty.clone());
        }
        // Microsoft Learn "Deftype statements"
        // (learn.microsoft.com/office/vba/language/concepts/getting-started/deftype-statements)
        // documents A-Z as also covering extended alphabetic names.
        if lo == 0 && hi == 25 && self.extended_alpha.is_none() {
            self.extended_alpha = Some(ty.clone());
        }
        Ok(())
    }

    fn type_for(&self, name: &str) -> Option<VarTypeRef> {
        let normalized = normalize_identifier_token(name);
        let first = normalized.chars().next()?;
        if let Some(idx) = ascii_letter_index(first) {
            return self.ascii[idx].clone();
        }
        first.is_alphabetic().then(|| self.extended_alpha.clone())?
    }
}

fn module_default_types(root: SyntaxNode<'_>) -> Result<DefaultTypeTable, SymbolModelError> {
    let mut table = DefaultTypeTable::default();
    for node in root.child_nodes() {
        let Some((ty, ranges)) = parse_deftype_directive(node)? else {
            continue;
        };
        for (start, end) in ranges {
            table.set_range(start, end, &ty)?;
        }
    }
    Ok(table)
}

type DeftypeRanges = Vec<(char, char)>;
type DeftypeDirective = Option<(VarTypeRef, DeftypeRanges)>;

fn parse_deftype_directive(node: SyntaxNode<'_>) -> Result<DeftypeDirective, SymbolModelError> {
    let tokens = significant_tokens_deep(node);
    let Some(first) = tokens.first() else {
        return Ok(None);
    };
    if first.kind != SyntaxKind::Ident {
        return Ok(None);
    }
    let ty = match deftype_statement_type(first.text)? {
        Some(ty) => ty,
        None => return Ok(None),
    };
    let mut ranges = Vec::new();
    let mut i = 1usize;
    while i < tokens.len() {
        if tokens[i].kind == SyntaxKind::Comma {
            i += 1;
            continue;
        }
        let Some(start) = letter_token(tokens[i]) else {
            i += 1;
            continue;
        };
        let mut end = start;
        if tokens
            .get(i + 1)
            .is_some_and(|t| t.kind == SyntaxKind::Minus)
            && let Some(next) = tokens.get(i + 2).and_then(|token| letter_token(*token))
        {
            end = next;
            i += 2;
        }
        ranges.push((start, end));
        i += 1;
    }
    Ok((!ranges.is_empty()).then_some((ty, ranges)))
}

fn deftype_statement_type(name: &str) -> Result<Option<VarTypeRef>, SymbolModelError> {
    let ty = match name.to_ascii_lowercase().as_str() {
        "defbool" => VarTypeRef::Builtin(BuiltinType::Boolean),
        "defbyte" => VarTypeRef::Builtin(BuiltinType::Byte),
        "defint" => VarTypeRef::Builtin(BuiltinType::Integer),
        "deflng" => VarTypeRef::Builtin(BuiltinType::Long),
        "deflnglng" => VarTypeRef::Builtin(BuiltinType::LongLong),
        "deflngptr" => VarTypeRef::Builtin(BuiltinType::LongPtr),
        "defcur" => VarTypeRef::Builtin(BuiltinType::Currency),
        "defsng" => VarTypeRef::Builtin(BuiltinType::Single),
        "defdbl" => VarTypeRef::Builtin(BuiltinType::Double),
        "defdate" => VarTypeRef::Builtin(BuiltinType::Date),
        "defstr" => VarTypeRef::Builtin(BuiltinType::String),
        "defobj" => VarTypeRef::Object("object".to_string()),
        "defvar" => VarTypeRef::Variant,
        // Microsoft Learn "Decimal data type" documents Decimal as usable only
        // inside a Variant, not as ordinary declared storage:
        // learn.microsoft.com/office/vba/language/reference/user-interface-help/decimal-data-type
        // Treat DefDec as an explicit unsupported declared-storage request
        // rather than silently falling back to Variant.
        "defdec" => return Err(SymbolModelError::UnsupportedDefDec),
        _ => return Ok(None),
    };
    Ok(Some(ty))
}

fn reject_unsupported_declared_decimal_storage(
    node: SyntaxNode<'_>,
) -> Result<(), SymbolModelError> {
    // Microsoft Learn "Decimal data type" documents Decimal as a Variant-only
    // subtype, not ordinary declared storage:
    // learn.microsoft.com/office/vba/language/reference/user-interface-help/decimal-data-type
    // Reject bare `As Decimal` explicitly so it cannot fall through as an
    // unresolved object type named "decimal".
    if node.kind() == SyntaxKind::TypeRef && type_ref_name(node).eq_ignore_ascii_case("decimal") {
        return Err(SymbolModelError::UnsupportedDeclaredDecimal);
    }
    for child in node.child_nodes() {
        reject_unsupported_declared_decimal_storage(child)?;
    }
    Ok(())
}

fn significant_tokens_deep(node: SyntaxNode<'_>) -> Vec<SyntaxToken<'_>> {
    let mut out = Vec::new();
    collect_significant_tokens(node, &mut out);
    out
}

fn collect_significant_tokens<'a>(node: SyntaxNode<'a>, out: &mut Vec<SyntaxToken<'a>>) {
    for child in node.children() {
        match child {
            SyntaxElement::Token(token) if !token.kind.is_trivia() => out.push(token),
            SyntaxElement::Node(node) => collect_significant_tokens(node, out),
            _ => {}
        }
    }
}

fn letter_token(token: SyntaxToken<'_>) -> Option<char> {
    if !matches!(token.kind, SyntaxKind::Ident | SyntaxKind::BracketedIdent) {
        return None;
    }
    normalize_identifier_token(token.text).chars().next()
}

fn ascii_letter_index(ch: char) -> Option<usize> {
    let ch = ch.to_ascii_uppercase();
    ch.is_ascii_uppercase().then(|| (ch as u8 - b'A') as usize)
}

fn source_module_attributes(root: SyntaxNode<'_>) -> ScannedModuleAttributes {
    let mut attrs = ScannedModuleAttributes::default();
    for node in root.child_nodes() {
        if node.kind() != SyntaxKind::AttributeStmt {
            continue;
        }
        let Some((key, value)) = parse_attribute_line(&node.text()) else {
            continue;
        };
        match key.as_str() {
            "vb_name" => attrs.vb_name = Some(value),
            "vb_globalnamespace" => attrs.vb_global_namespace = parse_bool_attribute(&value),
            "vb_creatable" => attrs.vb_creatable = parse_bool_attribute(&value),
            "vb_predeclaredid" => attrs.vb_predeclared_id = parse_bool_attribute(&value),
            "vb_exposed" => attrs.vb_exposed = parse_bool_attribute(&value),
            _ => {}
        }
    }
    attrs
}

fn parse_attribute_line(text: &str) -> Option<(String, String)> {
    let trimmed = text.trim();
    if !trimmed.to_ascii_lowercase().starts_with("attribute ") {
        return None;
    }
    let rest = &trimmed["attribute ".len()..];
    let (key, value) = rest.split_once('=')?;
    Some((
        key.trim().to_ascii_lowercase(),
        unquote_attribute_value(value.trim()),
    ))
}

fn unquote_attribute_value(value: &str) -> String {
    let inner = value.strip_prefix('"').unwrap_or(value);
    let inner = inner.strip_suffix('"').unwrap_or(inner);
    inner.replace("\"\"", "\"")
}

fn parse_bool_attribute(value: &str) -> Option<bool> {
    if value.eq_ignore_ascii_case("true") {
        Some(true)
    } else if value.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

fn collect_member_attributes_with_user_mem_id(
    node: SyntaxNode<'_>,
    id: i32,
    attrs: &mut BTreeSet<String>,
) {
    if node.kind() == SyntaxKind::AttributeStmt
        && let Some(member) = member_attribute_name_with_user_mem_id(&node.text(), id)
    {
        attrs.insert(fold_identifier(&member));
    }
    for child in node.child_nodes() {
        collect_member_attributes_with_user_mem_id(child, id, attrs);
    }
}

fn member_attribute_name_with_user_mem_id(text: &str, id: i32) -> Option<String> {
    let compact = text.to_ascii_lowercase().replace([' ', '\t'], "");
    if !compact.starts_with("attribute") || !compact.contains(".vb_usermemid=") {
        return None;
    }
    let after_keyword = text.trim().split_once(char::is_whitespace)?.1.trim();
    let (lhs, value) = after_keyword.split_once('=')?;
    if parse_user_mem_id_value(value)? != id {
        return None;
    }
    let (member, attr) = lhs.trim().rsplit_once('.')?;
    if !attr.trim().eq_ignore_ascii_case("VB_UserMemId") {
        return None;
    }
    let member = member.trim();
    (!member.is_empty()).then(|| member.to_string())
}

fn line_has_user_mem_id(line: &str, id: i32) -> bool {
    let Some((_, value)) = line.split_once('=') else {
        return false;
    };
    line.to_ascii_lowercase().contains("vb_usermemid") && parse_user_mem_id_value(value) == Some(id)
}

fn parse_user_mem_id_value(value: &str) -> Option<i32> {
    value.trim().parse::<i32>().ok()
}

/// Parse a parameter's literal default (`Optional x As Long = 5`). With the
/// `oxvba-syntax` parser fix, the `= default` is folded into the `Param` node, so
/// this reads the text after the (first) `=`.
enum ParsedParamDefault {
    Absent,
    Value(DefaultValue),
    Unparsed,
    Invalid,
}

fn default_from_param(
    node: SyntaxNode<'_>,
    ty: &VarTypeRef,
    target: ConditionalCompilationTarget,
) -> ParsedParamDefault {
    let text = node.text();
    let Some((_, rhs)) = text.split_once('=') else {
        return ParsedParamDefault::Absent;
    };
    let Some(raw) = parse_default_literal(rhs.trim()) else {
        return ParsedParamDefault::Unparsed;
    };
    let Some(value) = crate::const_eval::coerce_const_to_declared_type(raw, ty, target) else {
        return ParsedParamDefault::Invalid;
    };
    match default_value_from_core_const(value) {
        Some(value) => ParsedParamDefault::Value(value),
        None => ParsedParamDefault::Unparsed,
    }
}

fn parse_default_literal(rhs: &str) -> Option<CoreConst> {
    if rhs.eq_ignore_ascii_case("true") {
        return Some(CoreConst::Bool(true));
    }
    if rhs.eq_ignore_ascii_case("false") {
        return Some(CoreConst::Bool(false));
    }
    if let Some(inner) = rhs.strip_prefix('"') {
        let end = inner.find('"').unwrap_or(inner.len());
        return Some(CoreConst::Str(inner[..end].to_string()));
    }
    if rhs.starts_with('#')
        && let Some(end) = rhs[1..].find('#')
    {
        let literal = &rhs[..=end + 1];
        return crate::const_eval::date::parse_date_literal_serial_bits(literal)
            .map(CoreConst::Date);
    }
    // Numeric, with an optional sign.
    let (negate, body) = match rhs.strip_prefix('-') {
        Some(rest) => (true, rest.trim()),
        None => (false, rhs.strip_prefix('+').map(str::trim).unwrap_or(rhs)),
    };
    let token = body.split_whitespace().next().unwrap_or(body);
    if token.contains('.') || token.contains(['e', 'E']) && !token.starts_with('&') {
        let value: f64 = token.trim_end_matches(['!', '#', '@']).parse().ok()?;
        return Some(CoreConst::F64(
            if negate { -value } else { value }.to_bits(),
        ));
    }
    let raw = parse_int_literal(token)?;
    if negate {
        crate::const_eval::negate_const(raw)
    } else {
        Some(raw)
    }
}

fn default_value_from_core_const(value: CoreConst) -> Option<DefaultValue> {
    Some(match value {
        CoreConst::I16(value) => DefaultValue::I16(value),
        CoreConst::I32(value) => DefaultValue::I32(value),
        CoreConst::I64(value) => DefaultValue::I64(value),
        CoreConst::F64(bits) => DefaultValue::F64(bits),
        CoreConst::F32(bits) => DefaultValue::F32(bits),
        CoreConst::Bool(value) => DefaultValue::Bool(value),
        CoreConst::Str(value) => DefaultValue::Str(value),
        CoreConst::Currency(value) => DefaultValue::CurrencyScaledI64(value),
        CoreConst::Date(bits) => DefaultValue::DateSerialF64(bits),
        CoreConst::Empty | CoreConst::Null | CoreConst::Nothing => return None,
    })
}

fn parse_int_literal(text: &str) -> Option<CoreConst> {
    let trimmed = text.trim();
    // Hex/oct literals carry the width-based two's-complement sign rule, shared
    // with the binder (MS-VBAL §3.3.2).
    let radix = match trimmed.as_bytes() {
        [b'&', b'H' | b'h', ..] => Some(16),
        [b'&', b'O' | b'o', ..] => Some(8),
        _ => None,
    };
    if let Some(radix) = radix {
        return CoreConst::from_vba_radix(trimmed, radix);
    }
    CoreConst::from_int_literal(trimmed)
}

fn is_identifier_like(kind: SyntaxKind) -> bool {
    kind == SyntaxKind::Ident || kind == SyntaxKind::BracketedIdent
}

fn redim_stmt_nodes<'a>(root: SyntaxNode<'a>) -> Vec<SyntaxNode<'a>> {
    let mut out = Vec::new();
    collect_redim_stmt_nodes(root, &mut out);
    out
}

fn collect_redim_stmt_nodes<'a>(node: SyntaxNode<'a>, out: &mut Vec<SyntaxNode<'a>>) {
    if node.kind() == SyntaxKind::ReDimStmt {
        out.push(node);
        return;
    }
    for child in node.child_nodes() {
        collect_redim_stmt_nodes(child, out);
    }
}

fn redim_is_preserve(node: SyntaxNode<'_>) -> bool {
    node.child_tokens()
        .iter()
        .any(|token| token.kind == SyntaxKind::KwPreserve)
}

fn simple_redim_target_tokens(node: SyntaxNode<'_>) -> Vec<SyntaxToken<'_>> {
    let mut targets = Vec::new();
    let mut segments: Vec<SyntaxToken<'_>> = Vec::new();
    for element in node.children() {
        match element {
            SyntaxElement::Token(token)
                if is_identifier_like(token.kind)
                    || (token.kind.is_keyword()
                        && !matches!(
                            token.kind,
                            SyntaxKind::KwReDim | SyntaxKind::KwPreserve | SyntaxKind::KwMe
                        )) =>
            {
                segments.push(token);
            }
            SyntaxElement::Token(token)
                if matches!(
                    token.kind,
                    SyntaxKind::Dot | SyntaxKind::Bang | SyntaxKind::TypeSuffix
                ) => {}
            SyntaxElement::Token(token) if token.kind == SyntaxKind::Comma => {
                segments.clear();
            }
            SyntaxElement::Node(child) if child.kind() == SyntaxKind::ArrayBounds => {
                if segments.len() == 1 {
                    targets.push(segments[0]);
                }
                segments.clear();
            }
            _ => {}
        }
    }
    targets
}

fn normalize_identifier_token(text: &str) -> &str {
    text.strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .unwrap_or(text)
}

fn first_identifier_token(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    node.child_tokens()
        .into_iter()
        .find(|token| is_identifier_like(token.kind))
        .or_else(|| first_identifier_token_deep(node))
}

fn first_identifier_token_deep(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    for element in node.children() {
        match element {
            SyntaxElement::Token(token)
                if is_identifier_like(token.kind)
                    || (node.kind() == SyntaxKind::IdentExpr && token.kind.is_keyword()) =>
            {
                return Some(token);
            }
            SyntaxElement::Node(child) => {
                if let Some(token) = first_identifier_token_deep(child) {
                    return Some(token);
                }
            }
            _ => {}
        }
    }
    None
}

fn declare_name_token(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    let mut after_proc_kind = false;
    for token in node.child_tokens() {
        if matches!(token.kind, SyntaxKind::KwFunction | SyntaxKind::KwSub) {
            after_proc_kind = true;
            continue;
        }
        if after_proc_kind && is_identifier_like(token.kind) {
            return Some(token);
        }
    }
    first_identifier_token(node)
}

pub(crate) fn parameter_name_token(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
    let mut after_modifier = true;
    let mut in_type_ref = false;
    for element in node.children() {
        match element {
            SyntaxElement::Token(token)
                if matches!(
                    token.kind,
                    SyntaxKind::KwOptional
                        | SyntaxKind::KwByVal
                        | SyntaxKind::KwByRef
                        | SyntaxKind::KwParamArray
                ) =>
            {
                after_modifier = true;
            }
            SyntaxElement::Token(token) if token.kind == SyntaxKind::KwAs => {
                in_type_ref = true;
                after_modifier = false;
            }
            SyntaxElement::Token(token)
                if !in_type_ref
                    && after_modifier
                    && (is_identifier_like(token.kind) || token.kind.is_keyword()) =>
            {
                return Some(token);
            }
            SyntaxElement::Node(child)
                if !in_type_ref && after_modifier && child.kind() == SyntaxKind::IdentExpr =>
            {
                return first_identifier_token_deep(child);
            }
            SyntaxElement::Node(child) if child.kind() == SyntaxKind::TypeRef => {
                in_type_ref = true;
            }
            SyntaxElement::Token(token) if !token.kind.is_trivia() => {
                if token.kind != SyntaxKind::TypeSuffix {
                    after_modifier = false;
                }
            }
            _ => {}
        }
    }
    None
}

fn parameter_has_modifier(node: SyntaxNode<'_>, keyword: SyntaxKind) -> bool {
    node.child_tokens()
        .iter()
        .any(|token| token.kind == keyword)
}

fn parameter_passing_mode(node: SyntaxNode<'_>) -> PassingMode {
    if parameter_has_modifier(node, SyntaxKind::KwByVal) {
        PassingMode::ByVal
    } else {
        PassingMode::ByRef
    }
}

/// A declarator's declared type, refining `As String * N` to a fixed-length string,
/// and wrapping an **array** declarator (`x()` or `x(1 To 3)`) in [`VarTypeRef::Array`]
/// or [`VarTypeRef::FixedArray`] of its element type. The array wrap matters because
/// the binder distinguishes a whole-array assignment (`x = arr`, no scalar coercion)
/// from a scalar store, and reads the element type back through the array wrapper for
/// `ReDim`/`Erase`/frame layout — without it a `Dim x() As Byte` would be typed as a
/// scalar `Byte` and a whole-array assignment would wrongly coerce the array to that
/// scalar.
fn declared_var_type_with_default(
    declarator: SyntaxNode<'_>,
    default_types: &DefaultTypeTable,
    apply_default_type: bool,
) -> VarTypeRef {
    let base = declarator
        .declared_type()
        .map(type_ref_node)
        .or_else(|| type_suffix_type(declarator))
        .or_else(|| {
            apply_default_type
                .then(|| declarator.declarator_name())
                .flatten()
                .and_then(|token| default_types.type_for(token.text))
        })
        .unwrap_or(VarTypeRef::Variant);
    let element = fixed_string_refine(base, declarator);
    if let Some(bounds) = declarator.array_bounds() {
        if let Some(bounds) = fixed_array_bounds_from_bounds(bounds) {
            return VarTypeRef::FixedArray {
                element: Box::new(element),
                bounds,
            };
        }
        return VarTypeRef::Array(Box::new(element));
    }
    element
}

fn proc_return_type(node: SyntaxNode<'_>, default_types: &DefaultTypeTable) -> Option<VarTypeRef> {
    let returns_value = node.kind() == SyntaxKind::FunctionDecl
        || (node.kind() == SyntaxKind::PropertyDecl
            && property_accessor(node) == PropertyAccessor::Get);
    if !returns_value {
        return None;
    }
    let explicit = node.return_type().map(type_ref_node);
    let base = explicit
        .or_else(|| type_suffix_type_after_proc_name(node))
        .or_else(|| {
            node.proc_name_token()
                .and_then(|token| default_types.type_for(token.text))
        })
        .unwrap_or(VarTypeRef::Variant);
    if node.return_type().is_some() && node.array_bounds().is_some() {
        Some(VarTypeRef::Array(Box::new(base)))
    } else {
        Some(base)
    }
}

fn declare_return_type(
    node: SyntaxNode<'_>,
    name_token: SyntaxToken<'_>,
    default_types: &DefaultTypeTable,
) -> VarTypeRef {
    node.return_type()
        .map(type_ref_node)
        .or_else(|| type_suffix_type_after_proc_name(node))
        .or_else(|| default_types.type_for(name_token.text))
        .unwrap_or(VarTypeRef::Variant)
}

fn type_suffix_type(node: SyntaxNode<'_>) -> Option<VarTypeRef> {
    node.child_tokens()
        .into_iter()
        .find(|token| token.kind == SyntaxKind::TypeSuffix)
        .and_then(|token| type_suffix_ref(token.text))
}

fn type_suffix_type_after_proc_name(node: SyntaxNode<'_>) -> Option<VarTypeRef> {
    let name = node.proc_name_token()?;
    let mut after_name = false;
    for token in node.child_tokens() {
        if !after_name {
            after_name = token.offset == name.offset && token.text == name.text;
            continue;
        }
        if token.kind == SyntaxKind::TypeSuffix {
            return type_suffix_ref(token.text);
        }
        if !token.kind.is_trivia() {
            return None;
        }
    }
    None
}

fn type_suffix_ref(suffix: &str) -> Option<VarTypeRef> {
    let builtin = match suffix {
        "%" => BuiltinType::Integer,
        "&" => BuiltinType::Long,
        "^" => BuiltinType::LongLong,
        "!" => BuiltinType::Single,
        "#" => BuiltinType::Double,
        "@" => BuiltinType::Currency,
        "$" => BuiltinType::String,
        _ => return None,
    };
    Some(VarTypeRef::Builtin(builtin))
}

fn fixed_string_refine(base: VarTypeRef, node: SyntaxNode<'_>) -> VarTypeRef {
    if base == VarTypeRef::Builtin(BuiltinType::String)
        && let Some(len) = node.fixed_string_length().and_then(parse_fixed_string_len)
    {
        return VarTypeRef::FixedString(len);
    }
    base
}

fn parse_fixed_string_len(node: SyntaxNode<'_>) -> Option<u32> {
    let tok = node.first_significant_token()?;
    (tok.kind == SyntaxKind::IntLiteral).then(|| tok.text.trim().parse::<u32>().ok())?
}

/// Map a `TypeRef` node to a resolved type reference.
/// Scan every `Type … End Type` block reachable from the module roots into a
/// `folded type name → ordered [(folded field name, field type)]` map — the binder's
/// UDT field table (field indices + types for `p.X` access and record allocation).
pub fn collect_udt_fields(
    module_roots: &[SyntaxNode<'_>],
) -> std::collections::HashMap<String, Vec<(String, VarTypeRef)>> {
    let mut out = std::collections::HashMap::new();
    for root in module_roots {
        collect_udt_fields_in(*root, &mut out);
    }
    out
}

fn collect_udt_fields_in(
    node: SyntaxNode<'_>,
    out: &mut std::collections::HashMap<String, Vec<(String, VarTypeRef)>>,
) {
    if node.kind() == SyntaxKind::TypeBlock
        && let Some(token) = first_identifier_token(node)
    {
        let name = fold_identifier(normalize_identifier_token(token.text));
        let fields = node
            .type_fields()
            .into_iter()
            .filter_map(|f| {
                let field = fold_identifier(normalize_identifier_token(f.declarator_name()?.text));
                // A `TypeField` shares the declarator accessors (`declared_type`,
                // `array_bounds`, fixed-string `*N`), so reuse the same array-aware
                // refinement: an array field (`Words() As OcrWord`) must type as
                // `Array(OcrWord)`, not the scalar element type, so member access
                // through an index step (`o.Words(i).Text`) resolves the element UDT.
                let ty = declared_udt_field_type(f);
                Some((field, ty))
            })
            .collect();
        out.insert(name, fields);
    }
    for child in node.child_nodes() {
        collect_udt_fields_in(child, out);
    }
}

fn declared_udt_field_type(field: SyntaxNode<'_>) -> VarTypeRef {
    declared_var_type_with_default(field, &DefaultTypeTable::default(), false)
}

fn fixed_array_bounds_from_bounds(
    bounds: SyntaxNode<'_>,
) -> Option<Vec<crate::signature::FixedArrayBound>> {
    let mut out = Vec::new();
    let mut saw_bound = false;
    for bound in bounds.children_of(SyntaxKind::Bound) {
        saw_bound = true;
        let exprs = bound.expr_children();
        let (lower, upper) = match exprs.as_slice() {
            [upper] => (0, literal_i32(*upper)?),
            [lower, upper] => (literal_i32(*lower)?, literal_i32(*upper)?),
            _ => return None,
        };
        let dim_len = upper.checked_sub(lower)?.checked_add(1)?;
        if dim_len <= 0 {
            return None;
        }
        out.push(crate::signature::FixedArrayBound {
            lower,
            len: dim_len as usize,
        });
    }
    saw_bound.then_some(out)
}

fn literal_i32(expr: SyntaxNode<'_>) -> Option<i32> {
    let token = expr.first_significant_token()?;
    if token.kind == SyntaxKind::IntLiteral {
        return token.text.trim().parse::<i32>().ok();
    }
    None
}

/// Trim a leading `New` keyword from a `TypeRef`'s text (`As New Foo` ⇒ `Foo`).
/// The keyword only counts when it is followed by whitespace, so a type literally
/// named `Newfoo` is left untouched.
fn strip_leading_new_keyword(s: &str) -> &str {
    let s = s.trim_start();
    if let Some(prefix) = s.get(..3)
        && prefix.eq_ignore_ascii_case("new")
        && s[3..].starts_with(char::is_whitespace)
    {
        return s[3..].trim();
    }
    s
}

fn type_ref_name(node: SyntaxNode<'_>) -> String {
    let text = node.text();
    let name = strip_leading_new_keyword(&text)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim();
    normalize_identifier_token(name).to_string()
}

fn type_ref_node(node: SyntaxNode<'_>) -> VarTypeRef {
    let name = type_ref_name(node);
    match name.to_ascii_lowercase().as_str() {
        "boolean" => VarTypeRef::Builtin(BuiltinType::Boolean),
        "byte" => VarTypeRef::Builtin(BuiltinType::Byte),
        "integer" => VarTypeRef::Builtin(BuiltinType::Integer),
        "long" => VarTypeRef::Builtin(BuiltinType::Long),
        "longlong" => VarTypeRef::Builtin(BuiltinType::LongLong),
        "longptr" => VarTypeRef::Builtin(BuiltinType::LongPtr),
        "single" => VarTypeRef::Builtin(BuiltinType::Single),
        "double" => VarTypeRef::Builtin(BuiltinType::Double),
        "currency" => VarTypeRef::Builtin(BuiltinType::Currency),
        "date" => VarTypeRef::Builtin(BuiltinType::Date),
        "string" => VarTypeRef::Builtin(BuiltinType::String),
        "variant" | "" => VarTypeRef::Variant,
        "object" => VarTypeRef::Object("object".to_string()),
        other => VarTypeRef::Object(other.to_string()),
    }
}

fn declare_param_type(ty: &VarTypeRef) -> DeclareParamType {
    match ty {
        VarTypeRef::Builtin(BuiltinType::Boolean) => DeclareParamType::Boolean,
        VarTypeRef::Builtin(BuiltinType::Byte) => DeclareParamType::Byte,
        VarTypeRef::Builtin(BuiltinType::Integer) => DeclareParamType::Integer,
        VarTypeRef::Builtin(BuiltinType::Long) => DeclareParamType::Long,
        VarTypeRef::Builtin(BuiltinType::LongLong) => DeclareParamType::LongLong,
        VarTypeRef::Builtin(BuiltinType::LongPtr) => DeclareParamType::LongPtr,
        VarTypeRef::Builtin(BuiltinType::Single) => DeclareParamType::Single,
        VarTypeRef::Builtin(BuiltinType::Double) => DeclareParamType::Double,
        VarTypeRef::Builtin(BuiltinType::Currency) => DeclareParamType::Currency,
        VarTypeRef::Builtin(BuiltinType::Date) => DeclareParamType::Date,
        VarTypeRef::Builtin(BuiltinType::String) | VarTypeRef::FixedString(_) => {
            DeclareParamType::String
        }
        VarTypeRef::Variant => DeclareParamType::Variant,
        VarTypeRef::Object(_)
        | VarTypeRef::Udt(_)
        | VarTypeRef::Array(_)
        | VarTypeRef::FixedArray { .. } => DeclareParamType::Any,
    }
}

fn provenance(module_name: &str, token: SyntaxToken<'_>) -> SourceProvenance {
    SourceProvenance {
        module_name: Some(module_name.to_string()),
        span: Some(SourceSpan {
            start: token.offset,
            end: token.offset + token.text.len() as u32,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ModuleAttributes, ModuleKind};
    use crate::model::ScopeKind;

    /// Scan one procedural module's source and return its scanned members.
    fn scan_members_for_kind(
        module_kind: ModuleKind,
        source: &str,
    ) -> Result<Vec<ScannedMember>, SymbolModelError> {
        scan_state_for_kind(module_kind, source).map(|(_, _, members)| members)
    }

    fn scan_state_for_kind(
        module_kind: ModuleKind,
        source: &str,
    ) -> Result<(SymbolTable, SignatureTable, Vec<ScannedMember>), SymbolModelError> {
        let module = ModuleUnit {
            module_name: "M".into(),
            module_kind,
            attributes: ModuleAttributes::named("M"),
            source: source.into(),
        };
        let parse = oxvba_syntax::parse(source);
        assert!(
            parse.errors().is_empty(),
            "parse errors: {:?}",
            parse.errors()
        );
        let mut symbols = SymbolTable::new();
        let mut signatures = SignatureTable::new();
        let mut next = 0u32;
        let project = symbols
            .add_scope(ScopeKind::Project, symbols.global_scope(), Some("P"))
            .unwrap();
        scan_module(
            &mut symbols,
            &mut signatures,
            &mut next,
            &module,
            parse.syntax(),
            project,
            ConditionalCompilationTarget::default(),
        )
        .map(|scan| (symbols, signatures, scan.members))
    }

    fn scan_members(source: &str) -> Vec<ScannedMember> {
        scan_members_for_kind(ModuleKind::Procedural, source).unwrap()
    }

    fn vis_of(members: &[ScannedMember], name: &str) -> Visibility {
        members
            .iter()
            .find(|m| m.name_folded == fold_identifier(name))
            .unwrap_or_else(|| panic!("member `{name}` not scanned"))
            .visibility
    }

    #[test]
    fn captures_declared_visibility_with_vba_defaults() {
        let members = scan_members(
            "Public Sub PubSub()\nEnd Sub\n\
             Private Sub PrivSub()\nEnd Sub\n\
             Sub BareSub()\nEnd Sub\n\
             Public x As Long\n\
             Dim y As Long\n\
             Private z As Long\n\
             Const K As Long = 1\n\
             Public Const PK As Long = 2\n\
             Public Enum E\n  A = 1\nEnd Enum\n",
        );
        // Sub/Function default Public; explicit Private overrides.
        assert_eq!(vis_of(&members, "PubSub"), Visibility::Public);
        assert_eq!(vis_of(&members, "PrivSub"), Visibility::Private);
        assert_eq!(vis_of(&members, "BareSub"), Visibility::Public);
        // Module variables default Private; `Public` exposes them.
        assert_eq!(vis_of(&members, "x"), Visibility::Public);
        assert_eq!(vis_of(&members, "y"), Visibility::Private);
        assert_eq!(vis_of(&members, "z"), Visibility::Private);
        // `Const` defaults Private; `Public Const` exposes it.
        assert_eq!(vis_of(&members, "K"), Visibility::Private);
        assert_eq!(vis_of(&members, "PK"), Visibility::Public);
        // `Enum` defaults Public; its members inherit the enum's visibility.
        assert_eq!(vis_of(&members, "E"), Visibility::Public);
        assert_eq!(vis_of(&members, "A"), Visibility::Public);
    }

    #[test]
    fn friend_is_distinct_from_public_and_private() {
        let members =
            scan_members_for_kind(ModuleKind::Class, "Friend Sub Helper()\nEnd Sub\n").unwrap();
        assert_eq!(vis_of(&members, "Helper"), Visibility::Friend);
    }

    #[test]
    fn scanner_rejects_friend_in_standard_modules() {
        let err = scan_members_for_kind(ModuleKind::Procedural, "Friend Sub Helper()\nEnd Sub\n")
            .expect_err("standard module Friend should be rejected");
        assert_eq!(
            err,
            SymbolModelError::FriendNotValidInStandardModule {
                name: "Helper".to_string()
            }
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-FRIEND-ONLY-VALID-IN-OBJECT-MODULE"
        );
    }

    #[test]
    fn invalid_literal_optional_default_does_not_publish_raw_metadata() {
        let (symbols, signatures, members) = scan_state_for_kind(
            ModuleKind::Procedural,
            "Sub S(Optional ByVal n As Long = \"abc\")\nEnd Sub\n",
        )
        .expect("scan succeeds; model validation rejects the default later");
        let member = members
            .iter()
            .find(|m| m.name_folded == fold_identifier("S"))
            .expect("S scanned");
        let symbol = symbols.symbol(member.symbol).expect("S symbol");
        let SymbolImpl::Signature(sig_id) = symbol.imp else {
            panic!("expected signature");
        };
        let signature = signatures.get(sig_id).expect("signature");
        assert_eq!(signature.params[0].default, None);
    }

    #[test]
    fn scanner_rejects_withevents_in_standard_modules() {
        let err =
            scan_members_for_kind(ModuleKind::Procedural, "Private WithEvents src As Clock\n")
                .expect_err("standard module WithEvents should be rejected");
        assert_eq!(
            err,
            SymbolModelError::WithEventsNotValidInStandardModule {
                name: "src".to_string()
            }
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-WITHEVENTS-ONLY-VALID-IN-OBJECT-MODULE"
        );
    }

    #[test]
    fn scanner_rejects_withevents_in_local_scope() {
        let err = scan_members_for_kind(
            ModuleKind::Class,
            "Public Sub Hook()\n    Dim WithEvents src As Clock\nEnd Sub\n",
        )
        .expect_err("local WithEvents should be rejected");
        assert_eq!(
            err,
            SymbolModelError::WithEventsNotValidInLocalScope {
                name: "src".to_string()
            }
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-WITHEVENTS-ONLY-VALID-AT-MODULE-LEVEL"
        );
    }

    #[test]
    fn scanner_rejects_withevents_const_declarations() {
        for source in [
            "Private Const WithEvents src As Long = 1\n",
            "Public Sub Hook()\n    Const WithEvents src As Long = 1\nEnd Sub\n",
        ] {
            let err = scan_members_for_kind(ModuleKind::Class, source)
                .expect_err("WithEvents Const should be rejected");
            assert_eq!(
                err,
                SymbolModelError::InvalidWithEventsConstDeclaration {
                    name: "src".to_string()
                }
            );
            assert_eq!(
                err.to_diagnostic().code.as_str(),
                "SYM-E-INVALID-WITHEVENTS-CONST-DECLARATION"
            );
        }
    }

    #[test]
    fn scanner_accepts_object_withevents_fields_in_class_modules() {
        let (symbols, _, members) =
            scan_state_for_kind(ModuleKind::Class, "Private WithEvents src As Clock\n")
                .expect("class WithEvents object field should scan");
        let member = members
            .iter()
            .find(|m| m.name_folded == fold_identifier("src"))
            .expect("WithEvents field member should publish");
        assert_eq!(member.kind, SymbolKind::WithEventsField);
        let symbol = symbols.symbol(member.symbol).expect("symbol");
        assert_eq!(
            symbol.imp,
            SymbolImpl::DeclaredType(VarTypeRef::Object("clock".to_string()))
        );
    }

    #[test]
    fn scanner_rejects_non_object_withevents_fields() {
        for source in [
            "Private WithEvents src As Long\n",
            "Private WithEvents src As Variant\n",
            "Private WithEvents src\n",
            "Private WithEvents src() As Clock\n",
        ] {
            let err = scan_members_for_kind(ModuleKind::Class, source)
                .expect_err("non-object WithEvents field should be rejected");
            assert_eq!(
                err,
                SymbolModelError::InvalidWithEventsFieldType {
                    name: "src".to_string()
                }
            );
            assert_eq!(
                err.to_diagnostic().code.as_str(),
                "SYM-E-INVALID-WITHEVENTS-FIELD-TYPE"
            );
        }
    }

    #[test]
    fn scanner_rejects_withevents_as_new_fields() {
        let err = scan_members_for_kind(ModuleKind::Class, "Private WithEvents src As New Clock\n")
            .expect_err("WithEvents As New field should be rejected");
        assert_eq!(
            err,
            SymbolModelError::InvalidWithEventsAutoInstantiation {
                name: "src".to_string()
            }
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-INVALID-WITHEVENTS-AUTO-INSTANTIATION"
        );
    }

    #[test]
    fn scanner_rejects_event_in_standard_modules() {
        let err = scan_members_for_kind(ModuleKind::Procedural, "Public Event Changed()\n")
            .expect_err("standard module Event should be rejected");
        assert_eq!(
            err,
            SymbolModelError::EventNotValidInStandardModule {
                name: "Changed".to_string()
            }
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-EVENT-ONLY-VALID-IN-OBJECT-MODULE"
        );
    }

    #[test]
    fn scanner_rejects_invalid_event_parameter_modifiers() {
        for (source, parameter, reason, diagnostic) in [
            (
                "Public Event Changed(Optional ByVal value As Long)\n",
                "value",
                "Event arguments cannot be Optional",
                "SYM-E-INVALID-OPTIONAL-PARAMETER-DECLARATION",
            ),
            (
                "Public Event Changed(ParamArray values() As Variant)\n",
                "values",
                "Event arguments cannot be ParamArray",
                "SYM-E-INVALID-PARAMARRAY-DECLARATION",
            ),
        ] {
            let err = scan_members_for_kind(ModuleKind::Class, source)
                .expect_err("invalid event argument modifier should be rejected");
            let matches_error = match &err {
                SymbolModelError::InvalidOptionalParameterDeclaration {
                    procedure,
                    parameter: actual_parameter,
                    reason: actual_reason,
                }
                | SymbolModelError::InvalidParamArrayDeclaration {
                    procedure,
                    parameter: actual_parameter,
                    reason: actual_reason,
                } => {
                    procedure == "Changed"
                        && actual_parameter == parameter
                        && *actual_reason == reason
                }
                _ => false,
            };
            assert!(matches_error, "unexpected error: {err:?}");
            assert_eq!(err.to_diagnostic().code.as_str(), diagnostic);
        }
    }

    #[test]
    fn scanner_rejects_implements_in_standard_modules() {
        let err = scan_members_for_kind(ModuleKind::Procedural, "Implements IFoo\n")
            .expect_err("standard module Implements should be rejected");
        assert_eq!(
            err,
            SymbolModelError::ImplementsNotValidInStandardModule {
                name: "IFoo".to_string()
            }
        );
        assert_eq!(
            err.to_diagnostic().code.as_str(),
            "SYM-E-IMPLEMENTS-ONLY-VALID-IN-OBJECT-MODULE"
        );
    }

    /// A UDT field declared with a trailing `()` array marker must type as
    /// `Array(element)`, not the scalar element — so a member-access index step
    /// (`o.Lines(i).Text`) resolves through the element UDT. A scalar UDT field
    /// keeps its scalar type.
    #[test]
    fn udt_array_field_types_as_array() {
        let source = "Private Type Inner\n  Text As String\nEnd Type\n\
                      Private Type Outer\n  Lines() As Inner\n  Scalar As Inner\nEnd Type\n";
        let parse = oxvba_syntax::parse(source);
        assert!(
            parse.errors().is_empty(),
            "parse errors: {:?}",
            parse.errors()
        );
        let roots = [parse.syntax()];
        let udts = collect_udt_fields(&roots);
        let outer = udts.get("outer").expect("Outer UDT scanned");
        let (_, lines_ty) = outer
            .iter()
            .find(|(name, _)| name == "lines")
            .expect("Lines field present");
        match lines_ty {
            VarTypeRef::Array(inner) => assert_eq!(
                **inner,
                VarTypeRef::Object("inner".into()),
                "array element should be the Inner type (object/UDT name)"
            ),
            other => panic!("expected Array element type, got {other:?}"),
        }
        let (_, scalar_ty) = outer
            .iter()
            .find(|(name, _)| name == "scalar")
            .expect("Scalar field present");
        assert_eq!(
            *scalar_ty,
            VarTypeRef::Object("inner".into()),
            "a scalar UDT field stays scalar"
        );
    }
}
