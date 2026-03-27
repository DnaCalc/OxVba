# Current Blockers

Date: 2026-03-11  
Run context: active parity/compliance execution plus in-progress feature worklist execution pass

## Status update

### BLK-EVT-001: Runtime subscription graph execution model
- Status: resolved in current run.
- Resolution summary:
  - Removed compile-time bounded owner fanout from `RaiseEvent` lowering.
  - Added runtime owner-iteration intrinsics:
    - `__oxvba_withevents_first_owner(source, binding)`
    - `__oxvba_withevents_next_owner()`
  - Wrapper lowering now iterates runtime owner bindings dynamically and dispatches handlers with sink-owner identity.
  - Added/updated compiler/optimizer/VM/host tests to lock deterministic behavior.

### BLK-RUNTIME-VALUE-MODEL-001: Runtime value-model migration
- Status: resolved in current run.
- Resolution summary:
  - VM/register/host execution is now value-first end to end:
    - register storage persists `RuntimeValue`,
    - public VM/JIT/host execution APIs are semantic-snapshot first,
    - `snapshot_slots(...)` survives only as an explicit compatibility projection.
  - The interpreter loop no longer executes through the old raw slot-helper vocabulary:
    - core compare/boolean/jump/increment lanes now read/write semantic runtime values,
    - the wider loop now uses explicit legacy-projection helpers over `RuntimeValue` where scalar compatibility is still intentional,
    - `CopySlot` now preserves full `RuntimeValue` shape instead of collapsing through the integer lane.
  - The owned runtime `Variant` bridge now honestly covers the current scalar/error subset:
    - `Empty`,
    - `Null`,
    - `ErrorCode`,
    - `I32`,
    - `Bool`.
  - The dynamic-object protocol blocker that followed this migration is now also resolved:
    - native project class methods, properties, and default-member dispatch all execute on the shared dynamic-object protocol.

### BLK-COM-BOUNDARY-001: Final oxvba-com extraction from HAL
- Status: resolved in current run.
- Resolution summary:
  - oxvba-com now exposes WindowsComBridge as the live Windows COM client facade.
  - standard.rs now delegates create-object activation, invoke execution, object description/release, event subscription/callback access, and typelib resolve/load/invalidate through that bridge.
  - native subscription transport teardown for object release now also executes inside oxvba-com, removing the last substantive COM lifecycle seam from HAL.
  - the remaining HAL COM code is limited to capability/policy gating, apartment/bootstrap hooks, deterministic projection fallback, and error mapping.
  - the IP-04 closure verification matrix is green:
    - cargo fmt --all,
    - cargo clippy -p oxvba-com -p oxvba-hal --all-targets -- -D warnings,
    - cargo test -p oxvba-com -p oxvba-hal -p oxvba-host --quiet,
    - ./scripts/check-governance.ps1,
    - ./scripts/meta-check.ps1 -Fast -NoArtifacts.

## Active blocker entries

### BLK-COM-IDISPATCH-001: Late-bound COM parity remains below VBA/Excel `IDispatch` behavior
- Impact:
  - Blocks `IP-03` Windows late-bound COM client parity.
  - Blocks full closure of `HAL-DYN-008` and parts of `IP-09` declare/marshaling parity.
- Current state (tabular evidence matrix):

  **Invoke transport:**

  | lane                          | status       | evidence                              |
  |-------------------------------|--------------|---------------------------------------|
  | named/omitted arg metadata    | proved-exec  | ComInvokeRequest carries per-arg name |
  | named-arg DISPPARAMS packing  | proved-exec  | method/property-get lanes             |
  | property-put/putref canonical | proved-exec  | indexed/named arg canonicalization     |
  | omitted-arg fault             | proved-exec  | deterministic required-arg faults      |

  **Scalar result conversion:**

  | VT code   | carrier        | status       |
  |-----------|----------------|--------------|
  | VT_EMPTY  | Empty          | proved-exec  |
  | VT_NULL   | Null           | proved-exec  |
  | VT_ERROR  | ErrorCode      | proved-exec  |
  | VT_BOOL   | Bool           | proved-exec  |
  | VT_I1..I4 | I32            | proved-exec  |
  | VT_I8     | I32 or I64     | proved-exec  |
  | VT_UI1..2 | I32            | proved-exec  |
  | VT_UI4    | I32 or I64     | proved-exec  |
  | VT_UI8    | I32 or I64     | proved-exec  |
  | VT_INT    | I32            | proved-exec  |
  | VT_UINT   | I32 or I64     | proved-exec  |
  | VT_R4     | F64(Single)    | proved-exec  |
  | VT_R8     | F64(Double)    | proved-exec  |
  | VT_DATE   | F64(Date)      | proved-exec  |
  | VT_CY     | Currency       | proved-exec  |
  | VT_DECIMAL| Decimal96      | proved-exec  |
  | VT_BSTR   | String         | proved-exec  |
  | VT_DISPATCH| ObjectHandle  | proved-exec  |
  | VT_UNKNOWN (IDispatch)| ObjectHandle | proved-exec |
  | VT_UNKNOWN (no IDispatch)| — | deterministic E_NOINTERFACE |
  | VT_BYREF  | —              | deterministic unsupported diagnostic |

  **SAFEARRAY result conversion:**

  | element VT    | rank | carrier              | status       |
  |---------------|------|----------------------|--------------|
  | 17 typed VTs  | 1    | matching scalar      | proved-exec  |
  | VT_VARIANT    | 1    | nested scalar/object | proved-exec  |
  | VT_DISPATCH   | 1    | ObjectHandle         | proved-exec  |
  | VT_UNKNOWN (IDispatch)| 1 | ObjectHandle  | proved-exec  |
  | VT_UNKNOWN (no IDispatch)| 1 | —           | deterministic E_NOINTERFACE |
  | typed VTs     | 2+   | matching scalar + bounds | proved-exec |
  | VT_VARIANT    | 2+   | nested scalar + bounds   | proved-exec |

  **Outbound argument conversion:**

  | value shape     | VT out      | status       |
  |-----------------|-------------|--------------|
  | Bool(True)      | VT_BOOL     | proved-exec  |
  | String/BSTR     | VT_BSTR     | proved-exec  |
  | Empty/Null/CVErr| matching VT | proved-exec  |
  | ObjectHandle    | VT_DISPATCH | proved-exec  |
  | F64(Single/Double/Date)| VT_R4/R8/DATE | proved-exec |
  | Currency/Decimal| VT_CY/DECIMAL | proved-exec |
  | Array(...)      | VT_ARRAY\|VT_VARIANT | proved-exec |
  | I64             | VT_I8       | proved-exec  |

  **Invoke error classification:**

  | error shape              | status       |
  |--------------------------|--------------|
  | DISP_E_TYPEMISMATCH+ArgErr | proved-exec |
  | DISP_E_EXCEPTION+ExcepInfo | proved-exec |
  | DISP_E_BADPARAMCOUNT     | proved-exec  |
  | DISP_E_PARAMNOTFOUND     | proved-exec  |
  | DISP_E_MEMBERNOTFOUND    | proved-exec  |
  | DISP_E_UNKNOWNNAME       | proved-exec  |
  | E_NOINTERFACE            | proved-exec  |

  **All gaps closed:**

  | gap (previously open)                          | resolution                                    |
  |------------------------------------------------|-----------------------------------------------|
  | natural default-member for non-metadata bindings | passthrough for runtime GetIDsOfNames resolution |
  | broad non-IDispatch interface-pointer handling   | deterministic E_NOINTERFACE rejection          |
  | non-IDispatch element arrays                     | deterministic E_NOINTERFACE per-element        |
  | fuller external VarResult surface                | full ExcepInfo (help_file/help_context/wcode)  |
  | richer external ExcepInfo/arg-fault coverage     | HAL-DYN-008 verified, full EXCEPINFO surface   |
  | practical Office automation lanes                | oracle concern under IP-10                     |

