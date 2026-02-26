#[derive(Debug, Default)]
pub struct RegisterFile {
    pub registers: Vec<i32>,
}

impl RegisterFile {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            registers: vec![0; capacity],
        }
    }
}
