#[derive(Debug, Default)]
pub struct CycleCollector {
    pub enabled: bool,
}

impl CycleCollector {
    pub fn run_epoch(&self) {}
}