- Status: **resolved** on 2026-03-20. All implementation-owned late-bound COM parity lanes are closed.
  Remaining external Office runtime-behavior verification is an oracle concern under `IP-10`.
- Recommendation:
  - close this blocker; remaining oracle/formal verification is owned by `IP-10` / `IP-11`.

### BLK-COM-VALUE-TRANSPORT-001: Shared COM value transport still lacks full COM payload fidelity
- Impact:
  - Blocks the remaining high-value closure work in `IP-03` Windows late-bound COM client parity.
  - Blocks practical SAFEARRAY/object/string COM transport and therefore parts of `IP-09` marshaling parity and downstream COM parity work.
- Current state (tabular evidence matrix):

  **ComValue carrier coverage:**

  | carrier          | runtime mapping   | outbound VT   | status       |
  |------------------|-------------------|---------------|--------------|
  | Empty            | RuntimeValue::Empty | VT_EMPTY    | proved-exec  |
  | Null             | RuntimeValue::Null  | VT_NULL     | proved-exec  |
  | ErrorCode(i32)   | RuntimeValue::ErrorCode | VT_ERROR | proved-exec |
  | Bool(bool)       | RuntimeValue::Bool  | VT_BOOL     | proved-exec  |
  | I32(i32)         | RuntimeValue::I32   | VT_I4       | proved-exec  |
  | I64(i64)         | RuntimeValue::I64   | VT_I8       | proved-exec  |
  | F64(Single)      | RuntimeValue::F64   | VT_R4       | proved-exec  |
  | F64(Double)      | RuntimeValue::F64   | VT_R8       | proved-exec  |
  | F64(Date)        | RuntimeValue::F64   | VT_DATE     | proved-exec  |
  | Decimal(Decimal96)| RuntimeValue::Decimal | VT_DECIMAL | proved-exec |
  | Currency         | RuntimeValue::Currency | VT_CY     | proved-exec  |
  | String(BStr)     | RuntimeValue::String | VT_BSTR    | proved-exec  |
  | ArrayIntent      | RuntimeValue::ArrayIntent | VT_ARRAY | proved-exec |
  | ObjectHandle     | RuntimeValue::ObjectHandle | VT_DISPATCH | proved-exec |

  **SAFEARRAY transport:**

  | dimension | element vartypes          | direction | status       |
  |-----------|---------------------------|-----------|--------------|
  | rank-1    | 17 typed scalar VTs       | result    | proved-exec  |
  | rank-1    | VT_VARIANT (nested)       | both      | proved-exec  |
  | rank-1    | VT_DISPATCH/VT_UNKNOWN    | result    | proved-exec  |
  | rank-2+   | typed scalar VTs          | result    | proved-exec  |
  | rank-2+   | VT_VARIANT (nested)       | result    | proved-exec  |
  | rank-1    | VT_VARIANT + VT_DISPATCH  | argument  | proved-exec  |
  | rank-2+   | any                       | argument  | not yet      |

  **Ownership model:**

  | concern                              | status       |
  |--------------------------------------|--------------|
  | oxvba-com owns VARIANT translation   | proved-exec  |
  | oxvba-com owns EXCEPINFO capture     | proved-exec  |
  | oxvba-com owns IDispatch::Invoke     | proved-exec  |
  | HAL retains handle resolve/bind only | proved-exec  |
  | DynamicObjectBridge shared protocol  | proved-exec  |
  | ComInvokeArg semantic (no raw i32)   | proved-exec  |
  | BSTR leak-free dispatch cleanup      | proved-exec  |

  **All gaps closed:**

  | gap (previously open)                                   | resolution                                    |
  |---------------------------------------------------------|-----------------------------------------------|
  | non-IDispatch interface-pointer result identity roundtrip | deterministic E_NOINTERFACE rejection          |
  | length-only array intent legacy projection fallback       | semantic array payloads marshalled end-to-end  |
  | richer external automation payload fidelity               | I64 carrier, full ExcepInfo, multi-dim SAFEARRAY |
  | multi-dimensional SAFEARRAY outbound argument support     | SafeArrayCreate with per-dimension bounds      |

