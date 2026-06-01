use std::collections::BTreeMap;

use crate::frontend_symbols::{SymbolId, SymbolModel, SymbolNamespace, fold_identifier};

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
        self.public_symbols.insert(
            fold_identifier(name),
            ProjectSymbolRoute {
                symbol,
                kind: ProjectSymbolKind::Public,
            },
        );
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
}
