# AOT Compilation with Cranelift: Windows Linking and DLL Export Design

**Date:** 2026-06-20
**Status:** Design note / discussion record (no implementation yet)
**Scope:** How Cranelift can be used as an AOT backend for a contained-scope
language on Windows, how to avoid requiring a system linker at user build time,
and how to produce a real exported `.dll` without a conventional link step.

---

## 1. Background: Cranelift as a library

Cranelift (Bytecode Alliance) is a fast code-generation backend written in Rust.
It takes an intermediate representation (IR) and emits native machine code for
x86-64, ARM64, RISC-V, s390x. It prioritizes **compilation speed** over peak
runtime performance (contrast with LLVM). It ships entirely as Rust crates;
there is no separate compiler process to invoke.

### Crates

| Crate | Role |
|---|---|
| `cranelift-codegen` | Core IR, compilation pipeline, ISA backends. |
| `cranelift-frontend` | `FunctionBuilder` — ergonomic IR construction with automatic SSA. |
| `cranelift-module` | `Module` trait: symbol/linkage abstraction shared by the backends. |
| `cranelift-jit` | In-memory JIT; returns callable `fn` pointers. |
| `cranelift-object` | Emits relocatable object files (ELF/Mach-O/COFF) for offline linking. |

Typical usage needs `cranelift-codegen` + `cranelift-frontend`, plus *either*
`cranelift-jit` (execute in-process) *or* `cranelift-object` (emit an object).

### Pipeline shape

1. **Target ISA** — build from `settings` + `target_lexicon` (e.g. `Triple::host()`).
2. **Build IR** — `FunctionBuilder`: blocks, SSA values, typed instructions
   (`iadd`, `imul`, `load`, `call`, `brif`, `return_`, ...).
3. **Compile** — via a `Module` (`define_function`).
4. **Use result** — JIT: `fn` pointer; object: bytes to link.

`cranelift-module::Module` is the key abstraction: it manages declarations,
linkage (`Export`/`Import`/`Local`), cross-function references, and a single
`finalize_definitions()` that patches relocations — regardless of whether the
backend is `JITModule` or `ObjectModule`.

### What you don't get

- No general-purpose frontend (no C/Rust parser). You produce IR yourself.
- No link editor. `cranelift-object` gives a relocatable object; linking is
  someone else's job.
- Lighter optimization than LLVM; expect slower generated code than `-O2` but
  dramatically faster compile times.

### When Cranelift vs LLVM

- **Cranelift:** fast compilation, Rust ergonomics, small footprint, "good
  enough" code. Good for JIT / interactive / DSL use.
- **LLVM:** maximum runtime performance and advanced opts, at the cost of much
  slower compilation.

---

## 2. The Windows linker situation (AOT)

A **stock Windows install has no linker.** Linkers only appear when a
development toolchain is installed:

- **MSVC Build Tools / Visual Studio** — `link.exe` (COFF linker) + Windows CRT
  + SDK import libs/headers. Required by Rust's `*-pc-windows-msvc` target.
  The most complete, self-contained native-linking story on Windows.
- **MinGW-w64 / MSYS2** — GNU `ld` and `lld`; used by Rust's
  `*-pc-windows-gnu` target. Redistributable, MinGW-flavored output.
- **`rust-lld`** — shipped with the Rust toolchain (not on `PATH`). Supports
  COFF (`-flavor link`), so it can link Windows objects without MSVC *if* you
  also supply the import libraries and CRT startup. A bare Rust install does
  not fully provide those for arbitrary native binaries.

### What Cranelift AOT tools do in practice

There is no single convention; it splits into camps:

- **Expect MSVC installed** — the common choice for tools producing real
  Windows native binaries. They invoke `link.exe` (often discovered via the
  `cc` crate / `vswhere`) and rely on the Windows SDK. Wasmtime's
  standalone-native-object path and most Cranelift `.exe`-producing examples
  fall here. Missing MSVC → "couldn't find `link.exe`."
