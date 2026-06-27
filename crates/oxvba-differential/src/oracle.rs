//! Oracle-conformance corpus: the bridge from the **live Excel/VBA 7.1 oracle** (the
//! `oracle/` probe `.bas` files + their captured `results/*.json`) to a runnable corpus
//! the OxVBA executors are validated against.
//!
//! Each `PROBE_*` function in a probe `.bas` is turned into a **self-contained, runnable
//! program**: the probe function, the transitive set of *private helper* procedures it
//! calls, and a generated `Sub Main` that stores the probe's return value in snapshot
//! slot 0. Running that program under an [`Executor`] and reading slot 0 yields the
//! observable to compare against the oracle's captured string.
//!
//! This is the runtime test corpus the plan calls for — usable by **vm3 today and the
//! JIT later** by swapping the [`Executor`]. The oracle JSON is the ground truth
//! (Windows Office VBA 7.1); where vm2 deviates from it, that is a documented vm2
//! non-compliance, and vm3 must be 100% oracle-compliant.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::{Canon, Executor, run_with_project};

/// The project name the oracle corpus runs under — Excel's default VBProject name, so a
/// probe that reads `Err.Source` (which defaults to the project name) matches the
/// captured oracle string `"VBAProject"` exactly.
pub const ORACLE_PROJECT: &str = "VBAProject";

/// One oracle-conformance case: a self-contained, executor-runnable program plus the
/// observable string the live Excel/VBA 7.1 oracle produced for it.
#[derive(Debug, Clone)]
pub struct OracleCase {
    /// The probe name (e.g. `PROBE_oe_resume_next`).
    pub name: String,
    /// A self-contained VBA program: the probe + its private helpers + a `Sub Main`
    /// that writes the probe's return into snapshot slot 0.
    pub source: String,
    /// The string the oracle captured for this probe.
    pub expected: String,
}

/// What an executor produced for an [`OracleCase`]'s observable (snapshot slot 0).
#[derive(Debug, Clone)]
pub enum OracleObservation {
    /// Slot 0, rendered as the VBA string the probe returned.
    Value(String),
    /// The executor does not yet implement a construct the program uses (vm3 during
    /// M2/M3) — out of scope, not a divergence.
    Unsupported(String),
    /// A bind/elaborate/run failure: a front-end gap (e.g. a statement the binder does
    /// not accept) or a genuine bug.
    Failed(String),
    /// Slot 0 was present but not a string (the wrapper stores a `String`, so this is
    /// unexpected).
    NotAString(Canon),
    /// No snapshot slot 0 at all.
    Empty,
    /// The run did not finish within the time budget — for a probe that is bounded under
    /// correct VBA semantics this is itself a divergence (the executor is spinning, e.g.
    /// a mishandled `Resume`/`Static`/`On Error GoTo -1`).
    Timeout,
}

impl OracleObservation {
    /// The captured string if the run produced one, else `None`.
    pub fn value(&self) -> Option<&str> {
        match self {
            OracleObservation::Value(s) => Some(s),
            _ => None,
        }
    }
}

/// The committed `oracle/` directory (probe `.bas` + `results/*.json`), resolved relative
/// to this crate so it works from any working directory.
pub fn oracle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

/// Load the oracle cases for one probe file + its captured results JSON (both relative to
/// [`oracle_dir`]). Only `PROBE_*` functions that have a captured oracle value become
/// cases; helper procedures are folded into the programs that call them.
pub fn load_oracle_cases(bas_file: &str, json_file: &str) -> Vec<OracleCase> {
    let dir = oracle_dir();
    let bas = std::fs::read_to_string(dir.join(bas_file))
        .unwrap_or_else(|e| panic!("read {bas_file}: {e}"));
    let json = std::fs::read_to_string(dir.join("results").join(json_file))
        .unwrap_or_else(|e| panic!("read results/{json_file}: {e}"));
    let expected: BTreeMap<String, String> =
        serde_json::from_str(&json).unwrap_or_else(|e| panic!("parse results/{json_file}: {e}"));

    let procs = split_procedures(&bas);
    let by_name: BTreeMap<&str, &Procedure> = procs.iter().map(|p| (p.name.as_str(), p)).collect();

    let mut cases = Vec::new();
    for p in &procs {
        if !p.name.starts_with("PROBE_") {
            continue;
        }
        let Some(exp) = expected.get(&p.name) else {
            continue;
        };
        let helpers = transitive_helpers(p, &by_name);
        // `gResult` is a module-level global declared first, so it is snapshot slot 0
        // regardless of any `Static` locals a probe's helpers promote to hidden globals
        // (those are allocated after explicit module globals).
        let mut source = String::from("Public gResult As String\n");
        source.push_str(&p.text);
        source.push('\n');
        for h in &helpers {
            source.push_str(&by_name[h.as_str()].text);
            source.push('\n');
        }
        source.push_str(&format!(
            "Sub Main()\n  gResult = {}()\nEnd Sub\n",
            p.name
        ));
        cases.push(OracleCase {
            name: p.name.clone(),
            source,
            expected: exp.clone(),
        });
    }
    cases
}

