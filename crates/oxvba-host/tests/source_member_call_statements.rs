//! Statement-form member calls on source-class objects: `obj.Method arg` and
//! no-arg `obj.Method` (without the `Call` keyword). Before the fix these failed
//! to compile ("call to unknown procedure: <flattened name>"); they must now
//! compile and dispatch through the same path as the `Call obj.Method(args)`
//! form. (Public-field instance-state read like `c.Total` is a separate
//! pre-existing object-semantics limitation tracked elsewhere; this test asserts
//! the statement-form calls compile and execute, not field-state accumulation.)

use oxvba_compiler::{ModuleKind, ProjectKind, ProjectManifest, module_unit_from_source};
use oxvba_host::{Engine, HostConfig};

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
    engine
        .execute_project_with_variant_snapshot_phased(&manifest)
        .expect("bare statement-form member calls should compile and dispatch");
}
