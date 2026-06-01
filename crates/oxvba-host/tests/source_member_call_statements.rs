//! Statement-form member calls on source-class objects: `obj.Method arg` and
//! no-arg `obj.Method` (without the `Call` keyword). Before the fix these failed
//! to compile ("call to unknown procedure: <flattened name>"); they must now
//! compile and dispatch through the same path as the `Call obj.Method(args)`
//! form. The same fixture also reads a public field afterward so the dispatch
//! path proves it preserved per-instance public-field state.

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_host::{Engine, HostConfig};
use oxvba_runtime::Variant;

#[test]
fn statement_form_member_calls_compile_and_dispatch() {
    let counter = module_unit_from_source(
        "Counter",
        ModuleKind::Class,
        "Public Total As Long\n\
         Public Sub Add(ByVal n As Long)\n\
         Total = Total + n\n\
         End Sub\n\
         Public Sub Reset()\n\
         Total = 0\n\
         End Sub\n",
    )
    .expect("counter class");

    let main = module_unit_from_source(
        "Main",
        ModuleKind::Procedural,
        "Sub Main()\n\
         Dim c As New Counter\n\
         c.Add 5\n\
         c.Reset\n\
         c.Add 7\n\
         Dim result\n\
         result = c.Total\n\
         End Sub\n",
    )
    .expect("main module");

    let manifest = ProjectManifest {
        project_name: "MemberCallStmt".to_string(),
        project_kind: ProjectKind::Source,
        modules: vec![main, counter],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
    };

    let engine = Engine::new(HostConfig { enable_jit: false });
    // Before the fix this returned a CompileTime error ("unknown procedure
    // c_add" / "unsupported statement"). It must now compile and execute.
    let out = engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("bare statement-form member calls should compile and dispatch");
    assert!(
        out.contains(&Variant::from_i32(7)),
        "public-field read should observe per-instance state after bare statement-form calls; out={out:?}"
    );
}
