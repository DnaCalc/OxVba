# Windows fixture admission test assets

These are test-only parser/loader probes. They are not canonical built fixtures,
do not supply a matrix hash and confer no capability or certification credit.

`assets-v1.json` contains base64 forms of binaries generated from fixed,
hash-pinned sources:

- `fixture.c` was compiled with MSVC x64 `cl /c /GS- /O1 /Zl`, then linked with
  `link /machine:x64 /nodefaultlib /Brepro`. The DLL uses `/dll /noentry`; the
  EXE uses `/entry:fixture_main /subsystem:console`.
- `fixture.idl` was compiled with Windows SDK MIDL `/env x64` and the SDK
  `um`/`shared` include roots. Its 1,468-byte output is retained only as the
  one-TypeInfo `msft-tlb-probe-v1` generic-parser probe; it cannot satisfy a
  controlled bundle row.
- The repository `tools/OxVba.TestEventServer` project was built with
  `PlatformTarget=x64`, then exported by .NET Framework 4.8.1 x64 `TlbExp`
  4.8.9037.0. Its 6,628-byte, 10-TypeInfo output is `msft-tlb-v1`, pinned at
  SHA-256 `9bdbc6a597d233296bd39adba69db9765552fdcce0261c6102b2e19d4d4c1a12`.

Recorded producers are MSVC 14.51.36231, Windows SDK 10.0.26100.0, and the
TlbExp lane above. The manifest pins each adjacent or repository-scoped source
SHA-256 using the repository's canonical UTF-8/LF text bytes, plus every output
length, SHA-256 and byte payload. Checkout CRLF is normalized only for source
provenance; binary hashes remain raw. Integration therefore depends on keeping
the existing canonical LF blobs and does not change repository EOL policy. The
mutation suite verifies those values, then requires the portable PE/MSFT
parsers and, on Windows, the non-executing image loader or
`LoadTypeLibEx(REGKIND_NONE)` to accept the decoded bytes.
