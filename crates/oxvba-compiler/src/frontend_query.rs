use std::collections::BTreeMap;

use crate::frontend_assignment_semantics::collect_assignment_semantics_from_typed_hir;
use crate::frontend_diagnostics::{
    FrontendDiagnostic, FrontendDiagnosticFamily, FrontendDiagnosticMapper,
    FrontendDiagnosticSeverity,
};
use crate::frontend_semantic_model::SemanticModel;
use crate::frontend_symbols::FrontendSourceSpan;
use crate::frontend_type_hooks::{TypedHirModule, collect_type_hooks_from_source};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FrontendQueryLayer {
    Parse,
    Bind,
    Typecheck,
    Diagnostics,
    SemanticModel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendQueryState {
    revisions: BTreeMap<FrontendQueryLayer, u64>,
}

impl Default for FrontendQueryState {
    fn default() -> Self {
        let mut revisions = BTreeMap::new();
        for layer in [
            FrontendQueryLayer::Parse,
            FrontendQueryLayer::Bind,
            FrontendQueryLayer::Typecheck,
            FrontendQueryLayer::Diagnostics,
            FrontendQueryLayer::SemanticModel,
        ] {
            revisions.insert(layer, 0);
        }
        Self { revisions }
    }
}

impl FrontendQueryState {
    pub fn revision(&self, layer: FrontendQueryLayer) -> u64 {
        self.revisions.get(&layer).copied().unwrap_or(0)
    }

    pub fn invalidate_from(&mut self, layer: FrontendQueryLayer) {
        for affected in affected_layers(layer) {
            *self.revisions.entry(affected).or_default() += 1;
        }
    }
}

pub fn affected_layers(layer: FrontendQueryLayer) -> Vec<FrontendQueryLayer> {
    match layer {
        FrontendQueryLayer::Parse => vec![
            FrontendQueryLayer::Parse,
            FrontendQueryLayer::Bind,
            FrontendQueryLayer::Typecheck,
            FrontendQueryLayer::Diagnostics,
            FrontendQueryLayer::SemanticModel,
        ],
        FrontendQueryLayer::Bind => vec![
            FrontendQueryLayer::Bind,
            FrontendQueryLayer::Typecheck,
            FrontendQueryLayer::Diagnostics,
            FrontendQueryLayer::SemanticModel,
        ],
        FrontendQueryLayer::Typecheck => vec![
            FrontendQueryLayer::Typecheck,
            FrontendQueryLayer::Diagnostics,
            FrontendQueryLayer::SemanticModel,
        ],
        FrontendQueryLayer::Diagnostics => vec![FrontendQueryLayer::Diagnostics],
        FrontendQueryLayer::SemanticModel => vec![FrontendQueryLayer::SemanticModel],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendParseQueryResult {
    pub root_kind: String,
    pub text_len: usize,
    pub errors: Vec<oxvba_syntax::ParseError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendTypecheckQueryResult {
    pub assignment_count: usize,
    pub coercion_count: usize,
    pub diagnostics: Vec<FrontendDiagnostic>,
}

#[derive(Debug, Clone)]
struct QueryCache<T> {
    revision: u64,
    value: T,
}

#[derive(Debug, Clone)]
pub struct FrontendQueryDatabase {
    module_name: String,
    source: String,
    state: FrontendQueryState,
    recomputes: BTreeMap<FrontendQueryLayer, u64>,
    parse_cache: Option<QueryCache<FrontendParseQueryResult>>,
    bind_cache: Option<QueryCache<Result<TypedHirModule, String>>>,
    typecheck_cache: Option<QueryCache<Result<FrontendTypecheckQueryResult, String>>>,
    diagnostics_cache: Option<QueryCache<Vec<FrontendDiagnostic>>>,
    semantic_model_cache: Option<QueryCache<Result<SemanticModel, String>>>,
}

impl FrontendQueryDatabase {
    pub fn new(module_name: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            module_name: module_name.into(),
            source: source.into(),
            state: FrontendQueryState::default(),
            recomputes: BTreeMap::new(),
            parse_cache: None,
            bind_cache: None,
            typecheck_cache: None,
            diagnostics_cache: None,
            semantic_model_cache: None,
        }
    }

    pub fn set_source(&mut self, source: impl Into<String>) {
        let source = source.into();
        if self.source != source {
            self.source = source;
            self.invalidate_from(FrontendQueryLayer::Parse);
        }
    }

    pub fn invalidate_from(&mut self, layer: FrontendQueryLayer) {
        self.state.invalidate_from(layer);
    }

    pub fn revision(&self, layer: FrontendQueryLayer) -> u64 {
        self.state.revision(layer)
    }

    pub fn recompute_count(&self, layer: FrontendQueryLayer) -> u64 {
        self.recomputes.get(&layer).copied().unwrap_or(0)
    }

    pub fn parse(&mut self) -> FrontendParseQueryResult {
        let revision = self.revision(FrontendQueryLayer::Parse);
        if self
            .parse_cache
            .as_ref()
            .is_none_or(|cache| cache.revision != revision)
        {
            self.bump_recompute(FrontendQueryLayer::Parse);
            let parsed = oxvba_syntax::parse(&self.source);
            self.parse_cache = Some(QueryCache {
                revision,
                value: FrontendParseQueryResult {
                    root_kind: format!("{:?}", parsed.syntax().kind()),
                    text_len: parsed.syntax().text().len(),
                    errors: parsed.errors().to_vec(),
                },
            });
        }
        self.parse_cache
            .as_ref()
            .expect("parse cache")
            .value
            .clone()
    }

    pub fn bind(&mut self) -> Result<TypedHirModule, String> {
        let revision = self.revision(FrontendQueryLayer::Bind);
        if self
            .bind_cache
            .as_ref()
            .is_none_or(|cache| cache.revision != revision)
        {
            self.bump_recompute(FrontendQueryLayer::Bind);
            let parse = self.parse();
            let value = if parse.errors.is_empty() {
                collect_type_hooks_from_source(&self.module_name, &self.source)
                    .map_err(|err| err.to_string())
            } else {
                Err("parse errors prevented binding".to_string())
            };
            self.bind_cache = Some(QueryCache { revision, value });
        }
        self.bind_cache.as_ref().expect("bind cache").value.clone()
    }

    pub fn typecheck(&mut self) -> Result<FrontendTypecheckQueryResult, String> {
        let revision = self.revision(FrontendQueryLayer::Typecheck);
        if self
            .typecheck_cache
            .as_ref()
            .is_none_or(|cache| cache.revision != revision)
        {
            self.bump_recompute(FrontendQueryLayer::Typecheck);
            let value = self.bind().map(|typed| {
                let assignments = collect_assignment_semantics_from_typed_hir(&typed);
                let diagnostics = assignments
                    .iter()
                    .filter_map(|assignment| {
                        let diagnostic = assignment.diagnostic.as_ref()?;
                        Some(FrontendDiagnostic {
                            family: FrontendDiagnosticFamily::Type,
                            severity: FrontendDiagnosticSeverity::Error,
                            code: diagnostic.code.clone(),
                            legacy_code: None,
                            span: typed
                                .module
                                .arenas
                                .stmt(assignment.stmt)
                                .map(|stmt| stmt.cst.span)
                                .unwrap_or(FrontendSourceSpan { start: 0, end: 0 }),
                            message: diagnostic.message.clone(),
                        })
                    })
                    .collect::<Vec<_>>();
                FrontendTypecheckQueryResult {
                    assignment_count: assignments.len(),
                    coercion_count: typed.hooks.coercions().count(),
                    diagnostics,
                }
            });
            self.typecheck_cache = Some(QueryCache { revision, value });
        }
        self.typecheck_cache
            .as_ref()
            .expect("typecheck cache")
            .value
            .clone()
    }

    pub fn diagnostics(&mut self) -> Vec<FrontendDiagnostic> {
        let revision = self.revision(FrontendQueryLayer::Diagnostics);
        if self
            .diagnostics_cache
            .as_ref()
            .is_none_or(|cache| cache.revision != revision)
        {
            self.bump_recompute(FrontendQueryLayer::Diagnostics);
            let parse = self.parse();
            let mut diagnostics = FrontendDiagnosticMapper::from_parse_errors(&parse.errors)
                .diagnostics()
                .to_vec();
            diagnostics.extend(FrontendDiagnosticMapper::declare_ptrsafe_diagnostics(
                &self.source,
            ));
            match self.typecheck() {
                Ok(typecheck) => diagnostics.extend(typecheck.diagnostics),
                Err(err) if parse.errors.is_empty() => diagnostics.push(FrontendDiagnostic {
                    family: FrontendDiagnosticFamily::Binder,
                    severity: FrontendDiagnosticSeverity::Error,
                    code: "BIND-E-HIR".to_string(),
                    legacy_code: None,
                    span: FrontendSourceSpan { start: 0, end: 0 },
                    message: err,
                }),
                Err(_) => {}
            }
            self.diagnostics_cache = Some(QueryCache {
                revision,
                value: diagnostics,
            });
        }
        self.diagnostics_cache
            .as_ref()
            .expect("diagnostics cache")
            .value
            .clone()
    }

    pub fn semantic_model(&mut self) -> Result<SemanticModel, String> {
        let revision = self.revision(FrontendQueryLayer::SemanticModel);
        if self
            .semantic_model_cache
            .as_ref()
            .is_none_or(|cache| cache.revision != revision)
        {
            self.bump_recompute(FrontendQueryLayer::SemanticModel);
            let value = self.bind().map(|typed| {
                let mut model = SemanticModel::from_bound_hir_module(typed.module);
                for diagnostic in self.diagnostics() {
                    model.push_diagnostic(diagnostic.to_semantic_diagnostic());
                }
                model
            });
            self.semantic_model_cache = Some(QueryCache { revision, value });
        }
        self.semantic_model_cache
            .as_ref()
            .expect("semantic model cache")
            .value
            .clone()
    }

    fn bump_recompute(&mut self, layer: FrontendQueryLayer) {
        *self.recomputes.entry(layer).or_default() += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend_symbols::SymbolNamespace;

    #[test]
    fn query_state_parse_edit_invalidates_all_layers() {
        let mut state = FrontendQueryState::default();
        state.invalidate_from(FrontendQueryLayer::Parse);
        assert_eq!(state.revision(FrontendQueryLayer::Parse), 1);
        assert_eq!(state.revision(FrontendQueryLayer::SemanticModel), 1);
    }

    #[test]
    fn query_state_typecheck_edit_does_not_reparse_or_rebind() {
        let mut state = FrontendQueryState::default();
        state.invalidate_from(FrontendQueryLayer::Typecheck);
        assert_eq!(state.revision(FrontendQueryLayer::Parse), 0);
        assert_eq!(state.revision(FrontendQueryLayer::Bind), 0);
        assert_eq!(state.revision(FrontendQueryLayer::Diagnostics), 1);
    }

    #[test]
    fn query_database_caches_parse_bind_typecheck_diagnostics_and_semantic_model() {
        let source = "Sub Main()\nDim x As Long\nx = 1\nEnd Sub\n";
        let mut db = FrontendQueryDatabase::new("Module1", source);

        let model = db.semantic_model().expect("semantic model");
        assert_eq!(db.recompute_count(FrontendQueryLayer::Parse), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Bind), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Typecheck), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Diagnostics), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::SemanticModel), 1);

        let symbol = model
            .symbols()
            .symbols()
            .iter()
            .find(|symbol| {
                symbol.namespace == SymbolNamespace::Local
                    && model
                        .symbols()
                        .name(symbol.name)
                        .is_some_and(|name| name.folded == "x")
            })
            .expect("local symbol");
        assert_eq!(symbol.namespace, SymbolNamespace::Local);

        db.semantic_model().expect("cached semantic model");
        assert_eq!(db.recompute_count(FrontendQueryLayer::Parse), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Bind), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Typecheck), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Diagnostics), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::SemanticModel), 1);
    }

    #[test]
    fn query_database_source_edit_recomputes_all_layers_lazily() {
        let mut db =
            FrontendQueryDatabase::new("Module1", "Sub Main()\nDim x As Long\nx = 1\nEnd Sub\n");
        db.semantic_model().expect("initial model");

        db.set_source("Sub Main()\nDim y As Long\ny = 2\nEnd Sub\n");
        assert_eq!(db.revision(FrontendQueryLayer::Parse), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Parse), 1);

        let model = db.semantic_model().expect("edited model");
        assert_eq!(db.recompute_count(FrontendQueryLayer::Parse), 2);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Bind), 2);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Typecheck), 2);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Diagnostics), 2);
        assert_eq!(db.recompute_count(FrontendQueryLayer::SemanticModel), 2);
        assert!(model.symbols().symbols().iter().any(|symbol| {
            symbol.namespace == SymbolNamespace::Local
                && model
                    .symbols()
                    .name(symbol.name)
                    .is_some_and(|name| name.folded == "y")
        }));
    }

    #[test]
    fn query_database_typecheck_invalidation_reuses_parse_and_bind() {
        let mut db = FrontendQueryDatabase::new(
            "Module1",
            "Sub Main()\nDim obj As Object\nLet obj = Nothing\nEnd Sub\n",
        );
        let diagnostics = db.diagnostics();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "BIND-E-LET-OBJECT-TARGET"),
            "expected object Let diagnostic: {diagnostics:#?}"
        );
        assert_eq!(db.recompute_count(FrontendQueryLayer::Parse), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Bind), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Typecheck), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Diagnostics), 1);

        db.invalidate_from(FrontendQueryLayer::Typecheck);
        let diagnostics = db.diagnostics();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "BIND-E-LET-OBJECT-TARGET"),
            "expected object Let diagnostic after invalidation: {diagnostics:#?}"
        );
        assert_eq!(db.recompute_count(FrontendQueryLayer::Parse), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Bind), 1);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Typecheck), 2);
        assert_eq!(db.recompute_count(FrontendQueryLayer::Diagnostics), 2);
    }

    #[test]
    fn query_database_parse_errors_stop_bind_but_still_feed_diagnostics() {
        let mut db = FrontendQueryDatabase::new("Module1", "Sub Main()\nx = \nEnd Sub\n");
        let diagnostics = db.diagnostics();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "PARSE-E-SYNTAX"),
            "expected parse diagnostic: {diagnostics:#?}"
        );
        assert!(db.semantic_model().is_err());
    }
}
