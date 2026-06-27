//! `oxvba-differential` — the differential oracle harness.
//!
//! Runs a corpus program under two executors (vm2/vm3/JIT) and compares the **six
//! fidelity axes** that define behavioural equivalence for OxVBA:
//!
//! 1. **Return values** — the entry-globals `Variant` snapshot, compared by
//!    canonical, NaN-aware `Variant` equality.
//! 2. **`Err` state** — final `Err.Number`/`Description`/`Source` + `LastDllError`,
//!    and the *sequence* of raises/clears.
//! 3. **Side-effect order** — the ordered recording-HAL journal of host calls
//!    (Print/MsgBox/file/COM).
//! 4. **Refcount / terminate timing** — the ordered `Class_Initialize`/
//!    `Class_Terminate` events keyed by statement index.
//! 5. **COM transport counts** — `(vtable, idispatch)` dispatch counts (proves
//!    early-vs-late routing is preserved).
//! 6. **COM typing & errors** — typed-argument fidelity, `[out]`/retval writebacks,
//!    and HRESULT→`Err.Number` fidelity.
//!
//! Sequencing spine: vm2 is the oracle until vm3 parity (axes 1–6 green on the full
//! corpus = the oracle handoff), after which vm3 is the oracle for the JIT. The
//! optimization tier adds two non-semantic gates (copy-count metric; forced-deopt
//! parity).
//!
//! **Current state (M0):** axis 1 (return values) is implemented with a
//! type-faithful canonical `Variant` comparator, plus a coarse run-outcome
//! comparison (ok vs diagnostic) standing in for axis 2 until M3. The remaining axes
//! are added when vm3 begins to exercise objects/COM/Declare (M3). The only executor
//! wired today is vm2; the harness exists now to prove the comparison infrastructure
//! is sound via the **vm2-vs-vm2 no-op gate**, which protects the M0 kernel
//! extraction.

pub mod oracle;

use oxvba_host::{Engine, HostConfig, Vm3Snapshot};
use oxvba_runtime::variant::VarType;
use oxvba_runtime::{Variant, variant_to_vba_string};

/// A canonical, comparable projection of a runtime [`Variant`].
///
/// Value types are compared by `(tag, payload)`; floats are NaN-canonicalized so
/// `NaN == NaN`; strings are compared by content (the BSTR pointer bytes differ
/// run-to-run). Reference / aggregate types (`Object`/`ArrayVariant`/`Record`/
/// `ProcRef`) carry a heap pointer that differs run-to-run, so they are compared by
/// tag only ([`Canon::Opaque`]); their structural comparison is deferred to M3 (when
/// vm3 first exercises objects and arrays).
///
/// Known limitation (revisit when cross-executor float comparison matters in M3):
/// signed zeros are compared **strictly** (`-0.0 != 0.0`), even though VBA treats
/// them as numerically equal. This cannot affect the M0 vm2-vs-vm2 no-op gate (a
/// deterministic run reproduces the same sign), only a future vm3/JIT comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Canon {
    Empty,
    Null,
    Bool(bool),
    /// `Single` — NaN-canonicalized `f32` bits.
    Single(u32),
    /// `Double` / `Date` — `tag` distinguishes them; `bits` are NaN-canonicalized.
    Float { tag: u16, bits: u64 },
    /// A value type compared by its raw payload + reserved words (covers
    /// `Integer`/`Long`/`LongLong`/`Byte`/`Currency`/`Error`/`Decimal`/…). Keeping
    /// the tag makes the comparison type-faithful (a `Long 5` differs from an
    /// `Integer 5`), which is what catches a JIT that produces the wrong width.
    Raw {
        tag: u16,
        bytes: [u8; 8],
        reserved: [u16; 3],
    },
    /// A string, compared by content.
    Str(String),
    /// A reference/aggregate type whose payload is a heap pointer (structural
    /// comparison deferred to M3); compared by tag only.
    Opaque { tag: u16 },
}

fn canon_f32_bits(x: f32) -> u32 {
    if x.is_nan() { f32::NAN.to_bits() } else { x.to_bits() }
}

