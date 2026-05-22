# Host UDF W093 metadata descriptor evidence

Date: 2026-05-22
Bead: `bd-sg5h.2`
Related upstream work: OxFunc `W093`, OxFml `W074`

## Scope

This evidence covers the OxVba-owned host UDF discovery descriptor surface for
cross-repo name/reference rollout support. It proves that host UDF catalog
entries expose enough typed OxVba metadata for a downstream registry owner to
construct source-neutral registration requests and invocation descriptors.

This is not an OxFunc registry implementation, formula name-precedence claim, or
worksheet binding/recalc claim.

## Commands

```powershell
cargo fmt --all
cargo test -p oxvba-host --test invoke_procedure_tests host_udf --quiet
```

## Verified behavior

- `HostUdfFunctionDescriptor` keeps the existing stable host-call identity and
  now also exposes `HostUdfRegistrationIdentity` with an OxVba source system,
  source fingerprint, and unregister key input.
- Callable metadata is separated from invocation routing through
  `HostUdfCallableMetadata` and `HostUdfInvocationTarget`.
- Callable metadata includes worksheet-visible name, export kind, arity,
  parameter type text, return type text, optional help/category fields, and a
  descriptor fingerprint.
- Invocation target metadata names the prepared project session route, stable
  host-call id, module/procedure route, argument/result conversion lanes, and
  diagnostic projection lane.
- Capability metadata is exposed through `HostUdfCapabilityConstraints`,
  including supported first-tier value subsets, allowed contexts,
  side-effect policy, thread-safety policy, and unsupported reasons.
- Change-signal inputs are explicit: project load/unload, module edit, function
  admission/rejection change, and descriptor fingerprint change.

## Boundary

OxVba provides discovery, provenance, capability, and invocation-route facts.
OxFunc owns UDF registration/unregistration, immutable registry snapshot
identity, capability overlays, and registry change-set emission. OxFml owns
formula binding/name precedence and invalidation behavior.