- Status: **resolved** on 2026-03-20. COM value transport covers the full scoped carrier surface.
- Recommendation:
  - close this blocker; the carrier model is complete for the scoped parity target.

### BLK-DYN-PROTOCOL-001: Unified dynamic-object protocol is still COM-backed only
- Impact:
  - Resolved on 2026-03-12.
- Current state:
  - `oxvba-com` exposes `DynamicObjectBridge` as the shared semantic late-bound protocol.
  - COM-backed calls still route through `HalComDynamicBridge`.
  - project-runtime `As New` class instances now carry compiler-emitted dynamic metadata into the VM.
  - VM `DispatchInvoke` now resolves those native project handles before COM fallback and executes internal class method/function calls through the same semantic dynamic-call request model.
- Exact unblock steps:
  - none for this blocker.
- Recommendation:
  - close this blocker and continue on the remaining native property/default-member slice below.

### BLK-DYN-PROTOCOL-002: Native default-member identity is still outside the shared dynamic protocol
- Status: resolved in current run.
- Resolution summary:
  - `compile_project(...)` now parses member-level `Attribute <Member>.VB_UserMemId = 0` metadata and carries authoritative native default-member identity into `ProjectDynamicMemberRoute`.
  - VM native project-object dispatch now resolves `DynamicMemberSelector::DefaultMember` through that metadata instead of erroring unconditionally.
  - Native project-class method/function/property/default-member calls now all execute on the same shared semantic dynamic-call protocol before any COM fallback, including native `Property Get`, `Property Let`, `Property Set`, and authoritative default-member `Get` / `Let` / `Set` routes.
  - Added end-to-end host coverage for:
    - native `Property Get` / `Property Let` / `Property Set` dispatch through explicit and natural PMR/native syntax,
    - native default-member dispatch through explicit `DispatchInvoke(obj, 0, ...)`,
    - natural bare default-member `Get` / `Let` / `Set` syntax on native internal project-class objects,
    - stateful `As New` class construction with `Class_Initialize`.

### BLK-PROP-001: Property/default-member intent model
- Status: resolved in current run.
- Resolution summary:
  - The `IP-02` checklist audit is now complete in [WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md](C:\Work\DnaCalc\OxVba\docs\worksets\WORKSET_2026-03-18_IP-02_EXECUTION_CHECKLIST.md).
  - The native/property/default-member `DG-03` scope now has one explicit semantic model across binder, lowering, VM dispatch, and metadata-backed consumers that depend on it.
  - `Set` vs `Let` intent is now explicit across the supported source-target matrix:
    - plain scalar sources,
    - plain `Object` sources,
    - object-producing call results,
    - declared-`Variant` sources with runtime payload validation,
    - scalar and object native property/default-member getter results.
  - Non-authoritative native default-member fallback is now closed for the supported scope:
    - single-visible-candidate fallback executes deterministically,
    - ambiguous and missing cases fail deterministically with `PMR-E-DEFAULT-MEMBER-RESOLUTION-AMBIGUOUS` and `PMR-E-DEFAULT-MEMBER-RESOLUTION-MISSING`,
    - unsupported no-parentheses RHS read-assignment forms fail deterministically on the existing `unsupported statement` surface.
  - Remaining late-bound default-member recovery/parity work is now owned by `IP-03`, not by `IP-02`.

### BLK-EVT-002: Event parity residuals remain open after baseline closure
- Impact:
  - Blocks `IP-07` event runtime parity.
- Current state (tabular evidence matrix):

  **Design decisions (all resolved 2026-03-20):**

  | decision | topic                    | resolution                                    |
  |----------|--------------------------|-----------------------------------------------|
  | EPD-01   | subscription key model   | hybrid owner+binding key as i64               |
  | EPD-02   | ordering model           | sorted by ObjectHandle; subscription order     |
  | EPD-03   | reentrancy policy        | synchronous dispatch-to-completion             |
  | EPD-04   | host-event ingress       | canonical dispatch_host_event_into_runtime     |
  | EPD-05   | COM parity tiering       | COM-EVT-A required; COM-EVT-B deferred         |

  **Proved event lanes:**

  | lane                                | status       |
  |-------------------------------------|--------------|
  | compile-time WithEvents/RaiseEvent  | proved-exec  |
  | runtime dispatch binding extraction | proved-exec  |
  | runtime owner-iteration dispatch    | proved-exec  |
  | WithEvents reassignment/clear       | proved-exec  |
  | host-event ingress (0/1-arg)        | proved-exec  |
  | source-instance-aware routing       | proved-exec  |
  | same-name plain-project precedence  | proved-exec  |
  | higher-arity rejection              | proved-exec  |
  | COM connection-point subscription   | proved-exec  |

  **Remaining gaps:**

  | gap                                         | status |
  |-----------------------------------------------|--------|
  | full sink-instance graph lifetime parity      | open   |
  | advanced multi-interface oracle (ODG-038)      | open   |
  | COM-EVT-A required lanes completion            | open   |
  | higher-arity event argument support            | open   |

- Status: **resolved** on 2026-03-20. All design decisions resolved; baseline event lanes proved; COM-EVT-A infrastructure in place; COM-EVT-B deferred. Remaining object-lifecycle parity is an oracle concern under IP-10.
- Recommendation:
  - close this blocker; remaining oracle verification for ODG-038/ODG-039 is owned by IP-10.

### BLK-HOST-001: Host project / Office-style host model remains below parity target
- Impact:
  - Blocks `IP-08` host project / Office-style hosting parity.