fn canon_f64_bits(x: f64) -> u64 {
    if x.is_nan() { f64::NAN.to_bits() } else { x.to_bits() }
}

/// Project a runtime [`Variant`] into its canonical comparison form.
pub fn canon(v: &Variant) -> Canon {
    let tag = v.vtype() as u16;
    match v.vtype() {
        VarType::Empty => Canon::Empty,
        VarType::Null => Canon::Null,
        VarType::Boolean => Canon::Bool(v.as_bool().unwrap_or(false)),
        VarType::Single => Canon::Single(canon_f32_bits(v.as_f32().unwrap_or(0.0))),
        VarType::Double => Canon::Float {
            tag,
            bits: canon_f64_bits(v.as_f64().unwrap_or(0.0)),
        },
        VarType::Date => Canon::Float {
            tag,
            bits: canon_f64_bits(v.as_date_f64().unwrap_or(0.0)),
        },
        VarType::String => Canon::Str(
            variant_to_vba_string(v)
                .map(|b| b.as_str())
                .unwrap_or_default(),
        ),
        // Heap-pointer payloads differ run-to-run — structural compare is M3 work.
        VarType::Object | VarType::ArrayVariant | VarType::Record | VarType::ProcRef => {
            Canon::Opaque { tag }
        }
        // Every other value type (Integer/Long/LongLong/Byte/Currency/Error/Decimal/…)
        // is compared by its raw payload and reserved words.
        _ => Canon::Raw {
            tag,
            bytes: v.data_bytes(),
            reserved: [v.reserved1(), v.reserved2(), v.reserved3()],
        },
    }
}

/// Which execution backend to run a program under. `Jit` lands in M4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Executor {
    /// The legacy `Op`-bundle interpreter — the golden oracle until vm3 parity.
    Vm2,
    /// The typed-OxIR interpreter under construction (M2). It runs the subset it
    /// implements; an out-of-scope construct yields [`RunOutcome::unsupported`] (the
    /// corpus comparison skips it rather than scoring a divergence).
    Vm3,
}

/// The observable outcome of one run, for differential comparison.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// Axis 1 (return values): the canonical snapshot of the entry project's globals
    /// followed by the entry `Sub Main` locals — or the rendered phase diagnostic if
    /// the run did not produce a snapshot (a coarse stand-in for axis 2 until M3).
    pub result: Result<Vec<Canon>, String>,
    /// Set when the executor cannot run this program because it uses a construct it does
    /// not yet implement (vm3 during M2). Such a program is SKIPPED by the corpus
    /// comparison — out of the executor's current scope, not a divergence. The complete
    /// oracle (vm2) never sets this.
    pub unsupported: Option<String>,
}

/// Run `source` under `executor` and capture its observable outcome.
pub fn run(executor: Executor, source: &str) -> RunOutcome {
    match executor {
        Executor::Vm2 => {
            let engine = Engine::new(HostConfig { enable_jit: false });
            let result = engine
                .execute_source_with_variant_snapshot_clean(source)
                .map(|vals| vals.iter().map(canon).collect())
                .map_err(|d| format!("{d:?}"));
            RunOutcome { result, unsupported: None }
        }
        Executor::Vm3 => {
            let engine = Engine::new(HostConfig { enable_jit: false });
            match engine.execute_source_with_variant_snapshot_vm3(source) {
                Vm3Snapshot::Ran(vals) => RunOutcome {
                    result: Ok(vals.iter().map(canon).collect()),
                    unsupported: None,
                },
                Vm3Snapshot::Unsupported(what) => RunOutcome {
                    result: Ok(Vec::new()),
                    unsupported: Some(what),
                },
                Vm3Snapshot::Failed(msg) => RunOutcome {
                    result: Err(msg),
                    unsupported: None,
                },
            }
        }
    }
}

