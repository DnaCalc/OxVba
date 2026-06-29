//! `oxvba-differential` — the vm3 conformance harness.
//!
//! Runs a corpus program on **vm3** (the sole product runtime + the JIT oracle) and
//! captures its observable across the fidelity axes that define behavioural
//! equivalence for OxVBA:
//!
//! 1. **Return values** — the entry-globals `Variant` snapshot, compared by
//!    canonical, NaN-aware `Variant` equality.
//! 2. **`Err` state** — final `Err.Number`/`Description`/`Source` + `LastDllError`.
//! 3. **Side-effect order** — the ordered recording-HAL journal of host calls
//!    (Print/MsgBox/file/COM).
//! 4. **Refcount / terminate timing** — the ordered `Class_Initialize`/
//!    `Class_Terminate` events keyed by statement index.
//! 5. **COM transport counts** — `(vtable, idispatch)` dispatch counts.
//! 6. **COM typing & errors** — typed-argument fidelity, `[out]`/retval writebacks,
//!    and HRESULT→`Err.Number` fidelity.
//!
//! Sequencing spine: vm3 IS the oracle (the legacy `Op`-bundle interpreter has been
//! retired). The vm3 GOLDEN SNAPSHOT ([`tests::vm3_golden_snapshot`]) pins
//! vm3's validated observable for every corpus program — the standalone regression net
//! that the legacy differential used to provide — and the live-Excel oracle gate
//! ([`oracle`]) validates vm3 against captured VBA 7.1 ground truth. The optimization
//! tier (M4 JIT) differentials the JIT against this same vm3 observable.

pub mod oracle;

use oxvba_host::{Engine, FinalErr, HostConfig, SnapshotOutcome};
use oxvba_runtime::variant::VarType;
use oxvba_runtime::{Variant, variant_to_vba_string};

/// A canonical, comparable projection of a runtime [`Variant`].
///
/// Value types are compared by `(tag, payload)`; floats are NaN-canonicalized so
/// `NaN == NaN`; strings are compared by content (the BSTR pointer bytes differ
/// run-to-run). Reference / aggregate types (`Object`/`ArrayVariant`/`Record`/
/// `ProcRef`) carry a heap pointer that differs run-to-run, so they are compared by
/// tag only ([`Canon::Opaque`]); their structural comparison is deferred (when
/// cross-executor object/array comparison matters for the JIT).
///
/// Known limitation: signed zeros are compared **strictly** (`-0.0 != 0.0`), even
/// though VBA treats them as numerically equal.
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
    /// comparison deferred); compared by tag only.
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
        // Heap-pointer payloads differ run-to-run — structural compare is deferred.
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

/// Which execution backend to run a program under. `Jit` lands in M4; vm3 is the sole
/// interpreter (the legacy `Op`-bundle interpreter has been retired).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Executor {
    /// The typed-OxIR interpreter — the product runtime and the JIT oracle.
    Vm3,
}

/// The observable outcome of one run, for differential comparison.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// Axis 1 (return values): the canonical snapshot of the entry project's globals
    /// followed by the entry `Sub Main` locals on a completed run; `Err(msg)` if the run did
    /// not complete — an uncaught VBA error (see `raised`) or a pre-execution defect.
    pub result: Result<Vec<Canon>, String>,
    /// Axis 2 (error state): the final `Err` object (number / source / description /
    /// `LastDllError`). Populated whenever the program reached execution (a completed run's
    /// residual `Err`, or an uncaught raised error); [`FinalErr::default`] otherwise
    /// (unsupported skip or pre-execution defect).
    pub err: FinalErr,
    /// True when `result` is `Err` because an uncaught *VBA* run-time error propagated, as
    /// opposed to a compile/defect failure. Lets the gate compare a raised error's number
    /// instead of coarsely matching any-error-with-any-error.
    pub raised: bool,
    /// Set when the executor cannot run this program because it uses a construct it does
    /// not yet implement. Such a program is SKIPPED by the corpus comparison — out of the
    /// executor's current scope, not a divergence.
    pub unsupported: Option<String>,
}

