# Windows fixture admission test assets

These are test-only parser/loader probes. They are not canonical built fixtures,
do not supply a matrix hash and confer no capability or certification credit.

`assets-v1.json` contains base64 forms of binaries generated from the adjacent
fixed sources:

- `fixture.c` was compiled with MSVC x64 `cl /c /GS- /O1 /Zl`, then linked with
  `link /machine:x64 /nodefaultlib /Brepro`. The DLL uses `/dll /noentry`; the
  EXE uses `/entry:fixture_main /subsystem:console`.
- `fixture.idl` was compiled with Windows SDK MIDL `/env x64` and the SDK
  `um`/`shared` include roots.

Recorded producer: MSVC 14.51.36231 and Windows SDK 10.0.26100.0. The manifest
pins each source SHA-256 plus every output length, SHA-256 and byte payload. The
mutation suite verifies those values, then requires the portable PE/MSFT
parsers and, on Windows, the non-executing image loader or
`LoadTypeLibEx(REGKIND_NONE)` to accept the decoded bytes.
