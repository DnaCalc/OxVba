use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use oxvba_bundle::NumericMode;
use oxvba_oxir::{OxInst, OxProgram};
use oxvba_symbol::CatalogTypeLibResolver;
use oxvba_symbol::manifest as sym;

#[derive(Default, Clone)]
struct Counts {
    programs: usize,
    checked_arith: usize,
    widening_arith: usize,
    compare: usize,
    failed: usize,
    checked_targets: BTreeMap<String, usize>,
}

impl Counts {
    fn add_program(&mut self, program: &OxProgram) {
        self.programs += 1;
        for func in &program.funcs {
            for block in &func.blocks {
                for inst in &block.instrs {
                    match inst {
                        OxInst::Arith { mode, .. } | OxInst::Neg { mode, .. } => {
                            self.add_numeric_mode(*mode);
                        }
                        OxInst::Compare { .. } | OxInst::CompareObjectIs { .. } => {
                            self.compare += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn add_numeric_mode(&mut self, mode: NumericMode) {
        match mode {
            NumericMode::Widening => self.widening_arith += 1,
            NumericMode::Checked(target) => {
                self.checked_arith += 1;
                *self
                    .checked_targets
                    .entry(format!("{target:?}"))
                    .or_insert(0) += 1;
            }
        }
    }

    fn add_counts(&mut self, other: &Counts) {
        self.programs += other.programs;
        self.checked_arith += other.checked_arith;
        self.widening_arith += other.widening_arith;
        self.compare += other.compare;
        self.failed += other.failed;
        for (target, count) in &other.checked_targets {
            *self.checked_targets.entry(target.clone()).or_insert(0) += count;
        }
    }
}

struct Row {
    lane: String,
    path: String,
    status: String,
    counts: Counts,
}

fn manifest_for_source(path: &Path, source: String) -> sym::SymbolProjectManifest {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Main")
        .replace('-', "_");
    sym::SymbolProjectManifest {
        project_name: "VBAProject".to_string(),
        project_kind: sym::ProjectKind::Source,
        modules: vec![sym::ModuleUnit {
            module_name: stem,
            module_kind: sym::ModuleKind::Procedural,
            attributes: sym::ModuleAttributes::named("Main"),
            source,
        }],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn count_manifest(manifest: &sym::SymbolProjectManifest) -> Result<Counts, String> {
    let program = oxvba_bind::bind_program(manifest, &CatalogTypeLibResolver)
        .map_err(|err| format!("bind: {err}"))?;
    let oxp =
        oxvba_oxir::elaborate::elaborate(&program).map_err(|err| format!("elaborate: {err}"))?;
    let mut counts = Counts::default();
    counts.add_program(&oxp);
    Ok(counts)
}

fn count_bas(path: &Path) -> Row {
    let source = match fs::read_to_string(path) {
        Ok(source) if !source.trim().is_empty() => source,
        Ok(_) => {
            return Row {
                lane: "bas".to_string(),
                path: display_path(path),
                status: "empty".to_string(),
                counts: Counts::default(),
            };
        }
        Err(err) => {
            let counts = Counts {
                failed: 1,
                ..Counts::default()
            };
            return Row {
                lane: "bas".to_string(),
                path: display_path(path),
                status: format!("read: {err}"),
                counts,
            };
        }
    };
    match count_manifest(&manifest_for_source(path, source)) {
        Ok(counts) => Row {
            lane: "bas".to_string(),
            path: display_path(path),
            status: "ok".to_string(),
            counts,
        },
        Err(err) => Row {
            lane: "bas".to_string(),
            path: display_path(path),
            status: err,
            counts: Counts {
                failed: 1,
                ..Counts::default()
            },
        },
    }
}

fn count_project(path: &Path) -> Row {
    let mut counts = Counts::default();
    let status = match oxvba_project::load_project_closure(path) {
        Ok(closure) => match oxvba_bind::bind_projects(&closure, &CatalogTypeLibResolver) {
            Ok(programs) => {
                let mut status = "ok".to_string();
                for program in &programs {
                    match oxvba_oxir::elaborate::elaborate(program) {
                        Ok(oxp) => counts.add_program(&oxp),
                        Err(err) => {
                            counts.failed += 1;
                            status = format!("elaborate: {err}");
                            break;
                        }
                    }
                }
                status
            }
            Err(err) => {
                counts.failed = 1;
                format!("bind: {err}")
            }
        },
        Err(err) => {
            counts.failed = 1;
            format!("load: {err}")
        }
    };
    Row {
        lane: "project".to_string(),
        path: display_path(path),
        status,
        counts,
    }
}

fn collect_bas_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_bas_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "bas") {
            out.push(path);
        }
    }
}

fn display_path(path: &Path) -> String {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    path.strip_prefix(&cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn render_csv(rows: &[Row]) -> String {
    let mut out =
        "lane,path,status,programs,checked_arith,widening_arith,compare,failed,checked_targets\n"
            .to_string();
    for row in rows {
        let checked_targets = row
            .counts
            .checked_targets
            .iter()
            .map(|(target, count)| format!("{target}:{count}"))
            .collect::<Vec<_>>()
            .join("|");
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&row.lane),
            csv_escape(&row.path),
            csv_escape(&row.status),
            row.counts.programs,
            row.counts.checked_arith,
            row.counts.widening_arith,
            row.counts.compare,
            row.counts.failed,
            csv_escape(&checked_targets)
        ));
    }
    out
}

fn render_markdown(rows: &[Row], total: &Counts) -> String {
    let arith_total = total.checked_arith + total.widening_arith;
    let checked_pct = if arith_total == 0 {
        0.0
    } else {
        (total.checked_arith as f64 * 100.0) / arith_total as f64
    };
    let widening_pct = if arith_total == 0 {
        0.0
    } else {
        (total.widening_arith as f64 * 100.0) / arith_total as f64
    };
    let mut out = String::new();
    out.push_str("# JIT M4-0 NumericMode Census\n\n");
    out.push_str(
        "- Scope: `conformance/**/*.bas`, `examples/**/*.bas`, and explicit project arguments.\n",
    );
    out.push_str("- Compare instructions currently carry `StringCompareMode`, not `NumericMode`; this census records compare totals separately.\n\n");
    out.push_str("| metric | value |\n|---|---:|\n");
    out.push_str(&format!("| programs elaborated | {} |\n", total.programs));
    out.push_str(&format!("| failed inputs | {} |\n", total.failed));
    out.push_str(&format!(
        "| checked arithmetic ops | {} |\n",
        total.checked_arith
    ));
    out.push_str(&format!(
        "| widening arithmetic ops | {} |\n",
        total.widening_arith
    ));
    out.push_str(&format!(
        "| checked arithmetic share | {:.2}% |\n",
        checked_pct
    ));
    out.push_str(&format!(
        "| widening arithmetic share | {:.2}% |\n",
        widening_pct
    ));
    out.push_str(&format!("| compare ops | {} |\n\n", total.compare));
    out.push_str("## Checked Targets\n\n| target | count |\n|---|---:|\n");
    for (target, count) in &total.checked_targets {
        out.push_str(&format!("| `{target}` | {count} |\n"));
    }
    out.push_str("\n## Inputs\n\n| lane | path | status | checked | widening | compare |\n|---|---|---|---:|---:|---:|\n");
    for row in rows {
        out.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} |\n",
            row.lane,
            row.path,
            row.status.replace('|', "\\|"),
            row.counts.checked_arith,
            row.counts.widening_arith,
            row.counts.compare
        ));
    }
    out
}

fn main() {
    let mut csv_out = None::<PathBuf>;
    let mut md_out = None::<PathBuf>;
    let mut projects = Vec::<PathBuf>::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--csv-out" => csv_out = args.next().map(PathBuf::from),
            "--md-out" => md_out = args.next().map(PathBuf::from),
            "--project" => {
                if let Some(path) = args.next() {
                    projects.push(PathBuf::from(path));
                }
            }
            other => projects.push(PathBuf::from(other)),
        }
    }

    let mut bas_files = Vec::new();
    collect_bas_files(Path::new("conformance"), &mut bas_files);
    collect_bas_files(Path::new("examples"), &mut bas_files);
    bas_files.sort();

    let mut rows = Vec::new();
    rows.extend(bas_files.iter().map(|path| count_bas(path)));
    rows.extend(projects.iter().map(|path| count_project(path)));

    let mut total = Counts::default();
    for row in &rows {
        total.add_counts(&row.counts);
    }

    let csv = render_csv(&rows);
    print!("{csv}");
    if let Some(path) = csv_out {
        fs::write(path, &csv).expect("write csv output");
    }
    if let Some(path) = md_out {
        fs::write(path, render_markdown(&rows, &total)).expect("write markdown output");
    }
}