impl RunOutcome {
    fn from_snapshot(outcome: SnapshotOutcome) -> Self {
        match outcome {
            SnapshotOutcome::Completed { values, err } => RunOutcome {
                result: Ok(values.iter().map(canon).collect()),
                err,
                raised: false,
                unsupported: None,
            },
            SnapshotOutcome::Raised { err } => RunOutcome {
                result: Err(format!("VBA error {}", err.number)),
                err,
                raised: true,
                unsupported: None,
            },
            SnapshotOutcome::Unsupported(what) => RunOutcome {
                result: Ok(Vec::new()),
                err: FinalErr::default(),
                raised: false,
                unsupported: Some(what),
            },
            SnapshotOutcome::Failed(msg) => RunOutcome {
                result: Err(msg),
                err: FinalErr::default(),
                raised: false,
                unsupported: None,
            },
        }
    }
}

/// Run `source` under `executor` and capture its observable outcome (as project `"Main"`).
pub fn run(executor: Executor, source: &str) -> RunOutcome {
    run_with_project(executor, source, "Main")
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
    let outcome = match executor {
        Executor::Vm3 => engine.execute_manifest_snapshot_with_err_vm3(&manifest),
    };
    RunOutcome::from_snapshot(outcome)
}

/// Run a multi-module project under `executor` (e.g. a procedural `Main` plus a class module),
/// capturing the same observable as [`run_with_project`]. Needed to exercise project classes
/// (`New`/`Class_Initialize`/`Class_Terminate`/`Implements`), which a single `.bas` standard
/// module cannot declare. Module order matters for the snapshot: name helper modules so they
/// sort after `Main`.
pub fn run_modules(
    executor: Executor,
    modules: &[(&str, oxvba_symbol::manifest::ModuleKind, &str)],
    project_name: &str,
) -> RunOutcome {
    use oxvba_symbol::manifest as sym;
    let manifest = sym::SymbolProjectManifest {
        project_name: project_name.to_string(),
        project_kind: sym::ProjectKind::Source,
        modules: modules
            .iter()
            .map(|(name, kind, src)| sym::ModuleUnit {
                module_name: name.to_string(),
                module_kind: *kind,
                attributes: sym::ModuleAttributes::named(*name),
                source: src.to_string(),
            })
            .collect(),
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };
    let engine = Engine::new(HostConfig { enable_jit: false });
    let outcome = match executor {
        Executor::Vm3 => engine.execute_manifest_snapshot_with_err_vm3(&manifest),
    };
    RunOutcome::from_snapshot(outcome)
}

