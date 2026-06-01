use std::collections::BTreeMap;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
