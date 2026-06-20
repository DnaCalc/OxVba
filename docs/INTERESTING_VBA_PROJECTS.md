# Interesting VBA Projects

This is a local watchlist of external VBA projects that are useful as design,
compatibility, interop, or stress-test inspiration for OxVBA. Inclusion is not
a compatibility claim and does not authorize copying project code into OxVBA.

Clean-room rule: use these projects only through public documentation, public
source review at a high level, and reproducible black-box behavior observations
when needed.

| Project | URL | Notes | OxVBA Interest |
| --- | --- | --- | --- |
| Riff | https://github.com/uesleibros/riff | Single-module VBA audio engine for Office/VBA hosts, using Windows audio/media APIs, direct COM vtable calls, conditional 32-bit/64-bit declarations, runtime machine-code callback thunks, static UDT state, and DSP/audio-buffer workloads. | Stress case for `Declare`/Win32 interop, COM vtable dispatch idioms, pointer-heavy VBA, conditional compilation, arrays/UDTs, timers/callbacks, and host-reset safety behavior. |
| Wasabi | https://github.com/uesleibros/wasabi | Single-module VBA WebSocket, MQTT, and raw TCP client for Office/VBA hosts, using Windows socket/TLS APIs, Schannel-style security surfaces, async event dispatch, middleware/compression extension hooks, and conditional x86/x64 support. | Stress case for Win32 networking declarations, byte-array and string transport, async callback/event patterns, object lifetime requirements for handler classes, `DoEvents`-driven host behavior, TLS/proxy configuration surfaces, and long-running Office host interop. |
| Awesome VBA | https://github.com/sancarn/awesome-vba | Curated list of VBA/VB6 frameworks, libraries, tools, examples, and resources, including category and platform/host/architecture symbols. | Source for future candidate projects and idiom coverage across JSON/CSV/XML, data structures, math, database, userforms, low-level APIs, parsers/interpreters, web tools, add-ins, games, Win32 resources, and developer tooling. |

## Showcase and Proofing Candidates

These candidates were identified as possible larger proofing grounds for the
compiler and COM/runtime surface. The intent is to use them as external
behavioral targets, compatibility probes, and gap-finding corpora, not as source
to copy.

| Candidate | URL | Why It Fits | Caveats |
| --- | --- | --- | --- |
| Project Explorer | https://github.com/leonunezcl/pexplorer | Ambitious VB6 desktop tool that analyzes VB4/VB5/VB6 project source. It lists explicit OLE Automation, `TLBINF32.DLL`, custom ActiveX DLL, `MSCOMCTL.OCX`, and `RICHTX32.OCX` dependencies, and should stress forms, modules, classes, declarations, enums, arrays, file IO, string parsing, project metadata, and COM/ActiveX references. | GPL-3.0; Spanish docs/comments likely; custom `PVB_XMENU.DLL` dependency may need isolation or stubbing for repeatable proofing. |
| VB Migration Partner code samples | https://github.com/codearchitects/vbmigration-code-samples | Broad VB6 sample corpus advertised as about one hundred examples and 2 MB of source, covering advanced graphics, OOP, database programming, forms/controls, databinding, OLE drag-and-drop, custom user controls, COM components, ADO data source/consumer classes, and Windows API methods. | Better as a regression gauntlet than as one coherent showcase application. |
| ExcelAddin4Atlassian | https://github.com/dagiz007/ExcelAddin4Atlassian | Real Excel `.xlam` add-in with Jira/Confluence workflow, worksheet formulas, settings/authentication UI, HTTP/JSON plumbing, and Excel object-model usage. Good Office/VBA host proofing lane. | External Atlassian APIs make runtime oracles harder unless the HTTP boundary is mocked or localized. |
| VBA-Web | https://github.com/VBA-tools/VBA-Web | Mature VBA web-service library with authentication, JSON, cookies, headers, request/response abstractions, class modules, dictionaries/collections, optional parameters, object factories, late-bound COM, and error handling. | Library rather than end-user app; cross-platform design may avoid some Windows COM paths. |
| Barcode Function Library Excel Add-In | https://github.com/EszopiCoder/excel-barcode-fx-library | Excel add-in that generates 1D/2D barcodes using Excel autoshapes. Useful for Excel object model automation, algorithmic string/array work, and visual/value-checkable output. | GPL-3.0; likely less COM-reference-heavy outside Excel itself. |

Recommended tiering:

1. Use Project Explorer as the ambitious showcase target.
2. Use the VB Migration Partner samples as the broad legacy-language and COM
   regression gauntlet.
3. Add ExcelAddin4Atlassian or VBA-Web as the Office/VBA-specific proofing lane.

The combination gives a real VB6 application, broad legacy language coverage,
COM/ActiveX references, Excel/VBA add-in behavior, and enough surface area to
expose compiler/runtime gaps beyond handpicked fixtures.
