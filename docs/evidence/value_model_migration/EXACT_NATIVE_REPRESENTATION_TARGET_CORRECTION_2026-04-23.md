# Exact Native Representation Target Correction (2026-04-23)

Status: binding correction for the value-model migration workset

This correction supersedes any prior checklist or bead wording that treated
`BSTR`-shaped, `VARIANT`-shaped, or `SAFEARRAY`-style internal carriers as
sufficient closure for the representation migration.

Correct target:

1. The canonical internal OxVba string type must be `BSTR`.
2. The canonical internal late-bound/general value used for `Dim x` must be
   exactly the Windows/COM `VARIANT` representation defined by the Windows
   headers and COM Automation specification.
3. String payloads in that value must be real `BSTR` pointers.
4. Array payloads in that value must be real `SAFEARRAY*` pointers.
5. Object/interface payloads in that value must be real COM-compatible
   interface pointers.

Non-closure examples:

1. A UTF-16/BSTR-shaped Rust-owned string carrier is progress, not completion.
2. A 16-byte `Variant` core beside a separate semantic runtime value model is
   progress, not completion.
3. A SAFEARRAY-style descriptor allocated and owned by OxVba is progress, not
   completion if the intended internal payload is the actual COM `SAFEARRAY`
   contract.
4. Passing COM boundary tests is not sufficient if the native representation is
   still only materialized at boundary/helper seams.

Immediate process effect:

1. Reopen the string/BSTR representation epic and closure checklist.
2. Reopen the Variant/SAFEARRAY representation epic and closure checklist.
3. Keep the final report and final checklist open.
4. Update the expected completion matrix so it distinguishes:
   - exact internal native representation,
   - native-shaped intermediate representation,
   - boundary projection only.

Prior checklist status:

1. `VARIANT_SAFEARRAY_INTRINSIC_CLOSURE_CHECKLIST_2026-04-23.md` is superseded
   as a closure artifact.
2. It remains useful evidence for intermediate progress only.
