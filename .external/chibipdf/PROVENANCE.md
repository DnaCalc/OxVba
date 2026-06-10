# ChibiPDF — vendored corpus source

Real-world VBA used as an OxVba compiler-acceptance corpus entry.

- **Upstream:** https://github.com/KallunWillock/ChibiPDF
- **Commit:** `40c87f379f770fc66edba1e84b4d1c5a767089d7` (branch `main`)
- **Vendored:** 2026-06-10
- **License:** MIT (see `LICENSE`; © 2026 Kallun Willock)
- **What it is:** "A dependency-free suite of PDF related tools for VBA" —
  PDF text extraction, OCR, and rendering with no Adobe/Tesseract/external
  DLLs, using Windows-native WinRT OCR via in-process COM interop.

## Why it's in the corpus

`src/ChibiEx.cls` is a single ~1280-line class module that exercises a dense
slice of the real VBA surface in one file:

- **37 `Declare` statements** across Win32 (`kernel32`/`ole32`/`oleaut32`/
  `gdiplus`/`shlwapi`/`shcore`) and WinRT (`combase`: `RoGetActivationFactory`,
  `WindowsCreateString`, `WindowsGetStringRawBuffer`), including
  `DispCallFunc` and `CopyMemory`/`RtlMoveMemory`.
- **Conditional compilation** — `#If Win64` and `#If VBA7` dual `PtrSafe` /
  legacy `Declare` blocks.
- **6 UDTs** (`GUID`, `RECTF`, `OcrLanguage`, `OcrWord`, `ocrLine`,
  `OcrResults`), `As Any` and `ByRef GUID`/`LongPtr` parameters.
- A broad public surface (`Property Get/Let`, `Optional ... As Variant`
  parameters, array returns).

This makes it a high-value **front-end acceptance** test: binding +
linearizing it cleanly proves the clean stack accepts a demanding real-world
class. Full execution needs Windows-native WinRT OCR + a PDF input and is a
later integration scenario.

The source under `src/` is verbatim from upstream; do not edit it (corpus
fidelity). Update by re-vendoring at a new pinned commit and bumping the
commit hash above.
