# PROGRAM-0 Directed Rollout Acceptance

Date: 2026-07-10  
Completed: 2026-07-11  
Epic: `bd-59co.1`

Outcome: complete. The Ideal OxVba umbrella is accepted for AutoRun. PROGRAM-0 reconciled the x64-only target, established the three-profile executable graph and canonical truth surfaces, migrated every non-closed legacy issue, generalized the validators, and passed the final independent post-repair semantic and documentation reviews.

Control-leaf evidence:

- `PROGRAM-0-1.md`: x64-only scope reconciliation (`7f14bf48`);
- `PROGRAM-0-2.md`: umbrella/profile/epic/matrix rollout (`a6adfcc5`);
- `PROGRAM-0-3.md`: legacy migration and validator modernization (`43f3d2ab`);
- `PROGRAM-0-4.md`: graph polish and AutoRun queue acceptance.

Terminal control state:

- active manifest: `docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json`;
- program root and terminal gate: `bd-59co`;
- accepted workset roots: Core `bd-59co.2`, Windows x64 `bd-59co.3`, IDE `bd-59co.4`;
- sole claim queue: `br ready -l ideal-2026-07 -t task`;
- first claim: `bd-59co.2.1.1`;
- AutoRun stops only when all three profile roots close beneath the umbrella or every remaining path is genuinely blocked and recorded under the repository blocker protocol.

PROGRAM-0 is support/control work. Its closure establishes a delivery-ready graph; it does not close or implement any compiler, library, VM3, JIT, Windows interop/native-output, or language-service capability row.
