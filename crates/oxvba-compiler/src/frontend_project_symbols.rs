use std::collections::BTreeMap;

use crate::{
    frontend_symbols::{
        ScopeId, ScopeKind, SourceProvenance, SymbolId, SymbolModel, SymbolModelError,
        SymbolNamespace, collect_symbols_from_source_into_model, fold_identifier,
    },
    project::{ModuleKind, ModuleUnit, ProjectManifest},
    resolve::normalize_source_lines,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedName {
    pub parts: Vec<String>,
}

impl QualifiedName {
    pub fn new(parts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            parts: parts.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectSymbolKind {
    Project,
    Module,
    Class,
    Procedure,
    PropertyGet,
    PropertyLet,
    PropertySet,
    Event,
    Field,
    WithEventsField,
    Public,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectSymbolRoute {
    pub symbol: SymbolId,
    pub kind: ProjectSymbolKind,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectSymbolTables {
    project: Option<ProjectSymbolRoute>,
    project_name: Option<String>,
    modules: BTreeMap<String, ProjectSymbolRoute>,
    classes: BTreeMap<String, ProjectSymbolRoute>,
    public_symbols: BTreeMap<String, Vec<ProjectSymbolRoute>>,
    module_members: BTreeMap<(String, String), Vec<ProjectSymbolRoute>>,
    class_members: BTreeMap<(String, String), Vec<ProjectSymbolRoute>>,
    default_members: BTreeMap<String, Vec<ProjectSymbolRoute>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFieldArrayDescriptor {
    pub owner: String,
    pub field: String,
    pub bounds: Vec<(i32, i32)>,
    pub dynamic: bool,
}

#[derive(Debug, Clone)]
pub struct ProjectSymbolIndex {
    pub symbols: SymbolModel,
    pub tables: ProjectSymbolTables,
    pub project_scope: ScopeId,
    pub module_scopes: BTreeMap<String, ScopeId>,
    pub field_arrays: BTreeMap<(String, String), ProjectFieldArrayDescriptor>,
}

impl ProjectSymbolTables {
    pub fn record_project(&mut self, symbol: SymbolId) {
        self.project = Some(ProjectSymbolRoute {
            symbol,
            kind: ProjectSymbolKind::Project,
        });
    }

    pub fn record_project_named(&mut self, name: &str, symbol: SymbolId) {
        self.record_project(symbol);
        self.project_name = Some(fold_identifier(name));
    }

    pub fn record_module(&mut self, name: &str, symbol: SymbolId) {
        self.modules.insert(
            fold_identifier(name),
            ProjectSymbolRoute {
                symbol,
                kind: ProjectSymbolKind::Module,
            },
        );
    }

    pub fn record_class(&mut self, name: &str, symbol: SymbolId) {
        self.classes.insert(
            fold_identifier(name),
            ProjectSymbolRoute {
                symbol,
                kind: ProjectSymbolKind::Class,
            },
        );
    }

    pub fn record_public_symbol(&mut self, name: &str, symbol: SymbolId) {
        self.record_public_route(name, symbol, ProjectSymbolKind::Public);
    }

    pub fn record_public_route(&mut self, name: &str, symbol: SymbolId, kind: ProjectSymbolKind) {
        self.public_symbols
            .entry(fold_identifier(name))
            .or_default()
            .push(ProjectSymbolRoute { symbol, kind });
    }

    pub fn record_module_member(
        &mut self,
        module: &str,
        name: &str,
        kind: ProjectSymbolKind,
        symbol: SymbolId,
    ) {
        self.module_members
            .entry((fold_identifier(module), fold_identifier(name)))
            .or_default()
            .push(ProjectSymbolRoute { symbol, kind });
    }

    pub fn record_class_member(
        &mut self,
        class: &str,
        name: &str,
        kind: ProjectSymbolKind,
        symbol: SymbolId,
    ) {
        self.class_members
            .entry((fold_identifier(class), fold_identifier(name)))
            .or_default()
            .push(ProjectSymbolRoute { symbol, kind });
    }

    pub fn record_default_member(
        &mut self,
        owner: &str,
        symbol: SymbolId,
        kind: ProjectSymbolKind,
    ) {
        self.default_members
            .entry(fold_identifier(owner))
            .or_default()
            .push(ProjectSymbolRoute { symbol, kind });
    }

    pub fn resolve_unqualified(&self, name: &str) -> Option<ProjectSymbolRoute> {
        let name = fold_identifier(name);
        self.public_symbols
            .get(&name)
            .and_then(|routes| unique_route(routes))
            .or_else(|| self.modules.get(&name).copied())
            .or_else(|| self.classes.get(&name).copied())
    }

    pub fn resolve_public_candidates(&self, name: &str) -> Vec<ProjectSymbolRoute> {
        self.public_symbols
            .get(&fold_identifier(name))
            .cloned()
            .unwrap_or_default()
    }

    pub fn resolve_class(&self, name: &str) -> Option<ProjectSymbolRoute> {
        self.classes.get(&fold_identifier(name)).copied()
    }

    pub fn resolve_qualified(&self, name: &QualifiedName) -> Option<ProjectSymbolRoute> {
        match name.parts.as_slice() {
            [single] => self.resolve_unqualified(single),
            [owner, member] => self.resolve_owner_member(owner, member),
            [project, owner, member] if self.project_matches(project) => {
                self.resolve_owner_member(owner, member)
            }
            _ => None,
        }
    }

    fn project_matches(&self, name: &str) -> bool {
        self.project_name
            .as_ref()
            .is_some_and(|project_name| project_name == &fold_identifier(name))
    }

    fn resolve_owner_member(&self, owner: &str, member: &str) -> Option<ProjectSymbolRoute> {
        unique_route(&self.resolve_owner_member_candidates(owner, member))
    }

    pub fn resolve_property_accessor(
        &self,
        owner: &str,
        member: &str,
        kind: ProjectSymbolKind,
    ) -> Option<ProjectSymbolRoute> {
        if !matches!(
            kind,
            ProjectSymbolKind::PropertyGet
                | ProjectSymbolKind::PropertyLet
                | ProjectSymbolKind::PropertySet
        ) {
            return None;
        }
        self.resolve_owner_member_candidates(owner, member)
            .into_iter()
            .find(|route| route.kind == kind)
    }

    pub fn resolve_default_member_accessor(
        &self,
        owner: &str,
        kind: ProjectSymbolKind,
    ) -> Option<ProjectSymbolRoute> {
        if !matches!(
            kind,
            ProjectSymbolKind::PropertyGet
                | ProjectSymbolKind::PropertyLet
                | ProjectSymbolKind::PropertySet
        ) {
            return None;
        }
        let mut routes = self
            .default_members
            .get(&fold_identifier(owner))
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|route| route.kind == kind)
            .collect::<Vec<_>>();
        routes.sort_by_key(|route| (route.symbol.0, route_kind_order(route.kind)));
        routes.dedup();
        unique_route(&routes)
    }

    pub fn resolve_default_or_single_property_accessor(
        &self,
        owner: &str,
        kind: ProjectSymbolKind,
    ) -> Option<ProjectSymbolRoute> {
        self.resolve_default_member_accessor(owner, kind)
            .or_else(|| {
                let mut routes = self
                    .owner_member_routes(owner)
                    .into_iter()
                    .filter(|route| route.kind == kind)
                    .collect::<Vec<_>>();
                routes.sort_by_key(|route| (route.symbol.0, route_kind_order(route.kind)));
                routes.dedup();
                unique_route(&routes)
            })
    }

    pub fn resolve_event(&self, owner: &str, event: &str) -> Option<ProjectSymbolRoute> {
        self.resolve_owner_member_of_kind(owner, event, ProjectSymbolKind::Event)
    }

    pub fn resolve_owner_member_of_kind(
        &self,
        owner: &str,
        member: &str,
        kind: ProjectSymbolKind,
    ) -> Option<ProjectSymbolRoute> {
        self.resolve_owner_member_candidates(owner, member)
            .into_iter()
            .find(|route| route.kind == kind)
    }

    fn owner_member_routes(&self, owner: &str) -> Vec<ProjectSymbolRoute> {
        let owner = fold_identifier(owner);
        let mut routes = Vec::new();
        for ((route_owner, _), owner_routes) in &self.module_members {
            if route_owner == &owner {
                routes.extend(owner_routes.iter().copied());
            }
        }
        for ((route_owner, _), owner_routes) in &self.class_members {
            if route_owner == &owner {
                routes.extend(owner_routes.iter().copied());
            }
        }
        routes.sort_by_key(|route| (route.symbol.0, route_kind_order(route.kind)));
        routes.dedup();
        routes
    }

    fn resolve_owner_member_candidates(
        &self,
        owner: &str,
        member: &str,
    ) -> Vec<ProjectSymbolRoute> {
        let owner = fold_identifier(owner);
        let member = fold_identifier(member);
        let mut routes = self
            .module_members
            .get(&(owner.clone(), member.clone()))
            .cloned()
            .unwrap_or_default();
        routes.extend(
            self.class_members
                .get(&(owner, member))
                .cloned()
                .unwrap_or_default(),
        );
        routes.sort_by_key(|route| (route.symbol.0, route_kind_order(route.kind)));
        routes.dedup();
        routes
    }
}

impl ProjectSymbolIndex {
    pub fn resolve_class_route(&self, name: &str) -> Option<ProjectSymbolRoute> {
        self.tables.resolve_class(name)
    }

    pub fn resolve_class_field_names(&self, owner: &str) -> Vec<String> {
        let owner = fold_identifier(owner);
        let mut names = self
            .tables
            .class_members
            .iter()
            .filter_map(|((route_owner, member), routes)| {
                (route_owner == &owner
                    && routes
                        .iter()
                        .any(|route| route.kind == ProjectSymbolKind::Field))
                .then_some(member.clone())
            })
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    pub fn resolve_class_withevents_field_names(&self, owner: &str) -> Vec<String> {
        let owner = fold_identifier(owner);
        let mut names = self
            .tables
            .class_members
            .iter()
            .filter_map(|((route_owner, member), routes)| {
                (route_owner == &owner
                    && routes
                        .iter()
                        .any(|route| route.kind == ProjectSymbolKind::WithEventsField))
                .then_some(member.clone())
            })
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    pub fn resolve_field_array_descriptor(
        &self,
        owner: &str,
        field: &str,
    ) -> Option<&ProjectFieldArrayDescriptor> {
        self.field_arrays
            .get(&(fold_identifier(owner), fold_identifier(field)))
    }
}

fn unique_route(routes: &[ProjectSymbolRoute]) -> Option<ProjectSymbolRoute> {
    match routes {
        [route] => Some(*route),
        _ => None,
    }
}

fn route_kind_order(kind: ProjectSymbolKind) -> u8 {
    match kind {
        ProjectSymbolKind::Project => 0,
        ProjectSymbolKind::Module => 1,
        ProjectSymbolKind::Class => 2,
        ProjectSymbolKind::Procedure => 3,
        ProjectSymbolKind::PropertyGet => 4,
        ProjectSymbolKind::PropertyLet => 5,
        ProjectSymbolKind::PropertySet => 6,
        ProjectSymbolKind::Event => 7,
        ProjectSymbolKind::Field => 8,
        ProjectSymbolKind::WithEventsField => 9,
        ProjectSymbolKind::Public => 10,
    }
}

pub fn build_project_symbol_index_from_manifest(
    manifest: &ProjectManifest,
) -> Result<ProjectSymbolIndex, SymbolModelError> {
    let mut symbols = SymbolModel::default();
    let mut tables = ProjectSymbolTables::default();
    let mut field_arrays = BTreeMap::new();
    let project_scope = symbols.add_scope(
        ScopeKind::Project,
        symbols.global_scope(),
        Some(&manifest.project_name),
    )?;
    let project_symbol = symbols.declare_symbol(
        symbols.global_scope(),
        SymbolNamespace::Project,
        &manifest.project_name,
        SourceProvenance {
            module_name: None,
            span: None,
        },
    )?;
    tables.record_project_named(&manifest.project_name, project_symbol);
    tables.record_public_symbol(&manifest.project_name, project_symbol);

    let mut module_scopes = BTreeMap::new();
    for module in &manifest.modules {
        index_module(
            module,
            &mut symbols,
            &mut tables,
            &mut field_arrays,
            project_scope,
            &mut module_scopes,
        )?;
    }

    Ok(ProjectSymbolIndex {
        symbols,
        tables,
        project_scope,
        module_scopes,
        field_arrays,
    })
}

fn index_module(
    module: &ModuleUnit,
    symbols: &mut SymbolModel,
    tables: &mut ProjectSymbolTables,
    field_arrays: &mut BTreeMap<(String, String), ProjectFieldArrayDescriptor>,
    project_scope: ScopeId,
    module_scopes: &mut BTreeMap<String, ScopeId>,
) -> Result<(), SymbolModelError> {
    let module_name = manifest_module_name(module);
    let provenance = SourceProvenance {
        module_name: Some(module_name.clone()),
        span: None,
    };
    let module_symbol = symbols.declare_symbol(
        project_scope,
        SymbolNamespace::Module,
        &module_name,
        provenance.clone(),
    )?;
    let module_scope = symbols.add_scope(ScopeKind::Module, project_scope, Some(&module_name))?;
    module_scopes.insert(fold_identifier(&module_name), module_scope);
    tables.record_module(&module_name, module_symbol);

    let class_symbol = if matches!(
        module.module_kind,
        ModuleKind::Class | ModuleKind::Document | ModuleKind::Form
    ) {
        let symbol = symbols.declare_symbol(
            project_scope,
            SymbolNamespace::Type,
            &module_name,
            provenance,
        )?;
        tables.record_class(&module_name, symbol);
        tables.record_public_route(&module_name, symbol, ProjectSymbolKind::Class);
        Some(symbol)
    } else {
        None
    };
    let default_member_names = collect_default_member_attribute_names(&module.source);
    let declared_event_names = collect_declared_event_names(&module.source);
    let withevents_field_names = collect_withevents_field_names(&module.source);
    for descriptor in collect_field_array_descriptors(&module_name, &module.source) {
        field_arrays.insert(
            (
                fold_identifier(&descriptor.owner),
                fold_identifier(&descriptor.field),
            ),
            descriptor,
        );
    }

    match collect_symbols_from_source_into_model(
        symbols,
        &module_name,
        module_scope,
        &module.source,
    ) {
        Ok(()) => {}
        Err(SymbolModelError::Syntax(_)) => {
            collect_legacy_line_symbols_into_model(
                symbols,
                &module_name,
                module_scope,
                &module.source,
            )?;
        }
        Err(error) => return Err(error),
    }
    for symbol_id in symbols.symbols_in_scope(module_scope)? {
        let Some(symbol) = symbols.symbol(symbol_id) else {
            continue;
        };
        let Some(name) = symbols
            .name(symbol.name)
            .map(|name| name.first_spelling.clone())
        else {
            continue;
        };
        match symbol.namespace {
            SymbolNamespace::Procedure => {
                let kind = property_route_kind(&name).unwrap_or(ProjectSymbolKind::Procedure);
                tables.record_module_member(&module_name, &name, kind, symbol_id);
                if let Some(alias) = property_group_alias(&name) {
                    tables.record_module_member(&module_name, alias, kind, symbol_id);
                    if default_member_names.contains_key(&fold_identifier(alias)) {
                        tables.record_default_member(&module_name, symbol_id, kind);
                    }
                }
                if class_symbol.is_some() {
                    tables.record_class_member(&module_name, &name, kind, symbol_id);
                    if let Some(alias) = property_group_alias(&name) {
                        tables.record_class_member(&module_name, alias, kind, symbol_id);
                        if default_member_names.contains_key(&fold_identifier(alias)) {
                            tables.record_default_member(&module_name, symbol_id, kind);
                        }
                    }
                }
                if is_unqualified_public_symbol_candidate(module) {
                    tables.record_public_route(&name, symbol_id, kind);
                    if let Some(alias) = property_group_alias(&name) {
                        tables.record_public_route(alias, symbol_id, kind);
                    }
                }
            }
            SymbolNamespace::Local | SymbolNamespace::Member => {
                let kind = if declared_event_names.contains_key(&fold_identifier(&name)) {
                    ProjectSymbolKind::Event
                } else if withevents_field_names.contains_key(&fold_identifier(&name)) {
                    ProjectSymbolKind::WithEventsField
                } else {
                    ProjectSymbolKind::Field
                };
                tables.record_module_member(&module_name, &name, kind, symbol_id);
                if class_symbol.is_some() {
                    tables.record_class_member(&module_name, &name, kind, symbol_id);
                }
            }
            SymbolNamespace::Type => {
                tables.record_module_member(
                    &module_name,
                    &name,
                    ProjectSymbolKind::Class,
                    symbol_id,
                );
                if is_unqualified_public_symbol_candidate(module) {
                    tables.record_public_route(&name, symbol_id, ProjectSymbolKind::Class);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn manifest_module_name(module: &ModuleUnit) -> String {
    if module.attributes.vb_name.trim().is_empty() {
        module.module_name.clone()
    } else {
        module.attributes.vb_name.clone()
    }
}

fn is_unqualified_public_symbol_candidate(module: &ModuleUnit) -> bool {
    module.module_kind == ModuleKind::Procedural && !module.attributes.option_private_module
}

fn property_group_alias(name: &str) -> Option<&str> {
    name.strip_prefix("property_get_")
        .or_else(|| name.strip_prefix("property_let_"))
        .or_else(|| name.strip_prefix("property_set_"))
}

fn property_route_kind(name: &str) -> Option<ProjectSymbolKind> {
    if name.starts_with("property_get_") {
        Some(ProjectSymbolKind::PropertyGet)
    } else if name.starts_with("property_let_") {
        Some(ProjectSymbolKind::PropertyLet)
    } else if name.starts_with("property_set_") {
        Some(ProjectSymbolKind::PropertySet)
    } else {
        None
    }
}

fn collect_default_member_attribute_names(source: &str) -> BTreeMap<String, ()> {
    normalize_source_lines(source)
        .into_iter()
        .filter_map(|line| {
            default_member_attribute_name(&line).map(|name| (fold_identifier(name), ()))
        })
        .collect()
}

fn default_member_attribute_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("attribute ") || !lower.contains(".vb_usermemid") {
        return None;
    }
    let payload = trimmed[10..].trim();
    let dot = payload.find('.')?;
    let name = payload[..dot].trim();
    let eq = payload.find('=')?;
    let value = payload[eq + 1..].trim();
    (value == "0" && !name.is_empty()).then_some(name)
}

fn collect_declared_event_names(source: &str) -> BTreeMap<String, ()> {
    normalize_source_lines(source)
        .into_iter()
        .filter_map(|line| declared_event_name(&line).map(|name| (fold_identifier(name), ())))
        .collect()
}

fn collect_withevents_field_names(source: &str) -> BTreeMap<String, ()> {
    normalize_source_lines(source)
        .into_iter()
        .filter_map(|line| withevents_field_name(&line).map(|name| (fold_identifier(name), ())))
        .collect()
}

fn collect_field_array_descriptors(owner: &str, source: &str) -> Vec<ProjectFieldArrayDescriptor> {
    let option_base = module_option_base(source);
    let mut descriptors = Vec::new();
    let mut in_procedure = false;
    let mut in_type_or_enum = false;
    for line in normalize_source_lines(source) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if legacy_line_procedure_symbol_name(trimmed).is_some() {
            in_procedure = true;
            continue;
        }
        if lower == "end sub" || lower == "end function" || lower == "end property" {
            in_procedure = false;
            continue;
        }
        if lower.starts_with("type ")
            || lower.starts_with("public type ")
            || lower.starts_with("private type ")
        {
            in_type_or_enum = true;
            continue;
        }
        if lower.starts_with("enum ")
            || lower.starts_with("public enum ")
            || lower.starts_with("private enum ")
        {
            in_type_or_enum = true;
            continue;
        }
        if lower == "end type" || lower == "end enum" {
            in_type_or_enum = false;
            continue;
        }
        if in_procedure || in_type_or_enum {
            continue;
        }
        let Some(rest) = field_array_declaration_payload(trimmed) else {
            continue;
        };
        let rest_lower = rest.to_ascii_lowercase();
        if rest_lower.starts_with("withevents ")
            || rest_lower.starts_with("sub ")
            || rest_lower.starts_with("function ")
            || rest_lower.starts_with("property ")
            || rest_lower.starts_with("event ")
            || rest_lower.starts_with("declare ")
            || rest_lower.starts_with("const ")
        {
            continue;
        }
        for segment in split_decl_segments(rest) {
            if let Some(descriptor) = parse_field_array_descriptor(owner, segment, option_base) {
                descriptors.push(descriptor);
            }
        }
    }
    descriptors
}

fn field_array_declaration_payload(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("dim ") {
        return Some(trimmed[4..].trim_start());
    }
    if lower.starts_with("global ") {
        return Some(trimmed[7..].trim_start());
    }
    let stripped = strip_access_modifiers(trimmed);
    if stripped.len() == trimmed.len() {
        return None;
    }
    let lower = stripped.to_ascii_lowercase();
    if lower.starts_with("dim ") {
        Some(stripped[4..].trim_start())
    } else {
        Some(stripped)
    }
}

fn module_option_base(source: &str) -> i32 {
    normalize_source_lines(source)
        .into_iter()
        .find_map(|line| {
            let trimmed = line.trim();
            let lower = trimmed.to_ascii_lowercase();
            let rest = lower.strip_prefix("option base ")?;
            match rest.trim() {
                "1" => Some(1),
                "0" => Some(0),
                _ => None,
            }
        })
        .unwrap_or(0)
}

fn split_decl_segments(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                let segment = text[start..idx].trim();
                if !segment.is_empty() {
                    out.push(segment);
                }
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    let segment = text[start..].trim();
    if !segment.is_empty() {
        out.push(segment);
    }
    out
}

fn parse_field_array_descriptor(
    owner: &str,
    segment: &str,
    option_base: i32,
) -> Option<ProjectFieldArrayDescriptor> {
    let name_part = split_keyword_ci(segment, "as")
        .map(|(lhs, _)| lhs.trim())
        .unwrap_or(segment.trim());
    let open = name_part.find('(')?;
    let close = name_part.rfind(')')?;
    if close <= open {
        return None;
    }
    let field = identifier_after_keyword(name_part[..open].trim(), 0)?.to_string();
    let bounds_text = name_part[open + 1..close].trim();
    let dynamic = bounds_text.is_empty();
    let bounds = if dynamic {
        Vec::new()
    } else {
        parse_field_array_bounds(bounds_text, option_base)?
    };
    Some(ProjectFieldArrayDescriptor {
        owner: owner.to_string(),
        field,
        bounds,
        dynamic,
    })
}

fn parse_field_array_bounds(raw: &str, option_base: i32) -> Option<Vec<(i32, i32)>> {
    let mut bounds = Vec::new();
    for dim in split_decl_segments(raw) {
        let (lower, upper) = if let Some((lhs, rhs)) = split_keyword_ci(dim, "to") {
            (lhs.trim().parse().ok()?, rhs.trim().parse().ok()?)
        } else {
            (option_base, dim.trim().parse().ok()?)
        };
        if upper < lower {
            return None;
        }
        bounds.push((lower, upper));
    }
    (!bounds.is_empty()).then_some(bounds)
}

fn split_keyword_ci<'a>(text: &'a str, keyword: &str) -> Option<(&'a str, &'a str)> {
    let lower = text.to_ascii_lowercase();
    let keyword = keyword.to_ascii_lowercase();
    let mut start = 0usize;
    while let Some(rel) = lower[start..].find(&keyword) {
        let idx = start + rel;
        let end = idx + keyword.len();
        let before_ok = lower[..idx]
            .chars()
            .next_back()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_');
        let after_ok = lower[end..]
            .chars()
            .next()
            .is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_');
        if before_ok && after_ok {
            return Some((&text[..idx], &text[end..]));
        }
        start = end;
    }
    None
}

fn withevents_field_name(line: &str) -> Option<&str> {
    let rest = strip_access_modifiers(line.trim());
    let lower = rest.to_ascii_lowercase();
    let payload = if lower.starts_with("withevents ") {
        rest[11..].trim_start()
    } else if lower.starts_with("dim withevents ") {
        rest[15..].trim_start()
    } else {
        return None;
    };
    identifier_after_keyword(payload, 0)
}

fn declared_event_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    let payload = if lower.starts_with("event ") {
        trimmed[6..].trim()
    } else if lower.starts_with("public event ") {
        trimmed[13..].trim()
    } else if lower.starts_with("private event ") {
        trimmed[14..].trim()
    } else {
        return None;
    };
    let name = payload
        .split(|ch: char| ch.is_ascii_whitespace() || ch == '(')
        .next()
        .unwrap_or_default()
        .trim();
    (!name.is_empty()).then_some(name)
}

fn collect_legacy_line_symbols_into_model(
    symbols: &mut SymbolModel,
    module_name: &str,
    module_scope: ScopeId,
    source: &str,
) -> Result<(), SymbolModelError> {
    let provenance = SourceProvenance {
        module_name: Some(module_name.to_string()),
        span: None,
    };
    for line in normalize_source_lines(source) {
        if let Some(name) = legacy_line_procedure_symbol_name(&line) {
            declare_legacy_symbol(
                symbols,
                module_scope,
                SymbolNamespace::Procedure,
                &name,
                provenance.clone(),
            )?;
        } else if let Some(name) = legacy_line_field_symbol_name(&line) {
            declare_legacy_symbol(
                symbols,
                module_scope,
                SymbolNamespace::Local,
                name,
                provenance.clone(),
            )?;
        }
    }
    Ok(())
}

fn declare_legacy_symbol(
    symbols: &mut SymbolModel,
    module_scope: ScopeId,
    namespace: SymbolNamespace,
    name: &str,
    provenance: SourceProvenance,
) -> Result<(), SymbolModelError> {
    if symbols
        .find_in_scope(module_scope, namespace, name)?
        .is_none()
    {
        symbols.declare_symbol(module_scope, namespace, name, provenance)?;
    }
    Ok(())
}

fn legacy_line_procedure_symbol_name(line: &str) -> Option<String> {
    let mut rest = strip_access_modifiers(line.trim());
    let lower = rest.to_ascii_lowercase();
    if lower.starts_with("declare ") {
        rest = rest[8..].trim_start();
        let lower = rest.to_ascii_lowercase();
        if lower.starts_with("ptrsafe ") {
            rest = rest[8..].trim_start();
        }
    }
    let lower = rest.to_ascii_lowercase();
    if lower.starts_with("sub ") {
        return identifier_after_keyword(rest, 4).map(str::to_string);
    }
    if lower.starts_with("function ") {
        return identifier_after_keyword(rest, 9).map(str::to_string);
    }
    if lower.starts_with("property get ") {
        return identifier_after_keyword(rest, 13).map(|name| format!("property_get_{name}"));
    }
    if lower.starts_with("property let ") {
        return identifier_after_keyword(rest, 13).map(|name| format!("property_let_{name}"));
    }
    if lower.starts_with("property set ") {
        return identifier_after_keyword(rest, 13).map(|name| format!("property_set_{name}"));
    }
    None
}

fn legacy_line_field_symbol_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    let rest = if lower.starts_with("dim ") {
        &trimmed[4..]
    } else if lower.starts_with("const ") {
        &trimmed[6..]
    } else if lower.starts_with("public ") || lower.starts_with("private ") {
        strip_access_modifiers(trimmed)
    } else {
        return None;
    };
    let lower = rest.to_ascii_lowercase();
    let rest = if lower.starts_with("dim ") {
        &rest[4..]
    } else if lower.starts_with("const ") {
        &rest[6..]
    } else {
        rest
    };
    let lower = rest.to_ascii_lowercase();
    if lower.starts_with("sub ")
        || lower.starts_with("function ")
        || lower.starts_with("property ")
        || lower.starts_with("declare ")
        || lower.starts_with("event ")
    {
        return None;
    }
    identifier_after_keyword(rest, 0)
}

fn strip_access_modifiers(mut text: &str) -> &str {
    loop {
        let lower = text.to_ascii_lowercase();
        let Some(prefix_len) = ["public ", "private ", "friend ", "static "]
            .iter()
            .find_map(|prefix| lower.starts_with(prefix).then_some(prefix.len()))
        else {
            return text.trim_start();
        };
        text = text[prefix_len..].trim_start();
    }
}

fn identifier_after_keyword(text: &str, start: usize) -> Option<&str> {
    let tail = text.get(start..)?.trim_start();
    let name = tail
        .split(|ch: char| ch.is_ascii_whitespace() || ch == '(' || ch == ',' || ch == ':')
        .next()
        .unwrap_or_default()
        .trim();
    (!name.is_empty()).then_some(name)
}

pub fn seed_project_symbol_table_from_symbols(
    symbols: &mut SymbolModel,
    project_name: &str,
    module_names: &[&str],
) -> ProjectSymbolTables {
    let mut table = ProjectSymbolTables::default();
    if let Some(project) = symbols
        .find_in_scope(
            symbols.global_scope(),
            SymbolNamespace::Project,
            project_name,
        )
        .ok()
        .flatten()
    {
        table.record_project_named(project_name, project);
        table.record_public_symbol(project_name, project);
    }
    for module in module_names {
        if let Some(symbol) = symbols
            .find_in_scope(symbols.global_scope(), SymbolNamespace::Module, module)
            .ok()
            .flatten()
        {
            table.record_module(module, symbol);
        }
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend_symbols::{FrontendSourceSpan, SourceProvenance};
    use crate::project::{ModuleAttributes, ProjectKind, module_unit_from_source};

    fn provenance() -> SourceProvenance {
        SourceProvenance {
            module_name: Some("Module1".to_string()),
            span: Some(FrontendSourceSpan { start: 0, end: 1 }),
        }
    }

    #[test]
    fn project_symbol_tables_resolve_module_and_public_names_case_insensitively() {
        let mut symbols = SymbolModel::default();
        let project = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Project,
                "Book1",
                provenance(),
            )
            .expect("project");
        let module = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Module,
                "Module1",
                provenance(),
            )
            .expect("module");

        let mut table = seed_project_symbol_table_from_symbols(&mut symbols, "book1", &["module1"]);
        table.record_public_symbol("PublicValue", project);

        assert_eq!(
            table.resolve_unqualified("MODULE1"),
            Some(ProjectSymbolRoute {
                symbol: module,
                kind: ProjectSymbolKind::Module,
            })
        );
        assert_eq!(
            table.resolve_unqualified("publicvalue"),
            Some(ProjectSymbolRoute {
                symbol: project,
                kind: ProjectSymbolKind::Public,
            })
        );
    }

    #[test]
    fn project_symbol_tables_resolve_module_procedure_and_field_routes() {
        let mut symbols = SymbolModel::default();
        let module = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Module,
                "Module1",
                provenance(),
            )
            .expect("module");
        let proc_symbol = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Procedure,
                "Run",
                provenance(),
            )
            .expect("procedure");
        let field_symbol = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Member,
                "Value",
                provenance(),
            )
            .expect("field");

        let mut table = ProjectSymbolTables::default();
        table.record_module("Module1", module);
        table.record_module_member("Module1", "Run", ProjectSymbolKind::Procedure, proc_symbol);
        table.record_module_member("Module1", "Value", ProjectSymbolKind::Field, field_symbol);

        assert_eq!(
            table.resolve_qualified(&QualifiedName::new(["module1", "run"])),
            Some(ProjectSymbolRoute {
                symbol: proc_symbol,
                kind: ProjectSymbolKind::Procedure,
            })
        );
        assert_eq!(
            table.resolve_qualified(&QualifiedName::new(["MODULE1", "VALUE"])),
            Some(ProjectSymbolRoute {
                symbol: field_symbol,
                kind: ProjectSymbolKind::Field,
            })
        );
        table.record_project_named("Book1", module);
        assert_eq!(
            table.resolve_qualified(&QualifiedName::new(["book1", "module1", "run"])),
            Some(ProjectSymbolRoute {
                symbol: proc_symbol,
                kind: ProjectSymbolKind::Procedure,
            })
        );
        assert_eq!(
            table.resolve_qualified(&QualifiedName::new(["other", "module1", "run"])),
            None
        );
    }

    #[test]
    fn project_symbol_tables_resolve_class_members_separately_from_modules() {
        let mut symbols = SymbolModel::default();
        let class = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Type,
                "Customer",
                provenance(),
            )
            .expect("class");
        let member = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Member,
                "Name",
                provenance(),
            )
            .expect("member");

        let mut table = ProjectSymbolTables::default();
        table.record_class("Customer", class);
        table.record_class_member("Customer", "Name", ProjectSymbolKind::Field, member);

        assert_eq!(
            table.resolve_qualified(&QualifiedName::new(["customer", "name"])),
            Some(ProjectSymbolRoute {
                symbol: member,
                kind: ProjectSymbolKind::Field,
            })
        );
    }

    #[test]
    fn project_symbol_index_resolves_class_routes_and_field_names() {
        let module = ModuleUnit {
            module_name: "Widget".to_string(),
            module_kind: ModuleKind::Class,
            attributes: Default::default(),
            source:
                "Private score As Long\nPrivate WithEvents source As Emitter\nPublic Function Value() As Long\nEnd Function"
                    .to_string(),
        };
        let manifest = ProjectManifest {
            project_name: "Book1".to_string(),
            project_kind: crate::project::ProjectKind::Source,
            modules: vec![module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let index =
            build_project_symbol_index_from_manifest(&manifest).expect("index should build");
        assert_eq!(
            index.resolve_class_route("widget").map(|route| route.kind),
            Some(ProjectSymbolKind::Class)
        );
        assert_eq!(
            index.resolve_class_field_names("widget"),
            vec!["score".to_string()]
        );
        assert_eq!(
            index.resolve_class_withevents_field_names("widget"),
            vec!["source".to_string()]
        );
    }

    #[test]
    fn project_symbol_index_records_field_array_descriptors() {
        let module = ModuleUnit {
            module_name: "Widget".to_string(),
            module_kind: ModuleKind::Class,
            attributes: Default::default(),
            source: "Option Base 1\nPrivate scores(2) As Long\nPublic grid(0 To 1, 3 To 4) As Integer\nPrivate dyn() As String\nPublic Sub Use()\nEnd Sub"
                .to_string(),
        };
        let manifest = ProjectManifest {
            project_name: "Book1".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let index = build_project_symbol_index_from_manifest(&manifest).expect("index");
        let scores = index
            .resolve_field_array_descriptor("Widget", "scores")
            .expect("scores descriptor");
        assert_eq!(scores.bounds, vec![(1, 2)]);
        assert!(!scores.dynamic);
        let grid = index
            .resolve_field_array_descriptor("widget", "GRID")
            .expect("grid descriptor");
        assert_eq!(grid.bounds, vec![(0, 1), (3, 4)]);
        assert!(!grid.dynamic);
        let dyn_array = index
            .resolve_field_array_descriptor("Widget", "dyn")
            .expect("dynamic descriptor");
        assert!(dyn_array.dynamic);
        assert!(dyn_array.bounds.is_empty());
    }

    #[test]
    fn project_symbol_index_records_procedural_module_array_fields() {
        let module = ModuleUnit {
            module_name: "MainModule".to_string(),
            module_kind: ModuleKind::Procedural,
            attributes: Default::default(),
            source: "Private cache(0 To 2) As Variant\nPublic Sub Main()\nEnd Sub".to_string(),
        };
        let manifest = ProjectManifest {
            project_name: "Book1".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let index = build_project_symbol_index_from_manifest(&manifest).expect("index");
        let cache = index
            .resolve_field_array_descriptor("MainModule", "cache")
            .expect("module field array descriptor");
        assert_eq!(cache.bounds, vec![(0, 2)]);
        assert!(!cache.dynamic);
    }

    #[test]
    fn project_symbol_index_does_not_record_local_or_udt_array_fields_as_module_fields() {
        let module = ModuleUnit {
            module_name: "MainModule".to_string(),
            module_kind: ModuleKind::Procedural,
            attributes: Default::default(),
            source: "Private module_cache(0 To 2) As Variant\nType Record\nscores(1 To 2) As Long\nEnd Type\nPublic Sub Main()\nDim local(1 To 3) As Long\nEnd Sub".to_string(),
        };
        let manifest = ProjectManifest {
            project_name: "Book1".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let index = build_project_symbol_index_from_manifest(&manifest).expect("index");
        assert!(
            index
                .resolve_field_array_descriptor("MainModule", "module_cache")
                .is_some()
        );
        assert!(
            index
                .resolve_field_array_descriptor("MainModule", "local")
                .is_none(),
            "procedure-local arrays must not be recorded as module field arrays"
        );
        assert!(
            index
                .resolve_field_array_descriptor("MainModule", "scores")
                .is_none(),
            "UDT array fields are not module field arrays"
        );
    }

    #[test]
    fn project_symbol_index_distinguishes_class_events_from_fields() {
        let module = ModuleUnit {
            module_name: "Emitter".to_string(),
            module_kind: ModuleKind::Class,
            attributes: Default::default(),
            source: "Public Event Changed(ByVal value As Long)\nPrivate value As Long".to_string(),
        };
        let manifest = ProjectManifest {
            project_name: "Book1".to_string(),
            project_kind: crate::project::ProjectKind::Source,
            modules: vec![module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };
        let index =
            build_project_symbol_index_from_manifest(&manifest).expect("index should build");
        assert_eq!(
            index
                .tables
                .resolve_event("Emitter", "Changed")
                .map(|route| route.kind),
            Some(ProjectSymbolKind::Event)
        );
        assert_eq!(
            index.resolve_class_field_names("Emitter"),
            vec!["value".to_string()]
        );
    }

    #[test]
    fn project_symbol_tables_keep_property_accessors_distinct_under_group_alias() {
        let mut symbols = SymbolModel::default();
        let get_symbol = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Procedure,
                "property_get_Value",
                provenance(),
            )
            .expect("get");
        let let_symbol = symbols
            .declare_symbol(
                symbols.global_scope(),
                SymbolNamespace::Procedure,
                "property_let_Value",
                provenance(),
            )
            .expect("let");

        let mut table = ProjectSymbolTables::default();
        table.record_class_member(
            "Widget",
            "Value",
            ProjectSymbolKind::PropertyGet,
            get_symbol,
        );
        table.record_class_member(
            "Widget",
            "Value",
            ProjectSymbolKind::PropertyLet,
            let_symbol,
        );

        assert_eq!(
            table.resolve_qualified(&QualifiedName::new(["Widget", "Value"])),
            None
        );
        assert_eq!(
            table.resolve_property_accessor("Widget", "Value", ProjectSymbolKind::PropertyGet),
            Some(ProjectSymbolRoute {
                symbol: get_symbol,
                kind: ProjectSymbolKind::PropertyGet,
            })
        );
        assert_eq!(
            table.resolve_property_accessor("Widget", "Value", ProjectSymbolKind::PropertyLet),
            Some(ProjectSymbolRoute {
                symbol: let_symbol,
                kind: ProjectSymbolKind::PropertyLet,
            })
        );
    }

    #[test]
    fn project_symbol_index_records_manifest_module_procedure_and_public_routes() {
        let main = module_unit_from_source(
            "MainModule",
            ModuleKind::Procedural,
            r#"
Public Function Add(ByVal lhs As Long, ByVal rhs As Long) As Long
    Add = lhs + rhs
End Function
"#,
        )
        .expect("module");
        let mut private_module = module_unit_from_source(
            "PrivateModule",
            ModuleKind::Procedural,
            r#"
Public Function Hidden() As Long
    Hidden = 1
End Function
"#,
        )
        .expect("private module");
        private_module.attributes.option_private_module = true;
        let manifest = ProjectManifest {
            project_name: "Book1".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![main, private_module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let index = build_project_symbol_index_from_manifest(&manifest).expect("index");
        let add = index
            .tables
            .resolve_qualified(&QualifiedName::new(["book1", "mainmodule", "add"]))
            .expect("qualified add");
        assert_eq!(add.kind, ProjectSymbolKind::Procedure);
        assert_eq!(
            index
                .tables
                .resolve_unqualified("Add")
                .map(|route| route.kind),
            Some(ProjectSymbolKind::Procedure)
        );
        assert_eq!(index.tables.resolve_unqualified("Hidden"), None);
        assert_eq!(index.module_scopes.len(), 2);
    }

    #[test]
    fn project_symbol_index_records_class_fields_and_attribute_names() {
        let mut customer = module_unit_from_source(
            "Class1",
            ModuleKind::Class,
            r#"
Public Name As String
Public Property Get DisplayName() As String
    DisplayName = Name
End Property
"#,
        )
        .expect("class module");
        customer.attributes = ModuleAttributes {
            vb_name: "Customer".to_string(),
            ..customer.attributes
        };
        let manifest = ProjectManifest {
            project_name: "Book1".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![customer],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let index = build_project_symbol_index_from_manifest(&manifest).expect("index");
        assert_eq!(
            index
                .tables
                .resolve_unqualified("Customer")
                .map(|route| route.kind),
            Some(ProjectSymbolKind::Class)
        );
        assert_eq!(
            index
                .tables
                .resolve_qualified(&QualifiedName::new(["Customer", "Name"]))
                .map(|route| route.kind),
            Some(ProjectSymbolKind::Field)
        );
        assert_eq!(
            index
                .tables
                .resolve_qualified(&QualifiedName::new(["Book1", "Customer", "DisplayName"]))
                .map(|route| route.kind),
            Some(ProjectSymbolKind::PropertyGet)
        );
    }

    #[test]
    fn project_symbol_index_falls_back_to_legacy_lines_when_parser_rejects_body() {
        let module = module_unit_from_source(
            "Module1",
            ModuleKind::Procedural,
            r#"
Public Function Add(ByVal lhs As Long, ByVal rhs As Long) As Long
    Add = UnsupportedMember(Name := lhs)
End Function

Public Property Get Value() As Long
    Value = 1
End Property
"#,
        )
        .expect("module");
        let manifest = ProjectManifest {
            project_name: "Book1".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let index = build_project_symbol_index_from_manifest(&manifest).expect("index");
        assert_eq!(
            index
                .tables
                .resolve_qualified(&QualifiedName::new(["Book1", "Module1", "Add"]))
                .map(|route| route.kind),
            Some(ProjectSymbolKind::Procedure)
        );
        assert_eq!(
            index.tables.resolve_property_accessor(
                "Module1",
                "Value",
                ProjectSymbolKind::PropertyGet
            ),
            index
                .tables
                .resolve_qualified(&QualifiedName::new(["Book1", "Module1", "Value"]))
        );
    }

    #[test]
    fn project_symbol_index_records_default_property_accessors_from_member_attributes() {
        let module = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            r#"
Public Property Get Value() As Long
    Value = 1
End Property
Public Property Let Value(ByVal nextValue As Long)
End Property
Attribute Value.VB_UserMemId = 0
"#,
        )
        .expect("module");
        let manifest = ProjectManifest {
            project_name: "Book1".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let index = build_project_symbol_index_from_manifest(&manifest).expect("index");
        assert_eq!(
            index
                .tables
                .resolve_default_member_accessor("Widget", ProjectSymbolKind::PropertyGet)
                .map(|route| route.kind),
            Some(ProjectSymbolKind::PropertyGet)
        );
        assert_eq!(
            index
                .tables
                .resolve_default_member_accessor("Widget", ProjectSymbolKind::PropertyLet)
                .map(|route| route.kind),
            Some(ProjectSymbolKind::PropertyLet)
        );
    }

    #[test]
    fn project_symbol_index_resolves_single_property_accessor_as_default_candidate() {
        let module = module_unit_from_source(
            "Widget",
            ModuleKind::Class,
            r#"
Public Property Get Value() As Long
    Value = 1
End Property
"#,
        )
        .expect("module");
        let manifest = ProjectManifest {
            project_name: "Book1".to_string(),
            project_kind: ProjectKind::Source,
            modules: vec![module],
            references: Vec::new(),
            reference_projects: Vec::new(),
            conditional_constants: BTreeMap::new(),
        };

        let index = build_project_symbol_index_from_manifest(&manifest).expect("index");
        assert_eq!(
            index
                .tables
                .resolve_default_or_single_property_accessor(
                    "Widget",
                    ProjectSymbolKind::PropertyGet
                )
                .map(|route| route.kind),
            Some(ProjectSymbolKind::PropertyGet)
        );
        assert_eq!(
            index.tables.resolve_default_or_single_property_accessor(
                "Widget",
                ProjectSymbolKind::PropertyLet
            ),
            None
        );
    }
}
