//! Shared differential-COM test machinery for the `com_matrix_*` category files.
//!
//! This file is **not** a `#[test]` file: it carries only helpers and is
//! `#[path]`-included into each category binary (`com_matrix_properties.rs`,
//! `com_matrix_methods.rs`, …), so it contributes **no tests** of its own. Every
//! category file begins with:
//!
//! ```ignore
//! #![cfg(target_os = "windows")]
//! #[path = "com_matrix_common.rs"]
//! mod common;
//! use common::*;
//! ```
//!
//! ## The differential principle
//!
//! Every scenario runs the *same* observable behaviour three ways and asserts the
//! three AGREE — any divergence is a bug:
//!
//! 1. **late-bound** — `Dim x As Object` + `CreateObject` (by-name `LateDispatch`);
//! 2. **early-bound, `PreferVtable`** — typed receiver + typelib reference, vtable
//!    fast path preferred (so eligible members slot-call the COM vtable);
//! 3. **early-bound, `DispatchOnly`** — typed receiver + typelib reference, but the
//!    vtable bridge is never touched (transport counter stays 0).
//!
//! Leg (1) vs (2) catches a *binder* divergence (late `As Object` vs early typed);
//! leg (2) vs (3) catches a *transport* divergence (vtable vs IDispatch) — the only
//! way to see a vtable bug when both `late` and `early-DispatchOnly` would otherwise
//! ride IDispatch. Plus an exact transport-count assertion proves WHICH transport
//! ran (the per-bridge counter is zeroed fresh on each `Engine::new`), and the
//! test reaching its assertions is itself the no-host-AV guard.
//!
//! Windows-only; every category test is `#[ignore]` (live COM) with an env-skip on
//! an absent component / typelib.
#![cfg(target_os = "windows")]
#![allow(dead_code)] // each category file uses a different subset of these helpers.

use std::path::PathBuf;

use oxvba_hal::model::{ComInvocationStrategy, HostPolicy};
use oxvba_host::{Engine, HostConfig};
use oxvba_symbol::manifest::{self as sym, ProjectReference};

// Re-exported so category files can name `Variant` in their own local differential
// helpers (they `use common::*`).
pub use oxvba_runtime::Variant;

/// A late-bound run result: just the snapshot (or an error string).
pub type LateRun = Result<Vec<Variant>, String>;

/// An early-bound run result: the snapshot plus the bridge's
/// `(vtable_count, idispatch_count)` transport counts (or an error string).
pub type EarlyRun = Result<(Vec<Variant>, (u64, u64)), String>;

// ─────────────────────────────────────────────────────────────────────────────
// Existing harness helpers (copied verbatim from `com_office_integration.rs` so
// the new category files do not depend on the original smoke-baseline file).
// ─────────────────────────────────────────────────────────────────────────────

/// The placeholder a side-effecting scenario embeds in any per-run-unique target
/// (e.g. a DAO `CreateDatabase` path). The differential runner runs the SAME VBA
/// up to three times (late, early-PreferVtable, early-DispatchOnly); each leg
/// substitutes its own [`LEG_*`](LEG_LATE) token here so the three legs never
/// collide on a shared side effect. A source without the marker is unaffected.
pub const LEG_PLACEHOLDER: &str = "%%LEG%%";
/// Per-leg substitution token for the late-bound (`run_clean`) leg.
pub const LEG_LATE: &str = "late";
/// Per-leg substitution token for the early-bound `PreferVtable` leg.
pub const LEG_EARLY_VTABLE: &str = "earlyvt";
/// Per-leg substitution token for the early-bound `DispatchOnly` leg.
pub const LEG_EARLY_DISPATCH: &str = "earlydo";
/// The full set of leg tokens — used by [`TempDbPath`] teardown to remove every
/// per-leg side-effect target a scenario may have produced.
pub const ALL_LEG_TOKENS: &[&str] = &[LEG_LATE, LEG_EARLY_VTABLE, LEG_EARLY_DISPATCH];

/// Substitute the per-leg token into a scenario source for one differential leg
/// (a no-op when the source carries no [`LEG_PLACEHOLDER`]).
fn with_leg_token(source: &str, leg: &str) -> String {
    source.replace(LEG_PLACEHOLDER, leg)
}

/// Run a single-module VBA source through the clean Engine under the
/// interactive-dev policy (which permits real COM activation), returning the
/// snapshot (module globals followed by `Main`'s locals). This is the LATE-bound
/// leg's runner: the source uses `Dim x As Object` + `CreateObject`.
pub fn run_clean(source: &str) -> Result<Vec<Variant>, String> {
    let source = with_leg_token(source, LEG_LATE);
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_source_with_variant_snapshot_clean(&source)
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()))
}

