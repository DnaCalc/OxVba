# Workset — True COM vtable dispatch for early-bound calls

**Opened:** 2026-06-12
**Branch:** single-package-descriptor-vm
**Tracking task:** #22
**Status:** planning complete → implementing

## Goal

For **dual interfaces** (Excel / DAO / Access objects), dispatch early-bound member
calls through the **COM vtable** directly — the custom-interface slots after the 7
`IUnknown`+`IDispatch` slots — exactly as the VBA IDE does, instead of today's
DispID binding (bind-time dispid baked into `EarlyCom{dispid}`, then dispatched over
`IDispatch::Invoke`). `IDispatch::Invoke` is **retained as the fallback** for
dispinterface-only members, missing/unsupported signatures, and marshalling shapes
not yet covered. Platform: x64 Windows only.

### Why (vs today's DispID binding)

Today the vtable fast-path (`windows_bridge.rs:393-421`) is guarded to the in-process
`OxVba.TestDispatch` fixture and compiles to `Ok(None)` in real builds — every real
early-bound call goes through `IDispatch::Invoke(dispid)`. That is observably
equivalent for normal calls but differs from the IDE on performance, error surfacing
(`IErrorInfo`/HRESULT vs `EXCEPINFO`), and dual-interface edge cases. This workset
closes that fidelity gap.

## Headline findings (de-risk the whole feature)

1. **The dynamic-call mechanism already exists in `oxvba-com`.** `Cargo.toml:18`
   declares `libffi = "5.1.0"`; `windows_ffi_bridge.rs:127` `invoke_stdcall_x64`
   already builds a `libffi::middle` `Cif` from per-arg runtime types and calls a
   runtime function pointer with a typed return (HRESULT/i32/f64/pointer/…). A
   this-call is the same shape + a `this` arg0 + a trailing `[out,retval]` pointer.
2. **The typelib loader already extracts the full FUNCDESC** — `oVft` (vtable slot),
   `invkind`, per-param VARTYPE, `[out,retval]`, return type
   (`windows_typelib_loader.rs:1505-1656` → `TypeLibMemberMetadata`). The data is
   **dropped** when projecting to `ComMemberSpec` (`runtime_state.rs:305-311`). So the
   "missing extraction" is mostly a **plumbing** gap, not an ITypeInfo gap.
3. The existing `raw_oxvba_test_dispatch_vtable_invoke` is a **behavioral oracle**
   (re-implements members in Rust), **not** an ABI template — S2 must add a real
   custom dual vtable fixture to exercise libffi.

## Decision — call mechanism: **reuse the libffi mechanism with a COM-typed marshaller**

A vtable call: `fnptr = (*(*this))[slot]; hr = fnptr(this, arg1, …, retval_ptr)`,
HRESULT in EAX. Options weighed:

- **(A) Reuse the Declare *marshaller* (`dynlink.rs`).** Rejected: it speaks C-ABI
  types (ANSI strings, ByVal scalars, LongPtr pins), the wrong vocabulary for COM
  (BSTR/VARIANT/IDispatch*/HRESULT+out-pointer). Bending it is worse than a new
  marshaller.
- **(B) Reuse the libffi *mechanism* + a new COM marshaller.** ✅ **Chosen.** Factor
  the `Cif`-construction core out of `invoke_stdcall_x64`; new `oxvba-com`
  `vtable_invoke` marshals Variant↔typed-param per the table below; one ABI engine
  (libffi) for both Declare and vtable. No new dependency.
- **(C) Hand-rolled x64 this-call thunk.** Rejected: the codebase already concluded
  the hand-rolled path (`invoke_stdcall_raw`, ≤6 int args, no floats, non-x86_64-only)
  is insufficient and uses libffi on x64. Reintroducing a thunk regresses that.

## Signature enrichment (S1)

Add to `ComMemberSpec` (`runtime_state.rs:305`) and populate in
`member_specs_from_typelib_metadata` (`:196`):
`vtable_slot: Option<u16>`, `parameter_types: Vec<TypeLibParamType>`,
`return_type: Option<TypeLibParamType>`, `callconv_is_stdcall: bool`.
Surface `callconv` (assert `CC_STDCALL == 4`), `funckind`, and `TYPEFLAG_FDUAL` from
`extract_members_from_typeinfo` (`windows_typelib_loader.rs:1526`). **`oVft` is a byte
offset → slot = `oVft / 8` on x64.** Dual-eligibility v1 gate = `vtable_slot.is_some()
&& oVft >= 7*8 && callconv == CC_STDCALL` (an explicit `is_dual` bit from
`TYPEFLAG_FDUAL` is a nice-to-have).

