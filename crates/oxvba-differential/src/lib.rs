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

use oxvba_host::{Engine, HostConfig};
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

/// Which execution backend to run a program under. Only [`Executor::Vm2`] is wired
/// today; `Vm3` and `Jit` land in M2/M4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Executor {
    /// The legacy `Op`-bundle interpreter — the golden oracle until vm3 parity.
    Vm2,
}

/// The observable outcome of one run, for differential comparison.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// Axis 1 (return values): the canonical snapshot of the entry project's globals
    /// followed by the entry `Sub Main` locals — or the rendered phase diagnostic if
    /// the run did not produce a snapshot (a coarse stand-in for axis 2 until M3).
    pub result: Result<Vec<Canon>, String>,
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
            RunOutcome { result }
        }
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
}
