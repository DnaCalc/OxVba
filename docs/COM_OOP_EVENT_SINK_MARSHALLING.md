# Out-of-process COM event delivery — sink marshalling

## Symptom

`WithEvents app As Excel.Application` (an **out-of-process** COM source) never delivers
events, and the host hangs: `Set appEv = app` (the connection-point `Advise`) does not
return, with `EXCEL.EXE` pinned at ~100% CPU. In-process `WithEvents` (the in-proc
`OxVba.TestEventServer`, matrix V1–V6) works fine. Matrix `com_matrix_events.rs` V7
(`Application.NewWorkbook`) and V8 (`Workbook.SheetChange`) exercise the OOP path.

## Bisection (control experiment, 2026-06-15)

A throwaway standalone crate (`com_sink_probe`, since removed) built an event sink with
the **canonical `windows`-crate `#[implement(IDispatch)]`** machinery — which produces a
correctly cross-apartment-marshallable COM object — and Advised it on the *same* live
out-of-process Excel `AppEvents` connection point:

```rust
#[implement(IDispatch)]
struct EventSink;
impl IDispatch_Impl for EventSink_Impl {
    fn GetTypeInfoCount(&self) -> Result<u32> { Ok(0) }
    fn GetTypeInfo(&self, _: u32, _: u32) -> Result<ITypeInfo> { Err(E_NOTIMPL.into()) }
    fn GetIDsOfNames(&self, ..) -> Result<()> { Err(E_NOTIMPL.into()) }
    fn Invoke(&self, dispid: i32, ..) -> Result<()> { /* record fired */ Ok(()) }
}
// CoInitializeEx(STA); CoCreateInstance(Excel.Application);
// app.cast::<IConnectionPointContainer>().FindConnectionPoint(DIID_AppEvents)
//   .Advise(sink.cast::<IUnknown>())  -> cookie
// app.Workbooks.Add  -> fires NewWorkbook
```

**Result — the canonical sink received the events flawlessly:**

```
[probe] FindConnectionPoint(AppEvents) OK
[probe] Advise OK cookie=1                 <- no hang
[SINK] Invoke FIRED dispid=1565            <- NewWorkbook (0x61D)
[SINK] Invoke FIRED dispid=1568
[SINK] Invoke FIRED dispid=1556
[probe] EVENT DELIVERED after 0 pumps      <- delivered SYNCHRONOUSLY during Add
[probe] RESULT fired=3
```

Events arrived **re-entrantly during the outbound `Workbooks.Add` call** (Excel called
back into the STA and COM serviced it automatically — no explicit message pump needed),
and `Unadvise` + `Quit` tore down with **no lingering `EXCEL.EXE`**.

## Conclusion

The environment is entirely correct: OOP Excel fires events, STA RPC re-entrancy delivers
them, Advise/Unadvise/Quit all behave. **The hang is specific to OxVba's hand-rolled raw
`IDispatch` event sink** (`crates/oxvba-com/src/windows_connection_point.rs`
`create_dispatch_event_sink` — a static Rust vtable). It works in-process (same apartment,
no marshalling), but it is **not correctly cross-apartment marshallable**, so on an OOP
`Advise` Excel cannot establish a usable callback path and spins; our `Advise` never
returns. (This is why the instrumented v7 run hung *before* the post-`Advise` log with
Excel at 100% CPU.)

## Fix

Make the OxVba sink cross-apartment marshallable, mirroring what the `windows`-crate object
gets for free. Implemented approach: **aggregate the COM free-threaded marshaler**
(`CoCreateFreeThreadedMarshaler`) into the sink and delegate `QueryInterface(IID_IMarshal)`
to it, so the sink is agile and COM marshals it directly to the out-of-process server
(bypassing the STA-stub round-trip that deadlocked). Agility is harmless in-process, so the
same sink keeps working for:

  * in-proc COM source (matrix V1–V6, `OxVba.TestEventServer`), and
  * **OxVba running in-process inside Excel** (a future hosting mode: OxVba loaded into
    Excel's own STA, where `WithEvents` on Excel objects is same-apartment) — events must
    work there too; the FTM aggregation does not interfere with same-apartment calls.

Verified by re-running matrix V1–V6 (in-proc, no regression) and V7/V8 (OOP Excel).

## Follow-on layers (after the FTM fix)

The FTM aggregation removed the `Advise` deadlock, but three further layers gated
real delivery. With all of them fixed, **V7 (`Application.NewWorkbook`) passes live
end-to-end in ~3 s**; V8 (`Workbook.SheetChange`) delivers identically and its one
residual is a binder gap, not an event gap.

### Layer 2 — the source binding had no event metadata

`Excel.Application`'s `HKCR\CLSID\{00024500-…}` key has **no `\TypeLib` subkey**, so
the ProgID→typelib resolver fails and the runtime binding is built with `None`
metadata → empty `event_specs` → `subscribe_event` returns
`COM-E-EVENT-CONNECTIONPOINT-MISSING`. Fix: recover the typelib from the **live
object itself** (`IDispatch::GetTypeInfo(0)` → `ITypeInfo::GetContainingTypeLib`),
scoping enumeration to the object's own coclass (default-interface name `_Foo` →
coclass `Foo`). Applied at activation and lazily at subscribe time for objects
returned by a method call (a `Workbook` from `Workbooks.Add`, whose binding starts
metadata-less). The recovered blob carries **events only** — never members (see the
performance layer below).

