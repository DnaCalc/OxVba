# Workset: Host Program Design And UDF Rework

Date: 2026-05-10
Owner: Codex
Status: planned
Bead root: `bd-sg5h`
Sequencing: after the next WrappedComServer workset reaches its reopened direct-host build gate.

## Purpose

Design and rework the OxVba host-program surface for hosts that load, inspect,
run, and coordinate projects. This includes the narrower but important case where
a host examines public module functions and invokes them as UDF-like host calls
with explicit host context.

This workset is not a COM Automation Add-In workset. It is about the direct host
contract: project sessions, callable function catalogs, host call frames,
calculation-style context, diagnostics, capability states, and evidence that the
host-facing APIs actually drive runtime behavior rather than only echoing DTO
shape.

## Scope

In scope:

- Define the host program lifecycle for loading a project, preparing a runtime
  session, inspecting project/module/function inventory, invoking entry points,
  and reporting diagnostics/status back to the host.
- Define the UDF-like host-call contract for public module functions, including
  stable identity, signature metadata, caller/locale/context, volatile and
  dependency semantics, result mapping, and conservative side-effect policy.
- Decide which metadata source is authoritative for host-call catalogs:
  compiled exports, persisted bundle descriptor inventory, or a merged typed
  descriptor model.
- Rework current `PH-0011` implementation surfaces so tests prove context and
  descriptor semantics, not only DTO echo.
- Update `PROJECT_HOSTING_VALIDATION_MATRIX_V1.csv` and any host integration
  docs once the design is implemented and evidenced.

Out of scope:

- Excel/XLL implementation.
- COM Automation Add-In compatibility.
- OxIde UI layout or DnaOneCalc calculation engine implementation.
- The reopened WrappedComServer direct-host build execution gap, which remains
  owned by `bd-wcs1.9.4` in the WrappedComServer workset.

## DNA Calc Registry Alignment

This workset is the OxVba source-discovery counterpart to OxFunc `W093` and
OxFml `W074`. OxVba owns VBA project/module/procedure discovery, VBA/XLL-adjacent
descriptor facts, and invocation-time provenance. It does not own worksheet
formula name precedence, registry mutation semantics, or TreeCalc reference
semantics.

The host-call/UDF catalog produced here must be convertible into OxFunc
source-neutral UDF registration requests without a host-local function mirror.
The first metadata lane must carry:

1. stable source identity: project/session id, module id, procedure id, and a
   source fingerprint suitable for unregister/re-register decisions,
2. callable surface metadata: worksheet-visible name, public/procedural function
   classification, arity, parameter names, parameter type text where known,
   return type text where known, help/category text when the source supplies it,
3. invocation descriptor: OxVba project handle, module/procedure route, runtime
   profile, host-call context needs, argument conversion lane, result conversion
   lane, and diagnostic projection lane,
4. capability and safety facts: supported scalar/array/result subsets,
   side-effect policy, volatility/dependency hints when known, thread-safety
   posture when known, and explicit unsupported reasons,
5. change facts: project load/unload, module edit, function admission/rejection
   change, and descriptor fingerprint change.

Downstream ownership is fixed:

1. OxFunc owns `UdfRegistrationRequest`, UDF registration/unregistration,
   immutable registry snapshot identity, capability overlays, and
   `RegistryChangeSet` output.
2. OxFml owns formula parse/bind/name resolution, registry snapshot consumption,
   `#NAME?` recovery, unregister/capability-denial invalidation, and Excel
   oracle-backed name/call precedence.
3. DnaOneCalc owns its single-formula host orchestration and may load OxVba
   projects as a UDF source, but it must not keep a second comprehensive
   function catalog or decide formula-name precedence locally.

OxVba evidence for this lane should therefore prove that the discovered catalog
is stable, typed, provenance-rich, and retraction-capable. It must not claim
that formulas bind or that UDF names shadow built-ins or defined names; those
claims depend on OxFunc `W093` plus OxFml `W074` evidence.

Current support state after `bd-sg5h.2`: `HostUdfFunctionDescriptor` now exposes
the OxVba-owned descriptor facts needed by W093-oriented consumers without
owning registry behavior. The descriptor separates registration identity,
callable metadata, invocation target, capability constraints, and change-signal
inputs. Evidence:
`docs/evidence/HOST_UDF_W093_METADATA_DESCRIPTOR_2026-05-22.md`.

## Seed Issues From Review

These notes are intentionally early seeds, not final design decisions.

1. `HostUdfCallContext` is not delivered into execution.
   `Engine::invoke_host_udf_with_variants` builds a `RuntimeCallFrame` with
   `RuntimeCallSource::HostUdf`, caller, locale, and positional arguments, but
   then discards it and invokes through the generic
   `invoke_procedure_with_variants` path. The current result echoes caller,
   dependency, and volatile fields, but runtime code does not receive that
   context.

2. Persisted host-call descriptor metadata is not the catalog source of truth.
   Bundle creation persists `descriptor_inventory.host_calls`, but prepared
   bundle sessions currently restore `export_inventory.host_exports`, and
   `host_udf_catalog` rebuilds descriptors with hardcoded category,
   description, volatility, dependency, side-effect, threading, and allowed
   context values. The workset needs to decide whether descriptor inventory is
   authoritative or whether a richer merged model is required.

3. Host-call descriptor inventory currently mixes functions and Subs under a
   function selection policy. `collect_host_exports` can export public Subs and
   Functions, while `BundleHostCallDescriptor.selection_policy` is currently
   `"public-procedural-functions"`. The UDF-like surface should be explicit
   about function-only catalog rules, Sub entry-point rules, and any shared
   host-call descriptor substrate.

## Design Questions

- What is the canonical host-call request model: a `RuntimeCallFrame`, a
  host-specific wrapper over it, or a new typed facade that lowers into frames?
- How does a host pass caller identity, locale, dependency tokens, calculation
  mode, cancellation state, and volatile requests without smuggling host-specific
  concepts into VM internals?
- Should UDF catalog descriptors be loaded from bundle descriptor inventory for
  packaged projects, rebuilt from compiled metadata for source projects, or
  normalized into one host descriptor model before session preparation?
- Which result shapes are first tier: scalar only, scalar plus arrays, or scalar
  plus arrays/errors/object references?
- How should side effects be denied or reported for worksheet-like function
  calls while still allowing ordinary host entry-point execution elsewhere?

## Initial Bead Shape

The first execution pass should roll this into child epics before implementation:

- host lifecycle and session contract design,
- host-call/UDF descriptor model,
- host-call frame and runtime context delivery,
- catalog/invoke implementation rework,
- validation and evidence refresh for `PH-0011`.

## Terminal Condition

This workset is complete only when:

1. the host-program design is documented with clear direct-host boundaries,
2. UDF-like host calls deliver context into actual runtime execution,
3. catalog descriptors use the documented metadata source of truth,
4. function-only UDF behavior and non-UDF Sub/entry-point behavior are separated,
5. tests/evidence cover the supported host-call subset, and
6. `PH-0011` matrix language matches the implemented and evidenced behavior.
