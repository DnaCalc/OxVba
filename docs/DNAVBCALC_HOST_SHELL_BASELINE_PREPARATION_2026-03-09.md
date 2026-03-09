# DNA VbCalc Host-Shell Baseline Preparation — 2026-03-09

Status: preparatory note for a future separate `DnaVbCalc` repository  
Scope: define the first concrete host-shell baseline that the future DNA VbCalc project should implement in order to validate embedded OxVba hosting with the least avoidable scope risk.

## 1. Purpose

This document is not an OxVba workset.

It exists to prepare a future external repository by answering:
1. what the first real DNA VbCalc application should be,
2. what it should load,
3. what the first UX surface should do,
4. what is explicitly out of scope for the initial baseline.

## 2. Baseline decision

The first DNA VbCalc host-shell baseline is:
1. a Tauri desktop shell,
2. with a Rust backend,
3. and a web UI frontend,
4. loading an OxVba `oxvba.toml` project,
5. presenting a debug/immediate-style shell as the first interaction model.

This baseline is intentionally debug-centric, not form-designer-centric.

## 3. What the baseline host should do

### 3.1 Shell and startup

1. Launch as a desktop app through Tauri.
2. Accept an `oxvba.toml` project path as a startup argument.
3. Also support opening a project from the UI (`File -> Open` or equivalent web-shell open flow).

### 3.2 Project model

1. One active OxVba project at a time.
2. Load project from `oxvba.toml` plus its project directory.
3. Compile and execute through the embedded OxVba engine.

### 3.3 First user-facing surface

The first UX is a debug shell similar in spirit to the VBA IDE debug/immediate windows:
1. run the project entrypoint,
2. reset execution,
3. evaluate simple expressions/commands,
4. print diagnostics/output,
5. inspect runtime feedback at a practical debugging level.

This should be enough to validate:
1. project loading,
2. bridge calls,
3. diagnostics routing,
4. run/reset behavior,
5. basic host-event ingress.

## 4. Operational baseline

### 4.1 Reload model

v1 reload behavior is:
1. full reset,
2. full recompile,
3. fresh execution state.

No attempt should be made in the first baseline to preserve:
1. live object identity,
2. module-level runtime state,
3. subscriptions across reload,
4. partial hot reload.

### 4.2 Event model

1. Start with the non-COM host bridge path first.
2. Use the explicit host-event ingress contract already locked in OxVba:
   - `Engine::dispatch_host_event(subscription, args)`
3. Treat COM as a separate transport lane, not as the baseline pathfinder dependency.

### 4.3 Threading/process posture

1. Keep the baseline operational model simple.
2. Prefer one engine-execution owner on the Rust backend side.
3. UI-to-engine interactions should be explicit backend commands, not implicit shared-state coupling.

This does not fully prescribe the final threading model, but it rules out premature complexity in the first baseline.

## 5. Initial host object model

The initial host shell does not need a rich visual object hierarchy.

Recommended minimal host surface:
1. `Application`
2. `Workspace` or `Project`
3. `DebugConsole` / `Immediate`
4. a minimal command surface for:
   - `Open`
   - `Run`
   - `Reset`
   - `Print` / output

The first baseline should prove:
1. the host bridge works,
2. the engine can load and execute a project under host control,
3. diagnostics and event ingress are usable,
4. developer iteration is possible.

## 6. Explicitly out of scope for baseline v1

Not part of the first DNA VbCalc baseline:
1. a visual form designer,
2. a full workbook/control hierarchy,
3. complex control binding or layout tooling,
4. full IDE parity with the VBA editor,
5. COM-hosted UI integration as the primary path,
6. multi-project workspace orchestration,
7. incremental/hot reload with state preservation.

## 7. Why this baseline is preferred

This baseline is preferred because it:
1. validates real embedded hosting with low ambiguity,
2. exercises the OxVba host bridge and diagnostics in a usable shell,
3. avoids overcommitting to a much larger application before the host model is proven,
4. aligns with the chosen Tauri/web UI stack pathfinder,
5. keeps COM transport work and host-shell work decoupled enough to evolve sanely.

## 8. Relation to OxVba repo planning

This note should inform OxVba-side design docs, but DNA VbCalc itself belongs in a separate repository.

Use this document as:
1. a seed for the future `DnaVbCalc` repo README/design note,
2. a baseline scope definition before creating repo tasks,
3. a guardrail against prematurely turning the pathfinder into a full application platform.

## 9. Suggested first external-repo milestones

When the separate `DnaVbCalc` repo is created, the likely first milestones are:
1. shell boots and opens a project path,
2. backend loads and compiles `oxvba.toml`,
3. debug/immediate window can issue run/reset/eval-style commands,
4. diagnostics and output are visible in the shell,
5. one minimal host-event ingress path is demonstrated end-to-end.
