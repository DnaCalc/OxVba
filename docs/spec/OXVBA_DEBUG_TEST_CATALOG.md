# OxVba Debug Handle Test Catalog

Status: B00 binding test catalog for `docs/worksets/WORKSET_2026-05-23_OXVBA_DEBUG_HANDLE_ARCHITECTURE.md`.
Owner bead: `bd-00fz.1`.
Design companion: `docs/spec/OXVBA_DEBUG_HANDLE_DESIGN.md`.

This catalog is the executable-test plan for B02-B15. B02 creates ignored stubs for these names so `cargo test -p oxvba-debug -- --list` exposes the full workset shape before feature code lands. Later beads remove `#[ignore]` and turn their owned stubs green.

## Catalog rules

- Test names are stable unless a later bead updates this catalog and its stubs together.
- Each test maps to exactly one owning bead, even when it protects behavior introduced by earlier beads.
- Tests that require Windows COM are `#[cfg(windows)]`; non-COM `None` paths remain cross-platform.
- Tests that require tokio are behind `--features tokio`.
- Property/stress/benchmark lanes may be excluded from fast default CI only if their bead records and evidence state say so explicitly.
- A green test must assert the acceptance claim, not just instantiate scaffolding.

## Fixtures created by B02

| Fixture | Purpose | Minimum content |
|---|---|---|
| `thin_slice` | basic stepping, breakpoints, watches, output, scenarios | one module with stable Dim / assignment / `Debug.Print` lines at file lines 6/7/8 |
| `multi_module_walkthrough` | stack, module-loaded, source-map independence, property sequences | at least two modules with a call path and inspectable locals |
| `com_dispatch_smoke` | COM apartment tests | COM-bearing project path; Windows tests use real apartment assertions, non-Windows skips COM init |
| `bare_no_preamble` | source-map identity | module with no dropped preamble |
| `empty_module` | source-map edge | empty module |
| `preamble_only_module` | source-map edge | attributes/options without executable procedures |

Shared helper file: `crates/oxvba-debug/tests/_shared.rs`.

## B02 - harness, fixtures, and type assertions

| Test file | Test name | Claim |
|---|---|---|
| `handle_send_sync.rs` | `debug_session_handle_is_send_sync_clone` | `DebugSessionHandle: Send + Sync + Clone` without unsafe impls |
| `handle_send_sync.rs` | `debug_session_core_is_not_send_or_sync` | `DebugSessionCore: !Send + !Sync` |
| `fixture_catalog.rs` | `fixture_thin_slice_has_expected_statement_lines` | thin-slice fixture encodes the documented line anchors |
| `fixture_catalog.rs` | `fixture_multi_module_walkthrough_loads` | multi-module fixture is usable by later tests |
| `fixture_catalog.rs` | `fixture_bare_no_preamble_loads` | bare fixture is usable for identity mapping |
| `fixture_catalog.rs` | `fixture_com_dispatch_smoke_declared` | COM smoke fixture is present and cfg-safe |
| `catalog_inventory.rs` | `catalog_stub_inventory_matches_b00` | enumerable ignored stubs cover this catalog |

B02 also creates ignored stubs for every later test listed below.

## B03 - core move regression tests

