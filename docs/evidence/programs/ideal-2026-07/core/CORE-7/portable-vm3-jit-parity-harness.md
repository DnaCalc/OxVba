# CORE-7 Portable VM3/JIT Parity Harness

Date: 2026-08-18
Bead: `bd-59co.2.9.2`
Status: in-progress delivery evidence. This does not close `CORE-JIT-LOWERING`.

## Outcome

`crates/oxvba-differential/tests/jit_portable_vm3_parity.rs` is the fail-closed
portable-basics differential harness. Each fixture is either an exact VM3/JIT
match or an owned later CORE-7 gap. Silent skips are rejected.

Compared axes: result snapshot, full `Err` (Number/Source/Description/
LastDllError), raised, and current-thread handle-balance. `Erl` is observed
through an explicit `e = Erl` global until the host `FinalErr` surface grows
that field.

Windows COM, Declare execution, pointers, sessions and packaging are not in
this corpus except as an admission-decline fixture that must not execute
native code.

## Command

`cargo test -p oxvba-differential --test jit_portable_vm3_parity -- --nocapture`

Result: 7 passed, 0 failed.

## Ledger

| Family/label | Expectation | Observed |
|---|---|---|
| scalar/checked_long_loop | match | match |
| scalar/boolean_and_compare | match | match |
| coercion/variant_string_long | match | match |
| control/if_elseif_else | match | match |
| control/do_while | match | match |
| string/concat_and_len | match | match |
| string/mid_mutation_boundary | match | match |
| array/dynamic_long_loop | match | match |
| record/simple_udt_field | match | match |
| error/resume_next_div_zero | match | match |
| error/erl_numeric_line | open gap `bd-59co.2.9.3` | owned |
| error/err_number_write | open gap `bd-59co.2.9.3` | owned |
| call/static_function_byval | match | match |
| call/byref_writeback | match | match |
| call/optional_omitted_long | match | match (first run recorded early match; locked) |
| library/abs_long | match | match |
| library/new_collection_count | match | match |
| admission/unused_declare_metadata | JIT decline `bd-59co.2.9.4` | decline |

## Residual

Remaining portable gaps stay with `bd-59co.2.9.3` through `.7`. Remaining
CORE-7 architecture stays with `bd-59co.2.9.9`. This harness is not VBA
oracle evidence.
