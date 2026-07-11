# CORE-1 Cross-Platform EOL and Snapshot Contract

Date: 2026-07-11
Bead: `bd-59co.2.2.4`
Base: `c94d1e9a85b510ef16479448a7a3686c98606bb3`
Matrix route: `CORE-READINESS/CORE-BASELINE-EOL-SNAPSHOT`
Clause: `CONF-QUALITY-001`

## Result

The branch now has one explicit repository line-ending contract: every
Git-detected text file is stored and checked out with LF, while the source,
snapshot, documentation, evidence, and control-file families are forced text
instead of relying on content heuristics. Known product/Office/native artifacts
are forced binary. One captured UTF-16/terminal evidence file whose `.txt`
suffix is misleading has an exact path-scoped binary exception.

`scripts/validate-line-endings.ps1` implements the byte-exact V1 contract and
fails closed when the root attributes are missing, changed in the working tree
or index, untracked, non-regular, overridden by a nested attributes file, or
produce a non-LF tracked text state. It checks both index and working-tree EOL
classifications, scans forced text for raw carriage-return bytes, and requires
non-vacuous source, snapshot, and documentation witnesses.

The three authoritative differential snapshots have unchanged Git blob IDs and
unchanged EOL-normalized content. The change is checkout transport only; no
expected result was regenerated or blessed.

## Authoritative snapshot identity

The `pre working` values capture the original Windows checkout with
`core.autocrlf=true`. `Post/index` is the LF byte stream now present in both the
index and working tree. The semantic hash normalizes CRLF and bare CR to LF
before SHA-256.

| snapshot | pre/post Git blob | pre working raw SHA-256 | post/index raw and semantic SHA-256 | pre/post bytes |
|---|---|---|---|---|
| `crates/oxvba-differential/jit_linux_safe_scope.snap` | `ba5e6e502da0d5c75eee4993c6eaa9d7c47faa68` | `c0416f5c59919b17fa7722bf3d6041c39be5ef8c3db338c30521e9b4f51a3cf4` | `9e5430b2e27590bcce76fa97d7116b37f99198e886b9bde44be84f8993018342` | `440 / 430` |
| `crates/oxvba-differential/jit_scope.snap` | `a5fca8bfe149c6100c27a6554e0700d685445d0c` | `d40984fd2daa9574635147ad3afbd484f1254421b06633229d33f52b57efe4fd` | `8a9d843dac1102a77f6eaa7d64b9f98d7ca48e0c0046d122b3b5752c81b8f088` | `63399 / 62239` |
| `crates/oxvba-differential/vm3_golden.snap` | `1cbe766d34d9d978ee5c6c821811db8b4c45d92d` | `a011d4019fe5f8f3e805542778714c3f4e1e9e65f5dc1220e7439bb3126fab52` | `24294e5b9d05cc4d86156020a9bb7101b99ed08506e4235a722f81921adf8584` | `172063 / 171418` |

For all three rows, the post index SHA-256, post working SHA-256, and pre/post
semantic SHA-256 are the same value shown in the fourth column.

## Required historical text normalization

Four historical text blobs contained either bare carriage returns or one final
CRLF. Git therefore classified some of them as binary-like even after a forced
text attribute. They were mechanically normalized because the V1 contract
cannot truthfully claim exact LF while retaining those bytes.

Fresh review also found one trailing ASCII space in three of the files. None was
a Markdown hard break, so those three spaces were removed as nonsemantic
whitespace cleanup. `STATUS_TOUR_2026-02-27_145257.md` remains the exact EOL-only
case: its final raw SHA-256 equals its pre-normalized SHA-256. The other three
rows record distinct pre-normalized and final hashes rather than overstating
byte equality. No prose, expectation, or evidence meaning changed.

| path | cleanup | pre/post blob | pre raw SHA-256 | pre normalized SHA-256 | final raw = final normalized SHA-256 | bytes |
|---|---|---|---|---|---|---|
| `docs/archive/status-tours/STATUS_TOUR_2026-02-27_142354.md` | EOL + one trailing space | `2146a048043009db6dc7c9d09edf848582df25c8` / `ef4ca9093c87e3a6461ca43424fdaa05af69a983` | `9a3d113843926c07096d6b6bbb2801904785546060a7416098e620748ad55c14` | `2edae572f23fc2b7acc1898144151fd04aed3044bb0d271e0269a28efd51ed2c` | `a07675e7172362591ed39fa6e3017bd05651b2cfa25c1b7cc45012967d02342c` | `1248 / 1246` |
| `docs/archive/status-tours/STATUS_TOUR_2026-02-27_145257.md` | EOL only | `b579754fc5a255b23f4e95b1e6a16412b0f254dc` / `30fbfdeec5c2c393bef315b02fc7d7c8f7b542b5` | `7170849e4c4bad6571fd078f5f14cd2b32182389594ecaf1de91559ec3cc01e9` | `5fc03cad790e21aba07e42f55e7a4a660d197a98fdf2ab9d765670a5132860fe` | `5fc03cad790e21aba07e42f55e7a4a660d197a98fdf2ab9d765670a5132860fe` | `1012 / 1011` |
| `docs/evidence/conformance/com_early/COM_EARLY_OPEN_QUESTIONS_V416.md` | EOL + one trailing space | `8418df94cb7ede543e1f8c0d0560725c9b9ee629` / `daf9c08ce158b898da1492636ec4928771b7b361` | `7b66bfc6eef6f23cd403b37598ebfed1b0e967b462ef32fd8ebcaa8351132562` | `cf47e31b8812c1ef9e247b82d3a45808b896241d102c884947d54cf3c57b9961` | `5d58f6d6f3d8fb73c85fd0491fc331358dd0bd367c4831c9b56e5135c6e3d0c9` | `1139 / 1137` |
| `docs/evidence/language/COVERAGE_AUDIT_V178.md` | EOL + one trailing space | `73c49c5027a1092622d8a24555e66da7811f2022` / `7001caed9018911c156eb7e0d2f23ea5dbe4c100` | `2107d4901079a1bdd7b73118863067fb826b5611513024ec6731568679300707` | `d591b268686e3d78aabb14f684f5359cb1fa03dd42ccfb9b42d492edb3871941` | `b399d9d607fdf0333a454f24e9c56943e7cc8927a3e2e1d1b1b2e2d34c1f9e8f` | `546 / 544` |

