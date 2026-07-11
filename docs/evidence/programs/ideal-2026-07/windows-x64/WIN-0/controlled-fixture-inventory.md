# WIN-0 Controlled x64 Fixture Inventory

Date: 2026-07-11
Bead: `bd-59co.3.1.4`
Effect: support only
Clauses: `CONF-MATRIX-001`, `PROFILE-WIN-001`

## Outcome and no-credit boundary

`docs/validation/IDEAL_WINDOWS_X64_FIXTURE_MANIFEST_V1.csv` is the controlled
fixture inventory for all 57 required rows in the six Windows matrices. Every
entry pins an immutable x64 recipe and artifact identity, process shape,
Office bitness, apartment shape, exact signature, execution recipe, owned
cleanup recipe and the six required observable axes.

This result does not implement or certify a Windows capability. Every manifest
row has `capability_credit=none`; all 57 canonical matrix rows retain
`truth_state=planned` and `evidence_state=planned`. Current source/recipe hashes
identify reproducible input bytes only. They are not built-artifact, runtime,
VM3/JIT, Excel/VBA or release-certification evidence.

## Exact coverage

| matrix | rows | current source/recipe | pending source/recipe | pending built artifacts |
|---|---:|---:|---:|---:|
| `WIN-ABI-CARRIER` | 18 | 8 | 10 | 18 |
| `WIN-COM-CLIENT` | 9 | 6 | 3 | 9 |
| `WIN-COM-EVENTS` | 7 | 1 | 6 | 7 |
| `WIN-COM-SERVER` | 7 | 4 | 3 | 7 |
| `WIN-NATIVE-IMPORT` | 8 | 0 | 8 | 8 |
| `WIN-NATIVE-EXPORT` | 8 | 1 | 7 | 8 |
| **total** | **57** | **20** | **37** | **57** |

The manifest key is `matrix_id|row_id`. Row, fixture, recipe and built-artifact
identities are unique. `fixture_id`, `process_shape`, `apartment_shape`,
`exact_signature`, environment identity and all six expectations are copied
from the canonical matrix row and checked against it; the manifest cannot
silently redefine capability truth.

## Recipe and hash semantics

### Source/recipe

Twenty rows reuse controlled current repository sources for bounded fixture or
recipe inputs: the repo-owned in-process COM dispatch/vtable fixture, current
COM matrix source, current wrapped-server source, current runtime carrier
source and the current typelib test-server source. Their
`source_recipe_hash` is SHA-256 over:

1. the immutable row recipe fields and six expectations in fixed order;
2. normalized, ordinal-sorted, deduplicated repo-relative source paths; and
3. each source file's UTF-8 bytes after BOM removal and CRLF/CR-to-LF
   normalization.

This makes source hashes stable across Git checkout EOL policy. The other 37
rows use the exact triple `source_recipe_state=pending`,
`source_recipe_paths=pending`, `source_recipe_hash=pending` and retain their
active matrix residual owner in `source_recipe_owner_bead`.

The validator rejects generated/history/evidence paths and binary extensions as
source/recipe inputs. Current source hashes therefore cannot be fabricated from
an old DLL, TLB, workbook or archived capture.

### Built artifacts

All 57 `built_artifact_state` values are `pending`, with `path=pending`,
`hash=pending` and the exact active matrix residual owner. No historical or
ad-hoc binary is reused as current proof. Each row now has an explicit artifact
contract rather than a generic future path:

| artifact class | rows | immutable file name | admitted content |
|---|---:|---|---|
| `pe-dll-x64` | 23 | `fixture.dll` | PE32+ AMD64 executable with the DLL characteristic |
| `pe-exe-x64` | 18 | `fixture.exe` | PE32+ AMD64 executable without the DLL characteristic |
| `fixture-bundle-json-v1` | 16 | `fixture-bundle.json` | exact versioned JSON schema plus the row's ordered component-type contract |

The controlled root is exactly
`artifacts/windows-x64/controlled-fixtures/v1/<matrix-id>/<row-id>` with
lower-case immutable matrix and row IDs. A `current` transition must use that
exact root/name pair. The validator rejects traversal, reparse-point escape,
mutable aliases and historical/generated/evidence locations before reading
content.

Direct binaries are parsed by .NET's portable `PEReader`, then subjected to
bounded PE32+ mapping checks: AMD64 machine, DLL-versus-EXE class, file/section
alignment, header and image sizes, raw and virtual section bounds and
non-overlap, aggregate sizes, executable entry mapping and contained data
directories. Portable policy also rejects reserved/obsolete low
`DllCharacteristics` bits, `FORCE_INTEGRITY` because this lane does not verify
Authenticode, AppContainer/WDM-driver roles, and Native/EFI/other non-desktop
subsystems. Controlled DLLs require `WindowsGui`; controlled EXEs require
`WindowsGui` or `WindowsCui`. On Windows, admission additionally requires both
`LoadLibraryExW(DONT_RESOLVE_DLL_REFERENCES)` and an
`SEC_IMAGE_NO_EXECUTE` mapping to accept the image. These checks do not invoke
the entry point or resolve dependencies.

