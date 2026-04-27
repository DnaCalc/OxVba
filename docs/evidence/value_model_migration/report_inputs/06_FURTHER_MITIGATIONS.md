# Further Mitigations

Status: finalized backlog

1. Fix `string_slice_ops_dollar.bas`
   - this remains the highest-value correctness follow-up because it still
     fails on both baseline and migrated `HEAD`
2. Stabilize the multi-iteration string-perf harness
   - replace the current bounded one-iteration canonical artifact once the
     longer paired run completes cleanly
3. Investigate the remaining JIT `long_strings` regression in the current
   core-only `BStr` artifact
   - this is the largest current candidate slowdown in the string-perf artifact
4. Investigate the mild JIT `small_strings` regression and keep VM string rows
   under watch
   - the refreshed artifact improved several VM rows, but perf tuning should
     keep those gains from regressing
5. Look for copy-elision opportunities around `BStr` carrier transitions
   - especially VM/runtime helper paths and COM/native writeback seams
6. Investigate the Variant scalar-classifier slowdown
   - `vme5-perf-check` still shows `scalar_classifier` as the largest measured
     Variant perf regression
7. Investigate object-result and variant-matrix perf deltas
   - those rows still trend materially slower in the current bounded Variant
     perf artifact
8. Revisit hot-path `Variant` and callback footprint once perf tuning begins
   - `Variant = 16 -> 80` and `ComCallbackPayload = 40 -> 48` are accepted
     today, but later tuning should verify whether some growth can be recovered
     without breaking the new observable contract
9. If broader native parity becomes in-scope later, open a dedicated follow-on
   for native UDT-byref and struct-overlay closure
   - that work is explicitly bounded out of this migration and should not be
     smuggled into generic perf cleanup
