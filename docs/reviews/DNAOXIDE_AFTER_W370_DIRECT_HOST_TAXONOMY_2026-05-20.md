# DNA OxIde After-W370 Direct-Host Taxonomy Review

Date: 2026-05-20
Bead: `bd-94av.5.1`

## Scope

This review records the direct-host issue/status taxonomy hardening for:

- stale workspace and document versions;
- runtime busy/running/stopped command states;
- no-session Immediate and debug command states;
- browser unsupported response families;
- COM invocation unavailable cases.

## Delivered Stable Codes

| Category | Stable code | Intended use |
| --- | --- | --- |
| Stale workspace revision | `DH-WORKSPACE-VERSION-STALE` | Host/editor request used an older workspace revision than the current host state. |
| Stale document version | `DH-DOCUMENT-VERSION-STALE` | Host/editor request used an older document version than the current document state. |
| Runtime busy | `DH-RUNTIME-BUSY` | Runtime cannot accept the command because another runtime operation is active. |
| Runtime already running | `DH-RUNTIME-ALREADY-RUNNING` | Start/run command was issued while the runtime is already running. |
| Runtime not running | `DH-RUNTIME-NOT-RUNNING` | Continue/stop/step command requires a running runtime. |
| Runtime stopped | `DH-RUNTIME-STOPPED` | Runtime has stopped and the command requires an active runtime. |
| Immediate session unavailable | `DH-IMMEDIATE-SESSION-UNAVAILABLE` | Immediate command has no valid Immediate session. |
| Debug session unavailable | `DH-DEBUG-SESSION-UNAVAILABLE` | Debug command has no valid debug session. |
| Browser response family unsupported | `DH-BROWSER-RESPONSE-FAMILY-UNSUPPORTED` | Browser profile intentionally cannot serve a native response family such as runtime, debug, Immediate, COM, or native-service. |
| COM invocation unavailable | `DH-COM-INVOCATION-UNAVAILABLE` | COM invocation is not available in the current host/platform boundary. |

## Context Carried

`DirectHostIssueContext` now carries:

- expected and actual workspace revisions;
- expected and actual document versions;
- response family code for browser unsupported responses.

The existing typed context fields continue to carry document, runtime,
Immediate, debug, breakpoint, stack-frame, watch, project, workspace, source,
and path identities.

## Evidence

Pinned by `crates/oxvba-host/src/direct_host.rs` unit tests:

- stable code table covers all new `DirectHostIssueKind` values;
- stale workspace/document helper constructors preserve version context and are retryable;
- browser response-family helper preserves the unsupported family and is non-retryable.

COM availability now uses the more precise `DH-COM-INVOCATION-UNAVAILABLE`
code for non-Windows runtime invocation gating.

## Non-Claims

This does not claim broad COM runtime invocation support.

This does not claim a browser shell implementation for native runtime, debug,
Immediate, COM, or native-service command families. It provides the stable
typed issue vocabulary those response surfaces can use.
