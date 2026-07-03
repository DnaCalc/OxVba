use std::sync::Arc;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use oxvba_bundle::ProcedureKind;
use oxvba_host::{Engine, HostConfig, RuntimeProfileId};
use oxvba_oxir::{OxImage, OxProgram};
use oxvba_runtime::live_handle_counts;
use oxvba_symbol::CatalogTypeLibResolver;
use oxvba_symbol::manifest as sym;

struct Fixture {
    name: &'static str,
    source: &'static str,
    references: Vec<sym::ProjectReference>,
}

fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture::new("scalar_loop", include_str!("fixtures/scalar_loop.bas")),
        Fixture::new("string_concat", include_str!("fixtures/string_concat.bas")),
        Fixture::new("array_loop", include_str!("fixtures/array_loop.bas")),
        Fixture::new("udt_fields", include_str!("fixtures/udt_fields.bas")),
        Fixture::new("call_overhead", include_str!("fixtures/call_overhead.bas")),
        Fixture::new("error_loop", include_str!("fixtures/error_loop.bas")),
        Fixture::new(
            "collection_ops",
            include_str!("fixtures/collection_ops.bas"),
        ),
        Fixture::new(
            "com_late_vs_early",
            include_str!("fixtures/com_late_vs_early.bas"),
        )
        .with_references(vec![test_dispatch_typelib_ref()]),
    ]
}

impl Fixture {
    fn new(name: &'static str, source: &'static str) -> Self {
        Self {
            name,
            source,
            references: Vec::new(),
        }
    }

    fn with_references(mut self, references: Vec<sym::ProjectReference>) -> Self {
        self.references = references;
        self
    }
}

fn test_dispatch_typelib_ref() -> sym::ProjectReference {
    let identity = oxvba_com::known_typelib_identity_for_prog_id_name("OxVba.TestDispatch")
        .expect("fixture typelib identity for OxVba.TestDispatch");
    sym::ProjectReference::TypeLibrary {
        name: identity.reference_name,
        guid: identity.libid,
        version_major: Some(identity.major_version),
        version_minor: Some(identity.minor_version),
        lcid: identity.lcid,
        import_lib: Some(identity.importlib),
    }
}

fn manifest_for_fixture(fixture: &Fixture) -> sym::SymbolProjectManifest {
    sym::SymbolProjectManifest {
        project_name: format!("Bench_{}", fixture.name),
        project_kind: sym::ProjectKind::Source,
        modules: vec![sym::ModuleUnit {
            module_name: "Main".to_string(),
            module_kind: sym::ModuleKind::Procedural,
            attributes: sym::ModuleAttributes::named("Main"),
            source: fixture.source.to_string(),
        }],
        references: fixture.references.clone(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn compile_fixture(fixture: &Fixture) -> OxProgram {
    let manifest = manifest_for_fixture(fixture);
    let program = oxvba_bind::bind_program(&manifest, &CatalogTypeLibResolver)
        .unwrap_or_else(|err| panic!("{}: bind failed: {err}", fixture.name));
    oxvba_oxir::elaborate::elaborate(&program)
        .unwrap_or_else(|err| panic!("{}: elaborate failed: {err}", fixture.name))
}

fn oracle_host_services() -> Arc<dyn oxvba_hal::traits::HostServices> {
    Engine::new(HostConfig::vm3())
        .with_runtime_profile(RuntimeProfileId::WindowsHeadless)
        .host_services()
}

fn bench_vm3_execution(c: &mut Criterion) {
    let prepared: Vec<(&str, OxProgram)> = fixtures()
        .iter()
        .map(|fixture| (fixture.name, compile_fixture(fixture)))
        .collect();
    let host_services = oracle_host_services();
    let mut group = c.benchmark_group("vm3_execution_precompiled_oxir");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    for (name, program) in &prepared {
        group.bench_with_input(BenchmarkId::from_parameter(name), program, |b, program| {
            b.iter(|| {
                let before = live_handle_counts();
                {
                    let vm = oxvba_vm3::Vm3::run(black_box(program), &*host_services)
                        .unwrap_or_else(|err| panic!("{name}: vm3 failed: {err}"));
                    let local_count = program
                        .entry
                        .and_then(|entry| program.funcs.get(entry.0))
                        .filter(|func| matches!(func.kind, ProcedureKind::Sub))
                        .map(|func| func.locals.len())
                        .unwrap_or(0);
                    let snapshot = (0..program.globals.len() + local_count)
                        .map(|slot| vm.slot(slot))
                        .collect::<Vec<_>>();
                    black_box(snapshot);
                }
                let balance = before.balance_to(live_handle_counts());
                assert!(balance.is_zero(), "{name}: handle imbalance {balance:?}");
            });
        });
    }
    group.finish();
}

fn bench_source_compile(c: &mut Criterion) {
    let fixtures = fixtures();
    let mut group = c.benchmark_group("source_to_oxir_compile");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    for fixture in &fixtures {
        group.bench_with_input(
            BenchmarkId::from_parameter(fixture.name),
            fixture,
            |b, fixture| {
                b.iter(|| black_box(compile_fixture(black_box(fixture))));
            },
        );
    }
    group.finish();
}

fn image_load_fixture() -> Fixture {
    Fixture::new("image_load", include_str!("fixtures/image_load.bas"))
}

fn bench_image_load(c: &mut Criterion) {
    let fixture = image_load_fixture();
    let program = compile_fixture(&fixture);
    let bytes = OxImage::new(vec![program])
        .to_bytes()
        .expect("serialize image");
    let mut group = c.benchmark_group("image_load_json_parse");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("from_bytes_validate", |b| {
        b.iter(|| {
            let image = OxImage::from_bytes(black_box(&bytes)).expect("parse image");
            black_box(image);
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_vm3_execution,
    bench_source_compile,
    bench_image_load
);
criterion_main!(benches);
