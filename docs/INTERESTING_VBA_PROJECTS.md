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