### Layer 3 — object-typed event arguments cross an apartment boundary

The agile sink fires on an MTA RPC worker thread, but the VM consumes events on its
STA thread; an object argument (the new `Workbook`, the changed `Range`) marshalled
into the MTA cannot be touched from the STA. Fix: register each object arg in the
process **Global Interface Table** on the delivery thread, queue a `Nothing`
placeholder plus `(arg_index, cookie)`, and revive it into a thread-correct binding
on the VM thread at poll time (`GetInterfaceFromGlobal`), revoking the cookie.
A callback discarded before it is pumped must revoke its cookies so the GIT never
pins a source object (which would hold a COM reference forever and can keep an
out-of-process source from shutting down). EVERY teardown path funnels through
`revoke_marshal_cookies`: unsubscribe (`remove_subscription_callbacks`), release
(`release_callback` / `release_object_binding`), a declined queue (`callback_sink`
when the subscription is already gone), and client `WindowsComClientState::drop`.
The revoke is double-revoke-safe — poll-revival clears `pending_marshals` and every
teardown removes the callback from the map, so each cookie is revoked at most once.
Also: a `WithEvents` source that reads LIBRARY-WIDE typelib events can
produce duplicate routes for a name shared across source interfaces (`SheetChange` on
both `AppEvents` and `WorkbookEvents`, same dispid) — `build_event_routes` dedupes
`(binding, event, handler)` so the connection point is advised exactly once.

### Layer 4 — the ~56-minute "wedge" was OUT-OF-PROCESS DISPATCH, not events

Once delivery worked, V7/V8 still took ~50 min. A raw `IDispatch::Invoke` probe
proved Excel itself answers in **~60 ms** — the cost was entirely OxVba's dispatch:
for a late-bound member with `PreferVtable`, the bridge recovered the member's
FUNCDESC from the object's **live ITypeInfo**, and for an out-of-process object that
ITypeInfo is a MARSHALLED proxy whose every read is a cross-apartment RPC — locating
one member in Excel's 471-member `_Application` took ~5 minutes **per call**, and the
proxy was declined for the slot-call anyway. Fixes:
  * `build_metadata_blob_from_dispatch` reloads the typelib **locally by libid** and
    enumerates **events only** (no member walk), so recovery is in-process and an
    out-of-process object dispatches its own members late-bound;
  * `try_live_vtable_invoke` **declines a marshaling proxy first**, on a cheap
    `QueryInterface(IID_IProxyManager)` probe, before any live-typelib FUNCDESC
    recovery — so an out-of-process object goes straight to fast IDispatch. A direct
    in-process interface (DAO) still vtable-calls as before.

### Layer 5 — multi-arg event sink arg ORDER (V8 green)

V8 (`Workbook.SheetChange(Sh, Target)`) initially failed with `Target.Column` →
`DISP_E_UNKNOWNNAME`: the handler's `Target` was the **Worksheet**, not the **Range**.
Root cause was NOT the binder (it late-binds `.Column` on an `As Excel.Range` receiver
correctly — `is_late_bound_receiver` is true for any non-project-class Object) — it was
the **sink arg order**, and the order is delivery-dependent:
  * an OUT-OF-PROCESS source's marshaled call to our agile (FTM) sink arrives in DECLARED
    (forward) order, on an RPC-worker thread;
  * a DIRECT in-process source call (the matrix fixture, V1–V6) arrives in the standard
    IDispatch caller-side REVERSED order (`rgvarg[0]` = last arg), on the subscriber thread.

The sink can't assume one order. It picks the layout by comparing the calling thread to the
thread that created/advised the sink: same thread ⇒ direct ⇒ reversed (un-reverse); a
different thread ⇒ marshaled ⇒ forward. (Apartment type is unreliable — an agile call
reports neutral, not MTA — but the thread identity is exact.) Single-arg events (V1–V7)
cannot reveal the order; the first 2-arg event exposed it. V8 now green — the GIT-revived
`Range` is the real `Target` and `Target.Column == 3`; V1–V6 (in-proc, incl. the 2-arg
arg-order pin V3) stay green.

**Caveat — this is an empirical heuristic, not a contract guarantee.** The IDispatch
`rgvarg`-reversed convention is *transport-invariant* (a faithful proxy/stub preserves the
array order), so "forward-for-OOP" is not predicted by the contract and the thread→layout
inference is known to be wrong for at least one *untested* topology: an in-process
FREE-THREADED source firing off the advise thread would deliver reversed args on a different
thread, yet be read forward. The two transports the matrix exercises (in-proc same-thread,
OOP RPC-worker) happen to fall on the heuristic's correct side. The robust resolution is a
live per-slot `rgvarg` `vt`/identity dump confirming the layout per transport before relying
on the thread rule; until then the heuristic is retained because changing it (e.g. to
unconditional un-reverse) would break the live-green V8 OOP path AND the in-proc V3 pin. See
the `created_thread_id` doc-comment in `windows_connection_point.rs`.