| Test file | Test name | Claim |
|---|---|---|
| `core_start.rs` | `core_start_stops_on_entry` | moved core preserves entry pause semantics |
| `core_continue.rs` | `core_continue_runs_to_completion` | continue semantics match old host debugger |
| `core_step_into.rs` | `core_step_into_advances_to_next_statement` | step-into semantics preserved |
| `core_step_over.rs` | `core_step_over_preserves_call_depth_policy` | step-over semantics preserved |
| `core_step_out.rs` | `core_step_out_returns_to_caller` | step-out semantics preserved |
| `core_breakpoints.rs` | `core_set_line_breakpoint_binds` | line breakpoint binding preserved |
| `core_breakpoints.rs` | `core_disabled_breakpoint_does_not_stop` | disabled breakpoints remain inert |
| `core_breakpoints.rs` | `core_clear_breakpoint_removes_binding` | clear semantics preserved |
| `core_watches.rs` | `core_add_evaluate_update_remove_watch` | watch registry/evaluation preserved |
| `core_inspect.rs` | `core_stack_frames_and_locals_project` | pause frame/local projection preserved |
| `core_inspect.rs` | `core_evaluate_current_frame_identifier` | bounded evaluation preserved |
| `core_retained_values.rs` | `core_pause_retains_variant_values_for_inspection` | retained-Variant pause data remains valid |
| `core_host_primitives.rs` | `host_debug_runtime_primitives_cover_core_needs` | `oxvba-host` exposes only the narrow required primitives |

Required checks: `cargo test -p oxvba-host`; `cargo test -p oxvba-debug core_`.

## B04 - public view DTOs and projections

| Test file | Test name | Claim |
|---|---|---|
| `views_traits.rs` | `view_types_are_transport_safe` | all view DTOs implement `Send + Sync + Clone + Debug + Serialize + Deserialize` |
| `views_serde.rs` | `pause_view_round_trips_json` | pause view serde round-trip |
| `views_serde.rs` | `breakpoint_view_round_trips_json` | breakpoint view serde round-trip |
| `views_serde.rs` | `watch_view_round_trips_json` | watch view serde round-trip |
| `views_serde.rs` | `frame_view_round_trips_json` | frame view serde round-trip |
| `views_serde.rs` | `value_view_round_trips_json` | value view serde round-trip without raw `Variant` |
| `views_projection.rs` | `pause_state_projects_to_pause_view` | pause projection carries reason/frame/location |
| `views_projection.rs` | `breakpoint_record_projects_to_breakpoint_view` | breakpoint binding status projects correctly |
| `views_projection.rs` | `watch_record_projects_to_watch_view` | watch evaluation projects correctly |
| `views_projection.rs` | `run_result_projects_completion_as_exited` | normal completion is a view result, not an error |

## B05 - handle, worker, and command marshalling

| Test file | Test name | Claim |
|---|---|---|
| `handle_attach.rs` | `attach_returns_handle_and_initial_receiver` | attach constructs worker-owned core and returns usable handle |
| `handle_start.rs` | `handle_start_matches_core_start` | handle start matches raw core semantics |
| `handle_step_into.rs` | `handle_step_into_matches_core_flow` | step-into command marshals and projects correctly |
| `handle_step_over.rs` | `handle_step_over_matches_core_flow` | step-over command marshals and projects correctly |
| `handle_step_out.rs` | `handle_step_out_matches_core_flow` | step-out command marshals and projects correctly |
| `handle_continue.rs` | `handle_continue_matches_core_flow` | continue command marshals and projects correctly |
| `handle_breakpoint_set.rs` | `handle_set_source_breakpoint_binds_real_line` | source breakpoint set uses real binding semantics |
| `handle_breakpoint_toggle.rs` | `handle_set_breakpoint_enabled_toggles_real_binding` | enable/disable affects stops |
| `handle_breakpoint_clear.rs` | `handle_clear_source_breakpoint_removes_stop` | clear removes binding |
| `handle_breakpoints.rs` | `handle_breakpoints_lists_current_records` | list returns projected current records |
| `handle_watch_add.rs` | `handle_add_watch_records_expression` | add watch works through worker |
| `handle_watch_update.rs` | `handle_update_watch_changes_expression` | update watch works through worker |
| `handle_watch_remove.rs` | `handle_remove_watch_deletes_record` | remove watch works through worker |
| `handle_watch_evaluate.rs` | `handle_evaluate_watches_returns_current_values` | evaluate watches returns projected values |
| `handle_inspect.rs` | `handle_current_pause_stack_locals_and_evaluate_work` | inspect commands work when paused |
| `handle_completion.rs` | `handle_continue_past_end_returns_exited_then_completed_errors` | completion is reported as `Exited`; later stepping returns `Completed` |
| `concurrency_serialization.rs` | `eight_callers_serialize_at_worker_channel` | concurrent callers observe worker serialization |

