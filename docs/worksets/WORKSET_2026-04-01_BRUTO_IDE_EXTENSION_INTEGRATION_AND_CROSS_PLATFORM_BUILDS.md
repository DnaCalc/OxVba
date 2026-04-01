# Workset: Bruto-IDE Extension Integration and Cross-Platform Builds

Date: 2026-04-01

## 1. Purpose

This workset defines the execution path for adding a Bruto-IDE integration as an extension project inside the OxVba repository, then providing:
- a native Windows x64 release build
- a simple native Linux x64 source-build path

The goal is not to route Bruto through LSP.
The goal is to use the direct OxVba project and language-service APIs in the host style that Bruto actually expects.

## 2. External Contract

The current Bruto-IDE repository describes a pluggable TUI IDE whose integration point is a `Language` trait with:
1. `name`
2. `file_extension`
3. `sample_program`
4. `create_highlighter`
5. `build(source: &str) -> Result<BuildResult, String>`

The Bruto README also says the host binary should call:
- `bruto_ide::ide::run(Box::new(YourLanguage))`

So the near-term OxVba integration target is:
1. a Bruto language-extension crate in this repo,
2. a small Bruto/OxVba binary crate in this repo,
3. direct use of OxVba project-loading / compile / run / diagnostics APIs,
4. no claim of full debug parity unless proved.

## 3. Problem Statement

We now have:
1. a direct OxVba language-service core,
2. thin `oxvba-lsp`,
3. host project-helper APIs,
4. canonical project-loading helpers.

But we do not yet have:
1. a Bruto-compatible integration crate,
2. a direct-host sample that exercises OxVba through Bruto’s language trait,
3. a packaged Windows x64 build plus a simple Linux x64 native-host build path.

Without this work, the direct-host story remains documented but not demonstrated in a real external host shell.

## 4. Design Policy

### 4.1 Direct embed, not LSP

Bruto integration should use:
1. `oxvba-project` for workspace/project loading and project helper operations,
2. `oxvba-languageservice` for diagnostics and semantic queries where relevant,
3. existing compile/run surfaces for build execution.

Bruto integration should not depend on `oxvba-lsp` unless a later transport experiment explicitly needs it.

### 4.2 Honest capability slice

The first Bruto slice should only claim what the current Bruto trait can honestly support through OxVba:
1. language registration,
2. syntax highlighting,
3. sample program generation,
4. build/compile diagnostics,
5. bounded run/build invocation where the host contract allows it.

Do not claim:
1. full debugging parity,
2. live semantic IDE parity,
3. complete project authoring inside Bruto,
4. complete multi-file workspace management unless implemented and demonstrated.

### 4.3 Repo-local extension project

The Bruto integration should live as one explicit extension project in this repository.

Expected shape:
1. one library crate implementing the Bruto `Language` contract for OxVba,
2. one binary crate wiring that language into `bruto_ide::ide::run(...)`,
3. clear separation between host adapter glue and OxVba core logic.

### 4.4 Cross-platform build target

This workset owns native buildability for:
1. `x86_64-pc-windows-msvc`
2. `x86_64-unknown-linux-gnu`

The acceptance target is:
1. a successful Windows x64 release build produced in this repo,
2. a simple Linux x64 native-host build path published in-repo,
3. explicit notes on runtime or packaging caveats.

## 5. Desired End State

This workset is complete when:
1. a Bruto/OxVba extension crate exists in-repo,
2. a Bruto/OxVba binary exists in-repo,
3. the extension uses direct OxVba APIs rather than LSP,
4. the bounded supported feature set is documented honestly,
5. a Windows x64 release build exists and the Linux x64 native-host build path is published,
6. usage/build notes are published for evaluating the integration.

## 6. Execution Plan

### Phase A. Contract review and integration design

1. confirm the current Bruto `Language` trait and binary wiring model,
2. map each Bruto hook to the best OxVba direct API,
3. define the bounded first feature slice and explicit non-goals.

### Phase B. Extension project scaffold

1. add the Bruto language-extension crate,
2. add the Bruto/OxVba binary crate,
3. add workspace wiring and dependency boundaries.

### Phase C. First usable host slice

1. provide `name`, `file_extension`, and `sample_program`,
2. implement syntax highlighting,
3. implement `build(source)` using OxVba compile/diagnostic surfaces,
4. keep error reporting deterministic and host-friendly.

### Phase D. Cross-platform build execution

1. add build instructions / scripts for Windows x64 and Linux x64,
2. run the Windows x64 release build,
3. publish the Linux x64 native-host source build path,
4. document any target-specific caveats or missing runtime affordances.

### Phase E. Documentation and evidence

1. publish a short integration guide,
2. describe the direct-host boundary and why Bruto uses direct APIs rather than LSP,
3. record the build evidence and supported scope honestly.

## 7. Bead Root

Execution proceeds through a new bead subtree rooted at `bd-br1`.

Initial intended shape:
1. `bd-br1.1` publish the workset and roll out the bead graph,
2. `bd-br1.2` review the Bruto trait contract and define the bounded OxVba integration surface,
3. `bd-br1.3` scaffold the Bruto/OxVba extension and binary crates,
4. `bd-br1.4` implement the first build/diagnostic path over direct OxVba APIs,
5. `bd-br1.5` add highlighting, sample program, and host polish,
6. `bd-br1.6` produce a Windows x64 release build and publish the Linux x64 native-host build path,
7. `bd-br1.7` publish docs and build evidence.

## 8. Acceptance Statement

At the end of this workset, OxVba should be able to say:

1. there is an in-repo Bruto integration project,
2. it uses the direct OxVba host/language-service/project surfaces rather than LSP,
3. it has a produced Windows x64 release build and a simple Linux x64 native-host build path,
4. its supported scope is documented honestly.

## 9. Current Boundary Note

The first bounded Bruto integration surface is documented in:
- `docs/BRUTO_INTEGRATION_BOUNDARY.md`

That document is the design lock for:
1. using direct APIs rather than LSP,
2. starting with `.bas` as the primary extension,
3. treating Bruto build integration as a one-file build/diagnostic path first,
4. deferring broader multi-file/project-depth/editor-parity claims until separately implemented.