/// Run `source` under `executor` (as project `project_name`) on a worker thread with a
/// wall-clock timeout; returns `None` if it does not finish in `dur`.
///
/// This guards the gates against an executor spinning on a program that terminates under
/// correct semantics. vm3 must never time out on an in-scope program (that would be a vm3
/// bug, which the gate treats as a failure, not a skip). A timed-out worker is left to exit
/// on its own.
pub fn run_with_timeout(
    executor: Executor,
    source: &str,
    project_name: &str,
    dur: std::time::Duration,
) -> Option<RunOutcome> {
    let src = source.to_string();
    let proj = project_name.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run_with_project(executor, &src, &proj));
    });
    rx.recv_timeout(dur).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert a vm3 run completed (not unsupported, not a defect/raised error) and return its
    /// canonical snapshot. The vm3 conformance probes below assert specific result slots; the
    /// per-program validated observable is otherwise pinned by [`vm3_golden_snapshot`].
    fn run_vm3_ok(source: &str) -> Vec<Canon> {
        let o = run(Executor::Vm3, source);
        assert!(
            o.unsupported.is_none(),
            "vm3 unexpectedly skipped an in-scope program ({:?}):\n{source}",
            o.unsupported
        );
        o.result
            .unwrap_or_else(|e| panic!("vm3 run failed: {e}\n{source}"))
    }

    #[test]
    fn vm3_runs_arithmetic() {
        let snap = run_vm3_ok("Sub Main()\n  Dim n As Long\n  n = (10 + 5) * 2\nEnd Sub\n");
        assert!(snap.contains(&canon(&Variant::from_i32(30))), "{snap:?}");
    }

    #[test]
    fn vm3_runs_strings() {
        let snap = run_vm3_ok("Sub Main()\n  Dim s As String\n  s = \"ab\" & \"cd\"\nEnd Sub\n");
        assert!(snap.contains(&Canon::Str("abcd".to_string())), "{snap:?}");
    }

    #[test]
    fn vm3_runs_control_flow_and_calls() {
        let snap = run_vm3_ok(
            "Sub Main()\n  Dim n As Long\n  n = Doubler(7)\nEnd Sub\n\
             Function Doubler(ByVal x As Long) As Long\n  Doubler = x * 2\nEnd Function\n",
        );
        assert!(snap.contains(&canon(&Variant::from_i32(14))), "{snap:?}");
    }

    /// vm3-finish W3: built-in `Collection` end-to-end — `New Collection` + Add (positional,
    /// keyed, and a before-anchor with an omitted key → MISSING_ARG), Count, Item (by index and
    /// by key), Remove — runs on vm3 via the shared keyed dispatch over the object box.
    #[test]
    fn vm3_runs_collection_methods() {
        let snap = run_vm3_ok(
            "Sub Main()\n\
             Dim c As New Collection\n\
             c.Add 10\n\
             c.Add 20, \"k\"\n\
             c.Add 30, , 1\n\
             Dim n As Long\n\
             n = c.Count\n\
             Dim a As Variant\n\
             a = c.Item(1)\n\
             Dim b As Variant\n\
             b = c.Item(\"k\")\n\
             c.Remove 1\n\
             End Sub\n",
        );
        // After Add 10; Add 20,"k"; Add 30,,before:=1 the collection is [30, 10, 20(key "k")].
        // Snapshot = [c (Object), n=Count=3, a=Item(1)=30, b=Item("k")=20].
        assert_eq!(snap.first(), Some(&Canon::Opaque { tag: 9 }), "c is an Object: {snap:?}");
        assert!(snap.contains(&canon(&Variant::from_i32(3))), "Count==3: {snap:?}");
        assert!(snap.contains(&canon(&Variant::from_i32(30))), "Item(1)==30: {snap:?}");
        assert!(snap.contains(&canon(&Variant::from_i32(20))), "Item(\"k\")==20: {snap:?}");
        assert!(
            !snap.contains(&canon(&Variant::from_i32(10))),
            "10 stays inside the collection, not in the snapshot: {snap:?}"
        );
    }

    /// vm3-finish W4: built-in `Collection` default-member `c(i)` (by index and by key) and
    /// `For Each` over a Collection run on vm3.
    #[test]
    fn vm3_runs_collection_default_member_and_for_each() {
        let snap = run_vm3_ok(
            "Sub Main()\n\
             Dim c As New Collection\n\
             c.Add 10\n\
             c.Add 20, \"k\"\n\
             Dim a As Variant\n\
             a = c(1)\n\
             Dim b As Variant\n\
             b = c(\"k\")\n\
             Dim total As Long\n\
             Dim v As Variant\n\
             For Each v In c\n\
             total = total + v\n\
             Next v\n\
             End Sub\n",
        );
        assert!(snap.contains(&canon(&Variant::from_i32(30))), "total: {snap:?}");
    }

    /// Regression for task_842916b0: a `Double` `/` result stored into a `Long` local must
    /// narrow (coerce-on-store with banker's rounding), not keep the `Double`. `10 / 4 = 2.5`
    /// → `Long` 2 (round half to even).
    #[test]
    fn vm3_coerces_div_result_to_a_long_local() {
        let snap = run_vm3_ok("Sub Main()\n  Dim n As Long\n  n = 10 / 4\nEnd Sub\n");
        assert!(snap.contains(&canon(&Variant::from_i32(2))), "{snap:?}");
    }

    #[test]
    fn vm3_runs_a_for_loop() {
        let snap = run_vm3_ok(
            "Sub Main()\n  Dim n As Long\n  Dim i As Long\n  For i = 1 To 5\n    n = n + i\n  Next i\nEnd Sub\n",
        );
        assert!(snap.contains(&canon(&Variant::from_i32(15))), "{snap:?}");
    }

    // ── M3-5: object model + lifecycle micro-corpus ─────────────────────────────
    // The conformance corpus is all `.bas` (standard modules), which cannot declare a
    // project class — so these multi-module (class-bearing) programs are the differential
    // coverage for New / Class_Initialize / Class_Terminate timing / `Is` / `TypeOf`.

    /// Run a multi-module (class-bearing) program on vm3, asserting it completed, and return
    /// its canonical snapshot.
    fn run_obj_ok(modules: &[(&str, oxvba_symbol::manifest::ModuleKind, &str)]) -> Vec<Canon> {
        let o = run_modules(Executor::Vm3, modules, "VBAProject");
        assert!(
            o.unsupported.is_none(),
            "vm3 unexpectedly skipped an in-scope object program ({:?})",
            o.unsupported
        );
        o.result
            .unwrap_or_else(|e| panic!("vm3 object run failed: {e}"))
    }

    #[test]
    fn vm3_new_runs_class_initialize() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gResult As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\nEnd Sub\n",
            ),
            (
                "Widget",
                Class,
                "Private Sub Class_Initialize()\n  gResult = 42\nEnd Sub\n",
            ),
        ]);
        // Global 0 (gResult) holds 42 — Class_Initialize ran.
        assert_eq!(snap.first(), Some(&canon(&Variant::from_i32(42))), "{snap:?}");
    }

    /// `Set w = Nothing` in the entry `Main` does NOT drain `Class_Terminate` in the current
    /// implementation: the entry frame is never popped (it holds the result snapshot), so the
    /// Widget's last reference is never released to zero and `Class_Terminate` (`gTerm + 1`)
    /// does not run before the snapshot is read. This test PINS that current, known-divergent
    /// Set=Nothing-at-Main-scope behaviour: the residual `gTerm` (global 0) is 100 — only the
    /// `+100` store landed, NOT 101. This is a PRE-EXISTING behaviour (vm3 here matched the now-
    /// retired vm2 exactly; it is not a W12 regression). Correct VBA drains the terminate at the
    /// `Set = Nothing` boundary → 101; FOLLOW-UP: drain at `Set <local> = Nothing` even inside
    /// the never-popped entry frame, then flip this assertion to 101 and rename to
    /// `..._runs_class_terminate`. (Terminate timing IS exercised correctly in a *called* proc by
    /// `vm3_cross_proc_object_terminates_on_caught_fault`, where the frame is popped.)
    #[test]
    fn vm3_set_nothing_at_main_scope_does_not_yet_drain_terminate() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gTerm As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  Set w = Nothing\n  gTerm = gTerm + 100\nEnd Sub\n",
            ),
            (
                "Widget",
                Class,
                "Private Sub Class_Terminate()\n  gTerm = gTerm + 1\nEnd Sub\n",
            ),
        ]);
        assert_eq!(
            snap.first(),
            Some(&canon(&Variant::from_i32(100))),
            "Set=Nothing at Main scope does not drain Class_Terminate (pinned known behaviour): {snap:?}"
        );
    }

    #[test]
    fn vm3_object_identity_is() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gDiff As Boolean\nPublic gSame As Boolean\nSub Main()\n  Dim a As Widget\n  Dim b As Widget\n  Set a = New Widget\n  Set b = New Widget\n  gDiff = (a Is b)\n  Set b = a\n  gSame = (a Is b)\nEnd Sub\n",
            ),
            ("Widget", Class, "' a minimal class\n"),
        ]);
        // gDiff (two distinct instances) = False; gSame (aliased) = True.
        assert_eq!(snap.first(), Some(&canon(&Variant::from_bool(false))), "{snap:?}");
        assert_eq!(snap.get(1), Some(&canon(&Variant::from_bool(true))), "{snap:?}");
    }

    #[test]
    fn vm3_typeof_is() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gIsWidget As Boolean\nPublic gIsGadget As Boolean\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  gIsWidget = (TypeOf w Is Widget)\n  gIsGadget = (TypeOf w Is Gadget)\nEnd Sub\n",
            ),
            ("Widget", Class, "' widget\n"),
            ("Gadget", Class, "' gadget\n"),
        ]);
        assert_eq!(snap.first(), Some(&canon(&Variant::from_bool(true))), "{snap:?}");
        assert_eq!(snap.get(1), Some(&canon(&Variant::from_bool(false))), "{snap:?}");
    }

    /// `TypeOf Nothing Is X` is False, not error 91 — and so is the test on an unset object
    /// variable. Closes the `typeof-nothing-raises-91` gap.
    #[test]
    fn vm3_typeof_nothing_is_false() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gNothing As Boolean\nPublic gUnset As Boolean\nSub Main()\n  Dim w As Widget\n  gNothing = (TypeOf Nothing Is Widget)\n  gUnset = (TypeOf w Is Widget)\nEnd Sub\n",
            ),
            ("Widget", Class, "' widget\n"),
        ]);
        // Neither raised (run_obj_ok asserts that); both are False.
        assert_eq!(snap.first(), Some(&canon(&Variant::from_bool(false))), "{snap:?}");
        assert_eq!(snap.get(1), Some(&canon(&Variant::from_bool(false))), "{snap:?}");
    }

    /// An object created in a called proc that faults: the object parks as the fault unwinds
    /// out of the proc, and (the error caught by `On Error Resume Next`) its `Class_Terminate`
    /// runs before `Main` continues — so the post-resume read sees the terminate's effect.
    /// Cross-proc lifecycle coverage the happy-path tests miss (M3-5 review).
    #[test]
    fn vm3_cross_proc_object_terminates_on_caught_fault() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gTerm As Long\nPublic gAfter As Long\nSub Main()\n  On Error Resume Next\n  Foo\n  gAfter = gTerm\nEnd Sub\nSub Foo()\n  Dim w As Widget\n  Set w = New Widget\n  Err.Raise 5\nEnd Sub\n",
            ),
            (
                "Widget",
                Class,
                "Private Sub Class_Terminate()\n  gTerm = gTerm + 1\nEnd Sub\n",
            ),
        ]);
        // gAfter (global 1) saw the terminate effect: gTerm == 1.
        assert_eq!(snap.get(1), Some(&canon(&Variant::from_i32(1))), "{snap:?}");
    }

    /// The fully-uncaught counterpart (regression for the H1 fault-path-drain fix): an object
    /// is created in a called proc that dies with an uncaught error. The run ends Raised (the
    /// terminate effect isn't snapshot-observable), guarding against a crash/divergence as the
    /// stack unwinds and the parked terminate drains.
    #[test]
    fn vm3_uncaught_fault_with_object_raises_cleanly() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let o = run_modules(
            Executor::Vm3,
            &[
                (
                    "Main",
                    Procedural,
                    "Public gTerm As Long\nSub Main()\n  Foo\nEnd Sub\nSub Foo()\n  Dim w As Widget\n  Set w = New Widget\n  Err.Raise 5\nEnd Sub\n",
                ),
                (
                    "Widget",
                    Class,
                    "Private Sub Class_Terminate()\n  gTerm = gTerm + 1\nEnd Sub\n",
                ),
            ],
            "VBAProject",
        );
        assert!(o.raised, "the uncaught Err.Raise 5 must surface as Raised");
        assert_eq!(o.err.number, 5, "raised error number");
    }

    /// A late-bound method call on a typed project instance returns its function result and
    /// passes its ByVal arg (M3-6 project method dispatch via `ComCallLate`).
    #[test]
    fn vm3_project_method_call_returns_result() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gResult As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  gResult = w.Twice(21)\nEnd Sub\n",
            ),
            (
                "Widget",
                Class,
                "Public Function Twice(ByVal n As Long) As Long\n  Twice = n * 2\nEnd Function\n",
            ),
        ]);
        assert_eq!(snap.first(), Some(&canon(&Variant::from_i32(42))), "{snap:?}");
    }

    /// End-to-end project events: a sink wires a source via `WithEvents`, the source
    /// `RaiseEvent`s, and the sink's handler fires with the event arg (M3-6).
    #[test]
    fn vm3_project_event_fires_handler() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gResult As Long\nSub Main()\n  Dim src As Source\n  Set src = New Source\n  Dim snk As Sink\n  Set snk = New Sink\n  snk.Wire src\n  src.Go\nEnd Sub\n",
            ),
            (
                "Source",
                Class,
                "Public Event Fired(ByVal n As Long)\nPublic Sub Go()\n  RaiseEvent Fired(7)\nEnd Sub\n",
            ),
            (
                "Sink",
                Class,
                "Private WithEvents s As Source\nPublic Sub Wire(ByVal src As Source)\n  Set s = src\nEnd Sub\nPrivate Sub s_Fired(ByVal n As Long)\n  gResult = n\nEnd Sub\n",
            ),
        ]);
        // The event handler set gResult = 7.
        assert_eq!(snap.first(), Some(&canon(&Variant::from_i32(7))), "{snap:?}");
    }

    // NB: most VBA built-ins (`Len`, `UCase`, …) lower to a cross-bundle `CallExtern`
    // into the "VBA library" bundle, NOT `CallNative`. As of M3-1 vm3 resolves those against
    // the synthetic `VBA` library bundle and runs them through the same `invoke_native_lib`
    // bridge as `CallNative { Builtin }`, so builtin-using programs run across the corpus.

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

    /// A deterministic single-line rendering of a run's observable (axis 1 snapshot + axis 2
    /// `Err` + completion shape), for the vm3 golden snapshot. `{:?}` on the `Err` keeps it on
    /// one line (newlines in a description escape to `\n`).
    fn render_outcome(o: &RunOutcome) -> String {
        if let Some(what) = &o.unsupported {
            return format!("unsupported({what})");
        }
        let body = match &o.result {
            Ok(values) => format!(
                "ok[{}]",
                values
                    .iter()
                    .map(|c| format!("{c:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Err(msg) => format!("err({msg})"),
        };
        format!("{body} raised={} err={:?}", o.raised, o.err)
    }

    /// W11 — the vm3 GOLDEN SNAPSHOT regression net. Pins vm3's validated observable for every
    /// corpus program: this is the standalone gate that REPLACES the vm2-vs-vm3 differential now
    /// that vm2 is gone (a vm3-minted snapshot, oracle-validated on the captured subset +
    /// vm2-cross-checked on the rest as it was blessed). Drift fails the test; re-bless an
    /// intentional change with `OXVBA_BLESS_GOLDEN=1`.
    #[test]
    fn vm3_golden_snapshot() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let budget = std::time::Duration::from_secs(8);
        let mut lines: Vec<String> = Vec::new();
        for dir in ["conformance", "examples"] {
            for path in bas_files(&root.join(dir)) {
                let Ok(source) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if source.trim().is_empty() {
                    continue;
                }
                let rel = path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let rendered = match run_with_timeout(Executor::Vm3, &source, "Main", budget) {
                    Some(outcome) => render_outcome(&outcome),
                    None => "TIMEOUT".to_string(),
                };
                lines.push(format!("{rel}\t{rendered}"));
            }
        }
        lines.sort();
        let actual = format!("{}\n", lines.join("\n"));
        let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vm3_golden.snap");
        if std::env::var_os("OXVBA_BLESS_GOLDEN").is_some() {
            std::fs::write(&golden, &actual).expect("write golden snapshot");
            eprintln!("blessed vm3 golden snapshot: {} programs", lines.len());
            return;
        }
        let expected = std::fs::read_to_string(&golden).unwrap_or_default();
        if actual != expected {
            let a: Vec<&str> = actual.lines().collect();
            let e: Vec<&str> = expected.lines().collect();
            let detail = match a.iter().zip(e.iter()).position(|(x, y)| x != y) {
                Some(i) => format!(
                    "first drift at line {i}:\n  golden: {}\n  actual: {}",
                    e.get(i).copied().unwrap_or("<none>"),
                    a.get(i).copied().unwrap_or("<none>")
                ),
                None => format!(
                    "length differs: golden {} lines, actual {} lines",
                    e.len(),
                    a.len()
                ),
            };
            panic!(
                "vm3 golden snapshot drift (re-bless with OXVBA_BLESS_GOLDEN=1 if intended).\n{detail}"
            );
        }
    }
}
