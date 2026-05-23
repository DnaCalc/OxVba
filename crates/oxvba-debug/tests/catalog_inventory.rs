const CATALOG: &[(&str, &str)] = &[
    (
        "async_all_commands.rs",
        "every_sync_command_has_async_wrapper",
    ),
    (
        "async_cancellation.rs",
        "dropping_future_does_not_poison_worker",
    ),
    (
        "async_concurrent.rs",
        "concurrent_async_commands_serialize_at_worker",
    ),
    (
        "async_event_stream.rs",
        "tokio_event_receiver_observes_same_sequence_as_sync_receiver",
    ),
    (
        "async_step_into.rs",
        "step_into_async_matches_sync_step_into",
    ),
    (
        "benches/handle_latency.rs",
        "bench_evaluate_watches_round_trip",
    ),
    (
        "benches/handle_latency.rs",
        "bench_set_source_breakpoint_round_trip",
    ),
    ("benches/handle_latency.rs", "bench_step_into_round_trip"),
    (
        "com_apartment_mta_init.rs",
        "worker_reports_mta_when_configured_mta",
    ),
    (
        "com_apartment_multi_session.rs",
        "multiple_sessions_have_independent_apartments",
    ),
    (
        "com_apartment_none.rs",
        "none_mode_does_not_initialize_com_and_runs_cross_platform",
    ),
    (
        "com_apartment_sta_init.rs",
        "worker_reports_sta_when_configured_sta",
    ),
    (
        "com_apartment_teardown.rs",
        "com_uninitialize_runs_on_worker_shutdown",
    ),
    (
        "compiler_source_map.rs",
        "compiled_project_contains_structured_source_maps",
    ),
    (
        "concurrency_serialization.rs",
        "eight_callers_serialize_at_worker_channel",
    ),
    (
        "core_breakpoints.rs",
        "core_clear_breakpoint_removes_binding",
    ),
    (
        "core_breakpoints.rs",
        "core_disabled_breakpoint_does_not_stop",
    ),
    ("core_breakpoints.rs", "core_set_line_breakpoint_binds"),
    ("core_continue.rs", "core_continue_runs_to_completion"),
    (
        "core_host_primitives.rs",
        "host_debug_runtime_primitives_cover_core_needs",
    ),
    ("core_inspect.rs", "core_evaluate_current_frame_identifier"),
    ("core_inspect.rs", "core_stack_frames_and_locals_project"),
    (
        "core_retained_values.rs",
        "core_pause_retains_variant_values_for_inspection",
    ),
    ("core_start.rs", "core_start_stops_on_entry"),
    (
        "core_step_into.rs",
        "core_step_into_advances_to_next_statement",
    ),
    ("core_step_out.rs", "core_step_out_returns_to_caller"),
    (
        "core_step_over.rs",
        "core_step_over_preserves_call_depth_policy",
    ),
    ("core_watches.rs", "core_add_evaluate_update_remove_watch"),
    (
        "events_breakpoint_changed.rs",
        "breakpoint_add_toggle_clear_emit_changed_events",
    ),
    (
        "events_continued.rs",
        "continue_emits_continued_before_stop_or_exit",
    ),
    (
        "events_default_channel_mode.rs",
        "default_event_channel_is_bounded_256",
    ),
    ("events_exited.rs", "completion_emits_exited"),
    (
        "events_initial_receiver_from_attach.rs",
        "initial_receiver_observes_startup_events",
    ),
    (
        "events_late_subscriber_future_only.rs",
        "late_subscriber_does_not_receive_replay",
    ),
    (
        "events_module_loaded.rs",
        "attach_to_two_module_project_emits_two_module_loaded_events",
    ),
    (
        "events_multi_subscriber.rs",
        "multiple_subscribers_receive_same_sequence",
    ),
    (
        "events_ordering.rs",
        "event_seq_precedes_command_response_for_state_change",
    ),
    (
        "events_output_debug_print.rs",
        "debug_print_emits_output_host_without_suppressing_host_callback",
    ),
    (
        "events_output_stdio.rs",
        "stdio_output_emits_typed_output_channels",
    ),
    (
        "events_slow_subscriber_bounded.rs",
        "bounded_slow_subscriber_reports_drop_without_blocking_worker",
    ),
    (
        "events_stopped_on_breakpoint.rs",
        "continue_to_breakpoint_emits_stopped_breakpoint",
    ),
    (
        "events_stopped_on_entry.rs",
        "attach_stop_on_entry_emits_stopped_entry",
    ),
    ("events_stopped_on_step.rs", "step_into_emits_stopped_step"),
    (
        "events_subscriber_drop_safe.rs",
        "dropping_subscriber_does_not_poison_worker",
    ),
    (
        "events_thread_started.rs",
        "attach_emits_primary_thread_started",
    ),
    (
        "handle_attach.rs",
        "attach_returns_handle_and_initial_receiver",
    ),
    (
        "handle_breakpoint_clear.rs",
        "handle_clear_source_breakpoint_removes_stop",
    ),
    (
        "handle_breakpoint_set.rs",
        "handle_set_source_breakpoint_binds_real_line",
    ),
    (
        "handle_breakpoint_toggle.rs",
        "handle_set_breakpoint_enabled_toggles_real_binding",
    ),
    (
        "handle_breakpoints.rs",
        "handle_breakpoints_lists_current_records",
    ),
    (
        "handle_completion.rs",
        "handle_continue_past_end_returns_exited_then_completed_errors",
    ),
    ("handle_continue.rs", "handle_continue_matches_core_flow"),
    (
        "handle_inspect.rs",
        "handle_current_pause_stack_locals_and_evaluate_work",
    ),
    ("handle_start.rs", "handle_start_matches_core_start"),
    ("handle_step_into.rs", "handle_step_into_matches_core_flow"),
    ("handle_step_out.rs", "handle_step_out_matches_core_flow"),
    ("handle_step_over.rs", "handle_step_over_matches_core_flow"),
    ("handle_watch_add.rs", "handle_add_watch_records_expression"),
    (
        "handle_watch_evaluate.rs",
        "handle_evaluate_watches_returns_current_values",
    ),
    (
        "handle_watch_remove.rs",
        "handle_remove_watch_deletes_record",
    ),
    (
        "handle_watch_modify.rs",
        "handle_update_watch_changes_expression",
    ),
    (
        "lifecycle_attach_failure.rs",
        "bad_manifest_returns_attach_error_without_worker_leak",
    ),
    (
        "lifecycle_drop_implicit_detach.rs",
        "dropping_all_handles_shuts_down_worker",
    ),
    (
        "lifecycle_drop_with_command_in_flight.rs",
        "shutdown_wakes_in_flight_command_with_session_detached",
    ),
    (
        "lifecycle_explicit_detach.rs",
        "detach_last_handle_joins_worker_cleanly",
    ),
    (
        "lifecycle_explicit_detach.rs",
        "detach_with_clones_returns_outstanding_handles",
    ),
    (
        "lifecycle_reattach.rs",
        "reattach_after_detach_has_fresh_session_id_and_state",
    ),
    (
        "lifecycle_resource_counters.rs",
        "attach_detach_loop_has_stable_thread_and_fd_counts",
    ),
    (
        "lifecycle_worker_panic.rs",
        "worker_panic_marks_handle_failed_without_deadlock",
    ),
    (
        "property_random_sequences.rs",
        "random_handle_command_sequences_do_not_panic_deadlock_or_return_untyped_errors",
    ),
    (
        "property_snapshot.rs",
        "canonical_sequence_event_and_view_log_matches_snapshot",
    ),
    (
        "scenarios_dap_style_flow.rs",
        "dap_style_flow_attach_breakpoint_stack_evaluate_exit",
    ),
    (
        "scenarios_oxide_cockpit_flow.rs",
        "oxide_cockpit_flow_watch_breakpoint_step_disable_exit",
    ),
    ("source_map_attributes.rs", "attribute_lines_are_dropped"),
    ("source_map_bare.rs", "bare_source_maps_identity"),
    (
        "source_map_blanks.rs",
        "blank_lines_are_preserved_in_file_mapping",
    ),
    (
        "source_map_class_implements.rs",
        "class_implements_line_is_dropped",
    ),
    (
        "source_map_comments.rs",
        "comments_are_preserved_in_file_mapping",
    ),
    (
        "source_map_empty_module.rs",
        "empty_module_has_no_executable_lines",
    ),
    (
        "source_map_handle_snapshot.rs",
        "thin_slice_file_lines_bind_through_handle",
    ),
    (
        "source_map_helpers.rs",
        "compiler_inserted_helper_lines_are_non_user",
    ),
    (
        "source_map_inverse_property.rs",
        "runtime_to_file_round_trips_executable_user_lines",
    ),
    ("source_map_multi_module.rs", "module_maps_are_independent"),
    (
        "source_map_option_private.rs",
        "option_private_module_is_dropped",
    ),
    (
        "source_map_options.rs",
        "attribute_dropped_option_explicit_preserved",
    ),
    (
        "source_map_options.rs",
        "option_compare_and_option_base_are_preserved",
    ),
    (
        "source_map_preamble_only.rs",
        "preamble_only_module_has_no_executable_lines",
    ),
    (
        "stress_attach_detach.rs",
        "one_hundred_sequential_attach_detach_cycles_stable",
    ),
    (
        "stress_concurrent_sessions.rs",
        "one_hundred_concurrent_sessions_complete_without_crosstalk_or_leak",
    ),
    (
        "stress_sequential_commands.rs",
        "one_thousand_sequential_commands_stay_bounded",
    ),
    (
        "views_projection.rs",
        "breakpoint_record_projects_to_breakpoint_view",
    ),
    ("views_projection.rs", "pause_state_projects_to_pause_view"),
    (
        "views_projection.rs",
        "run_result_projects_completion_as_exited",
    ),
    ("views_projection.rs", "watch_record_projects_to_watch_view"),
    ("views_serde.rs", "breakpoint_view_round_trips_json"),
    ("views_serde.rs", "frame_view_round_trips_json"),
    ("views_serde.rs", "pause_view_round_trips_json"),
    ("views_serde.rs", "value_view_round_trips_json"),
    ("views_serde.rs", "watch_view_round_trips_json"),
    ("views_traits.rs", "view_types_are_transport_safe"),
];

#[test]
fn catalog_stub_inventory_matches_b00() {
    assert!(
        CATALOG.len() >= 80,
        "catalog should enumerate the full B03-B15 stub set"
    );
    let catalog_doc = include_str!("../../../docs/spec/OXVBA_DEBUG_TEST_CATALOG.md");
    for (file, test_name) in CATALOG {
        assert!(catalog_doc.contains(file), "catalog doc should name {file}");
        assert!(
            catalog_doc.contains(test_name),
            "catalog doc should name {test_name}"
        );
    }
}
