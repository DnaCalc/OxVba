//! `ModuleScanner` — walk a module's lossless CST and declare its symbols
//! (procedures with signatures, properties, events, `Declare`s, types/enums,
//! fields/consts) into the symbol table. One code path, used for the active
//! module and every sibling / referenced-project module. Ported from the legacy
//! `frontend_symbols` CST collection, signature-aware (no text-parsing fallback).

use std::collections::BTreeSet;

use oxvba_bundle::DeclareParamType;
use oxvba_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

use crate::manifest::ModuleUnit;
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
) -> Result<ModuleScan, SymbolModelError> {
    let module_name = if module.attributes.vb_name.is_empty() {
        module.module_name.clone()
    } else {
        module.attributes.vb_name.clone()
    };
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
    };
    let default_member_attrs = default_member_attributes(module_syntax);
    let mut ctx = ScanCtx {
        symbols,
        signatures,
        next_descriptor_id,
        scan: &mut scan,
        module_name: &module_name,
        default_member_attrs,
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
    default_member_attrs: BTreeSet<String>,
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
                    let declared_type = declared_var_type(declarator);
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
        let is_default = is_default_member_node(node)
            || self
                .default_member_attrs
                .contains(&fold_identifier(&logical));
        // Sub/Function/Property default to Public; `Private`/`Friend` override.
        let visibility = decl_visibility(node, Visibility::Public);
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
                    if module_level {
                        self.scan.members.push(ScannedMember {
                            name_folded: fold_identifier(&logical),
                            enum_name: None,
                            symbol,
                            kind: SymbolKind::Property,
                            namespace: SymbolNamespace::Procedure,
                            is_default,
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
        if module_level {
            self.scan.members.push(ScannedMember {
                name_folded: fold_identifier(&logical),
                enum_name: None,
                symbol,
                kind,
                namespace: SymbolNamespace::Procedure,
                is_default,
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
                        SymbolImpl::DeclaredType(param_type(param)),
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
        }
        Ok(())
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
                param_types.push(declare_param_type(&param_type(param)));
                param_by_ref.push(parameter_passing_mode(param) == PassingMode::ByRef);
                param_optional.push(parameter_has_modifier(param, SyntaxKind::KwOptional));
                if parameter_has_modifier(param, SyntaxKind::KwParamArray) {
                    param_param_array = true;
                }
            }
        }
        let return_type = node
            .return_type()
            .map(|t| declare_param_type(&type_ref_node(t)));

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
            for param in param_list.params() {
                let name = parameter_name_token(param)
                    .map(|t| normalize_identifier_token(t.text).to_string())
                    .unwrap_or_default();
                let optional = parameter_has_modifier(param, SyntaxKind::KwOptional);
                let default = default_from_param(param)
                    .or_else(|| optional.then_some(DefaultValue::VariantMissing));
                params.push(Param {
                    name,
                    ty: param_type(param),
                    mode: parameter_passing_mode(param),
                    optional,
                    param_array: parameter_has_modifier(param, SyntaxKind::KwParamArray),
                    default,
                });
            }
        }
        // An array return (`Function F() As Byte()`) is typed `Array(element)`, so a
        // whole-array `F = arr` assignment is a copy, not a scalar coercion (the proc
        // decl carries the return's `ArrayBounds` as a direct child).
        let return_type = node.return_type().map(type_ref_node).map(|t| {
            if node.array_bounds().is_some() {
                VarTypeRef::Array(Box::new(t))
            } else {
                t
            }
        });
        Signature {
            params,
            return_type,
            call_shape,
        }
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
        if module_level {
            self.scan.members.push(ScannedMember {
                name_folded: fold_identifier(name),
                symbol,
                kind,
                namespace,
                is_default: false,
                visibility,
                enum_name: enum_name.map(str::to_string),
            });
        }
        Ok(())
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

/// `Attribute VB_UserMemId = 0` inside the procedure → the type's default member.
fn is_default_member_node(node: SyntaxNode<'_>) -> bool {
    node.text().lines().any(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("vb_usermemid") && lower.replace(' ', "").contains("=0")
    })
}

/// Exported `.cls` files place member attributes after the member body:
/// `Attribute Value.VB_UserMemId = 0`. The parser keeps those as top-level
/// `AttributeStmt` nodes, so associate them with the logical member during scan.
fn default_member_attributes(root: SyntaxNode<'_>) -> BTreeSet<String> {
    let mut attrs = BTreeSet::new();
    collect_default_member_attributes(root, &mut attrs);
    attrs
}

fn collect_default_member_attributes(node: SyntaxNode<'_>, attrs: &mut BTreeSet<String>) {
    if node.kind() == SyntaxKind::AttributeStmt
        && let Some(member) = default_member_attribute_name(&node.text())
    {
        attrs.insert(fold_identifier(&member));
    }
    for child in node.child_nodes() {
        collect_default_member_attributes(child, attrs);
    }
}

fn default_member_attribute_name(text: &str) -> Option<String> {
    let compact = text.to_ascii_lowercase().replace([' ', '\t'], "");
    if !compact.starts_with("attribute") || !compact.contains(".vb_usermemid=0") {
        return None;
    }
    let after_keyword = text.trim().split_once(char::is_whitespace)?.1.trim();
    let (lhs, _) = after_keyword.split_once('=')?;
    let (member, attr) = lhs.trim().rsplit_once('.')?;
    if !attr.trim().eq_ignore_ascii_case("VB_UserMemId") {
        return None;
    }
    let member = member.trim();
    (!member.is_empty()).then(|| member.to_string())
}

/// Parse a parameter's literal default (`Optional x As Long = 5`). With the
/// `oxvba-syntax` parser fix, the `= default` is folded into the `Param` node, so
/// this reads the text after the (first) `=`.
fn default_from_param(node: SyntaxNode<'_>) -> Option<DefaultValue> {
    let text = node.text();
    let rhs = text.split_once('=')?.1.trim();
    parse_default_literal(rhs)
}

fn parse_default_literal(rhs: &str) -> Option<DefaultValue> {
    if rhs.eq_ignore_ascii_case("true") {
        return Some(DefaultValue::Bool(true));
    }
    if rhs.eq_ignore_ascii_case("false") {
        return Some(DefaultValue::Bool(false));
    }
    if let Some(inner) = rhs.strip_prefix('"') {
        let end = inner.find('"').unwrap_or(inner.len());
        return Some(DefaultValue::Str(inner[..end].to_string()));
    }
    // Numeric, with an optional sign.
    let (negate, body) = match rhs.strip_prefix('-') {
        Some(rest) => (true, rest.trim()),
        None => (false, rhs.strip_prefix('+').map(str::trim).unwrap_or(rhs)),
    };
    let token = body.split_whitespace().next().unwrap_or(body);
    if token.contains('.') || token.contains(['e', 'E']) && !token.starts_with('&') {
        let value: f64 = token.trim_end_matches(['!', '#', '@']).parse().ok()?;
        return Some(DefaultValue::F64(
            if negate { -value } else { value }.to_bits(),
        ));
    }
    let raw = parse_int_literal(token)?;
    let value = if negate { -raw } else { raw };
    Some(
        i32::try_from(value)
            .map(DefaultValue::I32)
            .unwrap_or(DefaultValue::I64(value)),
    )
}

fn parse_int_literal(text: &str) -> Option<i64> {
    let trimmed = text.trim_end_matches(['&', '!', '#', '@', '%']);
    if let Some(hex) = trimmed
        .strip_prefix("&H")
        .or_else(|| trimmed.strip_prefix("&h"))
    {
        return i64::from_str_radix(hex, 16).ok();
    }
    if let Some(oct) = trimmed
        .strip_prefix("&O")
        .or_else(|| trimmed.strip_prefix("&o"))
    {
        return i64::from_str_radix(oct, 8).ok();
    }
    trimmed.parse().ok()
}

fn is_identifier_like(kind: SyntaxKind) -> bool {
    kind == SyntaxKind::Ident || kind == SyntaxKind::BracketedIdent
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

fn parameter_name_token(node: SyntaxNode<'_>) -> Option<SyntaxToken<'_>> {
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
                if !in_type_ref && after_modifier && is_identifier_like(token.kind) =>
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

fn param_type(node: SyntaxNode<'_>) -> VarTypeRef {
    let base = node
        .child_nodes()
        .into_iter()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .map(type_ref_node)
        .unwrap_or(VarTypeRef::Variant);
    fixed_string_refine(base, node)
}

/// A declarator's declared type, refining `As String * N` to a fixed-length string,
/// and wrapping an **array** declarator (`x()` or `x(1 To 3)`) in [`VarTypeRef::Array`]
/// of its element type. The array wrap matters because the binder distinguishes a
/// whole-array assignment (`x = arr`, no scalar coercion) from a scalar store, and
/// reads the element type back through `Array(..)` for `ReDim`/`Erase`/frame layout —
/// without it a `Dim x() As Byte` would be typed as a scalar `Byte` and a whole-array
/// assignment would wrongly coerce the array to that scalar.
fn declared_var_type(declarator: SyntaxNode<'_>) -> VarTypeRef {
    let base = declarator
        .declared_type()
        .map(type_ref_node)
        .unwrap_or(VarTypeRef::Variant);
    let element = fixed_string_refine(base, declarator);
    if declarator.array_bounds().is_some() {
        return VarTypeRef::Array(Box::new(element));
    }
    element
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
                let ty = declared_var_type(f);
                Some((field, ty))
            })
            .collect();
        out.insert(name, fields);
    }
    for child in node.child_nodes() {
        collect_udt_fields_in(child, out);
    }
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

fn type_ref_node(node: SyntaxNode<'_>) -> VarTypeRef {
    let text = node.text();
    let name = strip_leading_new_keyword(&text)
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim();
    let name = normalize_identifier_token(name);
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
        VarTypeRef::Object(_) | VarTypeRef::Udt(_) | VarTypeRef::Array(_) => DeclareParamType::Any,
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
    fn scan_members(source: &str) -> Vec<ScannedMember> {
        let module = ModuleUnit {
            module_name: "M".into(),
            module_kind: ModuleKind::Procedural,
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
        )
        .unwrap()
        .members
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
        let members = scan_members("Friend Sub Helper()\nEnd Sub\n");
        assert_eq!(vis_of(&members, "Helper"), Visibility::Friend);
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
