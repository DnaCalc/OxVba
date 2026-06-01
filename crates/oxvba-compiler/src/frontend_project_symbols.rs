use std::collections::BTreeMap;

use crate::{
    frontend_symbols::{
        ScopeId, ScopeKind, SourceProvenance, SymbolId, SymbolModel, SymbolModelError,
        SymbolNamespace, collect_symbols_from_source_into_model, fold_identifier,
    },
    project::{ModuleKind, ModuleUnit, ProjectManifest},
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
    Field,
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
    public_symbols: BTreeMap<String, ProjectSymbolRoute>,
    module_members: BTreeMap<(String, String), ProjectSymbolRoute>,
    class_members: BTreeMap<(String, String), ProjectSymbolRoute>,
}

#[derive(Debug, Clone)]
pub struct ProjectSymbolIndex {
    pub symbols: SymbolModel,
    pub tables: ProjectSymbolTables,
    pub project_scope: ScopeId,
    pub module_scopes: BTreeMap<String, ScopeId>,
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
            .insert(fold_identifier(name), ProjectSymbolRoute { symbol, kind });
    }

    pub fn record_module_member(
        &mut self,
        module: &str,
        name: &str,
        kind: ProjectSymbolKind,
        symbol: SymbolId,
    ) {
        self.module_members.insert(
            (fold_identifier(module), fold_identifier(name)),
            ProjectSymbolRoute { symbol, kind },
        );
    }

    pub fn record_class_member(
        &mut self,
        class: &str,
        name: &str,
        kind: ProjectSymbolKind,
        symbol: SymbolId,
    ) {
        self.class_members.insert(
            (fold_identifier(class), fold_identifier(name)),
            ProjectSymbolRoute { symbol, kind },
        );
    }

    pub fn resolve_unqualified(&self, name: &str) -> Option<ProjectSymbolRoute> {
        let name = fold_identifier(name);
        self.public_symbols
            .get(&name)
            .or_else(|| self.modules.get(&name))
            .or_else(|| self.classes.get(&name))
            .copied()
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
        let owner = fold_identifier(owner);
        let member = fold_identifier(member);
        self.module_members
            .get(&(owner.clone(), member.clone()))
            .or_else(|| self.class_members.get(&(owner, member)))
            .copied()
    }
}

pub fn build_project_symbol_index_from_manifest(
    manifest: &ProjectManifest,
) -> Result<ProjectSymbolIndex, SymbolModelError> {
    let mut symbols = SymbolModel::default();
    let mut tables = ProjectSymbolTables::default();
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
            project_scope,
            &mut module_scopes,
        )?;
    }

    Ok(ProjectSymbolIndex {
        symbols,
        tables,
        project_scope,
        module_scopes,
    })
}

fn index_module(
    module: &ModuleUnit,
    symbols: &mut SymbolModel,
    tables: &mut ProjectSymbolTables,
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

    collect_symbols_from_source_into_model(symbols, &module_name, module_scope, &module.source)?;
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
                tables.record_module_member(
                    &module_name,
                    &name,
                    ProjectSymbolKind::Procedure,
                    symbol_id,
                );
                if class_symbol.is_some() {
                    tables.record_class_member(
                        &module_name,
                        &name,
                        ProjectSymbolKind::Procedure,
                        symbol_id,
                    );
                }
                if is_unqualified_public_symbol_candidate(module) {
                    tables.record_public_route(&name, symbol_id, ProjectSymbolKind::Procedure);
                }
            }
            SymbolNamespace::Local | SymbolNamespace::Member => {
                tables.record_module_member(
                    &module_name,
                    &name,
                    ProjectSymbolKind::Field,
                    symbol_id,
                );
                if class_symbol.is_some() {
                    tables.record_class_member(
                        &module_name,
                        &name,
                        ProjectSymbolKind::Field,
                        symbol_id,
                    );
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
            Some(ProjectSymbolKind::Procedure)
        );
    }
}