- **Use `lld` / `rust-lld` directly** — avoids MSVC but still needs import
  libs + CRT, so rarer for general-purpose AOT tools.
- **Ship their own linker** — occasionally bundle `lld` (and the needed
  import libs/CRT). Heavy; pure-Cranelift tools rarely go this far.
- **Skip linking entirely** — the pragmatic escape hatch. Wasmtime's default
  AOT output (`.cwasm`) is a serialized, *unlinked* blob of relocatable native
  code; relocations are applied in memory at load time. No OS linker needed.

### Rule of thumb

- Tool advertises a **standalone `.exe`/`.dll`** → it almost certainly
  **expects MSVC Build Tools** (or MinGW); it does not bundle a linker.
- Tool only **loads its own compiled output later** (cached/JIT-style, even if
  on disk) → it usually avoids linking via serialized relocatable code, and no
  Windows linker is required at all.

Cranelift deliberately stays a *code generator*. Linking (cross-object symbol
resolution, library paths, CRT/startup, dynamic linking, ABI) is a separate
concern that `lld`/`ld`/`link.exe` already handle, so Cranelift composes with
them rather than reimplementing linking.

---

## 3. Stub-loader pattern: avoid linking at user build time

Goal: the expensive link happens **once, on the tool author's machine**, and
the end user never needs a linker. This is feasible specifically because the
target language is **contained-scope** and already has a built-in mechanism for
external native calls (a host import table), so there are no external OS
symbols to resolve.

### Shape

