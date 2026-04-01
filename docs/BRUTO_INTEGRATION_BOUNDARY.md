# Bruto Integration Boundary

This document defines the first bounded OxVba integration surface for Bruto-IDE.

It is based on the current Bruto upstream contract:

```rust
pub trait Language {
    fn name(&self) -> &str;
    fn file_extension(&self) -> &str;
    fn sample_program(&self) -> &str;
    fn create_highlighter(&self) -> Box<dyn SyntaxHighlighter>;
    fn build(&self, source: &str) -> Result<BuildResult, String>;
}
```

The Bruto binary entry point is:

```rust
bruto_ide::ide::run(Box::new(YourLanguage))
```

Source:
- <https://github.com/aovestdipaperino/bruto-ide>

## Direct-API Rule

The Bruto integration must use direct OxVba APIs.

It should use:
- `oxvba-project` for canonical project/workspace loading and host project helpers
- `oxvba-languageservice` for diagnostics and semantic analysis where useful
- existing compile/build surfaces for build execution

It should not use:
- `oxvba-lsp`
- LSP-shaped transport/session logic
- a second project model

## Hook Mapping

### `name()`

Bounded OxVba mapping:
- return a stable host label such as `OxVba`

This is host presentation only.

### `file_extension()`

Bounded OxVba mapping:
- return `bas`

First-slice rationale:
- Bruto’s contract appears single-primary-extension oriented
- `.bas` is the cleanest minimal entry surface
- `.cls` / multi-file project workflows should be treated as later expansion, not hidden first-slice behavior

### `sample_program()`

Bounded OxVba mapping:
- return a minimal procedural `.bas` sample that compiles under the default local CLI/runtime posture

Candidate shape:
- `Sub Main()`
- one simple output statement

This should be intentionally minimal and not require a `.basproj`.

### `create_highlighter()`

Bounded OxVba mapping:
- provide a Bruto syntax highlighter for core OxVba/VBA lexical categories

First-slice categories:
- keywords
- identifiers
- comments
- strings
- numbers

Important boundary:
- this does not require semantic classification parity on day one
- semantic token parity remains a later concern

### `build(source)`

Bounded OxVba mapping:
- treat the Bruto editor buffer as a one-file OxVba source input
- build it through direct OxVba compile/project surfaces
- return deterministic diagnostics/build output to Bruto

First-slice behavior:
1. synthesize or load a bounded in-memory single-file executable project
2. compile/analyze the source
3. return build success/failure and diagnostics

Near-term note:
- this begins as a build/diagnostic path first
- bounded run wiring may be added only when it is implemented through the same direct-host boundary and proved end-to-end

## First Supported Slice

The first honest Bruto/OxVba slice should support:
1. launching Bruto with an OxVba language integration
2. editing `.bas` source
3. lexical syntax highlighting for core OxVba/VBA token categories
4. sample program generation
5. deterministic build diagnostics for a single-file executable-style source
6. bounded single-file run execution via the Bruto build contract, with captured console output

## Explicit Non-Goals For The First Slice

The first slice does not claim:
1. full multi-file `.basproj` authoring inside Bruto
2. `.cls` / class-module editing parity
3. LSP integration
4. full semantic IDE parity
5. debugger parity with OxVba runtime semantics
6. complete project/reference management inside Bruto

## Next Implementation Implication

Because of this boundary, the next scaffold bead should create:
1. one Bruto language crate for OxVba
2. one Bruto/OxVba binary crate
3. a minimal one-file build path over direct OxVba APIs

Only after that should broader host polish or project-depth expansion be considered.
