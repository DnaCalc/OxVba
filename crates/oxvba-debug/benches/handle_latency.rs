use std::{collections::BTreeMap, sync::Arc};

use criterion::{Criterion, criterion_group, criterion_main};
use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_debug::{DebugAttachConfig, attach_debug_session};
use oxvba_host::{DirectHostBreakpointId, Engine, HostConfig};

fn manifest() -> ProjectManifest {
    ProjectManifest {
        project_name: "DebugHandleBench".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![
            module_unit_from_source(
                "Module1",
                ModuleKind::Procedural,
                "Sub Main()\nCall Foo(4)\nEnd Sub",
            )
            .expect("module1"),
            module_unit_from_source(
                "Module2",
                ModuleKind::Procedural,
                "Sub Foo(ByVal y As Long)\nDim z As Long\nz = y + 1\nEnd Sub",
            )
            .expect("module2"),
        ],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: BTreeMap::new(),
    }
}

fn attach() -> oxvba_debug::DebugSessionHandle {
    attach_debug_session(
        Arc::new(Engine::new(HostConfig::default())),
        manifest(),
        DebugAttachConfig::default(),
    )
    .expect("attach")
    .handle
}

fn handle_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("debug_handle_latency");
    group.sample_size(10);

    group.bench_function("step_into_round_trip", |b| {
        b.iter(|| {
            let handle = attach();
            let _ = handle.start().expect("start");
            let result = handle.step_into().expect("step into");
            handle.detach().expect("detach");
            result
        });
    });

    group.bench_function("set_source_breakpoint_round_trip", |b| {
        b.iter(|| {
            let handle = attach();
            let result = handle
                .set_source_breakpoint("Module1", 2, true)
                .expect("set breakpoint");
            let id = DirectHostBreakpointId::new(result.id.clone());
            handle.clear_source_breakpoint(&id).expect("clear");
            handle.detach().expect("detach");
            result
        });
    });

    group.bench_function("evaluate_watches_round_trip", |b| {
        b.iter(|| {
            let handle = attach();
            let _ = handle.start().expect("start");
            let watch = handle.add_watch("y").expect("watch");
            let result = handle.evaluate_watches().expect("evaluate watches");
            let id = oxvba_host::DirectHostWatchId::new(watch.id);
            handle.remove_watch(&id).expect("remove watch");
            handle.detach().expect("detach");
            result
        });
    });

    group.finish();
}

criterion_group!(benches, handle_latency);
criterion_main!(benches);