1. **Build `stub.exe` once (author side, with MSVC/lld).** Contains:
   - Normal PE/CRT startup and entry point.
   - A small loader: open its own file (or sibling), find the appended code
     blob, `VirtualAlloc(PAGE_EXECUTE_READWRITE)` (prefer allocate RW → copy →
     `VirtualProtect` to RX for W^X hygiene), apply relocations, jump to the
     blob's entry.
   - A **host import table**: array of function pointers to runtime services
     the language exposes. Compiled code calls through fixed slots/offsets
     rather than against OS symbol names → no resolution against
     `kernel32.lib` etc.
   - A marker/offset where the blob begins (magic + length at a known file
     offset; or an overlay after the PE's last section; or a PE resource).

2. **At user build time:** run Cranelift → relocatable native code
   (`cranelift-object` COFF, or a serialized relocatable blob like `.cwasm`),
   then **append it to a copy of `stub.exe`** (or write into a reserved
   section / resource). **No `link.exe`, no `lld`, no import libs** — just file
   I/O.

3. **At runtime:** `stub.exe`'s loader reads the blob, maps it, applies
   Cranelift relocations against the actual load address, fills the import-table
   slots, calls the entry. This is in-process "linking," but trivial because
   both ends are controlled and the only "symbols" are the ones we define.

### Why it fits a contained-scope language

The built-in external-call mechanism is the key enabler. All foreign calls go
through *our* import table (fixed indices into a struct of function pointers
the stub sets up), so the Cranelift code never references external PE symbols.
That removes the one thing that would otherwise force a real linker (resolving
imports against Windows libs). We replace "linking" with "fill in a table of
known pointers."

### Things to handle (small, but real)

- **Relocations / base address.** `VirtualAlloc` returns an OS-chosen address,
  so code must be position-independent or relocated. Cranelift emits
  relocation records (`cranelift-codegen` `Reloc`s, exposed via the
  object/serialized form); the loader applies them against the chosen base.
  PIC-flavored code reduces fixup. This is exactly Wasmtime's `.cwasm` model.
- **Data sections.** Copy `.rdata`/`.data` alongside `.text`; make `.text` RX
  after copy for W^X / AV friendliness.
- **Entry-point discovery.** Store entry offset + section layout in a small
  header at the start of the blob.
- **Security / AV.** A self-extracting, RWX-allocating launcher that jumps into
  an appended blob is the shape of a packer; AV/SmartScreen may flag it.
  Signing the stub helps; proper W^X (RWX → RX) helps; avoid anything that
  looks like decryption. **This is the biggest practical risk, not correctness.**
- **DEP / RWX allocation** is legal on Windows; just do allocate-RW → copy →
  reprotect-RX for hygiene.

### Alternative: patch into a reserved PE section

Instead of an appended overlay, build `stub.exe` with a large empty `.text`
slot at a known RVA and binary-patch Cranelift bytes into the section's file
offset, fixing the entry RVA. Yields a normal-looking single PE with no
overlay and no runtime allocation — but forces fixed load address (or
hand-rolled PE base relocations) and is more fragile. Buys "looks less like a
packer." Most projects that go this direction use the overlay/blob form.

### Bottom line (§3)

Yes, feasible and essentially what Wasmtime's `.cwasm` does, wrapped in a
launcher `.exe`. Because the language routes external calls through a
controlled table, no real OS linker is needed at user build time — just file
concatenation plus a ~100-line loader. Main caveat: AV/SmartScreen perception,
not correctness.

---

## 4. Producing a real exported `.dll`

Extension: use a Rust-compiled `.dll` stub; Cranelift produces the executable
code placed in the `.dll`; the `.dll` must also expose some **exports**.

Question: *Is the export table easy to build in binary?*

**Yes — the PE export table is one of the simplest structures in the format.**
But there is a design constraint that matters for this setup.

### The format

The export directory is a single fixed header plus three parallel arrays plus
name strings. All offsets inside it are **RVAs** (offsets from the image base),
not file offsets, not VAs.

```
IMAGE_EXPORT_DIRECTORY  (40 bytes)
  DWORD  Characteristics        // 0
  DWORD  TimeDateStamp          // 0
  WORD   MajorVersion           // 0
  WORD   MinorVersion           // 0
  DWORD  Name                   // RVA -> ASCII "mylib.dll\0"
  DWORD  Base                   // ordinal base, usually 1
  DWORD  NumberOfFunctions      // size of the function RVA array
  DWORD  NumberOfNames          // size of the names array
  DWORD  AddressOfFunctions     // RVA -> DWORD[NumberOfFunctions]  (RVAs to code)
  DWORD  AddressOfNames         // RVA -> DWORD[NumberOfNames]      (RVAs to name strings)
  DWORD  AddressOfNameOrdinals  // RVA -> WORD[NumberOfNames]       (indices into the above)
```

The three arrays:

- **Function array:** `DWORD[N]` of RVAs, one per exported function (indexed by
  `ordinal - Base`).
- **Names array:** `DWORD[M]` of RVAs to ASCII strings.
- **Ordinals array:** `WORD[M]` giving, for each name, its index into the
  function array.

No relocations, no complex symbol records, no hashing — the Windows loader just
binary-searches the (loader-sorted) names array. Size for N functions with M
named exports is roughly `40 + 4N + 4M + 2M + (sum of name lengths)` bytes.

The PE header needs only `DataDirectory[0]` (IMAGE_DIRECTORY_ENTRY_EXPORT) =
`(RVA of the directory, size)`. That is the one header field to patch.

### The gotcha

**Exports are resolved by the Windows loader *before* `DllMain` runs.** So the
export table — with correct RVAs pointing at real, mapped code — must already
be present in the on-disk DLL *before* it is loaded. You cannot build the
export table at runtime inside `DllMain`; by then `GetProcAddress` consumers
have already seen (or won't see) it.

This collides with the "append a code blob and `VirtualAlloc` it at runtime"
pattern (§3), for two reasons:

1. The blob lives in the **overlay / an extra file region the Windows loader
   does not map**. Only PE sections are mapped. So blob code has no RVA and
   cannot be an export target.
2. Even if mapped at runtime, the export table was already read.

### Option A — Bake code + export table into the PE at build time (no linker, pure binary patch)

Reserve a section in the Rust-compiled stub DLL for (a) the Cranelift code and
(b) the export directory/arrays/strings. At build time:

1. Write the Cranelift code into the reserved section at a known RVA.
2. Build `IMAGE_EXPORT_DIRECTORY` + the three arrays + name strings next to
   it, with function RVAs = known section RVA + each function's offset within
   the blob (the blob header/Cranelift tells you offsets).
3. Patch `DataDirectory[0]` to point at the export directory.

No `link.exe`, no `lld` — all bytes via the `object` crate (read/write PE,
manipulate sections/data directories) or even raw `std::fs` patching if the
stub layout is fixed. The export table is genuinely easy to build in binary
in its purest form. Costs: section must fit the largest program; you are doing
real PE surgery (grow section, fix size-of-image, optionally checksum). The
`object` crate handles most of it.

### Option B — Fixed thunk exports in the stub, blob appended, wired at runtime (keeps the simple append model)

The export table is built **once, in the Rust stub** and never touched
per-build. The stub exports N small thunk functions that live in the real
`.text` (valid RVAs, appear in the export table normally):

```asm
exported_foo:  jmp  qword ptr [rip + slot_foo]   ; indirect jump through a slot
```

The slots are a fixed array of function pointers in `.data`. `DllMain` loads
the appended Cranelift blob, applies relocations, and **writes the resolved
blob function addresses into the slots**. `GetProcAddress("foo")` returns the
thunk; calling it jumps into the blob. The export table is static; only slot
wiring is per-build (fill a pointer array at runtime, no binary patching of
the PE).

Tradeoffs:

- Commit to a fixed set of export *names* at stub-build time. If the export
  set is dynamic per program → Option A. If it is a fixed ABI surface (e.g. a
  known plugin interface) → Option B is simpler and keeps per-build work to
  "append blob."
- A tiny extra `jmp` per export (negligible).
- Blob still needs to be position-independent/relocatable, as in §3.

### Which to pick

- **Exports vary per compiled program** → Option A: build the export table in
  binary per build. Format is simple; the `object` crate makes PE
  section/data-directory manipulation clean.
- **Export surface is a fixed ABI** (fixed names/arity) → Option B: bake
  thunks once, never touch the export table, just append the blob and wire
  slots in `DllMain`. Preserves the "no real linking, just append a blob"
  model most cleanly.

### Direct answer

The export table is easy to build in binary — ~40 bytes + three flat arrays +
strings, all RVAs, no relocations or symbol machinery. The only constraint:
it must be present and correct *in the on-disk PE* before the Windows loader
maps the DLL, so exports can't be constructed purely at runtime. Either bake
them into the file at build time (Option A) or use fixed thunks wired to the
blob at runtime (Option B). Option B preserves the append-a-blob model most
cleanly.

---

## 5. Open questions / follow-ups

- Choose Option A vs B based on whether OxVba's DLL export surface is dynamic
  per program or a fixed ABI. (Needs a decision before implementation.)
- AV/SmartScreen mitigation strategy for the stub launcher (signing, W^X,
  avoiding packer-like behavior).
- Whether to use `cranelift-object` (COFF) or a Wasmtime-`.cwasm`-style
  serialized relocatable blob as the per-build artifact.
- Concrete loader implementation: relocation application, data-section copy,
  W^X reprotect, entry dispatch.
- If Option A: PE-surgery details with the `object` crate (section growth,
  size-of-image, data-directory patching, optional checksum).

## 6. References

- Cranelift crates: `cranelift-codegen`, `cranelift-frontend`,
  `cranelift-module`, `cranelift-jit`, `cranelift-object` (crates.io).
- Wasmtime `.cwasm` serialized relocatable-code model (default AOT output).
- PE/COFF specification: `IMAGE_EXPORT_DIRECTORY`,
  `IMAGE_DIRECTORY_ENTRY_EXPORT`, RVA semantics.
- Rust `object` crate for PE read/write and section/data-directory
  manipulation.
