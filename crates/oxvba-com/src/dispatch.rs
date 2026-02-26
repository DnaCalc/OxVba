pub type DispatchResult = Result<(), String>;

pub trait ComDispatch {
    fn invoke(&self, name: &str) -> DispatchResult;
}
