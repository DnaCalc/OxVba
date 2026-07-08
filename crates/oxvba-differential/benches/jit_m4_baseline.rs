use std::sync::Arc;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use oxvba_bundle::ProcedureKind;
use oxvba_host::{Engine, HostConfig, RuntimeProfileId};
use oxvba_oxir::{OxImage, OxProgram};
use oxvba_runtime::live_handle_counts;
use oxvba_symbol::CatalogTypeLibResolver;
use oxvba_symbol::manifest as sym;
use oxvba_symbol::surface::{ProjectExportSurface, synthesize_export_surface_from_core_program};

struct Fixture {
    name: &'static str,
    modules: Vec<FixtureModule>,
    references: Vec<sym::ProjectReference>,
    reference_projects: Vec<FixtureProject>,
    bundle_reference_projects: Vec<FixtureProject>,
}

struct FixtureModule {
    name: &'static str,
    kind: sym::ModuleKind,
    source: &'static str,
    exposed_creatable: bool,
}

struct FixtureProject {
    name: &'static str,
    modules: Vec<FixtureModule>,
}

fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture::new("scalar_loop", include_str!("fixtures/scalar_loop.bas")),
        Fixture::new("string_concat", include_str!("fixtures/string_concat.bas")),
        Fixture::new("array_loop", include_str!("fixtures/array_loop.bas")),
        Fixture::new("array_redim", include_str!("fixtures/array_redim.bas")),
        Fixture::new(
            "array_set_long",
            include_str!("fixtures/array_set_long.bas"),
        ),
        Fixture::new(
            "array_get_long",
            include_str!("fixtures/array_get_long.bas"),
        ),
        Fixture::new(
            "array_for_each_variant",
            include_str!("fixtures/array_for_each_variant.bas"),
        ),
        Fixture::new("udt_fields", include_str!("fixtures/udt_fields.bas")),
        Fixture::new(
            "udt_nested_arrays",
            include_str!("fixtures/udt_nested_arrays.bas"),
        ),
        Fixture::with_modules(
            "class_field_aggregates",
            vec![
                FixtureModule::procedural(
                    "Main",
                    include_str!("fixtures/class_field_aggregates.bas"),
                ),
                FixtureModule::class("Box", include_str!("fixtures/class_field_aggregates.cls")),
            ],
        ),
        Fixture::with_project_refs(
            "referenced_class_aggregates",
            vec![FixtureModule::procedural(
                "Main",
                include_str!("fixtures/referenced_class_aggregates.bas"),
            )],
            vec![FixtureProject {
                name: "Lib",
                modules: vec![FixtureModule::exposed_class(
                    "Box",
                    include_str!("fixtures/referenced_class_aggregates_box.cls"),
                )],
            }],
        ),
        Fixture::with_bundle_project_refs(
            "bundle_only_referenced_class_aggregates",
            vec![FixtureModule::procedural(
                "Main",
                include_str!("fixtures/referenced_class_aggregates.bas"),
            )],
            vec![FixtureProject {
                name: "Lib",
                modules: vec![FixtureModule::exposed_class(
                    "Box",
                    include_str!("fixtures/referenced_class_aggregates_box.cls"),
                )],
            }],
        ),
        Fixture::new("call_overhead", include_str!("fixtures/call_overhead.bas")),
        Fixture::new("error_loop", include_str!("fixtures/error_loop.bas")),
        Fixture::new(
            "variant_box_unbox",
            include_str!("fixtures/variant_box_unbox.bas"),
        ),
        Fixture::with_modules(
            "project_object_calls",
            vec![
                FixtureModule::procedural(
                    "Main",
                    include_str!("fixtures/project_object_calls.bas"),
                ),
                FixtureModule::class("Counter", include_str!("fixtures/project_object_calls.cls")),
            ],
        ),
        Fixture::with_modules(
            "dynamic_dispatch_helpers",
            vec![
                FixtureModule::procedural(
                    "Main",
                    include_str!("fixtures/dynamic_dispatch_helpers.bas"),
                ),
                FixtureModule::class(
                    "Counter",
                    include_str!("fixtures/dynamic_dispatch_helpers.cls"),
                ),
            ],
        ),
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
        Self::with_modules(name, vec![FixtureModule::procedural("Main", source)])
    }

    fn with_modules(name: &'static str, modules: Vec<FixtureModule>) -> Self {
        Self {
            name,
            modules,
            references: Vec::new(),
            reference_projects: Vec::new(),
            bundle_reference_projects: Vec::new(),
        }
    }

    fn with_references(mut self, references: Vec<sym::ProjectReference>) -> Self {
        self.references = references;
        self
    }

    fn with_project_refs(
        name: &'static str,
        modules: Vec<FixtureModule>,
        reference_projects: Vec<FixtureProject>,
    ) -> Self {
        Self {
            name,
            modules,
            references: Vec::new(),
            reference_projects,
            bundle_reference_projects: Vec::new(),
        }
    }

    fn with_bundle_project_refs(
        name: &'static str,
        modules: Vec<FixtureModule>,
        bundle_reference_projects: Vec<FixtureProject>,
    ) -> Self {
        Self {
            name,
            modules,
            references: Vec::new(),
            reference_projects: Vec::new(),
            bundle_reference_projects,
        }
    }
}

