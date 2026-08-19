# CORE-7 Portable Basics Pause Gate

Date: 2026-08-18
Bead: `bd-59co.2.9.8`
Status: support evidence. This does not close CORE-7 or `CORE-JIT-LOWERING`.

## Outcome

The portable VM3/JIT basics tranche is in place. JIT now matches the
interpreter on the locked portable corpus for scalars, control, coercion,
strings, arrays, records, error/Erl, calls/ByRef/Optional/ParamArray, project
objects, and portable library routes.

Windows COM, Declare execution, pointers, JIT sessions/cache and packaging
were deliberately left alone.

## Locked portable corpus

All of these are exact VM3/JIT matches except the last row:

| Family/label | State |
|---|---|
| scalar/checked_long_loop | match |
| scalar/boolean_and_compare | match |
| coercion/variant_string_long | match |
| control/if_elseif_else | match |
| control/do_while | match |
| string/concat_and_len | match |
| string/mid_mutation_boundary | match |
| array/dynamic_long_loop | match |
| array/foreach_array_sum | match |
| array/array_function_sum | match |
| record/simple_udt_field | match |
| error/resume_next_div_zero | match |
| error/erl_numeric_line | match |
| error/err_number_write | match |
| call/static_function_byval | match |
| call/byref_writeback | match |
| call/optional_omitted_variant | match |
| call/paramarray_sum | match |
| call/optional_omitted_long | match |
| library/mid_left_len | match |
| library/abs_long | match |
| library/new_collection_count | match |
| admission/unused_declare_metadata | match |
| admission/used_declare_still_declines | owned decline `bd-59co.2.9.9` / Windows |

Neighboring suites also passed: `jit_project_objects` 45, `jit_udt_class_aggregates` 14, `jit_local_type_carriers` 5, `jit_linux_safe_generated` 11, `jit_linux_safe_scope` snapshot.

## Pause

Stop here. Next discussion is the Windows/COM/Declare/pointer/session/packaging
surface. `bd-59co.2.9.9` remains the residual CORE-7 architecture owner and is
blocked on CORE-3/4/5 plus this pause gate.
