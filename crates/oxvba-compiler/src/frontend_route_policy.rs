#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FrontendConstruct {
    LexerTrivia,
    Literals,
    Identifiers,
    Expressions,
    Postfix,
    Statements,
    Diagnostics,
    BinderHir,
    ProjectSemantics,
    Lowering,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontendRoute {
    V2Default,
    LegacyResidual { reason: String },
}

#[derive(Debug, Clone, Default)]
pub struct FrontendRoutePolicy {
    routes: std::collections::BTreeMap<FrontendConstruct, FrontendRoute>,
}

impl FrontendRoutePolicy {
    pub fn new_completed_construct_defaults() -> Self {
        let mut policy = Self::default();
        for construct in [
            FrontendConstruct::LexerTrivia,
            FrontendConstruct::Literals,
            FrontendConstruct::Identifiers,
            FrontendConstruct::Expressions,
            FrontendConstruct::Postfix,
            FrontendConstruct::Statements,
            FrontendConstruct::Diagnostics,
            FrontendConstruct::BinderHir,
        ] {
            policy.routes.insert(construct, FrontendRoute::V2Default);
        }
        policy.routes.insert(
            FrontendConstruct::ProjectSemantics,
            FrontendRoute::LegacyResidual {
                reason: "FE-7 project semantics has binder-owned symbol tables and a module-aware production lowering route, but project.rs line lowering/rewrite glue remains load-bearing"
                    .to_string(),
            },
        );
        policy.routes.insert(
            FrontendConstruct::Lowering,
            FrontendRoute::LegacyResidual {
                reason: "production bytecode lowering still migrates through legacy bridge"
                    .to_string(),
            },
        );
        policy
    }

    pub fn route(&self, construct: FrontendConstruct) -> Option<&FrontendRoute> {
        self.routes.get(&construct)
    }

    pub fn residuals(&self) -> Vec<(FrontendConstruct, String)> {
        self.routes
            .iter()
            .filter_map(|(construct, route)| match route {
                FrontendRoute::LegacyResidual { reason } => Some((*construct, reason.clone())),
                FrontendRoute::V2Default => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_policy_defaults_completed_constructs_to_v2() {
        let policy = FrontendRoutePolicy::new_completed_construct_defaults();
        assert_eq!(
            policy.route(FrontendConstruct::Expressions),
            Some(&FrontendRoute::V2Default)
        );
        assert!(matches!(
            policy.route(FrontendConstruct::ProjectSemantics),
            Some(FrontendRoute::LegacyResidual { reason })
                if reason.contains("project.rs line lowering")
        ));
    }

    #[test]
    fn route_policy_keeps_lowering_as_tracked_residual() {
        let policy = FrontendRoutePolicy::new_completed_construct_defaults();
        assert!(matches!(
            policy.route(FrontendConstruct::Lowering),
            Some(FrontendRoute::LegacyResidual { reason }) if reason.contains("legacy bridge")
        ));
        assert_eq!(policy.residuals().len(), 2);
    }
}