## Marshalling spec — v1 target surface

Covers Excel `Range.Value` get/put, `Worksheet.Range(name)`, `Workbooks.Add`,
`Worksheets(i)`; DAO `Database.OpenRecordset(sql)`, `Recordset.Fields`, `Fields(i)`,
`Field.Value`. General rules: `this` = `native_dispatch` ptr as `FfiArg::Pointer`
arg0; `[out,retval]` = trailing `FfiArg::Pointer(out_cell)`; HRESULT =
`FfiReturnType::Long`; **vtable params are left-to-right** (unlike DISPPARAMS, which
the IDispatch path reverses at `windows_invoke.rs:458`).

| VARTYPE | inbound param | `[out,retval]` | ownership |
|---|---|---|---|
| VT_VARIANT | `set_variant_from_com_value` → `Pointer(&mut VARIANT)` | zeroed `VARIANT` cell → `take_variant_result_variant` | clear inbound VARIANTs post-call; retval cleared by decoder |
| VT_BSTR | `SysAllocString(utf16)` → `Pointer(bstr)` | `*mut u16` → `bstr_to_string_and_free` | inbound freed by us; retval owned by us (transfer) |
| VT_DISPATCH/VT_UNKNOWN | ObjectRef→IDispatch, `Pointer(idisp)` (no extra AddRef [in]) | `*mut IDispatch` AddRef'd → `bind_native_runtime_object_result_shared` | retval owned by us → bindings map |
| VT_I4/I2/R8/R4/BOOL/DATE/CY/UI1/I8 | scalar `FfiArg` (reuse `dynlink.rs` conv) | scalar out cell | none; BOOL=i16(-1/0); CY=i64×10000; DATE=f64 |
| ByRef scalars | pin a cell → `Pointer`, write-back after | (the [in,out]/[out] non-retval params) | write back into caller Variant via the existing writeback channel |

**propget vs propput:** separate FUNCDESCs with separate `oVft`; pick the member-spec
whose `invoke_kind` matches intent. **default-member/indexer** (`Worksheets(1)`,
`Fields(0)`): ordinary propget-with-one-arg → one VT_VARIANT/VT_I4 [in] + IDispatch*
[out,retval].

**Deferred to IDispatch fallback in v1:** SAFEARRAY (`VT_ARRAY|*`), VT_DECIMAL,
VT_VARIANT **by value**, VT_RECORD/UDT, omitted optional args mid-list, named args,
any `callconv != CC_STDCALL` or `vtable_slot == None`.

## Error handling (S2)

A vtable call has no `EXCEPINFO`. On `hr < 0`, retrieve rich error via
`GetErrorInfo(0)` → `IErrorInfo::GetSource/GetDescription/GetHelpFile/GetHelpContext`
(BSTRs freed via `bstr_to_string_and_free`, then Release the IErrorInfo) → map into
the **existing** `ComInvokeExceptionInfo` (`windows_invoke.rs:82`) so
`render_invoke_fault_message` + `map_com_hresult_label` are reused verbatim. New
`vtable_invoke` returns `Result<Variant, ComInvokeFailure>` like
`invoke_dispatch_variant`, so the downstream `.map_err(render_invoke_fault_message)` +
`WindowsComBridgeDispatchError::InvokeFailure` lane carry it unchanged. Clear stale
thread error info (`SetErrorInfo(0, NULL)`) before the call.

## Gating + zero regression (S3)

Take vtable iff: `prefer_vtable` (policy `PreferVtable`) **and** chosen member
`vtable_slot.is_some() && oVft>=56 && callconv==CC_STDCALL` **and** full signature
present **and** every param + retval VARTYPE ∈ v1 set. Else `Ok(None)` → unchanged
IDispatch path. **Default policy is `DispatchOnly` for all profiles**
(`model.rs:200/213/230`), so until a profile opts in, the vtable branch never fires and
every currently-passing call is byte-for-byte unchanged; even under `PreferVtable`,
any member failing the gate falls back. Gate lives in the `try_vtable_invoke` body
(`windows_bridge.rs:385-422`); broaden the branch at `windows_invoke.rs:1328-1332`
from i32-only to full-Variant args; stop discarding `prefer_vtable` at
`windows_bridge.rs:477`.

> **Open sequencing decision (post-S4):** flip the default early-bound policy to
> `PreferVtable` so vtable is actually used in normal operation (the point of the
> feature), once the path is proven live with the fallback safety net. Tracked as S6.

## Verification (S3/S4)