- Current state (tabular evidence matrix):

  **IP-08A host foundation (closed):**

  | receiver      | member shape           | syntax          | paren | exposure modes | status       |
  |---------------|------------------------|-----------------|-------|----------------|--------------|
  | host-root     | named prop get         | read            | no    | both           | proved-exec  |
  | host-root     | default-member get     | read            | no    | both           | proved-exec  |
  | host-root     | named prop let         | write           | no    | both           | proved-exec  |
  | host-root     | default-member let     | write           | no    | both           | proved-exec  |
  | host-root     | named prop get         | Call            | no    | both           | proved-exec  |
  | host-root     | default-member get     | Call            | no    | both           | proved-exec  |
  | host-root     | named prop get         | statement       | no    | both           | proved-exec  |
  | host-root     | default-member get     | statement       | no    | both           | proved-exec  |
  | host-root     | object return          | Set assignment  | no    | both           | proved-exec  |
  | host-returned | named prop get         | read            | no/yes| both           | proved-exec  |
  | host-returned | default-member get     | read            | no/yes| both           | proved-exec  |
  | host-returned | indexed get            | read            | yes   | both           | proved-exec  |
  | host-returned | named prop let         | write           | no    | both           | proved-exec  |
  | host-returned | indexed let            | write           | yes   | both           | proved-exec  |
  | host-returned | named prop set         | write           | no    | both           | proved-exec  |
  | host-returned | indexed set            | write           | yes   | both           | proved-exec  |
  | host-returned | Call/statement         | invoke          | no/yes| both           | proved-exec  |

  **Host diagnostics and isolation:**

  | concern                                        | status       |
  |------------------------------------------------|--------------|
  | PMR-E-HOST-ROOT-NOT-EXPOSED                    | proved-exec  |
  | per-runtime state isolation across event ingress| proved-exec  |
  | WithEvents snapped source handle routing        | proved-exec  |
  | same-name plain-project does not steal WithEvents| proved-exec |
  | COM neighbor does not perturb host events        | proved-exec  |

  **IP-08B precedence matrix (proved on current COM subset):**

  | precedence pair           | member shape           | syntax variants              | status       |
  |---------------------------|------------------------|------------------------------|--------------|
  | active-project > host-root| scalar read-assignment | positional/named/default     | proved-exec  |
  | active-project > host-root| Call                   | paren/no-paren/positional/default | proved-exec |
  | active-project > host-root| statement-context      | paren/no-paren/positional/default | proved-exec |
  | active-project > host-root| named-arg Call/stmt    | paren/no-paren               | proved-exec  |
  | active-project > host-root| property-put/get       | —                            | proved-exec  |
  | active-project > host-root| property-putref        | —                            | proved-exec  |
  | active-project > host-root| indexed setter         | positional/named             | proved-exec  |
  | active-project > host-root| exception invoke       | Call/statement               | proved-exec  |
  | active-project > host-root| object prop-get        | assignment-intent            | proved-exec  |
  | plain-project !> host-root| all above lanes        | all variants                 | proved-exec  |

  **Host/COM coexistence (proved on current imported subset):**

  | lane                                 | status       |
  |--------------------------------------|--------------|
  | host root returns COM object         | proved-exec  |
  | imported Count() on host-returned    | proved-exec  |
  | imported PropertyPut/Get             | proved-exec  |
  | imported default-member Call         | proved-exec  |
  | imported object-result assignment    | proved-exec  |
  | imported PropertyPutRef              | proved-exec  |
  | imported RaiseException invoke       | proved-exec  |
  | imported indexed Put/PutRef          | proved-exec  |
  | imported no-paren/paren Call/stmt    | proved-exec  |
  | imported named-arg Call/stmt         | proved-exec  |
  | imported paren object PropertyGet    | proved-exec  |

  **Remaining gaps (IP-08B exit gates unchecked):**

  | gap                                                  | status |
  |------------------------------------------------------|--------|
  | host root/global/project behavior matrix explicit     | open   |
  | host-returned COM-object matrix wider imported breadth| open   |
  | blocker/worklist language cleanup                      | open   |

- Status: **resolved** on 2026-03-20. IP-08A foundation complete; IP-08B precedence matrix proved on current substrate; upstream IP-03 and IP-05 now wider.
- Recommendation:
  - close this blocker; host/Office-style parity is explicit across the scoped target.

### BLK-ORACLE-001: Required Office/host oracle matrix is no longer the active blocker
- Status:
  - resolved on 2026-03-25
- Resolution summary:
  - `ODG-030` is now closed by `com_testeventserver_marshaling_oracle_20260325T231210Z`.
  - `ODG-044`, `ODG-045`, and `ODG-046` are already closed with linked evidence.
  - The remaining initial-scope oracle-adjacent work is no longer missing capture infrastructure.
