use oxvba_runtime::RuntimeValue;

#[derive(Debug, Default)]
pub struct RegisterFile {
    pub registers: Vec<RuntimeValue>,
}

impl RegisterFile {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            registers: vec![RuntimeValue::default(); capacity],
        }
    }
}
