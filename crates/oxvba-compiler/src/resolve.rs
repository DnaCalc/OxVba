#[derive(Debug, Clone)]
pub struct BoundModule {
    pub source: String,
}

pub fn resolve_symbols(source: &str) -> BoundModule {
    BoundModule {
        source: source.to_string(),
    }
}
