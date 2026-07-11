# OxVba Compiler and Semantic Analysis Contract V2

Date: 2026-07-10
Status: current architecture contract
System clauses: `SRC-*`, `SYN-*`, `PROJ-*`, `COMP-*`, `IR-CORE-001`, `LS-FACT-001`
Supersedes: `OXVBA_FRONTEND_AND_CORE_IR_CONTRACT_V1.md` and `HIR_RESOLUTION_ENVIRONMENT_V1.md` for current architecture

## 1. Target state

The OxVba compiler turns a real project/reference closure into one compiler-owned analysis result and, when valid, one resolved CoreProgram per project. The same syntax, symbols, binding decisions, types, diagnostics and provenance feed production compilation and language-service analysis.

The compiler is tolerant at the analysis boundary and strict at the executable boundary. Incomplete editor text can produce partial/unknown facts and diagnostics; only an error-free result can produce Core IR for elaboration.

## 2. Pipeline

```text
decoded project modules and reference closure
  -> target-aware conditional compilation
  -> lossless CST of each active-source view
  -> declaration scan and provider composition
  -> scope/name/type/call binding
  -> AnalysisResultV1
       - semantic facts
       - diagnostics
       - source/virtual provenance
       - CoreProgram? (valid source only)
```

`oxvba-project` owns file/project decoding and closure construction. `oxvba-symbol` owns preprocessing, declarations, providers and resolution. `oxvba-syntax` owns CST structure. `oxvba-bind` owns typed binding, coercion/call decisions and Core IR emission. Language-service consumers index the resulting facts; they do not rerun these stages with editor-specific rules.

## 3. Source and preprocessing

Every supplied module has stable project/module/document identity, original encoding facts and a source-provenance chain. File decoding rejects unsupported or malformed byte sequences with diagnostics before the lexer receives `str` input.

Conditional preprocessing evaluates project constants, `#Const`, target constants and directive expressions according to VBA rules. Length-preserving blanking may retain module coordinates, but malformed directives or expressions fail closed. Project-generated/normalized text carries explicit original or virtual-source mapping.

The lexer is UTF-8 safe for its supplied text and never slices at invalid character boundaries. The supported VBA identifier and source-encoding policy is explicit rather than accidentally ASCII-only.

## 4. Syntax and recovery

The CST preserves all supplied active-view text, including trivia, comments, attributes, continuations and incomplete constructs. Parser entry points that require a total expression reject trailing tokens; recovery entry points retain bounded partial structure and diagnostics.

Parser recovery cannot silently convert malformed conditional syntax or declarations into a different valid program. Every accepted grammar family maps to clean-stack fixtures and compiler routes.

The grammar authority remains [`VBA_GRAMMAR_V1.md`](VBA_GRAMMAR_V1.md), reconciled with MS-VBAL and reproducible VBA observations.

## 5. Resolution environment

One resolution environment composes providers in explicit VBA precedence:

- procedure and local scopes;
- active module and sibling project modules;
- referenced source projects;
- referenced verified OxImage export surfaces;
- VBA base library;
- host-injected references;
- COM typelibs;
- source `Declare` declarations.

All providers publish stable source- or metadata-provenanced symbols through the same symbol/signature vocabulary. Provider origin affects visibility, mutability and navigation, not the fundamental binding algorithm.

Ambiguous case-insensitive identities, duplicate exports, reference diamonds, missing references and inaccessible/private symbols diagnose deterministically. Referenced source and compiled artifacts expose equivalent public callable, class and data surfaces.

## 6. Analysis result and identities

The public analysis mode is a closed enum with exactly `AnalysisMode::Strict` and `AnalysisMode::Editor`. Strict mode diagnoses any source that cannot produce executable semantics. Editor mode permits bounded recovery and poison/unknown facts for incomplete source, but it does not introduce a second grammar, binder or identity model.

Every project analysis entry point returns one immutable, schema-versioned `AnalysisResultV1`. Its required logical shape is:

```text
AnalysisResultV1
  schema/mode/target identity
  project, module, document and provider identities plus versions/digests
  lossless syntax/CST and preprocessing context
  explicit scope tree
  declarations, signatures and use-site bindings
  expression, place, member, call and result types
  argument mapping and ByVal/ByRef/Optional/named/omitted/ParamArray facts
  accessor, property, default-member and invoke-kind decisions
  diagnostics with primary/related locations
  original, normalized and generated/virtual provenance maps
  Option<CoreProgram>
```

