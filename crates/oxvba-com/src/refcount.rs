#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefCount(pub u32);

impl RefCount {
    pub fn add_ref(&mut self) {
        self.0 = self.0.saturating_add(1);
    }

    pub fn release(&mut self) {
        self.0 = self.0.saturating_sub(1);
    }
}