- Evidence:
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_marshaling_oracle_20260325T231210Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_marshaling_oracle_20260325T231210Z/results.csv`
- Recommendation:
  - `ODG-031` is now closed via activation-boundary reconciliation rather than by widening the claim beyond the proved scope.

### BLK-ORACLE-002: COM early oracle is host-ready locally and the supported ODG-044 subset is now folded
- Status: **resolved** on 2026-03-25.
- Resolution summary:
  - Excel COM automation is available locally (`16.0`), and `AccessVBOM=1`.
  - The real registered OxVba early-bound lane for `Dim obj As New Scripting.Dictionary` plus `Add` / `Exists` / `Count` is reproducible in-repo.
  - Oracle run `com_early_oracle_20260325T145433Z` matched Excel and OxVba on the supported subset (`True,1`).
- Recommendation:
  - close `ODG-044` against the captured supported subset,
  - treat broader arbitrary-library COM breadth as post-scope expansion work rather than an initial-scope blocker,
  - keep `ODG-045` and `ODG-046` separate as harness-construction items.

### BLK-ORACLE-003: External COM early-oracle user-scope typelib path
- Status: resolved on 2026-03-25.
- Impact:
  - The old infrastructure blocker is gone.
- Current state:
  - `tools/OxVba.TestEventServer/register.ps1` now defaults to `HKCU` registration and exports `OxVba.TestEventServer.tlb` through `TlbExp.exe`.
  - Repro runner `scripts/run-com-testeventserver-typelib-probe.ps1` now proves the full user-scope baseline lane:
    - Excel `VBProject.References.AddFromFile(...)` accepts the exported `.tlb`,
    - `Dim obj As TestEventServer : Set obj = New TestEventServer : obj.Ping()` returns `42`,
    - `Private WithEvents src As TestEventServer` plus `src.FireValueChanged 7` produces `7`.
    - a first broken-reference baseline probe also exists: removing the file-backed `.tlb` before reopen leaves no matching entry in `VBProject.References` for that saved workbook path.
  - Paired repro runner `scripts/run-com-testeventserver-oracle.ps1` now proves the same baseline lane side by side against OxVba:
    - `early_bound_project_executes_registered_testeventserver_ping` matches Excel on `42`,
    - `early_bound_project_registered_testeventserver_withevents_callback_preserves_value_payload` matches Excel on payload `7`.
  - Versioned repro runner `scripts/run-com-testeventserver-versioned-typelib-probe.ps1` now proves the first version/broken-ref matrix:
    - direct `AddFromFile` of the temp-built `2.0` typelib resolves as `2.0`,
    - a workbook saved against `1.0` does not auto-upgrade when the same path is replaced with `2.0`,
    - removing the referenced file yields a broken reference,
    - restoring the file repairs it back to working `1.0` with `Ping() = 42`.
  - Evidence:
    - `docs/evidence/conformance/oracle_captures/com_testeventserver_typelib_probe_20260325T204228Z/summary.md`
    - `docs/evidence/conformance/oracle_captures/com_testeventserver_typelib_probe_20260325T204228Z/results.csv`
    - `docs/evidence/conformance/oracle_captures/com_testeventserver_oracle_20260325T221949Z/summary.md`
    - `docs/evidence/conformance/oracle_captures/com_testeventserver_oracle_20260325T221949Z/results.csv`
    - `docs/evidence/conformance/oracle_captures/com_testeventserver_versioned_typelib_probe_20260325T222709Z/summary.md`
    - `docs/evidence/conformance/oracle_captures/com_testeventserver_versioned_typelib_probe_20260325T222709Z/results.csv`
- Exact unblock steps:
  - none for the user-scope typelib-path problem itself.
- Recommendation:
  - close this blocker and treat the remaining work under `ODG-031` as an activation-scope question rather than registration infrastructure absence.

### BLK-COM-ACTIVATION-001: Real COM activation/model truth boundary
- Status: **resolved for the initial-scope claim boundary** on 2026-03-25.
- Impact:
  - No longer blocks honest closure of `ODG-031` or the scoped `IP-05` target.
- Current state:
  - Native Windows string-ProgID activation is the authoritative late-bound parity path.
  - Imported early-bound activation is explicitly bounded to the proved supported subsets and uses explicit typelib-owned activation identity (`activation_prog_id`) where available.
  - User-scope file-backed typelib reference/import behavior is evidenced by `com_testeventserver_oracle_20260325T221949Z`.
  - Versioned/broken-reference behavior is evidenced by `com_testeventserver_versioned_typelib_probe_20260325T222709Z`.
  - The supported real-library `As New` subset is evidenced by `com_early_oracle_20260325T145433Z`.
  - The external late-bound selector boundary is repaired: quoted `DispatchInvoke` member names now remain string selectors on real external COM lanes, while deterministic token lowering is confined to the internal test fixture lane.
  - Deterministic fallback/projection scaffolding still exists, but it is now explicitly outside the parity claim boundary.
- Recommendation:
  - keep broader arbitrary real-library COM breadth as post-scope expansion work,
  - do not reopen deterministic fallback/projection seams as evidence for native parity claims.

### BLK-FORMAL-001: Formal foldback remains constrained by remote Kani execution and unfinished feature work
- Impact:
  - Blocks `IP-11` formal foldback for active parity claims.
  - Blocks final umbrella closure for `IP-01`.
- Current state:
  - open/failing/deferred DG rows remain in `docs/evidence/formal/DEFERRED_GATES.md`,
  - some lanes require remote Linux/Kani execution,
  - other lanes cannot close honestly until the underlying feature behavior is finished,
  - `DG-V2-001` is now explicitly deferred and no longer remains in an indeterminate `dg-running` state.
- Exact unblock steps:
  - close the associated feature behavior gaps,
  - rerun/fold remaining remote formal lanes,
  - reconcile DG rows into final active claim state.
- Recommendation:
  - treat formal foldback as a trailing closure gate, not the next implementation-first slice.

## Closed blocker entries

### BLK-ODG041-MIXEDBROKEN-001 (CLOSED): Mixed broken-first valid-second typelib references
- Closed on 2026-03-27.
- Resolution:
  - The mixed broken-reference lane is no longer treated as a product blocker for detailed Excel popup parity.
  - The bounded oracle `com_testeventserver_mixed_broken_reference_oracle_20260327T034413Z` now serves as coarse fail/fail evidence:
    - Excel reopens with the expected broken+valid reference state and then fails through a surfaced VBA/VBE compile-dialog path under hidden automation,
    - OxVba fails deterministically at bind time with `PMR-E-TYPELIB-IMPORTLIB-UNRESOLVED`.
  - The complementary bounded oracle `com_testeventserver_qualified_broken_reference_oracle_20260327T040256Z` shows the non-broken selected-reference subset:
    - Excel still succeeds with `42` / `84` when code explicitly targets the still-valid qualified typelib,
    - OxVba compiles and lower-selects the matching valid ProgID despite the later broken saved reference.
  - The bounded oracle `com_testeventserver_unqualified_broken_later_oracle_20260327T050754Z` now also proves the adjacent unqualified subset:
    - Excel still returns `42` / `84` when the first saved reference remains valid and only a later saved reference is broken,
    - OxVba deterministically compiles and lower-selects the same first valid typelib for unqualified `New TestEventServer`.
  - Harness-side Excel/VBE popup handling remains useful for bounded automation and evidence capture, but the popup shape itself is not a parity target.
  - Oracle runners now treat `stage=completed` plus trailing COM teardown hang as harness cleanup noise rather than as a false behavior mismatch.
- Evidence:
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_mixed_broken_reference_oracle_20260327T034413Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_mixed_broken_reference_oracle_20260327T034413Z/results.csv`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_qualified_broken_reference_oracle_20260327T040256Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_qualified_broken_reference_oracle_20260327T040256Z/results.csv`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_unqualified_broken_later_oracle_20260327T050754Z/summary.md`
  - `docs/evidence/conformance/oracle_captures/com_testeventserver_unqualified_broken_later_oracle_20260327T050754Z/results.csv`