/// Run a single-module VBA source that carries typelib `references` through the
/// clean Engine under the interactive-dev policy, so a typed receiver resolves to
/// early-bound COM dispatch (dispatch by dispid). Same snapshot as [`run_clean`].
///
/// This is the early-bound leg used by the two-leg error oracle; it substitutes
/// the `PreferVtable` per-leg token so it does not collide with [`run_clean`]'s
/// late leg on a shared side effect.
pub fn run_clean_with_references(
    source: &str,
    references: Vec<ProjectReference>,
) -> Result<Vec<Variant>, String> {
    let source = with_leg_token(source, LEG_EARLY_VTABLE);
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_source_with_references_and_snapshot(&source, references)
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()))
}

/// Like [`run_clean_with_references`] but runs under a `PreferVtable` COM
/// invocation strategy (the early-bound vtable fast path) and additionally
/// returns the bridge's `(vtable_call_count, idispatch_call_count)` transport
/// counts after the run, so a live test can prove that the typed-receiver member
/// calls dispatched through the COM vtable rather than `IDispatch::Invoke`.
///
/// This is the early-bound **PreferVtable** leg.
pub fn run_clean_with_references_prefer_vtable(
    source: &str,
    references: Vec<ProjectReference>,
) -> EarlyRun {
    let source = with_leg_token(source, LEG_EARLY_VTABLE);
    let mut policy = HostPolicy::interactive_dev();
    policy.com_invocation_strategy = ComInvocationStrategy::PreferVtable;
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(policy);
    let snapshot = engine
        .execute_source_with_references_and_snapshot(&source, references)
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()))?;
    // The engine retains its host services (and thus the COM bridge) after the
    // run, so the accumulated transport counts are still readable here.
    let counts = engine.host_services().com().com_dispatch_transport_counts();
    Ok((snapshot, counts))
}

/// A typelib reference identified by its LIBID GUID (the way the early-bound tests
/// point at a registered typelib). The live resolver loads it via `LoadRegTypeLib`.
pub fn typelib_ref_by_libid(name: &str, guid: &str, major: u16, minor: u16) -> ProjectReference {
    ProjectReference::TypeLibrary {
        name: name.to_string(),
        guid: Some(guid.to_string()),
        version_major: Some(major),
        version_minor: Some(minor),
        lcid: None,
        import_lib: None,
    }
}

/// True if an error denotes an absent/unregistered typelib (the early-bound
/// counterpart of [`is_component_absent`]): the reference never resolved, so the
/// typed receiver had no COM members and the member call failed to bind/dispatch.
pub fn is_typelib_absent(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("loadregtypelib")
        || lower.contains("type library")
        || lower.contains("typelib")
        || lower.contains("0x8002801d") // TYPE_E_LIBNOTREGISTERED
        || lower.contains("8002801d")
        || is_component_absent(err)
}

/// Whether an error denotes an absent COM component (class not registered /
/// invalid class string) rather than a real failure — an environment skip.
pub fn is_component_absent(err: &str) -> bool {
    err.contains("8004_0154")
        || err.contains("80040154")
        || err.contains("8004_01F3")
        || err.contains("800401F3")
        || err.to_ascii_lowercase().contains("not registered")
        || err.to_ascii_lowercase().contains("invalid class string")
}

/// A unique, PER-LEG temp path template for a test database, removed up front and
/// on teardown so the run is repeatable. `tag` keeps concurrent scenarios from
/// colliding; the embedded [`LEG_PLACEHOLDER`] keeps the three differential legs
/// (late / early-vtable / early-dispatch) from colliding on a SHARED side effect
/// — each leg's runner substitutes its own token, so `eng.CreateDatabase` writes
/// a distinct file per leg and legs 2-3 no longer fail "Database already exists".
pub struct TempDbPath {
    /// Filesystem path template, with [`LEG_PLACEHOLDER`] still un-substituted.
    template: PathBuf,
}

impl TempDbPath {
    pub fn new(tag: &str) -> Self {
        let pid = std::process::id();
        let template =
            std::env::temp_dir().join(format!("oxvba_{tag}_{pid}_{LEG_PLACEHOLDER}.accdb"));
        let me = Self { template };
        me.remove_all_legs();
        me
    }

    /// The path for one concrete leg (placeholder substituted).
    fn leg_path(&self, leg: &str) -> PathBuf {
        PathBuf::from(
            self.template
                .to_string_lossy()
                .replace(LEG_PLACEHOLDER, leg),
        )
    }