## B06 - event hub and subscription stream

| Test file | Test name | Claim |
|---|---|---|
| `events_initial_receiver_from_attach.rs` | `initial_receiver_observes_startup_events` | attach receiver sees startup events emitted after subscription |
| `events_late_subscriber_future_only.rs` | `late_subscriber_does_not_receive_replay` | later subscribers receive future events only |
| `events_multi_subscriber.rs` | `multiple_subscribers_receive_same_sequence` | event hub broadcasts to all subscribers |
| `events_slow_subscriber_bounded.rs` | `bounded_slow_subscriber_reports_drop_without_blocking_worker` | bounded drop-oldest and lag/drop signal work |
| `events_default_channel_mode.rs` | `default_event_channel_is_bounded_256` | public default is `Bounded(256)` |
| `events_subscriber_drop_safe.rs` | `dropping_subscriber_does_not_poison_worker` | subscriber drop is safe |
| `events_ordering.rs` | `event_seq_precedes_command_response_for_state_change` | event emitted before reply and sequence ordering proves it |

## B07 - event emission and output capture

| Test file | Test name | Claim |
|---|---|---|
| `events_stopped_on_entry.rs` | `attach_stop_on_entry_emits_stopped_entry` | entry pause emits `Stopped(Entry)` |
| `events_stopped_on_breakpoint.rs` | `continue_to_breakpoint_emits_stopped_breakpoint` | breakpoint hit emits `Stopped(Breakpoint)` |
| `events_stopped_on_step.rs` | `step_into_emits_stopped_step` | step command emits `Stopped(Step)` |
| `events_continued.rs` | `continue_emits_continued_before_stop_or_exit` | resume emits `Continued` in order |
| `events_exited.rs` | `completion_emits_exited` | completion emits `Exited` |
| `events_output_debug_print.rs` | `debug_print_emits_output_host_without_suppressing_host_callback` | output tap observes `Debug.Print` and preserves existing callbacks |
| `events_output_stdio.rs` | `stdio_output_emits_typed_output_channels` | stdout/stderr output channels are typed |
| `events_breakpoint_changed.rs` | `breakpoint_add_toggle_clear_emit_changed_events` | breakpoint mutations emit correct change kind |
| `events_module_loaded.rs` | `attach_to_two_module_project_emits_two_module_loaded_events` | modules are reported at attach |
| `events_thread_started.rs` | `attach_emits_primary_thread_started` | primary thread event is emitted |

## B08 - compiler source maps and projection use

| Test file | Test name | Claim |
|---|---|---|
| `source_map_bare.rs` | `bare_source_maps_identity` | no-preamble module maps file/runtime lines identically |
| `source_map_attributes.rs` | `attribute_lines_are_dropped` | `Attribute ...` lines are non-executable/dropped |
| `source_map_options.rs` | `attribute_dropped_option_explicit_preserved` | only attribute drops; `Option Explicit` remains user-visible |
| `source_map_options.rs` | `option_compare_and_option_base_are_preserved` | `Option Compare` / `Option Base` preserve user identity |
| `source_map_option_private.rs` | `option_private_module_is_dropped` | `Option Private Module` is non-executable/dropped |
| `source_map_class_implements.rs` | `class_implements_line_is_dropped` | class `Implements` line does not surface as executable user line |
| `source_map_blanks.rs` | `blank_lines_are_preserved_in_file_mapping` | blank lines preserve file-line identity where appropriate |
| `source_map_comments.rs` | `comments_are_preserved_in_file_mapping` | comments preserve file-line identity where appropriate |
| `source_map_helpers.rs` | `compiler_inserted_helper_lines_are_non_user` | generated helper lines never surface as editor locations |
| `source_map_inverse_property.rs` | `runtime_to_file_round_trips_executable_user_lines` | executable user lines satisfy inverse property |
| `source_map_multi_module.rs` | `module_maps_are_independent` | per-module maps do not cross-contaminate |
| `source_map_empty_module.rs` | `empty_module_has_no_executable_lines` | empty module edge handled |
| `source_map_preamble_only.rs` | `preamble_only_module_has_no_executable_lines` | preamble-only module edge handled |
| `source_map_handle_snapshot.rs` | `thin_slice_file_lines_bind_through_handle` | file lines 6/7/8 bind through handle and views report editor lines |
| `compiler_source_map.rs` | `compiled_project_contains_structured_source_maps` | `CompiledProject` exposes source-map data for every module |

