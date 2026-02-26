#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreenNode {
    pub width: usize,
    pub children: Vec<String>,
}

impl GreenNode {
    pub fn from_tokens<'a>(tokens: impl Iterator<Item = &'a str>) -> Self {
        let children: Vec<String> = tokens.map(ToOwned::to_owned).collect();
        let width = children.iter().map(|c| c.len()).sum();
        Self { width, children }
    }
}
