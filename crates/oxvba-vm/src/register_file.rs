#[derive(Debug, Default)]
pub struct RegisterFile {
    pub registers: Vec<String>,
}

impl RegisterFile {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            registers: vec![String::new(); capacity],
        }
    }
}