Required checks: `cargo test -p oxvba-compiler`; `cargo test -p oxvba-debug source_map_`.

## B09 - COM apartment management

| Test file | Test name | Claim |
|---|---|---|
| `com_apartment_sta_init.rs` | `worker_reports_sta_when_configured_sta` | Windows worker initializes STA and reports from worker thread |
| `com_apartment_mta_init.rs` | `worker_reports_mta_when_configured_mta` | Windows worker initializes MTA and reports from worker thread |
| `com_apartment_none.rs` | `none_mode_does_not_initialize_com_and_runs_cross_platform` | `None` path makes no COM call and works on non-Windows |
| `com_apartment_multi_session.rs` | `multiple_sessions_have_independent_apartments` | sessions initialize/tear down independently |
| `com_apartment_teardown.rs` | `com_uninitialize_runs_on_worker_shutdown` | shutdown uninitializes COM best-effort |

## B10 - async surface

All B10 tests are under `--features tokio`.

| Test file | Test name | Claim |
|---|---|---|
| `async_step_into.rs` | `step_into_async_matches_sync_step_into` | async wrapper returns same typed result as sync path |
| `async_all_commands.rs` | `every_sync_command_has_async_wrapper` | surface parity between sync and async command methods |
| `async_concurrent.rs` | `concurrent_async_commands_serialize_at_worker` | spawned async callers serialize through same worker |
| `async_cancellation.rs` | `dropping_future_does_not_poison_worker` | cancellation safe; next sync command succeeds |
| `async_event_stream.rs` | `tokio_event_receiver_observes_same_sequence_as_sync_receiver` | async event wrapper preserves event sequence |

Required check: `cargo test -p oxvba-debug --features tokio async_`.

## B11 - lifecycle, detach, and error propagation

| Test file | Test name | Claim |
|---|---|---|
| `lifecycle_attach_failure.rs` | `bad_manifest_returns_attach_error_without_worker_leak` | attach failure returns `DebugAttachError` and no worker remains |
| `lifecycle_explicit_detach.rs` | `detach_last_handle_joins_worker_cleanly` | explicit detach on last handle joins worker |
| `lifecycle_explicit_detach.rs` | `detach_with_clones_returns_outstanding_handles` | clone-safe detach semantics |
| `lifecycle_drop_implicit_detach.rs` | `dropping_all_handles_shuts_down_worker` | implicit drop detach works |
| `lifecycle_drop_with_command_in_flight.rs` | `shutdown_wakes_in_flight_command_with_session_detached` | in-flight command gets typed detach error |
| `lifecycle_worker_panic.rs` | `worker_panic_marks_handle_failed_without_deadlock` | worker panic propagates as `WorkerFailed` |
| `lifecycle_reattach.rs` | `reattach_after_detach_has_fresh_session_id_and_state` | reattach is independent |
| `lifecycle_resource_counters.rs` | `attach_detach_loop_has_stable_thread_and_fd_counts` | resource counters stable |

## B12 - property and snapshot replay

