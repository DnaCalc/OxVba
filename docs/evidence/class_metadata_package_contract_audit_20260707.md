# Class Metadata Package Contract Audit - 2026-07-07

Bead: `bd-h4oh.10.26`

Scope: project class metadata needed by VM3/JIT execution after the project-event JIT slice. COM
wire structs, COM export packaging, imported COM coclass activation, and live COM interop are
explicitly outside this audit and remain deferred to later COM/AOT lanes.

## Descriptor Path

The class contract is carried without source reconstruction through this path:

1. `oxvba-bundle::ClassDescriptor`
2. `oxvba-oxir::OxClass`
3. `oxvba-rt-abi::runtime_class_descriptors_for_program`
4. `oxvba-runtime::RuntimeClassDescriptor`
5. VM3/JIT loaded-program descriptor tables

## Audited Facts

| Fact | Package/OxIR carrier | Runtime carrier | Evidence |
| --- | --- | --- | --- |
| Class identity | `unit_name` + class table index / cross-bundle imports | `RuntimeProjectClassIdentity { unit_name, class_index }` | `jit_project_objects` referenced-project rows; `bd-h4oh.10.24` closure |
| Instance fields | `ClassField` / `OxClassField` with stable `token`, `ty`, array element metadata | `RuntimeClassFieldDescriptor` | `jit_project_objects`, `class_lifecycle_vm3`, verifier field/as-new checks |
| Lazy `As New` fields | `ClassAsNewField` / `OxClassAsNewField` | `RuntimeClassAsNewFieldDescriptor` | `jit_project_field_as_new_reinstantiates_after_set_nothing_without_fallback` |
| Methods and property accessors | `ClassMethod` / `OxClassMethod` with `ProjectMemberKind` | `RuntimeMemberDescriptor.invoke_kind` | `jit_project_method_*`, property get/let/set rows, OxIR verifier accessor-shape tests |
| Hidden `Me` | class procedure local 0 / verifier receiver checks | invoked through `ProcInvoker` and object dispatch helpers | OxIR verifier hidden-`Me` tests; VM3/JIT member dispatch rows |
| Default members | `is_default_member`, `dispid` | `RuntimeMemberDescriptor.is_default_member`, synthetic/default dispatch id | `jit_project_*default_member*`, `default_member_index_vm3`, runtime dispatch cache tests |
| `_NewEnum` | `is_enumerator_member` | `RuntimeMemberDescriptor.is_enumerator_member` | `project_class_newenum_vm3`, runtime COM-export-shape descriptor tests |
| Lifecycle hooks | `initialize` / `terminate` procedure ids | `RuntimeClassLifecycleDescriptor` | `jit_project_set_nothing_runs_class_terminate_before_next_statement`, lifecycle corpus |
| Predeclared singleton | `predeclared` | `RuntimeClassDescriptor.predeclared` and loaded singleton table | `jit_predeclared_*`, referenced predeclared singleton rows |
| Implemented project interfaces | `implements` names and project interface descriptors in `com_interfaces` | `implements` names plus generated project `RuntimeInterfaceDescriptor`s | `TypeOf`/`Set` compatibility rows and OxIR interface descriptor verifier coverage |
| Project events | `event_routes` table | `LoadedProgram.event_routes` | `raiseevent_fanout_vm3` VM3/JIT numeric rows; `bd-h4oh.10.25` closure |

## Residual Routing

No new package-contract field is required before the next M4-8 class beads.

Behavioral and source-level residuals remain routed to:

- `bd-h4oh.10.27`: binder/lowering class semantics residual suite.
- `bd-h4oh.10.28`: VM3/JIT class execution parity sweep for the accepted project-class subset.
- `bd-h4oh.10.29`: terminal docs, unsupported diagnostics, and residual handoff.
- `bd-h4oh.15.1`: deferred COM-export descriptor readiness handoff under the later COM/AOT lane.

This audit does not claim COM export/server readiness. Runtime descriptor COM-shape validators are
kept as Linux descriptor consistency evidence only.