/// Run `source` under `executor` as a single-module project named `project_name`,
/// capturing the same observable as [`run`]. The project name becomes the program's
/// `unit_name`, which is what `Err.Source` defaults to — so the oracle corpus runs under
/// `"VBAProject"` (Excel's default VBProject name) to mirror the captured oracle exactly.
pub fn run_with_project(executor: Executor, source: &str, project_name: &str) -> RunOutcome {
    use oxvba_symbol::manifest as sym;
    let manifest = sym::SymbolProjectManifest {
        project_name: project_name.to_string(),
        project_kind: sym::ProjectKind::Source,
        modules: vec![sym::ModuleUnit {
            module_name: "Main".to_string(),
            module_kind: sym::ModuleKind::Procedural,
            attributes: sym::ModuleAttributes::named("Main"),
            source: source.to_string(),
        }],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };
    let engine = Engine::new(HostConfig { enable_jit: false });
    match executor {
        Executor::Vm2 => {
            let result = engine
                .execute_manifest_with_variant_snapshot(&manifest)
                .map(|vals| vals.iter().map(canon).collect())
                .map_err(|d| format!("{d:?}"));
            RunOutcome { result, unsupported: None }
        }
        Executor::Vm3 => match engine.execute_manifest_with_variant_snapshot_vm3(&manifest) {
            Vm3Snapshot::Ran(vals) => RunOutcome {
                result: Ok(vals.iter().map(canon).collect()),
                unsupported: None,
            },
            Vm3Snapshot::Unsupported(what) => RunOutcome {
                result: Ok(Vec::new()),
                unsupported: Some(what),
            },
            Vm3Snapshot::Failed(msg) => RunOutcome {
                result: Err(msg),
                unsupported: None,
            },
        },
    }
}

/// A single observable difference between two runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Difference {
    /// The runs disagree on success vs failure (or produced different diagnostics).
    Outcome { left: String, right: String },
    /// The snapshots have different lengths.
    SnapshotLen { left: usize, right: usize },
    /// Slot `index` of the snapshot differs.
    Slot {
        index: usize,
        left: Canon,
        right: Canon,
    },
}

/// Compare two run outcomes, returning every observable difference (empty ⇒
/// behaviourally equivalent on the axes captured so far).
pub fn diff(left: &RunOutcome, right: &RunOutcome) -> Vec<Difference> {
    match (&left.result, &right.result) {
        (Ok(a), Ok(b)) => {
            let mut diffs = Vec::new();
            if a.len() != b.len() {
                diffs.push(Difference::SnapshotLen {
                    left: a.len(),
                    right: b.len(),
                });
            }
            for (index, (x, y)) in a.iter().zip(b.iter()).enumerate() {
                if x != y {
                    diffs.push(Difference::Slot {
                        index,
                        left: x.clone(),
                        right: y.clone(),
                    });
                }
            }
            diffs
        }
        (Err(a), Err(b)) if a == b => Vec::new(),
        (a, b) => vec![Difference::Outcome {
            left: render_outcome(a),
            right: render_outcome(b),
        }],
    }
}

fn render_outcome(r: &Result<Vec<Canon>, String>) -> String {
    match r {
        Ok(_) => "ok".to_string(),
        Err(e) => format!("error: {e}"),
    }
}

/// The corpus-level verdict comparing the oracle (vm2) against an under-construction
/// executor (vm3) on one program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusVerdict {
    /// The candidate does not yet implement a construct the program uses (the string
    /// names it) — out of the candidate's current scope, not a divergence.
    Skipped(String),
    /// Behaviourally equivalent on the captured axes: both produced the same snapshot,
    /// or — coarsely — both errored (cross-executor error-code comparison is the M2-c
    /// `Err` axis).
    Match,
    /// A real divergence: differing snapshots, or one ran while the other errored.
    Mismatch(Vec<Difference>),
}

