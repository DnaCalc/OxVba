use crossbeam_channel::Sender;
use oxvba_host::{DirectHostBreakpointId, DirectHostStackFrameId, DirectHostWatchId};

use crate::{
    com_apartment::DebugWorkerApartmentReport,
    errors::DebugError,
    views::{
        DebugBreakpointView, DebugFrameView, DebugPauseView, DebugRunResultView, DebugValueView,
        DebugWatchView,
    },
};

pub type CommandReply<T> = Sender<Result<T, DebugError>>;

/// Commands marshalled from `DebugSessionHandle` to the worker-owned core.
#[derive(Debug)]
pub enum DebugCommand {
    Start(CommandReply<DebugRunResultView>),
    StepInto(CommandReply<DebugRunResultView>),
    StepOver(CommandReply<DebugRunResultView>),
    StepOut(CommandReply<DebugRunResultView>),
    Continue(CommandReply<DebugRunResultView>),
    SetSourceBreakpoint {
        module: String,
        file_line: u32,
        enabled: bool,
        reply: CommandReply<DebugBreakpointView>,
    },
    SetBreakpointEnabled {
        id: DirectHostBreakpointId,
        enabled: bool,
        reply: CommandReply<DebugBreakpointView>,
    },
    ClearSourceBreakpoint {
        id: DirectHostBreakpointId,
        reply: CommandReply<()>,
    },
    Breakpoints(CommandReply<Vec<DebugBreakpointView>>),
    AddWatch {
        expression: String,
        reply: CommandReply<DebugWatchView>,
    },
    UpdateWatch {
        id: DirectHostWatchId,
        expression: String,
        reply: CommandReply<DebugWatchView>,
    },
    RemoveWatch {
        id: DirectHostWatchId,
        reply: CommandReply<()>,
    },
    EvaluateWatches(CommandReply<Vec<DebugWatchView>>),
    CurrentPause(CommandReply<Option<DebugPauseView>>),
    StackFrames(CommandReply<Vec<DebugFrameView>>),
    FrameLocals {
        frame: DirectHostStackFrameId,
        reply: CommandReply<Vec<DebugValueView>>,
    },
    Evaluate {
        frame: Option<DirectHostStackFrameId>,
        expression: String,
        reply: CommandReply<DebugValueView>,
    },
    ReportWorkerApartment(CommandReply<DebugWorkerApartmentReport>),
    #[doc(hidden)]
    PanicWorker,
    Shutdown(CommandReply<()>),
}