    fn remove_all_legs(&self) {
        for leg in ALL_LEG_TOKENS {
            let _ = std::fs::remove_file(self.leg_path(leg));
        }
    }

    /// Pre-create the target file for EVERY leg (read-only scenarios such as a
    /// `FileSystemObject.FileExists` check, where each leg's substituted literal
    /// must point at an already-existing file). Returns the count written.
    pub fn write_all_legs(&self, contents: &[u8]) -> usize {
        ALL_LEG_TOKENS
            .iter()
            .filter(|leg| std::fs::write(self.leg_path(leg), contents).is_ok())
            .count()
    }

    /// The VBA string literal for the templated path. Still carries
    /// [`LEG_PLACEHOLDER`]; each differential-leg runner substitutes its own token
    /// before executing, so every leg targets a distinct database file.
    pub fn as_vba_literal(&self) -> String {
        // VBA string literal: backslashes are fine; only `"` needs doubling.
        self.template.to_string_lossy().replace('"', "\"\"")
    }
}

impl Drop for TempDbPath {
    fn drop(&mut self) {
        self.remove_all_legs();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// New helpers (plan §1.2): the DispatchOnly third leg, the class-module manifest
// runner, and the Err extractor.
// ─────────────────────────────────────────────────────────────────────────────

/// Early-bound but EXPLICITLY `DispatchOnly`: a typed receiver + typelib reference,
/// yet the vtable bridge is never touched (the transport counter stays 0). This is
/// the third differential leg — it isolates a *transport* divergence (PreferVtable
/// vs DispatchOnly) from a *binder* divergence (late `As Object` vs early typed).
/// Returns `(snapshot, (vtable, idispatch))`; by construction `vtable == 0`.
pub fn run_clean_with_references_dispatch_only(
    source: &str,
    references: Vec<ProjectReference>,
) -> EarlyRun {
    let source = with_leg_token(source, LEG_EARLY_DISPATCH);
    let mut policy = HostPolicy::interactive_dev();
    policy.com_invocation_strategy = ComInvocationStrategy::DispatchOnly;
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(policy);
    let snap = engine
        .execute_source_with_references_and_snapshot(&source, references)
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()))?;
    let counts = engine.host_services().com().com_dispatch_transport_counts();
    Ok((snap, counts))
}

/// Class-module manifest runner (for `WithEvents` / events and other multi-module
/// scenarios). Mirrors the inline manifest in `com_office_integration.rs`'s event
/// test. Each `(name, kind, source)` becomes a `ModuleUnit`; the project is named
/// `"Main"` so `Main`'s globals sort first in the snapshot.
pub fn run_manifest(
    modules: Vec<(&str, sym::ModuleKind, String)>,
    references: Vec<ProjectReference>,
) -> Result<Vec<Variant>, String> {
    let manifest = sym::SymbolProjectManifest {
        project_name: "Main".to_string(),
        project_kind: sym::ProjectKind::Source,
        modules: modules
            .into_iter()
            .map(|(name, kind, src)| sym::ModuleUnit {
                module_name: name.to_string(),
                module_kind: kind,
                attributes: sym::ModuleAttributes::named(name),
                source: src,
            })
            .collect(),
        references,
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };
    let mut engine = Engine::new(HostConfig { enable_jit: false });
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .execute_manifest_with_variant_snapshot(&manifest)
        .map_err(|d| format!("{:?}: {}", d.phase(), d.message()))
}

/// Extract a by-CONVENTION error capture: the scenario stores `Err.Number` into a
/// Long global, `Err.Description` into a (non-empty) String global, and an optional
/// Boolean verdict. Returns `(number, description, verdict)`.
///
/// NOTE: do NOT use this to locate VALUE oracles — value scenarios reduce to a
/// single composite verdict cell located with [`find_verdict`].
pub fn extract_err(snap: &[Variant]) -> (Option<i32>, Option<String>, Option<bool>) {
    let n = snap.iter().find_map(|v| v.as_i32());
    let d = snap
        .iter()
        .find_map(|v| v.as_bstr().map(|b| b.as_str()))
        .filter(|s| !s.is_empty());
    let b = snap.iter().find_map(|v| v.as_bool());
    (n, d, b)
}

// ─────────────────────────────────────────────────────────────────────────────
// Locating values — the collision-proof rule (plan §1.3). Every value scenario
// reduces to ONE composite verdict global (a single Long) that encodes all its
// sub-checks, so positional `find_map` can never pick the wrong cell.
// ─────────────────────────────────────────────────────────────────────────────