Bundle and environment JSON bytes are first parsed by `JsonDocument` directly
from strict UTF-8. Every object is recursively walked before materialization;
duplicate property names are rejected using ordinal case-sensitive identity.
The later exact property-set checks are also ordinal case-sensitive, so a
mis-cased field cannot alias a canonical field. Bundle identity remains bound
to `matrix_id`, `row_id`, `fixture_id`, `built_artifact_id`, x64 and the bundle
class. Ordered components retain immutable IDs, exact controlled paths and
raw-byte SHA-256 values.

`msft-tlb-v1` components receive a bounded, identity-agnostic MSFT parser check
before any OS call. It admits bounded TypeInfo/name/import counts, the exact
conditional directory shape, aligned non-overlapping declared segments, and
complete fixed or variable records in their owning segments. GUID/name/string
hash links, imports, hreftypes, TypeDesc/ArrayDesc trees, custom data and
coclass reference chains must resolve to complete record starts. Every
func/var member block must live after declared segments, remain inside
`fileLength`, contain the declared records exactly, and carry complete
auxiliary member-name and record-offset arrays; member blocks may not overlap.
The parser no longer assumes one TypeInfo, the old probe LIBID/library name,
version 1.0 or an enum. On Windows, `LoadTypeLibEx(REGKIND_NONE)` must then
accept the library without registration; failure HRESULTs are rendered from
the explicit unsigned 32-bit bit pattern.

The bundle's own `component.sha256` is not treated as an independent identity
claim. `Get-WindowsFixtureArtifactContractMap` is the controller-owned source
of ordered component constraints. Its exact `WIN-ABI-CARRIER|WAC-TYPELIB-METADATA`
source row now carries
`component_constraints=n/a|sha256:9bdbc6a597d233296bd39adba69db9765552fdcce0261c6102b2e19d4d4c1a12`.
That non-serialized expected value is passed separately to generic bundle
admission and binds the TLB bytes even if an attacker rewrites both the
component and its self-reported hash. No WAC-specific identity exists inside
the generic MSFT parser, and no new canonical manifest column was introduced.
A later real-current transition must still set the canonical matrix
`fixture_hash` to the raw hash of the complete controlled bundle; deterministic
sync then supplies the same whole-bundle value as `built_artifact_hash`.

The genuine admission positives are test-only assets under
`scripts/testdata/windows-fixture-toolchain/`: the existing MSVC-linked x64
DLL/EXE and a 6,628-byte, 10-TypeInfo x64 `OxVba.TestEventServer` typelib
produced by .NET Framework 4.8.1 x64 TlbExp. The former 1,468-byte,
one-TypeInfo MIDL output remains `msft-tlb-probe-v1` for generic parser breadth
only and cannot satisfy the WAC bundle constraint. The asset manifest pins
producer, canonical source hashes, output lengths, hashes and base64 bytes.
These assets do not occupy a canonical artifact root, do not replace any
matrix hash and grant no capability credit.

Repository-scoped text provenance uses UTF-8 with BOM removal and newline
normalization to the repository's canonical LF bytes. This keeps source hashes
stable in a CRLF checkout while preserving the existing LF integration policy;
this bead does not alter `.gitattributes` or canonical EOLs. Binary asset and
bundle hashes remain raw-byte hashes.

### Environments

All 57 environment hashes remain pending because neither an immutable dev-host
capture nor the clean certification image has landed:

| environment | rows | pending owner | meaning |
|---|---:|---|---|
| `win-x64-dev-oracle-2026-07` | 12 | `bd-59co.3.1.2` | characterized, noncertifying development/oracle host |
| `win-x64-cert-vm-pending-v1` | 45 | `bd-59co.3.15.3` | clean pinned release-certification VM still blocking |

Environment hashes are separate from both source/recipe and built-artifact
hashes. Every row pins the canonical environment role, profile, target,
Office bitness, evidence state and this exact capture contract:

`artifacts/windows-x64/controlled-environments/v1/<environment-id>/environment-capture.json`

The capture must use schema
`oxvba-windows-x64-environment-capture-v1`, exact JSON properties and types,
and a versioned capture ID. It must reproduce the canonical environment ID,
role, `windows-x64` profile, x64 target, Office64 product/build/channel,
locale, OS build, evidence state, image identity and reset policy from
`IDEAL_ENVIRONMENT_MANIFEST_V1.csv`. The image must be pinned by SHA-256 and
the capture separately hashes the exact reset policy. Development-oracle
captures must remain explicitly noncertifying; certification captures must be
verified, authoritative and bind a pinned resettable snapshot/image. Thus the
current mutable development host and pending certification VM cannot be
promoted merely by hashing arbitrary notes.

The later WIN-0 reconciliation bead
`bd-59co.3.1.7` owns the environment/fixture/downstream handoff; this bead does
not edit `IDEAL_ENVIRONMENT_MANIFEST_V1.csv`.

