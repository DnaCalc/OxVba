# V0.2 Native Compilation Obligations

Status: active V0.2 obligation policy

Owner bead: `bd-bqm8.10.3`

Machine-readable matrix:
`docs/validation/V02_NATIVE_COMPILATION_OBLIGATIONS_V1.csv`

Decision dependency:
[V02_NATIVE_COMPILATION_PATH_DECISION_V1.md](/C:/Work/DnaCalc/OxVba/docs/validation/V02_NATIVE_COMPILATION_PATH_DECISION_V1.md)

## Policy

The V0.2 native compilation lane is wrapper-hosted. Native packaging evidence
must prove the generated artifact surface and runtime boundary it claims. It
must not turn source generation, disabled JIT placeholders, or registration skeletons
into claims of complete direct native-image, Office COM, XLL, or hostless AOT
parity.

## Obligations

| ID | Area | Obligation | Required Gate |
| --- | --- | --- | --- |
| NATIVE-V02-O001 | Bundle artifact | Embed compiled `.oxb` bundle through generated wrapper source. | Generated source contains deterministic `include_bytes!` bundle reference. |
| NATIVE-V02-O002 | Host runtime | Initialize the OxVba runtime host and execute through supported `Engine` APIs. | EXE scaffold validates generated source and runtime invocation surface. |
| NATIVE-V02-O003 | JIT placeholder | Keep `oxvba-jit` as an explicit not-implemented API boundary until JIT v2 lands. | Scaffold runs the not-implemented API guard. |
| NATIVE-V02-O004 | DLL exports | Respect declared calling conventions and C ABI carriers. | DLL shim source validates exported symbol shape and runtime bridge. |
| NATIVE-V02-O005 | COM server | Expose COM entry points and class factory skeletons with residuals explicit. | COM source structure validates; dispatch and registration gaps remain recorded. |
| NATIVE-V02-O006 | XLL add-in | Expose required XLL entry points and registration metadata. | XLL source validates entry points and type-string generation. |
| NATIVE-V02-O007 | Value ABI | Materialize ABI carriers only at external boundaries. | Evidence references retained `Variant` representation and slot ABI tests. |
| NATIVE-V02-O008 | Platform target | Keep Windows as primary COM/XLL target and bound non-Windows claims. | Scaffold records target-specific pass or skip behavior. |
| NATIVE-V02-O009 | Registration deployment | Validate deployment separately from source generation. | Final checklist records exercised registration helpers or residual status. |
| NATIVE-V02-O010 | Artifact provenance | Emit stable evidence identity. | Scaffold artifacts include command, run id, source docs, and evidence paths. |

## Claim Rules

- "Native compilation" in V0.2 means wrapper-hosted compiled artifacts around
  `.oxb` bundles plus runtime execution, not standalone native images.
- JIT evidence may only claim the disabled placeholder state until a new JIT v2
  execution design lands with explicit backend evidence.
- COM server and XLL evidence must distinguish source skeleton validation from
  Office registration, loading, and invocation validation.
- Deployment evidence must name the exact command, platform, artifact, and
  registration or packaging step exercised.