Per-call transport instrumentation on `WindowsComBridge` (`last_transport:
Arc<AtomicU8>`, mirror the `last_dll_error` atomic pattern), `Vtable`/`IDispatch`/
`Fallback`, read by tests. **S2 unit proof BEFORE live Office:** add a real custom
dual vtable to `windows_test_dispatch.rs` (`extern "system"` slots after the 7
IDispatch slots: `get_Count(this, i32* retval)`, `Exists(this, i32, VARIANT_BOOL*)`,
`put_Value(this, VARIANT)`, `Lookup(this, BSTR, IDispatch**)`, `raise_error` →
`SetErrorInfo`+fail HRESULT) and drive `vtable_invoke` through libffi against them.
**S4 live:** extend `excel_early_bound_range_value_round_trips` +
`dao_early_bound_recordset_field_round_trips` to run under `PreferVtable` and assert
`last_dispatch_transport == Vtable` (plus existing 42.5 / 7 value asserts).

## Slice ledger

| Slice | Content | Files | Test | Gate |
|---|---|---|---|---|
| **S1** | FUNCDESC signature into `ComMemberSpec` + dual detection | `runtime_state.rs`, `windows_typelib_loader.rs`, `typelib.rs` | fixture-typelib unit: dual member carries slot+params+callconv | data plumbing; no behavior change |
| **S2** | libffi this-call marshaller proven vs real fixture vtable + IErrorInfo | new `windows_ffi_bridge::call_via_libffi`, new `oxvba-com::vtable_invoke`, `windows_test_dispatch.rs` real dual slots | in-process unit: get_Count/Exists/put_Value/Lookup/raise_error | fixture-only; nothing wired live |
| **S3** | wire early-bound dual → vtable w/ IDispatch fallback + transport instrumentation | `windows_bridge.rs`, `windows_invoke.rs`, `com.rs` | in-process e2e under PreferVtable: transport==Vtable / ==Fallback | default DispatchOnly unchanged; behind flag |
| **S4** | live verification Excel Range + DAO Recordset assert vtable transport | `com_office_integration.rs` | the two early-bound tests under PreferVtable | `#[ignore]`, env-skip |
| **S5** | widen coverage: property-put, by-ref writeback, VT_CY/DATE/BOOL, IUnknown** | `vtable_invoke`, gate set | fixture unit per type | per-type green |
| **S6** | flip default early-bound policy to PreferVtable (post-live-proof) | `model.rs`, profiles | full COM lane stays green | decision gate (see above) |

## Risks (riskiest = S2)

- **x64 struct-return ABI:** a by-value VARIANT/CY/DECIMAL return uses a hidden
  pointer — but the dual `[out,retval]` convention passes retval as an explicit
  `VARIANT*` trailing param (HRESULT is the real EAX return), so the common case is a
  pointer. **Keep by-value struct returns in the IDispatch fallback for v1.**
- **`oVft` is a byte offset → slot = `oVft/8`.** Off-by-`*8` calls the wrong fn and
  crashes the host — verify against the fixture's known layout FIRST in S2.
- **BSTR/interface ownership:** inbound BSTR freed by us (callee borrows [in]); retval
  BSTR/interface owned by us (callee transfers). Reuse `take_variant_result_variant` /
  `bstr_to_string_and_free` / `bind_native_runtime_object_result_shared` rather than
  re-deriving.
- **callconv mismatch:** assert `CC_STDCALL` in the gate; never vtable-call otherwise.
- **arg omission:** vtable cannot drop optional positional args; gate omitted-optionals
  to fallback in v1.
- **Verify earliest in S2:** the simplest slot `get_Count(this, i32* retval)` proves
  this-ptr + out-cell + HRESULT before any BSTR/VARIANT complexity.

## Progress (2026-06-12)

**S1–S4 COMPLETE + verified** (commits `307f0ae2`, `3c035213`, `b40c0647`, `e755556c`).
The mechanism is proven, gating + IDispatch fallback are correct, and there is **zero
regression** (default policy stays `DispatchOnly`; all 7 live COM tests green; oxvba-com
118/0; clippy `-D warnings` clean). Two genuine ABI hazards were caught and fixed during
the build: **(a)** typelibs ship at different `oVft` granularities — `vtable_slot` =
`oVft / syskind_granularity` where granularity comes from the containing typelib's
`TLIBATTR.syskind` (SYS_WIN32 ACE DAO ships 4-byte oVft even when loaded 64-bit; forcing
`/8` computed half the slot and crashed). **(b)** VARIANT is passed by reference on Win64;
`VT_BOOL`/`VT_CY` need distinct out-cell decoders.

**THE KEY FINDING (reshapes S5 — this is what "fully" requires).** Live transport counts:
- DAO (in-process ACE): value 7 ✓, **vtable_count=1** (`rs.Close` dispatched via vtable),
  idispatch_count=7.
