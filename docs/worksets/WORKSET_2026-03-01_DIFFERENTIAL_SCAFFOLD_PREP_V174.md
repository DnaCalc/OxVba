# WORKSET_2026-03-01_DIFFERENTIAL_SCAFFOLD_PREP_V174.md

## Objective

Execute profile scope `v174`: prepare reusable oracle-probe scaffolding for future differential validation runs without introducing blocking gates.

## Scope

In scope for `v174`:
- add scaffold script for generating deferred oracle probe queue rows;
- document usage and non-blocking policy for deferred oracle work;
- synchronize formal checks/profile status docs.

Out of scope:
- executing external host oracle captures;
- closing deferred oracle gates.

## Deliverables

- Oracle scaffold assets:
  - `scripts/oracle-probe.ps1`
  - `docs/evidence/conformance/ORACLE_PROBE_SCAFFOLD.md`
- Formal checks:
  - `docs/evidence/formal/obligations.csv`
  - `crates/oxvba-host/src/engine.rs`
- Profile status:
  - `docs/profile-status/PROFILE_STATUS_V174.md`

## Closure Conditions

Profile `v174` is complete when oracle-probe scaffold artifacts exist and are referenced by formal checks with non-blocking deferred-oracle policy.