## Six observable axes

Every row carries the matrix's independent expectations for:

| axis | required observable |
|---|---|
| result | structural value/result, not a status tag |
| full Err | number, source, description and applicable help/line state |
| side effects | writeback and externally visible effects |
| lifecycle/order | call, event, reentry, writeback and cleanup order |
| transport | dispatch/vtable/proxy/native/export route and exact signature |
| balance | carrier, reference, pin, callback, registration and process balance |

Execution recipes pin `target=x64`, the exact fixture/process/apartment and a
six-axis capture. Cleanup recipes are category-specific and limit mutation and
cleanup to recorded owned PIDs, interfaces, callbacks, files and HKCU keys.
Blanket dialog, process, registry or filesystem cleanup is not admitted.

## Fail-closed checks

The deterministic sync and validator enforce:

- the exact V1 header, six matrices and 57-row identity set;
- no missing, extra or duplicate row/fixture/recipe/artifact identity;
- immutable versioned fixture identities and explicit x64 recipe/artifact
  identities, with x86/WOW64/ARM64 and mutable `latest/current/head` identities
  rejected;
- exact per-row artifact classes, roots, names, types and ordered bundle
  component contracts, with controller-owned component digest constraints,
  portable bounded PE32+/AMD64 mapping and role/flag policy, Windows
  non-executing image-loader admission and duplicate-aware exact JSON;
- complete bounded generic-MSFT segments, references, member blocks and
  auxiliary arrays plus Windows `LoadTypeLibEx(REGKIND_NONE)` acceptance for
  typelib components;
- exact versioned environment capture roots, names and schema, bound field by
  field to the canonical Windows x64/Office64 environment and its authority
  role, image and reset policy;
- normalized sorted deduplicated source paths and LF-stable hash recomputation;
- distinct source/recipe, built-artifact and environment state/hash/owner
  triples;
- an active exact owner for every pending triple;
- nonblank process, apartment, exact-signature, execution, cleanup and six-axis
  fields; and
- `capability_credit=none` on every row.

`scripts/test-windows-fixture-manifest.ps1` proves the clean generated copy, a
legal pending-with-owner population, CRLF checkout stability and successful
full-validator admission of genuine toolchain-built x64 DLL and EXE images, an
exact bundle, the genuine 6,628-byte TestEventServer MSFT bundle, the tiny
parser-only probe and a canonical environment capture. Its 53 fail-closed
mutations run without a platform skip. Portable PE cases cover reserved flags,
unsigned `FORCE_INTEGRITY`, WDM-driver role and Native subsystem in addition to
x86/class/truncation/alignment/overlap/image-size checks; valid positives retain
the Windows image-loader gates. JSON cases cover duplicate and mis-cased
bundle-root, component and environment fields. Direct portable typelib cases
cover the eight-byte stub, segment truncation/corruption, member offset at the
file tail and a `fileLength` truncation of the member auxiliary data; a separate
full-validator case proves the controller digest rejects the otherwise-valid
tiny probe in the WAC row. Environment binding mutations continue to cover
identity, target, Office bitness, role, image, reset policy and authority flags.

## Acceptance record

The focused acceptance commands pass:

- `./scripts/sync-windows-fixture-manifest.ps1 -Check` — 57 deterministic rows,
  20 current source recipes and 37 pending source recipes;
- `./scripts/validate-windows-fixture-manifest.ps1` — exact six-matrix/57-row x64
  inventory, 57 pending built artifacts, 57 pending environments and no credit;
- `./scripts/test-windows-fixture-manifest.ps1` — eight positive observations
  and 53 cross-platform negative mutations, including real
  toolchain/Windows-loader `current` admission probes;
- `./scripts/run-truth-reconciliation.ps1` — full check-only reconciliation,
  including the new sync and validator, with no generated/controller rewrite.

The final exact-code mutation run reported
`positive=8 negative=53 windows_loader_positive_minimum=3 rows=57 capability_credit=none`.
AST parsing, canonical provenance recomputation, stale probe-hardcode search and
`git diff --check` also passed. Implementation commit: `56a1fe20`.

The sync check and positive validator are wired into governance and truth
reconciliation. The mutation suite remains a focused validator-maintenance
check.

## Residuals

- Dedicated controlled source/recipe delivery remains pending for 37 rows under
  the exact owners recorded in the manifest.
- Every built fixture remains pending; there are zero current built-artifact
  hashes and therefore zero binary-proof claims.
- Every environment hash remains pending; the dev host is noncertifying and the
  clean certification VM is not provisioned.
- Capability implementation, VM3/JIT parity, real COM/native transport and
  Excel64 certification remain with WIN-1 through WIN-14. This support artifact
  supplies inventory and ownership only.
- The broader `check-governance.ps1` currently stops before this lane at the
  pre-existing stale `docs/generated/PMR_EVENT_DIAGNOSTICS_SNIPPET.md` check.
  That generated controller surface is outside this bead and was not rewritten;
  the full truth-reconciliation gate and all fixture-specific gates pass.
