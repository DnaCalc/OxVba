//! Compiler-acceptance corpus: the real-world ChibiPDF `ChibiEx` class (MIT,
//! vendored under `.external/chibipdf`) must bind and linearize cleanly through
//! the clean stack. It is a dense single-file slice of the VBA surface — 37
//! `Declare`s (Win32 + WinRT `combase`), `#If Win64`/`#If VBA7` dual-declare
//! conditional compilation, 6 UDTs, `As Any`/`ByRef GUID`/`LongPtr` params, and
//! a broad `Property`/`Optional Variant` public surface — so accepting it
//! exercises the front-end far more than any hand-written fixture.
//!
//! Full execution needs Windows-native WinRT OCR plus a PDF, so this is a
//! front-end (bind + linearize) acceptance, not an end-to-end run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxvba_bind::bind_program;
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, SymbolProjectManifest,
};
use oxvba_symbol::provider::TypeLibResolver;

struct NullTypeLibs;
impl TypeLibResolver for NullTypeLibs {
    fn resolve(
        &self,
        _request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob> {
        None
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

fn chibiex_source() -> String {
    let path = workspace_root().join(".external/chibipdf/src/ChibiEx.cls");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read vendored ChibiEx.cls at {}: {e}", path.display()));
    strip_class_header(&raw)
}

/// Strip the VB6/VBA `.cls` `VERSION 1.0 CLASS` / `BEGIN..END` metadata header,
/// mirroring the project loader's `normalize_loaded_module_source`, so the
/// module source matches what real `.cls` ingestion feeds the parser.
fn strip_class_header(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let first = lines.first().map(|l| l.trim().to_ascii_lowercase());
    if !first
        .as_deref()
        .is_some_and(|f| f.starts_with("version ") && f.ends_with(" class"))
    {
        return source.to_string();
    }
    let mut idx = 1;
    if lines
        .get(idx)
        .is_some_and(|l| l.trim().eq_ignore_ascii_case("BEGIN"))
    {
        idx += 1;
        while idx < lines.len() && !lines[idx].trim().eq_ignore_ascii_case("END") {
            idx += 1;
        }
        if idx < lines.len() {
            idx += 1;
        }
    }
    lines[idx..].join("\n")
}

#[test]
fn chibiex_class_parses_without_errors() {
    // The verbatim ChibiEx class must PARSE cleanly through the real pipeline
    // (which preprocesses `#If Win64`/`#If VBA7` conditional compilation before
    // parsing). It exercises 37 `Declare`s, 6 UDTs, single-line `Property`/
    // `Function` bodies (`X() As Long: … : End Property`), and call-site
    // `ByVal`/`ByRef` argument modifiers (`CopyMemory dst, ByVal src, n`).
    //
    // Binding may still fail later (see the nested-UDT-array gap in the
    // ignored test below); this guards only that no *parse* error survives —
    // i.e. the single-line-proc and ByVal-call-arg parser support holds against
    // a real-world file.
    let manifest = SymbolProjectManifest {
        project_name: "ChibiPdf".into(),
        project_kind: ProjectKind::Source,
        modules: vec![ModuleUnit {
            module_name: "ChibiEx".into(),
            module_kind: ModuleKind::Class,
            attributes: ModuleAttributes::named("ChibiEx"),
            source: chibiex_source(),
        }],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };
    if let Err(e) = bind_program(&manifest, &NullTypeLibs) {
        let rendered = format!("{e:?}");
        assert!(
            !rendered.contains("ParseError"),
            "ChibiEx must have no parse errors; got: {rendered}"
        );
    }
}

#[test]
fn chibiex_class_binds_and_elaborates() {
    // A `Main` entry that instantiates the class, plus the verbatim ChibiEx
    // class module. (Predefined cond-comp constants select the Win64/VBA7
    // PtrSafe declare branch, matching the 64-bit runtime target.)
    //
    // STATUS: binds AND elaborates to the vm3 OxIR cleanly through the clean
    // stack. This is a front-end acceptance (bind + OxIR lowering), NOT a VM
    // run — full execution needs Windows-native WinRT OCR plus a PDF. The UDT
    // round made it bind: array-aware UDT field types (`Lines() As ocrLine` ⇒
    // `Array(ocrLine)`), per-instance class-field record-init in
    // `Class_Initialize`, and UDT member access through a `With` receiver
    // (`With m_Results.Lines(i) … .Text`); the compound-place lowering (incl.
    // `Erase m_Results.Lines` on a UDT member array) makes it elaborate.
    let manifest = SymbolProjectManifest {
        project_name: "ChibiPdf".into(),
        project_kind: ProjectKind::Source,
        modules: vec![
            ModuleUnit {
                module_name: "Main".into(),
                module_kind: ModuleKind::Procedural,
                attributes: ModuleAttributes::named("Main"),
                source: "Sub Main()\n    Dim ex As ChibiEx\n    Set ex = New ChibiEx\nEnd Sub\n"
                    .into(),
            },
            ModuleUnit {
                module_name: "ChibiEx".into(),
                module_kind: ModuleKind::Class,
                attributes: ModuleAttributes::named("ChibiEx"),
                source: chibiex_source(),
            },
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };

    let cp = bind_program(&manifest, &NullTypeLibs)
        .unwrap_or_else(|e| panic!("ChibiEx failed to bind: {e:?}"));
    // ...and elaborates to the vm3 OxIR. The verbatim ChibiEx body exercises
    // `Erase m_Results.Lines` — Erase of a UDT *member* array — which the OxIR
    // lowering handles via the same materialize-and-write-back as a compound
    // `ReDim`. (Full execution still needs Windows-native WinRT OCR plus a PDF;
    // this is the front-end ceiling.)
    oxvba_oxir::elaborate::elaborate(&cp)
        .unwrap_or_else(|e| panic!("ChibiEx failed to elaborate to OxIR: {e:?}"));
}