The version suffix is part of the public compatibility contract: adding or changing required fact meaning creates a new version or an explicit compatible extension. Consumers cannot mutate facts, substitute their own identities or retain result-local handles as if they belonged to another result.

The compiler-owned analysis result contains:

- syntax trees and preprocessing context;
- project/module/document identities;
- declarations, scopes, signatures and declared types;
- every identifier/member/call use binding;
- expression, place, member and result types;
- argument-to-parameter mapping;
- ByVal/ByRef, Optional, omitted, named and ParamArray facts;
- property/default-member/accessor and invoke-kind decisions;
- dispatch/import/export provenance;
- diagnostics with primary and related locations;
- optional CoreProgram.

Project, module, document and provider IDs are collision-checked stable identities within the supplied closure and carry the version or digest needed to distinguish changed inputs. Compiler symbol IDs are stable within the analysis result. Consumers create snapshot-bound handles and deterministic logical keys for cross-snapshot equivalence; name alone is never identity.

Valid-source Strict and Editor analysis of the same closure and target produce identical syntax identities and semantic facts. `CoreProgram` is present only when the result contains no executable-blocking diagnostic or poison/unknown semantic fact. The executable pipeline accepts that `CoreProgram` from `AnalysisResultV1`; it never bypasses the result through a second analysis route.

## 7. Types, calls and coercion

Declared return, parameter, array element, UDT, object/interface and fixed-string types survive provider publication and binding. A call does not become Variant merely because its declaration originates outside the active source project.

One callable-signature model covers project procedures, properties, library members, host members, COM members and Declare procedures. Compile-time legality and runtime availability are distinct: for example, a missing DLL/export is not a compiler reference error when the declaration is otherwise legal.

The detailed semantic authorities are:

- [`VBA_TYPE_SYSTEM_V1.md`](VBA_TYPE_SYSTEM_V1.md);
- [`VBA_EXPRESSION_CALL_SEMANTICS_V1.md`](VBA_EXPRESSION_CALL_SEMANTICS_V1.md);
- [`VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md`](VBA_SEMANTIC_TABLES_AND_BINDING_REFERENCE_V1.md).

## 8. Core IR boundary

Core IR is a resolved semantic tree, not bytecode and not a language-service model. It contains no unresolved source names and makes implicit VBA behavior explicit:

- values and addressable places;
- Let/Set and assignment target intent;
- coercions and declared storage;
- static, dynamic, library, COM and native callees;
- property/default-member chains;
- ByRef aliases and call-site descriptors;
- arrays, records, objects and class/event metadata;
- error statements and source positions;
- project imports and exports.

Core IR remains backend-neutral. `oxvba-oxir::elaborate` owns conversion into typed CFG OxIR; VM3 and JIT never bind source constructs directly.

## 9. Diagnostics and provenance

Each syntax, symbol and binding diagnostic has a stable code, phase, severity, primary span and related spans. Diagnostics point to original module text when possible and to an explicit virtual/generated document otherwise.

Every compiler span is a half-open UTF-8 byte range in its identified, versioned supplied active-view document. A span boundary must be a valid UTF-8 boundary. Versioned maps relate active-view bytes to the original document or to an explicit normalized/generated virtual document; line/column or client-position encodings are projections and never replace compiler byte offsets.

Preprocessing, startup/mainline generation, class preamble handling and line-ending normalization cannot leave a runtime or compiler location with ambiguous provenance. Runtime source maps extend the same identity chain into OxIR/OxImage.

## 10. Completion evidence

Compiler completion requires:

- full grammar and declaration matrices;
- typed call/coercion and project/reference matrices;
- no-panic/fuzz coverage for decoded text and valid UTF-8 lexer/parser input;
- source/virtual provenance snapshots;
- `AnalysisMode::Strict`/`AnalysisMode::Editor` fact equality for valid source;
- Unicode, CRLF, astral-character and generated-source tests for half-open UTF-8 spans and source-map round trips;
- current Excel/VBA compile diagnostics and runtime timing where behavior is observable;
- no accepted route through legacy source surgery, deleted HIR or a second semantic model.