| Test file | Test name | Claim |
|---|---|---|
| `property_random_sequences.rs` | `random_handle_command_sequences_do_not_panic_deadlock_or_return_untyped_errors` | proptest safety invariants for random valid command sequences |
| `property_snapshot.rs` | `canonical_sequence_event_and_view_log_matches_snapshot` | fixed sequence serialized baseline catches semantic drift |

Evidence file: committed snapshot under `crates/oxvba-debug/tests/snapshots/`.

## B13 - concurrency, stress, and benchmarks

| Test/bench file | Test/bench name | Claim |
|---|---|---|
| `stress_concurrent_sessions.rs` | `one_hundred_concurrent_sessions_complete_without_crosstalk_or_leak` | session isolation and resource stability |
| `stress_sequential_commands.rs` | `one_thousand_sequential_commands_stay_bounded` | command path stays bounded and leak-free |
| `stress_attach_detach.rs` | `one_hundred_sequential_attach_detach_cycles_stable` | attach/detach resource stability |
| `benches/handle_latency.rs` | `bench_step_into_round_trip` | step latency baseline captured |
| `benches/handle_latency.rs` | `bench_set_source_breakpoint_round_trip` | breakpoint latency baseline captured |
| `benches/handle_latency.rs` | `bench_evaluate_watches_round_trip` | watch latency baseline captured |

Evidence path: `docs/evidence/oxvba-debug/benchmarks/`.

## B14 - downstream handoff docs

| Check file | Check name | Claim |
|---|---|---|
| `docs/HANDOFF_OXIDE_MIGRATE_TO_DEBUG_HANDLE.md` | read-through check | OxIde migration path maps old adapter helpers to handle methods and retires replay/line-map duplication |
| `docs/HANDOFF_OXVBA_DAP_FROM_DEBUG_HANDLE.md` | read-through check | DAP builder guide maps DAP requests/events to handle commands/events |
| governance | `./scripts/check-governance.ps1` | handoff docs pass repository governance |

No Rust tests are required for B14 unless docs reference generated anchors.

## B15 - reference scenarios and acceptance evidence

| Test/check file | Test/check name | Claim |
|---|---|---|
| `scenarios_dap_style_flow.rs` | `dap_style_flow_attach_breakpoint_stack_evaluate_exit` | scenario A runs end-to-end and unblocks future `oxvba-dap` |
| `scenarios_oxide_cockpit_flow.rs` | `oxide_cockpit_flow_watch_breakpoint_step_disable_exit` | scenario B runs end-to-end and unblocks OxIde `oxide-wf81` |
| acceptance evidence | `docs/evidence/oxvba-debug/acceptance.txt` | human-readable final matrix and command transcript summary exists |
| acceptance evidence | `docs/evidence/oxvba-debug/acceptance.json` | machine-readable final matrix summary exists |
| v1 handoff | `docs/HANDOFF_OXVBA_DEBUG_HANDLE_v1.md` | shipped/deferred/source-migration truth is reconciled |

Required final gates:
- `br lint bd-00fz.1 bd-00fz.2 bd-00fz.3 bd-00fz.4 bd-00fz.5 bd-00fz.6 bd-00fz.7 bd-00fz.8 bd-00fz.9 bd-00fz.10 bd-00fz.11 bd-00fz.12 bd-00fz.13 bd-00fz.14 bd-00fz.15 bd-00fz.16`
- `br dep cycles`
- `br ready` no longer shows open `bd-00fz` child work
- relevant cargo build/test/check lanes green
- `./scripts/check-governance.ps1`
- `docs/evidence/oxvba-debug/acceptance.{txt,json}` present
- downstream handoff docs present and consistent with implementation

## B00 approval checklist

B00 is review-complete when:
- every B02-B15 workset lane has concrete test/check names here;
- design-only B14 checks are explicit even though they are not Rust tests;
- final B15 gates include bead, governance, cargo, evidence, and handoff truth;
- this catalog is consistent with `OXVBA_DEBUG_HANDLE_DESIGN.md` and the workset.
