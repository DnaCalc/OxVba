use std::{sync::Arc, thread};

use crossbeam_channel::{Receiver, Sender, bounded};
use oxvba_compiler::ProjectManifest;
use oxvba_host::{DirectHostDebugSessionId, Engine};

use crate::{
    command::{CommandReply, DebugCommand},
    config::DebugAttachConfig,
    core::{DebugEvaluationRequest, DebugSessionCore, DebugSessionError},
    errors::{DebugAttachError, DebugError},
    views::{
        DebugRunResultView, breakpoint_view_from_core, frame_view_from_core, pause_view_from_core,
        run_result_view_from_core, value_view_from_core, watch_view_from_core,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugWorkerState {
    NotStarted,
    Running,
    Stopped,
}

#[derive(Debug)]
pub struct DebugWorkerAttach {
    pub session_id: DirectHostDebugSessionId,
    pub commands: Sender<DebugCommand>,
    pub join: thread::JoinHandle<()>,
}

pub fn spawn_debug_worker(
    engine: Arc<Engine>,
    manifest: ProjectManifest,
    _config: DebugAttachConfig,
) -> Result<DebugWorkerAttach, DebugAttachError> {
    let (commands_tx, commands_rx) = crossbeam_channel::unbounded();
    let (ready_tx, ready_rx) = bounded(1);
    let join = thread::Builder::new()
        .name(format!("oxvba-debug:{}", manifest.project_name))
        .spawn(move || {
            let runtime = match engine.compile_and_prepare_session(&manifest) {
                Ok(runtime) => runtime,
                Err(diagnostic) => {
                    let _ = ready_tx.send(Err(DebugAttachError::Prepare {
                        message: diagnostic.to_string(),
                    }));
                    return;
                }
            };
            let mut core = DebugSessionCore::new(engine, manifest, runtime);
            let session_id = core.debug_session_id().clone();
            if ready_tx.send(Ok(session_id)).is_err() {
                return;
            }
            run_command_loop(&mut core, commands_rx);
        })
        .map_err(|err| DebugAttachError::WorkerFailed {
            stage: "spawn",
            message: err.to_string(),
        })?;

    match ready_rx.recv() {
        Ok(Ok(session_id)) => Ok(DebugWorkerAttach {
            session_id,
            commands: commands_tx,
            join,
        }),
        Ok(Err(err)) => {
            let _ = join.join();
            Err(err)
        }
        Err(err) => {
            let _ = join.join();
            Err(DebugAttachError::WorkerFailed {
                stage: "ready",
                message: err.to_string(),
            })
        }
    }
}

fn run_command_loop(core: &mut DebugSessionCore, commands: Receiver<DebugCommand>) {
    while let Ok(command) = commands.recv() {
        if handle_command(core, command) {
            break;
        }
    }
}

fn handle_command(core: &mut DebugSessionCore, command: DebugCommand) -> bool {
    match command {
        DebugCommand::Start(reply) => reply_run(reply, core.start_variants()),
        DebugCommand::StepInto(reply) => reply_run(reply, core.step_into_variants()),
        DebugCommand::StepOver(reply) => reply_run(reply, core.step_over_variants()),
        DebugCommand::StepOut(reply) => reply_run(reply, core.step_out_variants()),
        DebugCommand::Continue(reply) => reply_run(reply, core.continue_execution_variants()),
        DebugCommand::SetSourceBreakpoint {
            module,
            file_line,
            enabled,
            reply,
        } => {
            let record = core.set_source_breakpoint(module, file_line as usize);
            let record = if record.enabled == enabled {
                record
            } else {
                core.set_breakpoint_enabled(&record.breakpoint_id, enabled)
                    .unwrap_or(record)
            };
            let _ = reply.send(Ok(breakpoint_view_from_core(&record)));
            false
        }
        DebugCommand::SetBreakpointEnabled { id, enabled, reply } => {
            let result = core
                .set_breakpoint_enabled(&id, enabled)
                .map(|record| breakpoint_view_from_core(&record))
                .ok_or(DebugError::UnknownBreakpoint(id));
            let _ = reply.send(result);
            false
        }
        DebugCommand::ClearSourceBreakpoint { id, reply } => {
            let result = core
                .clear_source_breakpoint(&id)
                .map(|_| ())
                .ok_or(DebugError::UnknownBreakpoint(id));
            let _ = reply.send(result);
            false
        }
        DebugCommand::Breakpoints(reply) => {
            let _ = reply.send(Ok(core
                .source_breakpoints()
                .iter()
                .map(breakpoint_view_from_core)
                .collect()));
            false
        }
        DebugCommand::AddWatch { expression, reply } => {
            let record = core.add_watch(expression);
            let evaluation = core
                .evaluate_watches()
                .into_iter()
                .find(|evaluation| evaluation.watch_id == record.watch_id);
            let result = evaluation
                .map(|evaluation| watch_view_from_core(&evaluation))
                .ok_or_else(|| DebugError::Internal("new watch missing after insert".to_string()));
            let _ = reply.send(result);
            false
        }
        DebugCommand::UpdateWatch {
            id,
            expression,
            reply,
        } => {
            let result = core
                .update_watch(&id, expression)
                .ok_or_else(|| DebugError::UnknownWatch(id.clone()))
                .and_then(|record| {
                    core.evaluate_watches()
                        .into_iter()
                        .find(|evaluation| evaluation.watch_id == record.watch_id)
                        .map(|evaluation| watch_view_from_core(&evaluation))
                        .ok_or_else(|| DebugError::Internal("updated watch missing".to_string()))
                });
            let _ = reply.send(result);
            false
        }
        DebugCommand::RemoveWatch { id, reply } => {
            let result = core
                .remove_watch(&id)
                .map(|_| ())
                .ok_or(DebugError::UnknownWatch(id));
            let _ = reply.send(result);
            false
        }
        DebugCommand::EvaluateWatches(reply) => {
            let _ = reply.send(Ok(core
                .evaluate_watches()
                .iter()
                .map(watch_view_from_core)
                .collect()));
            false
        }
        DebugCommand::CurrentPause(reply) => {
            let result = core
                .current_variant_pause_state()
                .map(|pause| pause.as_ref().map(pause_view_from_core))
                .map_err(debug_error_from_core);
            let _ = reply.send(result);
            false
        }
        DebugCommand::StackFrames(reply) => {
            let result = core
                .current_variant_pause_state()
                .map(|pause| {
                    pause
                        .map(|pause| pause.frames.iter().map(frame_view_from_core).collect())
                        .unwrap_or_default()
                })
                .map_err(debug_error_from_core);
            let _ = reply.send(result);
            false
        }
        DebugCommand::FrameLocals { frame, reply } => {
            let result = core
                .current_variant_pause_state()
                .map_err(debug_error_from_core)
                .and_then(|pause| {
                    pause.ok_or(DebugError::NotPaused).and_then(|pause| {
                        pause
                            .frames
                            .iter()
                            .find(|candidate| candidate.frame_id == frame)
                            .map(|frame| frame.values.iter().map(value_view_from_core).collect())
                            .ok_or(DebugError::UnknownFrame(frame))
                    })
                });
            let _ = reply.send(result);
            false
        }
        DebugCommand::Evaluate {
            frame,
            expression,
            reply,
        } => {
            let result = evaluate_on_worker(core, frame, expression);
            let _ = reply.send(result);
            false
        }
        DebugCommand::Shutdown(reply) => {
            let _ = reply.send(Ok(()));
            true
        }
    }
}

fn reply_run(
    reply: CommandReply<DebugRunResultView>,
    result: Result<crate::core::DebugCoreRunResult, DebugSessionError>,
) -> bool {
    let _ = reply.send(
        result
            .map(|result| run_result_view_from_core(&result))
            .map_err(debug_error_from_core),
    );
    false
}

fn evaluate_on_worker(
    core: &DebugSessionCore,
    frame: Option<oxvba_host::DirectHostStackFrameId>,
    expression: String,
) -> Result<crate::views::DebugValueView, DebugError> {
    if let Some(frame_id) = frame {
        let pause = core
            .current_variant_pause_state()
            .map_err(debug_error_from_core)?
            .ok_or(DebugError::NotPaused)?;
        if !pause.frames.iter().any(|frame| frame.frame_id == frame_id) {
            return Err(DebugError::UnknownFrame(frame_id));
        }
    }
    core.evaluate_variant(&DebugEvaluationRequest::new(expression.clone()))
        .map(|result| value_view_from_core(&result.value))
        .map_err(|err| match err {
            DebugSessionError::NotPaused => DebugError::NotPaused,
            DebugSessionError::UnsupportedEvaluation { .. }
            | DebugSessionError::UnknownVisibleName { .. } => DebugError::Evaluation {
                expression,
                message: err.to_string(),
            },
            other => debug_error_from_core(other),
        })
}

fn debug_error_from_core(err: DebugSessionError) -> DebugError {
    match err {
        DebugSessionError::NotPaused => DebugError::NotPaused,
        DebugSessionError::UnsupportedEvaluation { expression } => DebugError::Evaluation {
            expression: expression.clone(),
            message: DebugSessionError::UnsupportedEvaluation { expression }.to_string(),
        },
        DebugSessionError::UnknownVisibleName { name } => DebugError::Evaluation {
            expression: name.clone(),
            message: DebugSessionError::UnknownVisibleName { name }.to_string(),
        },
        other => DebugError::WorkerFailed {
            stage: "command",
            message: other.to_string(),
        },
    }
}
