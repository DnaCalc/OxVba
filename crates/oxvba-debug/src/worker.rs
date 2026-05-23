/// Worker module placeholder. B05 introduces the command loop that owns `DebugSessionCore`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugWorkerState {
    NotStarted,
    Running,
    Stopped,
}
