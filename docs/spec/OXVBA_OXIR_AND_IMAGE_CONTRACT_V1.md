# OxVba OxIR, OxImage and VM3 Contract V1

Date: 2026-07-10
Status: current architecture contract
System clauses: `SYS-ART-001`, `IR-*`, `IMAGE-*`, `VM3-*`, `DEBUG-MAP-001`
Supersedes: `EXECUTABLE_SEMANTIC_PACKAGE_V1.md`, `EXECUTABLE_SEMANTIC_PACKAGE_COMPLETION_MAP_V1.md`, `BYTECODE_VM_SEMANTIC_CONTRACT_V1.md` and `VMR06_DESCRIPTOR_DRIVEN_BEHAVIOR_SELECTION_V1.md` for current architecture

## 1. Target state

OxIR is the single typed executable-semantic representation shared by VM3 and the JIT. OxImage is the versioned serialized project-closure artifact containing verified OxPrograms and the metadata required to link, execute, debug, cache and package them.

The historical `Bundle`/`Op`/`.oxb` execution machine is retired. `oxvba-bundle::coreir` remains the compiler semantic tree; bounded legacy Bundle types used to describe the synthetic VBA library are migration scaffolding, not a second product artifact.

## 2. Layering

```text
compiler CoreProgram
  -> oxvba-oxir elaboration
  -> OxProgram (typed CFG and descriptors)
  -> verification
  -> VerifiedOxProgram
  -> OxImage project closure
  -> bounded decode + image verification
  -> VerifiedOxImage
       |-- VM3 session
       |-- JIT session/cache
       |-- wrapper/native packaging
```

Core IR owns resolved language intent. OxIR owns executable control/data flow and runtime-facing descriptors. Backends own execution strategy. No backend accepts source names or reconstructs binding facts.

## 3. OxIR program contract

Each OxProgram contains:

- typed functions, parameters, locals, places and basic blocks;
- an explicit program entry and global initializer;
- typed instructions, operands, results and terminators;
- fallible-operation fault edges and error-handler state;
- declared global storage and array metadata;
- class, interface, record and event metadata;
- external call, COM interface and native descriptor tables;
- project imports, exports and referenced identities;
- source/debug provenance.

The instruction vocabulary is closed and inspectable. Every instruction and terminator has:

- operand/result type rules;
- fault and side-effect classification;
- ownership/lifetime effects;
- VM3 disposition;
- JIT disposition;
- target/capability admission rule;
- direct OxIR and source-lowered tests.

Adding an instruction requires verifier, VM3, JIT, source-map and matrix dispositions in the same architectural change.

## 4. Type and descriptor fidelity

OxIR types retain declared scalar, fixed-string, Variant, array rank/element, record identity/layout and object/interface identities required by VBA behavior and native boundaries. Dynamic Variant execution does not erase declared storage or callable signatures.

Class, interface, COM, event, record, array and external descriptors use stable table identities. References cannot depend on vector position without verification. Nominal COM interface arrays and VT_RECORD-capable records carry the exact metadata required by Windows interop.

## 5. Verification boundary

Production consumers accept sealed verified handles. Test-only unchecked constructors are explicit and cannot leak into product APIs.

Bounded decoding validates size, nesting, counts and schema before large allocation. Semantic verification then covers:

- schema, target, profile and capability compatibility;
- program/image entry identities;
- unique case-folded project/unit/export identities;
- function, block, local, global, import and descriptor references;
- CFG and fault-edge structure;
- complete operand/result typing and rank rules;
- call arity/signature and ByRef/place legality;
- global initialization and source-map integrity;
- class/interface/event/record/array descriptor consistency;
- ownership, cleanup and effect invariants;
- import/export closure and provenance.

Malformed or hostile images return stable diagnostics without panic, unbounded allocation, linking or code generation.

## 6. OxImage identity and versioning

An OxImage contains:

- schema and feature version;
- ordered project closure with an explicit entry project;
- verified OxPrograms;
- content/integrity digest;
- target/profile/capability requirements;
- required helper-catalog identity, digest and ABI version;
- required carrier/layout identity, digest and ABI version;
- build/compiler version and settings affecting meaning;
- complete reference/provenance manifest;
- source/debug maps;
- optional packaging metadata that does not change execution semantics.

The entry field is authoritative; consumers do not infer the entry from “last program” convention. Link tables reject ambiguous imports/exports instead of choosing first matches.

Version compatibility is explicit: a loader either accepts and upgrades a known compatible schema or rejects it. It never interprets unknown fields by historical convention.

OxImage owns the recorded requirements and the verifier-owned compatibility check; it does not define helper or carrier semantics. `oxvba-rt-abi` owns the versioned helper descriptor catalog and carrier ABI facts. Early artifact-schema, sealed-handle, bounded-decode and base-verifier work therefore does not wait for the complete runtime catalog; later image compatibility verification consumes the exact catalog identity/digest produced by the runtime layer.

## 7. VM3 contract

VM3 executes `VerifiedOxImage`/`VerifiedOxProgram` through heap-owned frames, typed locals/places, explicit control flow and shared runtime/evaluation helpers. It implements every verified operation admitted for its target; raw `OxProgram` is never a production execution input.

VM3 is the reference interpreter for backend parity, but its behavior is not self-authorizing. Excel/VBA and public specifications remain the semantic target; corrected VM3 behavior updates the golden corpus and JIT differential expectations.

VM3 honors the declared image/program entry, global initialization order, project linking, error state, class lifecycle, termination drains, events and host/session state. Source recursion and logical frame limits do not depend on native-stack survival.

## 8. Runtime and ownership integration

VM3 and JIT share runtime carrier, evaluation and helper contracts through `oxvba-runtime`, `oxvba-eval` and `oxvba-rt-abi`. The versioned `oxvba-rt-abi` catalog is the sole source for VM3, JIT and Windows helper registration. Descriptor projection has one owner and session-bounded storage; backends do not independently leak parallel class/interface metadata or private helper catalogs.

AddRef, Release, cleanup, termination and panic/fault paths are explicit. Repeated image/session create/invoke/reset/drop cycles have bounded memory and zero carrier/interface imbalance.

## 9. Source and debug maps

OxIR instructions, blocks and generated helper/entry thunks map to compiler document identities and original or virtual source spans. Runtime errors, Erl, stack frames, debugging and native sidecars consume the same provenance.

Optimization and normalization passes preserve or deliberately merge source mappings with an inspectable rule. A backend cannot discard locations required by VBA error semantics.

## 10. Product sessions

A verified image can create equivalent VM3 or JIT project sessions. Session state includes globals, class singletons, live objects, events, Err, host/profile policy and backend-owned execution state.

Load, initialize, invoke, reset/reload and dispose are explicit transitions. Serialized artifacts can execute without source or compiler crates where the output class promises that property.

## 11. Evidence

The canonical OxIR/backend matrix covers every instruction and terminator. The package matrix covers every field, verifier family, compatibility rule and malformed-input class.

Completion requires:

- mutation/property/fuzz tests for all verifier families;
- non-last explicit-entry tests and ambiguous-link rejection;
- round-trip/version/target/ABI tests;
- VM3 execution tests for every verified operation;
- structural VM3/JIT differentials;
- repeated-session and fault cleanup stress;
- source/debug-map and runtime-error location tests;
- hostile artifact resource-bound evidence.
