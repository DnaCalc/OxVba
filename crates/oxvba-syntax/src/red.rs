use crate::green::GreenNode;

#[derive(Debug, Clone)]
pub struct RedNode<'a> {
    pub green: &'a GreenNode,
    pub absolute_offset: usize,
}

impl<'a> RedNode<'a> {
    pub fn new(green: &'a GreenNode) -> Self {
        Self {
            green,
            absolute_offset: 0,
        }
    }
}
