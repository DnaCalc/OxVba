#![cfg(feature = "proptest")]

#[path = "support_handle/mod.rs"]
mod support_handle;

use std::{sync::mpsc, time::Duration};

use oxvba_debug::{DebugError, DebugRunResultView};
use oxvba_host::DirectHostBreakpointId;
use proptest::prelude::*;

#[derive(Debug, Clone, Copy)]
enum RandomCommand {
    Start,
    StepInto,
    StepOver,
    StepOut,
    Continue,
    SetBreakpoint,
    DisableFirstBreakpoint,
    ClearFirstBreakpoint,
    Breakpoints,
    AddWatchY,
    EvaluateWatches,
    CurrentPause,
    StackFrames,
    EvaluateY,
}

fn command_strategy() -> impl Strategy<Value = RandomCommand> {
    prop_oneof![
        Just(RandomCommand::Start),
        Just(RandomCommand::StepInto),
        Just(RandomCommand::StepOver),
        Just(RandomCommand::StepOut),
        Just(RandomCommand::Continue),
        Just(RandomCommand::SetBreakpoint),
        Just(RandomCommand::DisableFirstBreakpoint),
        Just(RandomCommand::ClearFirstBreakpoint),
        Just(RandomCommand::Breakpoints),
        Just(RandomCommand::AddWatchY),
        Just(RandomCommand::EvaluateWatches),
        Just(RandomCommand::CurrentPause),
        Just(RandomCommand::StackFrames),
        Just(RandomCommand::EvaluateY),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, max_shrink_iters: 64, .. ProptestConfig::default() })]

    #[test]
    fn random_handle_command_sequences_do_not_panic_deadlock_or_return_untyped_errors(
        commands in prop::collection::vec(command_strategy(), 1..40)
    ) {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| run_sequence(&commands));
            let _ = tx.send(result);
        });
        let result = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("random debug command sequence should not deadlock");
        prop_assert!(result.is_ok(), "random sequence panicked: {result:?}");
    }
}

fn run_sequence(commands: &[RandomCommand]) {
    let handle = support_handle::attach(support_handle::multi_module_manifest()).handle;
    let mut first_breakpoint: Option<DirectHostBreakpointId> = None;
    for command in commands {
        match command {
            RandomCommand::Start => assert_typed_run(handle.start()),
            RandomCommand::StepInto => assert_typed_run(handle.step_into()),
            RandomCommand::StepOver => assert_typed_run(handle.step_over()),
            RandomCommand::StepOut => assert_typed_run(handle.step_out()),
            RandomCommand::Continue => assert_typed_run(handle.continue_execution()),
            RandomCommand::SetBreakpoint => {
                match handle.set_source_breakpoint("Module1", 2, true) {
                    Ok(bp) => first_breakpoint = Some(DirectHostBreakpointId::new(bp.id)),
                    Err(err) => assert_typed_error(err),
                }
            }
            RandomCommand::DisableFirstBreakpoint => {
                if let Some(id) = first_breakpoint.as_ref() {
                    if let Err(err) = handle.set_breakpoint_enabled(id, false) {
                        assert_typed_error(err);
                    }
                }
            }
            RandomCommand::ClearFirstBreakpoint => {
                if let Some(id) = first_breakpoint.take() {
                    if let Err(err) = handle.clear_source_breakpoint(&id) {
                        assert_typed_error(err);
                    }
                }
            }
            RandomCommand::Breakpoints => {
                if let Err(err) = handle.breakpoints() {
                    assert_typed_error(err);
                }
            }
            RandomCommand::AddWatchY => {
                if let Err(err) = handle.add_watch("y") {
                    assert_typed_error(err);
                }
            }
            RandomCommand::EvaluateWatches => {
                if let Err(err) = handle.evaluate_watches() {
                    assert_typed_error(err);
                }
            }
            RandomCommand::CurrentPause => {
                if let Err(err) = handle.current_pause() {
                    assert_typed_error(err);
                }
            }
            RandomCommand::StackFrames => {
                if let Err(err) = handle.stack_frames() {
                    assert_typed_error(err);
                }
            }
            RandomCommand::EvaluateY => {
                if let Err(err) = handle.evaluate(None, "y") {
                    assert_typed_error(err);
                }
            }
        }
    }
    let _ = handle.detach();
}

fn assert_typed_run(result: Result<DebugRunResultView, DebugError>) {
    if let Err(err) = result {
        assert_typed_error(err);
    }
}

fn assert_typed_error(err: DebugError) {
    match err {
        DebugError::NotPaused
        | DebugError::UnknownBreakpoint(_)
        | DebugError::UnknownWatch(_)
        | DebugError::UnknownFrame(_)
        | DebugError::Evaluation { .. }
        | DebugError::Completed
        | DebugError::UnsupportedCommand(_)
        | DebugError::OutstandingHandles { .. }
        | DebugError::SessionAlreadyDetached
        | DebugError::WorkerFailed { .. }
        | DebugError::Internal(_) => {}
    }
}
