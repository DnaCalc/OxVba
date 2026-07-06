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

use oxvba_host::{Engine, FinalErr, HostConfig, RuntimeProfileId, SnapshotOutcome, Vm3Snapshot};
use oxvba_runtime::variant::VarType;
use oxvba_runtime::{HandleBalance, Variant, live_handle_counts, variant_to_vba_string};

fn balance_measurement_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

fn differential_engine(config: HostConfig) -> Engine {
    Engine::new(config).with_runtime_profile(RuntimeProfileId::WindowsHeadless)
}

fn vm3_oracle_engine() -> Engine {
    differential_engine(HostConfig::vm3())
}

fn jit_candidate_engine() -> Engine {
    differential_engine(HostConfig::jit())
}

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
    Float {
        tag: u16,
        bits: u64,
    },
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
    Opaque {
        tag: u16,
    },
}

fn canon_f32_bits(x: f32) -> u32 {
    if x.is_nan() {
        f32::NAN.to_bits()
    } else {
        x.to_bits()
    }
}

fn canon_f64_bits(x: f64) -> u64 {
    if x.is_nan() {
        f64::NAN.to_bits()
    } else {
        x.to_bits()
    }
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
    /// The M4 Cranelift backend. During M4-3 it compiles a narrow straight-line Long slice
    /// and cleanly declines out-of-scope OxIR shapes.
    Jit,
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
    /// Live-handle delta over this run after snapshot values have been canonicalized and dropped.
    pub handle_balance: Option<HandleBalance>,
}

impl RunOutcome {
    fn unsupported(what: impl Into<String>) -> Self {
        RunOutcome {
            result: Ok(Vec::new()),
            err: FinalErr::default(),
            raised: false,
            unsupported: Some(what.into()),
            handle_balance: None,
        }
    }

    fn from_snapshot(outcome: SnapshotOutcome) -> Self {
        match outcome {
            SnapshotOutcome::Completed { values, err } => RunOutcome {
                result: Ok(values.iter().map(canon).collect()),
                err,
                raised: false,
                unsupported: None,
                handle_balance: None,
            },
            SnapshotOutcome::Raised { err } => RunOutcome {
                result: Err(format!("VBA error {}", err.number)),
                err,
                raised: true,
                unsupported: None,
                handle_balance: None,
            },
            SnapshotOutcome::Unsupported(what) => RunOutcome {
                result: Ok(Vec::new()),
                err: FinalErr::default(),
                raised: false,
                unsupported: Some(what),
                handle_balance: None,
            },
            SnapshotOutcome::Failed(msg) => RunOutcome {
                result: Err(msg),
                err: FinalErr::default(),
                raised: false,
                unsupported: None,
                handle_balance: None,
            },
        }
    }

    fn with_handle_balance(mut self, before: oxvba_runtime::LiveHandleCounts) -> Self {
        self.handle_balance = Some(before.balance_to(live_handle_counts()));
        self
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
        conditional_compilation_target: Default::default(),
    };
    let _balance_guard = balance_measurement_lock()
        .lock()
        .expect("balance measurement lock poisoned");
    let before = live_handle_counts();
    let engine = vm3_oracle_engine();
    let outcome = match executor {
        Executor::Vm3 => {
            RunOutcome::from_snapshot(engine.execute_manifest_snapshot_with_err_vm3(&manifest))
        }
        Executor::Jit => RunOutcome::from_snapshot(
            jit_candidate_engine().execute_manifest_snapshot_with_err_jit(&manifest),
        ),
    };
    outcome.with_handle_balance(before)
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
        conditional_compilation_target: Default::default(),
    };
    let _balance_guard = balance_measurement_lock()
        .lock()
        .expect("balance measurement lock poisoned");
    let before = live_handle_counts();
    let engine = vm3_oracle_engine();
    let outcome = match executor {
        Executor::Vm3 => {
            RunOutcome::from_snapshot(engine.execute_manifest_snapshot_with_err_vm3(&manifest))
        }
        Executor::Jit => RunOutcome::from_snapshot(
            jit_candidate_engine().execute_manifest_snapshot_with_err_jit(&manifest),
        ),
    };
    outcome.with_handle_balance(before)
}

/// Run a leaf-first project closure under `executor` and capture the entry project's globals.
///
/// This is the differential counterpart of
/// [`oxvba_host::Engine::execute_project_closure_with_variant_snapshot_vm3`]. It is used for
/// reference-project fixtures where a single manifest is not enough to exercise the production
/// cross-project binding/linking path.
pub fn run_project_closure(
    executor: Executor,
    closure_leaf_first: &[oxvba_symbol::manifest::SymbolProjectManifest],
) -> RunOutcome {
    let _balance_guard = balance_measurement_lock()
        .lock()
        .expect("balance measurement lock poisoned");
    let before = live_handle_counts();
    let engine = vm3_oracle_engine();
    let outcome = match executor {
        Executor::Vm3 => {
            match engine.execute_project_closure_with_variant_snapshot_vm3(closure_leaf_first) {
                Vm3Snapshot::Ran(values) => RunOutcome {
                    result: Ok(values.iter().map(canon).collect()),
                    err: FinalErr::default(),
                    raised: false,
                    unsupported: None,
                    handle_balance: None,
                },
                Vm3Snapshot::Unsupported(what) => RunOutcome {
                    result: Ok(Vec::new()),
                    err: FinalErr::default(),
                    raised: false,
                    unsupported: Some(what),
                    handle_balance: None,
                },
                Vm3Snapshot::Failed(msg) => RunOutcome {
                    result: Err(msg),
                    err: FinalErr::default(),
                    raised: false,
                    unsupported: None,
                    handle_balance: None,
                },
            }
        }
        Executor::Jit => {
            if closure_leaf_first.len() != 1 {
                RunOutcome::unsupported("M4-3 JIT supports one-project closure execution only")
            } else {
                match jit_candidate_engine()
                    .execute_project_closure_with_variant_snapshot(closure_leaf_first)
                {
                    Ok(values) => RunOutcome {
                        result: Ok(values.iter().map(canon).collect()),
                        err: FinalErr::default(),
                        raised: false,
                        unsupported: None,
                        handle_balance: None,
                    },
                    Err(err) => {
                        if err.diagnostic().code.as_str().contains("JIT-UNSUPPORTED") {
                            RunOutcome::unsupported(err.message().to_string())
                        } else {
                            RunOutcome {
                                result: Err(err.message().to_string()),
                                err: FinalErr::default(),
                                raised: false,
                                unsupported: None,
                                handle_balance: None,
                            }
                        }
                    }
                }
            }
        }
    };
    outcome.with_handle_balance(before)
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

    const JIT_STRAIGHT_LINE_LONG: &str = "\
Public g As Long
Sub Main()
  Dim n As Long
  n = 10
  n = n + 5
  n = n * 2
  g = n - 3
End Sub
";

    const JIT_LONG_OVERFLOW: &str = "\
Public g As Long
Sub Main()
  Dim n As Long
  n = 2147483647
  g = n + 1
End Sub
";

    const JIT_LONGLONG_ARITHMETIC: &str = "\
Public g As LongLong
Sub Main()
  Dim n As LongLong
  n = 5000000000^
  n = n + 12^
  n = n * 2^
  g = n - 4^
End Sub
";

    const JIT_LONGLONG_OVERFLOW: &str = "\
Public g As LongLong
Sub Main()
  Dim n As LongLong
  n = 3037000500^
  g = n * n
End Sub
";

    const JIT_CURRENCY_ARITHMETIC: &str = "\
Public g As Currency
Sub Main()
  Dim n As Currency
  n = 12.3456@
  n = n + 0.0004@
  n = n * 2@
  g = n - 1@
End Sub
";

    const JIT_CURRENCY_OVERFLOW: &str = "\
Public g As Currency
Sub Main()
  Dim n As Currency
  n = 922337203685477.5807@
  g = n + 0.0001@
End Sub
";

    const JIT_CURRENCY_TRUTHY_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim c As Currency
  c = 0.0001@
  If c Then
    g = 1
  Else
    g = 2
  End If
End Sub
";

    const JIT_CURRENCY_COMPARE_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim a As Currency
  Dim b As Currency
  a = 12.3456@
  b = 12.3400@
  If a > b Then
    g = 1
  Else
    g = 2
  End If
End Sub
";

    const JIT_LONG_NEGATION: &str = "\
Public g As Long
Sub Main()
  Dim n As Long
  n = 7
  g = -n
End Sub
";

    const JIT_LONGLONG_NEGATION: &str = "\
Public g As LongLong
Sub Main()
  Dim n As LongLong
  n = 5000000000^
  g = -n
End Sub
";

    const JIT_CURRENCY_NEGATION: &str = "\
Public g As Currency
Sub Main()
  Dim c As Currency
  c = 12.3456@
  g = -c
End Sub
";

    const JIT_SINGLE_NEGATION: &str = "\
Public g As Single
Sub Main()
  Dim s As Single
  s = 1.25!
  g = -s
End Sub
";

    const JIT_DOUBLE_NEGATION: &str = "\
Public g As Double
Sub Main()
  Dim d As Double
  d = 2.5
  g = -d
End Sub
";

    const JIT_LONG_INTDIV_MOD: &str = "\
Public g As Long
Sub Main()
  Dim n As Long
  n = 17
  g = (n \\ 5) + (n Mod 5)
End Sub
";

    const JIT_LONGLONG_INTDIV_MOD: &str = "\
Public g As LongLong
Sub Main()
  Dim n As LongLong
  n = 5000000017^
  g = (n \\ 5^) + (n Mod 5^)
End Sub
";

    const JIT_DOUBLE_DIVISION: &str = "\
Public g As Double
Sub Main()
  Dim d As Double
  d = 9#
  g = d / 2#
End Sub
";

    const JIT_DOUBLE_EXPONENTIATION: &str = "\
Public g As Double
Sub Main()
  Dim d As Double
  d = 3#
  g = d ^ 4#
End Sub
";

    const JIT_BUILTIN_ABS_LONG: &str = "\
Public g As Long
Sub Main()
  Dim n As Long
  n = -7
  g = Abs(n)
End Sub
";

    const JIT_BUILTIN_ABS_INTEGER: &str = "\
Public g As Integer
Sub Main()
  Dim n As Integer
  n = -5
  g = Abs(n)
End Sub
";

    const JIT_BUILTIN_ABS_INTEGER_MIN: &str = "\
Public g As Long
Sub Main()
  Dim n As Integer
  n = -32768
  g = Abs(n)
End Sub
";

    const JIT_BUILTIN_ABS_LONG_MIN: &str = "\
Public g As Double
Sub Main()
  Dim n As Long
  n = &H80000000
  g = Abs(n)
End Sub
";

    const JIT_BUILTIN_ABS_BOOL: &str = "\
Public g As Integer
Sub Main()
  Dim b As Boolean
  b = True
  g = Abs(b)
End Sub
";

    const JIT_BUILTIN_ABS_EMPTY: &str = "\
Public g As Integer
Sub Main()
  g = Abs(Empty)
End Sub
";

    const JIT_BUILTIN_ABS_NULL: &str = "\
Public g As Variant
Sub Main()
  g = Abs(Null)
End Sub
";

    const JIT_BUILTIN_ABS_DOUBLE: &str = "\
Public g As Double
Sub Main()
  Dim d As Double
  d = -2.5
  g = Abs(d)
End Sub
";

    const JIT_BUILTIN_ABS_SINGLE: &str = "\
Public g As Single
Sub Main()
  Dim s As Single
  s = -1.25!
  g = Abs(s)
End Sub
";

    const JIT_BUILTIN_ABS_CURRENCY: &str = "\
Public g As Currency
Sub Main()
  Dim c As Currency
  c = -12.3456@
  g = Abs(c)
End Sub
";

    const JIT_BUILTIN_ABS_LONGLONG: &str = "\
Public g As LongLong
Sub Main()
  Dim n As LongLong
  n = -5000000017^
  g = Abs(n)
End Sub
";

    const JIT_BUILTIN_INT_DOUBLE: &str = "\
Public g As Double
Sub Main()
  Dim d As Double
  d = -2.5
  g = Int(d)
End Sub
";

    const JIT_BUILTIN_INT_LONG: &str = "\
Public g As Long
Sub Main()
  Dim n As Long
  n = -5
  g = Int(n)
End Sub
";

    const JIT_BUILTIN_INT_INTEGER: &str = "\
Public g As Integer
Sub Main()
  Dim n As Integer
  n = -5
  g = Int(n)
End Sub
";

    const JIT_BUILTIN_INT_LONGLONG: &str = "\
Public g As LongLong
Sub Main()
  Dim n As LongLong
  n = -5000000017^
  g = Int(n)
End Sub
";

    const JIT_BUILTIN_INT_SINGLE: &str = "\
Public g As Single
Sub Main()
  Dim s As Single
  s = -2.5!
  g = Int(s)
End Sub
";

    const JIT_BUILTIN_INT_BOOL: &str = "\
Public g As Integer
Sub Main()
  Dim b As Boolean
  b = True
  g = Int(b)
End Sub
";

    const JIT_BUILTIN_INT_EMPTY: &str = "\
Public g As Integer
Sub Main()
  g = Int(Empty)
End Sub
";

    const JIT_BUILTIN_INT_NULL: &str = "\
Public g As Variant
Sub Main()
  g = Int(Null)
End Sub
";

    const JIT_BUILTIN_FIX_DOUBLE: &str = "\
Public g As Double
Sub Main()
  Dim d As Double
  d = -2.5
  g = Fix(d)
End Sub
";

    const JIT_BUILTIN_FIX_INTEGER: &str = "\
Public g As Integer
Sub Main()
  Dim n As Integer
  n = -5
  g = Fix(n)
End Sub
";

    const JIT_BUILTIN_FIX_LONG: &str = "\
Public g As Long
Sub Main()
  Dim n As Long
  n = -5
  g = Fix(n)
End Sub
";

    const JIT_BUILTIN_FIX_LONGLONG: &str = "\
Public g As LongLong
Sub Main()
  Dim n As LongLong
  n = -5000000017^
  g = Fix(n)
End Sub
";

    const JIT_BUILTIN_FIX_SINGLE: &str = "\
Public g As Single
Sub Main()
  Dim s As Single
  s = -2.5!
  g = Fix(s)
End Sub
";

    const JIT_BUILTIN_FIX_BOOL: &str = "\
Public g As Integer
Sub Main()
  Dim b As Boolean
  b = True
  g = Fix(b)
End Sub
";

    const JIT_BUILTIN_FIX_EMPTY: &str = "\
Public g As Integer
Sub Main()
  g = Fix(Empty)
End Sub
";

    const JIT_BUILTIN_FIX_NULL: &str = "\
Public g As Variant
Sub Main()
  g = Fix(Null)
End Sub
";

    const JIT_BUILTIN_INT_CURRENCY: &str = "\
Public g As Currency
Sub Main()
  Dim c As Currency
  c = -5.7@
  g = Int(c)
End Sub
";

    const JIT_BUILTIN_FIX_CURRENCY: &str = "\
Public g As Currency
Sub Main()
  Dim c As Currency
  c = -5.7@
  g = Fix(c)
End Sub
";

    const JIT_BUILTIN_INT_DATE: &str = "\
Public g As Date
Sub Main()
  Dim d As Date
  d = #2020-01-15# + 0.75
  g = Int(d)
End Sub
";

    const JIT_BUILTIN_FIX_DATE: &str = "\
Public g As Date
Sub Main()
  Dim d As Date
  d = #2020-01-15# + 0.75
  g = Fix(d)
End Sub
";

    const JIT_BUILTIN_SGN_DOUBLE: &str = "\
Public g As Integer
Sub Main()
  Dim d As Double
  d = -2.5
  g = Sgn(d)
End Sub
";

    const JIT_BUILTIN_SGN_LONG: &str = "\
Public g As Integer
Sub Main()
  Dim n As Long
  n = -42
  g = Sgn(n)
End Sub
";

    const JIT_BUILTIN_SGN_INTEGER: &str = "\
Public g As Integer
Sub Main()
  Dim n As Integer
  n = -5
  g = Sgn(n)
End Sub
";

    const JIT_BUILTIN_SGN_BOOL: &str = "\
Public g As Integer
Sub Main()
  Dim b As Boolean
  b = True
  g = Sgn(b)
End Sub
";

    const JIT_BUILTIN_SGN_EMPTY: &str = "\
Public g As Integer
Sub Main()
  g = Sgn(Empty)
End Sub
";

    const JIT_BUILTIN_SGN_ZERO: &str = "\
Public g As Integer
Sub Main()
  g = Sgn(0)
End Sub
";

    const JIT_BUILTIN_SGN_NULL: &str = "\
Public g As Integer
Sub Main()
  g = Sgn(Null)
End Sub
";

    const JIT_BUILTIN_SGN_LONGLONG: &str = "\
Public g As Integer
Sub Main()
  Dim n As LongLong
  n = 5000000017^
  g = Sgn(n)
End Sub
";

    const JIT_BUILTIN_SGN_SINGLE: &str = "\
Public g As Integer
Sub Main()
  Dim s As Single
  s = -1.25!
  g = Sgn(s)
End Sub
";

    const JIT_BUILTIN_SGN_CURRENCY: &str = "\
Public g As Integer
Sub Main()
  Dim c As Currency
  c = -5.7@
  g = Sgn(c)
End Sub
";

    const JIT_BUILTIN_CBOOL_EXPR: &str = "\
Public g As Boolean
Sub Main()
  g = CBool(2)
End Sub
";

    const JIT_BUILTIN_CBYTE_EXPR: &str = "\
Public g As Byte
Sub Main()
  g = CByte(13.5)
End Sub
";

    const JIT_BUILTIN_CINT_EXPR: &str = "\
Public g As Integer
Sub Main()
  g = CInt(13.5)
End Sub
";

    const JIT_BUILTIN_CLNG_EXPR: &str = "\
Public g As Long
Sub Main()
  g = CLng(42.5)
End Sub
";

    const JIT_BUILTIN_CLNGLNG_EXPR: &str = "\
Public g As LongLong
Sub Main()
  g = CLngLng(5000000013.5#)
End Sub
";

    const JIT_BUILTIN_CLNGPTR_EXPR: &str = "\
Public g As LongPtr
Sub Main()
  g = CLngPtr(5000000013.5#)
End Sub
";

    const JIT_BUILTIN_CSNG_EXPR: &str = "\
Public g As Single
Sub Main()
  g = CSng(1.25#)
End Sub
";

    const JIT_BUILTIN_CDBL_EXPR: &str = "\
Public g As Double
Sub Main()
  g = CDbl(12)
End Sub
";

    const JIT_BUILTIN_CCUR_EXPR: &str = "\
Public g As Currency
Sub Main()
  g = CCur(12.3456#)
End Sub
";

    const JIT_BUILTIN_CDATE_EXPR: &str = "\
Public g As Date
Sub Main()
  g = CDate(36527#)
End Sub
";

    const JIT_BUILTIN_CSTR_EXPR: &str = "\
Public g As Variant
Sub Main()
  g = CStr(42&)
End Sub
";

    const JIT_BUILTIN_CDEC_EXPR: &str = "\
Public g As Variant
Sub Main()
  g = CDec(10)
End Sub
";

    const JIT_BUILTIN_CVAR_EXPR: &str = "\
Public g As Variant
Sub Main()
  g = CVar(42&)
End Sub
";

    const JIT_BUILTIN_CVERR_EXPR: &str = "\
Public g As Variant
Sub Main()
  g = CVErr(2042)
End Sub
";

    const JIT_BUILTIN_CVERR_INVALID: &str = "\
Public g As Variant
Sub Main()
  g = CVErr(65536)
End Sub
";

    const JIT_BUILTIN_HEX_EXPR: &str = "\
Public g As Variant
Sub Main()
  g = Hex(255&)
End Sub
";

    const JIT_BUILTIN_OCT_EXPR: &str = "\
Public g As Variant
Sub Main()
  g = Oct(9&)
End Sub
";

    const JIT_BUILTIN_HEX_OCT_NEGATIVE_WIDTH_EXPRS: &str = "\
Public ghi As Variant
Public ghl As Variant
Public ghll As Variant
Public goi As Variant
Public gol As Variant
Public goll As Variant
Sub Main()
  Dim i As Integer
  Dim l As Long
  Dim ll As LongLong
  i = -1
  l = -1
  ll = -1
  ghi = Hex(i)
  ghl = Hex(l)
  ghll = Hex(ll)
  goi = Oct(i)
  gol = Oct(l)
  goll = Oct(ll)
End Sub
";

    const JIT_BUILTIN_STR_EXPR: &str = "\
Public g As Variant
Sub Main()
  g = Str(42&)
End Sub
";

    const JIT_BUILTIN_STRING_RESULT_DESTINATIONS: &str = "\
Public gcstr As String
Public ghex As String
Public gchr As String
Public gspace As String
Public gleft As String
Public gmid As String
Public grepl As String
Sub Main()
  gcstr = CStr(42&)
  ghex = Hex(255&)
  gchr = ChrW(65&)
  gspace = Space(3&)
  gleft = Left(\"12345\", 2)
  gmid = Mid(\"12345\", 2, 3)
  grepl = Replace(\"123123\", \"23\", \"99\")
End Sub
";

    const JIT_BUILTIN_STRING_TYPED_ALIASES: &str = "\
Public glower As Variant
Public gupper As Variant
Public gtrim As Variant
Public gleft As Variant
Public gright As Variant
Public gmid As Variant
Public gchr As Variant
Public gspace As Variant
Public gstring As Variant
Sub Main()
  glower = LCase$(\"AB\")
  gupper = UCase$(\"ab\")
  gtrim = Trim$(\"  x  \")
  gleft = Left$(\"12345\", 2)
  gright = Right$(\"12345\", 2)
  gmid = Mid$(\"12345\", 2, 3)
  gchr = ChrW$(65&)
  gspace = Space$(2&)
  gstring = String$(2&, 65&)
End Sub
";

    const JIT_BUILTIN_STRING_TYPED_ALIAS_NULL_ERROR: &str = "\
Public g As Variant
Sub Main()
  Dim value As Variant
  value = Null
  g = Left$(value, 2)
End Sub
";

    const JIT_BUILTIN_STRING_TYPED_ALIAS_DESTINATIONS: &str = "\
Public glower As String
Public gupper As String
Public gtrim As String
Public gleft As String
Public gright As String
Public gleftb As String
Public grightb As String
Public gmid As String
Public gchr As String
Public gspace As String
Public gstring As String
Sub Main()
  glower = LCase$(\"AB\")
  gupper = UCase$(\"ab\")
  gtrim = Trim$(\"  x  \")
  gleft = Left$(\"12345\", 2)
  gright = Right$(\"12345\", 2)
  gleftb = LeftB$(\"12345\", 2)
  grightb = RightB$(\"12345\", 2)
  gmid = Mid$(\"12345\", 2, 3)
  gchr = ChrW$(65&)
  gspace = Space$(2&)
  gstring = String$(2&, 65&)
End Sub
";

    const JIT_STRING_DESTINATION_NULL_VARIANT: &str = "\
Public gs As String
Sub Main()
  Dim value As Variant
  value = Null
  gs = value
End Sub
";

    const JIT_FIXED_STRING_LOCAL_PAD_TRUNCATE: &str = "\
Public gdefault As String
Public gpad As String
Public gtrunc As String
Sub Main()
  Dim fixedDefault As String * 3
  Dim fixedPad As String * 3
  Dim fixedTrunc As String * 3
  fixedPad = \"ab\"
  fixedTrunc = \"abcd\"
  gdefault = fixedDefault
  gpad = fixedPad
  gtrunc = fixedTrunc
End Sub
";

    const JIT_FIXED_STRING_LOCAL_NULL_ERROR: &str = "\
Public g As String
Sub Main()
  Dim fixed As String * 3
  fixed = Null
  g = fixed
End Sub
";

    const JIT_FIXED_STRING_GLOBAL_PAD_TRUNCATE: &str = "\
Public gdefault As String
Public gpad As String
Public gtrunc As String
Public fixedDefault As String * 3
Public fixedPad As String * 3
Public fixedTrunc As String * 3
Sub Main()
  fixedPad = \"ab\"
  fixedTrunc = \"abcd\"
  gdefault = fixedDefault
  gpad = fixedPad
  gtrunc = fixedTrunc
End Sub
";

    const JIT_FIXED_STRING_GLOBAL_NULL_ERROR: &str = "\
Public fixed As String * 3
Sub Main()
  fixed = Null
End Sub
";

    const JIT_BUILTIN_SCALAR_RESULT_DESTINATIONS: &str = "\
Public glen As Long
Public glenb As Long
Public ginstr As Long
Public gval As Double
Public gisnull As Boolean
Sub Main()
  glen = Len(\"abcd\")
  glenb = LenB(\"abcd\")
  ginstr = InStr(\"123123\", \"23\")
  gval = Val(\"1234\")
  gisnull = IsNull(Null)
End Sub
";

    const JIT_BUILTIN_SCALAR_RESULT_DESTINATION_FAMILIES: &str = "\
Public ground As Double
Public gyear As Long
Public gdate As Date
Public gtime As Date
Public grgb As Long
Public gqb As Long
Public gisdate As Boolean
Sub Main()
  Dim x As Double
  Dim digits As Long
  Dim d As Date
  Dim r As Long
  Dim g As Long
  Dim b As Long
  Dim q As Long
  x = 2.25#
  digits = 1
  d = #2020-01-16# + 0.5515625
  r = 0
  g = 0
  b = 1
  q = 1
  ground = Round(x, digits)
  gyear = Year(d)
  gdate = DateValue(d)
  gtime = TimeValue(d)
  grgb = RGB(r, g, b)
  gqb = QBColor(q)
  gisdate = IsDate(d)
End Sub
";

    const JIT_BUILTIN_SCALAR_RESULT_DESTINATION_COERCIONS: &str = "\
Public gb As Byte
Public gi As Integer
Public gs As Single
Public gd As Double
Public gc As Currency
Public gbool As Boolean
Sub Main()
  gb = Len(\"abcd\")
  gi = InStr(\"123123\", \"23\")
  gs = LenB(\"abcd\")
  gd = Len(\"abcd\")
  gc = Val(\"12.3456\")
  gbool = Len(\"x\")
End Sub
";

    const JIT_BUILTIN_SCALAR_RESULT_DESTINATION_COERCION_OVERFLOW: &str = "\
Public gb As Byte
Sub Main()
  gb = Len(\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\")
End Sub
";

    const JIT_BUILTIN_SCALAR_RESULT_DESTINATION_NULL_ERROR: &str = "\
Public g As Long
Sub Main()
  g = Abs(Null)
End Sub
";

    const JIT_BUILTIN_MATH_UNARY_EXPRS: &str = "\
Public gsqr As Variant
Public gsin As Variant
Public gcos As Variant
Public glog As Variant
Public gexp As Variant
Public gatn As Variant
Public gtan As Variant
Sub Main()
  Dim positive As Double
  positive = 9#
  Dim zero As Double
  zero = 0#
  Dim one As Double
  one = 1#
  gsqr = Sqr(positive)
  gsin = Sin(zero)
  gcos = Cos(zero)
  glog = Log(one)
  gexp = Exp(zero)
  gatn = Atn(one)
  gtan = Tan(zero)
End Sub
";

    const JIT_BUILTIN_SQR_INVALID: &str = "\
Public g As Variant
Sub Main()
  Dim x As Double
  x = -1#
  g = Sqr(x)
End Sub
";

    const JIT_BUILTIN_LOG_INVALID: &str = "\
Public g As Variant
Sub Main()
  Dim x As Double
  x = 0#
  g = Log(x)
End Sub
";

    const JIT_BUILTIN_EXP_OVERFLOW: &str = "\
Public g As Variant
Sub Main()
  Dim x As Double
  x = 1000#
  g = Exp(x)
End Sub
";

    const JIT_BUILTIN_ROUND_EXPR: &str = "\
Public g As Variant
Sub Main()
  Dim x As Double
  x = 2.5#
  g = Round(x)
End Sub
";

    const JIT_BUILTIN_ROUND_DIGITS_EXPR: &str = "\
Public g As Variant
Sub Main()
  Dim x As Double
  Dim digits As Long
  x = 2.25#
  digits = 1
  g = Round(x, digits)
End Sub
";

    const JIT_BUILTIN_ROUND_NEGATIVE_DIGITS: &str = "\
Public g As Variant
Sub Main()
  Dim x As Double
  Dim digits As Long
  x = 19#
  digits = -1
  g = Round(x, digits)
End Sub
";

    const JIT_BUILTIN_DATE_PART_EXPRS: &str = "\
Public gy As Variant
Public gm As Variant
Public gd As Variant
Public gh As Variant
Public gmi As Variant
Public gs As Variant
Sub Main()
  Dim d As Date
  d = #2020-01-16# + 0.5515625
  gy = Year(d)
  gm = Month(d)
  gd = Day(d)
  gh = Hour(d)
  gmi = Minute(d)
  gs = Second(d)
End Sub
";

    const JIT_BUILTIN_INFORMATION_EXPRS: &str = "\
Public gd As Date
Public gvErr As Variant
Public gVarType As Variant
Public gTypeName As Variant
Public gIsNumeric As Variant
Public gIsDate As Variant
Public gIsObject As Variant
Public gIsNull As Variant
Public gIsEmpty As Variant
Public gIsError As Variant
Sub Main()
  gd = #2020-01-16#
  gvErr = CVErr(2042)
  gVarType = VarType(gd)
  gTypeName = TypeName(gd)
  gIsNumeric = IsNumeric(42&)
  gIsDate = IsDate(gd)
  gIsObject = IsObject(17)
  gIsNull = IsNull(Null)
  gIsEmpty = IsEmpty(Empty)
  gIsError = IsError(gvErr)
End Sub
";

    const JIT_STDLIB_VARIANT_PREDICATES: &str =
        include_str!("../../../conformance/tests/stdlib_variant_predicates.bas");

    const JIT_COERCION_NULL_EMPTY_ERROR_PREDICATES: &str =
        include_str!("../../../conformance/tests/coercion_null_empty_error_predicates.bas");

    const JIT_INTROSPECTION_VARTYPE_ISNUMERIC_TAGS: &str =
        include_str!("../../../conformance/tests/introspection_vartype_isnumeric_tags.bas");

    const JIT_STRING_VBNULLSTRING_PREDICATES: &str =
        include_str!("../../../conformance/tests/string_vbnullstring_predicates.bas");

    const JIT_BUILTIN_ISARRAY_DYNAMIC_ARRAY_EXPRS: &str = "\
Sub Main()
Dim score As Long
Dim a()
If IsArray(a) Then score = score + 1
ReDim a(1)
If IsArray(a) Then score = score + 2
Erase a
If IsArray(a) Then score = score + 4
End Sub
";

    const JIT_BUILTIN_ISARRAY_VARIANT_CARRIER_EXPRS: &str = "\
Sub Main()
Dim score As Long
Dim v
If IsArray(v) Then score = score + 1
v = Array(1, 2)
If IsArray(v) Then score = score + 2
End Sub
";

    const JIT_BUILTIN_DATE_VALUE_TIME_VALUE_EXPRS: &str = "\
Public gdv As Variant
Public gtv As Variant
Sub Main()
  Dim d As Date
  d = #2020-01-16# + 0.5515625
  gdv = DateValue(d)
  gtv = TimeValue(d)
End Sub
";

    const JIT_STDLIB_DATE_STRING_POLICY: &str =
        include_str!("../../../conformance/tests/stdlib_date_string_policy.bas");
    const JIT_STDLIB_DATETIME_EXPANSION: &str =
        include_str!("../../../conformance/tests/stdlib_datetime_expansion.bas");
    const JIT_STDLIB_DATE_SERIAL_VALUE: &str =
        include_str!("../../../conformance/tests/stdlib_date_serial_value.bas");
    const JIT_STDLIB_TIME_SERIAL_VALUE: &str =
        include_str!("../../../conformance/tests/stdlib_time_serial_value.bas");
    const JIT_STDLIB_DATE_ADD_DIFF: &str =
        include_str!("../../../conformance/tests/stdlib_date_add_diff.bas");
    const JIT_STDLIB_LEN_BASIC: &str =
        include_str!("../../../conformance/tests/stdlib_len_basic.bas");
    const JIT_STDLIB_SLICE_OPS: &str =
        include_str!("../../../conformance/tests/stdlib_slice_ops.bas");
    const JIT_STDLIB_INSTR_CASE_OPS: &str =
        include_str!("../../../conformance/tests/stdlib_instr_case_ops.bas");
    const JIT_STDLIB_ADVANCED_INSTRREV_LIKE: &str =
        include_str!("../../../conformance/tests/stdlib_advanced_instrrev_like.bas");
    const JIT_STDLIB_ADVANCED_REPLACE_TRIM: &str =
        include_str!("../../../conformance/tests/stdlib_advanced_replace_trim.bas");
    const JIT_STDLIB_ADVANCED_STRCOMP: &str =
        include_str!("../../../conformance/tests/stdlib_advanced_strcomp.bas");
    const JIT_STDLIB_ADVANCED_SPLIT_JOIN: &str =
        include_str!("../../../conformance/tests/stdlib_advanced_split_join.bas");
    const JIT_STDLIB_STRING_EXPANSION_CORE: &str =
        include_str!("../../../conformance/tests/stdlib_string_expansion_core.bas");
    const JIT_STDLIB_FORMAT_CORE: &str =
        include_str!("../../../conformance/tests/stdlib_format_core.bas");
    const JIT_STDLIB_FINANCIAL_ZERO_RATE: &str =
        include_str!("../../../conformance/tests/stdlib_financial_zero_rate.bas");
    const JIT_FINANCIAL_ALGORITHM_RATE_NPER_SUBSET: &str =
        include_str!("../../../conformance/tests/financial_algorithm_rate_nper_subset.bas");
    const JIT_STDLIB_RND_ISOLATED: &str =
        include_str!("../../../conformance/tests/stdlib_rnd_isolated.bas");
    const JIT_STDLIB_NUMERIC_EXPANSION: &str =
        include_str!("../../../conformance/tests/stdlib_numeric_expansion.bas");
    const JIT_CONVERSION_EXTENDED_SCALAR_SUBSET: &str =
        include_str!("../../../conformance/tests/conversion_extended_scalar_subset.bas");
    const JIT_CONVERSION_CINT_BASIC: &str =
        include_str!("../../../conformance/tests/conversion_cint_basic.bas");
    const JIT_STDLIB_ERROR_CVERR_IDENTITY: &str =
        include_str!("../../../conformance/tests/stdlib_error_cverr_identity.bas");
    const JIT_STDLIB_ERROR_ERR_RAISE_FAIL: &str =
        include_str!("../../../conformance/tests/stdlib_error_err_raise_fail.bas");
    const JIT_STDLIB_ERROR_ERR_RAISE_RESUME: &str =
        include_str!("../../../conformance/tests/stdlib_error_err_raise_resume.bas");
    const JIT_ON_ERROR_RESUME_NEXT: &str =
        include_str!("../../../conformance/tests/on_error_resume_next.bas");
    const JIT_ON_ERROR_RESUME_CONTINUE: &str =
        include_str!("../../../conformance/tests/on_error_resume_continue.bas");
    const JIT_ON_ERROR_DEFAULT_FAIL: &str =
        include_str!("../../../conformance/tests/on_error_default_fail.bas");
    const JIT_ON_ERROR_GOTO_ZERO_FAIL: &str =
        include_str!("../../../conformance/tests/on_error_goto_zero_fail.bas");
    const JIT_RESUME_NEXT_STATEMENT_OK: &str =
        include_str!("../../../conformance/tests/resume_next_statement_ok.bas");
    const JIT_ERR_RESUME_NEXT_CLEARS: &str =
        include_str!("../../../conformance/tests/err_resume_next_clears.bas");
    const JIT_RESUME_STATEMENT_BASIC: &str =
        include_str!("../../../conformance/tests/resume_statement_basic.bas");
    const JIT_RESUME_LABEL_BASIC: &str =
        include_str!("../../../conformance/tests/resume_label_basic.bas");
    const JIT_ON_ERROR_GOTO_LABEL_RESUME: &str =
        include_str!("../../../conformance/tests/on_error_goto_label_resume.bas");
    const JIT_ERROR_GOTO_LABEL_RESUME_NEXT: &str =
        include_str!("../../../conformance/tests/error_goto_label_resume_next.bas");
    const JIT_ERR_CLEAR_BASIC: &str =
        include_str!("../../../conformance/tests/err_clear_basic.bas");
    const JIT_ERROR_RAISE_CUSTOM_CLEAR_CYCLE: &str =
        include_str!("../../../conformance/tests/error_raise_custom_clear_cycle.bas");
    const JIT_ERR_PROC_CALL_BOUNDARY_CLEARS: &str =
        include_str!("../../../conformance/tests/err_proc_call_boundary_clears.bas");
    const JIT_ERR_SURFACE_FIELDS_SUBSET: &str =
        include_str!("../../../conformance/tests/err_surface_fields_subset.bas");
    const JIT_ERR_CLEAR_FULL_SURFACE_RESET: &str =
        include_str!("../../../conformance/tests/err_clear_full_surface_reset.bas");
    const JIT_STDLIB_MATH_PRIMITIVES: &str =
        include_str!("../../../conformance/tests/stdlib_math_primitives.bas");
    const JIT_STDLIB_MATH_TRANSCENDENTAL_IDENTITY: &str =
        include_str!("../../../conformance/tests/stdlib_math_transcendental_identity.bas");
    const JIT_CONVERSION_VAL_STR_SUBSET: &str =
        include_str!("../../../conformance/tests/conversion_val_str_subset.bas");
    const JIT_CONVERSION_CLNG_CINT_CHAIN: &str =
        include_str!("../../../conformance/tests/conversion_clng_cint_chain.bas");
    const JIT_CONVERSION_NESTED_CLNG_CINT: &str =
        include_str!("../../../conformance/tests/conversion_nested_clng_cint.bas");

    const JIT_BUILTIN_DATE_SERIAL_TIME_SERIAL_EXPRS: &str = "\
Public gds As Variant
Public gts As Variant
Sub Main()
  Dim y As Long
  Dim m As Long
  Dim d As Long
  Dim h As Long
  Dim n As Long
  Dim s As Long
  y = 2020
  m = 1
  d = 16
  h = 13
  n = 14
  s = 15
  gds = DateSerial(y, m, d)
  gts = TimeSerial(h, n, s)
End Sub
";

    const JIT_BUILTIN_DATE_SERIAL_RANGE_ERROR: &str = "\
Public gds As Variant
Sub Main()
  Dim y As Long
  Dim m As Long
  Dim d As Long
  y = 10000
  m = 1
  d = 1
  gds = DateSerial(y, m, d)
End Sub
";

    const JIT_BUILTIN_RGB_QBCOLOR_EXPRS: &str = "\
Public grgb As Variant
Public gqb As Variant
Sub Main()
  Dim r As Long
  Dim g As Long
  Dim b As Long
  Dim q As Long
  r = 256
  g = 300
  b = 1000
  q = 12
  grgb = RGB(r, g, b)
  gqb = QBColor(q)
End Sub
";

    const JIT_BUILTIN_RGB_COMPONENT_EXPRS: &str = "\
Public gblue As Variant
Public gmid As Variant
Sub Main()
  Dim r0 As Long
  Dim g0 As Long
  Dim b1 As Long
  Dim mid As Long
  r0 = 0
  g0 = 0
  b1 = 1
  mid = 128
  gblue = RGB(r0, g0, b1)
  gmid = RGB(mid, mid, mid)
End Sub
";

    const JIT_BUILTIN_QBCOLOR_PALETTE_EXPRS: &str = "\
Public g1 As Variant
Public g7 As Variant
Public g12 As Variant
Public g15 As Variant
Sub Main()
  Dim q1 As Long
  Dim q7 As Long
  Dim q12 As Long
  Dim q15 As Long
  q1 = 1
  q7 = 7
  q12 = 12
  q15 = 15
  g1 = QBColor(q1)
  g7 = QBColor(q7)
  g12 = QBColor(q12)
  g15 = QBColor(q15)
End Sub
";

    const JIT_BUILTIN_QBCOLOR_OUT_OF_RANGE: &str = "\
Public gqb As Variant
Sub Main()
  Dim q As Long
  q = 99
  gqb = QBColor(q)
End Sub
";

    const JIT_BUILTIN_ERROR_TEXT_EXPR: &str = "\
Public ge As Variant
Sub Main()
  Dim code As Long
  code = 11
  ge = Error(code)
End Sub
";

    const JIT_BUILTIN_ERROR_TEXT_UNKNOWN_EXPR: &str = "\
Public ge As Variant
Sub Main()
  Dim code As Long
  code = 12345
  ge = Error(code)
End Sub
";

    const JIT_BUILTIN_ERROR_TEXT_INVALID_EXPR: &str = "\
Public ge As Variant
Sub Main()
  Dim code As Long
  code = -1
  ge = Error(code)
End Sub
";

    const JIT_BUILTIN_ERROR_TEXT_RESULT_DESTINATIONS: &str = "\
Public gknown As String
Public gunknown As String
Sub Main()
  Dim knownCode As Long
  Dim unknownCode As Long
  knownCode = 11
  unknownCode = 12345
  gknown = Error(knownCode)
  gunknown = Error(unknownCode)
End Sub
";

    const JIT_BUILTIN_ERROR_TEXT_RESULT_DESTINATION_INVALID: &str = "\
Public ge As String
Sub Main()
  Dim code As Long
  code = -1
  ge = Error(code)
End Sub
";

    const JIT_BUILTIN_ERROR_TEXT_ALIAS_DESTINATIONS: &str = "\
Public gknown As String
Public gunknown As String
Sub Main()
  Dim knownCode As Long
  Dim unknownCode As Long
  knownCode = 11
  unknownCode = 12345
  gknown = Error$(knownCode)
  gunknown = Error$(unknownCode)
End Sub
";

    const JIT_BUILTIN_ERROR_TEXT_ALIAS_DESTINATION_INVALID: &str = "\
Public ge As String
Sub Main()
  Dim code As Long
  code = -1
  ge = Error$(code)
End Sub
";

    const JIT_BUILTIN_LEN_VARIANT_EXPR: &str = "\
Public glen As Variant
Sub Main()
  Dim text As Variant
  text = CStr(1234&)
  glen = Len(text)
End Sub
";

    const JIT_BUILTIN_LENB_VARIANT_EXPR: &str = "\
Public glenb As Variant
Sub Main()
  Dim text As Variant
  text = CStr(1234&)
  glenb = LenB(text)
End Sub
";

    const JIT_BUILTIN_CHRW_ASCW_VARIANT_EXPRS: &str = "\
Public gc As Variant
Public ga As Variant
Sub Main()
  Dim code As Long
  code = 65
  gc = ChrW(code)
  ga = AscW(gc)
End Sub
";

    const JIT_BUILTIN_SPACE_EXPR: &str = "\
Public gs As Variant
Sub Main()
  Dim count As Long
  count = 3
  gs = Space(count)
End Sub
";

    const JIT_BUILTIN_SPACE_NEGATIVE_COUNT: &str = "\
Public gs As Variant
Sub Main()
  Dim count As Long
  count = -1
  gs = Space(count)
End Sub
";

    const JIT_BUILTIN_CASE_VARIANT_EXPRS: &str = "\
Public glc As Variant
Public guc As Variant
Sub Main()
  Dim code As Long
  Dim text As Variant
  code = 65
  text = ChrW(code)
  glc = LCase(text)
  guc = UCase(glc)
End Sub
";

    const JIT_BUILTIN_VAL_VARIANT_EXPR: &str = "\
Public gv As Variant
Sub Main()
  Dim text As Variant
  text = CStr(1234&)
  gv = Val(text)
End Sub
";

    const JIT_BUILTIN_TRIM_VARIANT_EXPRS: &str = "\
Public gt As Variant
Public glt As Variant
Public grt As Variant
Sub Main()
  Dim count As Long
  Dim text As Variant
  count = 3
  text = Space(count)
  gt = Trim(text)
  glt = LTrim(text)
  grt = RTrim(text)
End Sub
";

    const JIT_BUILTIN_STR_REVERSE_VARIANT_EXPR: &str = "\
Public gr As Variant
Sub Main()
  Dim text As Variant
  text = CStr(1234&)
  gr = StrReverse(text)
End Sub
";

    const JIT_BUILTIN_STRING_REPEAT_CHARCODE_EXPR: &str = "\
Public gs As Variant
Sub Main()
  Dim count As Long
  Dim code As Long
  count = 3
  code = 321
  gs = String(count, code)
End Sub
";

    const JIT_BUILTIN_STRING_REPEAT_CHARCODE_WRAP_EXPR: &str = "\
Public gs As Variant
Sub Main()
  Dim count As Long
  Dim code As Long
  count = 2
  code = 322
  gs = String(count, code)
End Sub
";

    const JIT_BUILTIN_STRING_REPEAT_NEGATIVE_COUNT: &str = "\
Public gs As Variant
Sub Main()
  Dim count As Long
  Dim code As Long
  count = -1
  code = 65
  gs = String(count, code)
End Sub
";

    const JIT_BUILTIN_LEFT_RIGHT_VARIANT_EXPRS: &str = "\
Public gl As Variant
Public gr As Variant
Sub Main()
  Dim text As Variant
  Dim count As Long
  text = CStr(12345&)
  count = 2
  gl = Left(text, count)
  gr = Right(text, count)
End Sub
";

    const JIT_BUILTIN_LEFT_RIGHT_VARIANT_COUNT_EDGES: &str = "\
Public glz As Variant
Public gro As Variant
Sub Main()
  Dim text As Variant
  Dim zero_count As Long
  Dim over_count As Long
  text = CStr(12345&)
  zero_count = 0
  over_count = 10
  glz = Left(text, zero_count)
  gro = Right(text, over_count)
End Sub
";

    const JIT_BUILTIN_LEFT_RIGHT_VARIANT_COMPLEMENT_COUNT_EDGES: &str = "\
Public glo As Variant
Public grz As Variant
Sub Main()
  Dim text As Variant
  Dim zero_count As Long
  Dim over_count As Long
  text = CStr(12345&)
  zero_count = 0
  over_count = 10
  glo = Left(text, over_count)
  grz = Right(text, zero_count)
End Sub
";

    const JIT_BUILTIN_LEFT_RIGHT_VARIANT_UNIT_COUNT: &str = "\
Public gl As Variant
Public gr As Variant
Sub Main()
  Dim text As Variant
  Dim count As Long
  text = CStr(12345&)
  count = 1
  gl = Left(text, count)
  gr = Right(text, count)
End Sub
";

    const JIT_BUILTIN_LEFT_RIGHT_VARIANT_EXACT_SOURCE_COUNT: &str = "\
Public gl As Variant
Public gr As Variant
Sub Main()
  Dim left_text As Variant
  Dim right_text As Variant
  Dim count As Long
  left_text = CStr(12345&)
  right_text = CStr(98765&)
  count = 5
  gl = Left(left_text, count)
  gr = Right(right_text, count)
End Sub
";

    const JIT_BUILTIN_LEFT_NEGATIVE_COUNT: &str = "\
Public g As Variant
Sub Main()
  Dim text As Variant
  Dim count As Long
  text = CStr(12345&)
  count = -1
  g = Left(text, count)
End Sub
";

    const JIT_BUILTIN_RIGHT_NEGATIVE_COUNT: &str = "\
Public g As Variant
Sub Main()
  Dim text As Variant
  Dim count As Long
  text = CStr(12345&)
  count = -1
  g = Right(text, count)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_LEFT_NEGATIVE_COUNT: &str = "\
Public g As Variant
Sub Main()
  Dim count As Long
  count = -1
  g = Left(\"12345\", count)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_RIGHT_NEGATIVE_COUNT: &str = "\
Public g As Variant
Sub Main()
  Dim count As Long
  count = -1
  g = Right(\"12345\", count)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_LEFT_NEGATIVE_COUNT: &str = "\
Public g As Variant
Sub Main()
  Dim text As String
  Dim count As Long
  text = \"12345\"
  count = -1
  g = Left(text, count)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_RIGHT_NEGATIVE_COUNT: &str = "\
Public g As Variant
Sub Main()
  Dim text As String
  Dim count As Long
  text = \"12345\"
  count = -1
  g = Right(text, count)
End Sub
";

    const JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_EXPRS: &str = "\
Public gl As Variant
Public gr As Variant
Sub Main()
  Dim text As Variant
  Dim count As Long
  text = CStr(12345&)
  count = 4
  gl = LeftB(text, count)
  gr = RightB(text, count)
End Sub
";

    const JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_UNIT_CODE_UNIT_BYTE_COUNT: &str = "\
Public gl As Variant
Public gr As Variant
Sub Main()
  Dim text As Variant
  Dim byte_count As Long
  text = CStr(12345&)
  byte_count = 2
  gl = LeftB(text, byte_count)
  gr = RightB(text, byte_count)
End Sub
";

    const JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_THREE_CODE_UNIT_BYTE_COUNT: &str = "\
Public gl As Variant
Public gr As Variant
Sub Main()
  Dim text As Variant
  Dim byte_count As Long
  text = CStr(12345&)
  byte_count = 6
  gl = LeftB(text, byte_count)
  gr = RightB(text, byte_count)
End Sub
";

    const JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_ODD_BYTE_EXPRS: &str = "\
Public gl1len As Variant
Public gl1lenb As Variant
Public gl3len As Variant
Public gl3lenb As Variant
Public gl3asc As Variant
Public gr1len As Variant
Public gr1lenb As Variant
Public gr3len As Variant
Public gr3lenb As Variant
Public gr3asc As Variant
Sub Main()
  Dim text As Variant
  Dim leftOne As Variant
  Dim leftThree As Variant
  Dim rightOne As Variant
  Dim rightThree As Variant
  text = \"ABC\"
  leftOne = LeftB(text, 1)
  leftThree = LeftB(text, 3)
  rightOne = RightB(text, 1)
  rightThree = RightB(text, 3)
  gl1len = Len(leftOne)
  gl1lenb = LenB(leftOne)
  gl3len = Len(leftThree)
  gl3lenb = LenB(leftThree)
  gl3asc = AscW(leftThree)
  gr1len = Len(rightOne)
  gr1lenb = LenB(rightOne)
  gr3len = Len(rightThree)
  gr3lenb = LenB(rightThree)
  gr3asc = AscW(rightThree)
End Sub
";

    const JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_BYTE_COUNT_EDGES: &str = "\
Public glz As Variant
Public gro As Variant
Sub Main()
  Dim text As Variant
  Dim zero_count As Long
  Dim over_count As Long
  text = CStr(12345&)
  zero_count = 0
  over_count = 20
  glz = LeftB(text, zero_count)
  gro = RightB(text, over_count)
End Sub
";

    const JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_COMPLEMENT_BYTE_COUNT_EDGES: &str = "\
Public glo As Variant
Public grz As Variant
Sub Main()
  Dim text As Variant
  Dim zero_count As Long
  Dim over_count As Long
  text = CStr(12345&)
  zero_count = 0
  over_count = 20
  glo = LeftB(text, over_count)
  grz = RightB(text, zero_count)
End Sub
";

    const JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_EXACT_BYTE_SOURCE_COUNT: &str = "\
Public gl As Variant
Public gr As Variant
Sub Main()
  Dim left_text As Variant
  Dim right_text As Variant
  Dim count As Long
  left_text = CStr(12345&)
  right_text = CStr(98765&)
  count = 10
  gl = LeftB(left_text, count)
  gr = RightB(right_text, count)
End Sub
";

    const JIT_BUILTIN_LEFTB_NEGATIVE_COUNT: &str = "\
Public g As Variant
Sub Main()
  Dim text As Variant
  Dim count As Long
  text = CStr(12345&)
  count = -1
  g = LeftB(text, count)
End Sub
";

    const JIT_BUILTIN_RIGHTB_NEGATIVE_COUNT: &str = "\
Public g As Variant
Sub Main()
  Dim text As Variant
  Dim count As Long
  text = CStr(12345&)
  count = -1
  g = RightB(text, count)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_LEFTB_NEGATIVE_COUNT: &str = "\
Public g As Variant
Sub Main()
  Dim count As Long
  count = -1
  g = LeftB(\"12345\", count)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_RIGHTB_NEGATIVE_COUNT: &str = "\
Public g As Variant
Sub Main()
  Dim count As Long
  count = -1
  g = RightB(\"12345\", count)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_LEFTB_NEGATIVE_COUNT: &str = "\
Public g As Variant
Sub Main()
  Dim text As String
  Dim count As Long
  text = \"12345\"
  count = -1
  g = LeftB(text, count)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_RIGHTB_NEGATIVE_COUNT: &str = "\
Public g As Variant
Sub Main()
  Dim text As String
  Dim count As Long
  text = \"12345\"
  count = -1
  g = RightB(text, count)
End Sub
";

    const JIT_BUILTIN_INSTR_INSTRREV_VARIANT_EXPRS: &str = "\
Public gi As Variant
Public gir As Variant
Sub Main()
  Dim text As Variant
  Dim needle As Variant
  text = CStr(123123&)
  needle = CStr(23&)
  gi = InStr(text, needle)
  gir = InStrRev(text, needle)
End Sub
";

    const JIT_BUILTIN_STRCOMP_VARIANT_EXPRS: &str = "\
Public gsc As Variant
Public gsclt As Variant
Sub Main()
  Dim first As Variant
  Dim same As Variant
  Dim later As Variant
  first = CStr(12345&)
  same = CStr(12345&)
  later = CStr(12346&)
  gsc = StrComp(first, same)
  gsclt = StrComp(first, later)
End Sub
";

    const JIT_BUILTIN_REPLACE_VARIANT_EXPR: &str = "\
Public grpl As Variant
Sub Main()
  Dim text As Variant
  Dim needle As Variant
  Dim replacement As Variant
  text = CStr(123123&)
  needle = CStr(23&)
  replacement = CStr(99&)
  grpl = Replace(text, needle, replacement)
End Sub
";

    const JIT_BUILTIN_LIKE_VARIANT_EXPR: &str = "\
Public glt As Variant
Public glf As Variant
Sub Main()
  Dim text As Variant
  Dim same As Variant
  Dim other As Variant
  text = CStr(12345&)
  same = CStr(12345&)
  other = CStr(12346&)
  glt = text Like same
  glf = text Like other
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS: &str = "\
Public glen As Variant
Public gcase As Variant
Public gval As Variant
Public gtrim As Variant
Public grev As Variant
Public gleft As Variant
Public gmid As Variant
Public ginstr As Variant
Public gcomp As Variant
Public grepl As Variant
Public glike As Variant
Sub Main()
  glen = Len(\"abcd\")
  gcase = UCase(\"ab\")
  gval = Val(\"1234\")
  gtrim = Trim(\"   \")
  grev = StrReverse(\"1234\")
  gleft = Left(\"12345\", 2)
  gmid = Mid(\"12345\", 2, 3)
  ginstr = InStr(\"123123\", \"23\")
  gcomp = StrComp(\"abc\", \"abd\")
  grepl = Replace(\"123123\", \"23\", \"99\")
  glike = \"12345\" Like \"12345\"
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_COMPANION_ARGS: &str = "\
Public glenb As Variant
Public gascw As Variant
Public gltrim As Variant
Public grtrim As Variant
Public gleftb As Variant
Public grightb As Variant
Public ginstrrev As Variant
Sub Main()
  glenb = LenB(\"abcd\")
  gascw = AscW(\"A\")
  gltrim = LTrim(\"   \")
  grtrim = RTrim(\"   \")
  gleftb = LeftB(\"12345\", 4)
  grightb = RightB(\"12345\", 4)
  ginstrrev = InStrRev(\"123123\", \"23\")
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_BYTE_COUNTS: &str = "\
Public gl1 As Variant
Public gr1 As Variant
Public gl3 As Variant
Public gr3 As Variant
Sub Main()
  gl1 = LeftB(\"12345\", 2)
  gr1 = RightB(\"12345\", 2)
  gl3 = LeftB(\"12345\", 6)
  gr3 = RightB(\"12345\", 6)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_BYTE_COUNT_EDGES: &str = "\
Public glz As Variant
Public gro As Variant
Sub Main()
  glz = LeftB(\"12345\", 0)
  gro = RightB(\"12345\", 20)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_COMPLEMENT_BYTE_COUNT_EDGES: &str = "\
Public glo As Variant
Public grz As Variant
Sub Main()
  glo = LeftB(\"12345\", 20)
  grz = RightB(\"12345\", 0)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_EXACT_BYTE_SOURCE_COUNT: &str = "\
Public gl As Variant
Public gr As Variant
Sub Main()
  gl = LeftB(\"12345\", 10)
  gr = RightB(\"98765\", 10)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_LEFT_RIGHT_COUNT_EDGES: &str = "\
Public gl1 As Variant
Public gr1 As Variant
Public glz As Variant
Public gro As Variant
Public gle As Variant
Public gre As Variant
Sub Main()
  gl1 = Left(\"12345\", 1)
  gr1 = Right(\"12345\", 1)
  glz = Left(\"12345\", 0)
  gro = Right(\"67890\", 10)
  gle = Left(\"12345\", 5)
  gre = Right(\"98765\", 5)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_MID_COUNT_EDGES: &str = "\
Public gf As Variant
Public gs As Variant
Public go As Variant
Public gz As Variant
Public ge As Variant
Public gt As Variant
Sub Main()
  gf = Mid(\"12345\", 1)
  gs = Mid(\"12345\", 5)
  go = Mid(\"12345\", 6)
  gz = Mid(\"12345\", 2, 0)
  ge = Mid(\"12345\", 1, 5)
  gt = Mid(\"12345\", 2, 4)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_MID_VALUE_EDGES: &str = "\
Public gom As Variant
Public gof As Variant
Public gel As Variant
Public gep As Variant
Sub Main()
  gom = Mid(\"12345\", 2, 10)
  gof = Mid(\"12345\", 1, 10)
  gel = Mid(\"12345\", 5, 1)
  gep = Mid(\"12345\", 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_MID_BOUNDARY_VALUE_EDGES: &str = "\
Public gos As Variant
Public gzf As Variant
Public gze As Variant
Sub Main()
  gos = Mid(\"12345\", 6, 2)
  gzf = Mid(\"12345\", 1, 0)
  gze = Mid(\"12345\", 5, 0)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_MID_START_ZERO: &str = "\
Public gm As Variant
Sub Main()
  Dim start As Long
  start = 0
  gm = Mid(\"12345\", start)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_MID_NEGATIVE_START: &str = "\
Public gm As Variant
Sub Main()
  Dim start As Long
  Dim count As Long
  start = -1
  count = 2
  gm = Mid(\"12345\", start, count)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_MID_NEGATIVE_LENGTH: &str = "\
Public gm As Variant
Sub Main()
  Dim start As Long
  Dim count As Long
  start = 2
  count = -1
  gm = Mid(\"12345\", start, count)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_OPERANDS: &str = "\
Public gcopy As String
Public glen As Variant
Public gcase As Variant
Public gval As Variant
Public gtrim As Variant
Public grev As Variant
Public gleft As Variant
Public gmid As Variant
Public ginstr As Variant
Public gcomp As Variant
Public grepl As Variant
Public glike As Variant
Sub Main()
  Dim text As String
  Dim needle As String
  Dim replacement As String
  Dim lower As String
  Dim padded As String
  Dim other As String
  text = \"123123\"
  needle = \"23\"
  replacement = \"99\"
  lower = \"ab\"
  padded = \"   \"
  other = \"123124\"
  gcopy = text
  glen = Len(text)
  gcase = UCase(lower)
  gval = Val(text)
  gtrim = Trim(padded)
  grev = StrReverse(text)
  gleft = Left(text, 3)
  gmid = Mid(text, 2, 3)
  ginstr = InStr(text, needle)
  gcomp = StrComp(text, other)
  grepl = Replace(text, needle, replacement)
  glike = text Like \"123123\"
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_COMPANION_OPERANDS: &str = "\
Public glenb As Variant
Public gascw As Variant
Public gltrim As Variant
Public grtrim As Variant
Public gleftb As Variant
Public grightb As Variant
Public ginstrrev As Variant
Sub Main()
  Dim text As String
  Dim letter As String
  Dim padded As String
  Dim sliceText As String
  Dim searchText As String
  Dim needle As String
  text = \"abcd\"
  letter = \"A\"
  padded = \"   \"
  sliceText = \"12345\"
  searchText = \"123123\"
  needle = \"23\"
  glenb = LenB(text)
  gascw = AscW(letter)
  gltrim = LTrim(padded)
  grtrim = RTrim(padded)
  gleftb = LeftB(sliceText, 4)
  grightb = RightB(sliceText, 4)
  ginstrrev = InStrRev(searchText, needle)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_BYTE_COUNTS: &str = "\
Public gl1 As Variant
Public gr1 As Variant
Public gl3 As Variant
Public gr3 As Variant
Sub Main()
  Dim text As String
  text = \"12345\"
  gl1 = LeftB(text, 2)
  gr1 = RightB(text, 2)
  gl3 = LeftB(text, 6)
  gr3 = RightB(text, 6)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_BYTE_COUNT_EDGES: &str = "\
Public glz As Variant
Public gro As Variant
Sub Main()
  Dim text As String
  text = \"12345\"
  glz = LeftB(text, 0)
  gro = RightB(text, 20)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_COMPLEMENT_BYTE_COUNT_EDGES: &str = "\
Public glo As Variant
Public grz As Variant
Sub Main()
  Dim text As String
  text = \"12345\"
  glo = LeftB(text, 20)
  grz = RightB(text, 0)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_EXACT_BYTE_SOURCE_COUNT: &str = "\
Public gl As Variant
Public gr As Variant
Sub Main()
  Dim leftText As String
  Dim rightText As String
  leftText = \"12345\"
  rightText = \"98765\"
  gl = LeftB(leftText, 10)
  gr = RightB(rightText, 10)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_LEFT_RIGHT_COUNT_EDGES: &str = "\
Public gl1 As Variant
Public gr1 As Variant
Public glz As Variant
Public gro As Variant
Public gle As Variant
Public gre As Variant
Sub Main()
  Dim text As String
  Dim overText As String
  Dim exactRight As String
  text = \"12345\"
  overText = \"67890\"
  exactRight = \"98765\"
  gl1 = Left(text, 1)
  gr1 = Right(text, 1)
  glz = Left(text, 0)
  gro = Right(overText, 10)
  gle = Left(text, 5)
  gre = Right(exactRight, 5)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_MID_COUNT_EDGES: &str = "\
Public gf As Variant
Public gs As Variant
Public go As Variant
Public gz As Variant
Public ge As Variant
Public gt As Variant
Sub Main()
  Dim text As String
  text = \"12345\"
  gf = Mid(text, 1)
  gs = Mid(text, 5)
  go = Mid(text, 6)
  gz = Mid(text, 2, 0)
  ge = Mid(text, 1, 5)
  gt = Mid(text, 2, 4)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_MID_VALUE_EDGES: &str = "\
Public gom As Variant
Public gof As Variant
Public gel As Variant
Public gep As Variant
Sub Main()
  Dim text As String
  text = \"12345\"
  gom = Mid(text, 2, 10)
  gof = Mid(text, 1, 10)
  gel = Mid(text, 5, 1)
  gep = Mid(text, 1, 2)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_MID_BOUNDARY_VALUE_EDGES: &str = "\
Public gos As Variant
Public gzf As Variant
Public gze As Variant
Sub Main()
  Dim text As String
  text = \"12345\"
  gos = Mid(text, 6, 2)
  gzf = Mid(text, 1, 0)
  gze = Mid(text, 5, 0)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_MID_START_ZERO: &str = "\
Public gm As Variant
Sub Main()
  Dim text As String
  Dim start As Long
  text = \"12345\"
  start = 0
  gm = Mid(text, start)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_MID_NEGATIVE_START: &str = "\
Public gm As Variant
Sub Main()
  Dim text As String
  Dim start As Long
  Dim count As Long
  text = \"12345\"
  start = -1
  count = 2
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_MID_NEGATIVE_LENGTH: &str = "\
Public gm As Variant
Sub Main()
  Dim text As String
  Dim start As Long
  Dim count As Long
  text = \"12345\"
  start = 2
  count = -1
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_STRING_NULL_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = Null
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_EMPTY_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = Empty
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_NUMERIC_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = 12345&
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_BOOLEAN_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = True
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_DOUBLE_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = 12345#
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_SINGLE_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = 12345!
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_INTEGER_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = 12345%
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_LONGLONG_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = 12345^
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_BYTE_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = CByte(123)
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_CURRENCY_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = 12345@
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_DATE_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = #2020-01-15#
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_ERROR_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = CVErr(1234)
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_STRING_DECIMAL_SLICE_ARGS: &str = "\
Public gl As Variant
Public gr As Variant
Public glb As Variant
Public grb As Variant
Public gm As Variant
Public gmc As Variant
Sub Main()
  Dim n As Variant
  n = CDec(12345)
  gl = Left(n, 2)
  gr = Right(n, 2)
  glb = LeftB(n, 2)
  grb = RightB(n, 2)
  gm = Mid(n, 1)
  gmc = Mid(n, 1, 2)
End Sub
";

    const JIT_BUILTIN_LEFTB_RIGHTB_ODD_BYTE_EXPRS: &str = "\
Public gl1len As Variant
Public gl1lenb As Variant
Public gl3len As Variant
Public gl3lenb As Variant
Public gl3asc As Variant
Public gr1len As Variant
Public gr1lenb As Variant
Public gr3len As Variant
Public gr3lenb As Variant
Public gr3asc As Variant
Sub Main()
  Dim text As String
  Dim leftOne As Variant
  Dim leftThree As Variant
  Dim rightOne As Variant
  Dim rightThree As Variant
  text = \"ABC\"
  leftOne = LeftB(text, 1)
  leftThree = LeftB(text, 3)
  rightOne = RightB(text, 1)
  rightThree = RightB(text, 3)
  gl1len = Len(leftOne)
  gl1lenb = LenB(leftOne)
  gl3len = Len(leftThree)
  gl3lenb = LenB(leftThree)
  gl3asc = AscW(leftThree)
  gr1len = Len(rightOne)
  gr1lenb = LenB(rightOne)
  gr3len = Len(rightThree)
  gr3lenb = LenB(rightThree)
  gr3asc = AscW(rightThree)
End Sub
";

    const JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_ODD_BYTE_EXPRS: &str = "\
Public gl1len As Variant
Public gl1lenb As Variant
Public gl3len As Variant
Public gl3lenb As Variant
Public gl3asc As Variant
Public gr1len As Variant
Public gr1lenb As Variant
Public gr3len As Variant
Public gr3lenb As Variant
Public gr3asc As Variant
Sub Main()
  Dim leftOne As Variant
  Dim leftThree As Variant
  Dim rightOne As Variant
  Dim rightThree As Variant
  leftOne = LeftB(\"ABC\", 1)
  leftThree = LeftB(\"ABC\", 3)
  rightOne = RightB(\"ABC\", 1)
  rightThree = RightB(\"ABC\", 3)
  gl1len = Len(leftOne)
  gl1lenb = LenB(leftOne)
  gl3len = Len(leftThree)
  gl3lenb = LenB(leftThree)
  gl3asc = AscW(leftThree)
  gr1len = Len(rightOne)
  gr1lenb = LenB(rightOne)
  gr3len = Len(rightThree)
  gr3lenb = LenB(rightThree)
  gr3asc = AscW(rightThree)
End Sub
";

    const JIT_BUILTIN_STRING_OPTIONAL_ARGS: &str = "\
Public ginstrStart As Variant
Public ginstrText As Variant
Public ginstrrevStart As Variant
Public ginstrrevText As Variant
Public ginstrrevOmitted As Variant
Public gcompText As Variant
Public greplStart As Variant
Public greplText As Variant
Public greplOmitted As Variant
Sub Main()
  ginstrStart = InStr(3, \"abcabc\", \"a\")
  ginstrText = InStr(1, \"ABC\", \"b\", 1)
  ginstrrevStart = InStrRev(\"abcabca\", \"a\", 4)
  ginstrrevText = InStrRev(\"aBcaBc\", \"b\", -1, 1)
  ginstrrevOmitted = InStrRev(\"abBAbA\", \"a\", , 1)
  gcompText = StrComp(\"a\", \"A\", 1)
  greplStart = Replace(\"abcabc\", \"a\", \"x\", 2, -1, 0)
  greplText = Replace(\"aAbB\", \"a\", \"x\", 1, -1, 1)
  greplOmitted = Replace(\"zAaA\", \"a\", \"x\", , , 1)
End Sub
";

    const JIT_BUILTIN_STATIC_STRING_OPTIONAL_ARGS: &str = "\
Public ginstrStart As Variant
Public ginstrText As Variant
Public ginstrrevStart As Variant
Public ginstrrevText As Variant
Public ginstrrevOmitted As Variant
Public gcompText As Variant
Public greplStart As Variant
Public greplText As Variant
Public greplOmitted As Variant
Sub Main()
  Dim abc As String
  Dim upper As String
  Dim mixed As String
  Dim abba As String
  Dim replText As String
  Dim ztext As String
  Dim a As String
  Dim b As String
  Dim capA As String
  Dim x As String
  abc = \"abcabc\"
  upper = \"ABC\"
  mixed = \"aBcaBc\"
  abba = \"abBAbA\"
  replText = \"aAbB\"
  ztext = \"zAaA\"
  a = \"a\"
  b = \"b\"
  capA = \"A\"
  x = \"x\"
  ginstrStart = InStr(3, abc, a)
  ginstrText = InStr(1, upper, b, 1)
  ginstrrevStart = InStrRev(abc, a, 4)
  ginstrrevText = InStrRev(mixed, b, -1, 1)
  ginstrrevOmitted = InStrRev(abba, a, , 1)
  gcompText = StrComp(a, capA, 1)
  greplStart = Replace(abc, a, x, 2, -1, 0)
  greplText = Replace(replText, a, x, 1, -1, 1)
  greplOmitted = Replace(ztext, a, x, , , 1)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_EXPR: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 2
  count = 3
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_OMITTED_LENGTH_EXPR: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  text = CStr(12345&)
  start = 3
  gm = Mid(text, start)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_OMITTED_LENGTH_FULL_SOURCE: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  text = CStr(12345&)
  start = 1
  gm = Mid(text, start)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_OMITTED_LENGTH_SUFFIX: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  text = CStr(12345&)
  start = 5
  gm = Mid(text, start)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_OMITTED_LENGTH_OVERLONG_START: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  text = CStr(12345&)
  start = 6
  gm = Mid(text, start)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_START_ZERO: &str = "\
Public gmz As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  text = CStr(12345&)
  start = 0
  gmz = Mid(text, start)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_ZERO_LENGTH: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 1
  count = 0
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_ZERO_LENGTH_MIDDLE: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 2
  count = 0
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_ZERO_LENGTH_AT_END: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 5
  count = 0
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_EXACT_LAST_CHAR: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 5
  count = 1
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_EXACT_FULL_SOURCE_COUNT: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 1
  count = 5
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_EXACT_SUFFIX_COUNT: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 2
  count = 4
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_EXACT_PREFIX_COUNT: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 1
  count = 2
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_NEGATIVE_LENGTH: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 2
  count = -1
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_NEGATIVE_START: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = -1
  count = 2
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_OVERLONG_START: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 6
  count = 2
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_OVERLONG_COUNT: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 5
  count = 10
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_OVERLONG_COUNT_MIDDLE: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 2
  count = 10
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_MID_VARIANT_OVERLONG_COUNT_FULL_SOURCE: &str = "\
Public gm As Variant
Sub Main()
  Dim text As Variant
  Dim start As Long
  Dim count As Long
  text = CStr(12345&)
  start = 1
  count = 10
  gm = Mid(text, start, count)
End Sub
";

    const JIT_BUILTIN_WEEKDAY_EXPR: &str = "\
Public gw As Variant
Sub Main()
  Dim d As Date
  d = #2020-01-16#
  gw = Weekday(d)
End Sub
";

    const JIT_BUILTIN_WEEKDAY_FIRSTDAY_EXPR: &str = "\
Public gw As Variant
Sub Main()
  Dim d As Date
  Dim firstDay As Long
  d = #2024-01-08#
  firstDay = 2
  gw = Weekday(d, firstDay)
End Sub
";

    const JIT_BUILTIN_DATE_NAME_EXPRS: &str = "\
Public gm As Variant
Public gw As Variant
Sub Main()
  Dim m As Long
  Dim w As Long
  m = 1
  w = 5
  gm = MonthName(m)
  gw = WeekdayName(w)
End Sub
";

    const JIT_BUILTIN_DATE_NAME_OPTIONAL_ARGS: &str = "\
Public gm As Variant
Public gw As Variant
Sub Main()
  Dim m As Long
  Dim w As Long
  Dim abbreviate As Boolean
  Dim firstDay As Long
  m = 1
  w = 1
  abbreviate = True
  firstDay = 2
  gm = MonthName(m, abbreviate)
  gw = WeekdayName(w, abbreviate, firstDay)
End Sub
";

    const JIT_BUILTIN_CONVERSION_VARIANT_OPERANDS: &str = "\
Public gb As Boolean
Public gy As Byte
Public gi As Integer
Public gl As Long
Public gll As LongLong
Public gp As LongPtr
Public gs As Single
Public gd As Double
Public gc As Currency
Public gt As Date
Public gdec As Variant
Public gv As Variant
Sub Main()
  Dim v As Variant
  v = 13.5
  gb = CBool(v)
  gy = CByte(v)
  gi = CInt(v)
  gl = CLng(v)
  v = 5000000013.5#
  gll = CLngLng(v)
  gp = CLngPtr(v)
  v = 1.25#
  gs = CSng(v)
  v = 12
  gd = CDbl(v)
  v = 12.3456#
  gc = CCur(v)
  v = 36527#
  gt = CDate(v)
  v = 10
  gdec = CDec(v)
  v = 42&
  gv = CVar(v)
End Sub
";

    const JIT_FOR_LOOP: &str = "\
Public g As Long
Sub Main()
  Dim i As Long
  For i = 1 To 3
    g = g + i
  Next i
End Sub
";

    const JIT_STATIC_SUB_CALL: &str = "\
Public g As Long
Sub Main()
  Call Worker(7)
End Sub
Sub Worker(ByVal x As Long)
  g = x * 2
End Sub
";
    const JIT_GOSUB_BASIC: &str = include_str!("../../../conformance/tests/gosub_basic.bas");
    const JIT_GOSUB_REPEATED: &str = include_str!("../../../conformance/tests/gosub_repeated.bas");
    const JIT_GOSUB_NESTED_LABELS: &str =
        include_str!("../../../conformance/tests/gosub_nested_labels.bas");
    const JIT_GOSUB_LOOP_ACCUMULATE: &str =
        include_str!("../../../conformance/tests/gosub_loop_accumulate.bas");
    const JIT_CONSOLIDATE_FOR_GOSUB_MIX: &str =
        include_str!("../../../conformance/tests/consolidate_for_gosub_mix.bas");
    const JIT_CONSOLIDATE_GOSUB_ERROR_MIX: &str =
        include_str!("../../../conformance/tests/consolidate_gosub_error_mix.bas");
    const JIT_GOSUB_RETURN_WITHOUT_GOSUB: &str = "\
Sub Main()
  Return
End Sub
";
    const JIT_GOSUB_RETURN_WITHOUT_GOSUB_RESUME_NEXT: &str = "\
Sub Main()
  Dim observed
  On Error Resume Next
  Return
  observed = Err.Number
End Sub
";
    const JIT_GOSUB_RETURN_WITHOUT_GOSUB_LABEL_HANDLER: &str = "\
Sub Main()
  Dim observed
  On Error GoTo Handler
  Return
Done:
  Exit Sub
Handler:
  observed = Err.Number
  Resume Done
End Sub
";
    const JIT_CONSOLIDATE_NESTED_CALL_CHAIN: &str =
        include_str!("../../../conformance/tests/consolidate_nested_call_chain.bas");
    const JIT_CONSOLIDATE_FOR_SELECT_CALL: &str =
        include_str!("../../../conformance/tests/consolidate_for_select_call.bas");
    const JIT_CONSOLIDATE_WHILE_BYREF_MIX: &str =
        include_str!("../../../conformance/tests/consolidate_while_byref_mix.bas");
    const JIT_ERROR_RESUME_FUNCTION_PROPAGATION: &str =
        include_str!("../../../conformance/tests/error_resume_function_propagation.bas");
    const JIT_ERROR_NESTED_MODE_TRANSITIONS: &str =
        include_str!("../../../conformance/tests/error_nested_mode_transitions.bas");

    const JIT_PARAMARRAY_UBOUND_PACK: &str =
        include_str!("../../../conformance/tests/params_paramarray_pack.bas");
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_PACK: &str =
        include_str!("../../../conformance/tests/params_paramarray_named_fixed_tail_pack.bas");
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_ARRAY_ELEMENT_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_array_element_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_ARRAY_ELEMENT_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_array_element_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_ARRAY_ELEMENT_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_array_element_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_DUPLICATE_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_duplicate_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_DUPLICATE_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_duplicate_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_DUPLICATE_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_duplicate_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_STRING_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_string_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_STRING_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_string_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_STRING_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_string_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_LONGPTR_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_longptr_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_LONGPTR_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_longptr_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_LONGPTR_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_longptr_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_typed_scalar_alias_bundle_copyout.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_typed_scalar_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_typed_scalar_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_FIXED_STRING_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_fixed_string_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_FIXED_STRING_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_fixed_string_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_FIXED_STRING_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_global_fixed_string_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_typed_long_string_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_typed_long_string_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_typed_long_string_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONGPTR_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_typed_longptr_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONGPTR_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_typed_longptr_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONGPTR_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_typed_longptr_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_typed_scalar_alias_bundle_copyout.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_typed_scalar_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_typed_scalar_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_FIXED_STRING_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_typed_fixed_string_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_FIXED_STRING_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_typed_fixed_string_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_FIXED_STRING_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_named_fixed_tail_typed_fixed_string_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_UBOUND_EMPTY: &str =
        include_str!("../../../conformance/tests/params_paramarray_empty.bas");
    const JIT_PARAMARRAY_OMITTED_TAIL_EMPTY: &str =
        include_str!("../../../conformance/tests/params_paramarray_omitted_tail_empty.bas");
    const JIT_PARAMARRAY_ALIAS_COPYOUT: &str =
        include_str!("../../../conformance/tests/params_paramarray_alias_copyout.bas");
    const JIT_PARAMARRAY_GLOBAL_ALIAS_COPYOUT: &str =
        include_str!("../../../conformance/tests/params_paramarray_global_alias_copyout.bas");
    const JIT_PARAMARRAY_GLOBAL_BYVAL_NO_ALIAS: &str =
        include_str!("../../../conformance/tests/params_paramarray_global_byval_no_alias.bas");
    const JIT_PARAMARRAY_GLOBAL_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_global_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_GLOBAL_STRING_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_global_string_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_GLOBAL_STRING_BYVAL_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_global_string_byval_no_alias.bas"
    );
    const JIT_PARAMARRAY_GLOBAL_STRING_PARENTHESIZED_NO_ALIAS: &str = include_str!(
        "../../../conformance/tests/params_paramarray_global_string_parenthesized_no_alias.bas"
    );
    const JIT_PARAMARRAY_ARRAY_ELEMENT_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_array_element_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_DUPLICATE_ALIAS_COPYOUT: &str =
        include_str!("../../../conformance/tests/params_paramarray_duplicate_alias_copyout.bas");
    const JIT_PARAMARRAY_PARENTHESIZED_NO_ALIAS: &str =
        include_str!("../../../conformance/tests/params_paramarray_parenthesized_no_alias.bas");
    const JIT_PARAMARRAY_BYVAL_NO_ALIAS: &str =
        include_str!("../../../conformance/tests/params_paramarray_byval_no_alias.bas");
    const JIT_PARAMARRAY_VARIANT_ARRAY_ELEMENT_MUTATION: &str = include_str!(
        "../../../conformance/tests/params_paramarray_variant_array_element_mutation.bas"
    );
    const JIT_PARAMARRAY_TYPED_SCALAR_ALIAS_COPYOUT: &str =
        include_str!("../../../conformance/tests/params_paramarray_typed_scalar_alias_copyout.bas");
    const JIT_PARAMARRAY_TYPED_LONGLONG_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_typed_longlong_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_TYPED_LONGPTR_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_typed_longptr_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_TYPED_INTEGER_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_typed_integer_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_TYPED_BYTE_ALIAS_COPYOUT: &str =
        include_str!("../../../conformance/tests/params_paramarray_typed_byte_alias_copyout.bas");
    const JIT_PARAMARRAY_TYPED_BOOLEAN_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_typed_boolean_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_TYPED_STRING_ALIAS_COPYOUT: &str =
        include_str!("../../../conformance/tests/params_paramarray_typed_string_alias_copyout.bas");
    const JIT_PARAMARRAY_TYPED_FIXED_STRING_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_typed_fixed_string_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_TYPED_CURRENCY_ALIAS_COPYOUT: &str = include_str!(
        "../../../conformance/tests/params_paramarray_typed_currency_alias_copyout.bas"
    );
    const JIT_PARAMARRAY_TYPED_SINGLE_ALIAS_COPYOUT: &str =
        include_str!("../../../conformance/tests/params_paramarray_typed_single_alias_copyout.bas");
    const JIT_PARAMARRAY_TYPED_DOUBLE_ALIAS_COPYOUT: &str =
        include_str!("../../../conformance/tests/params_paramarray_typed_double_alias_copyout.bas");
    const JIT_PARAMARRAY_TYPED_DATE_ALIAS_COPYOUT: &str =
        include_str!("../../../conformance/tests/params_paramarray_typed_date_alias_copyout.bas");
    const JIT_PARAMARRAY_BOUNDS_EXPLICIT_DIM: &str =
        include_str!("../../../conformance/tests/params_paramarray_bounds_explicit_dim.bas");
    const JIT_PARAMARRAY_OPTION_BASE_ONE_BOUNDS: &str =
        include_str!("../../../conformance/tests/params_paramarray_option_base_one_bounds.bas");
    const JIT_PARAMARRAY_LBOUND_DIM_ZERO_ERROR: &str =
        include_str!("../../../conformance/tests/params_paramarray_lbound_dim_zero_error.bas");
    const JIT_PARAMARRAY_UBOUND_DIM_TOO_HIGH_ERROR: &str =
        include_str!("../../../conformance/tests/params_paramarray_ubound_dim_too_high_error.bas");
    const JIT_ARRAY_LITERAL_BOUNDS: &str =
        include_str!("../../../conformance/tests/stdlib_array_introspection_bounds.bas");
    const JIT_ARRAY_LITERAL_BOUNDS_EXPLICIT_DIM: &str = "\
Sub Main()
Dim l
Dim u
Probe l, u
End Sub

Private Sub Probe(ByRef l, ByRef u)
Dim a
a = Array(10, 20, 30)
l = LBound(a, 1)
u = UBound(a, 1)
End Sub
";
    const JIT_ARRAY_LITERAL_BOUNDS_DIM_ZERO_ERROR: &str = "\
Sub Main()
Dim x
Dim a
a = Array(10, 20, 30)
x = LBound(a, 0)
End Sub
";
    const JIT_ARRAY_LITERAL_BOUNDS_DIM_TOO_HIGH_ERROR: &str = "\
Sub Main()
Dim x
Dim a
a = Array(10, 20, 30)
x = UBound(a, 2)
End Sub
";
    const JIT_ARRAY_DYNAMIC_LBOUND_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/dynamic_lbound_unallocated_error.bas");
    const JIT_ARRAY_DYNAMIC_UBOUND_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/dynamic_ubound_unallocated_error.bas");
    const JIT_ARRAY_DYNAMIC_BOUNDS_EXPLICIT_DIM: &str =
        include_str!("../../../conformance/tests/dynamic_bounds_explicit_dim.bas");
    const JIT_ARRAY_DYNAMIC_LBOUND_DIM_ZERO_ERROR: &str =
        include_str!("../../../conformance/tests/dynamic_lbound_dim_zero_error.bas");
    const JIT_ARRAY_DYNAMIC_UBOUND_DIM_TOO_HIGH_ERROR: &str =
        include_str!("../../../conformance/tests/dynamic_ubound_dim_too_high_error.bas");
    const JIT_ARRAY_DYNAMIC_TYPES: &str = "\
Sub Main()
Dim vtBefore
Dim tnBefore
Dim vtAfter
Dim tnAfter
Dim vtErase
Dim tnErase
Dim a()
vtBefore = VarType(a)
tnBefore = TypeName(a)
ReDim a(1)
vtAfter = VarType(a)
tnAfter = TypeName(a)
Erase a
vtErase = VarType(a)
tnErase = TypeName(a)
End Sub
";
    const JIT_ARRAY_FIXED_INFORMATION: &str = "\
Sub Main()
Dim vt
Dim tn
Dim score As Long
Dim a(1)
vt = VarType(a)
tn = TypeName(a)
If IsArray(a) Then score = score + 1
End Sub
";
    const JIT_ARRAY_FIXED_INFORMATION_AFTER_ERASE: &str = "\
Sub Main()
Dim vt
Dim tn
Dim score As Long
Dim a(1)
Erase a
vt = VarType(a)
tn = TypeName(a)
If IsArray(a) Then score = score + 3
End Sub
";
    const JIT_ARRAY_FIXED_BOUNDS_EXPLICIT_DIM: &str = "\
Sub Main()
Dim l
Dim u
Dim a(2 To 4)
l = LBound(a, 1)
u = UBound(a, 1)
End Sub
";
    const JIT_ARRAY_FIXED_LBOUND_DIM_ZERO_ERROR: &str = "\
Sub Main()
Dim x
Dim a(1)
x = LBound(a, 0)
End Sub
";
    const JIT_ARRAY_FIXED_UBOUND_DIM_TOO_HIGH_ERROR: &str = "\
Sub Main()
Dim x
Dim a(1)
x = UBound(a, 2)
End Sub
";
    const JIT_ARRAY_MULTIDIM_INDEXING: &str =
        include_str!("../../../conformance/tests/array_multidim_indexing.bas");
    const JIT_ARRAY_LITERAL_TYPES: &str =
        include_str!("../../../conformance/tests/stdlib_array_introspection_types.bas");
    const JIT_ARRAY_ZERO_INDEX: &str =
        include_str!("../../../conformance/tests/array_zero_index.bas");
    const JIT_ARRAY_STORE_LOAD: &str =
        include_str!("../../../conformance/tests/array_store_load.bas");
    const JIT_ARRAY_LONG_TYPED_STORE_LOAD: &str =
        include_str!("../../../conformance/tests/array_long_typed_store_load.bas");
    const JIT_ARRAY_LONG_READ_TO_VARIANT: &str =
        include_str!("../../../conformance/tests/array_long_read_to_variant.bas");
    const JIT_ARRAY_STRING_TYPED_STORE_LOAD: &str =
        include_str!("../../../conformance/tests/array_string_typed_store_load.bas");
    const JIT_ARRAY_FIXED_STRING_TYPED_STORE_LOAD: &str =
        include_str!("../../../conformance/tests/array_fixed_string_typed_store_load.bas");
    const JIT_ARRAY_FIXED_STRING_DYNAMIC_TYPED_STORE_LOAD: &str =
        include_str!("../../../conformance/tests/array_fixed_string_dynamic_typed_store_load.bas");
    const JIT_ARRAY_FIXED_STRING_MULTIDIM_TYPED_STORE_LOAD: &str =
        include_str!("../../../conformance/tests/array_fixed_string_multidim_typed_store_load.bas");
    const JIT_ARRAY_FIXED_STRING_DYNAMIC_MULTIDIM_TYPED_STORE_LOAD: &str = include_str!(
        "../../../conformance/tests/array_fixed_string_dynamic_multidim_typed_store_load.bas"
    );
    const JIT_ARRAY_FIXED_STRING_3D_TYPED_STORE_LOAD: &str =
        include_str!("../../../conformance/tests/array_fixed_string_3d_typed_store_load.bas");
    const JIT_ARRAY_FIXED_STRING_DYNAMIC_3D_TYPED_STORE_LOAD: &str = include_str!(
        "../../../conformance/tests/array_fixed_string_dynamic_3d_typed_store_load.bas"
    );
    const JIT_ARRAY_FIXED_STRING_4D_TYPED_STORE_LOAD: &str =
        include_str!("../../../conformance/tests/array_fixed_string_4d_typed_store_load.bas");
    const JIT_ARRAY_FIXED_STRING_DYNAMIC_4D_TYPED_STORE_LOAD: &str = include_str!(
        "../../../conformance/tests/array_fixed_string_dynamic_4d_typed_store_load.bas"
    );
    const JIT_ARRAY_TYPED_SCALAR_STORE_LOAD_BUNDLE: &str =
        include_str!("../../../conformance/tests/array_typed_scalar_store_load_bundle.bas");
    const JIT_ARRAY_TYPED_SCALAR_DYNAMIC_STORE_LOAD_BUNDLE: &str =
        include_str!("../../../conformance/tests/array_typed_scalar_dynamic_store_load_bundle.bas");
    const JIT_ARRAY_TYPED_SCALAR_MULTIDIM_STORE_LOAD_BUNDLE: &str = include_str!(
        "../../../conformance/tests/array_typed_scalar_multidim_store_load_bundle.bas"
    );
    const JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_STORE_LOAD_BUNDLE: &str = include_str!(
        "../../../conformance/tests/array_typed_scalar_dynamic_multidim_store_load_bundle.bas"
    );
    const JIT_ARRAY_TYPED_SCALAR_3D_STORE_LOAD_BUNDLE: &str =
        include_str!("../../../conformance/tests/array_typed_scalar_3d_store_load_bundle.bas");
    const JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_STORE_LOAD_BUNDLE: &str = include_str!(
        "../../../conformance/tests/array_typed_scalar_dynamic_3d_store_load_bundle.bas"
    );
    const JIT_ARRAY_TYPED_SCALAR_4D_STORE_LOAD_BUNDLE: &str =
        include_str!("../../../conformance/tests/array_typed_scalar_4d_store_load_bundle.bas");
    const JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_STORE_LOAD_BUNDLE: &str = include_str!(
        "../../../conformance/tests/array_typed_scalar_dynamic_4d_store_load_bundle.bas"
    );
    const JIT_ARRAY_TYPED_SCALAR_MULTIDIM_BOUNDS_BUNDLE: &str =
        include_str!("../../../conformance/tests/array_typed_scalar_multidim_bounds_bundle.bas");
    const JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_BOUNDS_BUNDLE: &str = include_str!(
        "../../../conformance/tests/array_typed_scalar_dynamic_multidim_bounds_bundle.bas"
    );
    const JIT_ARRAY_TYPED_SCALAR_MULTIDIM_BOUNDS_DIM_EXPR_BUNDLE: &str = include_str!(
        "../../../conformance/tests/array_typed_scalar_multidim_bounds_dim_expr_bundle.bas"
    );
    const JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_BOUNDS_DIM_EXPR_BUNDLE: &str = include_str!(
        "../../../conformance/tests/array_typed_scalar_dynamic_multidim_bounds_dim_expr_bundle.bas"
    );
    const JIT_ARRAY_TYPED_SCALAR_3D_BOUNDS_BUNDLE: &str =
        include_str!("../../../conformance/tests/array_typed_scalar_3d_bounds_bundle.bas");
    const JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_BOUNDS_BUNDLE: &str =
        include_str!("../../../conformance/tests/array_typed_scalar_dynamic_3d_bounds_bundle.bas");
    const JIT_ARRAY_TYPED_SCALAR_4D_BOUNDS_BUNDLE: &str =
        include_str!("../../../conformance/tests/array_typed_scalar_4d_bounds_bundle.bas");
    const JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_BOUNDS_BUNDLE: &str =
        include_str!("../../../conformance/tests/array_typed_scalar_dynamic_4d_bounds_bundle.bas");
    const JIT_ARRAY_TYPED_LONG_3D_LBOUND_DIM_ZERO_ERROR: &str =
        include_str!("../../../conformance/tests/array_typed_long_3d_lbound_dim_zero_error.bas");
    const JIT_ARRAY_TYPED_LONG_3D_UBOUND_DIM_TOO_HIGH_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_3d_ubound_dim_too_high_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_DYNAMIC_3D_LBOUND_DIM_ZERO_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_dynamic_3d_lbound_dim_zero_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_DYNAMIC_3D_UBOUND_DIM_TOO_HIGH_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_dynamic_3d_ubound_dim_too_high_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_3D_LBOUND_DIM_EXPR_ZERO_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_3d_lbound_dim_expr_zero_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_3D_UBOUND_DIM_EXPR_TOO_HIGH_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_3d_ubound_dim_expr_too_high_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_DYNAMIC_3D_LBOUND_DIM_EXPR_ZERO_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_dynamic_3d_lbound_dim_expr_zero_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_DYNAMIC_3D_UBOUND_DIM_EXPR_TOO_HIGH_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_dynamic_3d_ubound_dim_expr_too_high_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_4D_LBOUND_DIM_ZERO_ERROR: &str =
        include_str!("../../../conformance/tests/array_typed_long_4d_lbound_dim_zero_error.bas");
    const JIT_ARRAY_TYPED_LONG_4D_UBOUND_DIM_TOO_HIGH_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_4d_ubound_dim_too_high_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_DYNAMIC_4D_LBOUND_DIM_ZERO_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_dynamic_4d_lbound_dim_zero_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_DYNAMIC_4D_UBOUND_DIM_TOO_HIGH_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_dynamic_4d_ubound_dim_too_high_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_4D_LBOUND_DIM_EXPR_ZERO_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_4d_lbound_dim_expr_zero_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_4D_UBOUND_DIM_EXPR_TOO_HIGH_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_4d_ubound_dim_expr_too_high_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_DYNAMIC_4D_LBOUND_DIM_EXPR_ZERO_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_dynamic_4d_lbound_dim_expr_zero_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_DYNAMIC_4D_UBOUND_DIM_EXPR_TOO_HIGH_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_dynamic_4d_ubound_dim_expr_too_high_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_MULTIDIM_LBOUND_DIM_ZERO_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_multidim_lbound_dim_zero_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_MULTIDIM_UBOUND_DIM_TOO_HIGH_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_multidim_ubound_dim_too_high_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_DYNAMIC_MULTIDIM_LBOUND_DIM_ZERO_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_dynamic_multidim_lbound_dim_zero_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_DYNAMIC_MULTIDIM_UBOUND_DIM_TOO_HIGH_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_dynamic_multidim_ubound_dim_too_high_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_MULTIDIM_LBOUND_DIM_EXPR_ZERO_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_multidim_lbound_dim_expr_zero_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_MULTIDIM_UBOUND_DIM_EXPR_TOO_HIGH_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_multidim_ubound_dim_expr_too_high_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_DYNAMIC_MULTIDIM_LBOUND_DIM_EXPR_ZERO_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_dynamic_multidim_lbound_dim_expr_zero_error.bas"
    );
    const JIT_ARRAY_TYPED_LONG_DYNAMIC_MULTIDIM_UBOUND_DIM_EXPR_TOO_HIGH_ERROR: &str = include_str!(
        "../../../conformance/tests/array_typed_long_dynamic_multidim_ubound_dim_expr_too_high_error.bas"
    );
    const JIT_ARRAY_EXPLICIT_LOWER_BOUND: &str =
        include_str!("../../../conformance/tests/array_explicit_lower_bound.bas");
    const JIT_ARRAY_OPTION_BASE_ONE_BOUNDS: &str =
        include_str!("../../../conformance/tests/array_option_base_one_bounds.bas");
    const JIT_ARRAY_BOUNDS_ERROR: &str =
        include_str!("../../../conformance/tests/array_bounds_error.bas");
    const JIT_ARRAY_REDIM_EXPAND: &str =
        include_str!("../../../conformance/tests/redim_expand_allows_new_index.bas");
    const JIT_ARRAY_REDIM_WITHOUT_PRESERVE_RESETS: &str =
        include_str!("../../../conformance/tests/redim_without_preserve_resets.bas");
    const JIT_ARRAY_REDIM_SHRINK_BOUNDS_ERROR: &str =
        include_str!("../../../conformance/tests/redim_shrink_bounds_error.bas");
    const JIT_ARRAY_REDIM_UPPER_LESS_THAN_LOWER_ERROR: &str =
        include_str!("../../../conformance/tests/redim_upper_less_than_lower_error.bas");
    const JIT_ARRAY_REDIM_PRESERVE_UPPER_LESS_THAN_LOWER_ERROR: &str =
        include_str!("../../../conformance/tests/redim_preserve_upper_less_than_lower_error.bas");
    const JIT_ARRAY_REDIM_NEGATIVE_LOWER_BOUND: &str =
        include_str!("../../../conformance/tests/redim_negative_lower_bound.bas");
    const JIT_ARRAY_REDIM_DYNAMIC_BOUND_EXPRESSION: &str =
        include_str!("../../../conformance/tests/redim_dynamic_bound_expression.bas");
    const JIT_ARRAY_REDIM_OPTION_BASE_ONE_BOUNDS: &str =
        include_str!("../../../conformance/tests/redim_option_base_one_bounds.bas");
    const JIT_ARRAY_REDIM_FIXED_VARIANT_ARRAY_ERROR: &str =
        include_str!("../../../conformance/tests/redim_fixed_variant_array_error.bas");
    const JIT_ARRAY_REDIM_PRESERVE_KEEPS_VALUES: &str =
        include_str!("../../../conformance/tests/redim_preserve_keeps_values.bas");
    const JIT_ARRAY_REDIM_PRESERVE_UNALLOCATED_DEFAULTS: &str =
        include_str!("../../../conformance/tests/redim_preserve_unallocated_defaults.bas");
    const JIT_ARRAY_REDIM_PRESERVE_EXPLICIT_LOWER_KEEPS_VALUE: &str =
        include_str!("../../../conformance/tests/redim_preserve_explicit_lower_keeps_value.bas");
    const JIT_ARRAY_REDIM_PRESERVE_SHRINK_EXPAND_CLEARS_TAIL: &str =
        include_str!("../../../conformance/tests/redim_preserve_shrink_expand_clears_tail.bas");
    const JIT_ARRAY_REDIM_PRESERVE_LOWER_BOUND_CHANGE_ERROR: &str =
        include_str!("../../../conformance/tests/redim_preserve_lower_bound_change_error.bas");
    const JIT_ARRAY_REDIM_PRESERVE_FIXED_VARIANT_ARRAY_ERROR: &str =
        include_str!("../../../conformance/tests/redim_preserve_fixed_variant_array_error.bas");
    const JIT_ARRAY_REDIM_PRESERVE_MULTIDIM_LAST_DIMENSION: &str =
        include_str!("../../../conformance/tests/redim_preserve_multidim_last_dimension.bas");
    const JIT_ARRAY_REDIM_PRESERVE_ILLEGAL_NON_LAST_DIM_ERROR: &str =
        include_str!("../../../conformance/tests/redim_preserve_illegal_non_last_dim_error.bas");
    const JIT_ARRAY_ERASE_FIXED_RESET: &str =
        include_str!("../../../conformance/tests/erase_array_basic.bas");
    const JIT_FOR_EACH_ARRAY_DYNAMIC_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_array_dynamic_basic.bas");
    const JIT_FOR_EACH_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND: &str =
        include_str!("../../../conformance/tests/for_each_array_dynamic_explicit_lower_bound.bas");
    const JIT_FOR_EACH_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION: &str =
        include_str!("../../../conformance/tests/for_each_array_dynamic_item_after_completion.bas");
    const JIT_FOR_EACH_ARRAY_LITERAL_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_array_literal_basic.bas");
    const JIT_FOR_EACH_ARRAY_LITERAL_EMPTY_SKIPS: &str =
        include_str!("../../../conformance/tests/for_each_array_literal_empty_skips.bas");
    const JIT_FOR_EACH_ARRAY_LITERAL_ITEM_AFTER_COMPLETION: &str =
        include_str!("../../../conformance/tests/for_each_array_literal_item_after_completion.bas");
    const JIT_FOR_EACH_ARRAY_VARIABLE_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_array_variable_basic.bas");
    const JIT_FOR_EACH_ARRAY_VARIABLE_EXPLICIT_LOWER_BOUND: &str =
        include_str!("../../../conformance/tests/for_each_array_variable_explicit_lower_bound.bas");
    const JIT_FOR_EACH_ARRAY_VARIABLE_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_array_variable_item_after_completion.bas"
    );
    const JIT_FOR_EACH_BOOLEAN_ARRAY_DYNAMIC_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_boolean_array_dynamic_basic.bas");
    const JIT_FOR_EACH_BOOLEAN_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_boolean_array_dynamic_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_BOOLEAN_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_boolean_array_dynamic_item_after_completion.bas"
    );
    const JIT_FOR_EACH_BOOLEAN_ARRAY_DYNAMIC_MULTIDIM_ORDER: &str = include_str!(
        "../../../conformance/tests/for_each_boolean_array_dynamic_multidim_order.bas"
    );
    const JIT_FOR_EACH_BOOLEAN_ARRAY_FIXED_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_boolean_array_fixed_basic.bas");
    const JIT_FOR_EACH_BOOLEAN_ARRAY_FIXED_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_boolean_array_fixed_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_BOOLEAN_ARRAY_FIXED_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_boolean_array_fixed_item_after_completion.bas"
    );
    const JIT_FOR_EACH_BOOLEAN_ARRAY_FIXED_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_boolean_array_fixed_multidim_order.bas");
    const JIT_FOR_EACH_BYTE_ARRAY_DYNAMIC_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_byte_array_dynamic_basic.bas");
    const JIT_FOR_EACH_BYTE_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_byte_array_dynamic_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_BYTE_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_byte_array_dynamic_item_after_completion.bas"
    );
    const JIT_FOR_EACH_BYTE_ARRAY_DYNAMIC_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_byte_array_dynamic_multidim_order.bas");
    const JIT_FOR_EACH_BYTE_ARRAY_FIXED_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_byte_array_fixed_basic.bas");
    const JIT_FOR_EACH_BYTE_ARRAY_FIXED_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_byte_array_fixed_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_BYTE_ARRAY_FIXED_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_byte_array_fixed_item_after_completion.bas"
    );
    const JIT_FOR_EACH_BYTE_ARRAY_FIXED_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_byte_array_fixed_multidim_order.bas");
    const JIT_FOR_EACH_INTEGER_ARRAY_DYNAMIC_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_integer_array_dynamic_basic.bas");
    const JIT_FOR_EACH_INTEGER_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_integer_array_dynamic_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_INTEGER_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_integer_array_dynamic_item_after_completion.bas"
    );
    const JIT_FOR_EACH_INTEGER_ARRAY_DYNAMIC_MULTIDIM_ORDER: &str = include_str!(
        "../../../conformance/tests/for_each_integer_array_dynamic_multidim_order.bas"
    );
    const JIT_FOR_EACH_INTEGER_ARRAY_FIXED_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_integer_array_fixed_basic.bas");
    const JIT_FOR_EACH_INTEGER_ARRAY_FIXED_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_integer_array_fixed_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_INTEGER_ARRAY_FIXED_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_integer_array_fixed_item_after_completion.bas"
    );
    const JIT_FOR_EACH_INTEGER_ARRAY_FIXED_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_integer_array_fixed_multidim_order.bas");
    const JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_long_array_dynamic_basic.bas");
    const JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_long_array_dynamic_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_long_array_dynamic_item_after_completion.bas"
    );
    const JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_long_array_dynamic_multidim_order.bas");
    const JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_3D_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_long_array_dynamic_3d_order.bas");
    const JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_4D_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_long_array_dynamic_4d_order.bas");
    const JIT_FOR_EACH_LONG_ARRAY_FIXED_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_long_array_fixed_basic.bas");
    const JIT_FOR_EACH_LONG_ARRAY_FIXED_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_long_array_fixed_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_LONG_ARRAY_FIXED_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_long_array_fixed_item_after_completion.bas"
    );
    const JIT_FOR_EACH_LONG_ARRAY_FIXED_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_long_array_fixed_multidim_order.bas");
    const JIT_FOR_EACH_LONG_ARRAY_FIXED_3D_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_long_array_fixed_3d_order.bas");
    const JIT_FOR_EACH_LONG_ARRAY_FIXED_4D_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_long_array_fixed_4d_order.bas");
    const JIT_FOR_EACH_TYPED_SCALAR_3D_ORDER_BUNDLE: &str =
        include_str!("../../../conformance/tests/for_each_typed_scalar_3d_order_bundle.bas");
    const JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_3D_ORDER_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_typed_scalar_dynamic_3d_order_bundle.bas"
    );
    const JIT_FOR_EACH_TYPED_SCALAR_MULTIDIM_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_typed_scalar_multidim_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_MULTIDIM_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_typed_scalar_dynamic_multidim_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_TYPED_SCALAR_3D_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_typed_scalar_3d_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_3D_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_typed_scalar_dynamic_3d_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_TYPED_SCALAR_4D_ORDER_BUNDLE: &str =
        include_str!("../../../conformance/tests/for_each_typed_scalar_4d_order_bundle.bas");
    const JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_4D_ORDER_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_typed_scalar_dynamic_4d_order_bundle.bas"
    );
    const JIT_FOR_EACH_TYPED_SCALAR_4D_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_typed_scalar_4d_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_4D_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_typed_scalar_dynamic_4d_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_LONGLONG_ARRAY_DYNAMIC_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_longlong_array_dynamic_basic.bas");
    const JIT_FOR_EACH_LONGLONG_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_longlong_array_dynamic_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_LONGLONG_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_longlong_array_dynamic_item_after_completion.bas"
    );
    const JIT_FOR_EACH_LONGLONG_ARRAY_DYNAMIC_MULTIDIM_ORDER: &str = include_str!(
        "../../../conformance/tests/for_each_longlong_array_dynamic_multidim_order.bas"
    );
    const JIT_FOR_EACH_LONGLONG_ARRAY_FIXED_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_longlong_array_fixed_basic.bas");
    const JIT_FOR_EACH_LONGLONG_ARRAY_FIXED_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_longlong_array_fixed_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_LONGLONG_ARRAY_FIXED_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_longlong_array_fixed_item_after_completion.bas"
    );
    const JIT_FOR_EACH_LONGLONG_ARRAY_FIXED_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_longlong_array_fixed_multidim_order.bas");
    const JIT_FOR_EACH_SINGLE_ARRAY_DYNAMIC_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_single_array_dynamic_basic.bas");
    const JIT_FOR_EACH_SINGLE_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_single_array_dynamic_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_SINGLE_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_single_array_dynamic_item_after_completion.bas"
    );
    const JIT_FOR_EACH_SINGLE_ARRAY_DYNAMIC_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_single_array_dynamic_multidim_order.bas");
    const JIT_FOR_EACH_SINGLE_ARRAY_FIXED_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_single_array_fixed_basic.bas");
    const JIT_FOR_EACH_SINGLE_ARRAY_FIXED_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_single_array_fixed_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_SINGLE_ARRAY_FIXED_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_single_array_fixed_item_after_completion.bas"
    );
    const JIT_FOR_EACH_SINGLE_ARRAY_FIXED_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_single_array_fixed_multidim_order.bas");
    const JIT_FOR_EACH_DOUBLE_ARRAY_DYNAMIC_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_double_array_dynamic_basic.bas");
    const JIT_FOR_EACH_DOUBLE_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_double_array_dynamic_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_DOUBLE_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_double_array_dynamic_item_after_completion.bas"
    );
    const JIT_FOR_EACH_DOUBLE_ARRAY_DYNAMIC_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_double_array_dynamic_multidim_order.bas");
    const JIT_FOR_EACH_DOUBLE_ARRAY_FIXED_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_double_array_fixed_basic.bas");
    const JIT_FOR_EACH_DOUBLE_ARRAY_FIXED_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_double_array_fixed_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_DOUBLE_ARRAY_FIXED_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_double_array_fixed_item_after_completion.bas"
    );
    const JIT_FOR_EACH_DOUBLE_ARRAY_FIXED_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_double_array_fixed_multidim_order.bas");
    const JIT_FOR_EACH_CURRENCY_ARRAY_DYNAMIC_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_currency_array_dynamic_basic.bas");
    const JIT_FOR_EACH_CURRENCY_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_currency_array_dynamic_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_CURRENCY_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_currency_array_dynamic_item_after_completion.bas"
    );
    const JIT_FOR_EACH_CURRENCY_ARRAY_DYNAMIC_MULTIDIM_ORDER: &str = include_str!(
        "../../../conformance/tests/for_each_currency_array_dynamic_multidim_order.bas"
    );
    const JIT_FOR_EACH_CURRENCY_ARRAY_FIXED_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_currency_array_fixed_basic.bas");
    const JIT_FOR_EACH_CURRENCY_ARRAY_FIXED_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_currency_array_fixed_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_CURRENCY_ARRAY_FIXED_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_currency_array_fixed_item_after_completion.bas"
    );
    const JIT_FOR_EACH_CURRENCY_ARRAY_FIXED_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_currency_array_fixed_multidim_order.bas");
    const JIT_FOR_EACH_DATE_ARRAY_DYNAMIC_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_date_array_dynamic_basic.bas");
    const JIT_FOR_EACH_DATE_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_date_array_dynamic_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_DATE_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_date_array_dynamic_item_after_completion.bas"
    );
    const JIT_FOR_EACH_DATE_ARRAY_DYNAMIC_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_date_array_dynamic_multidim_order.bas");
    const JIT_FOR_EACH_DATE_ARRAY_FIXED_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_date_array_fixed_basic.bas");
    const JIT_FOR_EACH_DATE_ARRAY_FIXED_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_date_array_fixed_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_DATE_ARRAY_FIXED_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_date_array_fixed_item_after_completion.bas"
    );
    const JIT_FOR_EACH_DATE_ARRAY_FIXED_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_date_array_fixed_multidim_order.bas");
    const JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_string_array_dynamic_basic.bas");
    const JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_string_array_dynamic_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_string_array_dynamic_item_after_completion.bas"
    );
    const JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_string_array_dynamic_multidim_order.bas");
    const JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_4D_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_string_array_dynamic_4d_order.bas");
    const JIT_FOR_EACH_STRING_ARRAY_FIXED_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_string_array_fixed_basic.bas");
    const JIT_FOR_EACH_STRING_ARRAY_FIXED_EXPLICIT_LOWER_BOUND: &str = include_str!(
        "../../../conformance/tests/for_each_string_array_fixed_explicit_lower_bound.bas"
    );
    const JIT_FOR_EACH_STRING_ARRAY_FIXED_ITEM_AFTER_COMPLETION: &str = include_str!(
        "../../../conformance/tests/for_each_string_array_fixed_item_after_completion.bas"
    );
    const JIT_FOR_EACH_STRING_ARRAY_FIXED_MULTIDIM_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_string_array_fixed_multidim_order.bas");
    const JIT_FOR_EACH_STRING_ARRAY_FIXED_4D_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_string_array_fixed_4d_order.bas");
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_fixed_string_array_dynamic_basic.bas");
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_dynamic_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_WIDTH_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_dynamic_width_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_MULTIDIM_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_dynamic_multidim_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_MULTIDIM_WIDTH_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_dynamic_multidim_width_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_MULTIDIM_ORDER: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_dynamic_multidim_order.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_3D_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_fixed_string_array_dynamic_3d_order.bas");
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_3D_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_dynamic_3d_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_3D_WIDTH_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_dynamic_3d_width_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_4D_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_fixed_string_array_dynamic_4d_order.bas");
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_4D_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_dynamic_4d_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_4D_WIDTH_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_dynamic_4d_width_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_BASIC: &str =
        include_str!("../../../conformance/tests/for_each_fixed_string_array_fixed_basic.bas");
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_fixed_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_WIDTH_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_fixed_width_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_MULTIDIM_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_fixed_multidim_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_MULTIDIM_WIDTH_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_fixed_multidim_width_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_MULTIDIM_ORDER: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_fixed_multidim_order.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_3D_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_fixed_string_array_fixed_3d_order.bas");
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_3D_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_fixed_3d_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_3D_WIDTH_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_fixed_3d_width_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_4D_ORDER: &str =
        include_str!("../../../conformance/tests/for_each_fixed_string_array_fixed_4d_order.bas");
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_4D_ITEM_AFTER_COMPLETION_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_fixed_4d_item_after_completion_bundle.bas"
    );
    const JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_4D_WIDTH_BUNDLE: &str = include_str!(
        "../../../conformance/tests/for_each_fixed_string_array_fixed_4d_width_bundle.bas"
    );
    const JIT_FOR_EACH_BOOLEAN_SCALAR_ERROR: &str =
        include_str!("../../../conformance/tests/for_each_boolean_scalar_error.bas");
    const JIT_FOR_EACH_BYTE_SCALAR_ERROR: &str =
        include_str!("../../../conformance/tests/for_each_byte_scalar_error.bas");
    const JIT_FOR_EACH_CURRENCY_SCALAR_ERROR: &str =
        include_str!("../../../conformance/tests/for_each_currency_scalar_error.bas");
    const JIT_FOR_EACH_DATE_SCALAR_ERROR: &str =
        include_str!("../../../conformance/tests/for_each_date_scalar_error.bas");
    const JIT_FOR_EACH_DOUBLE_SCALAR_ERROR: &str =
        include_str!("../../../conformance/tests/for_each_double_scalar_error.bas");
    const JIT_FOR_EACH_FIXED_STRING_SCALAR_ERROR: &str =
        include_str!("../../../conformance/tests/for_each_fixed_string_scalar_error.bas");
    const JIT_FOR_EACH_INTEGER_SCALAR_ERROR: &str =
        include_str!("../../../conformance/tests/for_each_integer_scalar_error.bas");
    const JIT_FOR_EACH_LONG_SCALAR_ERROR: &str =
        include_str!("../../../conformance/tests/for_each_long_scalar_error.bas");
    const JIT_FOR_EACH_LONGLONG_SCALAR_ERROR: &str =
        include_str!("../../../conformance/tests/for_each_longlong_scalar_error.bas");
    const JIT_FOR_EACH_SINGLE_SCALAR_ERROR: &str =
        include_str!("../../../conformance/tests/for_each_single_scalar_error.bas");
    const JIT_FOR_EACH_STRING_SCALAR_ERROR: &str =
        include_str!("../../../conformance/tests/for_each_string_scalar_error.bas");
    const JIT_FOR_EACH_VARIANT_SCALAR_ERROR: &str =
        include_str!("../../../conformance/tests/for_each_variant_scalar_error.bas");
    const JIT_ARRAY_ERASE_FIXED_LONG_RESET: &str =
        include_str!("../../../conformance/tests/erase_fixed_long_reset.bas");
    const JIT_ARRAY_ERASE_FIXED_LONG_REJECTS_STRING_AFTER_RESET: &str =
        include_str!("../../../conformance/tests/erase_fixed_long_rejects_string_after_reset.bas");
    const JIT_ARRAY_ERASE_FIXED_TYPED_SCALAR_RESET_BUNDLE: &str =
        include_str!("../../../conformance/tests/erase_fixed_typed_scalar_reset_bundle.bas");
    const JIT_ARRAY_ERASE_FIXED_TYPED_SCALAR_MULTIDIM_RESET_BUNDLE: &str = include_str!(
        "../../../conformance/tests/erase_fixed_typed_scalar_multidim_reset_bundle.bas"
    );
    const JIT_ARRAY_ERASE_FIXED_TYPED_SCALAR_3D_RESET_BUNDLE: &str =
        include_str!("../../../conformance/tests/erase_fixed_typed_scalar_3d_reset_bundle.bas");
    const JIT_ARRAY_ERASE_FIXED_TYPED_SCALAR_4D_RESET_BUNDLE: &str =
        include_str!("../../../conformance/tests/erase_fixed_typed_scalar_4d_reset_bundle.bas");
    const JIT_ARRAY_ERASE_FIXED_BOUNDS_PRESERVED: &str = "\
Sub Main()
Dim score As Long
Dim value
Dim a(2 To 4)
a(2) = 7
score = LBound(a) * 1000 + UBound(a) * 100
Erase a
value = a(2)
score = score + LBound(a) * 10 + UBound(a)
End Sub
";
    const JIT_ARRAY_ERASE_DYNAMIC_BOUNDS_ERROR: &str = "\
Sub Main()
Dim a()
    ReDim a(1)
    Erase a
    a(0) = 7
End Sub
";
    const JIT_ARRAY_ERASE_DYNAMIC_LONG_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_long_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_BOOLEAN_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_boolean_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_BYTE_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_byte_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_INTEGER_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_integer_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_LONGLONG_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_longlong_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_SINGLE_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_single_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_DOUBLE_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_double_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_CURRENCY_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_currency_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_DATE_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_date_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_STRING_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_string_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_BOOLEAN_MULTIDIM_UNALLOCATED_ERROR: &str = include_str!(
        "../../../conformance/tests/erase_dynamic_boolean_multidim_unallocated_error.bas"
    );
    const JIT_ARRAY_ERASE_DYNAMIC_BYTE_MULTIDIM_UNALLOCATED_ERROR: &str = include_str!(
        "../../../conformance/tests/erase_dynamic_byte_multidim_unallocated_error.bas"
    );
    const JIT_ARRAY_ERASE_DYNAMIC_INTEGER_MULTIDIM_UNALLOCATED_ERROR: &str = include_str!(
        "../../../conformance/tests/erase_dynamic_integer_multidim_unallocated_error.bas"
    );
    const JIT_ARRAY_ERASE_DYNAMIC_LONG_MULTIDIM_UNALLOCATED_ERROR: &str = include_str!(
        "../../../conformance/tests/erase_dynamic_long_multidim_unallocated_error.bas"
    );
    const JIT_ARRAY_ERASE_DYNAMIC_LONGLONG_MULTIDIM_UNALLOCATED_ERROR: &str = include_str!(
        "../../../conformance/tests/erase_dynamic_longlong_multidim_unallocated_error.bas"
    );
    const JIT_ARRAY_ERASE_DYNAMIC_SINGLE_MULTIDIM_UNALLOCATED_ERROR: &str = include_str!(
        "../../../conformance/tests/erase_dynamic_single_multidim_unallocated_error.bas"
    );
    const JIT_ARRAY_ERASE_DYNAMIC_DOUBLE_MULTIDIM_UNALLOCATED_ERROR: &str = include_str!(
        "../../../conformance/tests/erase_dynamic_double_multidim_unallocated_error.bas"
    );
    const JIT_ARRAY_ERASE_DYNAMIC_CURRENCY_MULTIDIM_UNALLOCATED_ERROR: &str = include_str!(
        "../../../conformance/tests/erase_dynamic_currency_multidim_unallocated_error.bas"
    );
    const JIT_ARRAY_ERASE_DYNAMIC_DATE_MULTIDIM_UNALLOCATED_ERROR: &str = include_str!(
        "../../../conformance/tests/erase_dynamic_date_multidim_unallocated_error.bas"
    );
    const JIT_ARRAY_ERASE_DYNAMIC_STRING_MULTIDIM_UNALLOCATED_ERROR: &str = include_str!(
        "../../../conformance/tests/erase_dynamic_string_multidim_unallocated_error.bas"
    );
    const JIT_ARRAY_ERASE_DYNAMIC_BOOLEAN_3D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_boolean_3d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_BYTE_3D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_byte_3d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_INTEGER_3D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_integer_3d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_LONG_3D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_long_3d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_LONGLONG_3D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_longlong_3d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_SINGLE_3D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_single_3d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_DOUBLE_3D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_double_3d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_CURRENCY_3D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_currency_3d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_DATE_3D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_date_3d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_STRING_3D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_string_3d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_BOOLEAN_4D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_boolean_4d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_BYTE_4D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_byte_4d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_INTEGER_4D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_integer_4d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_LONG_4D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_long_4d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_LONGLONG_4D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_longlong_4d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_SINGLE_4D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_single_4d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_DOUBLE_4D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_double_4d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_CURRENCY_4D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_currency_4d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_DATE_4D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_date_4d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_STRING_4D_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_string_4d_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_LBOUND_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_lbound_unallocated_error.bas");
    const JIT_ARRAY_ERASE_DYNAMIC_UBOUND_UNALLOCATED_ERROR: &str =
        include_str!("../../../conformance/tests/erase_dynamic_ubound_unallocated_error.bas");

    const JIT_FUNCTION_RETURN_LONG_CALL: &str = "\
Public g As Long
Sub Main()
  g = Twice(7)
End Sub
Function Twice(ByVal x As Long) As Long
  Twice = x * 2
End Function
";

    const JIT_BYREF_LONG_CALL: &str = "\
Public g As Long
Sub Main()
  Dim n As Long
  n = 7
  Call Bump(n)
  g = n
End Sub
Sub Bump(ByRef x As Long)
  x = x + 1
End Sub
";

    const JIT_NESTED_BYREF_LONG_CALL: &str = "\
Public g As Long
Sub Main()
  Dim n As Long
  n = 7
  Call Forward(n)
  g = n
End Sub
Sub Forward(ByRef x As Long)
  Call Bump(x)
End Sub
Sub Bump(ByRef y As Long)
  y = y + 1
End Sub
";

    const JIT_TWO_ARG_FUNCTION_RETURN_LONG: &str = "\
Public g As Long
Sub Main()
  g = Sum2(5, 7)
End Sub
Function Sum2(ByVal a As Long, ByVal b As Long) As Long
  Sum2 = a + b
End Function
";

    const JIT_THREE_ARG_FUNCTION_RETURN_LONG: &str = "\
Public g As Long
Sub Main()
  g = Sum3(2, 4, 6)
End Sub
Function Sum3(ByVal a As Long, ByVal b As Long, ByVal c As Long) As Long
  Sum3 = a + b + c
End Function
";

    const JIT_FOUR_ARG_MIXED_SCALAR_CALL: &str = "\
Public g As Long
Sub Main()
  g = Pick4(3, 4, True, 5)
End Sub
Function Pick4(ByVal a As Long, ByVal b As Integer, ByVal flag As Boolean, ByVal c As Long) As Long
  If flag Then
    Pick4 = a + b + c
  Else
    Pick4 = 0
End If
End Function
";

    const JIT_FIVE_ARG_FUNCTION_RETURN_LONG: &str = "\
Public g As Long
Sub Main()
  g = Sum5(1, 2, 3, 4, 5)
End Sub
Function Sum5(ByVal a As Long, ByVal b As Long, ByVal c As Long, ByVal d As Long, ByVal e As Long) As Long
  Sum5 = a + b + c + d + e
End Function
";

    const JIT_OPTIONAL_VARIANT_DEFAULT_CALL: &str = "\
Public g As Long
Sub Main()
  g = AddOpt(5)
End Sub
Function AddOpt(ByVal n As Long, Optional ByVal bonus As Variant = 7) As Long
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_VARIANT_OMITTED_CALL: &str = "\
Public g As Long
Sub Main()
  Call Touch()
End Sub
Sub Touch(Optional ByVal value As Variant)
  g = 7
End Sub
";

    const JIT_OPTIONAL_VARIANT_INTERMEDIATE_OMITTED_ISMISSING_CALL: &str = "\
Public gMissing As Boolean
Public gPresent As Boolean
Public gSum As Long
Sub Main()
  Call Capture(3, , 5)
End Sub
Sub Capture(ByVal first As Long, Optional ByVal middle As Variant, Optional ByVal last As Variant)
  gMissing = IsMissing(middle)
  gPresent = IsMissing(last)
  gSum = first + last
End Sub
";

    const JIT_OPTIONAL_LONG_DEFAULT_CALL: &str = "\
Public g As Long
Sub Main()
  g = AddOpt(5)
End Sub
Function AddOpt(ByVal n As Long, Optional ByVal bonus As Long = 7) As Long
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_LONG_OMITTED_CALL: &str = "\
Public g As Long
Sub Main()
  Call Touch()
End Sub
Sub Touch(Optional ByVal value As Long)
  g = value
End Sub
";

    const JIT_OPTIONAL_DOUBLE_DEFAULT_CALL: &str = "\
Public g As Double
Sub Main()
  g = AddOpt(5#)
End Sub
Function AddOpt(ByVal n As Double, Optional ByVal bonus As Double = 1.5) As Double
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_DOUBLE_OMITTED_CALL: &str = "\
Public g As Double
Sub Main()
  Call Touch()
End Sub
Sub Touch(Optional ByVal value As Double)
  g = value
End Sub
";

    const JIT_OPTIONAL_CURRENCY_DEFAULT_CALL: &str = "\
Public g As Currency
Sub Main()
  g = AddOpt(1.25@)
End Sub
Function AddOpt(ByVal n As Currency, Optional ByVal bonus As Currency = 1.25@) As Currency
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_CURRENCY_OMITTED_CALL: &str = "\
Public g As Currency
Sub Main()
  Call Touch()
End Sub
Sub Touch(Optional ByVal value As Currency)
  g = value
End Sub
";

    const JIT_OPTIONAL_BOOL_DEFAULT_CALL: &str = "\
Public g As Boolean
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_OMITTED_CALL: &str = "\
Public g As Boolean
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As Boolean) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_DEFAULT_CALL: &str = "\
Public g As Byte
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_OMITTED_CALL: &str = "\
Public g As Byte
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As Byte) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_DEFAULT_CALL: &str = "\
Public g As Integer
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_OMITTED_CALL: &str = "\
Public g As Integer
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As Integer) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_DEFAULT_CALL: &str = "\
Public g As LongLong
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_OMITTED_CALL: &str = "\
Public g As LongLong
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As LongLong) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_DEFAULT_CALL: &str = "\
Public g As Single
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_OMITTED_CALL: &str = "\
Public g As Single
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As Single) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_DEFAULT_CALL: &str = "\
Public g As Date
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_OMITTED_CALL: &str = "\
Public g As Date
Sub Main()
  g = PickOpt()
End Sub
    Function PickOpt(Optional ByVal value As Date) As Date
      PickOpt = value
    End Function
    ";

    const JIT_OPTIONAL_VARIANT_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Long
Sub Main()
  Dim bonus As Variant
  bonus = 8&
  g = AddOpt(5, bonus)
End Sub
Function AddOpt(ByVal n As Long, Optional ByVal bonus As Variant = 7) As Long
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_LONG_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Long
Sub Main()
  Dim bonus As Long
  bonus = 8
  g = AddOpt(5, bonus)
End Sub
Function AddOpt(ByVal n As Long, Optional ByVal bonus As Long = 7) As Long
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_DOUBLE_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Double
Sub Main()
  Dim bonus As Double
  bonus = 2.25#
  g = AddOpt(5#, bonus)
End Sub
Function AddOpt(ByVal n As Double, Optional ByVal bonus As Double = 1.5) As Double
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_CURRENCY_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim bonus As Currency
  bonus = 2.5@
  g = AddOpt(1.25@, bonus)
End Sub
Function AddOpt(ByVal n As Currency, Optional ByVal bonus As Currency = 1.25@) As Currency
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_BOOL_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim value As Boolean
  value = False
  g = PickOpt(value)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim value As Byte
  value = 9
  g = PickOpt(value)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim value As Integer
  value = 34
  g = PickOpt(value)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_EXPLICIT_LOCAL_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim value As LongLong
  value = 5000000013^
  g = PickOpt(value)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Single
Sub Main()
  Dim value As Single
  value = 2.5!
  g = PickOpt(value)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Date
Sub Main()
  Dim value As Date
  value = #2000-01-03#
  g = PickOpt(value)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_VARIANT_NAMED_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Long
Sub Main()
  Dim extra As Variant
  extra = 8&
  g = AddOpt(bonus:=extra, n:=5)
End Sub
Function AddOpt(ByVal n As Long, Optional ByVal bonus As Variant = 7) As Long
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_LONG_NAMED_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Long
Sub Main()
  Dim extra As Long
  extra = 8
  g = AddOpt(bonus:=extra, n:=5)
End Sub
Function AddOpt(ByVal n As Long, Optional ByVal bonus As Long = 7) As Long
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_DOUBLE_NAMED_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Double
Sub Main()
  Dim extra As Double
  extra = 2.25#
  g = AddOpt(bonus:=extra, n:=5#)
End Sub
Function AddOpt(ByVal n As Double, Optional ByVal bonus As Double = 1.5) As Double
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_CURRENCY_NAMED_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim extra As Currency
  extra = 2.5@
  g = AddOpt(bonus:=extra, n:=1.25@)
End Sub
Function AddOpt(ByVal n As Currency, Optional ByVal bonus As Currency = 1.25@) As Currency
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_LONG_NAMED_ARG_ORDER_DOUBLE_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  Dim extra As Double
  extra = 8#
  g = AddOpt(bonus:=extra, n:=5)
End Sub
Function AddOpt(ByVal n As Long, Optional ByVal bonus As Long = 7) As Long
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_DOUBLE_NAMED_ARG_ORDER_LONG_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim extra As Long
  extra = 2
  g = AddOpt(bonus:=extra, n:=5#)
End Sub
Function AddOpt(ByVal n As Double, Optional ByVal bonus As Double = 1.5) As Double
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_CURRENCY_NAMED_ARG_ORDER_INTEGER_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim extra As Integer
  extra = 2
  g = AddOpt(bonus:=extra, n:=1.25@)
End Sub
Function AddOpt(ByVal n As Currency, Optional ByVal bonus As Currency = 1.25@) As Currency
  AddOpt = n + bonus
End Function
";

    const JIT_OPTIONAL_BOOL_NAMED_ARG_ORDER_DOUBLE_ZERO_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  Dim extra As Double
  extra = 0#
  g = PickWithFlag(flag:=extra, n:=5)
End Sub
Function PickWithFlag(ByVal n As Long, Optional ByVal flag As Boolean = True) As Long
  If flag Then
    PickWithFlag = n + 1
  Else
    PickWithFlag = n
  End If
End Function
";

    const JIT_OPTIONAL_BOOL_NAMED_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim actual As Boolean
  actual = False
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_NAMED_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim actual As Byte
  actual = 9
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_NAMED_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim actual As Integer
  actual = 34
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_NAMED_EXPLICIT_LOCAL_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim actual As LongLong
  actual = 5000000013^
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_NAMED_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Single
Sub Main()
  Dim actual As Single
  actual = 2.5!
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_NAMED_EXPLICIT_LOCAL_CALL: &str = "\
Public g As Date
Sub Main()
  Dim actual As Date
  actual = #2000-01-03#
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_EXPLICIT_DOUBLE_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  Dim actual As Double
  actual = 8#
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_EXPLICIT_LONG_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim actual As Long
  actual = 8
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_EXPLICIT_DOUBLE_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim actual As Double
  actual = 2.5#
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_EXPLICIT_LONG_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim actual As Long
  actual = 2
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Boolean = False) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_EXPLICIT_INTEGER_COERCE_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim actual As Integer
  actual = 9
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_EXPLICIT_LONG_COERCE_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim actual As Long
  actual = 34
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_EXPLICIT_LONG_COERCE_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim actual As Long
  actual = 34
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_EXPLICIT_DOUBLE_COERCE_CALL: &str = "\
Public g As Single
Sub Main()
  Dim actual As Double
  actual = 2.5#
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_EXPLICIT_DOUBLE_COERCE_CALL: &str = "\
Public g As Date
Sub Main()
  Dim actual As Double
  actual = 36528#
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_EXPLICIT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Long
Sub Main()
  g = PickOpt(Null)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_EXPLICIT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Boolean
Sub Main()
  g = PickOpt(Null)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_EXPLICIT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Byte
Sub Main()
  g = PickOpt(Null)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_EXPLICIT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Integer
Sub Main()
  g = PickOpt(Null)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_EXPLICIT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As LongLong
Sub Main()
  g = PickOpt(Null)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_EXPLICIT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Single
Sub Main()
  g = PickOpt(Null)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_EXPLICIT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Double
Sub Main()
  g = PickOpt(Null)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_EXPLICIT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Currency
Sub Main()
  g = PickOpt(Null)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_EXPLICIT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Date
Sub Main()
  g = PickOpt(Null)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_EXPLICIT_BOOLEAN_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  g = PickOpt(True)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_EXPLICIT_EMPTY_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  g = PickOpt(Empty)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_EXPLICIT_EMPTY_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  g = PickOpt(Empty)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_EXPLICIT_EMPTY_COERCE_CALL: &str = "\
Public g As Date
Sub Main()
  g = PickOpt(Empty)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_EXPLICIT_CURRENCY_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim actual As Currency
  actual = 2.5@
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_EXPLICIT_LONG_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim actual As Long
  actual = 2
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_EXPLICIT_LONG_COERCE_CALL: &str = "\
Public g As Single
Sub Main()
  Dim actual As Long
  actual = 34
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_EXPLICIT_LONG_OVERFLOW_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim actual As Long
  actual = 256
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_EXPLICIT_LONG_OVERFLOW_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim actual As Long
  actual = 32768
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_EXPLICIT_ERROR_COERCE_ERROR_CALL: &str = "\
Public g As Long
Sub Main()
  g = PickOpt(CVErr(1234))
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_EXPLICIT_LONG_COERCE_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim actual As Long
  actual = 34
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_EXPLICIT_BYTE_COERCE_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim actual As Byte
  actual = 9
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_EXPLICIT_DOUBLE_COERCE_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim actual As Double
  actual = 34#
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_EXPLICIT_CURRENCY_COERCE_CALL: &str = "\
Public g As Single
Sub Main()
  Dim actual As Currency
  actual = 2.5@
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_EXPLICIT_SINGLE_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim actual As Single
  actual = 2.5!
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_EXPLICIT_INTEGER_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim actual As Integer
  actual = 2
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_EXPLICIT_DOUBLE_ZERO_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim actual As Double
  actual = 0#
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_EXPLICIT_LONG_COERCE_CALL: &str = "\
Public g As Date
Sub Main()
  Dim actual As Long
  actual = 36528
  g = PickOpt(actual)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_NAMED_DOUBLE_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  Dim actual As Double
  actual = 8#
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_NAMED_LONG_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim actual As Long
  actual = 8
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_NAMED_DOUBLE_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim actual As Double
  actual = 2.5#
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_NAMED_LONG_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim actual As Long
  actual = 2
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Boolean = False) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_NAMED_INTEGER_COERCE_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim actual As Integer
  actual = 9
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_NAMED_LONG_COERCE_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim actual As Long
  actual = 34
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_NAMED_LONG_COERCE_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim actual As Long
  actual = 34
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_NAMED_DOUBLE_COERCE_CALL: &str = "\
Public g As Single
Sub Main()
  Dim actual As Double
  actual = 2.5#
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_NAMED_DOUBLE_COERCE_CALL: &str = "\
Public g As Date
Sub Main()
  Dim actual As Double
  actual = 36528#
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_NAMED_LONG_COERCE_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim actual As Long
  actual = 34
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_NAMED_BYTE_COERCE_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim actual As Byte
  actual = 9
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_NAMED_DOUBLE_COERCE_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim actual As Double
  actual = 34#
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_NAMED_CURRENCY_COERCE_CALL: &str = "\
Public g As Single
Sub Main()
  Dim actual As Currency
  actual = 2.5@
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_NAMED_SINGLE_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim actual As Single
  actual = 2.5!
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_NAMED_INTEGER_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim actual As Integer
  actual = 2
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_NAMED_DOUBLE_ZERO_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim actual As Double
  actual = 0#
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_NAMED_LONG_COERCE_CALL: &str = "\
Public g As Date
Sub Main()
  Dim actual As Long
  actual = 36528
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_NAMED_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Long
Sub Main()
  g = PickOpt(value:=Null)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_NAMED_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Boolean
Sub Main()
  g = PickOpt(value:=Null)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_NAMED_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Byte
Sub Main()
  g = PickOpt(value:=Null)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_NAMED_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Integer
Sub Main()
  g = PickOpt(value:=Null)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_NAMED_NULL_COERCE_ERROR_CALL: &str = "\
Public g As LongLong
Sub Main()
  g = PickOpt(value:=Null)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_NAMED_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Single
Sub Main()
  g = PickOpt(value:=Null)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_NAMED_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Double
Sub Main()
  g = PickOpt(value:=Null)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_NAMED_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Currency
Sub Main()
  g = PickOpt(value:=Null)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_NAMED_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Date
Sub Main()
  g = PickOpt(value:=Null)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_NAMED_BOOLEAN_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  g = PickOpt(value:=True)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_NAMED_EMPTY_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  g = PickOpt(value:=Empty)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_NAMED_EMPTY_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  g = PickOpt(value:=Empty)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_NAMED_EMPTY_COERCE_CALL: &str = "\
Public g As Date
Sub Main()
  g = PickOpt(value:=Empty)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_NAMED_CURRENCY_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim actual As Currency
  actual = 2.5@
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_NAMED_LONG_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim actual As Long
  actual = 2
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_NAMED_LONG_COERCE_CALL: &str = "\
Public g As Single
Sub Main()
  Dim actual As Long
  actual = 34
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_NAMED_LONG_OVERFLOW_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim actual As Long
  actual = 256
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_NAMED_LONG_OVERFLOW_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim actual As Long
  actual = 32768
  g = PickOpt(value:=actual)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_NAMED_ERROR_COERCE_ERROR_CALL: &str = "\
Public g As Long
Sub Main()
  g = PickOpt(value:=CVErr(1234))
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = 8#
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_EXPLICIT_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim v As Variant
  v = 8&
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim v As Variant
  v = 2.5#
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_EXPLICIT_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim v As Variant
  v = 2&
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Boolean = False) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_EXPLICIT_VARIANT_INTEGER_COERCE_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim actual As Integer
  Dim v As Variant
  actual = 9
  v = actual
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_EXPLICIT_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim v As Variant
  v = 34&
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_EXPLICIT_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim v As Variant
  v = 34&
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As Single
Sub Main()
  Dim v As Variant
  v = 2.5#
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As Date
Sub Main()
  Dim v As Variant
  v = 36528#
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_EXPLICIT_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim actual As Long
  Dim v As Variant
  actual = 34
  v = actual
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_EXPLICIT_VARIANT_BYTE_COERCE_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim actual As Byte
  Dim v As Variant
  actual = 9
  v = actual
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim actual As Double
  Dim v As Variant
  actual = 34#
  v = actual
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_EXPLICIT_VARIANT_CURRENCY_COERCE_CALL: &str = "\
Public g As Single
Sub Main()
  Dim actual As Currency
  Dim v As Variant
  actual = 2.5@
  v = actual
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_EXPLICIT_VARIANT_SINGLE_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim actual As Single
  Dim v As Variant
  actual = 2.5!
  v = actual
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_EXPLICIT_VARIANT_INTEGER_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim actual As Integer
  Dim v As Variant
  actual = 2
  v = actual
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_EXPLICIT_VARIANT_DOUBLE_ZERO_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim actual As Double
  Dim v As Variant
  actual = 0#
  v = actual
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_EXPLICIT_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Date
Sub Main()
  Dim actual As Long
  Dim v As Variant
  actual = 36528
  v = actual
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Single
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Double
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Date
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_BOOLEAN_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = True
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_EMPTY_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = Empty
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_EXPLICIT_VARIANT_EMPTY_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim v As Variant
  v = Empty
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_EXPLICIT_VARIANT_EMPTY_COERCE_CALL: &str = "\
Public g As Date
Sub Main()
  Dim v As Variant
  v = Empty
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_EXPLICIT_VARIANT_CURRENCY_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim v As Variant
  v = 2.5@
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_EXPLICIT_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim v As Variant
  v = 2&
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_EXPLICIT_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Single
Sub Main()
  Dim v As Variant
  v = 34&
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_EXPLICIT_VARIANT_LONG_OVERFLOW_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim v As Variant
  v = 256&
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_EXPLICIT_VARIANT_LONG_OVERFLOW_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim v As Variant
  v = 32768&
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_ERROR_COERCE_ERROR_CALL: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = CVErr(1234)
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_NAMED_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = 8#
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_NAMED_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim v As Variant
  v = 8&
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_NAMED_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim v As Variant
  v = 2.5#
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_NAMED_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim v As Variant
  v = 2&
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Boolean = False) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_NAMED_VARIANT_INTEGER_COERCE_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim actual As Integer
  Dim v As Variant
  actual = 9
  v = actual
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_NAMED_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim v As Variant
  v = 34&
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_NAMED_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim v As Variant
  v = 34&
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_NAMED_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As Single
Sub Main()
  Dim v As Variant
  v = 2.5#
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_NAMED_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As Date
Sub Main()
  Dim v As Variant
  v = 36528#
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_NAMED_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim actual As Long
  Dim v As Variant
  actual = 34
  v = actual
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_NAMED_VARIANT_BYTE_COERCE_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim actual As Byte
  Dim v As Variant
  actual = 9
  v = actual
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_NAMED_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim actual As Double
  Dim v As Variant
  actual = 34#
  v = actual
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_NAMED_VARIANT_CURRENCY_COERCE_CALL: &str = "\
Public g As Single
Sub Main()
  Dim actual As Currency
  Dim v As Variant
  actual = 2.5@
  v = actual
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_NAMED_VARIANT_SINGLE_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim actual As Single
  Dim v As Variant
  actual = 2.5!
  v = actual
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_NAMED_VARIANT_INTEGER_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim actual As Integer
  Dim v As Variant
  actual = 2
  v = actual
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_NAMED_VARIANT_DOUBLE_ZERO_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim actual As Double
  Dim v As Variant
  actual = 0#
  v = actual
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_NAMED_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Date
Sub Main()
  Dim actual As Long
  Dim v As Variant
  actual = 36528
  v = actual
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_NAMED_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_NAMED_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_NAMED_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_NAMED_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONGLONG_NAMED_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As LongLong = 5000000012^) As LongLong
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_NAMED_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Single
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_NAMED_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Double
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_NAMED_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_NAMED_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As Date
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_NAMED_VARIANT_BOOLEAN_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = True
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_NAMED_VARIANT_EMPTY_COERCE_CALL: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = Empty
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BOOL_NAMED_VARIANT_EMPTY_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  Dim v As Variant
  v = Empty
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Boolean = True) As Boolean
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DATE_NAMED_VARIANT_EMPTY_COERCE_CALL: &str = "\
Public g As Date
Sub Main()
  Dim v As Variant
  v = Empty
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Date = #2000-01-02#) As Date
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_DOUBLE_NAMED_VARIANT_CURRENCY_COERCE_CALL: &str = "\
Public g As Double
Sub Main()
  Dim v As Variant
  v = 2.5@
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Double = 1.5) As Double
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_CURRENCY_NAMED_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim v As Variant
  v = 2&
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Currency = 1.25@) As Currency
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_SINGLE_NAMED_VARIANT_LONG_COERCE_CALL: &str = "\
Public g As Single
Sub Main()
  Dim v As Variant
  v = 34&
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Single = 1.5!) As Single
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_BYTE_NAMED_VARIANT_LONG_OVERFLOW_CALL: &str = "\
Public g As Byte
Sub Main()
  Dim v As Variant
  v = 256&
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Byte = 7) As Byte
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_INTEGER_NAMED_VARIANT_LONG_OVERFLOW_CALL: &str = "\
Public g As Integer
Sub Main()
  Dim v As Variant
  v = 32768&
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Integer = 12) As Integer
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_LONG_NAMED_VARIANT_ERROR_COERCE_ERROR_CALL: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = CVErr(1234)
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As Long = 7) As Long
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_DEFAULT_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_OMITTED_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt()
End Sub
Function PickOpt(Optional ByVal value As String) As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_STRING_LITERAL_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(\"beta\")
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_STRING_LOCAL_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"beta\"
  g = PickOpt(text)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_EMPTY_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(Empty)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(Null)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_ERROR_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(CVErr(1234))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_DECIMAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(CDec(12345))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_NUMERIC_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(42&)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_BOOLEAN_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(True)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_DOUBLE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(12.5#)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_SINGLE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(12.5!)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_CURRENCY_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(12.3456@)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_INTEGER_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(44%)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_BYTE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim b As Byte
  b = 7
  g = PickOpt(b)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_LONGLONG_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(5000000012^)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_DATE_LITERAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(#2020-01-15#)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_DATESERIAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(DateSerial(2020, 1, 15))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_CDATE_NUMERIC_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(CDate(43845#))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_CDATE_STRING_LITERAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(CDate(\"2020-01-15\"))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_CDATE_MONTH_NAME_LITERAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(CDate(\"February 28, 2026\"))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_CDATE_INVALID_STRING_LITERAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(CDate(\"not-a-date\"))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_CDATE_STRING_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"2020-01-15\"
  g = PickOpt(CDate(text))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_CDATE_MONTH_NAME_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"February 28, 2026\"
  g = PickOpt(CDate(text))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_CDATE_INVALID_STRING_LOCAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"not-a-date\"
  g = PickOpt(CDate(text))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_DATE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim d As Date
  d = #2020-01-15#
  g = PickOpt(d)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 45&
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_BOOLEAN_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = True
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.5#
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_SINGLE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.5!
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_CURRENCY_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.3456@
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_INTEGER_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 44%
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_BYTE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  Dim b As Byte
  b = 7
  v = b
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_LONGLONG_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 5000000012^
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_DATE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = #2020-01-15#
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_STRING_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = \"beta\"
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_EMPTY_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_ERROR_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = CVErr(1234)
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_DECIMAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = CDec(12345)
  g = PickOpt(v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_NUMERIC_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=42&)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_LONG_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Long
  n = 42
  g = PickText(second:=n, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_DATE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim d As Date
  d = #2020-01-15#
  g = PickText(second:=d, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_DATESERIAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=DateSerial(2020, 1, 15), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_CDATE_NUMERIC_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=CDate(43845#), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_CDATE_MONTH_NAME_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"February 28, 2026\"
  g = PickText(second:=CDate(text), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_CDATE_INVALID_STRING_LITERAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=CDate(\"not-a-date\"), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_EMPTY_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=Empty, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_ERROR_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=CVErr(1234), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_BOOLEAN_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim flag As Boolean
  flag = True
  g = PickText(second:=flag, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_CURRENCY_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim amount As Currency
  amount = 12.3456@
  g = PickText(second:=amount, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_DOUBLE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=12.5#, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_SINGLE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=12.5!, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_INTEGER_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=44%, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_BYTE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim b As Byte
  b = 7
  g = PickText(second:=b, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_LONGLONG_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=5000000012^, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_DECIMAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = CDec(12345)
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_BOOLEAN_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = True
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.5#
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_SINGLE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.5!
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_CURRENCY_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.3456@
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_INTEGER_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 44%
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_BYTE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  Dim b As Byte
  b = 7
  v = b
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_LONGLONG_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 5000000012^
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_DATE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = #2020-01-15#
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_STRING_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = \"beta\"
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_ERROR_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = CVErr(1234)
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_EMPTY_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_NULL_COERCE_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=Null, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, Optional ByVal second As String = \"beta\") As String
  PickText = second
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_STRING_LITERAL_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=\"beta\")
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_STRING_LOCAL_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"beta\"
  g = PickOpt(value:=text)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_EMPTY_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=Empty)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_NULL_COERCE_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=Null)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_ERROR_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=CVErr(1234))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_DECIMAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=CDec(12345))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_BOOLEAN_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=True)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_DOUBLE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=12.5#)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_SINGLE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=12.5!)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_CURRENCY_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=12.3456@)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_INTEGER_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=44%)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_BYTE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim b As Byte
  b = 7
  g = PickOpt(value:=b)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_LONGLONG_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=5000000012^)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_DATE_LITERAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=#2020-01-15#)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_DATESERIAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=DateSerial(2020, 1, 15))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_CDATE_NUMERIC_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=CDate(43845#))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_CDATE_STRING_LITERAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=CDate(\"2020-01-15\"))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_CDATE_MONTH_NAME_LITERAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=CDate(\"February 28, 2026\"))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_CDATE_INVALID_STRING_LITERAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  g = PickOpt(value:=CDate(\"not-a-date\"))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_CDATE_STRING_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"2020-01-15\"
  g = PickOpt(value:=CDate(text))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_CDATE_MONTH_NAME_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"February 28, 2026\"
  g = PickOpt(value:=CDate(text))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_CDATE_INVALID_STRING_LOCAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"not-a-date\"
  g = PickOpt(value:=CDate(text))
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_DATE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim d As Date
  d = #2020-01-15#
  g = PickOpt(value:=d)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 45&
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_BOOLEAN_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = True
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_DOUBLE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.5#
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_SINGLE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.5!
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_CURRENCY_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.3456@
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_INTEGER_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 44%
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_BYTE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  Dim b As Byte
  b = 7
  v = b
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_LONGLONG_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 5000000012^
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_DATE_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = #2020-01-15#
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_STRING_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = \"beta\"
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_EMPTY_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_NULL_COERCE_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = Null
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_ERROR_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = CVErr(1234)
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_OPTIONAL_STRING_NAMED_VARIANT_DECIMAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = CDec(12345)
  g = PickOpt(value:=v)
End Sub
Function PickOpt(Optional ByVal value As String = \"alpha\") As String
  PickOpt = value
End Function
";

    const JIT_VARIANT_BOX_ASSIGNMENT: &str = "\
Public g As Variant
Sub Main()
  g = 42
End Sub
";

    const JIT_VARIANT_RETURN_CALL: &str = "\
Public g As Variant
Sub Main()
  g = Echo(42)
End Sub
Function Echo(ByVal v As Variant) As Variant
  Echo = v
End Function
";

    const JIT_VARIANT_BYREF_CALL: &str = "\
Public g As Variant
Sub Main()
  Call CopyValue(g, 42)
End Sub
    Sub CopyValue(ByRef target As Variant, ByVal value As Variant)
      target = value
    End Sub
    ";

    const JIT_STRING_BYVAL_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = EchoText(\"alpha\")
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_LOCAL_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  g = EchoText(text)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYREF_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String)
  text = \"beta\"
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(text, \"beta\")
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_NUMERIC_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(text, 42&)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_LONG_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As Long
  text = \"alpha\"
  n = 43
  Call ReplaceText(text, n)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_BOOLEAN_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim flag As Boolean
  text = \"alpha\"
  flag = True
  Call ReplaceText(text, flag)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_DOUBLE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As Double
  text = \"alpha\"
  n = 12.5#
  Call ReplaceText(text, n)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_SINGLE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As Single
  text = \"alpha\"
  n = 12.5!
  Call ReplaceText(text, n)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_CURRENCY_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As Currency
  text = \"alpha\"
  n = 12.3456@
  Call ReplaceText(text, n)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_INTEGER_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As Integer
  text = \"alpha\"
  n = 44
  Call ReplaceText(text, n)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_BYTE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As Byte
  text = \"alpha\"
  n = 7
  Call ReplaceText(text, n)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_LONGLONG_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As LongLong
  text = \"alpha\"
  n = 5000000012^
  Call ReplaceText(text, n)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_DATE_LITERAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(text, #2020-01-15#)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_DATE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim d As Date
  text = \"alpha\"
  d = #2020-01-15#
  Call ReplaceText(text, d)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_DATESERIAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(text, DateSerial(2020, 1, 15))
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_CDATE_NUMERIC_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(text, CDate(43845#))
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_CDATE_STRING_LITERAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(text, CDate(\"2020-01-15\"))
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_CDATE_STRING_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim dateText As String
  text = \"alpha\"
  dateText = \"2020-01-15\"
  Call ReplaceText(text, CDate(dateText))
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_CDATE_INVALID_STRING_LITERAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(text, CDate(\"not-a-date\"))
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_CDATE_INVALID_STRING_LOCAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim dateText As String
  text = \"alpha\"
  dateText = \"not-a-date\"
  Call ReplaceText(text, CDate(dateText))
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_CDATE_MONTH_NAME_LITERAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(text, CDate(\"February 28, 2026\"))
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_CDATE_MONTH_NAME_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim dateText As String
  text = \"alpha\"
  dateText = \"February 28, 2026\"
  Call ReplaceText(text, CDate(dateText))
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = 45&
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_BOOLEAN_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = True
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_DOUBLE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = 12.5#
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_SINGLE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = 12.5!
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_CURRENCY_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = 12.3456@
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_INTEGER_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = 44%
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_BYTE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  Dim b As Byte
  text = \"alpha\"
  b = 7
  v = b
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_LONGLONG_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = 5000000012^
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_DATE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = #2020-01-15#
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_STRING_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"beta\"
  v = \"alpha\"
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_ERROR_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = CVErr(1234)
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_DECIMAL_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = CDec(12345)
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_EMPTY_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_NULL_LOCAL_COERCE_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = Null
  Call ReplaceText(text, v)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_BYVAL_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=\"beta\", first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_NUMERIC_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=42&, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_LONG_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Long
  n = 43
  g = PickText(second:=n, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_BOOLEAN_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim flag As Boolean
  flag = True
  g = PickText(second:=flag, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_DOUBLE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Double
  n = 12.5#
  g = PickText(second:=n, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_SINGLE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Single
  n = 12.5!
  g = PickText(second:=n, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_CURRENCY_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Currency
  n = 12.3456@
  g = PickText(second:=n, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_INTEGER_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Integer
  n = 44
  g = PickText(second:=n, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_BYTE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Byte
  n = 7
  g = PickText(second:=n, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_LONGLONG_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As LongLong
  n = 5000000012^
  g = PickText(second:=n, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 45&
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_BOOLEAN_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = True
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_DOUBLE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.5#
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_SINGLE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.5!
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_CURRENCY_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.3456@
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_INTEGER_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 44%
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_BYTE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  Dim b As Byte
  b = 7
  v = b
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_LONGLONG_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 5000000012^
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_DATE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = #2020-01-15#
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_STRING_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = \"alpha\"
  g = PickText(second:=v, first:=\"beta\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_ERROR_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = CVErr(1234)
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_DECIMAL_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = CDec(12345)
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_EMPTY_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_VARIANT_NULL_LOCAL_COERCE_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = Null
  g = PickText(second:=v, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_DATE_LITERAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=#2020-01-15#, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_DATE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim d As Date
  d = #2020-01-15#
  g = PickText(second:=d, first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_DATESERIAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=DateSerial(2020, 1, 15), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_CDATE_NUMERIC_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=CDate(43845#), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_CDATE_STRING_LITERAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=CDate(\"2020-01-15\"), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_CDATE_STRING_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"2020-01-15\"
  g = PickText(second:=CDate(text), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_CDATE_INVALID_STRING_LITERAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=CDate(\"not-a-date\"), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_CDATE_INVALID_STRING_LOCAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"not-a-date\"
  g = PickText(second:=CDate(text), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_CDATE_MONTH_NAME_LITERAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = PickText(second:=CDate(\"February 28, 2026\"), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_BYVAL_CDATE_MONTH_NAME_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"February 28, 2026\"
  g = PickText(second:=CDate(text), first:=\"alpha\")
End Sub
Function PickText(ByVal first As String, ByVal second As String) As String
  PickText = second
End Function
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(value:=\"beta\", text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_NUMERIC_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(value:=42&, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_LONG_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As Long
  text = \"alpha\"
  n = 43
  Call ReplaceText(value:=n, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_BOOLEAN_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim flag As Boolean
  text = \"alpha\"
  flag = True
  Call ReplaceText(value:=flag, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_DOUBLE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As Double
  text = \"alpha\"
  n = 12.5#
  Call ReplaceText(value:=n, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_SINGLE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As Single
  text = \"alpha\"
  n = 12.5!
  Call ReplaceText(value:=n, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CURRENCY_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As Currency
  text = \"alpha\"
  n = 12.3456@
  Call ReplaceText(value:=n, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_INTEGER_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As Integer
  text = \"alpha\"
  n = 44
  Call ReplaceText(value:=n, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_BYTE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As Byte
  text = \"alpha\"
  n = 7
  Call ReplaceText(value:=n, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_LONGLONG_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim n As LongLong
  text = \"alpha\"
  n = 5000000012^
  Call ReplaceText(value:=n, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_DATE_LITERAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(value:=#2020-01-15#, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_DATE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim d As Date
  text = \"alpha\"
  d = #2020-01-15#
  Call ReplaceText(value:=d, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_DATESERIAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(value:=DateSerial(2020, 1, 15), text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_NUMERIC_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(value:=CDate(43845#), text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_STRING_LITERAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(value:=CDate(\"2020-01-15\"), text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_STRING_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim dateText As String
  text = \"alpha\"
  dateText = \"2020-01-15\"
  Call ReplaceText(value:=CDate(dateText), text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_INVALID_STRING_LITERAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(value:=CDate(\"not-a-date\"), text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_INVALID_STRING_LOCAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim dateText As String
  text = \"alpha\"
  dateText = \"not-a-date\"
  Call ReplaceText(value:=CDate(dateText), text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_MONTH_NAME_LITERAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"alpha\"
  Call ReplaceText(value:=CDate(\"February 28, 2026\"), text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_MONTH_NAME_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim dateText As String
  text = \"alpha\"
  dateText = \"February 28, 2026\"
  Call ReplaceText(value:=CDate(dateText), text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = 45&
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_BOOLEAN_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = True
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_DOUBLE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = 12.5#
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_SINGLE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = 12.5!
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_CURRENCY_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = 12.3456@
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_INTEGER_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = 44%
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_BYTE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  Dim b As Byte
  text = \"alpha\"
  b = 7
  v = b
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_LONGLONG_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = 5000000012^
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_DATE_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = #2020-01-15#
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_STRING_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"beta\"
  v = \"alpha\"
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_ERROR_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = CVErr(1234)
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_DECIMAL_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = CDec(12345)
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_EMPTY_LOCAL_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_NULL_LOCAL_COERCE_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  Dim v As Variant
  text = \"alpha\"
  v = Null
  Call ReplaceText(value:=v, text:=text)
  g = text
End Sub
Sub ReplaceText(ByRef text As String, ByVal value As String)
  text = value
End Sub
";

    const JIT_STRING_BYVAL_NUMERIC_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = EchoText(42&)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_LONG_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Long
  n = 43
  g = EchoText(n)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_BOOLEAN_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim flag As Boolean
  flag = True
  g = EchoText(flag)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_DOUBLE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Double
  n = 12.5#
  g = EchoText(n)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_SINGLE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Single
  n = 12.5!
  g = EchoText(n)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_CURRENCY_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Currency
  n = 12.3456@
  g = EchoText(n)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_INTEGER_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Integer
  n = 44
  g = EchoText(n)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_BYTE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Byte
  n = 7
  g = EchoText(n)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_LONGLONG_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As LongLong
  n = 5000000012^
  g = EchoText(n)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_DATE_LITERAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = EchoText(#2020-01-15#)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_DATE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim n As Date
  n = #2020-01-15#
  g = EchoText(n)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_DATESERIAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = EchoText(DateSerial(2020, 1, 15))
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_CDATE_NUMERIC_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = EchoText(CDate(43845#))
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_CDATE_STRING_LITERAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = EchoText(CDate(\"2020-01-15\"))
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_CDATE_STRING_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"2020-01-15\"
  g = EchoText(CDate(text))
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_CDATE_INVALID_STRING_LITERAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  g = EchoText(CDate(\"not-a-date\"))
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_CDATE_INVALID_STRING_LOCAL_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"not-a-date\"
  g = EchoText(CDate(text))
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_CDATE_MONTH_NAME_LITERAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  g = EchoText(CDate(\"February 28, 2026\"))
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_CDATE_MONTH_NAME_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim text As String
  text = \"February 28, 2026\"
  g = EchoText(CDate(text))
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 45&
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_BOOLEAN_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = True
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_DOUBLE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.5#
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_SINGLE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.5!
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_CURRENCY_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 12.3456@
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_INTEGER_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 44%
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_BYTE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim b As Byte
  Dim v As Variant
  b = 7
  v = b
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_LONGLONG_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = 5000000012^
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_DATE_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = #2020-01-15#
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_STRING_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = \"alpha\"
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_ERROR_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = CVErr(1234)
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_DECIMAL_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = CDec(12345)
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_EMPTY_LOCAL_COERCE_RETURN_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_STRING_BYVAL_VARIANT_NULL_LOCAL_COERCE_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  Dim v As Variant
  v = Null
  g = EchoText(v)
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_LONG_RETURN_TO_VARIANT_CALL: &str = "\
Public g As Variant
Sub Main()
  g = FortyTwo()
End Sub
Function FortyTwo() As Long
  FortyTwo = 42
End Function
";

    const JIT_STRING_RETURN_TO_VARIANT_CALL: &str = "\
Public g As Variant
Sub Main()
  g = EchoText(\"alpha\")
End Sub
Function EchoText(ByVal text As String) As String
  EchoText = text
End Function
";

    const JIT_VARIANT_RETURN_TO_LONG_CALL: &str = "\
Public g As Long
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = 42&
End Function
";

    const JIT_VARIANT_RETURN_TO_STRING_COERCE_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = 42&
End Function
";

    const JIT_VARIANT_RETURN_TO_BOOL_COERCE_CALL: &str = "\
Public g As Boolean
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = 2&
End Function
";

    const JIT_VARIANT_RETURN_TO_DOUBLE_CALL: &str = "\
Public g As Double
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = 12.5#
End Function
";

    const JIT_VARIANT_RETURN_TO_STRING_BOOLEAN_PAYLOAD_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = True
End Function
";

    const JIT_VARIANT_RETURN_TO_STRING_DOUBLE_PAYLOAD_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = 12.5#
End Function
";

    const JIT_VARIANT_RETURN_TO_STRING_SINGLE_PAYLOAD_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = 12.5!
End Function
";

    const JIT_VARIANT_RETURN_TO_STRING_CURRENCY_PAYLOAD_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = 12.3456@
End Function
";

    const JIT_VARIANT_RETURN_TO_STRING_INTEGER_PAYLOAD_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = 44%
End Function
";

    const JIT_VARIANT_RETURN_TO_STRING_BYTE_PAYLOAD_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Dim b As Byte
  b = 7
  Give = b
End Function
";

    const JIT_VARIANT_RETURN_TO_STRING_LONGLONG_PAYLOAD_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = 5000000012^
End Function
";

    const JIT_VARIANT_RETURN_TO_STRING_STRING_PAYLOAD_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = \"alpha\"
End Function
";

    const JIT_VARIANT_RETURN_TO_STRING_DATE_PAYLOAD_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = #2020-01-15#
End Function
";

    const JIT_VARIANT_RETURN_TO_STRING_ERROR_PAYLOAD_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = CVErr(1234)
End Function
";

    const JIT_VARIANT_RETURN_TO_STRING_DECIMAL_PAYLOAD_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = CDec(12345)
End Function
";

    const JIT_VARIANT_RETURN_EMPTY_TO_STRING_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
End Function
";

    const JIT_VARIANT_RETURN_NULL_TO_LONG_ERROR_CALL: &str = "\
Public g As Long
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = Null
End Function
";

    const JIT_VARIANT_RETURN_NULL_TO_STRING_ERROR_CALL: &str = "\
Public g As String
Sub Main()
  g = Give()
End Sub
Function Give() As Variant
  Give = Null
End Function
";

    const JIT_SCALAR_RETURNS_TO_VARIANT_CALL: &str = "\
Public gb As Variant
Public gby As Variant
Public gi As Variant
Public gll As Variant
Public gs As Variant
Public gd As Variant
Public gc As Variant
Public gdate As Variant
Sub Main()
  gb = GetBool()
  gby = GetByte()
  gi = GetInt()
  gll = GetWide()
  gs = GetSingle()
  gd = GetDouble()
  gc = GetCurrency()
  gdate = GetDate()
End Sub
Function GetBool() As Boolean
  GetBool = True
End Function
Function GetByte() As Byte
  GetByte = 12
End Function
Function GetInt() As Integer
  GetInt = 12
End Function
Function GetWide() As LongLong
  GetWide = 5000000012^
End Function
Function GetSingle() As Single
  GetSingle = 12.5!
End Function
Function GetDouble() As Double
  GetDouble = 12.5
End Function
Function GetCurrency() As Currency
  GetCurrency = 12.5@
End Function
Function GetDate() As Date
  GetDate = #2000-01-02#
End Function
";

    const JIT_MIXED_BYREF_BYVAL_LONG_CALL: &str = "\
Public g As Long
Sub Main()
  Dim n As Long
  n = 5
  Call AddInto(n, 7)
  g = n
End Sub
Sub AddInto(ByRef x As Long, ByVal y As Long)
  x = x + y
End Sub
";

    const JIT_INTEGER_BYREF_CALL: &str = "\
Public g As Long
Sub Main()
  Dim n As Integer
  n = 7
  Call SetInt(n)
  g = n
End Sub
Sub SetInt(ByRef x As Integer)
  x = 12
End Sub
";

    const JIT_BOOL_BYVAL_CALL: &str = "\
Public g As Long
Sub Main()
  Dim b As Boolean
  b = True
  Call Pick(b)
End Sub
Sub Pick(ByVal flag As Boolean)
  If flag Then
    g = 1
  Else
    g = 2
  End If
End Sub
";

    const JIT_BYTE_BYVAL_CALL: &str = "\
Public g As Long
Sub Main()
  Call TakeByte(12)
End Sub
Sub TakeByte(ByVal b As Byte)
  g = b
End Sub
";

    const JIT_BYTE_BYREF_CALL: &str = "\
Public g As Long
Sub Main()
  Dim n As Byte
  n = 7
  Call SetByte(n)
  g = n
End Sub
Sub SetByte(ByRef x As Byte)
  x = 12
End Sub
";

    const JIT_INTEGER_RETURN_CALL: &str = "\
Public g As Long
Sub Main()
  g = GetInt()
End Sub
Function GetInt() As Integer
  GetInt = 12
End Function
";

    const JIT_BYTE_RETURN_CALL: &str = "\
Public g As Long
Sub Main()
  g = GetByte()
End Sub
Function GetByte() As Byte
  GetByte = 12
End Function
";

    const JIT_BYTE_ARITHMETIC: &str = "\
Public g As Byte
Sub Main()
  Dim n As Byte
  n = 10
  n = n + 5
  n = n * 2
  g = n - 4
End Sub
";

    const JIT_BYTE_OVERFLOW: &str = "\
Public g As Byte
Sub Main()
  Dim n As Byte
  n = 255
  g = n + 1
End Sub
";

    const JIT_INTEGER_ARITHMETIC: &str = "\
Public g As Integer
Sub Main()
  Dim n As Integer
  n = 120
  n = n + 7
  n = n * 2
  g = n - 4
End Sub
";

    const JIT_INTEGER_OVERFLOW: &str = "\
Public g As Integer
Sub Main()
  Dim n As Integer
  n = 32767
  g = n + 1
End Sub
";

    const JIT_LONGLONG_BYREF_CALL: &str = "\
Public g As LongLong
Sub Main()
  Dim n As LongLong
  n = 5000000000^
  Call SetWide(n)
  g = n
End Sub
Sub SetWide(ByRef x As LongLong)
  x = 5000000012^
End Sub
";

    const JIT_LONGLONG_BYVAL_CALL: &str = "\
Public g As LongLong
Sub Main()
  g = EchoWide(5000000012^)
End Sub
Function EchoWide(ByVal x As LongLong) As LongLong
  EchoWide = x
End Function
";

    const JIT_LONGLONG_RETURN_CALL: &str = "\
Public g As LongLong
Sub Main()
  g = GetWide()
End Sub
Function GetWide() As LongLong
  GetWide = 5000000012^
End Function
";

    const JIT_LONGPTR_BYVAL_RETURN_CALL: &str = "\
Public g As LongPtr
Sub Main()
  g = EchoPtr(2147483647)
End Sub
Function EchoPtr(ByVal p As LongPtr) As LongPtr
  EchoPtr = p + p
End Function
";

    const JIT_LONGLONG_TRUTHY_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim n As LongLong
  n = 5000000012^
  If n Then
    g = 1
  Else
    g = 2
End If
End Sub
";

    const JIT_LONGLONG_COMPARE_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim a As LongLong
  Dim b As LongLong
  a = 5000000012^
  b = 5000000000^
  If a > b Then
    g = 1
  Else
    g = 2
  End If
End Sub
";

    const JIT_MIXED_FIXED_INTEGER_COMPARE_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim wide As LongLong
  Dim l As Long
  Dim i As Integer
  Dim b As Byte
  wide = 5000000012^
  l = 500000000
  i = 12
  b = 7
  If wide > l And i >= b Then
    g = 1
  Else
    g = 2
  End If
End Sub
";

    const JIT_DOUBLE_TRUTHY_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim d As Double
  d = 0.5
  If d Then
    g = 1
  Else
    g = 2
  End If
End Sub
";

    const JIT_DOUBLE_COMPARE_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim a As Double
  Dim b As Double
  a = 12.5
  b = 12.25
  If a > b Then
    g = 1
  Else
    g = 2
End If
End Sub
";

    const JIT_DOUBLE_ARITHMETIC: &str = "\
Public g As Double
Sub Main()
  Dim n As Double
  n = 1.25
  n = n + 2.5
  n = n * 2
  g = n - 1
End Sub
";

    const JIT_DOUBLE_BYREF_CALL: &str = "\
Public g As Double
Sub Main()
  Dim n As Double
  n = 1.5
  Call SetDouble(n)
  g = n
End Sub
Sub SetDouble(ByRef x As Double)
  x = 12.5
End Sub
";

    const JIT_DOUBLE_RETURN_CALL: &str = "\
Public g As Double
Sub Main()
  g = GetDouble()
End Sub
Function GetDouble() As Double
  GetDouble = 12.5
End Function
";

    const JIT_DOUBLE_BYVAL_CALL: &str = "\
Public g As Double
Sub Main()
  Dim n As Double
  n = 12.5
  g = EchoDouble(n)
End Sub
Function EchoDouble(ByVal x As Double) As Double
  EchoDouble = x
End Function
";

    const JIT_SINGLE_TRUTHY_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim s As Single
  s = 0.5!
  If s Then
    g = 1
  Else
    g = 2
  End If
End Sub
";

    const JIT_SINGLE_COMPARE_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim a As Single
  Dim b As Single
  a = 12.5!
  b = 12.25!
  If a > b Then
    g = 1
  Else
    g = 2
End If
End Sub
";

    const JIT_SINGLE_ARITHMETIC: &str = "\
Public g As Single
Sub Main()
  Dim n As Single
  n = 1.25!
  n = n + 2.5!
  n = n * 2!
  g = n - 1!
End Sub
";

    const JIT_SINGLE_BYREF_CALL: &str = "\
Public g As Single
Sub Main()
  Dim n As Single
  n = 1.5!
  Call SetSingle(n)
  g = n
End Sub
Sub SetSingle(ByRef x As Single)
  x = 12.5!
End Sub
";

    const JIT_SINGLE_RETURN_CALL: &str = "\
Public g As Single
Sub Main()
  g = GetSingle()
End Sub
Function GetSingle() As Single
  GetSingle = 12.5!
End Function
";

    const JIT_SINGLE_BYVAL_CALL: &str = "\
Public g As Single
Sub Main()
  Dim n As Single
  n = 12.5!
  g = EchoSingle(n)
End Sub
Function EchoSingle(ByVal x As Single) As Single
  EchoSingle = x
End Function
";

    const JIT_CURRENCY_BYREF_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim n As Currency
  n = 1.5@
  Call SetCurrency(n)
  g = n
End Sub
Sub SetCurrency(ByRef x As Currency)
  x = 12.5@
End Sub
";

    const JIT_CURRENCY_RETURN_CALL: &str = "\
Public g As Currency
Sub Main()
  g = GetCurrency()
End Sub
Function GetCurrency() As Currency
  GetCurrency = 12.5@
End Function
";

    const JIT_CURRENCY_BYVAL_CALL: &str = "\
Public g As Currency
Sub Main()
  Dim n As Currency
  n = 12.5@
  g = EchoCurrency(n)
End Sub
Function EchoCurrency(ByVal x As Currency) As Currency
  EchoCurrency = x
End Function
";

    const JIT_DATE_BYREF_CALL: &str = "\
Public g As Date
Sub Main()
  Dim n As Date
  n = #2000-01-01#
  Call SetDate(n)
  g = n
End Sub
Sub SetDate(ByRef x As Date)
  x = #2000-01-02#
End Sub
";

    const JIT_DATE_RETURN_CALL: &str = "\
Public g As Date
Sub Main()
  g = GetDate()
End Sub
Function GetDate() As Date
  GetDate = #2000-01-02#
End Function
";

    const JIT_DATE_BYVAL_CALL: &str = "\
Public g As Date
Sub Main()
  Dim n As Date
  n = #2000-01-02#
  g = EchoDate(n)
End Sub
Function EchoDate(ByVal x As Date) As Date
  EchoDate = x
End Function
";

    const JIT_DATE_TRUTHY_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim d As Date
  d = #2000-01-02#
  If d Then
    g = 1
  Else
    g = 2
  End If
End Sub
";

    const JIT_DATE_COMPARE_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim a As Date
  Dim b As Date
  a = #2000-01-02#
  b = #2000-01-01#
  If a > b Then
    g = 1
  Else
    g = 2
End If
End Sub
";

    const JIT_DATE_ARITHMETIC: &str = "\
Public g As Date
Sub Main()
  Dim d As Date
  d = #2000-01-01#
  d = d + 1
  g = d + 1
End Sub
";

    const JIT_BOOL_RETURN_CALL: &str = "\
Public g As Long
Sub Main()
  If IsReady(True) Then
    g = 1
  Else
    g = 2
  End If
End Sub
Function IsReady(ByVal flag As Boolean) As Boolean
  IsReady = flag
End Function
";

    const JIT_BOOL_NUMERIC_ASSIGNMENT: &str = "\
Public g As Boolean
Sub Main()
  g = 2
End Sub
";

    const JIT_BOOL_LOGICAL_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim a As Boolean
  Dim b As Boolean
  a = True
  b = False
  If a And Not b Then
    g = 1
  Else
    g = 2
  End If
End Sub
";

    const JIT_LONG_LOGICAL_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim a As Long
  Dim b As Long
  a = 6
  b = 3
  g = (a And b) Or 8
End Sub
";

    const JIT_FIXED_INTEGER_LOGICAL_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim i As Integer
  Dim b As Byte
  i = 12
  b = 5
  g = (i And b) Or (b Xor 2)
End Sub
";

    const JIT_LONGLONG_LOGICAL_EXPR: &str = "\
Public g As LongLong
Sub Main()
  Dim a As LongLong
  Dim b As LongLong
  a = 5000000012^
  b = 4294967296^
  g = (a And b) Or 3^
End Sub
";

    const JIT_LONGLONG_EQV_EXPR: &str = "\
Public g As LongLong
Sub Main()
  Dim a As LongLong
  a = 5000000012^
  g = a Eqv 0^
End Sub
";

    const JIT_LONGLONG_NOT_EXPR: &str = "\
Public g As LongLong
Sub Main()
  Dim a As LongLong
  a = 5000000012^
  g = Not a
End Sub
";

    const JIT_LONGLONG_MIXED_LOGICAL_EXPR: &str = "\
Public g As LongLong
Sub Main()
  Dim a As LongLong
  Dim l As Long
  Dim i As Integer
  Dim b As Byte
  a = 5000000012^
  l = 3
  i = 4
  b = 8
  g = (a Or l) + (a Xor i) + (a And b)
End Sub
";

    const JIT_VARIANT_LOGICAL_EXPR: &str = "\
Public g As Variant
Sub Main()
  Dim v As Variant
  v = Null
  g = Not (v And False)
End Sub
";

    const JIT_VARIANT_LOGICAL_NUMERIC_EXPR: &str = "\
Public g As Variant
Sub Main()
  Dim v As Variant
  v = 6
  g = v And 3
End Sub
";

    const JIT_VARIANT_TRUTHY_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = Null
  If v Then
    g = 1
  Else
    g = 2
End If
End Sub
";

    const JIT_VARIANT_COMPARE_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = Null
  If v = 1 Then
    g = 1
  Else
    g = 2
  End If
End Sub
";

    const JIT_VARIANT_COMPARE_NUMERIC_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = 2
  If v < 3.5 Then
    g = 1
  Else
    g = 2
  End If
End Sub
";

    const JIT_VARIANT_ARITHMETIC_NULL_EXPR: &str = "\
Public g As Variant
Sub Main()
  Dim v As Variant
  v = Null
  g = v + 3
End Sub
";

    const JIT_VARIANT_ARITHMETIC_MIXED_EXPR: &str = "\
Public g As Variant
Sub Main()
  Dim v As Variant
  v = 2
  g = v + 3.5
End Sub
";

    const JIT_VARIANT_NEGATION_EXPR: &str = "\
Public g As Variant
Sub Main()
  Dim v As Variant
  v = 2.5
  g = -v
End Sub
";

    const JIT_VARIANT_BYTE_COERCE_EXPR: &str = "\
Public g As Byte
Sub Main()
  Dim v As Variant
  v = 12
  g = v
End Sub
";

    const JIT_VARIANT_INTEGER_COERCE_EXPR: &str = "\
Public g As Integer
Sub Main()
  Dim v As Variant
  v = 1234
  g = v
End Sub
";

    const JIT_VARIANT_LONG_COERCE_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  v = 42
  g = v
End Sub
";

    const JIT_VARIANT_LONGLONG_COERCE_EXPR: &str = "\
Public g As LongLong
Sub Main()
  Dim v As Variant
  v = 5000000012^
  g = v
End Sub
";

    const JIT_VARIANT_SINGLE_COERCE_EXPR: &str = "\
Public g As Single
Sub Main()
  Dim v As Variant
  v = 1.25!
  g = v
End Sub
";

    const JIT_VARIANT_DOUBLE_COERCE_EXPR: &str = "\
Public g As Double
Sub Main()
  Dim v As Variant
  v = 6.5
  g = v
End Sub
";

    const JIT_VARIANT_CURRENCY_COERCE_EXPR: &str = "\
Public g As Currency
Sub Main()
  Dim v As Variant
  v = 12.5@
  g = v
End Sub
";

    const JIT_VARIANT_DATE_COERCE_EXPR: &str = "\
Public g As Date
Sub Main()
  Dim v As Variant
  v = #2000-01-02#
  g = v
End Sub
";

    const JIT_VARIANT_BOOL_COERCE_EXPR: &str = "\
Public g As Long
Sub Main()
  Dim v As Variant
  Dim b As Boolean
  v = 2
  b = v
  If b Then
    g = 1
  Else
    g = 2
  End If
End Sub
";

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
        assert!(
            o.handle_balance.is_some_and(HandleBalance::is_zero),
            "vm3 handle imbalance {:?}:\n{source}",
            o.handle_balance
        );
        o.result
            .unwrap_or_else(|e| panic!("vm3 run failed: {e}\n{source}"))
    }

    fn assert_jit_matches_vm3_contains(source: &str, expected: Variant) {
        let vm3 = run(Executor::Vm3, source);
        let jit = run(Executor::Jit, source);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let expected = canon(&expected);
        assert!(result.contains(&expected), "{result:?}");
    }

    fn assert_jit_matches_vm3_contains_canon(source: &str, expected: Canon) {
        let vm3 = run(Executor::Vm3, source);
        let jit = run(Executor::Jit, source);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&expected), "{result:?}");
    }

    fn assert_jit_matches_vm3_raises(source: &str, expected_number: i32) {
        let vm3 = run(Executor::Vm3, source);
        let jit = run(Executor::Jit, source);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert!(vm3.raised, "vm3 should raise {expected_number}: {vm3:?}");
        assert!(jit.raised, "jit should raise {expected_number}: {jit:?}");
        assert_eq!(jit.err.number, vm3.err.number);
        assert_eq!(jit.err.number, expected_number);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
    }

    #[test]
    fn vm3_runs_arithmetic() {
        let snap = run_vm3_ok("Sub Main()\n  Dim n As Long\n  n = (10 + 5) * 2\nEnd Sub\n");
        assert!(snap.contains(&canon(&Variant::from_i32(30))), "{snap:?}");
    }

    #[test]
    fn jit_matches_vm3_straight_line_long_arithmetic() {
        let vm3 = run(Executor::Vm3, JIT_STRAIGHT_LINE_LONG);
        let jit = run(Executor::Jit, JIT_STRAIGHT_LINE_LONG);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(27))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_overflow_matches_vm3_error_number() {
        let vm3 = run(Executor::Vm3, JIT_LONG_OVERFLOW);
        let jit = run(Executor::Jit, JIT_LONG_OVERFLOW);
        assert!(vm3.raised, "vm3 should raise overflow: {vm3:?}");
        assert!(jit.raised, "jit should raise overflow: {jit:?}");
        assert_eq!(jit.err.number, vm3.err.number);
        assert_eq!(jit.err.number, 6);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
    }

    #[test]
    fn jit_matches_vm3_longlong_arithmetic() {
        let vm3 = run(Executor::Vm3, JIT_LONGLONG_ARITHMETIC);
        let jit = run(Executor::Jit, JIT_LONGLONG_ARITHMETIC);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i64(10_000_000_020))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_longlong_overflow_matches_vm3_error_number() {
        let vm3 = run(Executor::Vm3, JIT_LONGLONG_OVERFLOW);
        let jit = run(Executor::Jit, JIT_LONGLONG_OVERFLOW);
        assert!(vm3.raised, "vm3 should raise overflow: {vm3:?}");
        assert!(jit.raised, "jit should raise overflow: {jit:?}");
        assert_eq!(jit.err.number, vm3.err.number);
        assert_eq!(jit.err.number, 6);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
    }

    #[test]
    fn jit_matches_vm3_currency_arithmetic() {
        let vm3 = run(Executor::Vm3, JIT_CURRENCY_ARITHMETIC);
        let jit = run(Executor::Jit, JIT_CURRENCY_ARITHMETIC);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(236_920))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_currency_overflow_matches_vm3_error_number() {
        let vm3 = run(Executor::Vm3, JIT_CURRENCY_OVERFLOW);
        let jit = run(Executor::Jit, JIT_CURRENCY_OVERFLOW);
        assert!(vm3.raised, "vm3 should raise overflow: {vm3:?}");
        assert!(jit.raised, "jit should raise overflow: {jit:?}");
        assert_eq!(jit.err.number, vm3.err.number);
        assert_eq!(jit.err.number, 6);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
    }

    #[test]
    fn jit_matches_vm3_currency_truthy_expr() {
        let vm3 = run(Executor::Vm3, JIT_CURRENCY_TRUTHY_EXPR);
        let jit = run(Executor::Jit, JIT_CURRENCY_TRUTHY_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_currency_compare_expr() {
        let vm3 = run(Executor::Vm3, JIT_CURRENCY_COMPARE_EXPR);
        let jit = run(Executor::Jit, JIT_CURRENCY_COMPARE_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_long_negation() {
        let vm3 = run(Executor::Vm3, JIT_LONG_NEGATION);
        let jit = run(Executor::Jit, JIT_LONG_NEGATION);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(-7))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_longlong_negation() {
        let vm3 = run(Executor::Vm3, JIT_LONGLONG_NEGATION);
        let jit = run(Executor::Jit, JIT_LONGLONG_NEGATION);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i64(-5_000_000_000))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_currency_negation() {
        let vm3 = run(Executor::Vm3, JIT_CURRENCY_NEGATION);
        let jit = run(Executor::Jit, JIT_CURRENCY_NEGATION);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(-123_456))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_single_negation() {
        let vm3 = run(Executor::Vm3, JIT_SINGLE_NEGATION);
        let jit = run(Executor::Jit, JIT_SINGLE_NEGATION);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f32(-1.25))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_double_negation() {
        let vm3 = run(Executor::Vm3, JIT_DOUBLE_NEGATION);
        let jit = run(Executor::Jit, JIT_DOUBLE_NEGATION);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(-2.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_long_intdiv_mod() {
        let vm3 = run(Executor::Vm3, JIT_LONG_INTDIV_MOD);
        let jit = run(Executor::Jit, JIT_LONG_INTDIV_MOD);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(5))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_longlong_intdiv_mod() {
        let vm3 = run(Executor::Vm3, JIT_LONGLONG_INTDIV_MOD);
        let jit = run(Executor::Jit, JIT_LONGLONG_INTDIV_MOD);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i64(1_000_000_005))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_double_division() {
        let vm3 = run(Executor::Vm3, JIT_DOUBLE_DIVISION);
        let jit = run(Executor::Jit, JIT_DOUBLE_DIVISION);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(4.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_double_exponentiation() {
        let vm3 = run(Executor::Vm3, JIT_DOUBLE_EXPONENTIATION);
        let jit = run(Executor::Jit, JIT_DOUBLE_EXPONENTIATION);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(81.0))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_for_loop() {
        let vm3 = run(Executor::Vm3, JIT_FOR_LOOP);
        let jit = run(Executor::Jit, JIT_FOR_LOOP);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(
            jit.unsupported.is_none(),
            "loop should compile in the M4-4 control-flow slice: {jit:?}"
        );
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(6))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_static_sub_call_byval_long() {
        let vm3 = run(Executor::Vm3, JIT_STATIC_SUB_CALL);
        let jit = run(Executor::Jit, JIT_STATIC_SUB_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(14))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_consolidate_nested_call_chain() {
        assert_jit_matches_vm3_contains(JIT_CONSOLIDATE_NESTED_CALL_CHAIN, Variant::from_i16(10));
    }

    #[test]
    fn jit_matches_vm3_gosub_basic() {
        assert_jit_matches_vm3_contains(JIT_GOSUB_BASIC, Variant::from_i16(4));
    }

    #[test]
    fn jit_matches_vm3_gosub_repeated() {
        assert_jit_matches_vm3_contains(JIT_GOSUB_REPEATED, Variant::from_i16(5));
    }

    #[test]
    fn jit_matches_vm3_gosub_nested_labels() {
        assert_jit_matches_vm3_contains(JIT_GOSUB_NESTED_LABELS, Variant::from_i16(15));
    }

    #[test]
    fn jit_matches_vm3_gosub_loop_accumulate() {
        assert_jit_matches_vm3_contains(JIT_GOSUB_LOOP_ACCUMULATE, Variant::from_i16(60));
        assert_jit_matches_vm3_contains(JIT_GOSUB_LOOP_ACCUMULATE, Variant::from_i16(4));
    }

    #[test]
    fn jit_matches_vm3_consolidate_for_gosub_mix() {
        assert_jit_matches_vm3_contains(JIT_CONSOLIDATE_FOR_GOSUB_MIX, Variant::from_i16(30));
        assert_jit_matches_vm3_contains(JIT_CONSOLIDATE_FOR_GOSUB_MIX, Variant::from_i16(5));
    }

    #[test]
    fn jit_matches_vm3_consolidate_gosub_error_mix() {
        assert_jit_matches_vm3_contains(JIT_CONSOLIDATE_GOSUB_ERROR_MIX, Variant::from_i16(10));
        assert_jit_matches_vm3_contains(JIT_CONSOLIDATE_GOSUB_ERROR_MIX, Variant::from_i32(77));
    }

    #[test]
    fn jit_matches_vm3_gosub_return_without_gosub() {
        assert_jit_matches_vm3_raises(JIT_GOSUB_RETURN_WITHOUT_GOSUB, 3);
    }

    #[test]
    fn jit_matches_vm3_gosub_return_without_gosub_resume_next() {
        assert_jit_matches_vm3_contains(
            JIT_GOSUB_RETURN_WITHOUT_GOSUB_RESUME_NEXT,
            Variant::from_i32(3),
        );
    }

    #[test]
    fn jit_matches_vm3_gosub_return_without_gosub_label_handler() {
        assert_jit_matches_vm3_contains(
            JIT_GOSUB_RETURN_WITHOUT_GOSUB_LABEL_HANDLER,
            Variant::from_i32(3),
        );
    }

    #[test]
    fn jit_matches_vm3_consolidate_for_select_call() {
        assert_jit_matches_vm3_contains(JIT_CONSOLIDATE_FOR_SELECT_CALL, Variant::from_i16(32));
        assert_jit_matches_vm3_contains(JIT_CONSOLIDATE_FOR_SELECT_CALL, Variant::from_i16(6));
    }

    #[test]
    fn jit_matches_vm3_consolidate_while_byref_mix() {
        assert_jit_matches_vm3_contains(JIT_CONSOLIDATE_WHILE_BYREF_MIX, Variant::from_i16(128));
        assert_jit_matches_vm3_contains(JIT_CONSOLIDATE_WHILE_BYREF_MIX, Variant::from_i16(7));
    }

    #[test]
    fn jit_matches_vm3_error_resume_function_propagation() {
        assert_jit_matches_vm3_contains(
            JIT_ERROR_RESUME_FUNCTION_PROPAGATION,
            Variant::from_i32(42),
        );
        assert_jit_matches_vm3_contains(
            JIT_ERROR_RESUME_FUNCTION_PROPAGATION,
            Variant::from_i16(7),
        );
    }

    #[test]
    fn jit_matches_vm3_error_nested_mode_transitions() {
        for expected in [5, 20, 0, 6] {
            assert_jit_matches_vm3_contains(
                JIT_ERROR_NESTED_MODE_TRANSITIONS,
                Variant::from_i32(expected),
            );
        }
    }

    #[test]
    fn jit_matches_vm3_paramarray_ubound_pack() {
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_UBOUND_PACK, Variant::from_i32(2));
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_pack() {
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_NAMED_FIXED_TAIL_PACK, Variant::from_i32(1));
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_ALIAS_COPYOUT,
            Variant::from_i32(17),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_ALIAS_COPYOUT,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_PARENTHESIZED_NO_ALIAS,
            Variant::from_i16(5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_byval_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_BYVAL_NO_ALIAS,
            Variant::from_i16(5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_BYVAL_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_array_element_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_ARRAY_ELEMENT_ALIAS_COPYOUT,
            Variant::from_i32(17),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_ARRAY_ELEMENT_ALIAS_COPYOUT,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_array_element_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_ARRAY_ELEMENT_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_ARRAY_ELEMENT_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_array_element_byval_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_ARRAY_ELEMENT_BYVAL_NO_ALIAS,
            Variant::from_i32(5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_ARRAY_ELEMENT_BYVAL_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_duplicate_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_DUPLICATE_ALIAS_COPYOUT,
            Variant::from_i32(23),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_DUPLICATE_ALIAS_COPYOUT,
            Variant::from_i32(102),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_duplicate_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_DUPLICATE_PARENTHESIZED_NO_ALIAS,
            Variant::from_i16(5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_DUPLICATE_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(102),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_duplicate_byval_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_DUPLICATE_BYVAL_NO_ALIAS,
            Variant::from_i16(5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_DUPLICATE_BYVAL_NO_ALIAS,
            Variant::from_i32(102),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_ALIAS_COPYOUT,
            Variant::from_i32(123),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_ALIAS_COPYOUT,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_byval_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_BYVAL_NO_ALIAS,
            Variant::from_i32(5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_BYVAL_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_string_alias_copyout() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_STRING_ALIAS_COPYOUT,
            Canon::Str("named-global".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_STRING_ALIAS_COPYOUT,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_string_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_STRING_PARENTHESIZED_NO_ALIAS,
            Canon::Str("before".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_STRING_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_string_byval_no_alias() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_STRING_BYVAL_NO_ALIAS,
            Canon::Str("before".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_STRING_BYVAL_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_longptr_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_LONGPTR_ALIAS_COPYOUT,
            Variant::from_i64(5_000_000_014),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_LONGPTR_ALIAS_COPYOUT,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_longptr_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_LONGPTR_PARENTHESIZED_NO_ALIAS,
            Variant::from_i64(17),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_LONGPTR_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_longptr_byval_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_LONGPTR_BYVAL_NO_ALIAS,
            Variant::from_i64(17),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_LONGPTR_BYVAL_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_typed_scalar_alias_bundle_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_i64(5_000_000_012),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_i16(99),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_u8(7),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_currency_scaled_i64(123_456),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_f32(1.25),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_f64(2.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_date_f64(36527.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_i32(108),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_typed_scalar_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_i64(1_111_111_111),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_i16(12),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_u8(3),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_bool(false),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_currency_scaled_i64(12_345),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_f32(4.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_f64(6.75),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_date_f64(2.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(108),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_typed_scalar_byval_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_i64(1_111_111_111),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_i16(12),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_u8(3),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_bool(false),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_currency_scaled_i64(12_345),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_f32(4.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_f64(6.75),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_date_f64(2.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_i32(108),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_fixed_string_alias_copyout() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_FIXED_STRING_ALIAS_COPYOUT,
            Canon::Str("abcdef".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_FIXED_STRING_ALIAS_COPYOUT,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_fixed_string_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_FIXED_STRING_PARENTHESIZED_NO_ALIAS,
            Canon::Str("abc".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_FIXED_STRING_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_global_fixed_string_byval_no_alias() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_FIXED_STRING_BYVAL_NO_ALIAS,
            Canon::Str("abc".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_FIXED_STRING_BYVAL_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_typed_long_string_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_ALIAS_COPYOUT,
            Variant::from_i32(99),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_ALIAS_COPYOUT,
            Canon::Str("mutated".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_ALIAS_COPYOUT,
            Variant::from_i32(102),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_typed_long_string_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(5),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_PARENTHESIZED_NO_ALIAS,
            Canon::Str("before".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(102),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_typed_long_string_byval_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_BYVAL_NO_ALIAS,
            Variant::from_i32(5),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_BYVAL_NO_ALIAS,
            Canon::Str("before".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_BYVAL_NO_ALIAS,
            Variant::from_i32(102),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_typed_longptr_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONGPTR_ALIAS_COPYOUT,
            Variant::from_i64(5_000_000_014),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONGPTR_ALIAS_COPYOUT,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_typed_longptr_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONGPTR_PARENTHESIZED_NO_ALIAS,
            Variant::from_i64(17),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONGPTR_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_typed_longptr_byval_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONGPTR_BYVAL_NO_ALIAS,
            Variant::from_i64(17),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONGPTR_BYVAL_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_typed_scalar_alias_bundle_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_i64(5_000_000_012),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_i16(99),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_u8(7),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_currency_scaled_i64(123_456),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_f32(1.25),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_f64(2.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_date_f64(36527.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            Variant::from_i32(108),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_typed_scalar_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_i64(1_111_111_111),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_i16(12),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_u8(3),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_bool(false),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_currency_scaled_i64(12_345),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_f32(4.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_f64(6.75),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_date_f64(2.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(108),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_typed_scalar_byval_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_i64(1_111_111_111),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_i16(12),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_u8(3),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_bool(false),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_currency_scaled_i64(12_345),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_f32(4.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_f64(6.75),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_date_f64(2.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            Variant::from_i32(108),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_typed_fixed_string_alias_copyout() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_FIXED_STRING_ALIAS_COPYOUT,
            Canon::Str("abcdef".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_FIXED_STRING_ALIAS_COPYOUT,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_typed_fixed_string_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_FIXED_STRING_PARENTHESIZED_NO_ALIAS,
            Canon::Str("abc".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_FIXED_STRING_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_named_fixed_tail_typed_fixed_string_byval_no_alias() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_FIXED_STRING_BYVAL_NO_ALIAS,
            Canon::Str("abc".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_FIXED_STRING_BYVAL_NO_ALIAS,
            Variant::from_i32(101),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_ubound_empty() {
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_UBOUND_EMPTY, Variant::from_i32(-1));
    }

    #[test]
    fn jit_matches_vm3_paramarray_omitted_tail_empty() {
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_OMITTED_TAIL_EMPTY, Variant::from_i32(0));
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_OMITTED_TAIL_EMPTY, Variant::from_i32(1));
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_OMITTED_TAIL_EMPTY,
            Canon::Str("Empty".into()),
        );
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_OMITTED_TAIL_EMPTY, Variant::from_i16(7));
    }

    #[test]
    fn jit_matches_vm3_paramarray_alias_copyout() {
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_ALIAS_COPYOUT, Variant::from_i16(11));
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_ALIAS_COPYOUT, Variant::from_i16(13));
    }

    #[test]
    fn jit_matches_vm3_paramarray_array_element_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_ARRAY_ELEMENT_ALIAS_COPYOUT,
            Variant::from_i16(11),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_ARRAY_ELEMENT_ALIAS_COPYOUT,
            Variant::from_i16(13),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_duplicate_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_DUPLICATE_ALIAS_COPYOUT,
            Variant::from_i16(13),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_PARENTHESIZED_NO_ALIAS,
            Variant::from_i16(5),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_PARENTHESIZED_NO_ALIAS,
            Variant::from_i16(9),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_byval_no_alias() {
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_BYVAL_NO_ALIAS, Variant::from_i16(5));
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_BYVAL_NO_ALIAS, Variant::from_i16(9));
    }

    #[test]
    fn jit_matches_vm3_paramarray_variant_array_element_mutation() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_VARIANT_ARRAY_ELEMENT_MUTATION,
            Variant::from_i16(99),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_global_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_GLOBAL_ALIAS_COPYOUT,
            Variant::from_i32(123),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_global_byval_no_alias() {
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_GLOBAL_BYVAL_NO_ALIAS, Variant::from_i32(5));
    }

    #[test]
    fn jit_matches_vm3_paramarray_global_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_GLOBAL_PARENTHESIZED_NO_ALIAS,
            Variant::from_i32(5),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_global_string_alias_copyout() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_GLOBAL_STRING_ALIAS_COPYOUT,
            Canon::Str("global".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_global_string_byval_no_alias() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_GLOBAL_STRING_BYVAL_NO_ALIAS,
            Canon::Str("before".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_global_string_parenthesized_no_alias() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_GLOBAL_STRING_PARENTHESIZED_NO_ALIAS,
            Canon::Str("before".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_typed_scalar_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_TYPED_SCALAR_ALIAS_COPYOUT,
            Variant::from_i32(99),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_typed_longlong_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_TYPED_LONGLONG_ALIAS_COPYOUT,
            Variant::from_i64(5_000_000_012),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_typed_longptr_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_TYPED_LONGPTR_ALIAS_COPYOUT,
            Variant::from_i64(5_000_000_014),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_typed_integer_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_TYPED_INTEGER_ALIAS_COPYOUT,
            Variant::from_i16(99),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_typed_byte_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_TYPED_BYTE_ALIAS_COPYOUT,
            Variant::from_u8(7),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_typed_boolean_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_TYPED_BOOLEAN_ALIAS_COPYOUT,
            Variant::from_bool(true),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_typed_string_alias_copyout() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_TYPED_STRING_ALIAS_COPYOUT,
            Canon::Str("mutated".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_typed_fixed_string_alias_copyout() {
        assert_jit_matches_vm3_contains_canon(
            JIT_PARAMARRAY_TYPED_FIXED_STRING_ALIAS_COPYOUT,
            Canon::Str("abcdef".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_typed_currency_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_TYPED_CURRENCY_ALIAS_COPYOUT,
            Variant::from_currency_scaled_i64(123_456),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_typed_single_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_TYPED_SINGLE_ALIAS_COPYOUT,
            Variant::from_f32(1.25),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_typed_double_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_TYPED_DOUBLE_ALIAS_COPYOUT,
            Variant::from_f64(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_typed_date_alias_copyout() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_TYPED_DATE_ALIAS_COPYOUT,
            Variant::from_date_f64(36527.0),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_bounds_explicit_dimension() {
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_BOUNDS_EXPLICIT_DIM, Variant::from_i32(0));
        assert_jit_matches_vm3_contains(JIT_PARAMARRAY_BOUNDS_EXPLICIT_DIM, Variant::from_i32(2));
    }

    #[test]
    fn jit_matches_vm3_paramarray_option_base_one_bounds() {
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_OPTION_BASE_ONE_BOUNDS,
            Variant::from_i32(0),
        );
        assert_jit_matches_vm3_contains(
            JIT_PARAMARRAY_OPTION_BASE_ONE_BOUNDS,
            Variant::from_i32(2),
        );
    }

    #[test]
    fn jit_matches_vm3_paramarray_lbound_dimension_zero_error() {
        assert_jit_matches_vm3_raises(JIT_PARAMARRAY_LBOUND_DIM_ZERO_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_paramarray_ubound_dimension_too_high_error() {
        assert_jit_matches_vm3_raises(JIT_PARAMARRAY_UBOUND_DIM_TOO_HIGH_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_literal_bounds() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_LITERAL_BOUNDS, Variant::from_i32(0));
        assert_jit_matches_vm3_contains(JIT_ARRAY_LITERAL_BOUNDS, Variant::from_i32(2));
    }

    #[test]
    fn jit_matches_vm3_array_literal_bounds_explicit_dimension() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_LITERAL_BOUNDS_EXPLICIT_DIM,
            Variant::from_i32(0),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_LITERAL_BOUNDS_EXPLICIT_DIM,
            Variant::from_i32(2),
        );
    }

    #[test]
    fn jit_matches_vm3_array_literal_bounds_dimension_zero_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_LITERAL_BOUNDS_DIM_ZERO_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_literal_bounds_dimension_too_high_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_LITERAL_BOUNDS_DIM_TOO_HIGH_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_dynamic_lbound_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_DYNAMIC_LBOUND_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_dynamic_ubound_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_DYNAMIC_UBOUND_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_dynamic_bounds_explicit_dimension() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_DYNAMIC_BOUNDS_EXPLICIT_DIM,
            Variant::from_i32(2),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_DYNAMIC_BOUNDS_EXPLICIT_DIM,
            Variant::from_i32(4),
        );
    }

    #[test]
    fn jit_matches_vm3_array_dynamic_lbound_dimension_zero_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_DYNAMIC_LBOUND_DIM_ZERO_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_dynamic_ubound_dimension_too_high_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_DYNAMIC_UBOUND_DIM_TOO_HIGH_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_dynamic_types() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_DYNAMIC_TYPES, Variant::from_i32(8204));
        assert_jit_matches_vm3_contains_canon(
            JIT_ARRAY_DYNAMIC_TYPES,
            Canon::Str("Variant()".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_array_fixed_information() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_FIXED_INFORMATION, Variant::from_i32(8204));
        assert_jit_matches_vm3_contains_canon(
            JIT_ARRAY_FIXED_INFORMATION,
            Canon::Str("Variant()".into()),
        );
        assert_jit_matches_vm3_contains(JIT_ARRAY_FIXED_INFORMATION, Variant::from_i32(1));
    }

    #[test]
    fn jit_matches_vm3_array_fixed_information_after_erase() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_INFORMATION_AFTER_ERASE,
            Variant::from_i32(8204),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_ARRAY_FIXED_INFORMATION_AFTER_ERASE,
            Canon::Str("Variant()".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_INFORMATION_AFTER_ERASE,
            Variant::from_i32(3),
        );
    }

    #[test]
    fn jit_matches_vm3_array_fixed_bounds_explicit_dimension() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_FIXED_BOUNDS_EXPLICIT_DIM, Variant::from_i32(2));
        assert_jit_matches_vm3_contains(JIT_ARRAY_FIXED_BOUNDS_EXPLICIT_DIM, Variant::from_i32(4));
    }

    #[test]
    fn jit_matches_vm3_array_fixed_lbound_dimension_zero_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_FIXED_LBOUND_DIM_ZERO_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_fixed_ubound_dimension_too_high_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_FIXED_UBOUND_DIM_TOO_HIGH_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_multidim_indexing() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_MULTIDIM_INDEXING, Variant::from_i16(17));
    }

    #[test]
    fn jit_matches_vm3_array_literal_types() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_LITERAL_TYPES, Variant::from_i32(8204));
        assert_jit_matches_vm3_contains_canon(
            JIT_ARRAY_LITERAL_TYPES,
            Canon::Str("Variant()".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_array_zero_index() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_ZERO_INDEX, Variant::from_i16(3));
    }

    #[test]
    fn jit_matches_vm3_array_store_load() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_STORE_LOAD, Variant::from_i16(7));
    }

    #[test]
    fn jit_matches_vm3_array_long_typed_store_load() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_LONG_TYPED_STORE_LOAD, Variant::from_i32(42));
    }

    #[test]
    fn jit_matches_vm3_array_long_read_to_variant() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_LONG_READ_TO_VARIANT, Variant::from_i32(42));
    }

    #[test]
    fn jit_matches_vm3_array_string_typed_store_load() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_STRING_TYPED_STORE_LOAD,
            Variant::from_string("alpha"),
        );
    }

    #[test]
    fn jit_matches_vm3_array_fixed_string_typed_store_load() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_TYPED_STORE_LOAD,
            Variant::from_string("a"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_TYPED_STORE_LOAD,
            Variant::from_string("abc"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_TYPED_STORE_LOAD,
            Variant::from_string("abcde"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_TYPED_STORE_LOAD,
            Variant::from_string("xy "),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_TYPED_STORE_LOAD,
            Variant::from_string("Z    "),
        );
    }

    #[test]
    fn jit_matches_vm3_array_fixed_string_dynamic_typed_store_load() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_TYPED_STORE_LOAD,
            Variant::from_string("a"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_TYPED_STORE_LOAD,
            Variant::from_string("abc"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_TYPED_STORE_LOAD,
            Variant::from_string("abcde"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_TYPED_STORE_LOAD,
            Variant::from_string("xy "),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_TYPED_STORE_LOAD,
            Variant::from_string("Z    "),
        );
    }

    #[test]
    fn jit_matches_vm3_array_fixed_string_multidim_typed_store_load() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_MULTIDIM_TYPED_STORE_LOAD,
            Variant::from_string("a"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_MULTIDIM_TYPED_STORE_LOAD,
            Variant::from_string("abc"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_MULTIDIM_TYPED_STORE_LOAD,
            Variant::from_string("abcde"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_MULTIDIM_TYPED_STORE_LOAD,
            Variant::from_string("xy "),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_MULTIDIM_TYPED_STORE_LOAD,
            Variant::from_string("Q    "),
        );
    }

    #[test]
    fn jit_matches_vm3_array_fixed_string_dynamic_multidim_typed_store_load() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_MULTIDIM_TYPED_STORE_LOAD,
            Variant::from_string("a"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_MULTIDIM_TYPED_STORE_LOAD,
            Variant::from_string("abc"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_MULTIDIM_TYPED_STORE_LOAD,
            Variant::from_string("abcde"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_MULTIDIM_TYPED_STORE_LOAD,
            Variant::from_string("xy "),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_MULTIDIM_TYPED_STORE_LOAD,
            Variant::from_string("Q    "),
        );
    }

    #[test]
    fn jit_matches_vm3_array_fixed_string_3d_typed_store_load() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_3D_TYPED_STORE_LOAD,
            Variant::from_string("a"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_3D_TYPED_STORE_LOAD,
            Variant::from_string("abc"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_3D_TYPED_STORE_LOAD,
            Variant::from_string("abcde"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_3D_TYPED_STORE_LOAD,
            Variant::from_string("xy "),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_3D_TYPED_STORE_LOAD,
            Variant::from_string("R    "),
        );
    }

    #[test]
    fn jit_matches_vm3_array_fixed_string_dynamic_3d_typed_store_load() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_3D_TYPED_STORE_LOAD,
            Variant::from_string("a"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_3D_TYPED_STORE_LOAD,
            Variant::from_string("abc"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_3D_TYPED_STORE_LOAD,
            Variant::from_string("abcde"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_3D_TYPED_STORE_LOAD,
            Variant::from_string("xy "),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_3D_TYPED_STORE_LOAD,
            Variant::from_string("R    "),
        );
    }

    #[test]
    fn jit_matches_vm3_array_fixed_string_4d_typed_store_load() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_4D_TYPED_STORE_LOAD,
            Variant::from_string("a"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_4D_TYPED_STORE_LOAD,
            Variant::from_string("abc"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_4D_TYPED_STORE_LOAD,
            Variant::from_string("abcde"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_4D_TYPED_STORE_LOAD,
            Variant::from_string("xy "),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_4D_TYPED_STORE_LOAD,
            Variant::from_string("S    "),
        );
    }

    #[test]
    fn jit_matches_vm3_array_fixed_string_dynamic_4d_typed_store_load() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_4D_TYPED_STORE_LOAD,
            Variant::from_string("a"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_4D_TYPED_STORE_LOAD,
            Variant::from_string("abc"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_4D_TYPED_STORE_LOAD,
            Variant::from_string("abcde"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_4D_TYPED_STORE_LOAD,
            Variant::from_string("xy "),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_FIXED_STRING_DYNAMIC_4D_TYPED_STORE_LOAD,
            Variant::from_string("S    "),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_store_load_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_STORE_LOAD_BUNDLE,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_STORE_LOAD_BUNDLE,
            Variant::from_u8(7),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_STORE_LOAD_BUNDLE,
            Variant::from_i16(44),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_STORE_LOAD_BUNDLE,
            Variant::from_i32(42),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_STORE_LOAD_BUNDLE,
            Variant::from_i64(5_000_000_012),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_STORE_LOAD_BUNDLE,
            Variant::from_f32(1.25),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_STORE_LOAD_BUNDLE,
            Variant::from_f64(2.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_STORE_LOAD_BUNDLE,
            Variant::from_currency_scaled_i64(123_456),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_STORE_LOAD_BUNDLE,
            Variant::from_date_f64(36527.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_STORE_LOAD_BUNDLE,
            Variant::from_string("alpha"),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_dynamic_store_load_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_STORE_LOAD_BUNDLE,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_STORE_LOAD_BUNDLE,
            Variant::from_u8(7),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_STORE_LOAD_BUNDLE,
            Variant::from_i16(44),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_STORE_LOAD_BUNDLE,
            Variant::from_i32(42),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_STORE_LOAD_BUNDLE,
            Variant::from_i64(5_000_000_012),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_STORE_LOAD_BUNDLE,
            Variant::from_f32(1.25),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_STORE_LOAD_BUNDLE,
            Variant::from_f64(2.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_STORE_LOAD_BUNDLE,
            Variant::from_currency_scaled_i64(123_456),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_STORE_LOAD_BUNDLE,
            Variant::from_date_f64(36527.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_STORE_LOAD_BUNDLE,
            Variant::from_string("alpha"),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_multidim_store_load_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_u8(7),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_i16(44),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_i32(42),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_i64(5_000_000_012),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_f32(1.25),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_f64(2.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_currency_scaled_i64(123_456),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_date_f64(36527.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_string("alpha"),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_dynamic_multidim_store_load_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_u8(7),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_i16(44),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_i32(42),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_i64(5_000_000_012),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_f32(1.25),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_f64(2.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_currency_scaled_i64(123_456),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_date_f64(36527.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_STORE_LOAD_BUNDLE,
            Variant::from_string("alpha"),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_3d_store_load_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_3D_STORE_LOAD_BUNDLE,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_3D_STORE_LOAD_BUNDLE,
            Variant::from_u8(7),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_3D_STORE_LOAD_BUNDLE,
            Variant::from_i16(44),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_3D_STORE_LOAD_BUNDLE,
            Variant::from_i32(42),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_3D_STORE_LOAD_BUNDLE,
            Variant::from_i64(5_000_000_012),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_3D_STORE_LOAD_BUNDLE,
            Variant::from_f32(1.25),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_3D_STORE_LOAD_BUNDLE,
            Variant::from_f64(2.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_3D_STORE_LOAD_BUNDLE,
            Variant::from_currency_scaled_i64(123_456),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_3D_STORE_LOAD_BUNDLE,
            Variant::from_date_f64(36527.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_3D_STORE_LOAD_BUNDLE,
            Variant::from_string("alpha"),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_dynamic_3d_store_load_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_STORE_LOAD_BUNDLE,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_STORE_LOAD_BUNDLE,
            Variant::from_u8(7),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_STORE_LOAD_BUNDLE,
            Variant::from_i16(44),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_STORE_LOAD_BUNDLE,
            Variant::from_i32(42),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_STORE_LOAD_BUNDLE,
            Variant::from_i64(5_000_000_012),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_STORE_LOAD_BUNDLE,
            Variant::from_f32(1.25),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_STORE_LOAD_BUNDLE,
            Variant::from_f64(2.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_STORE_LOAD_BUNDLE,
            Variant::from_currency_scaled_i64(123_456),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_STORE_LOAD_BUNDLE,
            Variant::from_date_f64(36527.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_STORE_LOAD_BUNDLE,
            Variant::from_string("alpha"),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_4d_store_load_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_4D_STORE_LOAD_BUNDLE,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_4D_STORE_LOAD_BUNDLE,
            Variant::from_u8(7),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_4D_STORE_LOAD_BUNDLE,
            Variant::from_i16(44),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_4D_STORE_LOAD_BUNDLE,
            Variant::from_i32(42),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_4D_STORE_LOAD_BUNDLE,
            Variant::from_i64(5_000_000_012),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_4D_STORE_LOAD_BUNDLE,
            Variant::from_f32(1.25),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_4D_STORE_LOAD_BUNDLE,
            Variant::from_f64(2.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_4D_STORE_LOAD_BUNDLE,
            Variant::from_currency_scaled_i64(123_456),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_4D_STORE_LOAD_BUNDLE,
            Variant::from_date_f64(36527.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_4D_STORE_LOAD_BUNDLE,
            Variant::from_string("alpha"),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_dynamic_4d_store_load_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_STORE_LOAD_BUNDLE,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_STORE_LOAD_BUNDLE,
            Variant::from_u8(7),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_STORE_LOAD_BUNDLE,
            Variant::from_i16(44),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_STORE_LOAD_BUNDLE,
            Variant::from_i32(42),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_STORE_LOAD_BUNDLE,
            Variant::from_i64(5_000_000_012),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_STORE_LOAD_BUNDLE,
            Variant::from_f32(1.25),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_STORE_LOAD_BUNDLE,
            Variant::from_f64(2.5),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_STORE_LOAD_BUNDLE,
            Variant::from_currency_scaled_i64(123_456),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_STORE_LOAD_BUNDLE,
            Variant::from_date_f64(36527.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_STORE_LOAD_BUNDLE,
            Variant::from_string("alpha"),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_multidim_bounds_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_MULTIDIM_BOUNDS_BUNDLE,
            Variant::from_i32(64_210),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_dynamic_multidim_bounds_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_BOUNDS_BUNDLE,
            Variant::from_i32(64_210),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_multidim_bounds_dimension_expression_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_MULTIDIM_BOUNDS_DIM_EXPR_BUNDLE,
            Variant::from_i32(64_210),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_dynamic_multidim_bounds_dimension_expression_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_BOUNDS_DIM_EXPR_BUNDLE,
            Variant::from_i32(64_210),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_3d_bounds_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_3D_BOUNDS_BUNDLE,
            Variant::from_i32(9_764_210),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_dynamic_3d_bounds_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_BOUNDS_BUNDLE,
            Variant::from_i32(9_764_210),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_4d_bounds_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_4D_BOUNDS_BUNDLE,
            Variant::from_i32(1_309_764_210),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_scalar_dynamic_4d_bounds_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_BOUNDS_BUNDLE,
            Variant::from_i32(1_309_764_210),
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_3d_lbound_dimension_zero_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_3D_LBOUND_DIM_ZERO_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_3d_ubound_dimension_too_high_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_3D_UBOUND_DIM_TOO_HIGH_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_dynamic_3d_lbound_dimension_zero_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_DYNAMIC_3D_LBOUND_DIM_ZERO_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_dynamic_3d_ubound_dimension_too_high_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_DYNAMIC_3D_UBOUND_DIM_TOO_HIGH_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_3d_lbound_dimension_expression_zero_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_3D_LBOUND_DIM_EXPR_ZERO_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_3d_ubound_dimension_expression_too_high_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_3D_UBOUND_DIM_EXPR_TOO_HIGH_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_dynamic_3d_lbound_dimension_expression_zero_error() {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_TYPED_LONG_DYNAMIC_3D_LBOUND_DIM_EXPR_ZERO_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_dynamic_3d_ubound_dimension_expression_too_high_error() {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_TYPED_LONG_DYNAMIC_3D_UBOUND_DIM_EXPR_TOO_HIGH_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_4d_lbound_dimension_zero_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_4D_LBOUND_DIM_ZERO_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_4d_ubound_dimension_too_high_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_4D_UBOUND_DIM_TOO_HIGH_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_dynamic_4d_lbound_dimension_zero_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_DYNAMIC_4D_LBOUND_DIM_ZERO_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_dynamic_4d_ubound_dimension_too_high_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_DYNAMIC_4D_UBOUND_DIM_TOO_HIGH_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_4d_lbound_dimension_expression_zero_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_4D_LBOUND_DIM_EXPR_ZERO_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_4d_ubound_dimension_expression_too_high_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_4D_UBOUND_DIM_EXPR_TOO_HIGH_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_dynamic_4d_lbound_dimension_expression_zero_error() {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_TYPED_LONG_DYNAMIC_4D_LBOUND_DIM_EXPR_ZERO_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_dynamic_4d_ubound_dimension_expression_too_high_error() {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_TYPED_LONG_DYNAMIC_4D_UBOUND_DIM_EXPR_TOO_HIGH_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_multidim_lbound_dimension_zero_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_MULTIDIM_LBOUND_DIM_ZERO_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_multidim_ubound_dimension_too_high_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_MULTIDIM_UBOUND_DIM_TOO_HIGH_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_dynamic_multidim_lbound_dimension_zero_error() {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_TYPED_LONG_DYNAMIC_MULTIDIM_LBOUND_DIM_ZERO_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_dynamic_multidim_ubound_dimension_too_high_error() {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_TYPED_LONG_DYNAMIC_MULTIDIM_UBOUND_DIM_TOO_HIGH_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_multidim_lbound_dimension_expression_zero_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_TYPED_LONG_MULTIDIM_LBOUND_DIM_EXPR_ZERO_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_multidim_ubound_dimension_expression_too_high_error() {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_TYPED_LONG_MULTIDIM_UBOUND_DIM_EXPR_TOO_HIGH_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_dynamic_multidim_lbound_dimension_expression_zero_error() {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_TYPED_LONG_DYNAMIC_MULTIDIM_LBOUND_DIM_EXPR_ZERO_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_typed_long_dynamic_multidim_ubound_dimension_expression_too_high_error()
     {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_TYPED_LONG_DYNAMIC_MULTIDIM_UBOUND_DIM_EXPR_TOO_HIGH_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_EXPLICIT_LOWER_BOUND, Variant::from_i16(11));
    }

    #[test]
    fn jit_matches_vm3_array_option_base_one_bounds() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_OPTION_BASE_ONE_BOUNDS, Variant::from_i16(4));
        assert_jit_matches_vm3_contains(JIT_ARRAY_OPTION_BASE_ONE_BOUNDS, Variant::from_i16(9));
    }

    #[test]
    fn jit_matches_vm3_array_bounds_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_BOUNDS_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_redim_expand() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_REDIM_EXPAND, Variant::from_i16(5));
    }

    #[test]
    fn jit_matches_vm3_array_redim_without_preserve_resets() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_REDIM_WITHOUT_PRESERVE_RESETS, Variant::empty());
    }

    #[test]
    fn jit_matches_vm3_array_redim_shrink_bounds_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_REDIM_SHRINK_BOUNDS_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_redim_upper_less_than_lower_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_REDIM_UPPER_LESS_THAN_LOWER_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_redim_preserve_upper_less_than_lower_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_REDIM_PRESERVE_UPPER_LESS_THAN_LOWER_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_redim_negative_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_REDIM_NEGATIVE_LOWER_BOUND,
            Variant::from_i16(17),
        );
    }

    #[test]
    fn jit_matches_vm3_array_redim_dynamic_bound_expression() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_REDIM_DYNAMIC_BOUND_EXPRESSION,
            Variant::from_i16(19),
        );
    }

    #[test]
    fn jit_matches_vm3_array_redim_option_base_one_bounds() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_REDIM_OPTION_BASE_ONE_BOUNDS,
            Variant::from_i32(1),
        );
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_REDIM_OPTION_BASE_ONE_BOUNDS,
            Variant::from_i32(3),
        );
    }

    #[test]
    fn jit_matches_vm3_array_redim_fixed_variant_array_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_REDIM_FIXED_VARIANT_ARRAY_ERROR, 10);
    }

    #[test]
    fn jit_matches_vm3_array_redim_preserve_keeps_values() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_REDIM_PRESERVE_KEEPS_VALUES,
            Variant::from_i16(7),
        );
    }

    #[test]
    fn jit_matches_vm3_array_redim_preserve_unallocated_defaults() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_REDIM_PRESERVE_UNALLOCATED_DEFAULTS,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_array_redim_preserve_explicit_lower_keeps_value() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_REDIM_PRESERVE_EXPLICIT_LOWER_KEEPS_VALUE,
            Variant::from_i16(8),
        );
    }

    #[test]
    fn jit_matches_vm3_array_redim_preserve_shrink_expand_clears_tail() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_REDIM_PRESERVE_SHRINK_EXPAND_CLEARS_TAIL,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_array_redim_preserve_lower_bound_change_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_REDIM_PRESERVE_LOWER_BOUND_CHANGE_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_redim_preserve_fixed_variant_array_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_REDIM_PRESERVE_FIXED_VARIANT_ARRAY_ERROR, 10);
    }

    #[test]
    fn jit_matches_vm3_array_redim_preserve_multidim_last_dimension() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_REDIM_PRESERVE_MULTIDIM_LAST_DIMENSION,
            Variant::from_i16(7),
        );
    }

    #[test]
    fn jit_matches_vm3_array_redim_preserve_illegal_non_last_dimension_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_REDIM_PRESERVE_ILLEGAL_NON_LAST_DIM_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_for_each_array_dynamic_basic() {
        assert_jit_matches_vm3_contains(JIT_FOR_EACH_ARRAY_DYNAMIC_BASIC, Variant::from_i16(8));
    }

    #[test]
    fn jit_matches_vm3_for_each_array_dynamic_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            Variant::from_i16(18),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_array_dynamic_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_array_literal_basic() {
        assert_jit_matches_vm3_contains(JIT_FOR_EACH_ARRAY_LITERAL_BASIC, Variant::from_i16(3));
    }

    #[test]
    fn jit_matches_vm3_for_each_array_literal_empty_skips() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_ARRAY_LITERAL_EMPTY_SKIPS,
            Variant::from_i32(17),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_array_literal_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_ARRAY_LITERAL_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_array_variable_basic() {
        assert_jit_matches_vm3_contains(JIT_FOR_EACH_ARRAY_VARIABLE_BASIC, Variant::from_i16(6));
    }

    #[test]
    fn jit_matches_vm3_for_each_array_variable_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_ARRAY_VARIABLE_EXPLICIT_LOWER_BOUND,
            Variant::from_i16(18),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_array_variable_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_ARRAY_VARIABLE_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_boolean_array_dynamic_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BOOLEAN_ARRAY_DYNAMIC_BASIC,
            Variant::from_bool(true),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_boolean_array_dynamic_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BOOLEAN_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            Variant::from_bool(true),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_boolean_array_dynamic_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BOOLEAN_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_boolean_array_dynamic_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BOOLEAN_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            Variant::from_f64(122_121.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_boolean_array_fixed_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BOOLEAN_ARRAY_FIXED_BASIC,
            Variant::from_bool(true),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_boolean_array_fixed_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BOOLEAN_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            Variant::from_bool(true),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_boolean_array_fixed_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BOOLEAN_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_boolean_array_fixed_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BOOLEAN_ARRAY_FIXED_MULTIDIM_ORDER,
            Variant::from_f64(122_121.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_byte_array_dynamic_basic() {
        assert_jit_matches_vm3_contains(JIT_FOR_EACH_BYTE_ARRAY_DYNAMIC_BASIC, Variant::from_u8(8));
    }

    #[test]
    fn jit_matches_vm3_for_each_byte_array_dynamic_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BYTE_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            Variant::from_u8(9),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_byte_array_dynamic_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BYTE_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_byte_array_dynamic_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BYTE_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_byte_array_fixed_basic() {
        assert_jit_matches_vm3_contains(JIT_FOR_EACH_BYTE_ARRAY_FIXED_BASIC, Variant::from_u8(6));
    }

    #[test]
    fn jit_matches_vm3_for_each_byte_array_fixed_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BYTE_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            Variant::from_u8(9),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_byte_array_fixed_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BYTE_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_byte_array_fixed_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_BYTE_ARRAY_FIXED_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_integer_array_dynamic_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_INTEGER_ARRAY_DYNAMIC_BASIC,
            Variant::from_i16(8),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_integer_array_dynamic_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_INTEGER_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            Variant::from_i16(9),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_integer_array_dynamic_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_INTEGER_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_integer_array_dynamic_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_INTEGER_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_integer_array_fixed_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_INTEGER_ARRAY_FIXED_BASIC,
            Variant::from_i16(6),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_integer_array_fixed_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_INTEGER_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            Variant::from_i16(9),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_integer_array_fixed_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_INTEGER_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_integer_array_fixed_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_INTEGER_ARRAY_FIXED_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_long_array_dynamic_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_BASIC,
            Variant::from_i32(8),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_long_array_dynamic_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            Variant::from_i32(9),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_long_array_dynamic_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_long_array_dynamic_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_long_array_dynamic_3d_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_3D_ORDER,
            Variant::from_f64(12_345_678.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_long_array_dynamic_4d_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_4D_ORDER,
            Variant::from_i32(16),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_long_array_fixed_basic() {
        assert_jit_matches_vm3_contains(JIT_FOR_EACH_LONG_ARRAY_FIXED_BASIC, Variant::from_i32(6));
    }

    #[test]
    fn jit_matches_vm3_for_each_long_array_fixed_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONG_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            Variant::from_i32(9),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_long_array_fixed_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONG_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_long_array_fixed_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONG_ARRAY_FIXED_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_long_array_fixed_3d_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONG_ARRAY_FIXED_3D_ORDER,
            Variant::from_f64(12_345_678.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_long_array_fixed_4d_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONG_ARRAY_FIXED_4D_ORDER,
            Variant::from_i32(16),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_typed_scalar_3d_order_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_TYPED_SCALAR_3D_ORDER_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_typed_scalar_dynamic_3d_order_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_3D_ORDER_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_typed_scalar_multidim_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_TYPED_SCALAR_MULTIDIM_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_typed_scalar_dynamic_multidim_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_MULTIDIM_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_typed_scalar_3d_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_TYPED_SCALAR_3D_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_typed_scalar_dynamic_3d_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_3D_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_typed_scalar_4d_order_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_TYPED_SCALAR_4D_ORDER_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_typed_scalar_dynamic_4d_order_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_4D_ORDER_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_typed_scalar_4d_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_TYPED_SCALAR_4D_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_typed_scalar_dynamic_4d_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_4D_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_longlong_array_dynamic_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONGLONG_ARRAY_DYNAMIC_BASIC,
            Variant::from_i64(5_000_000_008),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_longlong_array_dynamic_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONGLONG_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            Variant::from_i64(5_000_000_009),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_longlong_array_dynamic_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONGLONG_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_longlong_array_dynamic_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONGLONG_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_longlong_array_fixed_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONGLONG_ARRAY_FIXED_BASIC,
            Variant::from_i64(5_000_000_006),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_longlong_array_fixed_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONGLONG_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            Variant::from_i64(5_000_000_009),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_longlong_array_fixed_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONGLONG_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_longlong_array_fixed_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_LONGLONG_ARRAY_FIXED_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_single_array_dynamic_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_SINGLE_ARRAY_DYNAMIC_BASIC,
            Variant::from_f32(6.75),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_single_array_dynamic_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_SINGLE_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            Variant::from_f32(9.75),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_single_array_dynamic_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_SINGLE_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_single_array_dynamic_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_SINGLE_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_single_array_fixed_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_SINGLE_ARRAY_FIXED_BASIC,
            Variant::from_f32(3.75),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_single_array_fixed_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_SINGLE_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            Variant::from_f32(9.75),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_single_array_fixed_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_SINGLE_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_single_array_fixed_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_SINGLE_ARRAY_FIXED_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_double_array_dynamic_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DOUBLE_ARRAY_DYNAMIC_BASIC,
            Variant::from_f64(60.75),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_double_array_dynamic_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DOUBLE_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            Variant::from_f64(90.75),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_double_array_dynamic_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DOUBLE_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_double_array_dynamic_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DOUBLE_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_double_array_fixed_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DOUBLE_ARRAY_FIXED_BASIC,
            Variant::from_f64(30.75),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_double_array_fixed_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DOUBLE_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            Variant::from_f64(90.75),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_double_array_fixed_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DOUBLE_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_double_array_fixed_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DOUBLE_ARRAY_FIXED_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_currency_array_dynamic_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_CURRENCY_ARRAY_DYNAMIC_BASIC,
            Variant::from_currency_scaled_i64(67_500),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_currency_array_dynamic_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_CURRENCY_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            Variant::from_currency_scaled_i64(97_500),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_currency_array_dynamic_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_CURRENCY_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_currency_array_dynamic_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_CURRENCY_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_currency_array_fixed_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_CURRENCY_ARRAY_FIXED_BASIC,
            Variant::from_currency_scaled_i64(37_500),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_currency_array_fixed_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_CURRENCY_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            Variant::from_currency_scaled_i64(97_500),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_currency_array_fixed_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_CURRENCY_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_currency_array_fixed_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_CURRENCY_ARRAY_FIXED_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_date_array_dynamic_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DATE_ARRAY_DYNAMIC_BASIC,
            Variant::from_date_f64(36532.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_date_array_dynamic_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DATE_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            Variant::from_date_f64(36535.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_date_array_dynamic_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DATE_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_date_array_dynamic_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DATE_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_date_array_fixed_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DATE_ARRAY_FIXED_BASIC,
            Variant::from_date_f64(36529.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_date_array_fixed_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DATE_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            Variant::from_date_f64(36535.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_date_array_fixed_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DATE_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_date_array_fixed_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_DATE_ARRAY_FIXED_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_string_array_dynamic_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_BASIC,
            Variant::from_string("zeta"),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_string_array_dynamic_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            Variant::from_string("upper"),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_string_array_dynamic_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_string_array_dynamic_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_string_array_dynamic_4d_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_4D_ORDER,
            Variant::from_i32(16),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_string_array_fixed_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_STRING_ARRAY_FIXED_BASIC,
            Variant::from_string("gamma"),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_string_array_fixed_explicit_lower_bound() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_STRING_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            Variant::from_string("upper"),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_string_array_fixed_item_after_completion() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_STRING_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            Variant::empty(),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_string_array_fixed_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_STRING_ARRAY_FIXED_MULTIDIM_ORDER,
            Variant::from_f64(111_213_212_223.0),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_string_array_fixed_4d_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_STRING_ARRAY_FIXED_4D_ORDER,
            Variant::from_i32(16),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_dynamic_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_BASIC,
            Variant::from_i32(9651),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_dynamic_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(7),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_dynamic_width_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_WIDTH_BUNDLE,
            Variant::from_i32(16542),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_dynamic_multidim_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_MULTIDIM_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(7),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_dynamic_multidim_width_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_MULTIDIM_WIDTH_BUNDLE,
            Variant::from_i32(22530),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_dynamic_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            Variant::from_i32(13141),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_dynamic_3d_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_3D_ORDER,
            Variant::from_i32(28129),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_dynamic_3d_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_3D_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(7),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_dynamic_3d_width_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_3D_WIDTH_BUNDLE,
            Variant::from_i32(48694),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_dynamic_4d_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_4D_ORDER,
            Variant::from_i32(61272),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_dynamic_4d_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_4D_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(7),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_dynamic_4d_width_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_4D_WIDTH_BUNDLE,
            Variant::from_i32(110078),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_fixed_basic() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_BASIC,
            Variant::from_i32(9651),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_fixed_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(7),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_fixed_width_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_WIDTH_BUNDLE,
            Variant::from_i32(16542),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_fixed_multidim_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_MULTIDIM_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(7),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_fixed_multidim_width_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_MULTIDIM_WIDTH_BUNDLE,
            Variant::from_i32(22530),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_fixed_multidim_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_MULTIDIM_ORDER,
            Variant::from_i32(13141),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_fixed_3d_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_3D_ORDER,
            Variant::from_i32(28129),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_fixed_3d_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_3D_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(7),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_fixed_3d_width_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_3D_WIDTH_BUNDLE,
            Variant::from_i32(48694),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_fixed_4d_order() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_4D_ORDER,
            Variant::from_i32(61272),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_fixed_4d_item_after_completion_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_4D_ITEM_AFTER_COMPLETION_BUNDLE,
            Variant::from_i32(7),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_array_fixed_4d_width_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_4D_WIDTH_BUNDLE,
            Variant::from_i32(110078),
        );
    }

    #[test]
    fn jit_matches_vm3_for_each_boolean_scalar_error() {
        assert_jit_matches_vm3_raises(JIT_FOR_EACH_BOOLEAN_SCALAR_ERROR, 13);
    }

    #[test]
    fn jit_matches_vm3_for_each_byte_scalar_error() {
        assert_jit_matches_vm3_raises(JIT_FOR_EACH_BYTE_SCALAR_ERROR, 13);
    }

    #[test]
    fn jit_matches_vm3_for_each_currency_scalar_error() {
        assert_jit_matches_vm3_raises(JIT_FOR_EACH_CURRENCY_SCALAR_ERROR, 13);
    }

    #[test]
    fn jit_matches_vm3_for_each_date_scalar_error() {
        assert_jit_matches_vm3_raises(JIT_FOR_EACH_DATE_SCALAR_ERROR, 13);
    }

    #[test]
    fn jit_matches_vm3_for_each_double_scalar_error() {
        assert_jit_matches_vm3_raises(JIT_FOR_EACH_DOUBLE_SCALAR_ERROR, 13);
    }

    #[test]
    fn jit_matches_vm3_for_each_fixed_string_scalar_error() {
        assert_jit_matches_vm3_raises(JIT_FOR_EACH_FIXED_STRING_SCALAR_ERROR, 13);
    }

    #[test]
    fn jit_matches_vm3_for_each_integer_scalar_error() {
        assert_jit_matches_vm3_raises(JIT_FOR_EACH_INTEGER_SCALAR_ERROR, 13);
    }

    #[test]
    fn jit_matches_vm3_for_each_long_scalar_error() {
        assert_jit_matches_vm3_raises(JIT_FOR_EACH_LONG_SCALAR_ERROR, 13);
    }

    #[test]
    fn jit_matches_vm3_for_each_longlong_scalar_error() {
        assert_jit_matches_vm3_raises(JIT_FOR_EACH_LONGLONG_SCALAR_ERROR, 13);
    }

    #[test]
    fn jit_matches_vm3_for_each_single_scalar_error() {
        assert_jit_matches_vm3_raises(JIT_FOR_EACH_SINGLE_SCALAR_ERROR, 13);
    }

    #[test]
    fn jit_matches_vm3_for_each_string_scalar_error() {
        assert_jit_matches_vm3_raises(JIT_FOR_EACH_STRING_SCALAR_ERROR, 13);
    }

    #[test]
    fn jit_matches_vm3_for_each_variant_scalar_error() {
        assert_jit_matches_vm3_raises(JIT_FOR_EACH_VARIANT_SCALAR_ERROR, 13);
    }

    #[test]
    fn jit_matches_vm3_array_erase_fixed_reset() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_ERASE_FIXED_RESET, Variant::empty());
    }

    #[test]
    fn jit_matches_vm3_array_erase_fixed_long_reset() {
        assert_jit_matches_vm3_contains(JIT_ARRAY_ERASE_FIXED_LONG_RESET, Variant::from_i32(0));
        assert_jit_matches_vm3_contains(JIT_ARRAY_ERASE_FIXED_LONG_RESET, Variant::from_i32(2));
        assert_jit_matches_vm3_contains(JIT_ARRAY_ERASE_FIXED_LONG_RESET, Variant::from_i32(4));
    }

    #[test]
    fn jit_matches_vm3_array_erase_fixed_long_rejects_string_after_reset() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_FIXED_LONG_REJECTS_STRING_AFTER_RESET, 13);
    }

    #[test]
    fn jit_matches_vm3_array_erase_fixed_typed_scalar_reset_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_ERASE_FIXED_TYPED_SCALAR_RESET_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_array_erase_fixed_typed_scalar_multidim_reset_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_ERASE_FIXED_TYPED_SCALAR_MULTIDIM_RESET_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_array_erase_fixed_typed_scalar_3d_reset_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_ERASE_FIXED_TYPED_SCALAR_3D_RESET_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_array_erase_fixed_typed_scalar_4d_reset_bundle() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_ERASE_FIXED_TYPED_SCALAR_4D_RESET_BUNDLE,
            Variant::from_i32(1023),
        );
    }

    #[test]
    fn jit_matches_vm3_array_erase_fixed_bounds_preserved() {
        assert_jit_matches_vm3_contains(
            JIT_ARRAY_ERASE_FIXED_BOUNDS_PRESERVED,
            Variant::from_i32(2424),
        );
        assert_jit_matches_vm3_contains(JIT_ARRAY_ERASE_FIXED_BOUNDS_PRESERVED, Variant::empty());
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_bounds_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_BOUNDS_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_long_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_LONG_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_boolean_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_BOOLEAN_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_byte_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_BYTE_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_integer_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_INTEGER_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_longlong_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_LONGLONG_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_single_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_SINGLE_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_double_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_DOUBLE_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_currency_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_CURRENCY_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_date_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_DATE_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_string_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_STRING_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_boolean_multidim_unallocated_error() {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_ERASE_DYNAMIC_BOOLEAN_MULTIDIM_UNALLOCATED_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_byte_multidim_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_BYTE_MULTIDIM_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_integer_multidim_unallocated_error() {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_ERASE_DYNAMIC_INTEGER_MULTIDIM_UNALLOCATED_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_long_multidim_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_LONG_MULTIDIM_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_longlong_multidim_unallocated_error() {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_ERASE_DYNAMIC_LONGLONG_MULTIDIM_UNALLOCATED_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_single_multidim_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_SINGLE_MULTIDIM_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_double_multidim_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_DOUBLE_MULTIDIM_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_currency_multidim_unallocated_error() {
        assert_jit_matches_vm3_raises(
            JIT_ARRAY_ERASE_DYNAMIC_CURRENCY_MULTIDIM_UNALLOCATED_ERROR,
            9,
        );
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_date_multidim_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_DATE_MULTIDIM_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_string_multidim_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_STRING_MULTIDIM_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_boolean_3d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_BOOLEAN_3D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_byte_3d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_BYTE_3D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_integer_3d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_INTEGER_3D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_long_3d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_LONG_3D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_longlong_3d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_LONGLONG_3D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_single_3d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_SINGLE_3D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_double_3d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_DOUBLE_3D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_currency_3d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_CURRENCY_3D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_date_3d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_DATE_3D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_string_3d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_STRING_3D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_boolean_4d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_BOOLEAN_4D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_byte_4d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_BYTE_4D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_integer_4d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_INTEGER_4D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_long_4d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_LONG_4D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_longlong_4d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_LONGLONG_4D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_single_4d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_SINGLE_4D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_double_4d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_DOUBLE_4D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_currency_4d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_CURRENCY_4D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_date_4d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_DATE_4D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_string_4d_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_STRING_4D_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_lbound_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_LBOUND_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_array_erase_dynamic_ubound_unallocated_error() {
        assert_jit_matches_vm3_raises(JIT_ARRAY_ERASE_DYNAMIC_UBOUND_UNALLOCATED_ERROR, 9);
    }

    #[test]
    fn jit_matches_vm3_builtin_abs_long() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_ABS_LONG);
        let jit = run(Executor::Jit, JIT_BUILTIN_ABS_LONG);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(7))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_builtin_abs_integer() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_ABS_INTEGER, Variant::from_i16(5));
    }

    #[test]
    fn jit_matches_vm3_builtin_abs_integer_min_promotes() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_ABS_INTEGER_MIN, Variant::from_i32(32768));
    }

    #[test]
    fn jit_matches_vm3_builtin_abs_long_min_promotes() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_ABS_LONG_MIN, Variant::from_f64(2147483648.0));
    }

    #[test]
    fn jit_matches_vm3_builtin_abs_bool() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_ABS_BOOL, Variant::from_i16(1));
    }

    #[test]
    fn jit_matches_vm3_builtin_abs_empty() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_ABS_EMPTY, Variant::from_i16(0));
    }

    #[test]
    fn jit_matches_vm3_builtin_abs_null() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_ABS_NULL, Variant::null());
    }

    #[test]
    fn jit_matches_vm3_builtin_abs_double() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_ABS_DOUBLE);
        let jit = run(Executor::Jit, JIT_BUILTIN_ABS_DOUBLE);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(2.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_abs_single() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_ABS_SINGLE);
        let jit = run(Executor::Jit, JIT_BUILTIN_ABS_SINGLE);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f32(1.25))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_abs_currency() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_ABS_CURRENCY);
        let jit = run(Executor::Jit, JIT_BUILTIN_ABS_CURRENCY);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(123_456))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_abs_longlong() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_ABS_LONGLONG);
        let jit = run(Executor::Jit, JIT_BUILTIN_ABS_LONGLONG);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i64(5_000_000_017))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_int_double() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_INT_DOUBLE);
        let jit = run(Executor::Jit, JIT_BUILTIN_INT_DOUBLE);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(-3.0))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_int_long() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_INT_LONG, Variant::from_i32(-5));
    }

    #[test]
    fn jit_matches_vm3_builtin_int_integer() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_INT_INTEGER, Variant::from_i16(-5));
    }

    #[test]
    fn jit_matches_vm3_builtin_int_longlong() {
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_INT_LONGLONG,
            Variant::from_i64(-5_000_000_017),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_int_single() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_INT_SINGLE, Variant::from_f32(-3.0));
    }

    #[test]
    fn jit_matches_vm3_builtin_int_bool() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_INT_BOOL, Variant::from_i16(-1));
    }

    #[test]
    fn jit_matches_vm3_builtin_int_empty() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_INT_EMPTY, Variant::from_i16(0));
    }

    #[test]
    fn jit_matches_vm3_builtin_int_null() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_INT_NULL, Variant::null());
    }

    #[test]
    fn jit_matches_vm3_builtin_fix_double() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_FIX_DOUBLE);
        let jit = run(Executor::Jit, JIT_BUILTIN_FIX_DOUBLE);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(-2.0))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_fix_integer() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_FIX_INTEGER, Variant::from_i16(-5));
    }

    #[test]
    fn jit_matches_vm3_builtin_fix_long() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_FIX_LONG, Variant::from_i32(-5));
    }

    #[test]
    fn jit_matches_vm3_builtin_fix_longlong() {
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_FIX_LONGLONG,
            Variant::from_i64(-5_000_000_017),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_fix_single() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_FIX_SINGLE, Variant::from_f32(-2.0));
    }

    #[test]
    fn jit_matches_vm3_builtin_fix_bool() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_FIX_BOOL, Variant::from_i16(-1));
    }

    #[test]
    fn jit_matches_vm3_builtin_fix_empty() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_FIX_EMPTY, Variant::from_i16(0));
    }

    #[test]
    fn jit_matches_vm3_builtin_fix_null() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_FIX_NULL, Variant::null());
    }

    #[test]
    fn jit_matches_vm3_builtin_int_currency() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_INT_CURRENCY);
        let jit = run(Executor::Jit, JIT_BUILTIN_INT_CURRENCY);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(-60_000))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_fix_currency() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_FIX_CURRENCY);
        let jit = run(Executor::Jit, JIT_BUILTIN_FIX_CURRENCY);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(-50_000))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_int_date() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_INT_DATE, Variant::from_date_f64(43845.0));
    }

    #[test]
    fn jit_matches_vm3_builtin_fix_date() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_FIX_DATE, Variant::from_date_f64(43845.0));
    }

    #[test]
    fn jit_matches_vm3_builtin_sgn_double() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_SGN_DOUBLE);
        let jit = run(Executor::Jit, JIT_BUILTIN_SGN_DOUBLE);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i16(-1))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_sgn_long() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_SGN_LONG);
        let jit = run(Executor::Jit, JIT_BUILTIN_SGN_LONG);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i16(-1))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_sgn_integer() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_SGN_INTEGER, Variant::from_i16(-1));
    }

    #[test]
    fn jit_matches_vm3_builtin_sgn_bool() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_SGN_BOOL, Variant::from_i16(-1));
    }

    #[test]
    fn jit_matches_vm3_builtin_sgn_empty() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_SGN_EMPTY, Variant::from_i16(0));
    }

    #[test]
    fn jit_matches_vm3_builtin_sgn_zero() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_SGN_ZERO, Variant::from_i16(0));
    }

    #[test]
    fn jit_matches_vm3_builtin_sgn_null() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_SGN_NULL, 94);
    }

    #[test]
    fn jit_matches_vm3_builtin_sgn_longlong() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_SGN_LONGLONG, Variant::from_i16(1));
    }

    #[test]
    fn jit_matches_vm3_builtin_sgn_single() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_SGN_SINGLE, Variant::from_i16(-1));
    }

    #[test]
    fn jit_matches_vm3_builtin_sgn_currency() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_SGN_CURRENCY, Variant::from_i16(-1));
    }

    #[test]
    fn jit_matches_vm3_builtin_scalar_conversion_exprs() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_CBOOL_EXPR, Variant::from_bool(true));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_CBYTE_EXPR, Variant::from_u8(14));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_CINT_EXPR, Variant::from_i16(14));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_CLNG_EXPR, Variant::from_i32(42));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_CLNGLNG_EXPR, Variant::from_i64(5_000_000_014));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_CLNGPTR_EXPR, Variant::from_i64(5_000_000_014));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_CSNG_EXPR, Variant::from_f32(1.25));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_CDBL_EXPR, Variant::from_f64(12.0));
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_CCUR_EXPR,
            Variant::from_currency_scaled_i64(123_456),
        );
        assert_jit_matches_vm3_contains(JIT_BUILTIN_CDATE_EXPR, Variant::from_date_f64(36527.0));
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_CDEC_EXPR,
            Variant::from_decimal96(oxvba_runtime::Decimal96::from_parts(10, 0, 0, 0, false)),
        );
        assert_jit_matches_vm3_contains_canon(JIT_BUILTIN_CSTR_EXPR, Canon::Str("42".into()));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_CVAR_EXPR, Variant::from_i32(42));
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_CONVERSION_VARIANT_OPERANDS,
            Variant::from_decimal96(oxvba_runtime::Decimal96::from_parts(10, 0, 0, 0, false)),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_cverr_exprs() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_CVERR_EXPR, Variant::from_error_code(2042));
        assert_jit_matches_vm3_raises(JIT_BUILTIN_CVERR_INVALID, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_result_exprs() {
        assert_jit_matches_vm3_contains_canon(JIT_BUILTIN_HEX_EXPR, Canon::Str("FF".into()));
        assert_jit_matches_vm3_contains_canon(JIT_BUILTIN_OCT_EXPR, Canon::Str("11".into()));
        assert_jit_matches_vm3_contains_canon(JIT_BUILTIN_STR_EXPR, Canon::Str(" 42".into()));
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_HEX_OCT_NEGATIVE_WIDTH_EXPRS,
            Canon::Str("FFFF".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_HEX_OCT_NEGATIVE_WIDTH_EXPRS,
            Canon::Str("FFFFFFFF".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_HEX_OCT_NEGATIVE_WIDTH_EXPRS,
            Canon::Str("FFFFFFFFFFFFFFFF".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_HEX_OCT_NEGATIVE_WIDTH_EXPRS,
            Canon::Str("177777".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_HEX_OCT_NEGATIVE_WIDTH_EXPRS,
            Canon::Str("37777777777".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_HEX_OCT_NEGATIVE_WIDTH_EXPRS,
            Canon::Str("1777777777777777777777".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_result_destinations() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_RESULT_DESTINATIONS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_RESULT_DESTINATIONS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                Canon::Str("42".into()),
                Canon::Str("FF".into()),
                Canon::Str("A".into()),
                Canon::Str("   ".into()),
                Canon::Str("12".into()),
                Canon::Str("234".into()),
                Canon::Str("199199".into()),
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_typed_aliases() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_TYPED_ALIASES);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_TYPED_ALIASES);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                Canon::Str("ab".into()),
                Canon::Str("AB".into()),
                Canon::Str("x".into()),
                Canon::Str("12".into()),
                Canon::Str("45".into()),
                Canon::Str("234".into()),
                Canon::Str("A".into()),
                Canon::Str("  ".into()),
                Canon::Str("AA".into()),
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_typed_alias_null_error() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STRING_TYPED_ALIAS_NULL_ERROR, 94);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_typed_alias_destinations() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_TYPED_ALIAS_DESTINATIONS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_TYPED_ALIAS_DESTINATIONS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                Canon::Str("ab".into()),
                Canon::Str("AB".into()),
                Canon::Str("x".into()),
                Canon::Str("12".into()),
                Canon::Str("45".into()),
                Canon::Str("1".into()),
                Canon::Str("5".into()),
                Canon::Str("234".into()),
                Canon::Str("A".into()),
                Canon::Str("  ".into()),
                Canon::Str("AA".into()),
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_string_destination_null_variant() {
        assert_jit_matches_vm3_raises(JIT_STRING_DESTINATION_NULL_VARIANT, 94);
    }

    #[test]
    fn jit_matches_vm3_fixed_string_local_pad_truncate() {
        let vm3 = run(Executor::Vm3, JIT_FIXED_STRING_LOCAL_PAD_TRUNCATE);
        let jit = run(Executor::Jit, JIT_FIXED_STRING_LOCAL_PAD_TRUNCATE);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                Canon::Str("   ".into()),
                Canon::Str("ab ".into()),
                Canon::Str("abc".into()),
                Canon::Str("   ".into()),
                Canon::Str("ab ".into()),
                Canon::Str("abc".into())
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_fixed_string_local_null_error() {
        assert_jit_matches_vm3_raises(JIT_FIXED_STRING_LOCAL_NULL_ERROR, 94);
    }

    #[test]
    fn jit_matches_vm3_fixed_string_global_pad_truncate() {
        let vm3 = run(Executor::Vm3, JIT_FIXED_STRING_GLOBAL_PAD_TRUNCATE);
        let jit = run(Executor::Jit, JIT_FIXED_STRING_GLOBAL_PAD_TRUNCATE);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                Canon::Str("   ".into()),
                Canon::Str("ab ".into()),
                Canon::Str("abc".into()),
                Canon::Str("   ".into()),
                Canon::Str("ab ".into()),
                Canon::Str("abc".into())
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_fixed_string_global_null_error() {
        assert_jit_matches_vm3_raises(JIT_FIXED_STRING_GLOBAL_NULL_ERROR, 94);
    }

    #[test]
    fn jit_matches_vm3_builtin_scalar_result_destinations() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_SCALAR_RESULT_DESTINATIONS);
        let jit = run(Executor::Jit, JIT_BUILTIN_SCALAR_RESULT_DESTINATIONS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                canon(&Variant::from_i32(4)),
                canon(&Variant::from_i32(8)),
                canon(&Variant::from_i32(2)),
                canon(&Variant::from_f64(1234.0)),
                canon(&Variant::from_bool(true)),
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_scalar_result_destination_families() {
        let vm3 = run(
            Executor::Vm3,
            JIT_BUILTIN_SCALAR_RESULT_DESTINATION_FAMILIES,
        );
        let jit = run(
            Executor::Jit,
            JIT_BUILTIN_SCALAR_RESULT_DESTINATION_FAMILIES,
        );
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let d_value = 43846.5515625_f64;
        let time_value = d_value - 43846.0;
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                canon(&Variant::from_f64(2.2)),
                canon(&Variant::from_i32(2020)),
                canon(&Variant::from_date_f64(43846.0)),
                canon(&Variant::from_date_f64(time_value)),
                canon(&Variant::from_i32(65_536)),
                canon(&Variant::from_i32(8_388_608)),
                canon(&Variant::from_bool(true)),
                canon(&Variant::from_f64(2.25)),
                canon(&Variant::from_i32(1)),
                canon(&Variant::from_date_f64(d_value)),
                canon(&Variant::from_i32(0)),
                canon(&Variant::from_i32(0)),
                canon(&Variant::from_i32(1)),
                canon(&Variant::from_i32(1)),
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_scalar_result_destination_coercions() {
        let vm3 = run(
            Executor::Vm3,
            JIT_BUILTIN_SCALAR_RESULT_DESTINATION_COERCIONS,
        );
        let jit = run(
            Executor::Jit,
            JIT_BUILTIN_SCALAR_RESULT_DESTINATION_COERCIONS,
        );
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                canon(&Variant::from_u8(4)),
                canon(&Variant::from_i16(2)),
                canon(&Variant::from_f32(8.0)),
                canon(&Variant::from_f64(4.0)),
                canon(&Variant::from_currency_scaled_i64(123_456)),
                canon(&Variant::from_bool(true)),
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_scalar_result_destination_coercion_overflow() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_SCALAR_RESULT_DESTINATION_COERCION_OVERFLOW, 6);
    }

    #[test]
    fn jit_matches_vm3_builtin_scalar_result_destination_null_error() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_SCALAR_RESULT_DESTINATION_NULL_ERROR, 94);
    }

    #[test]
    fn jit_matches_vm3_builtin_math_unary_exprs() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_MATH_UNARY_EXPRS, Variant::from_f64(3.0));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_MATH_UNARY_EXPRS, Variant::from_f64(0.0));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_MATH_UNARY_EXPRS, Variant::from_f64(1.0));
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_MATH_UNARY_EXPRS,
            Variant::from_f64(1.0_f64.atan()),
        );
        assert_jit_matches_vm3_raises(JIT_BUILTIN_SQR_INVALID, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_LOG_INVALID, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_EXP_OVERFLOW, 6);
    }

    #[test]
    fn jit_matches_vm3_builtin_round_expr() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_ROUND_EXPR, Variant::from_f64(2.0));
    }

    #[test]
    fn jit_matches_vm3_builtin_round_digits_expr() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_ROUND_DIGITS_EXPR, Variant::from_f64(2.2));
        assert_jit_matches_vm3_raises(JIT_BUILTIN_ROUND_NEGATIVE_DIGITS, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_date_part_exprs() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_DATE_PART_EXPRS, Variant::from_i32(2020));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_DATE_PART_EXPRS, Variant::from_i32(1));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_DATE_PART_EXPRS, Variant::from_i32(16));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_DATE_PART_EXPRS, Variant::from_i32(13));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_DATE_PART_EXPRS, Variant::from_i32(14));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_DATE_PART_EXPRS, Variant::from_i32(15));
    }

    #[test]
    fn jit_matches_vm3_builtin_information_exprs() {
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_INFORMATION_EXPRS,
            Variant::from_i32(VarType::Date as i32),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_INFORMATION_EXPRS,
            Canon::Str("Date".into()),
        );
        assert_jit_matches_vm3_contains(JIT_BUILTIN_INFORMATION_EXPRS, Variant::from_bool(true));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_INFORMATION_EXPRS, Variant::from_bool(false));
    }

    #[test]
    fn jit_matches_vm3_stdlib_variant_predicates() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_VARIANT_PREDICATES, Variant::from_bool(true));
        assert_jit_matches_vm3_contains(JIT_STDLIB_VARIANT_PREDICATES, Variant::from_bool(false));
    }

    #[test]
    fn jit_matches_vm3_coercion_null_empty_error_predicates() {
        assert_jit_matches_vm3_contains(
            JIT_COERCION_NULL_EMPTY_ERROR_PREDICATES,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_COERCION_NULL_EMPTY_ERROR_PREDICATES,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_introspection_vartype_isnumeric_tags() {
        assert_jit_matches_vm3_contains(
            JIT_INTROSPECTION_VARTYPE_ISNUMERIC_TAGS,
            Variant::from_i32(8),
        );
        assert_jit_matches_vm3_contains(
            JIT_INTROSPECTION_VARTYPE_ISNUMERIC_TAGS,
            Variant::from_i32(1),
        );
        assert_jit_matches_vm3_contains(
            JIT_INTROSPECTION_VARTYPE_ISNUMERIC_TAGS,
            Variant::from_i32(10),
        );
        assert_jit_matches_vm3_contains(
            JIT_INTROSPECTION_VARTYPE_ISNUMERIC_TAGS,
            Variant::from_i32(2),
        );
        assert_jit_matches_vm3_contains(
            JIT_INTROSPECTION_VARTYPE_ISNUMERIC_TAGS,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_INTROSPECTION_VARTYPE_ISNUMERIC_TAGS,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_string_vbnullstring_predicates() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_VBNULLSTRING_PREDICATES,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains(
            JIT_STRING_VBNULLSTRING_PREDICATES,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_isarray_dynamic_array_exprs() {
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_ISARRAY_DYNAMIC_ARRAY_EXPRS,
            Variant::from_i32(7),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_isarray_variant_carrier_exprs() {
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_ISARRAY_VARIANT_CARRIER_EXPRS,
            Variant::from_i32(2),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_date_value_time_value_exprs() {
        let time_value = 43846.5515625_f64 - 43846.0;
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_DATE_VALUE_TIME_VALUE_EXPRS,
            Variant::from_date_f64(43846.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_DATE_VALUE_TIME_VALUE_EXPRS,
            Variant::from_date_f64(time_value),
        );
    }

    #[test]
    fn jit_matches_vm3_stdlib_date_string_policy() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_DATE_STRING_POLICY, Variant::from_i32(2000));
        assert_jit_matches_vm3_contains(JIT_STDLIB_DATE_STRING_POLICY, Variant::from_i32(1));
        assert_jit_matches_vm3_contains(JIT_STDLIB_DATE_STRING_POLICY, Variant::from_bool(true));
        assert_jit_matches_vm3_contains(JIT_STDLIB_DATE_STRING_POLICY, Variant::from_bool(false));
    }

    #[test]
    fn jit_matches_vm3_stdlib_datetime_expansion() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_DATETIME_EXPANSION, Variant::from_i32(2024));
        assert_jit_matches_vm3_contains(JIT_STDLIB_DATETIME_EXPANSION, Variant::from_i32(2));
        assert_jit_matches_vm3_contains(JIT_STDLIB_DATETIME_EXPANSION, Variant::from_i32(3));
        assert_jit_matches_vm3_contains(JIT_STDLIB_DATETIME_EXPANSION, Variant::from_i32(7));
    }

    #[test]
    fn jit_matches_vm3_stdlib_date_serial_value() {
        assert_jit_matches_vm3_contains(
            JIT_STDLIB_DATE_SERIAL_VALUE,
            Variant::from_date_f64(46081.0),
        );
    }

    #[test]
    fn jit_matches_vm3_stdlib_time_serial_value() {
        assert_jit_matches_vm3_contains(
            JIT_STDLIB_TIME_SERIAL_VALUE,
            Variant::from_date_f64((3600.0 + 2.0 * 60.0 + 3.0) / 86400.0),
        );
    }

    #[test]
    fn jit_matches_vm3_stdlib_date_add_diff_interval_error() {
        assert_jit_matches_vm3_raises(JIT_STDLIB_DATE_ADD_DIFF, 5);
    }

    #[test]
    fn jit_matches_vm3_stdlib_len_basic() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_LEN_BASIC, Variant::from_i32(4));
    }

    #[test]
    fn jit_matches_vm3_stdlib_slice_ops() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_SLICE_OPS, Variant::from_string("12"));
        assert_jit_matches_vm3_contains(JIT_STDLIB_SLICE_OPS, Variant::from_string("45"));
        assert_jit_matches_vm3_contains(JIT_STDLIB_SLICE_OPS, Variant::from_string("234"));
    }

    #[test]
    fn jit_matches_vm3_stdlib_instr_case_ops() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_INSTR_CASE_OPS, Variant::from_i32(3));
        assert_jit_matches_vm3_contains(JIT_STDLIB_INSTR_CASE_OPS, Variant::from_string("789"));
        assert_jit_matches_vm3_contains(JIT_STDLIB_INSTR_CASE_OPS, Variant::from_string("654"));
    }

    #[test]
    fn jit_matches_vm3_stdlib_advanced_instrrev_like() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_ADVANCED_INSTRREV_LIKE, Variant::from_i32(4));
        assert_jit_matches_vm3_contains(JIT_STDLIB_ADVANCED_INSTRREV_LIKE, Variant::from_i16(1));
    }

    #[test]
    fn jit_matches_vm3_stdlib_advanced_replace_trim() {
        assert_jit_matches_vm3_contains(
            JIT_STDLIB_ADVANCED_REPLACE_TRIM,
            Variant::from_string("16745"),
        );
        assert_jit_matches_vm3_contains(
            JIT_STDLIB_ADVANCED_REPLACE_TRIM,
            Variant::from_string("456"),
        );
        assert_jit_matches_vm3_contains(
            JIT_STDLIB_ADVANCED_REPLACE_TRIM,
            Variant::from_string("321"),
        );
    }

    #[test]
    fn jit_matches_vm3_stdlib_advanced_strcomp() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_ADVANCED_STRCOMP, Variant::from_i32(-1));
        assert_jit_matches_vm3_contains(JIT_STDLIB_ADVANCED_STRCOMP, Variant::from_i32(0));
    }

    #[test]
    fn jit_matches_vm3_stdlib_advanced_split_join_error() {
        assert_jit_matches_vm3_raises(JIT_STDLIB_ADVANCED_SPLIT_JOIN, 13);
    }

    #[test]
    fn jit_matches_vm3_stdlib_string_expansion_core() {
        assert_jit_matches_vm3_contains(
            JIT_STDLIB_STRING_EXPANSION_CORE,
            Variant::from_string("    "),
        );
        assert_jit_matches_vm3_contains(
            JIT_STDLIB_STRING_EXPANSION_CORE,
            Variant::from_string("AAA"),
        );
        assert_jit_matches_vm3_contains(
            JIT_STDLIB_STRING_EXPANSION_CORE,
            Variant::from_string("B"),
        );
        assert_jit_matches_vm3_contains(JIT_STDLIB_STRING_EXPANSION_CORE, Variant::from_i32(66));
        assert_jit_matches_vm3_contains(
            JIT_STDLIB_STRING_EXPANSION_CORE,
            Variant::from_string("777"),
        );
    }

    #[test]
    fn jit_matches_vm3_stdlib_format_core() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_FORMAT_CORE, Variant::from_i32(5));
        assert_jit_matches_vm3_contains(JIT_STDLIB_FORMAT_CORE, Variant::from_i32(3));
    }

    #[test]
    fn jit_matches_vm3_stdlib_financial_zero_rate() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_FINANCIAL_ZERO_RATE, Variant::from_f64(-11.0));
        assert_jit_matches_vm3_contains(JIT_STDLIB_FINANCIAL_ZERO_RATE, Variant::from_f64(-3.0));
    }

    #[test]
    fn jit_matches_vm3_financial_algorithm_rate_nper_subset() {
        assert_jit_matches_vm3_contains(
            JIT_FINANCIAL_ALGORITHM_RATE_NPER_SUBSET,
            Variant::from_f64(0.02922854076913337),
        );
        assert_jit_matches_vm3_contains(
            JIT_FINANCIAL_ALGORITHM_RATE_NPER_SUBSET,
            Variant::from_f64(10.0),
        );
    }

    #[test]
    fn jit_matches_vm3_stdlib_rnd_isolated() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_RND_ISOLATED, Variant::from_f64(33.0));
        assert_jit_matches_vm3_contains(JIT_STDLIB_RND_ISOLATED, Variant::from_f64(6.0));
    }

    #[test]
    fn jit_matches_vm3_stdlib_numeric_expansion() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_NUMERIC_EXPANSION, Variant::from_string("1F"));
        assert_jit_matches_vm3_contains(JIT_STDLIB_NUMERIC_EXPANSION, Variant::from_string("21"));
        assert_jit_matches_vm3_contains(
            JIT_STDLIB_NUMERIC_EXPANSION,
            Variant::from_f64(1.460139105621001),
        );
        assert_jit_matches_vm3_contains(
            JIT_STDLIB_NUMERIC_EXPANSION,
            Variant::from_f64(-225.95084645419513),
        );
    }

    #[test]
    fn jit_matches_vm3_conversion_cint_basic() {
        assert_jit_matches_vm3_contains(JIT_CONVERSION_CINT_BASIC, Variant::from_i16(5));
    }

    #[test]
    fn jit_matches_vm3_stdlib_error_cverr_identity() {
        assert_jit_matches_vm3_contains(
            JIT_STDLIB_ERROR_CVERR_IDENTITY,
            Variant::from_error_code(17),
        );
    }

    #[test]
    fn jit_matches_vm3_stdlib_error_err_raise_fail() {
        assert_jit_matches_vm3_raises(JIT_STDLIB_ERROR_ERR_RAISE_FAIL, 9);
    }

    #[test]
    fn jit_matches_vm3_stdlib_error_err_raise_resume() {
        assert_jit_matches_vm3_contains(JIT_STDLIB_ERROR_ERR_RAISE_RESUME, Variant::from_i32(11));
    }

    #[test]
    fn jit_matches_vm3_on_error_resume_next() {
        assert_jit_matches_vm3_contains(JIT_ON_ERROR_RESUME_NEXT, Variant::from_i32(5));
    }

    #[test]
    fn jit_matches_vm3_on_error_resume_continue() {
        assert_jit_matches_vm3_contains(JIT_ON_ERROR_RESUME_CONTINUE, Variant::from_i16(2));
    }

    #[test]
    fn jit_matches_vm3_on_error_default_fail() {
        assert_jit_matches_vm3_raises(JIT_ON_ERROR_DEFAULT_FAIL, 9);
    }

    #[test]
    fn jit_matches_vm3_on_error_goto_zero_fail() {
        assert_jit_matches_vm3_raises(JIT_ON_ERROR_GOTO_ZERO_FAIL, 3);
    }

    #[test]
    fn jit_matches_vm3_resume_next_statement_ok() {
        assert_jit_matches_vm3_contains(JIT_RESUME_NEXT_STATEMENT_OK, Variant::from_i16(1));
    }

    #[test]
    fn jit_matches_vm3_err_resume_next_clears() {
        assert_jit_matches_vm3_contains(JIT_ERR_RESUME_NEXT_CLEARS, Variant::from_i32(20));
    }

    #[test]
    fn jit_matches_vm3_resume_statement_basic() {
        assert_jit_matches_vm3_contains(JIT_RESUME_STATEMENT_BASIC, Variant::from_i16(2));
    }

    #[test]
    fn jit_matches_vm3_resume_label_basic() {
        assert_jit_matches_vm3_contains(JIT_RESUME_LABEL_BASIC, Variant::from_i32(6));
    }

    #[test]
    fn jit_matches_vm3_on_error_goto_label_resume() {
        assert_jit_matches_vm3_contains(JIT_ON_ERROR_GOTO_LABEL_RESUME, Variant::from_i16(100));
    }

    #[test]
    fn jit_matches_vm3_error_goto_label_resume_next() {
        assert_jit_matches_vm3_contains(JIT_ERROR_GOTO_LABEL_RESUME_NEXT, Variant::from_i16(20));
        assert_jit_matches_vm3_contains(JIT_ERROR_GOTO_LABEL_RESUME_NEXT, Variant::from_i32(42));
    }

    #[test]
    fn jit_matches_vm3_err_clear_basic() {
        assert_jit_matches_vm3_contains(JIT_ERR_CLEAR_BASIC, Variant::from_i32(0));
    }

    #[test]
    fn jit_matches_vm3_error_raise_custom_clear_cycle() {
        assert_jit_matches_vm3_contains(JIT_ERROR_RAISE_CUSTOM_CLEAR_CYCLE, Variant::from_i32(100));
        assert_jit_matches_vm3_contains(JIT_ERROR_RAISE_CUSTOM_CLEAR_CYCLE, Variant::from_i32(0));
        assert_jit_matches_vm3_contains(JIT_ERROR_RAISE_CUSTOM_CLEAR_CYCLE, Variant::from_i32(200));
    }

    #[test]
    fn jit_matches_vm3_err_proc_call_boundary_clears() {
        assert_jit_matches_vm3_contains(JIT_ERR_PROC_CALL_BOUNDARY_CLEARS, Variant::from_i32(7));
    }

    #[test]
    fn jit_matches_vm3_err_surface_fields_subset() {
        assert_jit_matches_vm3_contains(JIT_ERR_SURFACE_FIELDS_SUBSET, Variant::from_i32(9));
        assert_jit_matches_vm3_contains(
            JIT_ERR_SURFACE_FIELDS_SUBSET,
            Variant::from_string("Subscript out of range"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ERR_SURFACE_FIELDS_SUBSET,
            Variant::from_string("Main"),
        );
        assert_jit_matches_vm3_contains(
            JIT_ERR_SURFACE_FIELDS_SUBSET,
            Variant::from_i32(1_000_009),
        );
        assert_jit_matches_vm3_contains(
            JIT_ERR_SURFACE_FIELDS_SUBSET,
            Variant::from_string(
                "C:\\Program Files\\Common Files\\Microsoft Shared\\VBA\\VBA7.1\\1033\\VbLR6.chm",
            ),
        );
    }

    #[test]
    fn jit_matches_vm3_err_clear_full_surface_reset() {
        assert_jit_matches_vm3_contains(JIT_ERR_CLEAR_FULL_SURFACE_RESET, Variant::from_i32(0));
        assert_jit_matches_vm3_contains(JIT_ERR_CLEAR_FULL_SURFACE_RESET, Variant::from_string(""));
    }

    #[test]
    fn jit_matches_vm3_conversion_extended_scalar_subset() {
        assert_jit_matches_vm3_contains(
            JIT_CONVERSION_EXTENDED_SCALAR_SUBSET,
            Variant::from_f32(7.0),
        );
        assert_jit_matches_vm3_contains(JIT_CONVERSION_EXTENDED_SCALAR_SUBSET, Variant::from_u8(8));
        assert_jit_matches_vm3_contains(
            JIT_CONVERSION_EXTENDED_SCALAR_SUBSET,
            Variant::from_currency_scaled_i64(90_000),
        );
        assert_jit_matches_vm3_contains(
            JIT_CONVERSION_EXTENDED_SCALAR_SUBSET,
            Variant::from_decimal96(oxvba_runtime::Decimal96::from_parts(10, 0, 0, 0, false)),
        );
    }

    #[test]
    fn jit_matches_vm3_stdlib_math_primitives_round_error() {
        assert_jit_matches_vm3_raises(JIT_STDLIB_MATH_PRIMITIVES, 5);
    }

    #[test]
    fn jit_matches_vm3_stdlib_math_transcendental_identity() {
        let vm3 = run(Executor::Vm3, JIT_STDLIB_MATH_TRANSCENDENTAL_IDENTITY);
        let jit = run(Executor::Jit, JIT_STDLIB_MATH_TRANSCENDENTAL_IDENTITY);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                canon(&Variant::from_f64(0.0)),
                canon(&Variant::from_f64(1.0)),
                canon(&Variant::from_f64(0.0)),
                canon(&Variant::from_f64(1.0)),
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_conversion_val_str_subset() {
        assert_jit_matches_vm3_contains(JIT_CONVERSION_VAL_STR_SUBSET, Variant::from_f64(9.0));
    }

    #[test]
    fn jit_matches_vm3_conversion_clng_cint_chain() {
        assert_jit_matches_vm3_contains(JIT_CONVERSION_CLNG_CINT_CHAIN, Variant::from_i16(7));
        assert_jit_matches_vm3_contains(JIT_CONVERSION_CLNG_CINT_CHAIN, Variant::from_i32(10));
        assert_jit_matches_vm3_contains(JIT_CONVERSION_CLNG_CINT_CHAIN, Variant::from_i16(8));
    }

    #[test]
    fn jit_matches_vm3_conversion_nested_clng_cint() {
        assert_jit_matches_vm3_contains(JIT_CONVERSION_NESTED_CLNG_CINT, Variant::from_i32(7));
    }

    #[test]
    fn jit_matches_vm3_builtin_date_serial_time_serial_exprs() {
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_DATE_SERIAL_TIME_SERIAL_EXPRS,
            Variant::from_date_f64(43846.0),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_DATE_SERIAL_TIME_SERIAL_EXPRS,
            Variant::from_date_f64((13.0 * 3600.0 + 14.0 * 60.0 + 15.0) / 86400.0),
        );
        assert_jit_matches_vm3_raises(JIT_BUILTIN_DATE_SERIAL_RANGE_ERROR, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_rgb_qbcolor_exprs() {
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_RGB_QBCOLOR_EXPRS,
            Variant::from_i32(16_777_215),
        );
        assert_jit_matches_vm3_contains(JIT_BUILTIN_RGB_QBCOLOR_EXPRS, Variant::from_i32(255));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_RGB_COMPONENT_EXPRS, Variant::from_i32(65_536));
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_RGB_COMPONENT_EXPRS,
            Variant::from_i32(8_421_504),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_QBCOLOR_PALETTE_EXPRS,
            Variant::from_i32(8_388_608),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_QBCOLOR_PALETTE_EXPRS,
            Variant::from_i32(12_632_256),
        );
        assert_jit_matches_vm3_contains(JIT_BUILTIN_QBCOLOR_PALETTE_EXPRS, Variant::from_i32(255));
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_QBCOLOR_PALETTE_EXPRS,
            Variant::from_i32(16_777_215),
        );
        assert_jit_matches_vm3_raises(JIT_BUILTIN_QBCOLOR_OUT_OF_RANGE, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_error_text_expr() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_ERROR_TEXT_EXPR,
            Canon::Str("Division by zero".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_ERROR_TEXT_UNKNOWN_EXPR,
            Canon::Str("Application-defined or object-defined error".into()),
        );
        assert_jit_matches_vm3_raises(JIT_BUILTIN_ERROR_TEXT_INVALID_EXPR, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_error_text_result_destinations() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_ERROR_TEXT_RESULT_DESTINATIONS);
        let jit = run(Executor::Jit, JIT_BUILTIN_ERROR_TEXT_RESULT_DESTINATIONS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                Canon::Str("Division by zero".into()),
                Canon::Str("Application-defined or object-defined error".into()),
                canon(&Variant::from_i32(11)),
                canon(&Variant::from_i32(12345)),
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_error_text_alias_destinations() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_ERROR_TEXT_ALIAS_DESTINATIONS);
        let jit = run(Executor::Jit, JIT_BUILTIN_ERROR_TEXT_ALIAS_DESTINATIONS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                Canon::Str("Division by zero".into()),
                Canon::Str("Application-defined or object-defined error".into()),
                canon(&Variant::from_i32(11)),
                canon(&Variant::from_i32(12345)),
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_error_text_destination_invalid() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_ERROR_TEXT_RESULT_DESTINATION_INVALID, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_ERROR_TEXT_ALIAS_DESTINATION_INVALID, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_len_variant_expr() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_LEN_VARIANT_EXPR, Variant::from_i32(4));
    }

    #[test]
    fn jit_matches_vm3_builtin_lenb_variant_expr() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_LENB_VARIANT_EXPR, Variant::from_i32(8));
    }

    #[test]
    fn jit_matches_vm3_builtin_chrw_ascw_variant_exprs() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_CHRW_ASCW_VARIANT_EXPRS,
            Canon::Str("A".into()),
        );
        assert_jit_matches_vm3_contains(JIT_BUILTIN_CHRW_ASCW_VARIANT_EXPRS, Variant::from_i32(65));
    }

    #[test]
    fn jit_matches_vm3_builtin_space_expr() {
        assert_jit_matches_vm3_contains_canon(JIT_BUILTIN_SPACE_EXPR, Canon::Str("   ".into()));
    }

    #[test]
    fn jit_matches_vm3_builtin_space_negative_count() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_SPACE_NEGATIVE_COUNT, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_case_variant_exprs() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_CASE_VARIANT_EXPRS,
            Canon::Str("a".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_CASE_VARIANT_EXPRS,
            Canon::Str("A".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_val_variant_expr() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_VAL_VARIANT_EXPR, Variant::from_f64(1234.0));
    }

    #[test]
    fn jit_matches_vm3_builtin_trim_variant_exprs() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_TRIM_VARIANT_EXPRS,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_str_reverse_variant_expr() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STR_REVERSE_VARIANT_EXPR,
            Canon::Str("4321".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_repeat_charcode_expr() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_REPEAT_CHARCODE_EXPR,
            Canon::Str("AAA".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_REPEAT_CHARCODE_WRAP_EXPR,
            Canon::Str("BB".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_repeat_negative_count() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STRING_REPEAT_NEGATIVE_COUNT, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_left_right_variant_exprs() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFT_RIGHT_VARIANT_EXPRS,
            Canon::Str("12".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFT_RIGHT_VARIANT_EXPRS,
            Canon::Str("45".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_left_right_variant_count_edges() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFT_RIGHT_VARIANT_COUNT_EDGES,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFT_RIGHT_VARIANT_COUNT_EDGES,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_left_right_variant_complement_count_edges() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFT_RIGHT_VARIANT_COMPLEMENT_COUNT_EDGES,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFT_RIGHT_VARIANT_COMPLEMENT_COUNT_EDGES,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_left_right_variant_unit_count() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFT_RIGHT_VARIANT_UNIT_COUNT,
            Canon::Str("1".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFT_RIGHT_VARIANT_UNIT_COUNT,
            Canon::Str("5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_left_right_variant_exact_source_count() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFT_RIGHT_VARIANT_EXACT_SOURCE_COUNT,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFT_RIGHT_VARIANT_EXACT_SOURCE_COUNT,
            Canon::Str("98765".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_left_right_negative_count() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_LEFT_NEGATIVE_COUNT, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_RIGHT_NEGATIVE_COUNT, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_literal_left_right_negative_count() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STRING_LITERAL_LEFT_NEGATIVE_COUNT, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STRING_LITERAL_RIGHT_NEGATIVE_COUNT, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_static_string_left_right_negative_count() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STATIC_STRING_LEFT_NEGATIVE_COUNT, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STATIC_STRING_RIGHT_NEGATIVE_COUNT, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_leftb_rightb_variant_exprs() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_EXPRS,
            Canon::Str("12".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_EXPRS,
            Canon::Str("45".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_leftb_rightb_variant_unit_code_unit_byte_count() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_UNIT_CODE_UNIT_BYTE_COUNT,
            Canon::Str("1".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_UNIT_CODE_UNIT_BYTE_COUNT,
            Canon::Str("5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_leftb_rightb_variant_three_code_unit_byte_count() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_THREE_CODE_UNIT_BYTE_COUNT,
            Canon::Str("123".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_THREE_CODE_UNIT_BYTE_COUNT,
            Canon::Str("345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_leftb_rightb_variant_odd_byte_exprs() {
        let vm3 = run(
            Executor::Vm3,
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_ODD_BYTE_EXPRS,
        );
        let jit = run(
            Executor::Jit,
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_ODD_BYTE_EXPRS,
        );
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                canon(&Variant::from_i32(0)),
                canon(&Variant::from_i32(1)),
                canon(&Variant::from_i32(1)),
                canon(&Variant::from_i32(3)),
                canon(&Variant::from_i32(65)),
                canon(&Variant::from_i32(0)),
                canon(&Variant::from_i32(1)),
                canon(&Variant::from_i32(1)),
                canon(&Variant::from_i32(3)),
                canon(&Variant::from_i32(17152)),
                Canon::Str("ABC".into()),
                Canon::Str(String::new()),
                Canon::Str("A".into()),
                Canon::Str(String::new()),
                Canon::Str(String::from_utf16_lossy(&[0x4300])),
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_leftb_rightb_variant_byte_count_edges() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_BYTE_COUNT_EDGES,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_BYTE_COUNT_EDGES,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_leftb_rightb_variant_complement_byte_count_edges() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_COMPLEMENT_BYTE_COUNT_EDGES,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_COMPLEMENT_BYTE_COUNT_EDGES,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_leftb_rightb_variant_exact_byte_source_count() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_EXACT_BYTE_SOURCE_COUNT,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_EXACT_BYTE_SOURCE_COUNT,
            Canon::Str("98765".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_leftb_rightb_negative_count() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_LEFTB_NEGATIVE_COUNT, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_RIGHTB_NEGATIVE_COUNT, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_literal_leftb_rightb_negative_count() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STRING_LITERAL_LEFTB_NEGATIVE_COUNT, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STRING_LITERAL_RIGHTB_NEGATIVE_COUNT, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_static_string_leftb_rightb_negative_count() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STATIC_STRING_LEFTB_NEGATIVE_COUNT, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STATIC_STRING_RIGHTB_NEGATIVE_COUNT, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_instr_instrrev_variant_exprs() {
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_INSTR_INSTRREV_VARIANT_EXPRS,
            Variant::from_i32(2),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_INSTR_INSTRREV_VARIANT_EXPRS,
            Variant::from_i32(5),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_strcomp_variant_exprs() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_STRCOMP_VARIANT_EXPRS, Variant::from_i32(0));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_STRCOMP_VARIANT_EXPRS, Variant::from_i32(-1));
    }

    #[test]
    fn jit_matches_vm3_builtin_replace_variant_expr() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_REPLACE_VARIANT_EXPR,
            Canon::Str("199199".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_like_variant_expr() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_LIKE_VARIANT_EXPR, Variant::from_bool(true));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_LIKE_VARIANT_EXPR, Variant::from_bool(false));
    }

    #[test]
    fn jit_matches_vm3_builtin_string_literal_variant_args() {
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS,
            Variant::from_i32(4),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS,
            Canon::Str("AB".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS,
            Variant::from_f64(1234.0),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS,
            Canon::Str("4321".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS,
            Canon::Str("12".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS,
            Canon::Str("234".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS,
            Variant::from_i32(2),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS,
            Variant::from_i32(-1),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS,
            Canon::Str("199199".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS,
            Variant::from_bool(true),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_literal_companion_args() {
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STRING_LITERAL_COMPANION_ARGS,
            Variant::from_i32(8),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STRING_LITERAL_COMPANION_ARGS,
            Variant::from_i32(65),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_COMPANION_ARGS,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_COMPANION_ARGS,
            Canon::Str("12".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_COMPANION_ARGS,
            Canon::Str("45".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STRING_LITERAL_COMPANION_ARGS,
            Variant::from_i32(5),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_literal_leftb_rightb_byte_counts() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_BYTE_COUNTS,
            Canon::Str("1".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_BYTE_COUNTS,
            Canon::Str("5".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_BYTE_COUNTS,
            Canon::Str("123".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_BYTE_COUNTS,
            Canon::Str("345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_literal_leftb_rightb_byte_count_edges() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_BYTE_COUNT_EDGES,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_BYTE_COUNT_EDGES,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_COMPLEMENT_BYTE_COUNT_EDGES,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_COMPLEMENT_BYTE_COUNT_EDGES,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_EXACT_BYTE_SOURCE_COUNT,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_EXACT_BYTE_SOURCE_COUNT,
            Canon::Str("98765".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_literal_left_right_count_edges() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFT_RIGHT_COUNT_EDGES,
            Canon::Str("1".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFT_RIGHT_COUNT_EDGES,
            Canon::Str("5".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFT_RIGHT_COUNT_EDGES,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFT_RIGHT_COUNT_EDGES,
            Canon::Str("67890".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFT_RIGHT_COUNT_EDGES,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_LEFT_RIGHT_COUNT_EDGES,
            Canon::Str("98765".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_literal_mid_count_edges() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_MID_COUNT_EDGES,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_MID_COUNT_EDGES,
            Canon::Str("5".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_MID_COUNT_EDGES,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_MID_COUNT_EDGES,
            Canon::Str("2345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_literal_mid_value_edges() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_MID_VALUE_EDGES,
            Canon::Str("2345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_MID_VALUE_EDGES,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_MID_VALUE_EDGES,
            Canon::Str("5".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_LITERAL_MID_VALUE_EDGES,
            Canon::Str("12".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_literal_mid_boundary_value_edges() {
        let vm3 = run(
            Executor::Vm3,
            JIT_BUILTIN_STRING_LITERAL_MID_BOUNDARY_VALUE_EDGES,
        );
        let jit = run(
            Executor::Jit,
            JIT_BUILTIN_STRING_LITERAL_MID_BOUNDARY_VALUE_EDGES,
        );
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![Canon::Str(String::new()); 3]
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_literal_mid_error_edges() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STRING_LITERAL_MID_START_ZERO, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STRING_LITERAL_MID_NEGATIVE_START, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STRING_LITERAL_MID_NEGATIVE_LENGTH, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_static_string_operands() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_OPERANDS,
            Canon::Str("123123".into()),
        );
        assert_jit_matches_vm3_contains(JIT_BUILTIN_STATIC_STRING_OPERANDS, Variant::from_i32(6));
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_OPERANDS,
            Canon::Str("AB".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STATIC_STRING_OPERANDS,
            Variant::from_f64(123123.0),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_OPERANDS,
            Canon::Str("".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_OPERANDS,
            Canon::Str("321321".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_OPERANDS,
            Canon::Str("123".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_OPERANDS,
            Canon::Str("231".into()),
        );
        assert_jit_matches_vm3_contains(JIT_BUILTIN_STATIC_STRING_OPERANDS, Variant::from_i32(2));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_STATIC_STRING_OPERANDS, Variant::from_i32(-1));
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_OPERANDS,
            Canon::Str("199199".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STATIC_STRING_OPERANDS,
            Variant::from_bool(true),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_static_string_companion_operands() {
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STATIC_STRING_COMPANION_OPERANDS,
            Variant::from_i32(8),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STATIC_STRING_COMPANION_OPERANDS,
            Variant::from_i32(65),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_COMPANION_OPERANDS,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_COMPANION_OPERANDS,
            Canon::Str("12".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_COMPANION_OPERANDS,
            Canon::Str("45".into()),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STATIC_STRING_COMPANION_OPERANDS,
            Variant::from_i32(5),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_LEFTB_RIGHTB_ODD_BYTE_EXPRS,
            Variant::from_i32(0),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_LEFTB_RIGHTB_ODD_BYTE_EXPRS,
            Variant::from_i32(1),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_LEFTB_RIGHTB_ODD_BYTE_EXPRS,
            Variant::from_i32(3),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_LEFTB_RIGHTB_ODD_BYTE_EXPRS,
            Variant::from_i32(65),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_LEFTB_RIGHTB_ODD_BYTE_EXPRS,
            Variant::from_i32(17152),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_string_literal_leftb_rightb_odd_byte_exprs() {
        let vm3 = run(
            Executor::Vm3,
            JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_ODD_BYTE_EXPRS,
        );
        let jit = run(
            Executor::Jit,
            JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_ODD_BYTE_EXPRS,
        );
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                canon(&Variant::from_i32(0)),
                canon(&Variant::from_i32(1)),
                canon(&Variant::from_i32(1)),
                canon(&Variant::from_i32(3)),
                canon(&Variant::from_i32(65)),
                canon(&Variant::from_i32(0)),
                canon(&Variant::from_i32(1)),
                canon(&Variant::from_i32(1)),
                canon(&Variant::from_i32(3)),
                canon(&Variant::from_i32(17152)),
                Canon::Str(String::new()),
                Canon::Str("A".into()),
                Canon::Str(String::new()),
                Canon::Str(String::from_utf16_lossy(&[0x4300])),
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_static_string_leftb_rightb_byte_counts() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_BYTE_COUNTS,
            Canon::Str("1".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_BYTE_COUNTS,
            Canon::Str("5".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_BYTE_COUNTS,
            Canon::Str("123".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_BYTE_COUNTS,
            Canon::Str("345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_static_string_leftb_rightb_byte_count_edges() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_BYTE_COUNT_EDGES,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_BYTE_COUNT_EDGES,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_COMPLEMENT_BYTE_COUNT_EDGES,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_COMPLEMENT_BYTE_COUNT_EDGES,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_EXACT_BYTE_SOURCE_COUNT,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_EXACT_BYTE_SOURCE_COUNT,
            Canon::Str("98765".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_static_string_left_right_count_edges() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFT_RIGHT_COUNT_EDGES,
            Canon::Str("1".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFT_RIGHT_COUNT_EDGES,
            Canon::Str("5".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFT_RIGHT_COUNT_EDGES,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFT_RIGHT_COUNT_EDGES,
            Canon::Str("67890".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFT_RIGHT_COUNT_EDGES,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_LEFT_RIGHT_COUNT_EDGES,
            Canon::Str("98765".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_static_string_mid_count_edges() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_MID_COUNT_EDGES,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_MID_COUNT_EDGES,
            Canon::Str("5".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_MID_COUNT_EDGES,
            Canon::Str(String::new()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_MID_COUNT_EDGES,
            Canon::Str("2345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_static_string_mid_value_edges() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_MID_VALUE_EDGES,
            Canon::Str("2345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_MID_VALUE_EDGES,
            Canon::Str("12345".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_MID_VALUE_EDGES,
            Canon::Str("5".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_MID_VALUE_EDGES,
            Canon::Str("12".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_static_string_mid_boundary_value_edges() {
        let vm3 = run(
            Executor::Vm3,
            JIT_BUILTIN_STATIC_STRING_MID_BOUNDARY_VALUE_EDGES,
        );
        let jit = run(
            Executor::Jit,
            JIT_BUILTIN_STATIC_STRING_MID_BOUNDARY_VALUE_EDGES,
        );
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        assert_eq!(
            jit.result.expect("jit result"),
            vec![
                Canon::Str(String::new()),
                Canon::Str(String::new()),
                Canon::Str(String::new()),
                Canon::Str("12345".into()),
            ]
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_static_string_mid_error_edges() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STATIC_STRING_MID_START_ZERO, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STATIC_STRING_MID_NEGATIVE_START, 5);
        assert_jit_matches_vm3_raises(JIT_BUILTIN_STATIC_STRING_MID_NEGATIVE_LENGTH, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_null_slice_args() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_STRING_NULL_SLICE_ARGS, Variant::null());
    }

    #[test]
    fn jit_matches_vm3_builtin_string_empty_slice_args() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_EMPTY_SLICE_ARGS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_EMPTY_SLICE_ARGS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let mut expected = vec![Canon::Str("".into()); 6];
        expected.push(Canon::Empty);
        assert_eq!(result, expected);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_numeric_slice_args() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_NUMERIC_SLICE_ARGS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_NUMERIC_SLICE_ARGS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let mut expected = vec![
            Canon::Str("12".into()),
            Canon::Str("45".into()),
            Canon::Str("1".into()),
            Canon::Str("5".into()),
            Canon::Str("12345".into()),
            Canon::Str("12".into()),
        ];
        expected.push(canon(&Variant::from_i32(12345)));
        assert_eq!(result, expected);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_boolean_slice_args() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_BOOLEAN_SLICE_ARGS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_BOOLEAN_SLICE_ARGS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let mut expected = vec![
            Canon::Str("Tr".into()),
            Canon::Str("ue".into()),
            Canon::Str("T".into()),
            Canon::Str("e".into()),
            Canon::Str("True".into()),
            Canon::Str("Tr".into()),
        ];
        expected.push(canon(&Variant::from_bool(true)));
        assert_eq!(result, expected);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_double_slice_args() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_DOUBLE_SLICE_ARGS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_DOUBLE_SLICE_ARGS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let mut expected = vec![
            Canon::Str("12".into()),
            Canon::Str("45".into()),
            Canon::Str("1".into()),
            Canon::Str("5".into()),
            Canon::Str("12345".into()),
            Canon::Str("12".into()),
        ];
        expected.push(canon(&Variant::from_f64(12345.0)));
        assert_eq!(result, expected);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_single_slice_args() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_SINGLE_SLICE_ARGS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_SINGLE_SLICE_ARGS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let mut expected = vec![
            Canon::Str("12".into()),
            Canon::Str("45".into()),
            Canon::Str("1".into()),
            Canon::Str("5".into()),
            Canon::Str("12345".into()),
            Canon::Str("12".into()),
        ];
        expected.push(canon(&Variant::from_f32(12345.0)));
        assert_eq!(result, expected);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_integer_slice_args() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_INTEGER_SLICE_ARGS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_INTEGER_SLICE_ARGS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let mut expected = vec![
            Canon::Str("12".into()),
            Canon::Str("45".into()),
            Canon::Str("1".into()),
            Canon::Str("5".into()),
            Canon::Str("12345".into()),
            Canon::Str("12".into()),
        ];
        expected.push(canon(&Variant::from_i16(12345)));
        assert_eq!(result, expected);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_longlong_slice_args() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_LONGLONG_SLICE_ARGS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_LONGLONG_SLICE_ARGS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let mut expected = vec![
            Canon::Str("12".into()),
            Canon::Str("45".into()),
            Canon::Str("1".into()),
            Canon::Str("5".into()),
            Canon::Str("12345".into()),
            Canon::Str("12".into()),
        ];
        expected.push(canon(&Variant::from_i64(12345)));
        assert_eq!(result, expected);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_byte_slice_args() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_BYTE_SLICE_ARGS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_BYTE_SLICE_ARGS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let mut expected = vec![
            Canon::Str("12".into()),
            Canon::Str("23".into()),
            Canon::Str("1".into()),
            Canon::Str("3".into()),
            Canon::Str("123".into()),
            Canon::Str("12".into()),
        ];
        expected.push(canon(&Variant::from_u8(123)));
        assert_eq!(result, expected);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_currency_slice_args() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_CURRENCY_SLICE_ARGS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_CURRENCY_SLICE_ARGS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let mut expected = vec![
            Canon::Str("12".into()),
            Canon::Str("45".into()),
            Canon::Str("1".into()),
            Canon::Str("5".into()),
            Canon::Str("12345".into()),
            Canon::Str("12".into()),
        ];
        expected.push(canon(&Variant::from_currency_scaled_i64(123_450_000)));
        assert_eq!(result, expected);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_date_slice_args() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_DATE_SLICE_ARGS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_DATE_SLICE_ARGS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let mut expected = vec![
            Canon::Str("1/".into()),
            Canon::Str("20".into()),
            Canon::Str("1".into()),
            Canon::Str("0".into()),
            Canon::Str("1/15/2020".into()),
            Canon::Str("1/".into()),
        ];
        expected.push(canon(&Variant::from_date_f64(43845.0)));
        assert_eq!(result, expected);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_error_slice_args() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_ERROR_SLICE_ARGS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_ERROR_SLICE_ARGS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let mut expected = vec![
            Canon::Str("Er".into()),
            Canon::Str("34".into()),
            Canon::Str("E".into()),
            Canon::Str("4".into()),
            Canon::Str("Error 1234".into()),
            Canon::Str("Er".into()),
        ];
        expected.push(canon(&Variant::from_error_code(1234)));
        assert_eq!(result, expected);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_decimal_slice_args() {
        let vm3 = run(Executor::Vm3, JIT_BUILTIN_STRING_DECIMAL_SLICE_ARGS);
        let jit = run(Executor::Jit, JIT_BUILTIN_STRING_DECIMAL_SLICE_ARGS);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        let mut expected = vec![
            Canon::Str("12".into()),
            Canon::Str("45".into()),
            Canon::Str("1".into()),
            Canon::Str("5".into()),
            Canon::Str("12345".into()),
            Canon::Str("12".into()),
        ];
        expected.push(canon(&Variant::from_decimal96(
            oxvba_runtime::Decimal96::from_parts(12345, 0, 0, 0, false),
        )));
        assert_eq!(result, expected);
    }

    #[test]
    fn jit_matches_vm3_builtin_string_optional_args() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_STRING_OPTIONAL_ARGS, Variant::from_i32(4));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_STRING_OPTIONAL_ARGS, Variant::from_i32(2));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_STRING_OPTIONAL_ARGS, Variant::from_i32(5));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_STRING_OPTIONAL_ARGS, Variant::from_i32(6));
        assert_jit_matches_vm3_contains(JIT_BUILTIN_STRING_OPTIONAL_ARGS, Variant::from_i32(0));
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_OPTIONAL_ARGS,
            Canon::Str("bcxbc".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_OPTIONAL_ARGS,
            Canon::Str("xxbB".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STRING_OPTIONAL_ARGS,
            Canon::Str("zxxx".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_static_string_optional_args() {
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STATIC_STRING_OPTIONAL_ARGS,
            Variant::from_i32(4),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STATIC_STRING_OPTIONAL_ARGS,
            Variant::from_i32(2),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STATIC_STRING_OPTIONAL_ARGS,
            Variant::from_i32(5),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STATIC_STRING_OPTIONAL_ARGS,
            Variant::from_i32(6),
        );
        assert_jit_matches_vm3_contains(
            JIT_BUILTIN_STATIC_STRING_OPTIONAL_ARGS,
            Variant::from_i32(0),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_OPTIONAL_ARGS,
            Canon::Str("bcxbc".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_OPTIONAL_ARGS,
            Canon::Str("xxbB".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_STATIC_STRING_OPTIONAL_ARGS,
            Canon::Str("zxxx".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_expr() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_EXPR,
            Canon::Str("234".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_omitted_length_expr() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_OMITTED_LENGTH_EXPR,
            Canon::Str("345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_omitted_length_full_source() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_OMITTED_LENGTH_FULL_SOURCE,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_omitted_length_suffix() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_OMITTED_LENGTH_SUFFIX,
            Canon::Str("5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_omitted_length_overlong_start() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_OMITTED_LENGTH_OVERLONG_START,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_start_zero() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_MID_VARIANT_START_ZERO, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_zero_length() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_ZERO_LENGTH,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_zero_length_middle() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_ZERO_LENGTH_MIDDLE,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_zero_length_at_end() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_ZERO_LENGTH_AT_END,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_exact_last_char() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_EXACT_LAST_CHAR,
            Canon::Str("5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_exact_full_source_count() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_EXACT_FULL_SOURCE_COUNT,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_exact_suffix_count() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_EXACT_SUFFIX_COUNT,
            Canon::Str("2345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_exact_prefix_count() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_EXACT_PREFIX_COUNT,
            Canon::Str("12".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_negative_length() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_MID_VARIANT_NEGATIVE_LENGTH, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_negative_start() {
        assert_jit_matches_vm3_raises(JIT_BUILTIN_MID_VARIANT_NEGATIVE_START, 5);
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_overlong_start() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_OVERLONG_START,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_overlong_count() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_OVERLONG_COUNT,
            Canon::Str("5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_overlong_count_middle() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_OVERLONG_COUNT_MIDDLE,
            Canon::Str("2345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_mid_variant_overlong_count_full_source() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_MID_VARIANT_OVERLONG_COUNT_FULL_SOURCE,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_weekday_expr() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_WEEKDAY_EXPR, Variant::from_i32(5));
    }

    #[test]
    fn jit_matches_vm3_builtin_weekday_firstday_expr() {
        assert_jit_matches_vm3_contains(JIT_BUILTIN_WEEKDAY_FIRSTDAY_EXPR, Variant::from_i32(1));
    }

    #[test]
    fn jit_matches_vm3_builtin_date_name_exprs() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_DATE_NAME_EXPRS,
            Canon::Str("January".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_DATE_NAME_EXPRS,
            Canon::Str("Thursday".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_builtin_date_name_optional_args() {
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_DATE_NAME_OPTIONAL_ARGS,
            Canon::Str("Jan".into()),
        );
        assert_jit_matches_vm3_contains_canon(
            JIT_BUILTIN_DATE_NAME_OPTIONAL_ARGS,
            Canon::Str("Mon".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_function_return_long_call() {
        let vm3 = run(Executor::Vm3, JIT_FUNCTION_RETURN_LONG_CALL);
        let jit = run(Executor::Jit, JIT_FUNCTION_RETURN_LONG_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(14))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_byref_long_call() {
        let vm3 = run(Executor::Vm3, JIT_BYREF_LONG_CALL);
        let jit = run(Executor::Jit, JIT_BYREF_LONG_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(8))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_nested_byref_long_call() {
        let vm3 = run(Executor::Vm3, JIT_NESTED_BYREF_LONG_CALL);
        let jit = run(Executor::Jit, JIT_NESTED_BYREF_LONG_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(8))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_two_arg_function_return_long_call() {
        let vm3 = run(Executor::Vm3, JIT_TWO_ARG_FUNCTION_RETURN_LONG);
        let jit = run(Executor::Jit, JIT_TWO_ARG_FUNCTION_RETURN_LONG);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(12))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_three_arg_function_return_long_call() {
        let vm3 = run(Executor::Vm3, JIT_THREE_ARG_FUNCTION_RETURN_LONG);
        let jit = run(Executor::Jit, JIT_THREE_ARG_FUNCTION_RETURN_LONG);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(12))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_four_arg_mixed_scalar_call() {
        let vm3 = run(Executor::Vm3, JIT_FOUR_ARG_MIXED_SCALAR_CALL);
        let jit = run(Executor::Jit, JIT_FOUR_ARG_MIXED_SCALAR_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(12))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_five_arg_function_return_long_call() {
        let vm3 = run(Executor::Vm3, JIT_FIVE_ARG_FUNCTION_RETURN_LONG);
        let jit = run(Executor::Jit, JIT_FIVE_ARG_FUNCTION_RETURN_LONG);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(15))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_optional_variant_default_call() {
        let vm3 = run(Executor::Vm3, JIT_OPTIONAL_VARIANT_DEFAULT_CALL);
        let jit = run(Executor::Jit, JIT_OPTIONAL_VARIANT_DEFAULT_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(12))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_optional_variant_omitted_call() {
        let vm3 = run(Executor::Vm3, JIT_OPTIONAL_VARIANT_OMITTED_CALL);
        let jit = run(Executor::Jit, JIT_OPTIONAL_VARIANT_OMITTED_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(7))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_optional_variant_intermediate_omitted_ismissing_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_VARIANT_INTERMEDIATE_OMITTED_ISMISSING_CALL,
            Variant::from_bool(true),
        );
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_VARIANT_INTERMEDIATE_OMITTED_ISMISSING_CALL,
            Variant::from_bool(false),
        );
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_VARIANT_INTERMEDIATE_OMITTED_ISMISSING_CALL,
            Variant::from_i32(8),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_long_default_call() {
        let vm3 = run(Executor::Vm3, JIT_OPTIONAL_LONG_DEFAULT_CALL);
        let jit = run(Executor::Jit, JIT_OPTIONAL_LONG_DEFAULT_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(12))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_optional_long_omitted_call() {
        let vm3 = run(Executor::Vm3, JIT_OPTIONAL_LONG_OMITTED_CALL);
        let jit = run(Executor::Jit, JIT_OPTIONAL_LONG_OMITTED_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(0))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_optional_double_default_call() {
        let vm3 = run(Executor::Vm3, JIT_OPTIONAL_DOUBLE_DEFAULT_CALL);
        let jit = run(Executor::Jit, JIT_OPTIONAL_DOUBLE_DEFAULT_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(6.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_optional_double_omitted_call() {
        let vm3 = run(Executor::Vm3, JIT_OPTIONAL_DOUBLE_OMITTED_CALL);
        let jit = run(Executor::Jit, JIT_OPTIONAL_DOUBLE_OMITTED_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(0.0))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_optional_currency_default_call() {
        let vm3 = run(Executor::Vm3, JIT_OPTIONAL_CURRENCY_DEFAULT_CALL);
        let jit = run(Executor::Jit, JIT_OPTIONAL_CURRENCY_DEFAULT_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(25_000))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_optional_currency_omitted_call() {
        let vm3 = run(Executor::Vm3, JIT_OPTIONAL_CURRENCY_OMITTED_CALL);
        let jit = run(Executor::Jit, JIT_OPTIONAL_CURRENCY_OMITTED_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(0))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_optional_bool_default_call() {
        assert_jit_matches_vm3_contains(JIT_OPTIONAL_BOOL_DEFAULT_CALL, Variant::from_bool(true));
    }

    #[test]
    fn jit_matches_vm3_optional_bool_omitted_call() {
        assert_jit_matches_vm3_contains(JIT_OPTIONAL_BOOL_OMITTED_CALL, Variant::from_bool(false));
    }

    #[test]
    fn jit_matches_vm3_optional_byte_default_call() {
        assert_jit_matches_vm3_contains(JIT_OPTIONAL_BYTE_DEFAULT_CALL, Variant::from_u8(7));
    }

    #[test]
    fn jit_matches_vm3_optional_byte_omitted_call() {
        assert_jit_matches_vm3_contains(JIT_OPTIONAL_BYTE_OMITTED_CALL, Variant::from_u8(0));
    }

    #[test]
    fn jit_matches_vm3_optional_integer_default_call() {
        assert_jit_matches_vm3_contains(JIT_OPTIONAL_INTEGER_DEFAULT_CALL, Variant::from_i16(12));
    }

    #[test]
    fn jit_matches_vm3_optional_integer_omitted_call() {
        assert_jit_matches_vm3_contains(JIT_OPTIONAL_INTEGER_OMITTED_CALL, Variant::from_i16(0));
    }

    #[test]
    fn jit_matches_vm3_optional_longlong_default_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONGLONG_DEFAULT_CALL,
            Variant::from_i64(5_000_000_012),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_longlong_omitted_call() {
        assert_jit_matches_vm3_contains(JIT_OPTIONAL_LONGLONG_OMITTED_CALL, Variant::from_i64(0));
    }

    #[test]
    fn jit_matches_vm3_optional_single_default_call() {
        assert_jit_matches_vm3_contains(JIT_OPTIONAL_SINGLE_DEFAULT_CALL, Variant::from_f32(1.5));
    }

    #[test]
    fn jit_matches_vm3_optional_single_omitted_call() {
        assert_jit_matches_vm3_contains(JIT_OPTIONAL_SINGLE_OMITTED_CALL, Variant::from_f32(0.0));
    }

    #[test]
    fn jit_matches_vm3_optional_date_default_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_DEFAULT_CALL,
            Variant::from_date_f64(36527.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_date_omitted_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_OMITTED_CALL,
            Variant::from_date_f64(0.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_variant_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_VARIANT_EXPLICIT_LOCAL_CALL,
            Variant::from_i32(13),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_long_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_EXPLICIT_LOCAL_CALL,
            Variant::from_i32(13),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_double_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_EXPLICIT_LOCAL_CALL,
            Variant::from_f64(7.25),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_currency_explicit_local_call() {
        let vm3 = run(Executor::Vm3, JIT_OPTIONAL_CURRENCY_EXPLICIT_LOCAL_CALL);
        let jit = run(Executor::Jit, JIT_OPTIONAL_CURRENCY_EXPLICIT_LOCAL_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(37_500))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_optional_bool_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_EXPLICIT_LOCAL_CALL,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_byte_explicit_local_call() {
        assert_jit_matches_vm3_contains(JIT_OPTIONAL_BYTE_EXPLICIT_LOCAL_CALL, Variant::from_u8(9));
    }

    #[test]
    fn jit_matches_vm3_optional_integer_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_INTEGER_EXPLICIT_LOCAL_CALL,
            Variant::from_i16(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_longlong_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONGLONG_EXPLICIT_LOCAL_CALL,
            Variant::from_i64(5_000_000_013),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_single_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_EXPLICIT_LOCAL_CALL,
            Variant::from_f32(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_date_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_EXPLICIT_LOCAL_CALL,
            Variant::from_date_f64(36528.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_variant_named_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_VARIANT_NAMED_EXPLICIT_LOCAL_CALL,
            Variant::from_i32(13),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_long_named_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_NAMED_EXPLICIT_LOCAL_CALL,
            Variant::from_i32(13),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_double_named_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_NAMED_EXPLICIT_LOCAL_CALL,
            Variant::from_f64(7.25),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_currency_named_explicit_local_call() {
        let vm3 = run(
            Executor::Vm3,
            JIT_OPTIONAL_CURRENCY_NAMED_EXPLICIT_LOCAL_CALL,
        );
        let jit = run(
            Executor::Jit,
            JIT_OPTIONAL_CURRENCY_NAMED_EXPLICIT_LOCAL_CALL,
        );
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(37_500))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_arg_order_long_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_NAMED_ARG_ORDER_DOUBLE_COERCE_CALL,
            Variant::from_i32(13),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_arg_order_double_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_NAMED_ARG_ORDER_LONG_COERCE_CALL,
            Variant::from_f64(7.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_arg_order_currency_integer_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_CURRENCY_NAMED_ARG_ORDER_INTEGER_COERCE_CALL,
            Variant::from_currency_scaled_i64(32_500),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_arg_order_bool_double_zero_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_NAMED_ARG_ORDER_DOUBLE_ZERO_COERCE_CALL,
            Variant::from_i32(5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_bool_named_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_NAMED_EXPLICIT_LOCAL_CALL,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_byte_named_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BYTE_NAMED_EXPLICIT_LOCAL_CALL,
            Variant::from_u8(9),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_integer_named_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_INTEGER_NAMED_EXPLICIT_LOCAL_CALL,
            Variant::from_i16(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_longlong_named_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONGLONG_NAMED_EXPLICIT_LOCAL_CALL,
            Variant::from_i64(5_000_000_013),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_single_named_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_NAMED_EXPLICIT_LOCAL_CALL,
            Variant::from_f32(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_date_named_explicit_local_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_NAMED_EXPLICIT_LOCAL_CALL,
            Variant::from_date_f64(36528.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_long_explicit_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_EXPLICIT_DOUBLE_COERCE_CALL,
            Variant::from_i32(8),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_double_explicit_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_EXPLICIT_LONG_COERCE_CALL,
            Variant::from_f64(8.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_currency_explicit_double_coerce_call() {
        let vm3 = run(
            Executor::Vm3,
            JIT_OPTIONAL_CURRENCY_EXPLICIT_DOUBLE_COERCE_CALL,
        );
        let jit = run(
            Executor::Jit,
            JIT_OPTIONAL_CURRENCY_EXPLICIT_DOUBLE_COERCE_CALL,
        );
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(25_000))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_bool_explicit_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_EXPLICIT_LONG_COERCE_CALL,
            Variant::from_bool(true),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_byte_explicit_integer_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BYTE_EXPLICIT_INTEGER_COERCE_CALL,
            Variant::from_u8(9),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_integer_explicit_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_INTEGER_EXPLICIT_LONG_COERCE_CALL,
            Variant::from_i16(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_longlong_explicit_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONGLONG_EXPLICIT_LONG_COERCE_CALL,
            Variant::from_i64(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_single_explicit_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_EXPLICIT_DOUBLE_COERCE_CALL,
            Variant::from_f32(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_date_explicit_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_EXPLICIT_DOUBLE_COERCE_CALL,
            Variant::from_date_f64(36528.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_long_explicit_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_LONG_EXPLICIT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_bool_explicit_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_BOOL_EXPLICIT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_byte_explicit_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_BYTE_EXPLICIT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_integer_explicit_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_INTEGER_EXPLICIT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_longlong_explicit_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_LONGLONG_EXPLICIT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_single_explicit_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_SINGLE_EXPLICIT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_double_explicit_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_DOUBLE_EXPLICIT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_currency_explicit_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_CURRENCY_EXPLICIT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_date_explicit_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_DATE_EXPLICIT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_long_explicit_boolean_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_EXPLICIT_BOOLEAN_COERCE_CALL,
            Variant::from_i32(-1),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_long_explicit_empty_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_EXPLICIT_EMPTY_COERCE_CALL,
            Variant::from_i32(0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_bool_explicit_empty_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_EXPLICIT_EMPTY_COERCE_CALL,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_date_explicit_empty_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_EXPLICIT_EMPTY_COERCE_CALL,
            Variant::from_date_f64(0.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_double_explicit_currency_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_EXPLICIT_CURRENCY_COERCE_CALL,
            Variant::from_f64(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_currency_explicit_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_CURRENCY_EXPLICIT_LONG_COERCE_CALL,
            Variant::from_currency_scaled_i64(20_000),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_single_explicit_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_EXPLICIT_LONG_COERCE_CALL,
            Variant::from_f32(34.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_byte_explicit_long_overflow_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_BYTE_EXPLICIT_LONG_OVERFLOW_CALL, 6);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_integer_explicit_long_overflow_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_INTEGER_EXPLICIT_LONG_OVERFLOW_CALL, 6);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_long_explicit_error_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_LONG_EXPLICIT_ERROR_COERCE_ERROR_CALL, 13);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_byte_explicit_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BYTE_EXPLICIT_LONG_COERCE_CALL,
            Variant::from_u8(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_integer_explicit_byte_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_INTEGER_EXPLICIT_BYTE_COERCE_CALL,
            Variant::from_i16(9),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_longlong_explicit_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONGLONG_EXPLICIT_DOUBLE_COERCE_CALL,
            Variant::from_i64(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_single_explicit_currency_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_EXPLICIT_CURRENCY_COERCE_CALL,
            Variant::from_f32(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_double_explicit_single_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_EXPLICIT_SINGLE_COERCE_CALL,
            Variant::from_f64(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_currency_explicit_integer_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_CURRENCY_EXPLICIT_INTEGER_COERCE_CALL,
            Variant::from_currency_scaled_i64(20_000),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_bool_explicit_double_zero_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_EXPLICIT_DOUBLE_ZERO_COERCE_CALL,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_date_explicit_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_EXPLICIT_LONG_COERCE_CALL,
            Variant::from_date_f64(36528.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_long_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_NAMED_DOUBLE_COERCE_CALL,
            Variant::from_i32(8),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_double_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_NAMED_LONG_COERCE_CALL,
            Variant::from_f64(8.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_currency_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_CURRENCY_NAMED_DOUBLE_COERCE_CALL,
            Variant::from_currency_scaled_i64(25_000),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_bool_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_NAMED_LONG_COERCE_CALL,
            Variant::from_bool(true),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_byte_integer_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BYTE_NAMED_INTEGER_COERCE_CALL,
            Variant::from_u8(9),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_integer_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_INTEGER_NAMED_LONG_COERCE_CALL,
            Variant::from_i16(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_longlong_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONGLONG_NAMED_LONG_COERCE_CALL,
            Variant::from_i64(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_single_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_NAMED_DOUBLE_COERCE_CALL,
            Variant::from_f32(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_date_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_NAMED_DOUBLE_COERCE_CALL,
            Variant::from_date_f64(36528.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_byte_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BYTE_NAMED_LONG_COERCE_CALL,
            Variant::from_u8(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_integer_byte_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_INTEGER_NAMED_BYTE_COERCE_CALL,
            Variant::from_i16(9),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_longlong_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONGLONG_NAMED_DOUBLE_COERCE_CALL,
            Variant::from_i64(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_single_currency_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_NAMED_CURRENCY_COERCE_CALL,
            Variant::from_f32(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_double_single_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_NAMED_SINGLE_COERCE_CALL,
            Variant::from_f64(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_currency_integer_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_CURRENCY_NAMED_INTEGER_COERCE_CALL,
            Variant::from_currency_scaled_i64(20_000),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_bool_double_zero_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_NAMED_DOUBLE_ZERO_COERCE_CALL,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_date_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_NAMED_LONG_COERCE_CALL,
            Variant::from_date_f64(36528.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_long_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_LONG_NAMED_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_bool_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_BOOL_NAMED_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_byte_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_BYTE_NAMED_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_integer_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_INTEGER_NAMED_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_longlong_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_LONGLONG_NAMED_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_single_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_SINGLE_NAMED_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_double_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_DOUBLE_NAMED_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_currency_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_CURRENCY_NAMED_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_date_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_DATE_NAMED_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_long_boolean_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_NAMED_BOOLEAN_COERCE_CALL,
            Variant::from_i32(-1),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_long_empty_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_NAMED_EMPTY_COERCE_CALL,
            Variant::from_i32(0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_bool_empty_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_NAMED_EMPTY_COERCE_CALL,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_date_empty_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_NAMED_EMPTY_COERCE_CALL,
            Variant::from_date_f64(0.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_double_currency_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_NAMED_CURRENCY_COERCE_CALL,
            Variant::from_f64(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_currency_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_CURRENCY_NAMED_LONG_COERCE_CALL,
            Variant::from_currency_scaled_i64(20_000),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_single_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_NAMED_LONG_COERCE_CALL,
            Variant::from_f32(34.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_byte_long_overflow_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_BYTE_NAMED_LONG_OVERFLOW_CALL, 6);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_integer_long_overflow_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_INTEGER_NAMED_LONG_OVERFLOW_CALL, 6);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_long_error_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_LONG_NAMED_ERROR_COERCE_ERROR_CALL, 13);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_long_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL,
            Variant::from_i32(8),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_double_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            Variant::from_f64(8.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_currency_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_CURRENCY_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL,
            Variant::from_currency_scaled_i64(25_000),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_bool_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            Variant::from_bool(true),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_byte_integer_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BYTE_EXPLICIT_VARIANT_INTEGER_COERCE_CALL,
            Variant::from_u8(9),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_integer_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_INTEGER_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            Variant::from_i16(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_longlong_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONGLONG_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            Variant::from_i64(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_single_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL,
            Variant::from_f32(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_date_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL,
            Variant::from_date_f64(36528.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_byte_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BYTE_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            Variant::from_u8(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_integer_byte_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_INTEGER_EXPLICIT_VARIANT_BYTE_COERCE_CALL,
            Variant::from_i16(9),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_longlong_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONGLONG_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL,
            Variant::from_i64(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_single_currency_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_EXPLICIT_VARIANT_CURRENCY_COERCE_CALL,
            Variant::from_f32(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_double_single_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_EXPLICIT_VARIANT_SINGLE_COERCE_CALL,
            Variant::from_f64(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_currency_integer_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_CURRENCY_EXPLICIT_VARIANT_INTEGER_COERCE_CALL,
            Variant::from_currency_scaled_i64(20_000),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_bool_double_zero_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_EXPLICIT_VARIANT_DOUBLE_ZERO_COERCE_CALL,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_date_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            Variant::from_date_f64(36528.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_long_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_bool_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_BOOL_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_byte_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_BYTE_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_integer_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_INTEGER_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_longlong_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_LONGLONG_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_single_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_SINGLE_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_double_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_DOUBLE_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_currency_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_CURRENCY_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_date_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_DATE_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_long_boolean_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_BOOLEAN_COERCE_CALL,
            Variant::from_i32(-1),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_long_empty_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_EMPTY_COERCE_CALL,
            Variant::from_i32(0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_bool_empty_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_EXPLICIT_VARIANT_EMPTY_COERCE_CALL,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_date_empty_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_EXPLICIT_VARIANT_EMPTY_COERCE_CALL,
            Variant::from_date_f64(0.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_double_currency_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_EXPLICIT_VARIANT_CURRENCY_COERCE_CALL,
            Variant::from_f64(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_currency_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_CURRENCY_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            Variant::from_currency_scaled_i64(20_000),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_single_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            Variant::from_f32(34.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_byte_long_overflow_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_BYTE_EXPLICIT_VARIANT_LONG_OVERFLOW_CALL, 6);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_integer_long_overflow_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_INTEGER_EXPLICIT_VARIANT_LONG_OVERFLOW_CALL, 6);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_variant_long_error_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_ERROR_COERCE_ERROR_CALL,
            13,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_long_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_NAMED_VARIANT_DOUBLE_COERCE_CALL,
            Variant::from_i32(8),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_double_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_NAMED_VARIANT_LONG_COERCE_CALL,
            Variant::from_f64(8.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_currency_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_CURRENCY_NAMED_VARIANT_DOUBLE_COERCE_CALL,
            Variant::from_currency_scaled_i64(25_000),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_bool_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_NAMED_VARIANT_LONG_COERCE_CALL,
            Variant::from_bool(true),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_byte_integer_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BYTE_NAMED_VARIANT_INTEGER_COERCE_CALL,
            Variant::from_u8(9),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_integer_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_INTEGER_NAMED_VARIANT_LONG_COERCE_CALL,
            Variant::from_i16(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_longlong_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONGLONG_NAMED_VARIANT_LONG_COERCE_CALL,
            Variant::from_i64(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_single_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_NAMED_VARIANT_DOUBLE_COERCE_CALL,
            Variant::from_f32(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_date_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_NAMED_VARIANT_DOUBLE_COERCE_CALL,
            Variant::from_date_f64(36528.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_byte_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BYTE_NAMED_VARIANT_LONG_COERCE_CALL,
            Variant::from_u8(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_integer_byte_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_INTEGER_NAMED_VARIANT_BYTE_COERCE_CALL,
            Variant::from_i16(9),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_longlong_double_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONGLONG_NAMED_VARIANT_DOUBLE_COERCE_CALL,
            Variant::from_i64(34),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_single_currency_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_NAMED_VARIANT_CURRENCY_COERCE_CALL,
            Variant::from_f32(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_double_single_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_NAMED_VARIANT_SINGLE_COERCE_CALL,
            Variant::from_f64(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_currency_integer_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_CURRENCY_NAMED_VARIANT_INTEGER_COERCE_CALL,
            Variant::from_currency_scaled_i64(20_000),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_bool_double_zero_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_NAMED_VARIANT_DOUBLE_ZERO_COERCE_CALL,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_date_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_NAMED_VARIANT_LONG_COERCE_CALL,
            Variant::from_date_f64(36528.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_long_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_LONG_NAMED_VARIANT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_bool_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_BOOL_NAMED_VARIANT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_byte_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_BYTE_NAMED_VARIANT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_integer_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_INTEGER_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_longlong_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_LONGLONG_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_single_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_SINGLE_NAMED_VARIANT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_double_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_DOUBLE_NAMED_VARIANT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_currency_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_CURRENCY_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_date_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_DATE_NAMED_VARIANT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_long_boolean_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_NAMED_VARIANT_BOOLEAN_COERCE_CALL,
            Variant::from_i32(-1),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_long_empty_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_LONG_NAMED_VARIANT_EMPTY_COERCE_CALL,
            Variant::from_i32(0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_bool_empty_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_BOOL_NAMED_VARIANT_EMPTY_COERCE_CALL,
            Variant::from_bool(false),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_date_empty_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DATE_NAMED_VARIANT_EMPTY_COERCE_CALL,
            Variant::from_date_f64(0.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_double_currency_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_DOUBLE_NAMED_VARIANT_CURRENCY_COERCE_CALL,
            Variant::from_f64(2.5),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_currency_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_CURRENCY_NAMED_VARIANT_LONG_COERCE_CALL,
            Variant::from_currency_scaled_i64(20_000),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_single_long_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_OPTIONAL_SINGLE_NAMED_VARIANT_LONG_COERCE_CALL,
            Variant::from_f32(34.0),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_byte_long_overflow_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_BYTE_NAMED_VARIANT_LONG_OVERFLOW_CALL, 6);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_integer_long_overflow_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_INTEGER_NAMED_VARIANT_LONG_OVERFLOW_CALL, 6);
    }

    #[test]
    fn jit_matches_vm3_optional_scalar_named_variant_long_error_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_LONG_NAMED_VARIANT_ERROR_COERCE_ERROR_CALL, 13);
    }

    #[test]
    fn jit_matches_vm3_optional_string_default_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_DEFAULT_CALL,
            Canon::Str("alpha".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_omitted_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_OMITTED_CALL,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_string_literal_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_STRING_LITERAL_CALL,
            Canon::Str("beta".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_string_local_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_STRING_LOCAL_CALL,
            Canon::Str("beta".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_empty_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_EMPTY_COERCE_CALL,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_STRING_EXPLICIT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_error_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_ERROR_COERCE_CALL,
            Canon::Str("Error 1234".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_decimal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_DECIMAL_COERCE_CALL,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_numeric_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_NUMERIC_COERCE_CALL,
            Canon::Str("42".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_boolean_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_BOOLEAN_COERCE_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_double_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_DOUBLE_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_single_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_SINGLE_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_currency_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_CURRENCY_COERCE_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_integer_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_INTEGER_COERCE_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_byte_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_BYTE_COERCE_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_longlong_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_LONGLONG_COERCE_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_date_literal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_DATE_LITERAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_dateserial_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_DATESERIAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_cdate_numeric_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_CDATE_NUMERIC_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_cdate_string_literal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_CDATE_STRING_LITERAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_cdate_month_name_literal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_CDATE_MONTH_NAME_LITERAL_COERCE_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_cdate_invalid_string_literal_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_STRING_EXPLICIT_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            13,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_cdate_string_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_CDATE_STRING_LOCAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_cdate_month_name_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_CDATE_MONTH_NAME_LOCAL_COERCE_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_cdate_invalid_string_local_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_STRING_EXPLICIT_CDATE_INVALID_STRING_LOCAL_ERROR_CALL,
            13,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_date_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_DATE_LOCAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_COERCE_CALL,
            Canon::Str("45".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_boolean_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_BOOLEAN_COERCE_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_double_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_single_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_SINGLE_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_currency_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_CURRENCY_COERCE_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_integer_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_INTEGER_COERCE_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_byte_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_BYTE_COERCE_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_longlong_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_LONGLONG_COERCE_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_date_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_DATE_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_string_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_STRING_COERCE_CALL,
            Canon::Str("beta".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_empty_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_EMPTY_COERCE_CALL,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_error_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_ERROR_COERCE_CALL,
            Canon::Str("Error 1234".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_explicit_variant_decimal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_DECIMAL_COERCE_CALL,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_numeric_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_NUMERIC_COERCE_CALL,
            Canon::Str("42".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_long_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_LONG_COERCE_CALL,
            Canon::Str("42".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_date_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_DATE_LOCAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_dateserial_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_DATESERIAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_cdate_numeric_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_CDATE_NUMERIC_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_cdate_month_name_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_CDATE_MONTH_NAME_LOCAL_COERCE_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_cdate_invalid_string_literal_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            13,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_empty_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_EMPTY_COERCE_CALL,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_error_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_ERROR_COERCE_CALL,
            Canon::Str("Error 1234".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_boolean_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_BOOLEAN_LOCAL_COERCE_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_currency_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_CURRENCY_LOCAL_COERCE_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_double_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_DOUBLE_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_single_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_SINGLE_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_integer_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_INTEGER_COERCE_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_byte_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_BYTE_LOCAL_COERCE_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_longlong_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_LONGLONG_COERCE_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_variant_decimal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_DECIMAL_COERCE_CALL,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_variant_boolean_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_BOOLEAN_COERCE_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_variant_double_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_DOUBLE_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_variant_single_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_SINGLE_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_variant_currency_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_CURRENCY_COERCE_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_variant_integer_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_INTEGER_COERCE_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_variant_byte_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_BYTE_COERCE_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_variant_longlong_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_LONGLONG_COERCE_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_variant_date_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_DATE_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_variant_string_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_STRING_COERCE_CALL,
            Canon::Str("beta".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_variant_error_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_ERROR_COERCE_CALL,
            Canon::Str("Error 1234".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_variant_empty_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_EMPTY_COERCE_CALL,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_arg_order_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_NULL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_string_literal_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_STRING_LITERAL_CALL,
            Canon::Str("beta".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_string_local_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_STRING_LOCAL_CALL,
            Canon::Str("beta".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_empty_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_EMPTY_COERCE_CALL,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_STRING_NAMED_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_error_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_ERROR_COERCE_CALL,
            Canon::Str("Error 1234".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_decimal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_DECIMAL_COERCE_CALL,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_boolean_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_BOOLEAN_COERCE_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_double_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_DOUBLE_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_single_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_SINGLE_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_currency_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_CURRENCY_COERCE_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_integer_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_INTEGER_COERCE_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_byte_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_BYTE_COERCE_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_longlong_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_LONGLONG_COERCE_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_date_literal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_DATE_LITERAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_dateserial_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_DATESERIAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_cdate_numeric_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_CDATE_NUMERIC_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_cdate_string_literal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_CDATE_STRING_LITERAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_cdate_month_name_literal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_CDATE_MONTH_NAME_LITERAL_COERCE_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_cdate_invalid_string_literal_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_STRING_NAMED_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            13,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_cdate_string_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_CDATE_STRING_LOCAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_cdate_month_name_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_CDATE_MONTH_NAME_LOCAL_COERCE_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_cdate_invalid_string_local_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_OPTIONAL_STRING_NAMED_CDATE_INVALID_STRING_LOCAL_ERROR_CALL,
            13,
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_date_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_DATE_LOCAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_COERCE_CALL,
            Canon::Str("45".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_boolean_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_BOOLEAN_COERCE_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_double_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_DOUBLE_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_single_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_SINGLE_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_currency_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_CURRENCY_COERCE_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_integer_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_INTEGER_COERCE_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_byte_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_BYTE_COERCE_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_longlong_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_LONGLONG_COERCE_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_date_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_DATE_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_string_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_STRING_COERCE_CALL,
            Canon::Str("beta".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_empty_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_EMPTY_COERCE_CALL,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_null_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_OPTIONAL_STRING_NAMED_VARIANT_NULL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_error_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_ERROR_COERCE_CALL,
            Canon::Str("Error 1234".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_optional_string_named_variant_decimal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_OPTIONAL_STRING_NAMED_VARIANT_DECIMAL_COERCE_CALL,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_box_assignment() {
        let vm3 = run(Executor::Vm3, JIT_VARIANT_BOX_ASSIGNMENT);
        let jit = run(Executor::Jit, JIT_VARIANT_BOX_ASSIGNMENT);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i16(42))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_call() {
        let vm3 = run(Executor::Vm3, JIT_VARIANT_RETURN_CALL);
        let jit = run(Executor::Jit, JIT_VARIANT_RETURN_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i16(42))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_variant_byref_call() {
        let vm3 = run(Executor::Vm3, JIT_VARIANT_BYREF_CALL);
        let jit = run(Executor::Jit, JIT_VARIANT_BYREF_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i16(42))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_RETURN_CALL,
            Canon::Str("alpha".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_local_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_LOCAL_RETURN_CALL,
            Canon::Str("alpha".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byref_call() {
        assert_jit_matches_vm3_contains_canon(JIT_STRING_BYREF_CALL, Canon::Str("beta".into()));
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_CALL,
            Canon::Str("beta".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_numeric_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_NUMERIC_COERCE_CALL,
            Canon::Str("42".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_long_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_LONG_LOCAL_COERCE_CALL,
            Canon::Str("43".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_boolean_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_BOOLEAN_LOCAL_COERCE_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_double_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_DOUBLE_LOCAL_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_single_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_SINGLE_LOCAL_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_currency_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_CURRENCY_LOCAL_COERCE_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_integer_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_INTEGER_LOCAL_COERCE_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_byte_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_BYTE_LOCAL_COERCE_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_longlong_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_LONGLONG_LOCAL_COERCE_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_date_literal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_DATE_LITERAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_date_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_DATE_LOCAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_dateserial_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_DATESERIAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_cdate_numeric_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_CDATE_NUMERIC_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_cdate_string_literal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_CDATE_STRING_LITERAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_cdate_string_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_CDATE_STRING_LOCAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_cdate_invalid_string_literal_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_STRING_MIXED_BYREF_BYVAL_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            13,
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_cdate_invalid_string_local_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_STRING_MIXED_BYREF_BYVAL_CDATE_INVALID_STRING_LOCAL_ERROR_CALL,
            13,
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_cdate_month_name_literal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_CDATE_MONTH_NAME_LITERAL_COERCE_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_cdate_month_name_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_CDATE_MONTH_NAME_LOCAL_COERCE_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_LOCAL_COERCE_CALL,
            Canon::Str("45".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_boolean_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_BOOLEAN_LOCAL_COERCE_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_double_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_DOUBLE_LOCAL_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_single_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_SINGLE_LOCAL_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_currency_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_CURRENCY_LOCAL_COERCE_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_integer_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_INTEGER_LOCAL_COERCE_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_byte_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_BYTE_LOCAL_COERCE_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_longlong_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_LONGLONG_LOCAL_COERCE_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_date_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_DATE_LOCAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_string_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_STRING_LOCAL_COERCE_CALL,
            Canon::Str("alpha".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_error_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_ERROR_LOCAL_COERCE_CALL,
            Canon::Str("Error 1234".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_decimal_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_DECIMAL_LOCAL_COERCE_CALL,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_empty_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_EMPTY_LOCAL_COERCE_CALL,
            Canon::Str("".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_mixed_byref_byval_variant_null_local_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_NULL_LOCAL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_RETURN_CALL,
            Canon::Str("beta".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_numeric_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_NUMERIC_COERCE_RETURN_CALL,
            Canon::Str("42".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_long_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_LONG_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("43".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_boolean_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_BOOLEAN_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_double_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_DOUBLE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_single_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_SINGLE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_currency_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_CURRENCY_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_integer_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_INTEGER_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_byte_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_BYTE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_longlong_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_LONGLONG_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("45".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_boolean_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_BOOLEAN_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_double_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_DOUBLE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_single_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_SINGLE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_currency_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_CURRENCY_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_integer_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_INTEGER_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_byte_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_BYTE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_longlong_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_LONGLONG_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_date_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_DATE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_string_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_STRING_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("alpha".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_error_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_ERROR_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("Error 1234".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_decimal_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_DECIMAL_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_empty_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_VARIANT_EMPTY_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_variant_null_local_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_STRING_NAMED_BYVAL_VARIANT_NULL_LOCAL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_date_literal_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_DATE_LITERAL_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_date_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_DATE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_dateserial_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_DATESERIAL_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_cdate_numeric_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_CDATE_NUMERIC_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_cdate_string_literal_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_CDATE_STRING_LITERAL_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_cdate_string_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_CDATE_STRING_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_cdate_invalid_string_literal_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_STRING_NAMED_BYVAL_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            13,
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_cdate_invalid_string_local_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_STRING_NAMED_BYVAL_CDATE_INVALID_STRING_LOCAL_ERROR_CALL,
            13,
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_cdate_month_name_literal_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_CDATE_MONTH_NAME_LITERAL_COERCE_RETURN_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_byval_cdate_month_name_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_BYVAL_CDATE_MONTH_NAME_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CALL,
            Canon::Str("beta".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_numeric_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_NUMERIC_COERCE_CALL,
            Canon::Str("42".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_long_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_LONG_LOCAL_COERCE_CALL,
            Canon::Str("43".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_boolean_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_BOOLEAN_LOCAL_COERCE_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_double_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_DOUBLE_LOCAL_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_single_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_SINGLE_LOCAL_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_currency_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CURRENCY_LOCAL_COERCE_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_integer_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_INTEGER_LOCAL_COERCE_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_byte_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_BYTE_LOCAL_COERCE_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_longlong_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_LONGLONG_LOCAL_COERCE_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_date_literal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_DATE_LITERAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_date_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_DATE_LOCAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_dateserial_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_DATESERIAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_cdate_numeric_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_NUMERIC_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_cdate_string_literal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_STRING_LITERAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_cdate_string_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_STRING_LOCAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_cdate_invalid_string_literal_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            13,
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_cdate_invalid_string_local_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_INVALID_STRING_LOCAL_ERROR_CALL,
            13,
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_cdate_month_name_literal_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_MONTH_NAME_LITERAL_COERCE_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_cdate_month_name_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_MONTH_NAME_LOCAL_COERCE_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_LOCAL_COERCE_CALL,
            Canon::Str("45".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_boolean_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_BOOLEAN_LOCAL_COERCE_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_double_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_DOUBLE_LOCAL_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_single_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_SINGLE_LOCAL_COERCE_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_currency_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_CURRENCY_LOCAL_COERCE_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_integer_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_INTEGER_LOCAL_COERCE_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_byte_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_BYTE_LOCAL_COERCE_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_longlong_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_LONGLONG_LOCAL_COERCE_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_date_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_DATE_LOCAL_COERCE_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_string_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_STRING_LOCAL_COERCE_CALL,
            Canon::Str("alpha".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_error_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_ERROR_LOCAL_COERCE_CALL,
            Canon::Str("Error 1234".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_decimal_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_DECIMAL_LOCAL_COERCE_CALL,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_empty_local_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_EMPTY_LOCAL_COERCE_CALL,
            Canon::Str("".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_named_mixed_byref_byval_variant_null_local_coerce_error_call() {
        assert_jit_matches_vm3_raises(
            JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_NULL_LOCAL_COERCE_ERROR_CALL,
            94,
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_numeric_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_NUMERIC_COERCE_RETURN_CALL,
            Canon::Str("42".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_long_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_LONG_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("43".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_boolean_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_BOOLEAN_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_double_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_DOUBLE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_single_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_SINGLE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_currency_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_CURRENCY_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_integer_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_INTEGER_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_byte_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_BYTE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_longlong_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_LONGLONG_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_date_literal_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_DATE_LITERAL_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_date_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_DATE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_dateserial_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_DATESERIAL_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_cdate_numeric_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_CDATE_NUMERIC_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_cdate_string_literal_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_CDATE_STRING_LITERAL_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_cdate_string_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_CDATE_STRING_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_cdate_invalid_string_literal_error_call() {
        assert_jit_matches_vm3_raises(JIT_STRING_BYVAL_CDATE_INVALID_STRING_LITERAL_ERROR_CALL, 13);
    }

    #[test]
    fn jit_matches_vm3_string_byval_cdate_invalid_string_local_error_call() {
        assert_jit_matches_vm3_raises(JIT_STRING_BYVAL_CDATE_INVALID_STRING_LOCAL_ERROR_CALL, 13);
    }

    #[test]
    fn jit_matches_vm3_string_byval_cdate_month_name_literal_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_CDATE_MONTH_NAME_LITERAL_COERCE_RETURN_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_cdate_month_name_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_CDATE_MONTH_NAME_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("2/28/2026".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("45".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_boolean_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_BOOLEAN_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_double_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_DOUBLE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_single_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_SINGLE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_currency_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_CURRENCY_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_integer_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_INTEGER_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_byte_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_BYTE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_longlong_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_LONGLONG_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_date_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_DATE_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_string_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_STRING_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("alpha".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_error_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_ERROR_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("Error 1234".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_decimal_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_DECIMAL_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_empty_local_coerce_return_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_BYVAL_VARIANT_EMPTY_LOCAL_COERCE_RETURN_CALL,
            Canon::Str("".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_string_byval_variant_null_local_coerce_error_call() {
        assert_jit_matches_vm3_raises(JIT_STRING_BYVAL_VARIANT_NULL_LOCAL_COERCE_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_long_return_to_variant_call() {
        assert_jit_matches_vm3_contains(JIT_LONG_RETURN_TO_VARIANT_CALL, Variant::from_i32(42));
    }

    #[test]
    fn jit_matches_vm3_string_return_to_variant_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_STRING_RETURN_TO_VARIANT_CALL,
            Canon::Str("alpha".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_long_call() {
        assert_jit_matches_vm3_contains(JIT_VARIANT_RETURN_TO_LONG_CALL, Variant::from_i32(42));
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_string_coerce_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_TO_STRING_COERCE_CALL,
            Canon::Str("42".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_bool_coerce_call() {
        assert_jit_matches_vm3_contains(
            JIT_VARIANT_RETURN_TO_BOOL_COERCE_CALL,
            Variant::from_bool(true),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_double_call() {
        assert_jit_matches_vm3_contains(JIT_VARIANT_RETURN_TO_DOUBLE_CALL, Variant::from_f64(12.5));
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_string_boolean_payload_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_TO_STRING_BOOLEAN_PAYLOAD_CALL,
            Canon::Str("True".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_string_double_payload_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_TO_STRING_DOUBLE_PAYLOAD_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_string_single_payload_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_TO_STRING_SINGLE_PAYLOAD_CALL,
            Canon::Str("12.5".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_string_currency_payload_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_TO_STRING_CURRENCY_PAYLOAD_CALL,
            Canon::Str("12.3456".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_string_integer_payload_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_TO_STRING_INTEGER_PAYLOAD_CALL,
            Canon::Str("44".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_string_byte_payload_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_TO_STRING_BYTE_PAYLOAD_CALL,
            Canon::Str("7".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_string_longlong_payload_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_TO_STRING_LONGLONG_PAYLOAD_CALL,
            Canon::Str("5000000012".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_string_string_payload_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_TO_STRING_STRING_PAYLOAD_CALL,
            Canon::Str("alpha".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_string_date_payload_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_TO_STRING_DATE_PAYLOAD_CALL,
            Canon::Str("1/15/2020".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_string_error_payload_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_TO_STRING_ERROR_PAYLOAD_CALL,
            Canon::Str("Error 1234".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_to_string_decimal_payload_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_TO_STRING_DECIMAL_PAYLOAD_CALL,
            Canon::Str("12345".into()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_empty_to_string_call() {
        assert_jit_matches_vm3_contains_canon(
            JIT_VARIANT_RETURN_EMPTY_TO_STRING_CALL,
            Canon::Str(String::new()),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_return_null_to_long_error_call() {
        assert_jit_matches_vm3_raises(JIT_VARIANT_RETURN_NULL_TO_LONG_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_variant_return_null_to_string_error_call() {
        assert_jit_matches_vm3_raises(JIT_VARIANT_RETURN_NULL_TO_STRING_ERROR_CALL, 94);
    }

    #[test]
    fn jit_matches_vm3_scalar_returns_to_variant_call() {
        let vm3 = run(Executor::Vm3, JIT_SCALAR_RETURNS_TO_VARIANT_CALL);
        let jit = run(Executor::Jit, JIT_SCALAR_RETURNS_TO_VARIANT_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        for expected in [
            canon(&Variant::from_bool(true)),
            canon(&Variant::from_u8(12)),
            canon(&Variant::from_i16(12)),
            canon(&Variant::from_i64(5_000_000_012)),
            canon(&Variant::from_f32(12.5)),
            canon(&Variant::from_f64(12.5)),
            canon(&Variant::from_currency_scaled_i64(125_000)),
            canon(&Variant::from_date_f64(36527.0)),
        ] {
            assert!(
                result.contains(&expected),
                "{expected:?} missing from {result:?}"
            );
        }
    }

    #[test]
    fn jit_matches_vm3_mixed_byref_byval_long_call() {
        let vm3 = run(Executor::Vm3, JIT_MIXED_BYREF_BYVAL_LONG_CALL);
        let jit = run(Executor::Jit, JIT_MIXED_BYREF_BYVAL_LONG_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(12))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_integer_byref_call() {
        let vm3 = run(Executor::Vm3, JIT_INTEGER_BYREF_CALL);
        let jit = run(Executor::Jit, JIT_INTEGER_BYREF_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(12))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_bool_byval_call() {
        let vm3 = run(Executor::Vm3, JIT_BOOL_BYVAL_CALL);
        let jit = run(Executor::Jit, JIT_BOOL_BYVAL_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_byte_byval_call() {
        let vm3 = run(Executor::Vm3, JIT_BYTE_BYVAL_CALL);
        let jit = run(Executor::Jit, JIT_BYTE_BYVAL_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(12))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_byte_byref_call() {
        let vm3 = run(Executor::Vm3, JIT_BYTE_BYREF_CALL);
        let jit = run(Executor::Jit, JIT_BYTE_BYREF_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(12))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_integer_return_call() {
        let vm3 = run(Executor::Vm3, JIT_INTEGER_RETURN_CALL);
        let jit = run(Executor::Jit, JIT_INTEGER_RETURN_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(12))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_byte_return_call() {
        let vm3 = run(Executor::Vm3, JIT_BYTE_RETURN_CALL);
        let jit = run(Executor::Jit, JIT_BYTE_RETURN_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(12))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_integer_arithmetic() {
        let vm3 = run(Executor::Vm3, JIT_INTEGER_ARITHMETIC);
        let jit = run(Executor::Jit, JIT_INTEGER_ARITHMETIC);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i16(250))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_byte_arithmetic() {
        let vm3 = run(Executor::Vm3, JIT_BYTE_ARITHMETIC);
        let jit = run(Executor::Jit, JIT_BYTE_ARITHMETIC);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_u8(26))), "{result:?}");
    }

    #[test]
    fn jit_byte_overflow_matches_vm3_error_number() {
        let vm3 = run(Executor::Vm3, JIT_BYTE_OVERFLOW);
        let jit = run(Executor::Jit, JIT_BYTE_OVERFLOW);
        assert!(vm3.raised, "vm3 should raise overflow: {vm3:?}");
        assert!(jit.raised, "jit should raise overflow: {jit:?}");
        assert_eq!(jit.err.number, vm3.err.number);
        assert_eq!(jit.err.number, 6);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
    }

    #[test]
    fn jit_integer_overflow_matches_vm3_error_number() {
        let vm3 = run(Executor::Vm3, JIT_INTEGER_OVERFLOW);
        let jit = run(Executor::Jit, JIT_INTEGER_OVERFLOW);
        assert!(vm3.raised, "vm3 should raise overflow: {vm3:?}");
        assert!(jit.raised, "jit should raise overflow: {jit:?}");
        assert_eq!(jit.err.number, vm3.err.number);
        assert_eq!(jit.err.number, 6);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
    }

    #[test]
    fn jit_matches_vm3_longlong_byref_call() {
        let vm3 = run(Executor::Vm3, JIT_LONGLONG_BYREF_CALL);
        let jit = run(Executor::Jit, JIT_LONGLONG_BYREF_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i64(5_000_000_012))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_longlong_byval_call() {
        let vm3 = run(Executor::Vm3, JIT_LONGLONG_BYVAL_CALL);
        let jit = run(Executor::Jit, JIT_LONGLONG_BYVAL_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i64(5_000_000_012))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_longlong_return_call() {
        let vm3 = run(Executor::Vm3, JIT_LONGLONG_RETURN_CALL);
        let jit = run(Executor::Jit, JIT_LONGLONG_RETURN_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i64(5_000_000_012))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_longptr_byval_return_call() {
        assert_jit_matches_vm3_contains(
            JIT_LONGPTR_BYVAL_RETURN_CALL,
            Variant::from_i64(4_294_967_294),
        );
    }

    #[test]
    fn jit_matches_vm3_longlong_truthy_expr() {
        let vm3 = run(Executor::Vm3, JIT_LONGLONG_TRUTHY_EXPR);
        let jit = run(Executor::Jit, JIT_LONGLONG_TRUTHY_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_longlong_compare_expr() {
        let vm3 = run(Executor::Vm3, JIT_LONGLONG_COMPARE_EXPR);
        let jit = run(Executor::Jit, JIT_LONGLONG_COMPARE_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_mixed_fixed_integer_compare_expr() {
        assert_jit_matches_vm3_contains(JIT_MIXED_FIXED_INTEGER_COMPARE_EXPR, Variant::from_i32(1));
    }

    #[test]
    fn jit_matches_vm3_double_byref_call() {
        let vm3 = run(Executor::Vm3, JIT_DOUBLE_BYREF_CALL);
        let jit = run(Executor::Jit, JIT_DOUBLE_BYREF_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(12.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_double_return_call() {
        let vm3 = run(Executor::Vm3, JIT_DOUBLE_RETURN_CALL);
        let jit = run(Executor::Jit, JIT_DOUBLE_RETURN_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(12.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_double_byval_call() {
        let vm3 = run(Executor::Vm3, JIT_DOUBLE_BYVAL_CALL);
        let jit = run(Executor::Jit, JIT_DOUBLE_BYVAL_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(12.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_double_truthy_expr() {
        let vm3 = run(Executor::Vm3, JIT_DOUBLE_TRUTHY_EXPR);
        let jit = run(Executor::Jit, JIT_DOUBLE_TRUTHY_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_double_compare_expr() {
        let vm3 = run(Executor::Vm3, JIT_DOUBLE_COMPARE_EXPR);
        let jit = run(Executor::Jit, JIT_DOUBLE_COMPARE_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_double_arithmetic() {
        let vm3 = run(Executor::Vm3, JIT_DOUBLE_ARITHMETIC);
        let jit = run(Executor::Jit, JIT_DOUBLE_ARITHMETIC);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(6.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_single_byref_call() {
        let vm3 = run(Executor::Vm3, JIT_SINGLE_BYREF_CALL);
        let jit = run(Executor::Jit, JIT_SINGLE_BYREF_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f32(12.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_single_return_call() {
        let vm3 = run(Executor::Vm3, JIT_SINGLE_RETURN_CALL);
        let jit = run(Executor::Jit, JIT_SINGLE_RETURN_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f32(12.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_single_byval_call() {
        let vm3 = run(Executor::Vm3, JIT_SINGLE_BYVAL_CALL);
        let jit = run(Executor::Jit, JIT_SINGLE_BYVAL_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f32(12.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_single_truthy_expr() {
        let vm3 = run(Executor::Vm3, JIT_SINGLE_TRUTHY_EXPR);
        let jit = run(Executor::Jit, JIT_SINGLE_TRUTHY_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_single_compare_expr() {
        let vm3 = run(Executor::Vm3, JIT_SINGLE_COMPARE_EXPR);
        let jit = run(Executor::Jit, JIT_SINGLE_COMPARE_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_single_arithmetic() {
        let vm3 = run(Executor::Vm3, JIT_SINGLE_ARITHMETIC);
        let jit = run(Executor::Jit, JIT_SINGLE_ARITHMETIC);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f32(6.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_currency_byref_call() {
        let vm3 = run(Executor::Vm3, JIT_CURRENCY_BYREF_CALL);
        let jit = run(Executor::Jit, JIT_CURRENCY_BYREF_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(125_000))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_currency_return_call() {
        let vm3 = run(Executor::Vm3, JIT_CURRENCY_RETURN_CALL);
        let jit = run(Executor::Jit, JIT_CURRENCY_RETURN_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(125_000))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_currency_byval_call() {
        let vm3 = run(Executor::Vm3, JIT_CURRENCY_BYVAL_CALL);
        let jit = run(Executor::Jit, JIT_CURRENCY_BYVAL_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_currency_scaled_i64(125_000))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_date_byref_call() {
        let vm3 = run(Executor::Vm3, JIT_DATE_BYREF_CALL);
        let jit = run(Executor::Jit, JIT_DATE_BYREF_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_date_f64(36527.0))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_date_return_call() {
        let vm3 = run(Executor::Vm3, JIT_DATE_RETURN_CALL);
        let jit = run(Executor::Jit, JIT_DATE_RETURN_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_date_f64(36527.0))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_date_byval_call() {
        let vm3 = run(Executor::Vm3, JIT_DATE_BYVAL_CALL);
        let jit = run(Executor::Jit, JIT_DATE_BYVAL_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_date_f64(36527.0))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_date_truthy_expr() {
        let vm3 = run(Executor::Vm3, JIT_DATE_TRUTHY_EXPR);
        let jit = run(Executor::Jit, JIT_DATE_TRUTHY_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_date_compare_expr() {
        let vm3 = run(Executor::Vm3, JIT_DATE_COMPARE_EXPR);
        let jit = run(Executor::Jit, JIT_DATE_COMPARE_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_date_arithmetic() {
        let vm3 = run(Executor::Vm3, JIT_DATE_ARITHMETIC);
        let jit = run(Executor::Jit, JIT_DATE_ARITHMETIC);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_date_f64(36528.0))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_bool_return_call() {
        let vm3 = run(Executor::Vm3, JIT_BOOL_RETURN_CALL);
        let jit = run(Executor::Jit, JIT_BOOL_RETURN_CALL);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_bool_numeric_assignment() {
        assert_jit_matches_vm3_contains(JIT_BOOL_NUMERIC_ASSIGNMENT, Variant::from_bool(true));
    }

    #[test]
    fn jit_matches_vm3_bool_logical_expr() {
        let vm3 = run(Executor::Vm3, JIT_BOOL_LOGICAL_EXPR);
        let jit = run(Executor::Jit, JIT_BOOL_LOGICAL_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_long_logical_expr() {
        let vm3 = run(Executor::Vm3, JIT_LONG_LOGICAL_EXPR);
        let jit = run(Executor::Jit, JIT_LONG_LOGICAL_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_i32(10))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_fixed_integer_logical_expr() {
        assert_jit_matches_vm3_contains(JIT_FIXED_INTEGER_LOGICAL_EXPR, Variant::from_i32(7));
    }

    #[test]
    fn jit_matches_vm3_longlong_logical_expr() {
        assert_jit_matches_vm3_contains(JIT_LONGLONG_LOGICAL_EXPR, Variant::from_i64(4294967299));
    }

    #[test]
    fn jit_matches_vm3_longlong_eqv_expr() {
        assert_jit_matches_vm3_contains(JIT_LONGLONG_EQV_EXPR, Variant::from_i64(-5000000013));
    }

    #[test]
    fn jit_matches_vm3_longlong_not_expr() {
        assert_jit_matches_vm3_contains(JIT_LONGLONG_NOT_EXPR, Variant::from_i64(-5000000013));
    }

    #[test]
    fn jit_matches_vm3_longlong_mixed_logical_expr() {
        assert_jit_matches_vm3_contains(
            JIT_LONGLONG_MIXED_LOGICAL_EXPR,
            Variant::from_i64(10_000_000_031),
        );
    }

    #[test]
    fn jit_matches_vm3_variant_logical_expr() {
        let vm3 = run(Executor::Vm3, JIT_VARIANT_LOGICAL_EXPR);
        let jit = run(Executor::Jit, JIT_VARIANT_LOGICAL_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_bool(true))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_variant_logical_numeric_expr() {
        assert_jit_matches_vm3_contains(JIT_VARIANT_LOGICAL_NUMERIC_EXPR, Variant::from_i32(2));
    }

    #[test]
    fn jit_matches_vm3_variant_truthy_expr() {
        let vm3 = run(Executor::Vm3, JIT_VARIANT_TRUTHY_EXPR);
        let jit = run(Executor::Jit, JIT_VARIANT_TRUTHY_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(2))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_variant_compare_expr() {
        let vm3 = run(Executor::Vm3, JIT_VARIANT_COMPARE_EXPR);
        let jit = run(Executor::Jit, JIT_VARIANT_COMPARE_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(2))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_variant_compare_numeric_expr() {
        assert_jit_matches_vm3_contains(JIT_VARIANT_COMPARE_NUMERIC_EXPR, Variant::from_i32(1));
    }

    #[test]
    fn jit_matches_vm3_variant_arithmetic_null_expr() {
        assert_jit_matches_vm3_contains(JIT_VARIANT_ARITHMETIC_NULL_EXPR, Variant::null());
    }

    #[test]
    fn jit_matches_vm3_variant_arithmetic_mixed_expr() {
        assert_jit_matches_vm3_contains(JIT_VARIANT_ARITHMETIC_MIXED_EXPR, Variant::from_f64(5.5));
    }

    #[test]
    fn jit_matches_vm3_variant_negation_expr() {
        let vm3 = run(Executor::Vm3, JIT_VARIANT_NEGATION_EXPR);
        let jit = run(Executor::Jit, JIT_VARIANT_NEGATION_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(
            result.contains(&canon(&Variant::from_f64(-2.5))),
            "{result:?}"
        );
    }

    #[test]
    fn jit_matches_vm3_variant_bool_coerce_expr() {
        let vm3 = run(Executor::Vm3, JIT_VARIANT_BOOL_COERCE_EXPR);
        let jit = run(Executor::Jit, JIT_VARIANT_BOOL_COERCE_EXPR);
        assert!(vm3.unsupported.is_none(), "vm3 unsupported: {vm3:?}");
        assert!(jit.unsupported.is_none(), "jit unsupported: {jit:?}");
        assert_eq!(jit.raised, vm3.raised);
        assert_eq!(jit.err, vm3.err);
        assert_eq!(jit.result, vm3.result);
        assert!(
            jit.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit handle imbalance {:?}",
            jit.handle_balance
        );
        let result = jit.result.expect("jit result");
        assert!(result.contains(&canon(&Variant::from_i32(1))), "{result:?}");
    }

    #[test]
    fn jit_matches_vm3_variant_scalar_coerce_exprs() {
        assert_jit_matches_vm3_contains(JIT_VARIANT_BYTE_COERCE_EXPR, Variant::from_u8(12));
        assert_jit_matches_vm3_contains(JIT_VARIANT_INTEGER_COERCE_EXPR, Variant::from_i16(1234));
        assert_jit_matches_vm3_contains(JIT_VARIANT_LONG_COERCE_EXPR, Variant::from_i32(42));
        assert_jit_matches_vm3_contains(
            JIT_VARIANT_LONGLONG_COERCE_EXPR,
            Variant::from_i64(5_000_000_012),
        );
        assert_jit_matches_vm3_contains(JIT_VARIANT_SINGLE_COERCE_EXPR, Variant::from_f32(1.25));
        assert_jit_matches_vm3_contains(JIT_VARIANT_DOUBLE_COERCE_EXPR, Variant::from_f64(6.5));
        assert_jit_matches_vm3_contains(
            JIT_VARIANT_CURRENCY_COERCE_EXPR,
            Variant::from_currency_scaled_i64(125_000),
        );
        assert_jit_matches_vm3_contains(
            JIT_VARIANT_DATE_COERCE_EXPR,
            Variant::from_date_f64(36527.0),
        );
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
        // Excel/VBA returns the inserted small numeric literals as Integer variants
        // (VarType = 2), while Count is Long (VarType = 3).
        // Snapshot = [c (Object), n=Count=3, a=Item(1)=30, b=Item("k")=20].
        assert_eq!(
            snap.first(),
            Some(&Canon::Opaque { tag: 9 }),
            "c is an Object: {snap:?}"
        );
        assert!(
            snap.contains(&canon(&Variant::from_i32(3))),
            "Count==3: {snap:?}"
        );
        assert!(
            snap.contains(&canon(&Variant::from_i16(30))),
            "Item(1)==30: {snap:?}"
        );
        assert!(
            snap.contains(&canon(&Variant::from_i16(20))),
            "Item(\"k\")==20: {snap:?}"
        );
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
        assert!(
            snap.contains(&canon(&Variant::from_i32(30))),
            "total: {snap:?}"
        );
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
        assert!(
            o.handle_balance.is_some_and(HandleBalance::is_zero),
            "vm3 object handle imbalance {:?}",
            o.handle_balance
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
        assert_eq!(
            snap.first(),
            Some(&canon(&Variant::from_i32(42))),
            "{snap:?}"
        );
    }

    /// `Set w = Nothing` releases the local's object reference at the statement boundary, so
    /// `Class_Terminate` runs before the following statement even for the entry frame that is
    /// retained for the post-run snapshot.
    #[test]
    fn vm3_set_nothing_at_main_scope_runs_class_terminate() {
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
            Some(&canon(&Variant::from_i32(101))),
            "Set=Nothing at Main scope should drain Class_Terminate before the next statement: {snap:?}"
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
        assert_eq!(
            snap.first(),
            Some(&canon(&Variant::from_bool(false))),
            "{snap:?}"
        );
        assert_eq!(
            snap.get(1),
            Some(&canon(&Variant::from_bool(true))),
            "{snap:?}"
        );
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
        assert_eq!(
            snap.first(),
            Some(&canon(&Variant::from_bool(true))),
            "{snap:?}"
        );
        assert_eq!(
            snap.get(1),
            Some(&canon(&Variant::from_bool(false))),
            "{snap:?}"
        );
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
        assert_eq!(
            snap.first(),
            Some(&canon(&Variant::from_bool(false))),
            "{snap:?}"
        );
        assert_eq!(
            snap.get(1),
            Some(&canon(&Variant::from_bool(false))),
            "{snap:?}"
        );
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

    #[test]
    fn vm3_class_initialize_failure_releases_unassigned_instance() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gTerm As Long\nPublic gErr As Long\nPublic gAfter As Long\nSub Main()\n  On Error Resume Next\n  Dim w As Widget\n  Set w = New Widget\n  gErr = Err.Number\n  gAfter = gTerm\nEnd Sub\n",
            ),
            (
                "Widget",
                Class,
                "Private Sub Class_Initialize()\n  Err.Raise 5\nEnd Sub\nPrivate Sub Class_Terminate()\n  gTerm = gTerm + 1\nEnd Sub\n",
            ),
        ]);
        assert_eq!(
            snap.first(),
            Some(&canon(&Variant::from_i32(1))),
            "failed Class_Initialize should release the unassigned instance and run Terminate before the next statement: {snap:?}"
        );
        assert_eq!(
            snap.get(1),
            Some(&canon(&Variant::from_i32(5))),
            "initializer fault should remain visible through Err.Number: {snap:?}"
        );
        assert_eq!(
            snap.get(2),
            Some(&canon(&Variant::from_i32(1))),
            "statement after the caught initializer fault should observe Terminate: {snap:?}"
        );
    }

    #[test]
    fn vm3_class_initialize_failure_drains_child_fields_after_terminate() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gLog As String\nSub Main()\n  On Error Resume Next\n  Dim p As Parent\n  Set p = New Parent\n  gLog = gLog & \"A;\"\nEnd Sub\n",
            ),
            (
                "Parent",
                Class,
                "Private child As Child\nPrivate Sub Class_Initialize()\n  Set child = New Child\n  Err.Raise 5\nEnd Sub\nPrivate Sub Class_Terminate()\n  gLog = gLog & \"P;\"\nEnd Sub\n",
            ),
            (
                "Child",
                Class,
                "Private Sub Class_Terminate()\n  gLog = gLog & \"C;\"\nEnd Sub\n",
            ),
        ]);
        assert_eq!(
            snap.first(),
            Some(&canon(&Variant::from_string("P;C;A;"))),
            "failed parent initializer should run parent Terminate, then release child fields to the same drain fixpoint, before Main continues: {snap:?}"
        );
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
        assert_eq!(
            snap.first(),
            Some(&canon(&Variant::from_i32(42))),
            "{snap:?}"
        );
    }

    /// Project-instance dispatch must preserve VBA call binding semantics after the object
    /// descriptor resolves the target member: named args reorder and omitted Optional args
    /// receive their declared default.
    #[test]
    fn vm3_project_method_named_optional_args_reorder_and_default() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gResult As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  gResult = w.Combine(c:=30, a:=2)\nEnd Sub\n",
            ),
            (
                "Widget",
                Class,
                "Public Function Combine(ByVal a As Long, Optional ByVal b As Long = 7, Optional ByVal c As Long = 11) As Long\n  Combine = a * 100 + b * 10 + c\nEnd Function\n",
            ),
        ]);
        assert_eq!(
            snap.first(),
            Some(&canon(&Variant::from_i32(300))),
            "{snap:?}"
        );
    }

    /// Once a named fixed argument has been bound, later positional arguments form the
    /// ParamArray tail rather than trying to backfill fixed slots.
    #[test]
    fn vm3_project_method_named_fixed_paramarray_tail() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gResult As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  gResult = w.SumFrom(start:=5, 10, 20, 30)\nEnd Sub\n",
            ),
            (
                "Widget",
                Class,
                "Public Function SumFrom(ByVal start As Long, ParamArray xs() As Variant) As Long\n  Dim i As Long\n  SumFrom = start\n  For i = LBound(xs) To UBound(xs)\n    SumFrom = SumFrom + CLng(xs(i))\n  Next i\nEnd Function\n",
            ),
        ]);
        assert_eq!(
            snap.first(),
            Some(&canon(&Variant::from_i32(65))),
            "{snap:?}"
        );
    }

    /// Property Get calls on project instances use the same descriptor-selected dispatch path as
    /// methods, including named argument mapping and Optional defaults.
    #[test]
    fn vm3_project_property_get_named_optional_args() {
        use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};
        let snap = run_obj_ok(&[
            (
                "Main",
                Procedural,
                "Public gResult As Long\nSub Main()\n  Dim w As Widget\n  Set w = New Widget\n  gResult = w.Value(b:=8, a:=4)\nEnd Sub\n",
            ),
            (
                "Widget",
                Class,
                "Public Property Get Value(ByVal a As Long, Optional ByVal b As Long = 3) As Long\n  Value = a * 10 + b\nEnd Property\n",
            ),
        ]);
        assert_eq!(
            snap.first(),
            Some(&canon(&Variant::from_i32(48))),
            "{snap:?}"
        );
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
        assert_eq!(
            snap.first(),
            Some(&canon(&Variant::from_i32(7))),
            "{snap:?}"
        );
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
            let Ok(entries) = std::fs::read_dir(&d) else {
                continue;
            };
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
        assert!(
            o.handle_balance.is_some_and(HandleBalance::is_zero),
            "vm3 corpus handle imbalance: {:?} for outcome {o:?}",
            o.handle_balance
        );
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

    fn render_jit_scope(name: &str, source: &str) -> String {
        let outcome = run(Executor::Jit, source);
        assert!(
            outcome.handle_balance.is_some_and(HandleBalance::is_zero),
            "jit scope handle imbalance: {:?} for outcome {outcome:?}",
            outcome.handle_balance
        );
        let status = if outcome.unsupported.is_some() {
            "declined"
        } else if outcome.result.is_ok() && !outcome.raised {
            "compiled"
        } else if outcome.raised {
            "raised"
        } else {
            "failed"
        };
        format!("{name}\t{status}")
    }

    /// M4-3 JIT scope ratchet: the first compiled slice must stay compiled, while a loop stays
    /// a clean decline until the control-flow milestone intentionally blesses it.
    #[test]
    fn jit_scope_snapshot() {
        let mut lines = vec![
            render_jit_scope(
                "inline/straight_line_long_arithmetic",
                JIT_STRAIGHT_LINE_LONG,
            ),
            render_jit_scope("inline/longlong_arithmetic", JIT_LONGLONG_ARITHMETIC),
            render_jit_scope("inline/currency_arithmetic", JIT_CURRENCY_ARITHMETIC),
            render_jit_scope("inline/currency_compare_expr", JIT_CURRENCY_COMPARE_EXPR),
            render_jit_scope("inline/currency_truthy_expr", JIT_CURRENCY_TRUTHY_EXPR),
            render_jit_scope("inline/long_negation", JIT_LONG_NEGATION),
            render_jit_scope("inline/longlong_negation", JIT_LONGLONG_NEGATION),
            render_jit_scope("inline/currency_negation", JIT_CURRENCY_NEGATION),
            render_jit_scope("inline/single_negation", JIT_SINGLE_NEGATION),
            render_jit_scope("inline/double_negation", JIT_DOUBLE_NEGATION),
            render_jit_scope("inline/long_intdiv_mod", JIT_LONG_INTDIV_MOD),
            render_jit_scope("inline/longlong_intdiv_mod", JIT_LONGLONG_INTDIV_MOD),
            render_jit_scope("inline/double_division", JIT_DOUBLE_DIVISION),
            render_jit_scope("inline/double_exponentiation", JIT_DOUBLE_EXPONENTIATION),
            render_jit_scope("inline/for_loop", JIT_FOR_LOOP),
            render_jit_scope("calls/static_sub_byval_long", JIT_STATIC_SUB_CALL),
            render_jit_scope(
                "calls/consolidate_nested_call_chain",
                JIT_CONSOLIDATE_NESTED_CALL_CHAIN,
            ),
            render_jit_scope("calls/gosub_basic", JIT_GOSUB_BASIC),
            render_jit_scope("calls/gosub_repeated", JIT_GOSUB_REPEATED),
            render_jit_scope("control/gosub_nested_labels", JIT_GOSUB_NESTED_LABELS),
            render_jit_scope("control/gosub_loop_accumulate", JIT_GOSUB_LOOP_ACCUMULATE),
            render_jit_scope(
                "control/consolidate_for_gosub_mix",
                JIT_CONSOLIDATE_FOR_GOSUB_MIX,
            ),
            render_jit_scope(
                "control/consolidate_for_select_call",
                JIT_CONSOLIDATE_FOR_SELECT_CALL,
            ),
            render_jit_scope(
                "control/consolidate_while_byref_mix",
                JIT_CONSOLIDATE_WHILE_BYREF_MIX,
            ),
            render_jit_scope(
                "error/error_resume_function_propagation",
                JIT_ERROR_RESUME_FUNCTION_PROPAGATION,
            ),
            render_jit_scope(
                "error/consolidate_gosub_error_mix",
                JIT_CONSOLIDATE_GOSUB_ERROR_MIX,
            ),
            render_jit_scope(
                "error/gosub_return_without_gosub",
                JIT_GOSUB_RETURN_WITHOUT_GOSUB,
            ),
            render_jit_scope(
                "error/gosub_return_without_gosub_resume_next",
                JIT_GOSUB_RETURN_WITHOUT_GOSUB_RESUME_NEXT,
            ),
            render_jit_scope(
                "error/gosub_return_without_gosub_label_handler",
                JIT_GOSUB_RETURN_WITHOUT_GOSUB_LABEL_HANDLER,
            ),
            render_jit_scope(
                "error/error_nested_mode_transitions",
                JIT_ERROR_NESTED_MODE_TRANSITIONS,
            ),
            render_jit_scope("params/paramarray_pack_ubound", JIT_PARAMARRAY_UBOUND_PACK),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_pack",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_PACK,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_alias_copyout",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_parenthesized_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_byval_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_array_element_alias_copyout",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_ARRAY_ELEMENT_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_array_element_parenthesized_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_ARRAY_ELEMENT_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_array_element_byval_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_ARRAY_ELEMENT_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_duplicate_alias_copyout",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_DUPLICATE_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_duplicate_parenthesized_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_DUPLICATE_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_duplicate_byval_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_DUPLICATE_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_alias_copyout",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_parenthesized_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_byval_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_string_alias_copyout",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_STRING_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_string_parenthesized_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_STRING_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_string_byval_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_STRING_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_longptr_alias_copyout",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_LONGPTR_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_longptr_parenthesized_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_LONGPTR_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_longptr_byval_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_LONGPTR_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_typed_scalar_alias_bundle_copyout",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_typed_scalar_parenthesized_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_typed_scalar_byval_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_fixed_string_alias_copyout",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_FIXED_STRING_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_fixed_string_parenthesized_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_FIXED_STRING_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_global_fixed_string_byval_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_GLOBAL_FIXED_STRING_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_typed_long_string_alias_copyout",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_typed_long_string_parenthesized_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_typed_long_string_byval_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONG_STRING_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_typed_longptr_alias_copyout",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONGPTR_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_typed_longptr_parenthesized_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONGPTR_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_typed_longptr_byval_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_LONGPTR_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_typed_scalar_alias_bundle_copyout",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_ALIAS_BUNDLE_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_typed_scalar_parenthesized_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_typed_scalar_byval_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_SCALAR_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_typed_fixed_string_alias_copyout",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_FIXED_STRING_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_typed_fixed_string_parenthesized_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_FIXED_STRING_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_named_fixed_tail_typed_fixed_string_byval_no_alias",
                JIT_PARAMARRAY_NAMED_FIXED_TAIL_TYPED_FIXED_STRING_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_empty_ubound",
                JIT_PARAMARRAY_UBOUND_EMPTY,
            ),
            render_jit_scope(
                "params/paramarray_omitted_tail_empty",
                JIT_PARAMARRAY_OMITTED_TAIL_EMPTY,
            ),
            render_jit_scope(
                "params/paramarray_alias_copyout",
                JIT_PARAMARRAY_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_array_element_alias_copyout",
                JIT_PARAMARRAY_ARRAY_ELEMENT_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_duplicate_alias_copyout",
                JIT_PARAMARRAY_DUPLICATE_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_parenthesized_no_alias",
                JIT_PARAMARRAY_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_byval_no_alias",
                JIT_PARAMARRAY_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_variant_array_element_mutation",
                JIT_PARAMARRAY_VARIANT_ARRAY_ELEMENT_MUTATION,
            ),
            render_jit_scope(
                "params/paramarray_global_alias_copyout",
                JIT_PARAMARRAY_GLOBAL_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_global_byval_no_alias",
                JIT_PARAMARRAY_GLOBAL_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_global_parenthesized_no_alias",
                JIT_PARAMARRAY_GLOBAL_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_global_string_alias_copyout",
                JIT_PARAMARRAY_GLOBAL_STRING_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_global_string_byval_no_alias",
                JIT_PARAMARRAY_GLOBAL_STRING_BYVAL_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_global_string_parenthesized_no_alias",
                JIT_PARAMARRAY_GLOBAL_STRING_PARENTHESIZED_NO_ALIAS,
            ),
            render_jit_scope(
                "params/paramarray_typed_scalar_alias_copyout",
                JIT_PARAMARRAY_TYPED_SCALAR_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_typed_longlong_alias_copyout",
                JIT_PARAMARRAY_TYPED_LONGLONG_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_typed_longptr_alias_copyout",
                JIT_PARAMARRAY_TYPED_LONGPTR_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_typed_integer_alias_copyout",
                JIT_PARAMARRAY_TYPED_INTEGER_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_typed_byte_alias_copyout",
                JIT_PARAMARRAY_TYPED_BYTE_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_typed_boolean_alias_copyout",
                JIT_PARAMARRAY_TYPED_BOOLEAN_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_typed_string_alias_copyout",
                JIT_PARAMARRAY_TYPED_STRING_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_typed_fixed_string_alias_copyout",
                JIT_PARAMARRAY_TYPED_FIXED_STRING_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_typed_currency_alias_copyout",
                JIT_PARAMARRAY_TYPED_CURRENCY_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_typed_single_alias_copyout",
                JIT_PARAMARRAY_TYPED_SINGLE_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_typed_double_alias_copyout",
                JIT_PARAMARRAY_TYPED_DOUBLE_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_typed_date_alias_copyout",
                JIT_PARAMARRAY_TYPED_DATE_ALIAS_COPYOUT,
            ),
            render_jit_scope(
                "params/paramarray_bounds_explicit_dim",
                JIT_PARAMARRAY_BOUNDS_EXPLICIT_DIM,
            ),
            render_jit_scope(
                "params/paramarray_option_base_one_bounds",
                JIT_PARAMARRAY_OPTION_BASE_ONE_BOUNDS,
            ),
            render_jit_scope(
                "params/paramarray_lbound_dim_zero_error",
                JIT_PARAMARRAY_LBOUND_DIM_ZERO_ERROR,
            ),
            render_jit_scope(
                "params/paramarray_ubound_dim_too_high_error",
                JIT_PARAMARRAY_UBOUND_DIM_TOO_HIGH_ERROR,
            ),
            render_jit_scope("arrays/literal_bounds", JIT_ARRAY_LITERAL_BOUNDS),
            render_jit_scope(
                "arrays/literal_bounds_explicit_dim",
                JIT_ARRAY_LITERAL_BOUNDS_EXPLICIT_DIM,
            ),
            render_jit_scope(
                "arrays/literal_bounds_dim_zero_error",
                JIT_ARRAY_LITERAL_BOUNDS_DIM_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/literal_bounds_dim_too_high_error",
                JIT_ARRAY_LITERAL_BOUNDS_DIM_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/dynamic_lbound_unallocated_error",
                JIT_ARRAY_DYNAMIC_LBOUND_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/dynamic_ubound_unallocated_error",
                JIT_ARRAY_DYNAMIC_UBOUND_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/dynamic_bounds_explicit_dim",
                JIT_ARRAY_DYNAMIC_BOUNDS_EXPLICIT_DIM,
            ),
            render_jit_scope(
                "arrays/dynamic_lbound_dim_zero_error",
                JIT_ARRAY_DYNAMIC_LBOUND_DIM_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/dynamic_ubound_dim_too_high_error",
                JIT_ARRAY_DYNAMIC_UBOUND_DIM_TOO_HIGH_ERROR,
            ),
            render_jit_scope("arrays/dynamic_types", JIT_ARRAY_DYNAMIC_TYPES),
            render_jit_scope("arrays/fixed_information", JIT_ARRAY_FIXED_INFORMATION),
            render_jit_scope(
                "arrays/fixed_information_after_erase",
                JIT_ARRAY_FIXED_INFORMATION_AFTER_ERASE,
            ),
            render_jit_scope(
                "arrays/fixed_bounds_explicit_dim",
                JIT_ARRAY_FIXED_BOUNDS_EXPLICIT_DIM,
            ),
            render_jit_scope(
                "arrays/fixed_lbound_dim_zero_error",
                JIT_ARRAY_FIXED_LBOUND_DIM_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/fixed_ubound_dim_too_high_error",
                JIT_ARRAY_FIXED_UBOUND_DIM_TOO_HIGH_ERROR,
            ),
            render_jit_scope("arrays/multidim_indexing", JIT_ARRAY_MULTIDIM_INDEXING),
            render_jit_scope("arrays/literal_types", JIT_ARRAY_LITERAL_TYPES),
            render_jit_scope("arrays/zero_index", JIT_ARRAY_ZERO_INDEX),
            render_jit_scope("arrays/store_load", JIT_ARRAY_STORE_LOAD),
            render_jit_scope(
                "arrays/long_typed_store_load",
                JIT_ARRAY_LONG_TYPED_STORE_LOAD,
            ),
            render_jit_scope(
                "arrays/long_read_to_variant",
                JIT_ARRAY_LONG_READ_TO_VARIANT,
            ),
            render_jit_scope(
                "arrays/string_typed_store_load",
                JIT_ARRAY_STRING_TYPED_STORE_LOAD,
            ),
            render_jit_scope(
                "arrays/fixed_string_typed_store_load",
                JIT_ARRAY_FIXED_STRING_TYPED_STORE_LOAD,
            ),
            render_jit_scope(
                "arrays/fixed_string_dynamic_typed_store_load",
                JIT_ARRAY_FIXED_STRING_DYNAMIC_TYPED_STORE_LOAD,
            ),
            render_jit_scope(
                "arrays/fixed_string_multidim_typed_store_load",
                JIT_ARRAY_FIXED_STRING_MULTIDIM_TYPED_STORE_LOAD,
            ),
            render_jit_scope(
                "arrays/fixed_string_dynamic_multidim_typed_store_load",
                JIT_ARRAY_FIXED_STRING_DYNAMIC_MULTIDIM_TYPED_STORE_LOAD,
            ),
            render_jit_scope(
                "arrays/fixed_string_3d_typed_store_load",
                JIT_ARRAY_FIXED_STRING_3D_TYPED_STORE_LOAD,
            ),
            render_jit_scope(
                "arrays/fixed_string_dynamic_3d_typed_store_load",
                JIT_ARRAY_FIXED_STRING_DYNAMIC_3D_TYPED_STORE_LOAD,
            ),
            render_jit_scope(
                "arrays/fixed_string_4d_typed_store_load",
                JIT_ARRAY_FIXED_STRING_4D_TYPED_STORE_LOAD,
            ),
            render_jit_scope(
                "arrays/fixed_string_dynamic_4d_typed_store_load",
                JIT_ARRAY_FIXED_STRING_DYNAMIC_4D_TYPED_STORE_LOAD,
            ),
            render_jit_scope(
                "arrays/typed_scalar_store_load_bundle",
                JIT_ARRAY_TYPED_SCALAR_STORE_LOAD_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_dynamic_store_load_bundle",
                JIT_ARRAY_TYPED_SCALAR_DYNAMIC_STORE_LOAD_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_multidim_store_load_bundle",
                JIT_ARRAY_TYPED_SCALAR_MULTIDIM_STORE_LOAD_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_dynamic_multidim_store_load_bundle",
                JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_STORE_LOAD_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_3d_store_load_bundle",
                JIT_ARRAY_TYPED_SCALAR_3D_STORE_LOAD_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_dynamic_3d_store_load_bundle",
                JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_STORE_LOAD_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_4d_store_load_bundle",
                JIT_ARRAY_TYPED_SCALAR_4D_STORE_LOAD_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_dynamic_4d_store_load_bundle",
                JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_STORE_LOAD_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_multidim_bounds_bundle",
                JIT_ARRAY_TYPED_SCALAR_MULTIDIM_BOUNDS_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_dynamic_multidim_bounds_bundle",
                JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_BOUNDS_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_multidim_bounds_dim_expr_bundle",
                JIT_ARRAY_TYPED_SCALAR_MULTIDIM_BOUNDS_DIM_EXPR_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_dynamic_multidim_bounds_dim_expr_bundle",
                JIT_ARRAY_TYPED_SCALAR_DYNAMIC_MULTIDIM_BOUNDS_DIM_EXPR_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_3d_bounds_bundle",
                JIT_ARRAY_TYPED_SCALAR_3D_BOUNDS_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_dynamic_3d_bounds_bundle",
                JIT_ARRAY_TYPED_SCALAR_DYNAMIC_3D_BOUNDS_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_4d_bounds_bundle",
                JIT_ARRAY_TYPED_SCALAR_4D_BOUNDS_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_scalar_dynamic_4d_bounds_bundle",
                JIT_ARRAY_TYPED_SCALAR_DYNAMIC_4D_BOUNDS_BUNDLE,
            ),
            render_jit_scope(
                "arrays/typed_long_3d_lbound_dim_zero_error",
                JIT_ARRAY_TYPED_LONG_3D_LBOUND_DIM_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_3d_ubound_dim_too_high_error",
                JIT_ARRAY_TYPED_LONG_3D_UBOUND_DIM_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_dynamic_3d_lbound_dim_zero_error",
                JIT_ARRAY_TYPED_LONG_DYNAMIC_3D_LBOUND_DIM_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_dynamic_3d_ubound_dim_too_high_error",
                JIT_ARRAY_TYPED_LONG_DYNAMIC_3D_UBOUND_DIM_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_3d_lbound_dim_expr_zero_error",
                JIT_ARRAY_TYPED_LONG_3D_LBOUND_DIM_EXPR_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_3d_ubound_dim_expr_too_high_error",
                JIT_ARRAY_TYPED_LONG_3D_UBOUND_DIM_EXPR_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_dynamic_3d_lbound_dim_expr_zero_error",
                JIT_ARRAY_TYPED_LONG_DYNAMIC_3D_LBOUND_DIM_EXPR_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_dynamic_3d_ubound_dim_expr_too_high_error",
                JIT_ARRAY_TYPED_LONG_DYNAMIC_3D_UBOUND_DIM_EXPR_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_4d_lbound_dim_zero_error",
                JIT_ARRAY_TYPED_LONG_4D_LBOUND_DIM_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_4d_ubound_dim_too_high_error",
                JIT_ARRAY_TYPED_LONG_4D_UBOUND_DIM_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_dynamic_4d_lbound_dim_zero_error",
                JIT_ARRAY_TYPED_LONG_DYNAMIC_4D_LBOUND_DIM_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_dynamic_4d_ubound_dim_too_high_error",
                JIT_ARRAY_TYPED_LONG_DYNAMIC_4D_UBOUND_DIM_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_4d_lbound_dim_expr_zero_error",
                JIT_ARRAY_TYPED_LONG_4D_LBOUND_DIM_EXPR_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_4d_ubound_dim_expr_too_high_error",
                JIT_ARRAY_TYPED_LONG_4D_UBOUND_DIM_EXPR_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_dynamic_4d_lbound_dim_expr_zero_error",
                JIT_ARRAY_TYPED_LONG_DYNAMIC_4D_LBOUND_DIM_EXPR_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_dynamic_4d_ubound_dim_expr_too_high_error",
                JIT_ARRAY_TYPED_LONG_DYNAMIC_4D_UBOUND_DIM_EXPR_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_multidim_lbound_dim_zero_error",
                JIT_ARRAY_TYPED_LONG_MULTIDIM_LBOUND_DIM_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_multidim_ubound_dim_too_high_error",
                JIT_ARRAY_TYPED_LONG_MULTIDIM_UBOUND_DIM_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_dynamic_multidim_lbound_dim_zero_error",
                JIT_ARRAY_TYPED_LONG_DYNAMIC_MULTIDIM_LBOUND_DIM_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_dynamic_multidim_ubound_dim_too_high_error",
                JIT_ARRAY_TYPED_LONG_DYNAMIC_MULTIDIM_UBOUND_DIM_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_multidim_lbound_dim_expr_zero_error",
                JIT_ARRAY_TYPED_LONG_MULTIDIM_LBOUND_DIM_EXPR_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_multidim_ubound_dim_expr_too_high_error",
                JIT_ARRAY_TYPED_LONG_MULTIDIM_UBOUND_DIM_EXPR_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_dynamic_multidim_lbound_dim_expr_zero_error",
                JIT_ARRAY_TYPED_LONG_DYNAMIC_MULTIDIM_LBOUND_DIM_EXPR_ZERO_ERROR,
            ),
            render_jit_scope(
                "arrays/typed_long_dynamic_multidim_ubound_dim_expr_too_high_error",
                JIT_ARRAY_TYPED_LONG_DYNAMIC_MULTIDIM_UBOUND_DIM_EXPR_TOO_HIGH_ERROR,
            ),
            render_jit_scope(
                "arrays/explicit_lower_bound",
                JIT_ARRAY_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "arrays/option_base_one_bounds",
                JIT_ARRAY_OPTION_BASE_ONE_BOUNDS,
            ),
            render_jit_scope("arrays/bounds_error", JIT_ARRAY_BOUNDS_ERROR),
            render_jit_scope("arrays/redim_expand", JIT_ARRAY_REDIM_EXPAND),
            render_jit_scope(
                "arrays/redim_without_preserve_resets",
                JIT_ARRAY_REDIM_WITHOUT_PRESERVE_RESETS,
            ),
            render_jit_scope(
                "arrays/redim_shrink_bounds_error",
                JIT_ARRAY_REDIM_SHRINK_BOUNDS_ERROR,
            ),
            render_jit_scope(
                "arrays/redim_upper_less_than_lower_error",
                JIT_ARRAY_REDIM_UPPER_LESS_THAN_LOWER_ERROR,
            ),
            render_jit_scope(
                "arrays/redim_preserve_upper_less_than_lower_error",
                JIT_ARRAY_REDIM_PRESERVE_UPPER_LESS_THAN_LOWER_ERROR,
            ),
            render_jit_scope(
                "arrays/redim_negative_lower_bound",
                JIT_ARRAY_REDIM_NEGATIVE_LOWER_BOUND,
            ),
            render_jit_scope(
                "arrays/redim_dynamic_bound_expression",
                JIT_ARRAY_REDIM_DYNAMIC_BOUND_EXPRESSION,
            ),
            render_jit_scope(
                "arrays/redim_option_base_one_bounds",
                JIT_ARRAY_REDIM_OPTION_BASE_ONE_BOUNDS,
            ),
            render_jit_scope(
                "arrays/redim_fixed_variant_array_error",
                JIT_ARRAY_REDIM_FIXED_VARIANT_ARRAY_ERROR,
            ),
            render_jit_scope(
                "arrays/redim_preserve_keeps_values",
                JIT_ARRAY_REDIM_PRESERVE_KEEPS_VALUES,
            ),
            render_jit_scope(
                "arrays/redim_preserve_unallocated_defaults",
                JIT_ARRAY_REDIM_PRESERVE_UNALLOCATED_DEFAULTS,
            ),
            render_jit_scope(
                "arrays/redim_preserve_explicit_lower_keeps_value",
                JIT_ARRAY_REDIM_PRESERVE_EXPLICIT_LOWER_KEEPS_VALUE,
            ),
            render_jit_scope(
                "arrays/redim_preserve_shrink_expand_clears_tail",
                JIT_ARRAY_REDIM_PRESERVE_SHRINK_EXPAND_CLEARS_TAIL,
            ),
            render_jit_scope(
                "arrays/redim_preserve_lower_bound_change_error",
                JIT_ARRAY_REDIM_PRESERVE_LOWER_BOUND_CHANGE_ERROR,
            ),
            render_jit_scope(
                "arrays/redim_preserve_fixed_variant_array_error",
                JIT_ARRAY_REDIM_PRESERVE_FIXED_VARIANT_ARRAY_ERROR,
            ),
            render_jit_scope(
                "arrays/redim_preserve_multidim_last_dimension",
                JIT_ARRAY_REDIM_PRESERVE_MULTIDIM_LAST_DIMENSION,
            ),
            render_jit_scope(
                "arrays/redim_preserve_illegal_non_last_dim_error",
                JIT_ARRAY_REDIM_PRESERVE_ILLEGAL_NON_LAST_DIM_ERROR,
            ),
            render_jit_scope(
                "control/for_each_array_dynamic",
                JIT_FOR_EACH_ARRAY_DYNAMIC_BASIC,
            ),
            render_jit_scope(
                "control/for_each_array_dynamic_explicit_lower_bound",
                JIT_FOR_EACH_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_array_dynamic_item_after_completion",
                JIT_FOR_EACH_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_array_literal",
                JIT_FOR_EACH_ARRAY_LITERAL_BASIC,
            ),
            render_jit_scope(
                "control/for_each_array_literal_empty_skips",
                JIT_FOR_EACH_ARRAY_LITERAL_EMPTY_SKIPS,
            ),
            render_jit_scope(
                "control/for_each_array_literal_item_after_completion",
                JIT_FOR_EACH_ARRAY_LITERAL_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_array_variable",
                JIT_FOR_EACH_ARRAY_VARIABLE_BASIC,
            ),
            render_jit_scope(
                "control/for_each_array_variable_explicit_lower_bound",
                JIT_FOR_EACH_ARRAY_VARIABLE_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_array_variable_item_after_completion",
                JIT_FOR_EACH_ARRAY_VARIABLE_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_boolean_array_dynamic",
                JIT_FOR_EACH_BOOLEAN_ARRAY_DYNAMIC_BASIC,
            ),
            render_jit_scope(
                "control/for_each_boolean_array_dynamic_explicit_lower_bound",
                JIT_FOR_EACH_BOOLEAN_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_boolean_array_dynamic_item_after_completion",
                JIT_FOR_EACH_BOOLEAN_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_boolean_array_dynamic_multidim_order",
                JIT_FOR_EACH_BOOLEAN_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_boolean_array_fixed",
                JIT_FOR_EACH_BOOLEAN_ARRAY_FIXED_BASIC,
            ),
            render_jit_scope(
                "control/for_each_boolean_array_fixed_explicit_lower_bound",
                JIT_FOR_EACH_BOOLEAN_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_boolean_array_fixed_item_after_completion",
                JIT_FOR_EACH_BOOLEAN_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_boolean_array_fixed_multidim_order",
                JIT_FOR_EACH_BOOLEAN_ARRAY_FIXED_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_byte_array_dynamic",
                JIT_FOR_EACH_BYTE_ARRAY_DYNAMIC_BASIC,
            ),
            render_jit_scope(
                "control/for_each_byte_array_dynamic_explicit_lower_bound",
                JIT_FOR_EACH_BYTE_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_byte_array_dynamic_item_after_completion",
                JIT_FOR_EACH_BYTE_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_byte_array_dynamic_multidim_order",
                JIT_FOR_EACH_BYTE_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_byte_array_fixed",
                JIT_FOR_EACH_BYTE_ARRAY_FIXED_BASIC,
            ),
            render_jit_scope(
                "control/for_each_byte_array_fixed_explicit_lower_bound",
                JIT_FOR_EACH_BYTE_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_byte_array_fixed_item_after_completion",
                JIT_FOR_EACH_BYTE_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_byte_array_fixed_multidim_order",
                JIT_FOR_EACH_BYTE_ARRAY_FIXED_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_integer_array_dynamic",
                JIT_FOR_EACH_INTEGER_ARRAY_DYNAMIC_BASIC,
            ),
            render_jit_scope(
                "control/for_each_integer_array_dynamic_explicit_lower_bound",
                JIT_FOR_EACH_INTEGER_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_integer_array_dynamic_item_after_completion",
                JIT_FOR_EACH_INTEGER_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_integer_array_dynamic_multidim_order",
                JIT_FOR_EACH_INTEGER_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_integer_array_fixed",
                JIT_FOR_EACH_INTEGER_ARRAY_FIXED_BASIC,
            ),
            render_jit_scope(
                "control/for_each_integer_array_fixed_explicit_lower_bound",
                JIT_FOR_EACH_INTEGER_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_integer_array_fixed_item_after_completion",
                JIT_FOR_EACH_INTEGER_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_integer_array_fixed_multidim_order",
                JIT_FOR_EACH_INTEGER_ARRAY_FIXED_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_long_array_dynamic",
                JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_BASIC,
            ),
            render_jit_scope(
                "control/for_each_long_array_dynamic_explicit_lower_bound",
                JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_long_array_dynamic_item_after_completion",
                JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_long_array_dynamic_multidim_order",
                JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_long_array_dynamic_3d_order",
                JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_3D_ORDER,
            ),
            render_jit_scope(
                "control/for_each_long_array_dynamic_4d_order",
                JIT_FOR_EACH_LONG_ARRAY_DYNAMIC_4D_ORDER,
            ),
            render_jit_scope(
                "control/for_each_long_array_fixed",
                JIT_FOR_EACH_LONG_ARRAY_FIXED_BASIC,
            ),
            render_jit_scope(
                "control/for_each_long_array_fixed_explicit_lower_bound",
                JIT_FOR_EACH_LONG_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_long_array_fixed_item_after_completion",
                JIT_FOR_EACH_LONG_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_long_array_fixed_multidim_order",
                JIT_FOR_EACH_LONG_ARRAY_FIXED_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_long_array_fixed_3d_order",
                JIT_FOR_EACH_LONG_ARRAY_FIXED_3D_ORDER,
            ),
            render_jit_scope(
                "control/for_each_long_array_fixed_4d_order",
                JIT_FOR_EACH_LONG_ARRAY_FIXED_4D_ORDER,
            ),
            render_jit_scope(
                "control/for_each_typed_scalar_3d_order_bundle",
                JIT_FOR_EACH_TYPED_SCALAR_3D_ORDER_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_typed_scalar_dynamic_3d_order_bundle",
                JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_3D_ORDER_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_typed_scalar_multidim_item_after_completion_bundle",
                JIT_FOR_EACH_TYPED_SCALAR_MULTIDIM_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_typed_scalar_dynamic_multidim_item_after_completion_bundle",
                JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_MULTIDIM_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_typed_scalar_3d_item_after_completion_bundle",
                JIT_FOR_EACH_TYPED_SCALAR_3D_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_typed_scalar_dynamic_3d_item_after_completion_bundle",
                JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_3D_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_typed_scalar_4d_order_bundle",
                JIT_FOR_EACH_TYPED_SCALAR_4D_ORDER_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_typed_scalar_dynamic_4d_order_bundle",
                JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_4D_ORDER_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_typed_scalar_4d_item_after_completion_bundle",
                JIT_FOR_EACH_TYPED_SCALAR_4D_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_typed_scalar_dynamic_4d_item_after_completion_bundle",
                JIT_FOR_EACH_TYPED_SCALAR_DYNAMIC_4D_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_longlong_array_dynamic",
                JIT_FOR_EACH_LONGLONG_ARRAY_DYNAMIC_BASIC,
            ),
            render_jit_scope(
                "control/for_each_longlong_array_dynamic_explicit_lower_bound",
                JIT_FOR_EACH_LONGLONG_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_longlong_array_dynamic_item_after_completion",
                JIT_FOR_EACH_LONGLONG_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_longlong_array_dynamic_multidim_order",
                JIT_FOR_EACH_LONGLONG_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_longlong_array_fixed",
                JIT_FOR_EACH_LONGLONG_ARRAY_FIXED_BASIC,
            ),
            render_jit_scope(
                "control/for_each_longlong_array_fixed_explicit_lower_bound",
                JIT_FOR_EACH_LONGLONG_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_longlong_array_fixed_item_after_completion",
                JIT_FOR_EACH_LONGLONG_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_longlong_array_fixed_multidim_order",
                JIT_FOR_EACH_LONGLONG_ARRAY_FIXED_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_single_array_dynamic",
                JIT_FOR_EACH_SINGLE_ARRAY_DYNAMIC_BASIC,
            ),
            render_jit_scope(
                "control/for_each_single_array_dynamic_explicit_lower_bound",
                JIT_FOR_EACH_SINGLE_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_single_array_dynamic_item_after_completion",
                JIT_FOR_EACH_SINGLE_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_single_array_dynamic_multidim_order",
                JIT_FOR_EACH_SINGLE_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_single_array_fixed",
                JIT_FOR_EACH_SINGLE_ARRAY_FIXED_BASIC,
            ),
            render_jit_scope(
                "control/for_each_single_array_fixed_explicit_lower_bound",
                JIT_FOR_EACH_SINGLE_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_single_array_fixed_item_after_completion",
                JIT_FOR_EACH_SINGLE_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_single_array_fixed_multidim_order",
                JIT_FOR_EACH_SINGLE_ARRAY_FIXED_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_double_array_dynamic",
                JIT_FOR_EACH_DOUBLE_ARRAY_DYNAMIC_BASIC,
            ),
            render_jit_scope(
                "control/for_each_double_array_dynamic_explicit_lower_bound",
                JIT_FOR_EACH_DOUBLE_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_double_array_dynamic_item_after_completion",
                JIT_FOR_EACH_DOUBLE_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_double_array_dynamic_multidim_order",
                JIT_FOR_EACH_DOUBLE_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_double_array_fixed",
                JIT_FOR_EACH_DOUBLE_ARRAY_FIXED_BASIC,
            ),
            render_jit_scope(
                "control/for_each_double_array_fixed_explicit_lower_bound",
                JIT_FOR_EACH_DOUBLE_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_double_array_fixed_item_after_completion",
                JIT_FOR_EACH_DOUBLE_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_double_array_fixed_multidim_order",
                JIT_FOR_EACH_DOUBLE_ARRAY_FIXED_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_currency_array_dynamic",
                JIT_FOR_EACH_CURRENCY_ARRAY_DYNAMIC_BASIC,
            ),
            render_jit_scope(
                "control/for_each_currency_array_dynamic_explicit_lower_bound",
                JIT_FOR_EACH_CURRENCY_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_currency_array_dynamic_item_after_completion",
                JIT_FOR_EACH_CURRENCY_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_currency_array_dynamic_multidim_order",
                JIT_FOR_EACH_CURRENCY_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_currency_array_fixed",
                JIT_FOR_EACH_CURRENCY_ARRAY_FIXED_BASIC,
            ),
            render_jit_scope(
                "control/for_each_currency_array_fixed_explicit_lower_bound",
                JIT_FOR_EACH_CURRENCY_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_currency_array_fixed_item_after_completion",
                JIT_FOR_EACH_CURRENCY_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_currency_array_fixed_multidim_order",
                JIT_FOR_EACH_CURRENCY_ARRAY_FIXED_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_date_array_dynamic",
                JIT_FOR_EACH_DATE_ARRAY_DYNAMIC_BASIC,
            ),
            render_jit_scope(
                "control/for_each_date_array_dynamic_explicit_lower_bound",
                JIT_FOR_EACH_DATE_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_date_array_dynamic_item_after_completion",
                JIT_FOR_EACH_DATE_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_date_array_dynamic_multidim_order",
                JIT_FOR_EACH_DATE_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_date_array_fixed",
                JIT_FOR_EACH_DATE_ARRAY_FIXED_BASIC,
            ),
            render_jit_scope(
                "control/for_each_date_array_fixed_explicit_lower_bound",
                JIT_FOR_EACH_DATE_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_date_array_fixed_item_after_completion",
                JIT_FOR_EACH_DATE_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_date_array_fixed_multidim_order",
                JIT_FOR_EACH_DATE_ARRAY_FIXED_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_string_array_dynamic",
                JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_BASIC,
            ),
            render_jit_scope(
                "control/for_each_string_array_dynamic_explicit_lower_bound",
                JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_string_array_dynamic_item_after_completion",
                JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_string_array_dynamic_multidim_order",
                JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_string_array_dynamic_4d_order",
                JIT_FOR_EACH_STRING_ARRAY_DYNAMIC_4D_ORDER,
            ),
            render_jit_scope(
                "control/for_each_string_array_fixed",
                JIT_FOR_EACH_STRING_ARRAY_FIXED_BASIC,
            ),
            render_jit_scope(
                "control/for_each_string_array_fixed_explicit_lower_bound",
                JIT_FOR_EACH_STRING_ARRAY_FIXED_EXPLICIT_LOWER_BOUND,
            ),
            render_jit_scope(
                "control/for_each_string_array_fixed_item_after_completion",
                JIT_FOR_EACH_STRING_ARRAY_FIXED_ITEM_AFTER_COMPLETION,
            ),
            render_jit_scope(
                "control/for_each_string_array_fixed_multidim_order",
                JIT_FOR_EACH_STRING_ARRAY_FIXED_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_string_array_fixed_4d_order",
                JIT_FOR_EACH_STRING_ARRAY_FIXED_4D_ORDER,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_dynamic",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_BASIC,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_dynamic_item_after_completion_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_dynamic_width_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_WIDTH_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_dynamic_multidim_item_after_completion_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_MULTIDIM_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_dynamic_multidim_width_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_MULTIDIM_WIDTH_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_dynamic_multidim_order",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_dynamic_3d_order",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_3D_ORDER,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_dynamic_3d_item_after_completion_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_3D_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_dynamic_3d_width_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_3D_WIDTH_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_dynamic_4d_order",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_4D_ORDER,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_dynamic_4d_item_after_completion_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_4D_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_dynamic_4d_width_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_DYNAMIC_4D_WIDTH_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_fixed",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_BASIC,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_fixed_item_after_completion_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_fixed_width_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_WIDTH_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_fixed_multidim_item_after_completion_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_MULTIDIM_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_fixed_multidim_width_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_MULTIDIM_WIDTH_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_fixed_multidim_order",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_MULTIDIM_ORDER,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_fixed_3d_order",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_3D_ORDER,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_fixed_3d_item_after_completion_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_3D_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_fixed_3d_width_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_3D_WIDTH_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_fixed_4d_order",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_4D_ORDER,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_fixed_4d_item_after_completion_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_4D_ITEM_AFTER_COMPLETION_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_array_fixed_4d_width_bundle",
                JIT_FOR_EACH_FIXED_STRING_ARRAY_FIXED_4D_WIDTH_BUNDLE,
            ),
            render_jit_scope(
                "control/for_each_boolean_scalar_error",
                JIT_FOR_EACH_BOOLEAN_SCALAR_ERROR,
            ),
            render_jit_scope(
                "control/for_each_byte_scalar_error",
                JIT_FOR_EACH_BYTE_SCALAR_ERROR,
            ),
            render_jit_scope(
                "control/for_each_currency_scalar_error",
                JIT_FOR_EACH_CURRENCY_SCALAR_ERROR,
            ),
            render_jit_scope(
                "control/for_each_date_scalar_error",
                JIT_FOR_EACH_DATE_SCALAR_ERROR,
            ),
            render_jit_scope(
                "control/for_each_double_scalar_error",
                JIT_FOR_EACH_DOUBLE_SCALAR_ERROR,
            ),
            render_jit_scope(
                "control/for_each_fixed_string_scalar_error",
                JIT_FOR_EACH_FIXED_STRING_SCALAR_ERROR,
            ),
            render_jit_scope(
                "control/for_each_integer_scalar_error",
                JIT_FOR_EACH_INTEGER_SCALAR_ERROR,
            ),
            render_jit_scope(
                "control/for_each_long_scalar_error",
                JIT_FOR_EACH_LONG_SCALAR_ERROR,
            ),
            render_jit_scope(
                "control/for_each_longlong_scalar_error",
                JIT_FOR_EACH_LONGLONG_SCALAR_ERROR,
            ),
            render_jit_scope(
                "control/for_each_single_scalar_error",
                JIT_FOR_EACH_SINGLE_SCALAR_ERROR,
            ),
            render_jit_scope(
                "control/for_each_string_scalar_error",
                JIT_FOR_EACH_STRING_SCALAR_ERROR,
            ),
            render_jit_scope(
                "control/for_each_variant_scalar_error",
                JIT_FOR_EACH_VARIANT_SCALAR_ERROR,
            ),
            render_jit_scope("arrays/erase_fixed_reset", JIT_ARRAY_ERASE_FIXED_RESET),
            render_jit_scope(
                "arrays/erase_fixed_bounds_preserved",
                JIT_ARRAY_ERASE_FIXED_BOUNDS_PRESERVED,
            ),
            render_jit_scope(
                "arrays/erase_fixed_long_reset",
                JIT_ARRAY_ERASE_FIXED_LONG_RESET,
            ),
            render_jit_scope(
                "arrays/erase_fixed_long_rejects_string_after_reset",
                JIT_ARRAY_ERASE_FIXED_LONG_REJECTS_STRING_AFTER_RESET,
            ),
            render_jit_scope(
                "arrays/erase_fixed_typed_scalar_reset_bundle",
                JIT_ARRAY_ERASE_FIXED_TYPED_SCALAR_RESET_BUNDLE,
            ),
            render_jit_scope(
                "arrays/erase_fixed_typed_scalar_multidim_reset_bundle",
                JIT_ARRAY_ERASE_FIXED_TYPED_SCALAR_MULTIDIM_RESET_BUNDLE,
            ),
            render_jit_scope(
                "arrays/erase_fixed_typed_scalar_3d_reset_bundle",
                JIT_ARRAY_ERASE_FIXED_TYPED_SCALAR_3D_RESET_BUNDLE,
            ),
            render_jit_scope(
                "arrays/erase_fixed_typed_scalar_4d_reset_bundle",
                JIT_ARRAY_ERASE_FIXED_TYPED_SCALAR_4D_RESET_BUNDLE,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_bounds_error",
                JIT_ARRAY_ERASE_DYNAMIC_BOUNDS_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_long_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_LONG_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_boolean_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_BOOLEAN_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_byte_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_BYTE_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_integer_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_INTEGER_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_longlong_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_LONGLONG_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_single_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_SINGLE_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_double_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_DOUBLE_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_currency_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_CURRENCY_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_date_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_DATE_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_string_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_STRING_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_boolean_multidim_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_BOOLEAN_MULTIDIM_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_byte_multidim_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_BYTE_MULTIDIM_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_integer_multidim_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_INTEGER_MULTIDIM_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_long_multidim_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_LONG_MULTIDIM_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_longlong_multidim_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_LONGLONG_MULTIDIM_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_single_multidim_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_SINGLE_MULTIDIM_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_double_multidim_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_DOUBLE_MULTIDIM_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_currency_multidim_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_CURRENCY_MULTIDIM_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_date_multidim_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_DATE_MULTIDIM_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_string_multidim_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_STRING_MULTIDIM_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_boolean_3d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_BOOLEAN_3D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_byte_3d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_BYTE_3D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_integer_3d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_INTEGER_3D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_long_3d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_LONG_3D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_longlong_3d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_LONGLONG_3D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_single_3d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_SINGLE_3D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_double_3d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_DOUBLE_3D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_currency_3d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_CURRENCY_3D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_date_3d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_DATE_3D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_string_3d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_STRING_3D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_boolean_4d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_BOOLEAN_4D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_byte_4d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_BYTE_4D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_integer_4d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_INTEGER_4D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_long_4d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_LONG_4D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_longlong_4d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_LONGLONG_4D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_single_4d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_SINGLE_4D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_double_4d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_DOUBLE_4D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_currency_4d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_CURRENCY_4D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_date_4d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_DATE_4D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_string_4d_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_STRING_4D_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_lbound_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_LBOUND_UNALLOCATED_ERROR,
            ),
            render_jit_scope(
                "arrays/erase_dynamic_ubound_unallocated_error",
                JIT_ARRAY_ERASE_DYNAMIC_UBOUND_UNALLOCATED_ERROR,
            ),
            render_jit_scope("calls/function_return_long", JIT_FUNCTION_RETURN_LONG_CALL),
            render_jit_scope("calls/byref_long", JIT_BYREF_LONG_CALL),
            render_jit_scope("calls/nested_byref_long", JIT_NESTED_BYREF_LONG_CALL),
            render_jit_scope(
                "calls/two_arg_function_return_long",
                JIT_TWO_ARG_FUNCTION_RETURN_LONG,
            ),
            render_jit_scope(
                "calls/three_arg_function_return_long",
                JIT_THREE_ARG_FUNCTION_RETURN_LONG,
            ),
            render_jit_scope(
                "calls/four_arg_mixed_scalar",
                JIT_FOUR_ARG_MIXED_SCALAR_CALL,
            ),
            render_jit_scope(
                "calls/five_arg_function_return_long",
                JIT_FIVE_ARG_FUNCTION_RETURN_LONG,
            ),
            render_jit_scope("inline/builtin_abs_long", JIT_BUILTIN_ABS_LONG),
            render_jit_scope("inline/builtin_abs_integer", JIT_BUILTIN_ABS_INTEGER),
            render_jit_scope(
                "inline/builtin_abs_integer_min",
                JIT_BUILTIN_ABS_INTEGER_MIN,
            ),
            render_jit_scope("inline/builtin_abs_long_min", JIT_BUILTIN_ABS_LONG_MIN),
            render_jit_scope("inline/builtin_abs_bool", JIT_BUILTIN_ABS_BOOL),
            render_jit_scope("inline/builtin_abs_empty", JIT_BUILTIN_ABS_EMPTY),
            render_jit_scope("inline/builtin_abs_null", JIT_BUILTIN_ABS_NULL),
            render_jit_scope("inline/builtin_abs_double", JIT_BUILTIN_ABS_DOUBLE),
            render_jit_scope("inline/builtin_abs_single", JIT_BUILTIN_ABS_SINGLE),
            render_jit_scope("inline/builtin_abs_currency", JIT_BUILTIN_ABS_CURRENCY),
            render_jit_scope("inline/builtin_abs_longlong", JIT_BUILTIN_ABS_LONGLONG),
            render_jit_scope("inline/builtin_int_double", JIT_BUILTIN_INT_DOUBLE),
            render_jit_scope("inline/builtin_int_long", JIT_BUILTIN_INT_LONG),
            render_jit_scope("inline/builtin_int_integer", JIT_BUILTIN_INT_INTEGER),
            render_jit_scope("inline/builtin_int_longlong", JIT_BUILTIN_INT_LONGLONG),
            render_jit_scope("inline/builtin_int_single", JIT_BUILTIN_INT_SINGLE),
            render_jit_scope("inline/builtin_int_bool", JIT_BUILTIN_INT_BOOL),
            render_jit_scope("inline/builtin_int_empty", JIT_BUILTIN_INT_EMPTY),
            render_jit_scope("inline/builtin_int_null", JIT_BUILTIN_INT_NULL),
            render_jit_scope("inline/builtin_fix_double", JIT_BUILTIN_FIX_DOUBLE),
            render_jit_scope("inline/builtin_fix_integer", JIT_BUILTIN_FIX_INTEGER),
            render_jit_scope("inline/builtin_fix_long", JIT_BUILTIN_FIX_LONG),
            render_jit_scope("inline/builtin_fix_longlong", JIT_BUILTIN_FIX_LONGLONG),
            render_jit_scope("inline/builtin_fix_single", JIT_BUILTIN_FIX_SINGLE),
            render_jit_scope("inline/builtin_fix_bool", JIT_BUILTIN_FIX_BOOL),
            render_jit_scope("inline/builtin_fix_empty", JIT_BUILTIN_FIX_EMPTY),
            render_jit_scope("inline/builtin_fix_null", JIT_BUILTIN_FIX_NULL),
            render_jit_scope("inline/builtin_int_currency", JIT_BUILTIN_INT_CURRENCY),
            render_jit_scope("inline/builtin_fix_currency", JIT_BUILTIN_FIX_CURRENCY),
            render_jit_scope("inline/builtin_int_date", JIT_BUILTIN_INT_DATE),
            render_jit_scope("inline/builtin_fix_date", JIT_BUILTIN_FIX_DATE),
            render_jit_scope("inline/builtin_sgn_double", JIT_BUILTIN_SGN_DOUBLE),
            render_jit_scope("inline/builtin_sgn_long", JIT_BUILTIN_SGN_LONG),
            render_jit_scope("inline/builtin_sgn_integer", JIT_BUILTIN_SGN_INTEGER),
            render_jit_scope("inline/builtin_sgn_bool", JIT_BUILTIN_SGN_BOOL),
            render_jit_scope("inline/builtin_sgn_empty", JIT_BUILTIN_SGN_EMPTY),
            render_jit_scope("inline/builtin_sgn_zero", JIT_BUILTIN_SGN_ZERO),
            render_jit_scope("inline/builtin_sgn_null", JIT_BUILTIN_SGN_NULL),
            render_jit_scope("inline/builtin_sgn_longlong", JIT_BUILTIN_SGN_LONGLONG),
            render_jit_scope("inline/builtin_sgn_single", JIT_BUILTIN_SGN_SINGLE),
            render_jit_scope("inline/builtin_sgn_currency", JIT_BUILTIN_SGN_CURRENCY),
            render_jit_scope("inline/builtin_cbool_expr", JIT_BUILTIN_CBOOL_EXPR),
            render_jit_scope("inline/builtin_cbyte_expr", JIT_BUILTIN_CBYTE_EXPR),
            render_jit_scope("inline/builtin_cint_expr", JIT_BUILTIN_CINT_EXPR),
            render_jit_scope("inline/builtin_clng_expr", JIT_BUILTIN_CLNG_EXPR),
            render_jit_scope("inline/builtin_clnglng_expr", JIT_BUILTIN_CLNGLNG_EXPR),
            render_jit_scope("inline/builtin_clngptr_expr", JIT_BUILTIN_CLNGPTR_EXPR),
            render_jit_scope("inline/builtin_csng_expr", JIT_BUILTIN_CSNG_EXPR),
            render_jit_scope("inline/builtin_cdbl_expr", JIT_BUILTIN_CDBL_EXPR),
            render_jit_scope("inline/builtin_ccur_expr", JIT_BUILTIN_CCUR_EXPR),
            render_jit_scope("inline/builtin_cdate_expr", JIT_BUILTIN_CDATE_EXPR),
            render_jit_scope("inline/builtin_cdec_expr", JIT_BUILTIN_CDEC_EXPR),
            render_jit_scope("inline/builtin_cstr_expr", JIT_BUILTIN_CSTR_EXPR),
            render_jit_scope("inline/builtin_cvar_expr", JIT_BUILTIN_CVAR_EXPR),
            render_jit_scope("inline/builtin_cverr_expr", JIT_BUILTIN_CVERR_EXPR),
            render_jit_scope("inline/builtin_cverr_invalid", JIT_BUILTIN_CVERR_INVALID),
            render_jit_scope("inline/builtin_hex_expr", JIT_BUILTIN_HEX_EXPR),
            render_jit_scope("inline/builtin_oct_expr", JIT_BUILTIN_OCT_EXPR),
            render_jit_scope(
                "inline/builtin_hex_oct_negative_width_exprs",
                JIT_BUILTIN_HEX_OCT_NEGATIVE_WIDTH_EXPRS,
            ),
            render_jit_scope("inline/builtin_str_expr", JIT_BUILTIN_STR_EXPR),
            render_jit_scope(
                "inline/builtin_string_result_destinations",
                JIT_BUILTIN_STRING_RESULT_DESTINATIONS,
            ),
            render_jit_scope(
                "inline/builtin_string_typed_aliases",
                JIT_BUILTIN_STRING_TYPED_ALIASES,
            ),
            render_jit_scope(
                "inline/builtin_string_typed_alias_null_error",
                JIT_BUILTIN_STRING_TYPED_ALIAS_NULL_ERROR,
            ),
            render_jit_scope(
                "inline/builtin_string_typed_alias_destinations",
                JIT_BUILTIN_STRING_TYPED_ALIAS_DESTINATIONS,
            ),
            render_jit_scope(
                "inline/string_destination_null_variant",
                JIT_STRING_DESTINATION_NULL_VARIANT,
            ),
            render_jit_scope(
                "inline/fixed_string_local_pad_truncate",
                JIT_FIXED_STRING_LOCAL_PAD_TRUNCATE,
            ),
            render_jit_scope(
                "inline/fixed_string_local_null_error",
                JIT_FIXED_STRING_LOCAL_NULL_ERROR,
            ),
            render_jit_scope(
                "inline/fixed_string_global_pad_truncate",
                JIT_FIXED_STRING_GLOBAL_PAD_TRUNCATE,
            ),
            render_jit_scope(
                "inline/fixed_string_global_null_error",
                JIT_FIXED_STRING_GLOBAL_NULL_ERROR,
            ),
            render_jit_scope(
                "inline/builtin_scalar_result_destinations",
                JIT_BUILTIN_SCALAR_RESULT_DESTINATIONS,
            ),
            render_jit_scope(
                "inline/builtin_scalar_result_destination_families",
                JIT_BUILTIN_SCALAR_RESULT_DESTINATION_FAMILIES,
            ),
            render_jit_scope(
                "inline/builtin_scalar_result_destination_coercions",
                JIT_BUILTIN_SCALAR_RESULT_DESTINATION_COERCIONS,
            ),
            render_jit_scope(
                "inline/builtin_scalar_result_destination_coercion_overflow",
                JIT_BUILTIN_SCALAR_RESULT_DESTINATION_COERCION_OVERFLOW,
            ),
            render_jit_scope(
                "inline/builtin_scalar_result_destination_null_error",
                JIT_BUILTIN_SCALAR_RESULT_DESTINATION_NULL_ERROR,
            ),
            render_jit_scope(
                "inline/builtin_math_unary_exprs",
                JIT_BUILTIN_MATH_UNARY_EXPRS,
            ),
            render_jit_scope("inline/builtin_sqr_invalid", JIT_BUILTIN_SQR_INVALID),
            render_jit_scope("inline/builtin_log_invalid", JIT_BUILTIN_LOG_INVALID),
            render_jit_scope("inline/builtin_exp_overflow", JIT_BUILTIN_EXP_OVERFLOW),
            render_jit_scope("inline/builtin_round_expr", JIT_BUILTIN_ROUND_EXPR),
            render_jit_scope(
                "inline/builtin_round_digits_expr",
                JIT_BUILTIN_ROUND_DIGITS_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_round_negative_digits",
                JIT_BUILTIN_ROUND_NEGATIVE_DIGITS,
            ),
            render_jit_scope(
                "inline/builtin_date_part_exprs",
                JIT_BUILTIN_DATE_PART_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_information_exprs",
                JIT_BUILTIN_INFORMATION_EXPRS,
            ),
            render_jit_scope(
                "inline/stdlib_variant_predicates",
                JIT_STDLIB_VARIANT_PREDICATES,
            ),
            render_jit_scope(
                "inline/coercion_null_empty_error_predicates",
                JIT_COERCION_NULL_EMPTY_ERROR_PREDICATES,
            ),
            render_jit_scope(
                "inline/introspection_vartype_isnumeric_tags",
                JIT_INTROSPECTION_VARTYPE_ISNUMERIC_TAGS,
            ),
            render_jit_scope(
                "inline/string_vbnullstring_predicates",
                JIT_STRING_VBNULLSTRING_PREDICATES,
            ),
            render_jit_scope(
                "inline/builtin_isarray_dynamic_array_exprs",
                JIT_BUILTIN_ISARRAY_DYNAMIC_ARRAY_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_isarray_variant_carrier_exprs",
                JIT_BUILTIN_ISARRAY_VARIANT_CARRIER_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_date_value_time_value_exprs",
                JIT_BUILTIN_DATE_VALUE_TIME_VALUE_EXPRS,
            ),
            render_jit_scope(
                "inline/stdlib_date_string_policy",
                JIT_STDLIB_DATE_STRING_POLICY,
            ),
            render_jit_scope(
                "inline/stdlib_datetime_expansion",
                JIT_STDLIB_DATETIME_EXPANSION,
            ),
            render_jit_scope(
                "inline/stdlib_date_serial_value",
                JIT_STDLIB_DATE_SERIAL_VALUE,
            ),
            render_jit_scope(
                "inline/stdlib_time_serial_value",
                JIT_STDLIB_TIME_SERIAL_VALUE,
            ),
            render_jit_scope("inline/stdlib_date_add_diff", JIT_STDLIB_DATE_ADD_DIFF),
            render_jit_scope("inline/stdlib_len_basic", JIT_STDLIB_LEN_BASIC),
            render_jit_scope("inline/stdlib_slice_ops", JIT_STDLIB_SLICE_OPS),
            render_jit_scope("inline/stdlib_instr_case_ops", JIT_STDLIB_INSTR_CASE_OPS),
            render_jit_scope(
                "inline/stdlib_advanced_instrrev_like",
                JIT_STDLIB_ADVANCED_INSTRREV_LIKE,
            ),
            render_jit_scope(
                "inline/stdlib_advanced_replace_trim",
                JIT_STDLIB_ADVANCED_REPLACE_TRIM,
            ),
            render_jit_scope(
                "inline/stdlib_advanced_strcomp",
                JIT_STDLIB_ADVANCED_STRCOMP,
            ),
            render_jit_scope(
                "inline/stdlib_advanced_split_join",
                JIT_STDLIB_ADVANCED_SPLIT_JOIN,
            ),
            render_jit_scope(
                "inline/stdlib_string_expansion_core",
                JIT_STDLIB_STRING_EXPANSION_CORE,
            ),
            render_jit_scope("inline/stdlib_format_core", JIT_STDLIB_FORMAT_CORE),
            render_jit_scope(
                "inline/stdlib_financial_zero_rate",
                JIT_STDLIB_FINANCIAL_ZERO_RATE,
            ),
            render_jit_scope(
                "inline/financial_algorithm_rate_nper_subset",
                JIT_FINANCIAL_ALGORITHM_RATE_NPER_SUBSET,
            ),
            render_jit_scope("inline/stdlib_rnd_isolated", JIT_STDLIB_RND_ISOLATED),
            render_jit_scope(
                "inline/stdlib_numeric_expansion",
                JIT_STDLIB_NUMERIC_EXPANSION,
            ),
            render_jit_scope("inline/conversion_cint_basic", JIT_CONVERSION_CINT_BASIC),
            render_jit_scope(
                "inline/stdlib_error_cverr_identity",
                JIT_STDLIB_ERROR_CVERR_IDENTITY,
            ),
            render_jit_scope(
                "inline/stdlib_error_err_raise_fail",
                JIT_STDLIB_ERROR_ERR_RAISE_FAIL,
            ),
            render_jit_scope(
                "inline/stdlib_error_err_raise_resume",
                JIT_STDLIB_ERROR_ERR_RAISE_RESUME,
            ),
            render_jit_scope("inline/on_error_resume_next", JIT_ON_ERROR_RESUME_NEXT),
            render_jit_scope(
                "inline/on_error_resume_continue",
                JIT_ON_ERROR_RESUME_CONTINUE,
            ),
            render_jit_scope("inline/on_error_default_fail", JIT_ON_ERROR_DEFAULT_FAIL),
            render_jit_scope(
                "inline/on_error_goto_zero_fail",
                JIT_ON_ERROR_GOTO_ZERO_FAIL,
            ),
            render_jit_scope(
                "inline/resume_next_statement_ok",
                JIT_RESUME_NEXT_STATEMENT_OK,
            ),
            render_jit_scope("inline/err_resume_next_clears", JIT_ERR_RESUME_NEXT_CLEARS),
            render_jit_scope("inline/resume_statement_basic", JIT_RESUME_STATEMENT_BASIC),
            render_jit_scope("inline/resume_label_basic", JIT_RESUME_LABEL_BASIC),
            render_jit_scope(
                "inline/on_error_goto_label_resume",
                JIT_ON_ERROR_GOTO_LABEL_RESUME,
            ),
            render_jit_scope(
                "inline/error_goto_label_resume_next",
                JIT_ERROR_GOTO_LABEL_RESUME_NEXT,
            ),
            render_jit_scope("inline/err_clear_basic", JIT_ERR_CLEAR_BASIC),
            render_jit_scope(
                "inline/error_raise_custom_clear_cycle",
                JIT_ERROR_RAISE_CUSTOM_CLEAR_CYCLE,
            ),
            render_jit_scope(
                "inline/err_proc_call_boundary_clears",
                JIT_ERR_PROC_CALL_BOUNDARY_CLEARS,
            ),
            render_jit_scope(
                "inline/err_surface_fields_subset",
                JIT_ERR_SURFACE_FIELDS_SUBSET,
            ),
            render_jit_scope(
                "inline/err_clear_full_surface_reset",
                JIT_ERR_CLEAR_FULL_SURFACE_RESET,
            ),
            render_jit_scope(
                "inline/conversion_extended_scalar_subset",
                JIT_CONVERSION_EXTENDED_SCALAR_SUBSET,
            ),
            render_jit_scope("inline/stdlib_math_primitives", JIT_STDLIB_MATH_PRIMITIVES),
            render_jit_scope(
                "inline/stdlib_math_transcendental_identity",
                JIT_STDLIB_MATH_TRANSCENDENTAL_IDENTITY,
            ),
            render_jit_scope(
                "inline/conversion_val_str_subset",
                JIT_CONVERSION_VAL_STR_SUBSET,
            ),
            render_jit_scope(
                "inline/conversion_clng_cint_chain",
                JIT_CONVERSION_CLNG_CINT_CHAIN,
            ),
            render_jit_scope(
                "inline/conversion_nested_clng_cint",
                JIT_CONVERSION_NESTED_CLNG_CINT,
            ),
            render_jit_scope(
                "inline/builtin_date_serial_time_serial_exprs",
                JIT_BUILTIN_DATE_SERIAL_TIME_SERIAL_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_date_serial_range_error",
                JIT_BUILTIN_DATE_SERIAL_RANGE_ERROR,
            ),
            render_jit_scope(
                "inline/builtin_rgb_qbcolor_exprs",
                JIT_BUILTIN_RGB_QBCOLOR_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_rgb_component_exprs",
                JIT_BUILTIN_RGB_COMPONENT_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_qbcolor_palette_exprs",
                JIT_BUILTIN_QBCOLOR_PALETTE_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_qbcolor_out_of_range",
                JIT_BUILTIN_QBCOLOR_OUT_OF_RANGE,
            ),
            render_jit_scope(
                "inline/builtin_error_text_expr",
                JIT_BUILTIN_ERROR_TEXT_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_error_text_unknown_expr",
                JIT_BUILTIN_ERROR_TEXT_UNKNOWN_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_error_text_invalid_expr",
                JIT_BUILTIN_ERROR_TEXT_INVALID_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_error_text_result_destinations",
                JIT_BUILTIN_ERROR_TEXT_RESULT_DESTINATIONS,
            ),
            render_jit_scope(
                "inline/builtin_error_text_result_destination_invalid",
                JIT_BUILTIN_ERROR_TEXT_RESULT_DESTINATION_INVALID,
            ),
            render_jit_scope(
                "inline/builtin_error_text_alias_destinations",
                JIT_BUILTIN_ERROR_TEXT_ALIAS_DESTINATIONS,
            ),
            render_jit_scope(
                "inline/builtin_error_text_alias_destination_invalid",
                JIT_BUILTIN_ERROR_TEXT_ALIAS_DESTINATION_INVALID,
            ),
            render_jit_scope(
                "inline/builtin_len_variant_expr",
                JIT_BUILTIN_LEN_VARIANT_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_lenb_variant_expr",
                JIT_BUILTIN_LENB_VARIANT_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_chrw_ascw_variant_exprs",
                JIT_BUILTIN_CHRW_ASCW_VARIANT_EXPRS,
            ),
            render_jit_scope("inline/builtin_space_expr", JIT_BUILTIN_SPACE_EXPR),
            render_jit_scope(
                "inline/builtin_space_negative_count",
                JIT_BUILTIN_SPACE_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_case_variant_exprs",
                JIT_BUILTIN_CASE_VARIANT_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_val_variant_expr",
                JIT_BUILTIN_VAL_VARIANT_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_trim_variant_exprs",
                JIT_BUILTIN_TRIM_VARIANT_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_str_reverse_variant_expr",
                JIT_BUILTIN_STR_REVERSE_VARIANT_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_string_repeat_charcode_expr",
                JIT_BUILTIN_STRING_REPEAT_CHARCODE_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_string_repeat_charcode_wrap_expr",
                JIT_BUILTIN_STRING_REPEAT_CHARCODE_WRAP_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_string_repeat_negative_count",
                JIT_BUILTIN_STRING_REPEAT_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_left_right_variant_exprs",
                JIT_BUILTIN_LEFT_RIGHT_VARIANT_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_left_right_variant_count_edges",
                JIT_BUILTIN_LEFT_RIGHT_VARIANT_COUNT_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_left_right_variant_complement_count_edges",
                JIT_BUILTIN_LEFT_RIGHT_VARIANT_COMPLEMENT_COUNT_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_left_right_variant_unit_count",
                JIT_BUILTIN_LEFT_RIGHT_VARIANT_UNIT_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_left_right_variant_exact_source_count",
                JIT_BUILTIN_LEFT_RIGHT_VARIANT_EXACT_SOURCE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_left_negative_count",
                JIT_BUILTIN_LEFT_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_right_negative_count",
                JIT_BUILTIN_RIGHT_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_left_negative_count",
                JIT_BUILTIN_STRING_LITERAL_LEFT_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_right_negative_count",
                JIT_BUILTIN_STRING_LITERAL_RIGHT_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_static_string_left_negative_count",
                JIT_BUILTIN_STATIC_STRING_LEFT_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_static_string_right_negative_count",
                JIT_BUILTIN_STATIC_STRING_RIGHT_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_leftb_rightb_variant_exprs",
                JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_leftb_rightb_variant_unit_code_unit_byte_count",
                JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_UNIT_CODE_UNIT_BYTE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_leftb_rightb_variant_three_code_unit_byte_count",
                JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_THREE_CODE_UNIT_BYTE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_leftb_rightb_variant_odd_byte_exprs",
                JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_ODD_BYTE_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_leftb_rightb_variant_byte_count_edges",
                JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_BYTE_COUNT_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_leftb_rightb_variant_complement_byte_count_edges",
                JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_COMPLEMENT_BYTE_COUNT_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_leftb_rightb_variant_exact_byte_source_count",
                JIT_BUILTIN_LEFTB_RIGHTB_VARIANT_EXACT_BYTE_SOURCE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_leftb_negative_count",
                JIT_BUILTIN_LEFTB_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_rightb_negative_count",
                JIT_BUILTIN_RIGHTB_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_leftb_negative_count",
                JIT_BUILTIN_STRING_LITERAL_LEFTB_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_rightb_negative_count",
                JIT_BUILTIN_STRING_LITERAL_RIGHTB_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_static_string_leftb_negative_count",
                JIT_BUILTIN_STATIC_STRING_LEFTB_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_static_string_rightb_negative_count",
                JIT_BUILTIN_STATIC_STRING_RIGHTB_NEGATIVE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_instr_instrrev_variant_exprs",
                JIT_BUILTIN_INSTR_INSTRREV_VARIANT_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_strcomp_variant_exprs",
                JIT_BUILTIN_STRCOMP_VARIANT_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_replace_variant_expr",
                JIT_BUILTIN_REPLACE_VARIANT_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_like_variant_expr",
                JIT_BUILTIN_LIKE_VARIANT_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_variant_args",
                JIT_BUILTIN_STRING_LITERAL_VARIANT_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_companion_args",
                JIT_BUILTIN_STRING_LITERAL_COMPANION_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_leftb_rightb_byte_counts",
                JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_BYTE_COUNTS,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_leftb_rightb_byte_count_edges",
                JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_BYTE_COUNT_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_leftb_rightb_complement_byte_count_edges",
                JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_COMPLEMENT_BYTE_COUNT_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_leftb_rightb_exact_byte_source_count",
                JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_EXACT_BYTE_SOURCE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_left_right_count_edges",
                JIT_BUILTIN_STRING_LITERAL_LEFT_RIGHT_COUNT_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_mid_count_edges",
                JIT_BUILTIN_STRING_LITERAL_MID_COUNT_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_mid_value_edges",
                JIT_BUILTIN_STRING_LITERAL_MID_VALUE_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_mid_boundary_value_edges",
                JIT_BUILTIN_STRING_LITERAL_MID_BOUNDARY_VALUE_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_mid_start_zero",
                JIT_BUILTIN_STRING_LITERAL_MID_START_ZERO,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_mid_negative_start",
                JIT_BUILTIN_STRING_LITERAL_MID_NEGATIVE_START,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_mid_negative_length",
                JIT_BUILTIN_STRING_LITERAL_MID_NEGATIVE_LENGTH,
            ),
            render_jit_scope(
                "inline/builtin_static_string_operands",
                JIT_BUILTIN_STATIC_STRING_OPERANDS,
            ),
            render_jit_scope(
                "inline/builtin_static_string_companion_operands",
                JIT_BUILTIN_STATIC_STRING_COMPANION_OPERANDS,
            ),
            render_jit_scope(
                "inline/builtin_static_string_leftb_rightb_byte_counts",
                JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_BYTE_COUNTS,
            ),
            render_jit_scope(
                "inline/builtin_static_string_leftb_rightb_byte_count_edges",
                JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_BYTE_COUNT_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_static_string_leftb_rightb_complement_byte_count_edges",
                JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_COMPLEMENT_BYTE_COUNT_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_static_string_leftb_rightb_exact_byte_source_count",
                JIT_BUILTIN_STATIC_STRING_LEFTB_RIGHTB_EXACT_BYTE_SOURCE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_static_string_left_right_count_edges",
                JIT_BUILTIN_STATIC_STRING_LEFT_RIGHT_COUNT_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_static_string_mid_count_edges",
                JIT_BUILTIN_STATIC_STRING_MID_COUNT_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_static_string_mid_value_edges",
                JIT_BUILTIN_STATIC_STRING_MID_VALUE_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_static_string_mid_boundary_value_edges",
                JIT_BUILTIN_STATIC_STRING_MID_BOUNDARY_VALUE_EDGES,
            ),
            render_jit_scope(
                "inline/builtin_static_string_mid_start_zero",
                JIT_BUILTIN_STATIC_STRING_MID_START_ZERO,
            ),
            render_jit_scope(
                "inline/builtin_static_string_mid_negative_start",
                JIT_BUILTIN_STATIC_STRING_MID_NEGATIVE_START,
            ),
            render_jit_scope(
                "inline/builtin_static_string_mid_negative_length",
                JIT_BUILTIN_STATIC_STRING_MID_NEGATIVE_LENGTH,
            ),
            render_jit_scope(
                "inline/builtin_string_null_slice_args",
                JIT_BUILTIN_STRING_NULL_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_empty_slice_args",
                JIT_BUILTIN_STRING_EMPTY_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_numeric_slice_args",
                JIT_BUILTIN_STRING_NUMERIC_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_boolean_slice_args",
                JIT_BUILTIN_STRING_BOOLEAN_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_double_slice_args",
                JIT_BUILTIN_STRING_DOUBLE_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_single_slice_args",
                JIT_BUILTIN_STRING_SINGLE_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_integer_slice_args",
                JIT_BUILTIN_STRING_INTEGER_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_longlong_slice_args",
                JIT_BUILTIN_STRING_LONGLONG_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_byte_slice_args",
                JIT_BUILTIN_STRING_BYTE_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_currency_slice_args",
                JIT_BUILTIN_STRING_CURRENCY_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_date_slice_args",
                JIT_BUILTIN_STRING_DATE_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_error_slice_args",
                JIT_BUILTIN_STRING_ERROR_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_string_decimal_slice_args",
                JIT_BUILTIN_STRING_DECIMAL_SLICE_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_leftb_rightb_odd_byte_exprs",
                JIT_BUILTIN_LEFTB_RIGHTB_ODD_BYTE_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_string_literal_leftb_rightb_odd_byte_exprs",
                JIT_BUILTIN_STRING_LITERAL_LEFTB_RIGHTB_ODD_BYTE_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_string_optional_args",
                JIT_BUILTIN_STRING_OPTIONAL_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_static_string_optional_args",
                JIT_BUILTIN_STATIC_STRING_OPTIONAL_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_expr",
                JIT_BUILTIN_MID_VARIANT_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_omitted_length_expr",
                JIT_BUILTIN_MID_VARIANT_OMITTED_LENGTH_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_omitted_length_full_source",
                JIT_BUILTIN_MID_VARIANT_OMITTED_LENGTH_FULL_SOURCE,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_omitted_length_suffix",
                JIT_BUILTIN_MID_VARIANT_OMITTED_LENGTH_SUFFIX,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_omitted_length_overlong_start",
                JIT_BUILTIN_MID_VARIANT_OMITTED_LENGTH_OVERLONG_START,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_start_zero",
                JIT_BUILTIN_MID_VARIANT_START_ZERO,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_zero_length",
                JIT_BUILTIN_MID_VARIANT_ZERO_LENGTH,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_zero_length_middle",
                JIT_BUILTIN_MID_VARIANT_ZERO_LENGTH_MIDDLE,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_zero_length_at_end",
                JIT_BUILTIN_MID_VARIANT_ZERO_LENGTH_AT_END,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_exact_last_char",
                JIT_BUILTIN_MID_VARIANT_EXACT_LAST_CHAR,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_exact_full_source_count",
                JIT_BUILTIN_MID_VARIANT_EXACT_FULL_SOURCE_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_exact_suffix_count",
                JIT_BUILTIN_MID_VARIANT_EXACT_SUFFIX_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_exact_prefix_count",
                JIT_BUILTIN_MID_VARIANT_EXACT_PREFIX_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_negative_length",
                JIT_BUILTIN_MID_VARIANT_NEGATIVE_LENGTH,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_negative_start",
                JIT_BUILTIN_MID_VARIANT_NEGATIVE_START,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_overlong_start",
                JIT_BUILTIN_MID_VARIANT_OVERLONG_START,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_overlong_count",
                JIT_BUILTIN_MID_VARIANT_OVERLONG_COUNT,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_overlong_count_middle",
                JIT_BUILTIN_MID_VARIANT_OVERLONG_COUNT_MIDDLE,
            ),
            render_jit_scope(
                "inline/builtin_mid_variant_overlong_count_full_source",
                JIT_BUILTIN_MID_VARIANT_OVERLONG_COUNT_FULL_SOURCE,
            ),
            render_jit_scope("inline/builtin_weekday_expr", JIT_BUILTIN_WEEKDAY_EXPR),
            render_jit_scope(
                "inline/builtin_weekday_firstday_expr",
                JIT_BUILTIN_WEEKDAY_FIRSTDAY_EXPR,
            ),
            render_jit_scope(
                "inline/builtin_date_name_exprs",
                JIT_BUILTIN_DATE_NAME_EXPRS,
            ),
            render_jit_scope(
                "inline/builtin_date_name_optional_args",
                JIT_BUILTIN_DATE_NAME_OPTIONAL_ARGS,
            ),
            render_jit_scope(
                "inline/builtin_conversion_variant_operands",
                JIT_BUILTIN_CONVERSION_VARIANT_OPERANDS,
            ),
            render_jit_scope(
                "calls/optional_variant_default",
                JIT_OPTIONAL_VARIANT_DEFAULT_CALL,
            ),
            render_jit_scope(
                "calls/optional_variant_omitted",
                JIT_OPTIONAL_VARIANT_OMITTED_CALL,
            ),
            render_jit_scope(
                "calls/optional_variant_intermediate_omitted_ismissing",
                JIT_OPTIONAL_VARIANT_INTERMEDIATE_OMITTED_ISMISSING_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_default",
                JIT_OPTIONAL_LONG_DEFAULT_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_omitted",
                JIT_OPTIONAL_LONG_OMITTED_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_default",
                JIT_OPTIONAL_DOUBLE_DEFAULT_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_omitted",
                JIT_OPTIONAL_DOUBLE_OMITTED_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_default",
                JIT_OPTIONAL_CURRENCY_DEFAULT_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_omitted",
                JIT_OPTIONAL_CURRENCY_OMITTED_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_default",
                JIT_OPTIONAL_BOOL_DEFAULT_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_omitted",
                JIT_OPTIONAL_BOOL_OMITTED_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_default",
                JIT_OPTIONAL_BYTE_DEFAULT_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_omitted",
                JIT_OPTIONAL_BYTE_OMITTED_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_default",
                JIT_OPTIONAL_INTEGER_DEFAULT_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_omitted",
                JIT_OPTIONAL_INTEGER_OMITTED_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_default",
                JIT_OPTIONAL_LONGLONG_DEFAULT_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_omitted",
                JIT_OPTIONAL_LONGLONG_OMITTED_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_default",
                JIT_OPTIONAL_SINGLE_DEFAULT_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_omitted",
                JIT_OPTIONAL_SINGLE_OMITTED_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_default",
                JIT_OPTIONAL_DATE_DEFAULT_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_omitted",
                JIT_OPTIONAL_DATE_OMITTED_CALL,
            ),
            render_jit_scope(
                "calls/optional_variant_explicit_local",
                JIT_OPTIONAL_VARIANT_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_explicit_local",
                JIT_OPTIONAL_LONG_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_explicit_local",
                JIT_OPTIONAL_DOUBLE_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_explicit_local",
                JIT_OPTIONAL_CURRENCY_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_explicit_local",
                JIT_OPTIONAL_BOOL_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_explicit_local",
                JIT_OPTIONAL_BYTE_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_explicit_local",
                JIT_OPTIONAL_INTEGER_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_explicit_local",
                JIT_OPTIONAL_LONGLONG_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_explicit_local",
                JIT_OPTIONAL_SINGLE_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_explicit_local",
                JIT_OPTIONAL_DATE_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_variant_named_explicit_local",
                JIT_OPTIONAL_VARIANT_NAMED_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_named_explicit_local",
                JIT_OPTIONAL_LONG_NAMED_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_named_explicit_local",
                JIT_OPTIONAL_DOUBLE_NAMED_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_named_explicit_local",
                JIT_OPTIONAL_CURRENCY_NAMED_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_named_explicit_local",
                JIT_OPTIONAL_BOOL_NAMED_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_named_explicit_local",
                JIT_OPTIONAL_BYTE_NAMED_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_named_explicit_local",
                JIT_OPTIONAL_INTEGER_NAMED_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_named_explicit_local",
                JIT_OPTIONAL_LONGLONG_NAMED_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_named_explicit_local",
                JIT_OPTIONAL_SINGLE_NAMED_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_named_explicit_local",
                JIT_OPTIONAL_DATE_NAMED_EXPLICIT_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_named_arg_order_double_coerce",
                JIT_OPTIONAL_LONG_NAMED_ARG_ORDER_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_named_arg_order_long_coerce",
                JIT_OPTIONAL_DOUBLE_NAMED_ARG_ORDER_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_named_arg_order_integer_coerce",
                JIT_OPTIONAL_CURRENCY_NAMED_ARG_ORDER_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_named_arg_order_double_zero_coerce",
                JIT_OPTIONAL_BOOL_NAMED_ARG_ORDER_DOUBLE_ZERO_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_explicit_double_coerce",
                JIT_OPTIONAL_LONG_EXPLICIT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_explicit_long_coerce",
                JIT_OPTIONAL_DOUBLE_EXPLICIT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_explicit_double_coerce",
                JIT_OPTIONAL_CURRENCY_EXPLICIT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_explicit_long_coerce",
                JIT_OPTIONAL_BOOL_EXPLICIT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_explicit_integer_coerce",
                JIT_OPTIONAL_BYTE_EXPLICIT_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_explicit_long_coerce",
                JIT_OPTIONAL_INTEGER_EXPLICIT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_explicit_long_coerce",
                JIT_OPTIONAL_LONGLONG_EXPLICIT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_explicit_double_coerce",
                JIT_OPTIONAL_SINGLE_EXPLICIT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_explicit_double_coerce",
                JIT_OPTIONAL_DATE_EXPLICIT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_explicit_null_coerce_error",
                JIT_OPTIONAL_LONG_EXPLICIT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_explicit_null_coerce_error",
                JIT_OPTIONAL_BOOL_EXPLICIT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_explicit_null_coerce_error",
                JIT_OPTIONAL_BYTE_EXPLICIT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_explicit_null_coerce_error",
                JIT_OPTIONAL_INTEGER_EXPLICIT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_explicit_null_coerce_error",
                JIT_OPTIONAL_LONGLONG_EXPLICIT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_explicit_null_coerce_error",
                JIT_OPTIONAL_SINGLE_EXPLICIT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_explicit_null_coerce_error",
                JIT_OPTIONAL_DOUBLE_EXPLICIT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_explicit_null_coerce_error",
                JIT_OPTIONAL_CURRENCY_EXPLICIT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_explicit_null_coerce_error",
                JIT_OPTIONAL_DATE_EXPLICIT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_explicit_boolean_coerce",
                JIT_OPTIONAL_LONG_EXPLICIT_BOOLEAN_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_explicit_empty_coerce",
                JIT_OPTIONAL_LONG_EXPLICIT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_explicit_empty_coerce",
                JIT_OPTIONAL_BOOL_EXPLICIT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_explicit_empty_coerce",
                JIT_OPTIONAL_DATE_EXPLICIT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_explicit_currency_coerce",
                JIT_OPTIONAL_DOUBLE_EXPLICIT_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_explicit_long_coerce",
                JIT_OPTIONAL_CURRENCY_EXPLICIT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_explicit_long_coerce",
                JIT_OPTIONAL_SINGLE_EXPLICIT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_explicit_long_overflow",
                JIT_OPTIONAL_BYTE_EXPLICIT_LONG_OVERFLOW_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_explicit_long_overflow",
                JIT_OPTIONAL_INTEGER_EXPLICIT_LONG_OVERFLOW_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_explicit_error_coerce_error",
                JIT_OPTIONAL_LONG_EXPLICIT_ERROR_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_explicit_long_coerce",
                JIT_OPTIONAL_BYTE_EXPLICIT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_explicit_byte_coerce",
                JIT_OPTIONAL_INTEGER_EXPLICIT_BYTE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_explicit_double_coerce",
                JIT_OPTIONAL_LONGLONG_EXPLICIT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_explicit_currency_coerce",
                JIT_OPTIONAL_SINGLE_EXPLICIT_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_explicit_single_coerce",
                JIT_OPTIONAL_DOUBLE_EXPLICIT_SINGLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_explicit_integer_coerce",
                JIT_OPTIONAL_CURRENCY_EXPLICIT_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_explicit_double_zero_coerce",
                JIT_OPTIONAL_BOOL_EXPLICIT_DOUBLE_ZERO_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_explicit_long_coerce",
                JIT_OPTIONAL_DATE_EXPLICIT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_named_double_coerce",
                JIT_OPTIONAL_LONG_NAMED_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_named_long_coerce",
                JIT_OPTIONAL_DOUBLE_NAMED_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_named_double_coerce",
                JIT_OPTIONAL_CURRENCY_NAMED_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_named_long_coerce",
                JIT_OPTIONAL_BOOL_NAMED_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_named_integer_coerce",
                JIT_OPTIONAL_BYTE_NAMED_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_named_long_coerce",
                JIT_OPTIONAL_INTEGER_NAMED_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_named_long_coerce",
                JIT_OPTIONAL_LONGLONG_NAMED_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_named_double_coerce",
                JIT_OPTIONAL_SINGLE_NAMED_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_named_double_coerce",
                JIT_OPTIONAL_DATE_NAMED_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_named_long_coerce",
                JIT_OPTIONAL_BYTE_NAMED_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_named_byte_coerce",
                JIT_OPTIONAL_INTEGER_NAMED_BYTE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_named_double_coerce",
                JIT_OPTIONAL_LONGLONG_NAMED_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_named_currency_coerce",
                JIT_OPTIONAL_SINGLE_NAMED_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_named_single_coerce",
                JIT_OPTIONAL_DOUBLE_NAMED_SINGLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_named_integer_coerce",
                JIT_OPTIONAL_CURRENCY_NAMED_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_named_double_zero_coerce",
                JIT_OPTIONAL_BOOL_NAMED_DOUBLE_ZERO_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_named_long_coerce",
                JIT_OPTIONAL_DATE_NAMED_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_named_null_coerce_error",
                JIT_OPTIONAL_LONG_NAMED_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_named_null_coerce_error",
                JIT_OPTIONAL_BOOL_NAMED_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_named_null_coerce_error",
                JIT_OPTIONAL_BYTE_NAMED_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_named_null_coerce_error",
                JIT_OPTIONAL_INTEGER_NAMED_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_named_null_coerce_error",
                JIT_OPTIONAL_LONGLONG_NAMED_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_named_null_coerce_error",
                JIT_OPTIONAL_SINGLE_NAMED_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_named_null_coerce_error",
                JIT_OPTIONAL_DOUBLE_NAMED_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_named_null_coerce_error",
                JIT_OPTIONAL_CURRENCY_NAMED_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_named_null_coerce_error",
                JIT_OPTIONAL_DATE_NAMED_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_named_boolean_coerce",
                JIT_OPTIONAL_LONG_NAMED_BOOLEAN_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_named_empty_coerce",
                JIT_OPTIONAL_LONG_NAMED_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_named_empty_coerce",
                JIT_OPTIONAL_BOOL_NAMED_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_named_empty_coerce",
                JIT_OPTIONAL_DATE_NAMED_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_named_currency_coerce",
                JIT_OPTIONAL_DOUBLE_NAMED_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_named_long_coerce",
                JIT_OPTIONAL_CURRENCY_NAMED_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_named_long_coerce",
                JIT_OPTIONAL_SINGLE_NAMED_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_named_long_overflow",
                JIT_OPTIONAL_BYTE_NAMED_LONG_OVERFLOW_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_named_long_overflow",
                JIT_OPTIONAL_INTEGER_NAMED_LONG_OVERFLOW_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_named_error_coerce_error",
                JIT_OPTIONAL_LONG_NAMED_ERROR_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_explicit_variant_double_coerce",
                JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_explicit_variant_long_coerce",
                JIT_OPTIONAL_DOUBLE_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_explicit_variant_double_coerce",
                JIT_OPTIONAL_CURRENCY_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_explicit_variant_long_coerce",
                JIT_OPTIONAL_BOOL_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_explicit_variant_integer_coerce",
                JIT_OPTIONAL_BYTE_EXPLICIT_VARIANT_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_explicit_variant_long_coerce",
                JIT_OPTIONAL_INTEGER_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_explicit_variant_long_coerce",
                JIT_OPTIONAL_LONGLONG_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_explicit_variant_double_coerce",
                JIT_OPTIONAL_SINGLE_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_explicit_variant_double_coerce",
                JIT_OPTIONAL_DATE_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_explicit_variant_long_coerce",
                JIT_OPTIONAL_BYTE_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_explicit_variant_byte_coerce",
                JIT_OPTIONAL_INTEGER_EXPLICIT_VARIANT_BYTE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_explicit_variant_double_coerce",
                JIT_OPTIONAL_LONGLONG_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_explicit_variant_currency_coerce",
                JIT_OPTIONAL_SINGLE_EXPLICIT_VARIANT_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_explicit_variant_single_coerce",
                JIT_OPTIONAL_DOUBLE_EXPLICIT_VARIANT_SINGLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_explicit_variant_integer_coerce",
                JIT_OPTIONAL_CURRENCY_EXPLICIT_VARIANT_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_explicit_variant_double_zero_coerce",
                JIT_OPTIONAL_BOOL_EXPLICIT_VARIANT_DOUBLE_ZERO_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_explicit_variant_long_coerce",
                JIT_OPTIONAL_DATE_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_explicit_variant_null_coerce_error",
                JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_explicit_variant_null_coerce_error",
                JIT_OPTIONAL_BOOL_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_explicit_variant_null_coerce_error",
                JIT_OPTIONAL_BYTE_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_explicit_variant_null_coerce_error",
                JIT_OPTIONAL_INTEGER_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_explicit_variant_null_coerce_error",
                JIT_OPTIONAL_LONGLONG_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_explicit_variant_null_coerce_error",
                JIT_OPTIONAL_SINGLE_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_explicit_variant_null_coerce_error",
                JIT_OPTIONAL_DOUBLE_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_explicit_variant_null_coerce_error",
                JIT_OPTIONAL_CURRENCY_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_explicit_variant_null_coerce_error",
                JIT_OPTIONAL_DATE_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_explicit_variant_boolean_coerce",
                JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_BOOLEAN_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_explicit_variant_empty_coerce",
                JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_explicit_variant_empty_coerce",
                JIT_OPTIONAL_BOOL_EXPLICIT_VARIANT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_explicit_variant_empty_coerce",
                JIT_OPTIONAL_DATE_EXPLICIT_VARIANT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_explicit_variant_currency_coerce",
                JIT_OPTIONAL_DOUBLE_EXPLICIT_VARIANT_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_explicit_variant_long_coerce",
                JIT_OPTIONAL_CURRENCY_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_explicit_variant_long_coerce",
                JIT_OPTIONAL_SINGLE_EXPLICIT_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_explicit_variant_long_overflow",
                JIT_OPTIONAL_BYTE_EXPLICIT_VARIANT_LONG_OVERFLOW_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_explicit_variant_long_overflow",
                JIT_OPTIONAL_INTEGER_EXPLICIT_VARIANT_LONG_OVERFLOW_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_explicit_variant_error_coerce_error",
                JIT_OPTIONAL_LONG_EXPLICIT_VARIANT_ERROR_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_named_variant_double_coerce",
                JIT_OPTIONAL_LONG_NAMED_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_named_variant_long_coerce",
                JIT_OPTIONAL_DOUBLE_NAMED_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_named_variant_double_coerce",
                JIT_OPTIONAL_CURRENCY_NAMED_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_named_variant_long_coerce",
                JIT_OPTIONAL_BOOL_NAMED_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_named_variant_integer_coerce",
                JIT_OPTIONAL_BYTE_NAMED_VARIANT_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_named_variant_long_coerce",
                JIT_OPTIONAL_INTEGER_NAMED_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_named_variant_long_coerce",
                JIT_OPTIONAL_LONGLONG_NAMED_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_named_variant_double_coerce",
                JIT_OPTIONAL_SINGLE_NAMED_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_named_variant_double_coerce",
                JIT_OPTIONAL_DATE_NAMED_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_named_variant_long_coerce",
                JIT_OPTIONAL_BYTE_NAMED_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_named_variant_byte_coerce",
                JIT_OPTIONAL_INTEGER_NAMED_VARIANT_BYTE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_named_variant_double_coerce",
                JIT_OPTIONAL_LONGLONG_NAMED_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_named_variant_currency_coerce",
                JIT_OPTIONAL_SINGLE_NAMED_VARIANT_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_named_variant_single_coerce",
                JIT_OPTIONAL_DOUBLE_NAMED_VARIANT_SINGLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_named_variant_integer_coerce",
                JIT_OPTIONAL_CURRENCY_NAMED_VARIANT_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_named_variant_double_zero_coerce",
                JIT_OPTIONAL_BOOL_NAMED_VARIANT_DOUBLE_ZERO_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_named_variant_long_coerce",
                JIT_OPTIONAL_DATE_NAMED_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_named_variant_null_coerce_error",
                JIT_OPTIONAL_LONG_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_named_variant_null_coerce_error",
                JIT_OPTIONAL_BOOL_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_named_variant_null_coerce_error",
                JIT_OPTIONAL_BYTE_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_named_variant_null_coerce_error",
                JIT_OPTIONAL_INTEGER_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_longlong_named_variant_null_coerce_error",
                JIT_OPTIONAL_LONGLONG_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_named_variant_null_coerce_error",
                JIT_OPTIONAL_SINGLE_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_named_variant_null_coerce_error",
                JIT_OPTIONAL_DOUBLE_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_named_variant_null_coerce_error",
                JIT_OPTIONAL_CURRENCY_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_named_variant_null_coerce_error",
                JIT_OPTIONAL_DATE_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_named_variant_boolean_coerce",
                JIT_OPTIONAL_LONG_NAMED_VARIANT_BOOLEAN_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_named_variant_empty_coerce",
                JIT_OPTIONAL_LONG_NAMED_VARIANT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_bool_named_variant_empty_coerce",
                JIT_OPTIONAL_BOOL_NAMED_VARIANT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_date_named_variant_empty_coerce",
                JIT_OPTIONAL_DATE_NAMED_VARIANT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_double_named_variant_currency_coerce",
                JIT_OPTIONAL_DOUBLE_NAMED_VARIANT_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_currency_named_variant_long_coerce",
                JIT_OPTIONAL_CURRENCY_NAMED_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_single_named_variant_long_coerce",
                JIT_OPTIONAL_SINGLE_NAMED_VARIANT_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_byte_named_variant_long_overflow",
                JIT_OPTIONAL_BYTE_NAMED_VARIANT_LONG_OVERFLOW_CALL,
            ),
            render_jit_scope(
                "calls/optional_integer_named_variant_long_overflow",
                JIT_OPTIONAL_INTEGER_NAMED_VARIANT_LONG_OVERFLOW_CALL,
            ),
            render_jit_scope(
                "calls/optional_long_named_variant_error_coerce_error",
                JIT_OPTIONAL_LONG_NAMED_VARIANT_ERROR_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_default",
                JIT_OPTIONAL_STRING_DEFAULT_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_omitted",
                JIT_OPTIONAL_STRING_OMITTED_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_string_literal",
                JIT_OPTIONAL_STRING_EXPLICIT_STRING_LITERAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_string_local",
                JIT_OPTIONAL_STRING_EXPLICIT_STRING_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_empty_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_null_coerce_error",
                JIT_OPTIONAL_STRING_EXPLICIT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_error_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_ERROR_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_decimal_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_DECIMAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_numeric_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_NUMERIC_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_boolean_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_BOOLEAN_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_double_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_single_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_SINGLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_currency_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_integer_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_byte_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_BYTE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_longlong_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_LONGLONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_date_literal_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_DATE_LITERAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_dateserial_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_DATESERIAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_cdate_numeric_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_CDATE_NUMERIC_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_cdate_string_literal_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_CDATE_STRING_LITERAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_cdate_month_name_literal_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_CDATE_MONTH_NAME_LITERAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_cdate_invalid_string_literal_error",
                JIT_OPTIONAL_STRING_EXPLICIT_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_cdate_string_local_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_CDATE_STRING_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_cdate_month_name_local_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_CDATE_MONTH_NAME_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_cdate_invalid_string_local_error",
                JIT_OPTIONAL_STRING_EXPLICIT_CDATE_INVALID_STRING_LOCAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_date_local_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_DATE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_boolean_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_BOOLEAN_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_double_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_single_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_SINGLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_currency_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_integer_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_byte_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_BYTE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_longlong_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_LONGLONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_date_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_DATE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_string_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_STRING_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_empty_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_null_coerce_error",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_error_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_ERROR_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_explicit_variant_decimal_coerce",
                JIT_OPTIONAL_STRING_EXPLICIT_VARIANT_DECIMAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_numeric_coerce",
                JIT_OPTIONAL_STRING_NAMED_NUMERIC_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_long_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_LONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_date_local_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_DATE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_dateserial_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_DATESERIAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_cdate_numeric_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_CDATE_NUMERIC_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_cdate_month_name_local_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_CDATE_MONTH_NAME_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_cdate_invalid_string_literal_error",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_empty_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_error_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_ERROR_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_boolean_local_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_BOOLEAN_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_currency_local_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_CURRENCY_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_double_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_single_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_SINGLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_integer_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_byte_local_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_BYTE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_longlong_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_LONGLONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_variant_decimal_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_DECIMAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_variant_boolean_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_BOOLEAN_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_variant_double_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_variant_single_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_SINGLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_variant_currency_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_variant_integer_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_variant_byte_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_BYTE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_variant_longlong_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_LONGLONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_variant_date_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_DATE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_variant_string_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_STRING_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_variant_error_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_ERROR_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_variant_empty_coerce",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_VARIANT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_arg_order_null_coerce_error",
                JIT_OPTIONAL_STRING_NAMED_ARG_ORDER_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_string_literal",
                JIT_OPTIONAL_STRING_NAMED_STRING_LITERAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_string_local",
                JIT_OPTIONAL_STRING_NAMED_STRING_LOCAL_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_empty_coerce",
                JIT_OPTIONAL_STRING_NAMED_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_null_coerce_error",
                JIT_OPTIONAL_STRING_NAMED_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_error_coerce",
                JIT_OPTIONAL_STRING_NAMED_ERROR_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_decimal_coerce",
                JIT_OPTIONAL_STRING_NAMED_DECIMAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_boolean_coerce",
                JIT_OPTIONAL_STRING_NAMED_BOOLEAN_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_double_coerce",
                JIT_OPTIONAL_STRING_NAMED_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_single_coerce",
                JIT_OPTIONAL_STRING_NAMED_SINGLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_currency_coerce",
                JIT_OPTIONAL_STRING_NAMED_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_integer_coerce",
                JIT_OPTIONAL_STRING_NAMED_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_byte_coerce",
                JIT_OPTIONAL_STRING_NAMED_BYTE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_longlong_coerce",
                JIT_OPTIONAL_STRING_NAMED_LONGLONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_date_literal_coerce",
                JIT_OPTIONAL_STRING_NAMED_DATE_LITERAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_dateserial_coerce",
                JIT_OPTIONAL_STRING_NAMED_DATESERIAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_cdate_numeric_coerce",
                JIT_OPTIONAL_STRING_NAMED_CDATE_NUMERIC_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_cdate_string_literal_coerce",
                JIT_OPTIONAL_STRING_NAMED_CDATE_STRING_LITERAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_cdate_month_name_literal_coerce",
                JIT_OPTIONAL_STRING_NAMED_CDATE_MONTH_NAME_LITERAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_cdate_invalid_string_literal_error",
                JIT_OPTIONAL_STRING_NAMED_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_cdate_string_local_coerce",
                JIT_OPTIONAL_STRING_NAMED_CDATE_STRING_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_cdate_month_name_local_coerce",
                JIT_OPTIONAL_STRING_NAMED_CDATE_MONTH_NAME_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_cdate_invalid_string_local_error",
                JIT_OPTIONAL_STRING_NAMED_CDATE_INVALID_STRING_LOCAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_date_local_coerce",
                JIT_OPTIONAL_STRING_NAMED_DATE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_boolean_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_BOOLEAN_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_double_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_DOUBLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_single_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_SINGLE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_currency_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_CURRENCY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_integer_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_INTEGER_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_byte_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_BYTE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_longlong_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_LONGLONG_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_date_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_DATE_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_string_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_STRING_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_empty_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_EMPTY_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_null_coerce_error",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_NULL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_error_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_ERROR_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/optional_string_named_variant_decimal_coerce",
                JIT_OPTIONAL_STRING_NAMED_VARIANT_DECIMAL_COERCE_CALL,
            ),
            render_jit_scope("inline/variant_box_assignment", JIT_VARIANT_BOX_ASSIGNMENT),
            render_jit_scope("calls/variant_return", JIT_VARIANT_RETURN_CALL),
            render_jit_scope("calls/variant_byref", JIT_VARIANT_BYREF_CALL),
            render_jit_scope("calls/string_byval_return", JIT_STRING_BYVAL_RETURN_CALL),
            render_jit_scope(
                "calls/string_byval_local_return",
                JIT_STRING_BYVAL_LOCAL_RETURN_CALL,
            ),
            render_jit_scope("calls/string_byref", JIT_STRING_BYREF_CALL),
            render_jit_scope(
                "calls/string_mixed_byref_byval",
                JIT_STRING_MIXED_BYREF_BYVAL_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_numeric_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_NUMERIC_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_long_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_LONG_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_boolean_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_BOOLEAN_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_double_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_DOUBLE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_single_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_SINGLE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_currency_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_CURRENCY_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_integer_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_INTEGER_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_byte_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_BYTE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_longlong_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_LONGLONG_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_date_literal_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_DATE_LITERAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_date_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_DATE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_dateserial_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_DATESERIAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_cdate_numeric_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_CDATE_NUMERIC_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_cdate_string_literal_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_CDATE_STRING_LITERAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_cdate_string_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_CDATE_STRING_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_cdate_invalid_string_literal_error",
                JIT_STRING_MIXED_BYREF_BYVAL_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_cdate_invalid_string_local_error",
                JIT_STRING_MIXED_BYREF_BYVAL_CDATE_INVALID_STRING_LOCAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_cdate_month_name_literal_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_CDATE_MONTH_NAME_LITERAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_cdate_month_name_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_CDATE_MONTH_NAME_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_boolean_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_BOOLEAN_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_double_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_DOUBLE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_single_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_SINGLE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_currency_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_CURRENCY_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_integer_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_INTEGER_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_byte_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_BYTE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_longlong_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_LONGLONG_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_date_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_DATE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_string_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_STRING_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_error_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_ERROR_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_decimal_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_DECIMAL_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_empty_local_coerce",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_EMPTY_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_mixed_byref_byval_variant_null_local_coerce_error",
                JIT_STRING_MIXED_BYREF_BYVAL_VARIANT_NULL_LOCAL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_return",
                JIT_STRING_NAMED_BYVAL_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_numeric_coerce_return",
                JIT_STRING_NAMED_BYVAL_NUMERIC_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_long_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_LONG_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_boolean_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_BOOLEAN_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_double_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_DOUBLE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_single_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_SINGLE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_currency_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_CURRENCY_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_integer_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_INTEGER_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_byte_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_BYTE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_longlong_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_LONGLONG_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_boolean_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_BOOLEAN_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_double_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_DOUBLE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_single_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_SINGLE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_currency_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_CURRENCY_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_integer_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_INTEGER_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_byte_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_BYTE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_longlong_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_LONGLONG_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_date_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_DATE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_string_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_STRING_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_error_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_ERROR_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_decimal_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_DECIMAL_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_empty_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_VARIANT_EMPTY_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_variant_null_local_coerce_error",
                JIT_STRING_NAMED_BYVAL_VARIANT_NULL_LOCAL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_date_literal_coerce_return",
                JIT_STRING_NAMED_BYVAL_DATE_LITERAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_date_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_DATE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_dateserial_coerce_return",
                JIT_STRING_NAMED_BYVAL_DATESERIAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_cdate_numeric_coerce_return",
                JIT_STRING_NAMED_BYVAL_CDATE_NUMERIC_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_cdate_string_literal_coerce_return",
                JIT_STRING_NAMED_BYVAL_CDATE_STRING_LITERAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_cdate_string_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_CDATE_STRING_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_cdate_invalid_string_literal_error",
                JIT_STRING_NAMED_BYVAL_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_cdate_invalid_string_local_error",
                JIT_STRING_NAMED_BYVAL_CDATE_INVALID_STRING_LOCAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_cdate_month_name_literal_coerce_return",
                JIT_STRING_NAMED_BYVAL_CDATE_MONTH_NAME_LITERAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_byval_cdate_month_name_local_coerce_return",
                JIT_STRING_NAMED_BYVAL_CDATE_MONTH_NAME_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_numeric_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_NUMERIC_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_long_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_LONG_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_boolean_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_BOOLEAN_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_double_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_DOUBLE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_single_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_SINGLE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_currency_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CURRENCY_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_integer_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_INTEGER_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_byte_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_BYTE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_longlong_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_LONGLONG_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_date_literal_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_DATE_LITERAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_date_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_DATE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_dateserial_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_DATESERIAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_cdate_numeric_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_NUMERIC_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_cdate_string_literal_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_STRING_LITERAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_cdate_string_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_STRING_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_cdate_invalid_string_literal_error",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_cdate_invalid_string_local_error",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_INVALID_STRING_LOCAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_cdate_month_name_literal_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_MONTH_NAME_LITERAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_cdate_month_name_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_CDATE_MONTH_NAME_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_boolean_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_BOOLEAN_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_double_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_DOUBLE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_single_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_SINGLE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_currency_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_CURRENCY_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_integer_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_INTEGER_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_byte_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_BYTE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_longlong_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_LONGLONG_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_date_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_DATE_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_string_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_STRING_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_error_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_ERROR_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_decimal_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_DECIMAL_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_empty_local_coerce",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_EMPTY_LOCAL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/string_named_mixed_byref_byval_variant_null_local_coerce_error",
                JIT_STRING_NAMED_MIXED_BYREF_BYVAL_VARIANT_NULL_LOCAL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_numeric_coerce_return",
                JIT_STRING_BYVAL_NUMERIC_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_long_local_coerce_return",
                JIT_STRING_BYVAL_LONG_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_boolean_local_coerce_return",
                JIT_STRING_BYVAL_BOOLEAN_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_double_local_coerce_return",
                JIT_STRING_BYVAL_DOUBLE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_single_local_coerce_return",
                JIT_STRING_BYVAL_SINGLE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_currency_local_coerce_return",
                JIT_STRING_BYVAL_CURRENCY_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_integer_local_coerce_return",
                JIT_STRING_BYVAL_INTEGER_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_byte_local_coerce_return",
                JIT_STRING_BYVAL_BYTE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_longlong_local_coerce_return",
                JIT_STRING_BYVAL_LONGLONG_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_date_literal_coerce_return",
                JIT_STRING_BYVAL_DATE_LITERAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_date_local_coerce_return",
                JIT_STRING_BYVAL_DATE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_dateserial_coerce_return",
                JIT_STRING_BYVAL_DATESERIAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_cdate_numeric_coerce_return",
                JIT_STRING_BYVAL_CDATE_NUMERIC_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_cdate_string_literal_coerce_return",
                JIT_STRING_BYVAL_CDATE_STRING_LITERAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_cdate_string_local_coerce_return",
                JIT_STRING_BYVAL_CDATE_STRING_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_cdate_invalid_string_literal_error",
                JIT_STRING_BYVAL_CDATE_INVALID_STRING_LITERAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_cdate_invalid_string_local_error",
                JIT_STRING_BYVAL_CDATE_INVALID_STRING_LOCAL_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_cdate_month_name_literal_coerce_return",
                JIT_STRING_BYVAL_CDATE_MONTH_NAME_LITERAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_cdate_month_name_local_coerce_return",
                JIT_STRING_BYVAL_CDATE_MONTH_NAME_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_boolean_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_BOOLEAN_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_double_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_DOUBLE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_single_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_SINGLE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_currency_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_CURRENCY_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_integer_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_INTEGER_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_byte_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_BYTE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_longlong_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_LONGLONG_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_date_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_DATE_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_string_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_STRING_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_error_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_ERROR_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_decimal_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_DECIMAL_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_empty_local_coerce_return",
                JIT_STRING_BYVAL_VARIANT_EMPTY_LOCAL_COERCE_RETURN_CALL,
            ),
            render_jit_scope(
                "calls/string_byval_variant_null_local_coerce_error",
                JIT_STRING_BYVAL_VARIANT_NULL_LOCAL_COERCE_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/long_return_to_variant",
                JIT_LONG_RETURN_TO_VARIANT_CALL,
            ),
            render_jit_scope(
                "calls/string_return_to_variant",
                JIT_STRING_RETURN_TO_VARIANT_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_long",
                JIT_VARIANT_RETURN_TO_LONG_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_string_coerce",
                JIT_VARIANT_RETURN_TO_STRING_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_bool_coerce",
                JIT_VARIANT_RETURN_TO_BOOL_COERCE_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_double",
                JIT_VARIANT_RETURN_TO_DOUBLE_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_string_boolean_payload",
                JIT_VARIANT_RETURN_TO_STRING_BOOLEAN_PAYLOAD_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_string_double_payload",
                JIT_VARIANT_RETURN_TO_STRING_DOUBLE_PAYLOAD_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_string_single_payload",
                JIT_VARIANT_RETURN_TO_STRING_SINGLE_PAYLOAD_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_string_currency_payload",
                JIT_VARIANT_RETURN_TO_STRING_CURRENCY_PAYLOAD_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_string_integer_payload",
                JIT_VARIANT_RETURN_TO_STRING_INTEGER_PAYLOAD_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_string_byte_payload",
                JIT_VARIANT_RETURN_TO_STRING_BYTE_PAYLOAD_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_string_longlong_payload",
                JIT_VARIANT_RETURN_TO_STRING_LONGLONG_PAYLOAD_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_string_string_payload",
                JIT_VARIANT_RETURN_TO_STRING_STRING_PAYLOAD_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_string_date_payload",
                JIT_VARIANT_RETURN_TO_STRING_DATE_PAYLOAD_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_string_error_payload",
                JIT_VARIANT_RETURN_TO_STRING_ERROR_PAYLOAD_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_to_string_decimal_payload",
                JIT_VARIANT_RETURN_TO_STRING_DECIMAL_PAYLOAD_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_empty_to_string",
                JIT_VARIANT_RETURN_EMPTY_TO_STRING_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_null_to_long_error",
                JIT_VARIANT_RETURN_NULL_TO_LONG_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/variant_return_null_to_string_error",
                JIT_VARIANT_RETURN_NULL_TO_STRING_ERROR_CALL,
            ),
            render_jit_scope(
                "calls/scalar_returns_to_variant",
                JIT_SCALAR_RETURNS_TO_VARIANT_CALL,
            ),
            render_jit_scope(
                "calls/mixed_byref_byval_long",
                JIT_MIXED_BYREF_BYVAL_LONG_CALL,
            ),
            render_jit_scope("calls/integer_byref", JIT_INTEGER_BYREF_CALL),
            render_jit_scope("calls/bool_byval", JIT_BOOL_BYVAL_CALL),
            render_jit_scope("calls/byte_byval", JIT_BYTE_BYVAL_CALL),
            render_jit_scope("calls/byte_byref", JIT_BYTE_BYREF_CALL),
            render_jit_scope("calls/currency_byref", JIT_CURRENCY_BYREF_CALL),
            render_jit_scope("calls/currency_byval", JIT_CURRENCY_BYVAL_CALL),
            render_jit_scope("calls/currency_return", JIT_CURRENCY_RETURN_CALL),
            render_jit_scope("calls/date_byref", JIT_DATE_BYREF_CALL),
            render_jit_scope("calls/date_byval", JIT_DATE_BYVAL_CALL),
            render_jit_scope("calls/date_return", JIT_DATE_RETURN_CALL),
            render_jit_scope("inline/date_compare_expr", JIT_DATE_COMPARE_EXPR),
            render_jit_scope("inline/date_truthy_expr", JIT_DATE_TRUTHY_EXPR),
            render_jit_scope("inline/date_arithmetic", JIT_DATE_ARITHMETIC),
            render_jit_scope("calls/double_byval", JIT_DOUBLE_BYVAL_CALL),
            render_jit_scope("calls/double_byref", JIT_DOUBLE_BYREF_CALL),
            render_jit_scope("calls/double_return", JIT_DOUBLE_RETURN_CALL),
            render_jit_scope("inline/double_compare_expr", JIT_DOUBLE_COMPARE_EXPR),
            render_jit_scope("inline/double_truthy_expr", JIT_DOUBLE_TRUTHY_EXPR),
            render_jit_scope("inline/double_arithmetic", JIT_DOUBLE_ARITHMETIC),
            render_jit_scope("calls/single_byval", JIT_SINGLE_BYVAL_CALL),
            render_jit_scope("calls/single_byref", JIT_SINGLE_BYREF_CALL),
            render_jit_scope("calls/single_return", JIT_SINGLE_RETURN_CALL),
            render_jit_scope("inline/single_compare_expr", JIT_SINGLE_COMPARE_EXPR),
            render_jit_scope("inline/single_truthy_expr", JIT_SINGLE_TRUTHY_EXPR),
            render_jit_scope("inline/single_arithmetic", JIT_SINGLE_ARITHMETIC),
            render_jit_scope("calls/integer_return", JIT_INTEGER_RETURN_CALL),
            render_jit_scope("calls/byte_return", JIT_BYTE_RETURN_CALL),
            render_jit_scope("inline/byte_arithmetic", JIT_BYTE_ARITHMETIC),
            render_jit_scope("inline/integer_arithmetic", JIT_INTEGER_ARITHMETIC),
            render_jit_scope("calls/longlong_byref", JIT_LONGLONG_BYREF_CALL),
            render_jit_scope("calls/longlong_byval", JIT_LONGLONG_BYVAL_CALL),
            render_jit_scope("calls/longlong_return", JIT_LONGLONG_RETURN_CALL),
            render_jit_scope("calls/longptr_byval_return", JIT_LONGPTR_BYVAL_RETURN_CALL),
            render_jit_scope("inline/longlong_compare_expr", JIT_LONGLONG_COMPARE_EXPR),
            render_jit_scope(
                "inline/mixed_fixed_integer_compare_expr",
                JIT_MIXED_FIXED_INTEGER_COMPARE_EXPR,
            ),
            render_jit_scope("inline/longlong_truthy_expr", JIT_LONGLONG_TRUTHY_EXPR),
            render_jit_scope("calls/bool_return", JIT_BOOL_RETURN_CALL),
            render_jit_scope(
                "inline/bool_numeric_assignment",
                JIT_BOOL_NUMERIC_ASSIGNMENT,
            ),
            render_jit_scope("inline/bool_logical_expr", JIT_BOOL_LOGICAL_EXPR),
            render_jit_scope("inline/long_logical_expr", JIT_LONG_LOGICAL_EXPR),
            render_jit_scope(
                "inline/fixed_integer_logical_expr",
                JIT_FIXED_INTEGER_LOGICAL_EXPR,
            ),
            render_jit_scope("inline/longlong_logical_expr", JIT_LONGLONG_LOGICAL_EXPR),
            render_jit_scope("inline/longlong_eqv_expr", JIT_LONGLONG_EQV_EXPR),
            render_jit_scope("inline/longlong_not_expr", JIT_LONGLONG_NOT_EXPR),
            render_jit_scope(
                "inline/longlong_mixed_logical_expr",
                JIT_LONGLONG_MIXED_LOGICAL_EXPR,
            ),
            render_jit_scope("inline/variant_logical_expr", JIT_VARIANT_LOGICAL_EXPR),
            render_jit_scope(
                "inline/variant_logical_numeric_expr",
                JIT_VARIANT_LOGICAL_NUMERIC_EXPR,
            ),
            render_jit_scope("inline/variant_truthy_expr", JIT_VARIANT_TRUTHY_EXPR),
            render_jit_scope("inline/variant_compare_expr", JIT_VARIANT_COMPARE_EXPR),
            render_jit_scope(
                "inline/variant_compare_numeric_expr",
                JIT_VARIANT_COMPARE_NUMERIC_EXPR,
            ),
            render_jit_scope(
                "inline/variant_arithmetic_null_expr",
                JIT_VARIANT_ARITHMETIC_NULL_EXPR,
            ),
            render_jit_scope(
                "inline/variant_arithmetic_mixed_expr",
                JIT_VARIANT_ARITHMETIC_MIXED_EXPR,
            ),
            render_jit_scope("inline/variant_negation_expr", JIT_VARIANT_NEGATION_EXPR),
            render_jit_scope(
                "inline/variant_byte_coerce_expr",
                JIT_VARIANT_BYTE_COERCE_EXPR,
            ),
            render_jit_scope(
                "inline/variant_integer_coerce_expr",
                JIT_VARIANT_INTEGER_COERCE_EXPR,
            ),
            render_jit_scope(
                "inline/variant_long_coerce_expr",
                JIT_VARIANT_LONG_COERCE_EXPR,
            ),
            render_jit_scope(
                "inline/variant_longlong_coerce_expr",
                JIT_VARIANT_LONGLONG_COERCE_EXPR,
            ),
            render_jit_scope(
                "inline/variant_single_coerce_expr",
                JIT_VARIANT_SINGLE_COERCE_EXPR,
            ),
            render_jit_scope(
                "inline/variant_double_coerce_expr",
                JIT_VARIANT_DOUBLE_COERCE_EXPR,
            ),
            render_jit_scope(
                "inline/variant_currency_coerce_expr",
                JIT_VARIANT_CURRENCY_COERCE_EXPR,
            ),
            render_jit_scope(
                "inline/variant_date_coerce_expr",
                JIT_VARIANT_DATE_COERCE_EXPR,
            ),
            render_jit_scope(
                "inline/variant_bool_coerce_expr",
                JIT_VARIANT_BOOL_COERCE_EXPR,
            ),
        ];
        lines.sort();
        let actual = format!("{}\n", lines.join("\n"));
        let golden = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("jit_scope.snap");
        if std::env::var_os("OXVBA_BLESS_JIT_SCOPE").is_some() {
            std::fs::write(&golden, &actual).expect("write jit scope snapshot");
            eprintln!("blessed jit scope snapshot: {} cases", lines.len());
            return;
        }
        let expected = std::fs::read_to_string(&golden).unwrap_or_default();
        assert_eq!(
            actual, expected,
            "jit scope snapshot drift (re-bless with OXVBA_BLESS_JIT_SCOPE=1 if intended)"
        );
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
                let rendered = match run_with_timeout(Executor::Vm3, &source, "VBAProject", budget)
                {
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
