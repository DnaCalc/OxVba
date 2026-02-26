# Synthesis Report: MACH-1000 OxVba Plan

**Run ID:** `20260226-mach1000-synthesis`
**Date:** 2026-02-26
**Owner:** @govert / Claude Opus 4.6

---

## Summary

This synthesis run integrated the MACH-1000 theoretical architectures brainstorm into the baseline OxVba project plan, producing `MACH1000_PLAN.md` as the definitive project plan.

**18 suggestions extracted. 13 accepted, 3 adapted, 2 deferred, 0 rejected.**

## Major Architectural Changes

### 1. 16-byte Cache-Optimal Variant (MK001, MK002, MK003, MK016)
The 24-byte `repr(C)` Variant is replaced with a 16-byte (128-bit) tagged union. This packs exactly 4 Variants per 64-byte cache line (up from ~2.6), with small-string optimization (SSO) for strings ≤14 bytes. NaN-boxing was analyzed and rejected due to Decimal's 96-bit requirement. COM ABI bridging happens at boundaries, not in hot loops.

### 2. Multi-Level Intermediate Representation (MK006, MK018)
A new `oxvba-ir` crate implements three-tier progressive lowering (VbaHir → VbaMir → CfgIr), inspired by MLIR methodology but implemented in pure Rust. This preserves VBA-specific semantics (COM dispatch, implicit coercion, error handling) long enough for domain-aware optimization before committing to low-level forms.

### 3. Register-Window VM (MK009)
The stack-based VM is replaced with an MMIX-inspired register-window VM. Register-based bytecode eliminates operand-stack push/pop overhead. The sliding register window enables zero-copy argument passing via overlap regions. Spill/fill handles deep call trees. This is the most impactful single change for interpreter performance.

### 4. Guarded-Region Error Handling (MK007)
`On Error Resume Next` is modeled as a first-class guarded-region in VbaHir, with staged expansion through VbaMir to CfgIr. This prevents the irreducible CFGs that naive lowering creates, preserving optimization opportunities in early IR passes.

### 5. Broadword Instruction Decoding (MK008)
SWAR (SIMD Within A Register) techniques from Knuth scan 8 opcode bytes simultaneously per 64-bit word, reducing branch misprediction in the interpreter hot path.

### 6. Boundary-Tag Allocator (MK010)
A purpose-built boundary-tag allocator for BStr/SafeArray provides O(1) coalescing, addressing fragmentation in long-running VBA workloads.

### 7. Zero-Copy Bytecode Loading (MK011)
`rkyv`-based serialization with memory-mapped loading eliminates startup allocation for large macro corpora.

### 8. Kani Bounded Model Checking (MK014)
Kani proof harnesses verify critical unsafe code: Variant union access, SSO thresholds, broadword masks, register-window bounds, boundary-tag allocator invariants, COM pointer casts. Added as a fourth testing tier alongside unit tests, conformance tests, and property-based tests.

## Adapted Suggestions

### Finger Trees (MK004)
Adapted from "use finger trees" to "use SmallVec/ThinVec initially with finger-tree upgrade path." Finger trees are theoretically elegant but no mature Rust crate exists. The tiered container approach delivers good performance now with an upgrade path when incremental reparsing demands it.

### MLIR Framework (MK006)
Adapted from "use MLIR" to "implement MLIR concepts in pure Rust." The actual MLIR framework is C++-based, conflicting with values #4 (small runtime) and #5 (well-managed dev env). The three-tier IR captures the essential insight (progressive lowering) without the dependency.

### Cycle Detector Latency (MK017)
Adapted from "cycle detection causes latency spikes" to "epoch-based batching with configurable triggers." The Bacon-Rajan collector was already opt-in; the adaptation adds explicit scheduling guarantees and amortization.

## Deferred Suggestions

### Verus (MK012) and Creusot (MK013)
Both deductive verification tools are promising but immature. Deferred until: Lean specifications are stable, the multi-level IR is implemented, and tool maturity improves. The architecture accommodates future integration — critical unsafe code is isolated into small, verifiable functions.

## Structural Changes to Plan

| Plan Section | Change |
|---|---|
| 1.2 Values | Performance value expanded to reference MACH-1000 techniques |
| 1.5 References | Added Knuth TAOCP and MLIR paper |
| 2.1 Crates | Added `oxvba-ir` (9 crates total, up from 8) |
| 2.2 Pipeline | Complete rework: 8-stage pipeline with 3-tier IR |
| 2.3 Variant | Replaced 24-byte with 16-byte layout; added SSO and NaN-boxing analysis |
| 2.4 Memory | Added boundary-tag allocator and epoch-batched cycle detector |
| 2.7 Error | Added IR-level guarded-region modeling |
| 3 Formal | Added Kani (Section 3.3) and deferred Verus/Creusot (Section 3.5) |
| 4 Testing | Four tiers (added formal verification tier) |
| 5 Research | Added MLIR (5.5), MMIX (5.6), broadword (5.7) sections |
| 6 Design | Bytecode redesigned as register-based; VM redesigned as register-window |
| 7 Structure | Added oxvba-ir crate, formal/kani directory, new doc files |
| 8 Sequencing | 12 phases (up from 10): added Phase 3 (IR) and Phase 10 (optimization push) |