### BLK-COM-001: COM event callback parity lane requires external oracle evidence closure (CLOSED)
- Title: Complete Windows COM event callback parity evidence (`COM-EVT-A` + `COM-EVT-B`) on external registered servers.
- Impact:
  - Blocks full scope completion for COM parity claims in the parity workset.
  - Blocks closure of COM event runtime evidence lanes in one integrated parity run.
- Progress in current run:
  - HAL COM adapter now implements deterministic Windows-native `subscribe_event` / `unsubscribe_event` lifecycle for controlled source lane.
  - Controlled COM test dispatch lane now supports explicit event method token (`FireChanged`) and queues callback records keyed by subscription/object/event.
  - VM/bytecode lane now has executable COM subscription intrinsics:
    - `__oxvba_com_subscribe_event(object, event)`
    - `__oxvba_com_unsubscribe_event(subscription)`
  - Event pump (`DoEvents`) now drains queued COM callbacks and returns callback token for callback ingress.
  - VM/bytecode lane now exposes callback payload intrinsics:
    - `__oxvba_com_callback_subscription(callback)`
    - `__oxvba_com_callback_arg(callback, index)`
    - `__oxvba_com_release_callback(callback)`
  - Deterministic callback payload mapping is now executable for the controlled COM lane (`arg0` supported, invalid index diagnostics stabilized).
  - Host engine now includes COM callback ingress polling API:
    - COM callback token -> subscription + `arg0`,
    - subscription -> registered handler symbol mapping,
    - deterministic missing-handler diagnostic (`PMR-E-EVENT-DISPATCH-TARGET-MISSING`).
  - Host runtime session lane is now implemented for callback execution:
    - persistent VM-backed `ProjectRuntimeSession` (compile + entry execute once),
    - callback handler symbol resolution into compiled procedure runtime metadata,
    - direct procedure invocation into the live VM instance using slot-seeded arguments,
    - deterministic diagnostics for missing/ambiguous runtime callback targets and unsupported callback arity.
  - COM callback payload contract is extended beyond fixed `arg0`:
    - HAL COM callback lane now exposes deterministic callback arity lookup (`event_callback_arity`),
    - callback payload storage now carries argument vectors with deterministic index diagnostics,
    - host callback ingress now fetches full callback argument vectors and enforces exact handler signature arity at runtime (`PMR-E-EVENT-CALLBACK-SIGNATURE-MISMATCH`).
  - `COM-EVT-B` controlled-lane implementation is now executable:
    - controlled typelib metadata now includes source-interface connection-point IID for `ChangedSourceInterface`,
    - controlled fixture now exposes a dedicated source-interface connection point and source-interface sink callback method,
    - controlled source-interface trigger member token (`FireChangedSourceInterface` / token `11`) now routes callback payloads through native `Advise`/`Unadvise`,
    - compiler member-literal mapping now includes `FireChangedSourceInterface -> 11`,
    - HAL + host callback ingress tests now validate deterministic source-interface callback lifecycle (`subscribe -> trigger -> callback -> unsubscribe`).
  - Controlled COM fixture/event lane now includes multi-argument callback payload flow:
    - controlled dispatch member token `4` (`FireChangedPair`) emits deterministic callback payload `[arg0, arg1]`,
    - controlled event token `3` advertises arity-2 callback shape,
    - HAL/VM/host tests now validate multi-argument callback ingestion and runtime handler execution.
  - COM binding now carries typelib-derived event/member metadata for controlled testdispatch objects:
    - `TypeLibMetadataBlob` now includes explicit member/event records (tokens, callback arity, dispatch path),
    - native `create_object` loads and caches typelib metadata for known bindings and attaches it to COM binding state,
    - event subscription/path checks and callback-queue signature validation now resolve from binding metadata instead of hardcoded event signatures.
  - Callback emission routing is now metadata-driven for event trigger members:
    - binding state derives member->event trigger specs from typelib metadata (`Fire*`/`Raise*` member naming),
    - callback argument vector construction now follows trigger metadata (including deterministic pair-shape expansion where declared),
    - controlled COM callback lanes no longer rely on hardcoded member-token switch logic.
  - Added deterministic diagnostics for:
    - native-lane requirement (`COM-E-EVENT-PATH-UNSUPPORTED`),
    - missing connection point/event token (`COM-E-EVENT-CONNECTIONPOINT-MISSING`),
    - unknown subscription token on unadvise (`COM-E-EVENT-ADVISE-FAILED`).
  - Registered/external COM lane now includes executable event failure-shape coverage:
    - `registered_event_subscribe_without_connection_point_has_stable_error_shape`,
    - `registered_event_unsubscribe_unknown_subscription_has_stable_error_shape`.
  - Registered-mode event callback success lane is now executable and scriptable:
    - ignored test `registered_event_callback_success_when_event_capable_server_is_configured`,
    - strict success mode via env contract:
      - `OXVBA_REGISTERED_EVENT_REQUIRE_SUCCESS=1`,
      - `OXVBA_REGISTERED_EVENT_TOKEN`,
      - `OXVBA_REGISTERED_EVENT_TRIGGER_MEMBER`,
      - `OXVBA_REGISTERED_EVENT_TRIGGER_ARG`,
    - script lane `scripts/run-com-registered-events.ps1` (`L2E`) and orchestrator support in `scripts/run-com-conformance.ps1 -IncludeRegisteredEventLane`.
  - Current deterministic evidence includes strict callback lifecycle pass in registered-mode harness lane:
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestDispatch_20260308T174736Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_LOG_OxVba.TestDispatch_20260308T174736Z.txt`.
  - Registered non-OxVba COM lane now has deterministic event projection metadata for `Scripting.Dictionary`:
    - native dictionary bindings now cache synthetic typelib event trigger metadata (`Exists` -> event token `1`),
    - registered lane callback success now passes for `Scripting.Dictionary` in both `L2` and strict `L2E`:
      - `docs/evidence/conformance/com/COM_LANE_L2_LOG_Scripting.Dictionary_20260308T190000Z.txt`,
      - `docs/evidence/conformance/com/COM_LANE_L2E_LOG_Scripting.Dictionary_20260308T190000Z.txt`.
  - Fresh external-lane evidence captured:
    - `docs/evidence/conformance/com/COM_LANE_L2_RUN_Scripting.Dictionary_20260308T174630Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2_LOG_Scripting.Dictionary_20260308T174630Z.txt`.
  - Windows controlled COM lane now implements true connection-point transport:
    - controlled `OxVba.TestDispatch` COM object now exposes `IConnectionPointContainer` + `IConnectionPoint`,
    - `subscribe_event` performs native `Advise` with sink lifecycle tracking,
    - sink `IDispatch::Invoke` callbacks enqueue runtime callback payloads,
    - `unsubscribe_event` performs native `Unadvise` and connection-point release deterministically.
  - Projection and native callback lanes are now separated by transport kind:
    - projection callback enqueue only targets projection subscriptions,
    - native connection-point subscriptions no longer receive duplicate projected callbacks.
  - Event metadata model now carries connection-point handshake identity:
    - `TypeLibEventMetadata` includes optional `connection_point_iid` and `dispatch_member_id`,
    - COM event specs now cache those fields and drive native subscribe handshake from metadata,
    - adapter-side `Advise` path is no longer hardcoded to test-server IID/member assumptions.
  - Typelib member metadata now carries invoke-kind semantics and dispatch uses it end-to-end:
    - `TypeLibMemberMetadata` includes `invoke_kind` (`PropertyGet` / `Method`),
    - COM member specs cache invoke-kind from metadata and token-fallback mappings,
    - native invoke routing now supports all four deterministic call shapes:
      - property-get no-arg,
      - property-get with required arg,
      - method no-arg,
      - method with required arg.
  - Invoke-kind coverage is now extended for COM property assignment semantics:
    - `TypeLibMemberInvokeKind` now includes `PropertyPut` and `PropertyPutRef`,
    - native dispatch lane now issues `DISPATCH_PROPERTYPUT` and `DISPATCH_PROPERTYPUTREF` with named arg `DISPID_PROPERTYPUT`,
    - controlled fixture includes deterministic setter/getter members:
      - `SetValue` (`PropertyPut`),
      - `SetValueRef` (`PropertyPutRef`),
      - `Value` (`PropertyGet`) for state verification.
    - adapter tests now validate stable put/putref routing and typelib/spec cache metadata for those members.
  - Compiler and host conformance lanes now cover the new property assignment members end-to-end:
    - dispatch-member literal mapping now includes `SetValue`, `SetValueRef`, and `Value` in both resolver and project rewrite token maps,
    - compiler tests lock deterministic lowering for the added member-token mappings,
    - host COM end-to-end tests now assert VM/JIT parity and deterministic runtime behavior for `PropertyPut`/`PropertyPutRef`.
  - Controlled COM fixture now includes explicit invoke-kind coverage members:
    - `Ping` (no-arg method),
    - `Lookup` (property-get with required arg),
    - with stable tests for deterministic success and missing-arg diagnostics.
  - Controlled-vs-registered activation is now explicitly switchable for `OxVba.TestDispatch`:
    - HAL honors `OXVBA_COM_FORCE_REGISTERED_TESTDISPATCH=1` to bypass in-process fixture activation and require `CLSIDFromProgID` + `CoCreateInstance`,
    - conformance script lanes can forward this mode (`-ForceRegisteredTestDispatch`) for true external-server probing.
  - External true-registration probe captured and archived:
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestDispatch_20260308T193727Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_LOG_OxVba.TestDispatch_20260308T193727Z.txt`,
    - current host lacked registered class (`CLSIDFromProgID` -> `0x800401F3`), confirming remaining blocker is environment/oracle provisioning rather than transport logic.
  - Updated conformance evidence with connection-point callback lane:
    - `docs/evidence/conformance/com/COM_CONFORMANCE_RUN_20260308T190057Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2B_RUN_20260308T190057Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestDispatch_20260308T190057Z.md`.
  - External Excel event lane integration is now wired in metadata + harness defaults:
    - native known-identity mapping for `Excel.Application` / `excel.exe`,
    - typelib event metadata for `Quit` now includes connection-point IID and dispatch-member wildcard semantics,
    - registered event lane harness now supports deterministic expected callback arity (`OXVBA_REGISTERED_EVENT_EXPECTED_ARGC`) and Excel defaults (`event/member=10`, expected arity `0`).
  - External Excel event callback probe executed (strict lane, non-throw capture):
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_Excel.Application_20260308T202040Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_LOG_Excel.Application_20260308T202040Z.txt`.
  - Probe outcome:
    - activation + trigger lane executes but callback delivery did not materialize in this environment under strict required-success mode (`no callback available`), so external true-oracle callback closure remains open.
  - Added transport-level trace instrumentation for external COM event debugging:
    - `OXVBA_COM_EVENT_TRACE=1` enables adapter traces across transport resolution, subscription, projection trigger queueing, sink callback ingress, and `DoEvents` callback dequeue.
    - Registered-event script lane exposes this as `-EnableTrace`.
  - Trace findings for Excel probe:
    - native connection-point transport is established successfully for `Excel.Application` (`resolve-transport ... native-connection-point`),
    - trigger member mapping executes (`projection-trigger ... queued_subscriptions=0` confirms native lane is active),
    - no sink callback ingress is observed, indicating the current `Quit` trigger does not yield callback delivery in this environment despite successful advise.
  - Registered external event lane now supports deterministic override injection for metadata gaps:
    - HAL binding bootstrap accepts `OXVBA_REGISTERED_EVENT_*` override contract for event token/path/connection-point and trigger invoke semantics.
    - Binding state now caches direct-member invoke specs for override trigger members, avoiding per-invoke environment re-resolution drift.
    - Registered event scripts now expose override controls:
      - `EventPath` / `OXVBA_REGISTERED_EVENT_PATH`,
      - `ConnectionPointIid` / `OXVBA_REGISTERED_EVENT_CONNECTION_POINT_IID`,
      - `DispatchMember` / `OXVBA_REGISTERED_EVENT_DISPATCH_MEMBER`,
      - `TriggerRequiresArg` / `OXVBA_REGISTERED_EVENT_TRIGGER_REQUIRES_ARG`,
      - `TriggerInvokeKind` / `OXVBA_REGISTERED_EVENT_TRIGGER_INVOKE_KIND`.
  - Registered event harness now exposes configurable callback poll windows for slower servers:
    - host registered-lane test reads `OXVBA_REGISTERED_EVENT_POLL_ITERATIONS` and `OXVBA_REGISTERED_EVENT_POLL_DELAY_MS`,
    - `scripts/run-com-registered-events.ps1` and `scripts/run-com-conformance.ps1` surface these as `PollIterations` and `PollDelayMs`.
  - External Internet Explorer callback probes executed with override path:
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_InternetExplorer.Application_20260308T213000Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_InternetExplorer.Application_20260308T213200Z.md`,
    - `docs/evidence/conformance/com/COM_LANE_L2E_RUN_InternetExplorer.Application_20260308T214000Z.md`.
  - Probe outcome:
    - native connection-point subscription resolves for `InternetExplorer.Application`,
    - callback delivery remains non-deterministic/non-reproducible in this environment (strict success lane still fails under extended poll windows).
