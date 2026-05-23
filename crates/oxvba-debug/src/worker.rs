use std::{
    sync::{Arc, Mutex},
    thread,
};

use crossbeam_channel::{Receiver, Sender, bounded};
use oxvba_compiler::ProjectManifest;
use oxvba_hal::{HostOutputChannel, install_thread_output_tap};
use oxvba_host::{DirectHostDebugSessionId, Engine};

use crate::{
    command::{CommandReply, DebugCommand},
    config::DebugAttachConfig,
    core::{DebugCoreRunResult, DebugEvaluationRequest, DebugSessionCore, DebugSessionError},
    errors::{DebugAttachError, DebugError},
    events::{DebugBreakpointChangeKind, DebugEvent, DebugEventHub, DebugOutputChannel},
    views::{
        DebugExitView, DebugModuleView, DebugRunResultView, breakpoint_view_from_core,
        frame_view_from_core, pause_view_from_core, run_result_view_from_core,
        value_view_from_core, watch_view_from_core,
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

pub(crate) fn spawn_debug_worker(
    engine: Arc<Engine>,
    manifest: ProjectManifest,
    _config: DebugAttachConfig,
    events: DebugEventHub,
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
            let output_tap = Arc::new(Mutex::new(Vec::new()));
            let tap_buffer = output_tap.clone();
            let _output_tap_guard = install_thread_output_tap(Arc::new(
                move |channel: HostOutputChannel, text: &str| {
                    tap_buffer
                        .lock()
                        .expect("debug output tap buffer poisoned")
                        .push((channel, text.to_string()));
                },
            ));
            let mut publisher =
                DebugEventPublisher::new(events, session_id.as_str().to_string(), output_tap);
            for module in core.manifest().modules.iter() {
                publisher.publish(DebugEvent::ModuleLoaded {
                    seq: 0,
                    session_id: String::new(),
                    module: DebugModuleView {
                        name: module.module_name.clone(),
                        path: None,
                    },
                });
            }
            publisher.publish(DebugEvent::ThreadStarted {
                seq: 0,
                session_id: String::new(),
                thread_id: 1,
            });
            if ready_tx.send(Ok(session_id)).is_err() {
                publisher.close();
                return;
            }
            run_command_loop(&mut core, commands_rx, &mut publisher);
            publisher.close();
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

fn run_command_loop(
    core: &mut DebugSessionCore,
    commands: Receiver<DebugCommand>,
    publisher: &mut DebugEventPublisher,
) {
    while let Ok(command) = commands.recv() {
        if handle_command(core, command, publisher) {
            break;
        }
    }
}

fn handle_command(
    core: &mut DebugSessionCore,
    command: DebugCommand,
    publisher: &mut DebugEventPublisher,
) -> bool {
    match command {
        DebugCommand::Start(reply) => reply_run(reply, core.start_variants(), publisher, false),
        DebugCommand::StepInto(reply) => {
            reply_run(reply, core.step_into_variants(), publisher, true)
        }
        DebugCommand::StepOver(reply) => {
            reply_run(reply, core.step_over_variants(), publisher, true)
        }
        DebugCommand::StepOut(reply) => reply_run(reply, core.step_out_variants(), publisher, true),
        DebugCommand::Continue(reply) => {
            reply_run(reply, core.continue_execution_variants(), publisher, true)
        }
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
            let view = breakpoint_view_from_core(&record);
            publisher.publish(DebugEvent::BreakpointChanged {
                seq: 0,
                session_id: String::new(),
                change: DebugBreakpointChangeKind::Added,
                breakpoint: view.clone(),
            });
            let _ = reply.send(Ok(view));
            false
        }
        DebugCommand::SetBreakpointEnabled { id, enabled, reply } => {
            let result = core
                .set_breakpoint_enabled(&id, enabled)
                .map(|record| breakpoint_view_from_core(&record))
                .ok_or(DebugError::UnknownBreakpoint(id));
            if let Ok(view) = &result {
                publisher.publish(DebugEvent::BreakpointChanged {
                    seq: 0,
                    session_id: String::new(),
                    change: DebugBreakpointChangeKind::Changed,
                    breakpoint: view.clone(),
                });
            }
            let _ = reply.send(result);
            false
        }
        DebugCommand::ClearSourceBreakpoint { id, reply } => {
            let removed = core.clear_source_breakpoint(&id);
            let result = removed
                .as_ref()
                .map(|_| ())
                .ok_or(DebugError::UnknownBreakpoint(id));
            if let Some(record) = removed {
                publisher.publish(DebugEvent::BreakpointChanged {
                    seq: 0,
                    session_id: String::new(),
                    change: DebugBreakpointChangeKind::Removed,
                    breakpoint: breakpoint_view_from_core(&record),
                });
            }
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
    result: Result<DebugCoreRunResult, DebugSessionError>,
    publisher: &mut DebugEventPublisher,
    continued: bool,
) -> bool {
    if continued {
        publisher.publish(DebugEvent::Continued {
            seq: 0,
            session_id: String::new(),
            all_threads_continued: true,
        });
    }
    let projected = result.map(|result| {
        let view = run_result_view_from_core(&result);
        if continued {
            publisher.publish_pending_outputs();
        }
        match &view {
            DebugRunResultView::Paused(pause) => publisher.publish(DebugEvent::Stopped {
                seq: 0,
                session_id: String::new(),
                reason: pause.reason.clone(),
                thread_id: Some(1),
                frame_id: pause.frame_id.clone(),
                location: pause.current_location.clone(),
            }),
            DebugRunResultView::Exited(DebugExitView { exit_code }) => {
                publisher.publish(DebugEvent::Exited {
                    seq: 0,
                    session_id: String::new(),
                    exit_code: *exit_code,
                })
            }
        }
        view
    });
    let _ = reply.send(projected.map_err(debug_error_from_core));
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

#[derive(Debug)]
struct DebugEventPublisher {
    hub: DebugEventHub,
    session_id: String,
    next_seq: u64,
    output_tap: Arc<Mutex<Vec<(HostOutputChannel, String)>>>,
}

impl DebugEventPublisher {
    fn new(
        hub: DebugEventHub,
        session_id: String,
        output_tap: Arc<Mutex<Vec<(HostOutputChannel, String)>>>,
    ) -> Self {
        Self {
            hub,
            session_id,
            next_seq: 1,
            output_tap,
        }
    }

    fn publish(&mut self, event: DebugEvent) {
        let seq = self.next_seq;
        self.next_seq += 1;
        self.hub.publish(self.with_envelope(event, seq));
    }

    fn close(&self) {
        self.hub.close();
    }

    fn publish_pending_outputs(&mut self) {
        let outputs = {
            let mut outputs = self
                .output_tap
                .lock()
                .expect("debug output tap buffer poisoned");
            std::mem::take(&mut *outputs)
        };
        for (channel, text) in outputs {
            self.publish(DebugEvent::Output {
                seq: 0,
                session_id: String::new(),
                channel: debug_output_channel(channel),
                text,
            });
        }
    }

    fn with_envelope(&self, event: DebugEvent, seq: u64) -> DebugEvent {
        match event {
            DebugEvent::Stopped {
                reason,
                thread_id,
                frame_id,
                location,
                ..
            } => DebugEvent::Stopped {
                seq,
                session_id: self.session_id.clone(),
                reason,
                thread_id,
                frame_id,
                location,
            },
            DebugEvent::Output { channel, text, .. } => DebugEvent::Output {
                seq,
                session_id: self.session_id.clone(),
                channel,
                text,
            },
            DebugEvent::Continued {
                all_threads_continued,
                ..
            } => DebugEvent::Continued {
                seq,
                session_id: self.session_id.clone(),
                all_threads_continued,
            },
            DebugEvent::Exited { exit_code, .. } => DebugEvent::Exited {
                seq,
                session_id: self.session_id.clone(),
                exit_code,
            },
            DebugEvent::BreakpointChanged {
                change, breakpoint, ..
            } => DebugEvent::BreakpointChanged {
                seq,
                session_id: self.session_id.clone(),
                change,
                breakpoint,
            },
            DebugEvent::ModuleLoaded { module, .. } => DebugEvent::ModuleLoaded {
                seq,
                session_id: self.session_id.clone(),
                module,
            },
            DebugEvent::ThreadStarted { thread_id, .. } => DebugEvent::ThreadStarted {
                seq,
                session_id: self.session_id.clone(),
                thread_id,
            },
        }
    }
}

fn debug_output_channel(channel: HostOutputChannel) -> DebugOutputChannel {
    match channel {
        HostOutputChannel::Stdout => DebugOutputChannel::Stdout,
        HostOutputChannel::Stderr => DebugOutputChannel::Stderr,
        HostOutputChannel::Host => DebugOutputChannel::Host,
    }
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