/// The two committed oracle corpora (error model + control flow).
pub fn all_oracle_cases() -> Vec<OracleCase> {
    let mut cases = load_oracle_cases("probes.bas", "error_handling.json");
    cases.extend(load_oracle_cases("probes_controlflow.bas", "controlflow.json"));
    cases
}

/// Run one [`OracleCase`] under `executor` and read its observable (snapshot slot 0).
///
/// The case runs on a worker thread with a time budget: every oracle probe is bounded
/// under correct VBA semantics, so exceeding the budget means the executor is spinning on
/// a real divergence ([`OracleObservation::Timeout`]) rather than doing legitimate work.
/// A timed-out worker is left to exit on its own (it cannot corrupt this thread — vm3's
/// state, including the termination queue, is thread-local).
pub fn run_oracle_case(executor: Executor, case: &OracleCase) -> OracleObservation {
    let src = case.source.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run_with_project(executor, &src, ORACLE_PROJECT));
    });
    let out = match rx.recv_timeout(std::time::Duration::from_secs(3)) {
        Ok(o) => o,
        Err(_) => return OracleObservation::Timeout,
    };
    if let Some(u) = out.unsupported {
        return OracleObservation::Unsupported(u);
    }
    match out.result {
        Err(e) => OracleObservation::Failed(e),
        Ok(vals) => match vals.into_iter().next() {
            None => OracleObservation::Empty,
            Some(Canon::Str(s)) => OracleObservation::Value(s),
            Some(other) => OracleObservation::NotAString(other),
        },
    }
}

// ── procedure splitting ─────────────────────────────────────────────────────────────

struct Procedure {
    name: String,
    /// The full source text of the procedure (header line .. `End Function`/`End Sub`).
    text: String,
}

/// Split a probe `.bas` into its `Function`/`Sub` procedures (dropping the
/// `Attribute VB_Name` header and any leading comments). Robust to the leading
/// `Public`/`Private`/`Friend`/`Static` modifiers and to bodies that contain
/// `Exit Function`/`End Sub`/`Static <local>` lines (which are not declarations).
fn split_procedures(bas: &str) -> Vec<Procedure> {
    let mut out = Vec::new();
    let lines: Vec<&str> = bas.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if let Some(name) = proc_decl_name(lines[i]) {
            let start = i;
            let mut j = i + 1;
            while j < lines.len() {
                // Strip any trailing comment (`End Sub  ' note`) before matching — these
                // lines have no string literals, so splitting on the first `'` is safe.
                let code = lines[j].split('\'').next().unwrap_or("").trim();
                if code.eq_ignore_ascii_case("End Function")
                    || code.eq_ignore_ascii_case("End Sub")
                {
                    break;
                }
                j += 1;
            }
            let text = lines[start..=j.min(lines.len() - 1)].join("\n");
            out.push(Procedure { name, text });
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// If `line` is a procedure declaration, return its name. A declaration is the line that,
/// after stripping leading `Public`/`Private`/`Friend`/`Static` modifiers, begins with
/// `Function ` or `Sub ` followed by an identifier. `Exit Function`/`End Sub`/a local
/// `Static x As ...` never match (they don't start with `Function`/`Sub` after stripping).
fn proc_decl_name(line: &str) -> Option<String> {
    let mut t = line.trim_start();
    loop {
        let stripped = ["Public ", "Private ", "Friend ", "Static "]
            .iter()
            .find_map(|m| t.strip_prefix(m));
        match stripped {
            Some(rest) => t = rest.trim_start(),
            None => break,
        }
    }
    let rest = t
        .strip_prefix("Function ")
        .or_else(|| t.strip_prefix("Sub "))?;
    let name: String = rest
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// The transitive set of non-`PROBE_` procedures (private helpers) that `root` calls,
/// directly or indirectly. Returns names in a stable (sorted) order.
fn transitive_helpers(root: &Procedure, by_name: &BTreeMap<&str, &Procedure>) -> Vec<String> {
    let mut included: HashSet<String> = HashSet::new();
    let mut worklist = vec![root.name.clone()];
    while let Some(cur) = worklist.pop() {
        let Some(proc) = by_name.get(cur.as_str()) else {
            continue;
        };
        for tok in body_tokens(&proc.text) {
            if tok == root.name || tok.starts_with("PROBE_") || included.contains(&tok) {
                continue;
            }
            if by_name.contains_key(tok.as_str()) {
                included.insert(tok.clone());
                worklist.push(tok);
            }
        }
    }
    let mut v: Vec<String> = included.into_iter().collect();
    v.sort();
    v
}

/// The set of identifier-shaped tokens in a procedure body (used to detect which helper
/// procedures it references).
fn body_tokens(body: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let mut cur = String::new();
    for ch in body.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            cur.push(ch);
        } else if !cur.is_empty() {
            set.insert(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        set.insert(cur);
    }
    set
}
