# MACH-1000 Plan Refinement Proposals (2026-02-26)

## Context
This proposal set refines `MACH1000_PLAN.md` using implementation feedback and alignment with Foundation doctrine (`CHARTER.md`, `OPERATIONS.md`).

## Clarifications from Product Direction
1. Forms runtime integration is part of initial focus. Forms Designer remains future work.
2. DNA Calc integration is contextual; OxVBA should be host-aware and accept root host objects (for example `Application`) at runtime load. DNA Calc-specific integration work is primarily implemented in DNA Calc.

## Proposal Set

### RF001 - Scope Clarification: Forms and Hosting
- Keep Forms runtime in initial focus.
- Keep Forms Designer in future/not-active scope.
- Clarify host-aware API requirement in initial focus and DNA Calc section.

### RF002 - Add Early End-to-End Vertical Slice
- Insert an explicit MVP phase before full IR/VM optimization complexity.
- Goal: parser -> binder -> minimal bytecode -> execution path validated early.

### RF003 - Gate High-Risk Performance Features Behind Flags
- Keep broadword dispatch, aggressive register-window policies, and zero-copy bytecode paths behind feature flags until correctness gates are green.
- Define promotion criteria from experimental to default.

### RF004 - Make Milestones Quantitative
- Replace purely narrative phase milestones with measurable gates: pass rates, coverage, divergence counts, and benchmark thresholds.

### RF005 - Introduce a Formal Risk Register
- Add top technical risks with owner, mitigation, trigger, and exit criteria.
- Prioritize: VARIANT ABI bridge, On Error semantics, ByRef/default property edge cases, COM lifecycle/cycle collection.

### RF006 - Clarify AOT Responsibility Split
- Distinguish:
  - Backend/compiler AOT capability (code generation target).
  - CLI/distribution packaging of standalone binaries.

### RF007 - Compatibility Matrix Gate Model
- Add an initial matrix gate (Office/VBA versions, architecture widths, platform behavior classes).
- Mark as iterative: expand over time as evidence corpus grows.

### RF008 - Add Phase Execution Metadata
- For each phase include:
  - primary owner track,
  - estimated duration,
  - dependencies,
  - parallelizable tracks.

### RF009 - Align Sequencing with Foundation Recalc Discipline
- Explicitly state dirty-marking/dependency-closure mindset for plan updates.
- Tie major claims to pack-like gate artifacts and reproducible evidence where practical.

### RF010 - Keep Synthesis Provenance Current
- Record this refinement as a new synthesis run and update plan provenance counts.