/// The canonical verdict extractor for a value scenario: the FIRST `Long` in the
/// snapshot. Scenarios are authored so the verdict global is declared first and is
/// the only `Long` whose value can equal the expected composite, making this
/// unambiguous. Returned as `i64` so the differential runner can compare across
/// transports uniformly.
pub fn find_verdict(snap: &[Variant]) -> Option<i64> {
    snap.iter().find_map(|v| v.as_i32().map(i64::from))
}

/// Assert the snapshot contains EXACTLY this `i32` carried as a `Long`
/// (strict accessor — proves it did not arrive as some other VARTYPE).
pub fn assert_long(snap: &[Variant], expected: i32, ctx: &str) {
    assert!(
        snap.iter().any(|v| v.as_i32() == Some(expected)),
        "{ctx}: expected Long {expected} in {snap:?}"
    );
}

/// Assert the snapshot contains EXACTLY this `String`.
pub fn assert_str(snap: &[Variant], expected: &str, ctx: &str) {
    assert!(
        snap.iter()
            .any(|v| v.as_bstr().map(|b| b.as_str()).as_deref() == Some(expected)),
        "{ctx}: expected String {expected:?} in {snap:?}"
    );
}

/// Assert the snapshot contains a `True` Boolean flag.
pub fn assert_bool_true(snap: &[Variant], ctx: &str) {
    assert!(
        snap.iter().any(|v| v.as_bool() == Some(true)),
        "{ctx}: expected a True flag in {snap:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// The three core assertions every value scenario carries (plan §1.4).
// ─────────────────────────────────────────────────────────────────────────────

/// THE differential + transport + AV master assertion for a VALUE scenario.
///
/// - `late` — the late-bound (`Dim As Object` + `CreateObject`) run.
/// - `early` — the early-bound `PreferVtable` run + `(vtable, idispatch)` counts.
/// - `early_dispatch_only` — optional early-bound `DispatchOnly` run (the third
///   leg). Pass `None` only for scenarios with no vtable-eligible member.
/// - `verdict` — extracts the single composite verdict cell (usually [`find_verdict`]).
/// - `expected_verdict` — the ABSOLUTE invariant (catches late==early-but-both-wrong).
/// - `vtable_expectation` — `Some(n)` asserts EXACTLY `n` vtable calls (counted by
///   hand from the script's eligible members; a silent fallback changes the
///   number and fails). `None` makes no per-member vtable claim (forced-IDispatch /
///   loader-dependent cases).
///
/// An absent component / typelib in any leg downgrades the whole case to a SKIP.
/// Reaching the final line is itself the no-host-AV guard.
pub fn assert_differential(
    case: &str,
    late: LateRun,
    early: EarlyRun,
    early_dispatch_only: Option<EarlyRun>,
    verdict: impl Fn(&[Variant]) -> Option<i64>,
    expected_verdict: i64,
    vtable_expectation: Option<u64>,
) {
    // Env-skip: if either lane reports the component / typelib absent, skip.
    let late = match late {
        Ok(s) => s,
        Err(e) if is_component_absent(&e) => {
            eprintln!("SKIP {case}: {e}");
            return;
        }
        Err(e) => panic!("{case} late failed: {e}"),
    };
    let (early_snap, (vt, idisp)) = match early {
        Ok(x) => x,
        Err(e) if is_typelib_absent(&e) => {
            eprintln!("SKIP {case}: {e}");
            return;
        }
        Err(e) => panic!("{case} early(PreferVtable) failed: {e}"),
    };

    // (1) DIFFERENTIAL late == early: the master bug oracle.
    assert_eq!(
        verdict(&late),
        verdict(&early_snap),
        "{case}: late vs early-PreferVtable verdict divergence (BUG)"
    );

    // (1b) THIRD LEG: early-PreferVtable == early-DispatchOnly isolates a TRANSPORT
    // divergence from a binder divergence (catches the silent-fallback blindness).
    if let Some(eo) = early_dispatch_only {
        match eo {
            Ok((do_snap, _)) => assert_eq!(
                verdict(&early_snap),
                verdict(&do_snap),
                "{case}: PreferVtable vs DispatchOnly verdict divergence (vtable transport BUG)"
            ),
            Err(e) if is_typelib_absent(&e) => {}
            Err(e) => panic!("{case} early(DispatchOnly) failed: {e}"),
        }
    }

    // (2) ABSOLUTE INVARIANT: the value is actually correct.
    assert_eq!(
        verdict(&early_snap),
        Some(expected_verdict),
        "{case}: verdict {:?} != expected {expected_verdict}",
        verdict(&early_snap)
    );

    // (3) TRANSPORT ORACLE: prove the vtable path actually ran where claimed.
    if let Some(exact) = vtable_expectation {
        assert_eq!(
            vt, exact,
            "{case}: expected EXACTLY {exact} vtable calls (counted from the script's \
             eligible members), got vtable={vt} idispatch={idisp} — a fallback or an \
             unexpected extra slot call changes this number"
        );
    }
    // (3b) No-AV guard is implicit: reaching this line means neither lane crashed.
    eprintln!("{case}: OK (vtable={vt} idispatch={idisp})");
}

// ─────────────────────────────────────────────────────────────────────────────
// Errors harness (plan §1.5). The master assertion is split: the late==early
// transport agreement (always meaningful) PLUS a hard absolute oracle on the real
// VBA Err.Number, which the COM error plumbing now surfaces end to end.
// ─────────────────────────────────────────────────────────────────────────────

/// Errors: assert transport AGREEMENT on `Err.Number`/`Err.Description` AND the real
/// VBA `Err.Number`. The absolute-number assertion holds because the COM error
/// plumbing now carries the real number from the dispatch fault
/// (`ComInvokeFailure::vba_error_number` → `HalError::host_error_code` →
/// `Fault::from_hal`); a `Some(5)` here would mean a fault path failed to set a
/// host error code.
pub fn assert_err_oracle(case: &str, late: &[Variant], early: &[Variant], real_vba_err: i32) {
    let (ln, ld, _) = extract_err(late);
    let (en, ed, _) = extract_err(early);
    assert_eq!(
        ln, en,
        "{case}: Err.Number late {ln:?} != early {en:?} (transport divergence)"
    );
    assert_eq!(
        ld, ed,
        "{case}: Err.Description late {ld:?} != early {ed:?}"
    );
    // ABSOLUTE ORACLE — the real VBA number, surfaced end to end.
    assert_eq!(
        ln,
        Some(real_vba_err),
        "{case}: Err.Number {ln:?}, real VBA = {real_vba_err}. A Some(5) means a COM \
         dispatch fault path failed to set host_error_code (vba_error_number)."
    );
}

/// Errors AGREEMENT-only oracle (no RED): assert merely that an error OCCURRED
/// (`Err.Number != 0`) and that late and early AGREE on the captured number. Used
/// by error scenarios that do not pin a specific COM number, so they can be normal
/// `#[ignore]` live tests rather than RED.
pub fn assert_err_agrees(case: &str, late: &[Variant], early: &[Variant]) {
    let (ln, _, _) = extract_err(late);
    let (en, _, _) = extract_err(early);
    assert!(
        ln.is_some_and(|n| n != 0),
        "{case}: expected an error (Err.Number != 0) in late run, got {ln:?}"
    );
    assert_eq!(
        ln, en,
        "{case}: Err.Number late {ln:?} != early {en:?} (transport divergence)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared LIBIDs (verified against the existing live tests).
// ─────────────────────────────────────────────────────────────────────────────

/// Excel typelib LIBID, version 1.9.
pub const EXCEL_LIBID: &str = "{00020813-0000-0000-C000-000000000046}";
/// ACE DAO typelib LIBID, version 12.0.
pub const DAO_LIBID: &str = "{4AC9E1DA-5BAD-4AC7-86E3-24F4CDCECA28}";
/// Scripting (Dictionary / FileSystemObject) typelib LIBID, version 1.0.
pub const SCRIPTING_LIBID: &str = "{420B2830-E718-11CF-893D-00A0C9054228}";
/// OxVba.TestEventServer typelib LIBID, version 1.0.
pub const TES_LIBID: &str = "{E2A30001-0001-0001-0001-000000000001}";

/// The Excel typelib reference (`Excel.Application`, `Excel.Range`, …).
pub fn excel_ref() -> Vec<ProjectReference> {
    vec![typelib_ref_by_libid("Excel", EXCEL_LIBID, 1, 9)]
}

/// The ACE DAO typelib reference (`DAO.DBEngine`, `DAO.Recordset`, …).
pub fn dao_ref() -> Vec<ProjectReference> {
    vec![typelib_ref_by_libid("DAO", DAO_LIBID, 12, 0)]
}

/// The Scripting typelib reference (`Scripting.Dictionary`, `…FileSystemObject`).
pub fn scripting_ref() -> Vec<ProjectReference> {
    vec![typelib_ref_by_libid("Scripting", SCRIPTING_LIBID, 1, 0)]
}

/// The OxVba.TestEventServer typelib reference.
pub fn tes_ref() -> Vec<ProjectReference> {
    vec![typelib_ref_by_libid(
        "OxVba_TestEventServer",
        TES_LIBID,
        1,
        0,
    )]
}