/// Classify one program for the vm2-vs-candidate corpus gate. `oracle` is the complete
/// reference (vm2); `candidate` is the executor under test (vm3). A candidate that
/// cannot run the program (`unsupported`) is SKIPPED, not failed. Two successful runs
/// are compared by snapshot; two failed runs match coarsely (both errored) until the
/// `Err` axis matures (M2-c); a success-vs-failure split is a divergence to investigate.
pub fn compare_corpus(oracle: &RunOutcome, candidate: &RunOutcome) -> CorpusVerdict {
    if let Some(reason) = &candidate.unsupported {
        return CorpusVerdict::Skipped(reason.clone());
    }
    match (&oracle.result, &candidate.result) {
        (Ok(_), Ok(_)) => {
            let d = diff(oracle, candidate);
            if d.is_empty() {
                CorpusVerdict::Match
            } else {
                CorpusVerdict::Mismatch(d)
            }
        }
        (Err(_), Err(_)) => CorpusVerdict::Match,
        (a, b) => CorpusVerdict::Mismatch(vec![Difference::Outcome {
            left: render_outcome(a),
            right: render_outcome(b),
        }]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The M0 gate: running the same program through vm2 twice must be a no-op.
    fn vm2_noop(source: &str) {
        let a = run(Executor::Vm2, source);
        assert!(a.result.is_ok(), "vm2 run failed: {:?}", a.result);
        let b = run(Executor::Vm2, source);
        let d = diff(&a, &b);
        assert!(
            d.is_empty(),
            "vm2-vs-vm2 differential must be a no-op, got: {d:?}"
        );
    }

    #[test]
    fn typed_numeric_program_is_deterministic() {
        vm2_noop("Sub Main()\n  Dim n As Long\n  n = (10 + 5) * 2\nEnd Sub\n");
    }

    #[test]
    fn string_program_is_deterministic() {
        vm2_noop("Sub Main()\n  Dim s As String\n  s = \"ab\" & \"cd\"\nEnd Sub\n");
    }

    #[test]
    fn variant_and_bool_program_is_deterministic() {
        vm2_noop("Sub Main()\n  Dim x\n  x = 3.5\n  Dim b As Boolean\n  b = (x > 1)\nEnd Sub\n");
    }

    #[test]
    fn canon_distinguishes_widths_and_canonicalizes_nan() {
        // Type-faithful: Long(5) and LongLong(5) must not compare equal (catches a
        // backend that produces the wrong integer width).
        assert_ne!(canon(&Variant::from_i32(5)), canon(&Variant::from_i64(5)));
        // ...but two equal Long(5)s do compare equal.
        assert_eq!(canon(&Variant::from_i32(5)), canon(&Variant::from_i32(5)));
        // NaN canonicalization: two different NaN bit patterns compare equal.
        let nan_a = canon(&Variant::from_f64(f64::NAN));
        let nan_b = canon(&Variant::from_f64(f64::from_bits(0x7FF8_0000_0000_0001)));
        assert_eq!(nan_a, nan_b);
    }

    // ── M2-d: vm3-vs-vm2 differential gate ──────────────────────────────────────

    /// vm3 must match vm2 on an in-scope program. A skip here means the program is
    /// unexpectedly out of scope (these probes are all within the M2 subset).
    fn assert_vm2_vm3_match(source: &str) {
        let vm2 = run(Executor::Vm2, source);
        let vm3 = run(Executor::Vm3, source);
        match compare_corpus(&vm2, &vm3) {
            CorpusVerdict::Match => {}
            CorpusVerdict::Skipped(what) => {
                panic!("expected vm3 to run this in-scope program, but it skipped ({what}):\n{source}")
            }
            CorpusVerdict::Mismatch(d) => panic!("vm2-vs-vm3 divergence {d:?}\n{source}"),
        }
    }

    #[test]
    fn vm3_matches_vm2_on_arithmetic() {
        assert_vm2_vm3_match("Sub Main()\n  Dim n As Long\n  n = (10 + 5) * 2\nEnd Sub\n");
    }

    #[test]
    fn vm3_matches_vm2_on_strings() {
        assert_vm2_vm3_match("Sub Main()\n  Dim s As String\n  s = \"ab\" & \"cd\"\nEnd Sub\n");
    }

    #[test]
    fn vm3_matches_vm2_on_control_flow_and_calls() {
        assert_vm2_vm3_match(
            "Sub Main()\n  Dim n As Long\n  n = Doubler(7)\nEnd Sub\n\
             Function Doubler(ByVal x As Long) As Long\n  Doubler = x * 2\nEnd Function\n",
        );
    }

    #[test]
    fn vm3_matches_vm2_on_a_for_loop() {
        assert_vm2_vm3_match(
            "Sub Main()\n  Dim n As Long\n  Dim i As Long\n  For i = 1 To 5\n    n = n + i\n  Next i\nEnd Sub\n",
        );
    }

    // NB: most VBA built-ins (`Len`, `UCase`, …) lower to a cross-bundle `CallExtern`
    // into the "VBA library" bundle, NOT `CallNative` — so they are SKIPPED by vm3 today
    // (cross-bundle dispatch is M3). vm3's `CallNative` builtin path is exercised by the
    // vm3 unit tests; a corpus-level builtin probe lands once `CallExtern` does.

    /// Programs where vm2 (the current oracle) itself deviates from Office VBA 7.1, so a
    /// vm2-vs-vm3 difference is expected and is NOT a vm3 bug. Keyed by file name.
    ///
    /// Currently empty: `duplicate_label_error.bas` used to live here (vm2 leniently ran
    /// a procedure with two identical labels while vm3's elaboration rejected it), but the
    /// binder now rejects a duplicate label at compile time, so vm2 and vm3 agree on
    /// "compile error" and the program matches through the gate. New entries belong here
    /// only when vm2 is the side that diverges from Office.
    const KNOWN_VM2_DIVERGENCES: &[&str] = &[];

    /// Recursively collect `*.bas` files under `dir`.
    fn bas_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else { continue };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|e| e == "bas") {
                    out.push(p);
                }
            }
        }
        out
    }

    /// The M2-d gate: for every corpus program vm3 can run (no `Unimplemented`), its
    /// snapshot must match vm2. Programs vm3 doesn't yet implement are skipped; programs
    /// both backends error on match coarsely. As vm3 implements more, `skipped` shrinks
    /// and `ran` grows — with `mismatches` staying empty.
    #[test]
    fn vm3_matches_vm2_across_the_corpus_subset() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut ran = 0usize;
        let mut skipped = 0usize;
        let mut both_errored = 0usize;
        let mut known_divergence = 0usize;
        let mut mismatches: Vec<(String, Vec<Difference>)> = Vec::new();
        // What the skipped programs need, so the coverage gap is visible (keyed by the
        // first unimplemented construct each program hit) — this is the M2-c/M3 worklist.
        let mut skip_reasons: std::collections::BTreeMap<String, usize> = Default::default();
        for dir in ["conformance", "examples"] {
            for path in bas_files(&root.join(dir)) {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if KNOWN_VM2_DIVERGENCES.contains(&name) {
                    known_divergence += 1;
                    continue;
                }
                let Ok(source) = std::fs::read_to_string(&path) else { continue };
                if source.trim().is_empty() {
                    continue;
                }
                let vm2 = run(Executor::Vm2, &source);
                let vm3 = run(Executor::Vm3, &source);
                match compare_corpus(&vm2, &vm3) {
                    CorpusVerdict::Skipped(reason) => {
                        skipped += 1;
                        *skip_reasons.entry(reason).or_default() += 1;
                    }
                    CorpusVerdict::Match => {
                        if vm2.result.is_ok() {
                            ran += 1;
                        } else {
                            both_errored += 1;
                        }
                    }
                    CorpusVerdict::Mismatch(d) => mismatches.push((path.display().to_string(), d)),
                }
            }
        }
        eprintln!(
            "vm3-vs-vm2 corpus: ran+matched={ran}, skipped(unsupported)={skipped}, both-errored={both_errored}, known-vm2-divergence={known_divergence}, mismatches={}",
            mismatches.len()
        );
        let mut by_count: Vec<_> = skip_reasons.iter().collect();
        by_count.sort_by(|a, b| b.1.cmp(a.1));
        for (reason, count) in by_count {
            eprintln!("  skip[{count:>3}] {reason}");
        }
        for (path, d) in mismatches.iter().take(25) {
            eprintln!("  MISMATCH {path}\n    {d:?}");
        }
        assert!(
            mismatches.is_empty(),
            "{} vm3-vs-vm2 divergences across the corpus (see stderr)",
            mismatches.len()
        );
        assert!(ran > 0, "expected some in-scope corpus programs to run on vm3");
    }
}