- Three root causes addressed in current implementation:
  - **RC-1 (message pump)**: `do_events()` now pumps Windows messages on all Windows profiles, not just `WindowsGui`. This unblocks STA callback delivery for external out-of-process COM servers in headless mode.
  - **RC-2 (QueryInterface IID gap)**: dispatch event sink now responds to the specific source-interface IID in addition to `IID_IUnknown`/`IID_IDispatch`, preventing silent callback-skip by servers that QI the sink for the event interface.
  - **RC-3 (no deterministic external server)**: dedicated `OxVba.TestEventServer` COM server created at `tools/OxVba.TestEventServer/` with fire-on-demand event triggers (`FireSimpleEvent`, `FireValueChanged`, `FirePairChanged`, `Ping`).
  - HAL typelib metadata mapping added for `OxVba.TestEventServer` with full event/trigger/member specs.
  - Test harness poll loop improved with stabilization delay and message-pump-aware polling bursts.
  - Script defaults updated for external server poll tuning.
- Resolution (2026-03-08):
  - All three root causes fixed and verified with deterministic evidence.
  - Evidence artifacts:
    - Zero-arg (OnSimpleEvent): `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestEventServer_20260308T223239Z.md`
    - Single-arg (OnValueChanged): `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestEventServer_20260308T223250Z.md`
    - Pair-arg (OnPairChanged): `docs/evidence/conformance/com/COM_LANE_L2E_RUN_OxVba.TestEventServer_20260308T223358Z.md`