- Excel (out-of-process): value 42.5 ✓, **vtable_count=0** — every call fell back to
  IDispatch.

The headline data-round-trip members (Excel `Range.Value`, DAO `Field.Value`) do **not**
yet go through the vtable. Root cause: OxVba vtable-calls the **`IDispatch` pointer**
directly. For an **in-process** dual object that pointer aliases the custom dual-interface
vtable (`[IUnknown|IDispatch slots 0-6][custom slots 7+]`), so a slot ≥7 call works
(DAO `rs.Close`). For an **out-of-process** object we hold an `IDispatch` *marshaling
proxy* with only 7 slots — a slot ≥7 call read past it and **access-violated the host**
(caught live; now gated off by an `IID_IProxyManager` QI → proxies always fall back).

**The fix that delivers the goal fully (real IDE-style early binding):** do not vtable-call
the raw `IDispatch` pointer. Instead `QueryInterface` the object for the **typelib-declared
dual interface IID** (from the member's defining `ITypeInfo` `GetTypeAttr().guid`) and call
the slot on *that* pointer. The oleaut universal marshaler (`PSOAInterface`) builds a proper
**vtable proxy** for any oleaut-compatible dual interface, so this works **uniformly
in-process and out-of-process** — it is exactly how the VBA IDE holds and calls early-bound
references. This removes the proxy special-case (a proxy QI'd for the custom interface *is*
vtable-callable).

### S5 — REDEFINED (delivers "fully")
1. **Custom-interface QI dispatch** (the core): extract the defining dual-interface IID per
   member/interface (`ITypeInfo::GetTypeAttr().guid`, plumbed onto the member spec / live
   recovery path); QI the object for it; vtable-call on the QI'd pointer; manage its
   lifetime (Release). This makes Excel `Range.Value` and all out-of-process dual members
   dispatch via vtable. **Riskiest remaining work** (out-of-process marshaling; host-AV
   class bugs) — validate on Excel `Range.Value` first.
2. **Omitted-optional arguments**: vtable calls cannot drop trailing positional optionals;
   supply the typelib-declared default or `VT_ERROR`/`DISP_E_PARAMNOTFOUND` so members like
   DAO `Fields(0)` / `OpenRecordset(sql,…optionals)` go through vtable instead of falling
   back on arity.
3. **Raw-getter / Invoke divergence** (investigate): DAO `Field.Value` via the raw dual
   getter returned "Invalid operation" while `Invoke` returns 7 — likely a default-member
   getter quirk or wrong interface; resolve once (1) lands (correct interface) or keep it on
   the best-effort fallback if genuinely server-specific. Do **not** mask real errors.
4. Property-put via vtable for in-process servers; `VT_CY`/`VT_DATE` completeness;
   `IUnknown**` retvals (`query_dispatch_from_unknown`).

### S6 — flip default to PreferVtable (unchanged, post-S5)

## Cross-refs

`docs/spec/COM_EARLY_BINDING_TYPELIB_SCOPE_V1.md` (anticipates default `prefer_vtable`
in a strict/perf profile), `docs/spec/HAL_DECLARE_ABI_SPEC_V1` (shared libffi ABI
contract). Parent program: the COM/host completion + robustness program.

## Critical files

- `crates/oxvba-com/src/windows_ffi_bridge.rs` — libffi mechanism to factor/reuse
  (`invoke_stdcall_x64:127`, `FfiArg`/`FfiReturnType`)
- `crates/oxvba-com/src/windows_invoke.rs` — dispatch decision tree
  (`execute_bound_variant:1290`), VARIANT marshalling, `ComInvokeFailure`, error
  rendering; new `vtable_invoke` lives near here
- `crates/oxvba-com/src/windows_bridge.rs` — `try_vtable_invoke:385-422` (gate +
  transport instrumentation), `prefer_vtable:477`
- `crates/oxvba-com/src/runtime_state.rs` — `ComMemberSpec:305` +
  `member_specs_from_typelib_metadata:196` (signature enrichment)
- `crates/oxvba-com/src/fixtures/windows_test_dispatch.rs` — add real custom dual
  vtable for S2; `RawIDispatchVtbl` layout reference
- Supporting: `windows_typelib_loader.rs:1505` (FUNCDESC extraction),
  `windows_variant.rs:1543/1580` (Variant↔VARIANT helpers),
  `hal/adapters/standard/com.rs:306` + `hal/model.rs:124` (policy/`prefer_vtable`),
  `oxvba-host/tests/com_office_integration.rs:308/388` (live verification)