impl FixtureModule {
    fn procedural(name: &'static str, source: &'static str) -> Self {
        Self {
            name,
            kind: sym::ModuleKind::Procedural,
            source,
            exposed_creatable: false,
        }
    }

    fn class(name: &'static str, source: &'static str) -> Self {
        Self {
            name,
            kind: sym::ModuleKind::Class,
            source,
            exposed_creatable: false,
        }
    }

    fn exposed_class(name: &'static str, source: &'static str) -> Self {
        Self {
            name,
            kind: sym::ModuleKind::Class,
            source,
            exposed_creatable: true,
        }
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

fn manifest_for_modules(
    project_name: String,
    project_kind: sym::ProjectKind,
    modules: &[FixtureModule],
    references: Vec<sym::ProjectReference>,
    reference_projects: Vec<sym::ReferencedProjectManifest>,
) -> sym::SymbolProjectManifest {
    sym::SymbolProjectManifest {
        project_name,
        project_kind,
        modules: modules
            .iter()
            .map(|module| {
                let mut attributes = sym::ModuleAttributes::named(module.name);
                if module.exposed_creatable {
                    attributes.vb_exposed = true;
                    attributes.vb_creatable = true;
                }
                sym::ModuleUnit {
                    module_name: module.name.to_string(),
                    module_kind: module.kind,
                    attributes,
                    source: module.source.to_string(),
                }
            })
            .collect(),
        references,
        reference_projects,
        conditional_constants: std::collections::BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn referenced_manifest(project: &FixtureProject) -> sym::ReferencedProjectManifest {
    sym::ReferencedProjectManifest {
        project_name: project.name.to_string(),
        project_kind: sym::ProjectKind::Library,
        modules: manifest_for_modules(
            project.name.to_string(),
            sym::ProjectKind::Library,
            &project.modules,
            Vec::new(),
            Vec::new(),
        )
        .modules,
    }
}

fn manifest_for_fixture(fixture: &Fixture) -> sym::SymbolProjectManifest {
    let reference_projects: Vec<_> = fixture
        .reference_projects
        .iter()
        .map(referenced_manifest)
        .collect();
    let mut references = fixture.references.clone();
    references.extend(
        reference_projects
            .iter()
            .map(|project| sym::ProjectReference::Project {
                referenced_project_name: project.project_name.clone(),
            }),
    );
    references.extend(fixture.bundle_reference_projects.iter().map(|project| {
        sym::ProjectReference::Project {
            referenced_project_name: project.name.to_string(),
        }
    }));
    manifest_for_modules(
        format!("Bench_{}", fixture.name),
        sym::ProjectKind::Source,
        &fixture.modules,
        references,
        reference_projects,
    )
}

fn library_manifest(reference: &sym::ReferencedProjectManifest) -> sym::SymbolProjectManifest {
    sym::SymbolProjectManifest {
        project_name: reference.project_name.clone(),
        project_kind: sym::ProjectKind::Library,
        modules: reference.modules.clone(),
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

fn fixture_project_manifest(project: &FixtureProject) -> sym::SymbolProjectManifest {
    manifest_for_modules(
        project.name.to_string(),
        sym::ProjectKind::Library,
        &project.modules,
        Vec::new(),
        Vec::new(),
    )
}

fn compile_bundle_reference_projects(
    fixture: &Fixture,
) -> (Vec<ProjectExportSurface>, Vec<OxProgram>) {
    let mut surfaces = Vec::new();
    let mut programs = Vec::new();
    for project in &fixture.bundle_reference_projects {
        let manifest = fixture_project_manifest(project);
        let core =
            oxvba_bind::bind_program(&manifest, &CatalogTypeLibResolver).unwrap_or_else(|err| {
                panic!(
                    "{}:{}: reference bind failed: {err}",
                    fixture.name, project.name
                )
            });
        surfaces.push(synthesize_export_surface_from_core_program(&core));
        programs.push(
            oxvba_oxir::elaborate::elaborate(&core).unwrap_or_else(|err| {
                panic!(
                    "{}:{}: reference elaborate failed: {err}",
                    fixture.name, project.name
                )
            }),
        );
    }

    let bytes = OxImage::new(programs)
        .to_bytes()
        .unwrap_or_else(|err| panic!("{}: reference image serialize failed: {err}", fixture.name));
    let image = OxImage::from_bytes(&bytes)
        .unwrap_or_else(|err| panic!("{}: reference image load failed: {err}", fixture.name));
    (surfaces, image.programs)
}

fn compile_fixture(fixture: &Fixture) -> Vec<OxProgram> {
    let entry_manifest = manifest_for_fixture(fixture);
    if !fixture.bundle_reference_projects.is_empty() {
        let (surfaces, mut programs) = compile_bundle_reference_projects(fixture);
        for reference in &entry_manifest.reference_projects {
            let manifest = library_manifest(reference);
            let core = oxvba_bind::bind_program(&manifest, &CatalogTypeLibResolver).unwrap_or_else(
                |err| panic!("{}: source reference bind failed: {err}", fixture.name),
            );
            programs.push(
                oxvba_oxir::elaborate::elaborate(&core).unwrap_or_else(|err| {
                    panic!("{}: source reference elaborate failed: {err}", fixture.name)
                }),
            );
        }
        let core = oxvba_bind::bind_program_with_project_surfaces(
            &entry_manifest,
            &CatalogTypeLibResolver,
            &surfaces,
        )
        .unwrap_or_else(|err| {
            panic!(
                "{}: bind against compiled reference surface failed: {err}",
                fixture.name
            )
        });
        programs.push(
            oxvba_oxir::elaborate::elaborate(&core)
                .unwrap_or_else(|err| panic!("{}: elaborate failed: {err}", fixture.name)),
        );
        return programs;
    }

    let mut closure: Vec<_> = entry_manifest
        .reference_projects
        .iter()
        .map(library_manifest)
        .collect();
    closure.push(entry_manifest);
    let programs = oxvba_bind::bind_projects(&closure, &CatalogTypeLibResolver)
        .unwrap_or_else(|err| panic!("{}: bind failed: {err}", fixture.name));
    programs
        .iter()
        .map(|program| {
            oxvba_oxir::elaborate::elaborate(program)
                .unwrap_or_else(|err| panic!("{}: elaborate failed: {err}", fixture.name))
        })
        .collect()
}

fn oracle_host_services() -> Arc<dyn oxvba_hal::traits::HostServices> {
    Engine::new(HostConfig::vm3())
        .with_runtime_profile(RuntimeProfileId::WindowsHeadless)
        .host_services()
}

fn bench_vm3_execution(c: &mut Criterion) {
    let prepared: Vec<(&str, Vec<OxProgram>)> = fixtures()
        .iter()
        .map(|fixture| (fixture.name, compile_fixture(fixture)))
        .collect();
    let host_services = oracle_host_services();
    let mut group = c.benchmark_group("vm3_execution_precompiled_oxir");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    for (name, programs) in &prepared {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            programs,
            |b, programs| {
                b.iter(|| {
                    let before = live_handle_counts();
                    {
                        let refs: Vec<&OxProgram> = black_box(programs).iter().collect();
                        let mut vm = oxvba_vm3::Vm3::link(&refs, &*host_services)
                            .unwrap_or_else(|err| panic!("{name}: vm3 failed: {err}"));
                        vm.run_entry()
                            .unwrap_or_else(|err| panic!("{name}: vm3 failed: {err}"));
                        let program = programs.last().expect("fixture has entry program");
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
            },
        );
    }
    group.finish();
}

fn bench_jit_execution(c: &mut Criterion) {
    let prepared: Vec<(&str, Vec<OxProgram>)> = fixtures()
        .iter()
        .map(|fixture| (fixture.name, compile_fixture(fixture)))
        .collect();
    let host_services = oracle_host_services();
    let jit = oxvba_jit::JitEngine;
    let mut group = c.benchmark_group("jit_execution_precompiled_oxir");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    for (name, programs) in &prepared {
        let refs: Vec<&OxProgram> = programs.iter().collect();
        let compiled = match jit.compile_image(&refs) {
            Ok(compiled) => compiled,
            Err(err) if err.unsupported_message().is_some() => {
                eprintln!(
                    "jit_execution_precompiled_oxir/{name}: unsupported: {}",
                    err.unsupported_message().unwrap_or_default()
                );
                continue;
            }
            Err(err) => panic!("{name}: jit compile failed: {err}"),
        };
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &compiled,
            |b, compiled| {
                b.iter(|| {
                    let before = live_handle_counts();
                    {
                        let outcome = compiled
                            .run(&*host_services)
                            .unwrap_or_else(|err| panic!("{name}: jit run failed: {err}"));
                        assert!(!outcome.raised, "{name}: jit raised {:?}", outcome.err);
                        black_box(outcome.values);
                    }
                    let balance = before.balance_to(live_handle_counts());
                    assert!(balance.is_zero(), "{name}: handle imbalance {balance:?}");
                });
            },
        );
    }
    group.finish();
}

fn bench_jit_compile_precompiled(c: &mut Criterion) {
    let prepared: Vec<(&str, Vec<OxProgram>)> = fixtures()
        .iter()
        .map(|fixture| (fixture.name, compile_fixture(fixture)))
        .collect();
    let jit = oxvba_jit::JitEngine;
    let mut group = c.benchmark_group("jit_compile_precompiled_oxir");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    for (name, programs) in &prepared {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            programs,
            |b, programs| {
                b.iter(|| {
                    let refs: Vec<&OxProgram> = black_box(programs).iter().collect();
                    let supported = match jit.compile_image(&refs) {
                        Ok(compiled) => {
                            black_box(&compiled);
                            true
                        }
                        Err(err) if err.unsupported_message().is_some() => false,
                        Err(err) => panic!("{name}: jit compile failed: {err}"),
                    };
                    black_box(supported);
                });
            },
        );
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
    let programs = compile_fixture(&fixture);
    let bytes = OxImage::new(programs).to_bytes().expect("serialize image");
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
    bench_jit_execution,
    bench_jit_compile_precompiled,
    bench_source_compile,
    bench_image_load
);
criterion_main!(benches);