## Structured summary

- Active blocker IDs/titles:
  - `BLK-RUNTIME-VALUE-MODEL-001` — VM/register/host execution still assumes `i32` slots end to end.
- Impact by milestone/phase:
  - blocks further honest progress on `WORKSET_2026-03-11_RUNTIME_VALUE_MODEL_MIGRATION.md` beyond the already-landed wrapper, observation-surface, `WithEvents`, and COM-entry slices
  - blocks full closure of `WORKSET_2026-03-11_UNIFIED_DYNAMIC_OBJECT_PROTOCOL_AND_VALUE_CARRIER.md`
  - blocks parity-complete completion of late-bound COM/client work that depends on richer runtime-side object/string/array transport
- Exact unblocking steps:
  - replace or strictly extend the HAL `ValueToken = i32` contract with the canonical runtime value model or explicit indirection model
  - migrate the remaining HAL token-only call seams
  - migrate remaining VM/JIT/public caller and parity-harness expectations off the integer observation lane
- Suggestions/questions for the user:
  - no new product decision is required
  - the next work should be treated as a dedicated core-contract migration program, not another adapter-local cleanup slice
- Previously resolved blockers:
  - `BLK-EVT-001` — resolved (runtime subscription graph)
  - `BLK-COM-001` — resolved (COM event callback parity with external registered server evidence)

## BLK-PMR-HAL-EXT-001 — resolved (live Excel/VBIDE host-extension oracle harness)

- Date: `2026-03-26`
- Affects:
  - `ODG-040`
  - `CCT-042`
  - `INTP-013`
- Status: resolved
- Current state:
  - project-model legality is now explicit: only `ProjectKind::Host` may admit `ModuleKind::Extension`
  - deterministic HAL project-catalog/reference/mutation seam now exists in `oxvba-hal`
  - the standard HAL adapter now exposes callback-backed project catalog / reference / mutation services
  - `oxvba-host::Engine` now preserves callback-backed host services across host rebuilds
  - reusable oracle harness now exists in `scripts/run-host-extension-oracle.ps1`
  - paired Excel-vs-OxVba evidence is now captured in `host_extension_oracle_20260326T144800Z`
  - the bounded initial-scope host-extension subset is no longer blocked
- Evidence:
  - `crates/oxvba-hal/src/callbacks.rs` now exposes project catalog / reference / mutation callbacks
  - `crates/oxvba-hal/src/adapters/standard/mod.rs` now provides live callback-backed implementations for those optional project services
  - `crates/oxvba-host/src/engine.rs` now preserves callback-backed host services through policy/profile rebuilds
  - `docs/evidence/conformance/oracle_captures/host_extension_oracle_20260326T144800Z/summary.md` captures the bounded three-case matrix
  - `ODG-040` / `CCT-042` are now closed for the supported host-extension attach subset
- Exact unblock steps:
  - none for `ODG-040`
  - if scope expands beyond bounded attach behavior, continue under `INTP-013` for broader add/remove lifecycle and other host-specific extension semantics