The opaque capture
`docs/evidence/conformance/com/COM_LANE_L2E_LOG_OxVba.TestEventServer_20260309T000005Z.txt`
was not normalized. Its HEAD/index blob remains
`95dfeaa76b142982720475e9f110438c8b76e768`, its working SHA-256 remains
`a93edf76219d50851a27e52fe656c477493db2b5604892f42d6d283878bc6961`,
and Git reports `i/-text w/-text attr/-text`.

## Windows fixture cross-lane proof

During integration, the Windows fixture lane exposed `fixture.c` and
`fixture.idl` as LF in the index but CRLF in the primary Windows working tree,
while its asset manifest pinned LF raw SHA-256 values. V1 therefore forces both
`*.c` and `*.idl` to LF. The mutation suite creates CRLF inputs for both types,
commits their normalized LF blobs, and verifies fresh checkouts with
`core.autocrlf=true` and `false`. The controller still needs to re-materialize
the already-integrated fixture working files after this attributes commit lands;
that is transport reconciliation, not an asset-content change.

## Observable axes

| axis | evidence |
|---|---|
| result | `jit_scope_snapshot`, `linux_safe_jit_scope_snapshot`, and `vm3_golden_snapshot` all pass against the unchanged snapshot blobs. |
| full Err | No VBA error expectation changed. The aggregate VM3 golden, including full Err fields, passes with `INSTA_UPDATE=no`. |
| side effects | Mutation tests use owned repositories below the system temp root and delete them after every run. The isolated Cargo target used for the focused probes was path-checked and removed. No source, snapshot, or external fixture was generated. |
| lifecycle/order | Runtime/session lifecycle code is untouched. The focused snapshot tests execute their existing compile/run/capture order; only checkout transport changed. |
| transport | Git reports LF/empty for ordinary governed text. A control-bearing forced-text artifact that Git heuristically labels `-text` is raw-scanned in both index and worktree for CR/NUL; opaque binary paths require explicit exceptions. Byte-exact root attributes, absence of nested overrides, LF/CRLF input paths, and malformed/mutable states are tested. |
| balance | Not applicable to repository transport. No runtime carrier or allocation/free path changed, and this bead makes no new resource-balance claim. |

## Checks

Environment: Windows x64 development host, Git `core.autocrlf=true`; Rust/Cargo
stable. The initial and final-review Cargo probes used fresh isolated temp
targets `oxvba-target-bd-59co-2-2-4` and
`oxvba-target-bd-59co-2-2-4-review-repair`; both were removed afterward. The
final review first encountered an orphaned shared-target wait: process audit
found the owned Cargo waiter had no compiler/test child and no other Cargo,
Rust, or rustdoc owner. Only that owned waiter was terminated before the clean
isolated rerun.

```text
./scripts/validate-line-endings.ps1
PASS: V1; 4,542 tracked files. Ordinary governed text is LF or empty; forced control-bearing text is raw CR/NUL-free; every binary exception is `-text`.

./scripts/test-line-endings.ps1
PASS: 5 positive paths and 8 mutations. Positive paths include LF input, Windows-autocrlf and LF fresh checkouts, and CRLF source/snapshot/doc/C/IDL input normalized into an LF checkout. Mutations cover CRLF working/index snapshots, a NUL-bearing forced-text snapshot, missing/nested/conflicting attributes, and independent working/index attribute changes.

git ls-files --eol
PASS: no i/crlf, i/mixed, w/crlf, or w/mixed tracked state remains.

git diff --cached --check
git diff c94d1e9a85b510ef16479448a7a3686c98606bb3 --check
PASS: the narrow review repair and the full bead commit range have no whitespace errors.

cargo test -p oxvba-differential jit_scope_snapshot
PASS: `tests::jit_scope_snapshot` 1/1 and `linux_safe_jit_scope_snapshot` 1/1; no snapshot changed.

INSTA_UPDATE=no cargo test -p oxvba-differential vm3_golden_snapshot -- --nocapture
PASS: `tests::vm3_golden_snapshot` 1/1; no `.snap.new` and no bless.

./scripts/check-governance.ps1
PASS: line-ending validation/mutations plus every existing governance, traceability, rollout, negative-validator, and derived-summary gate.
```

## Residual boundary

- This Windows run simulates both checkout policies but does not substitute for
  the accepted Linux CI baseline/certification lanes.
- The Windows fixture lane must re-materialize its already-present `.c`/`.idl`
  working files after integration so their raw bytes match the LF asset hashes.
- No semantic snapshot residual was observed on this base. Broader compiler,
  VM3, JIT, oracle, and balance parity remains with its existing program owners;
  this transport bead does not close those capability rows.
