//! End-to-end cross-VBA-project references: real source for *several* projects →
//! `oxvba_bind::bind_projects` (one program per project) → `oxvba_oxir::elaborate`
//! → `oxvba_vm3::Vm3::link` (the ".NET assembly" model) → run. Exercises every
//! cross-bundle binding form: a hidden-module function call, `New` + method on a
//! referenced coclass, a referenced `Const`/`Enum` value, cross-bundle `WithEvents`,
//! `TypeOf … Is` a referenced interface, and multi-level / diamond reference chains.

use std::collections::BTreeMap;

use oxvba_bind::bind_projects;
use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, ProjectReference,
    ReferencedProjectManifest, SymbolProjectManifest,
};
use oxvba_symbol::provider::TypeLibResolver;

struct NullTypeLibs;
impl TypeLibResolver for NullTypeLibs {
    fn resolve(
        &self,
        _request: &oxvba_com::TypeLibResolveRequest,
    ) -> Option<oxvba_com::TypeLibMetadataBlob> {
        None
    }
}

// ── Fixture builders ─────────────────────────────────────────────────────────

fn proc_module(name: &str, src: &str) -> ModuleUnit {
    ModuleUnit {
        module_name: name.into(),
        module_kind: ModuleKind::Procedural,
        attributes: ModuleAttributes::named(name),
        source: src.into(),
    }
}

fn class_module(name: &str, src: &str, creatable: bool) -> ModuleUnit {
    let mut attrs = ModuleAttributes::named(name);
    attrs.vb_exposed = true;
    attrs.vb_creatable = creatable;
    ModuleUnit {
        module_name: name.into(),
        module_kind: ModuleKind::Class,
        attributes: attrs,
        source: src.into(),
    }
}

/// A `VB_PredeclaredId = True` class module — its name denotes a global singleton
/// instance (the `ThisWorkbook`/`Sheet1` document-module shape). Exposed so a
/// referencing project can reach it cross-bundle.
fn predeclared_class_module(name: &str, src: &str) -> ModuleUnit {
    let mut m = class_module(name, src, false);
    m.attributes.vb_predeclared_id = true;
    m
}

fn referenced(project_name: &str, modules: Vec<ModuleUnit>) -> ReferencedProjectManifest {
    ReferencedProjectManifest {
        project_name: project_name.into(),
        project_kind: ProjectKind::Library,
        modules,
    }
}

fn project(
    name: &str,
    modules: Vec<ModuleUnit>,
    refs: Vec<ReferencedProjectManifest>,
) -> SymbolProjectManifest {
    let references = refs
        .iter()
        .map(|r| ProjectReference::Project {
            referenced_project_name: r.project_name.clone(),
        })
        .collect();
    SymbolProjectManifest {
        project_name: name.into(),
        project_kind: ProjectKind::Source,
        modules,
        references,
        reference_projects: refs,
        conditional_constants: BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    }
}

/// Bind the whole closure (leaf-first, entry last), elaborate each project to OxIR,
/// link them on vm3 (entry last), run, and return the entry program's global 0.
fn link_run_global0_i32(closure_leaf_first: &[SymbolProjectManifest]) -> Option<i32> {
    let programs = bind_projects(closure_leaf_first, &NullTypeLibs).expect("bind_projects");
    let oxps: Vec<_> = programs
        .iter()
        .map(|p| oxvba_oxir::elaborate::elaborate(p).expect("elaborate"))
        .collect();
    let refs: Vec<&_> = oxps.iter().collect();
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let mut vm = oxvba_vm3::Vm3::link(&refs, &host).expect("link");
    vm.run_entry().expect("run");
    vm.slot(0).and_then(|v| v.as_i32())
}

fn link_run_fails(closure_leaf_first: &[SymbolProjectManifest]) {
    let programs = bind_projects(closure_leaf_first, &NullTypeLibs).expect("bind_projects");
    let oxps: Vec<_> = programs
        .iter()
        .map(|p| oxvba_oxir::elaborate::elaborate(p).expect("elaborate"))
        .collect();
    let refs: Vec<&_> = oxps.iter().collect();
    let host = NullHostServices::new(HostPolicy::deterministic_runtime());
    let mut vm = oxvba_vm3::Vm3::link(&refs, &host).expect("link");
    assert!(vm.run_entry().is_err());
}

fn bind_projects_error(closure_leaf_first: &[SymbolProjectManifest]) -> String {
    format!(
        "{:?}",
        bind_projects(closure_leaf_first, &NullTypeLibs).expect_err("bind_projects should fail")
    )
}

// ── The Lib + App two-project fixture ────────────────────────────────────────

/// A reusable `Lib` project: a hidden-module function + a `Const`/`Enum`, a
/// creatable `Widget` with a method, an event source `Clock`, an interface `IShape`
/// and an implementer `Circle`.
fn lib_modules() -> Vec<ModuleUnit> {
    vec![
        proc_module(
            "LibMod",
            "Public Function Add(ByVal a As Long, ByVal b As Long) As Long\n\
             Add = a + b\n\
             End Function\n\
             Public Const KMax As Long = 10\n\
             Public Enum Color\n  Red = 1\n  Green\nEnd Enum\n",
        ),
        class_module(
            "Widget",
            "Public Function Doubled(ByVal n As Long) As Long\nDoubled = n * 2\nEnd Function\n",
            true,
        ),
        class_module(
            "Clock",
            "Public Event Tick(ByVal n As Long)\n\
             Public Sub Fire()\nRaiseEvent Tick(7)\nEnd Sub\n",
            true,
        ),
        class_module("IShape", "Public Sub Draw()\nEnd Sub\n", false),
        class_module(
            "Circle",
            "Implements IShape\nPrivate Sub IShape_Draw()\nEnd Sub\n",
            true,
        ),
    ]
}

#[test]
fn cross_project_call_const_enum_new_method_and_typeof() {
    // App references Lib and exercises: a hidden-module function call, a referenced
    // `Const` and `Enum` value, `New` + a method on a referenced coclass, and
    // `TypeOf … Is` a referenced interface — accumulating one bit per check.
    let lib = project("Lib", lib_modules(), vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\n\
             Sub Main()\n\
             \x20   r = 0\n\
             \x20   If Add(2, 3) = 5 Then r = r + 1\n\
             \x20   If KMax = 10 Then r = r + 2\n\
             \x20   If Green = 2 Then r = r + 4\n\
             \x20   Dim w As Widget\n\
             \x20   Set w = New Lib.Widget\n\
             \x20   If w.Doubled(21) = 42 Then r = r + 8\n\
             \x20   Dim s As Object\n\
             \x20   Set s = New Lib.Circle\n\
             \x20   If TypeOf s Is Lib.IShape Then r = r + 16\n\
             End Sub\n",
        )],
        vec![referenced("Lib", lib_modules())],
    );
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(1 + 2 + 4 + 8 + 16));
}

#[test]
fn cross_module_enum_member_consts_execute() {
    let app = project(
        "App",
        vec![
            proc_module(
                "Types",
                "Public Enum EncodingMode\nStrictUrlEncoding = 0\nFormUrlEncoding = 1\nEnd Enum\n",
            ),
            proc_module(
                "Main",
                "Public r As Long\n\
                 Public Const FromBare As Long = FormUrlEncoding + 1\n\
                 Public Const FromQualified As Long = EncodingMode.FormUrlEncoding + 2\n\
                 Public Const FromModuleMember As Long = Types.FormUrlEncoding + 3\n\
                 Public Const FromProjectQualified As Long = App.EncodingMode.FormUrlEncoding + 4\n\
                 Sub Main()\n\
                 \x20   r = FromBare + FromQualified + FromModuleMember + FromProjectQualified\n\
                 End Sub\n",
            ),
        ],
        vec![],
    );
    assert_eq!(link_run_global0_i32(&[app]), Some(2 + 3 + 4 + 5));
}

#[test]
fn cross_project_withevents() {
    // A sink class in App holds `WithEvents src As Lib.Clock`; firing the source's
    // event (in Lib's bundle) must route to the sink's handler (in App's bundle).
    let app = project(
        "App",
        vec![
            proc_module(
                "Main",
                "Public r As Long\n\
                 Sub Main()\n\
                 \x20   Dim L As Listener\n\
                 \x20   Set L = New Listener\n\
                 \x20   L.Hook\n\
                 \x20   L.Go\n\
                 \x20   r = L.Fired\n\
                 End Sub\n",
            ),
            class_module(
                "Listener",
                "Private WithEvents src As Lib.Clock\n\
                 Public Fired As Long\n\
                 Public Sub Hook()\nSet src = New Lib.Clock\nEnd Sub\n\
                 Public Sub Go()\nsrc.Fire\nEnd Sub\n\
                 Private Sub src_Tick(ByVal n As Long)\nFired = n\nEnd Sub\n",
                true,
            ),
        ],
        vec![referenced("Lib", lib_modules())],
    );
    // The event carried 7 from Lib's `RaiseEvent Tick(7)` into App's `src_Tick`.
    let lib = project("Lib", lib_modules(), vec![]);
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(7));
}

#[test]
fn cross_bundle_call_arg_reads_callers_global() {
    // A cross-bundle call argument that is one of the *caller's* module globals must
    // be resolved against the caller's bundle, BEFORE dispatch switches to the
    // callee's bundle (bare global slots resolve against the current bundle). `seed`
    // is App global 1; if it were read after the bundle switch it would resolve to
    // Lib's globals (wrong / out of range).
    let lib = project(
        "Lib",
        vec![proc_module(
            "LibMod",
            "Public Function Echo(ByVal n As Long) As Long\nEcho = n\nEnd Function\n",
        )],
        vec![],
    );
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\n\
             Public seed As Long\n\
             Sub Main()\n\
             \x20   seed = 41\n\
             \x20   r = Echo(seed) + 1\n\
             End Sub\n",
        )],
        vec![referenced(
            "Lib",
            vec![proc_module(
                "LibMod",
                "Public Function Echo(ByVal n As Long) As Long\nEcho = n\nEnd Function\n",
            )],
        )],
    );
    // r (App global 0) = Echo(seed=41) + 1 = 42.
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(42));
}

#[test]
fn cross_bundle_free_function_reorders_named_args() {
    // Out-of-order named args to a referenced hidden-module function must land in
    // their declared positional slots (the cross-bundle callee is positional).
    let lib_mod = || {
        proc_module(
            "LibMod",
            "Public Function Diff(ByVal a As Long, ByVal b As Long) As Long\nDiff = a - b\nEnd Function\n",
        )
    };
    let lib = project("Lib", vec![lib_mod()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\nSub Main()\nr = Diff(b:=3, a:=10)\nEnd Sub\n",
        )],
        vec![referenced("Lib", vec![lib_mod()])],
    );
    // a - b = 10 - 3 = 7 (NOT source order 3 - 10 = -7).
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(7));
}

#[test]
fn cross_bundle_free_function_rejects_duplicate_named_arg_mapping() {
    let lib_mod = || {
        proc_module(
            "LibMod",
            "Public Function Diff(ByVal a As Long, ByVal b As Long) As Long\nDiff = a - b\nEnd Function\n",
        )
    };
    let lib = project("Lib", vec![lib_mod()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\nSub Main()\nr = Diff(1, a:=2)\nEnd Sub\n",
        )],
        vec![referenced("Lib", vec![lib_mod()])],
    );
    let err = bind_projects_error(&[lib, app]);
    assert!(
        err.contains("duplicate argument for parameter a"),
        "unexpected error: {err}"
    );
}

#[test]
fn cross_bundle_free_function_rejects_positional_after_named_arg() {
    let lib_mod = || {
        proc_module(
            "LibMod",
            "Public Function Diff(ByVal a As Long, ByVal b As Long) As Long\nDiff = a - b\nEnd Function\n",
        )
    };
    let lib = project("Lib", vec![lib_mod()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\nSub Main()\nr = Diff(b:=2, 1)\nEnd Sub\n",
        )],
        vec![referenced("Lib", vec![lib_mod()])],
    );
    let err = bind_projects_error(&[lib, app]);
    assert!(
        err.contains("positional argument cannot follow named argument"),
        "unexpected error: {err}"
    );
}

#[test]
fn cross_bundle_free_function_applies_optional_defaults() {
    let lib_mod = || {
        proc_module(
            "LibMod",
            "Public Function Pack(ByVal text As String, Optional SpaceAsPlus As Boolean = False, Optional EncodeUnsafe As Boolean = True, Optional mode As Long = 7) As Long\n\
             If text = \"\" Then text = \"x\"\n\
             Pack = Len(text)\n\
             If SpaceAsPlus Then Pack = Pack + 10\n\
             If EncodeUnsafe Then Pack = Pack + 20\n\
             Pack = Pack + mode * 100\n\
             End Function\n\
             Public Function TypedDefaults(Optional label As String, Optional enabled As Boolean, Optional count As Long) As Long\n\
             TypedDefaults = Len(label)\n\
             If enabled Then TypedDefaults = TypedDefaults + 10\n\
             TypedDefaults = TypedDefaults + count * 100\n\
             End Function\n",
        )
    };
    let lib = project("Lib", vec![lib_mod()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\n\
             Sub Main()\n\
             r = Pack(\"abc\") + TypedDefaults()\n\
             End Sub\n",
        )],
        vec![referenced("Lib", vec![lib_mod()])],
    );
    // Pack: Len=3, SpaceAsPlus=False, EncodeUnsafe=True, mode=7 -> 723.
    // TypedDefaults: string/boolean/long declared defaults -> "", False, 0 -> 0.
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(723));
}

#[test]
fn cross_bundle_variant_optional_default_preserves_integer_carrier() {
    let lib_mod = || {
        proc_module(
            "LibMod",
            "Public Function DefaultTag(Optional ByVal value As Variant = 7) As Long\n\
             DefaultTag = VarType(value)\n\
             End Function\n",
        )
    };
    let lib = project("Lib", vec![lib_mod()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\n\
             Sub Main()\n\
             r = DefaultTag()\n\
             End Sub\n",
        )],
        vec![referenced("Lib", vec![lib_mod()])],
    );
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(2));
}

#[test]
fn cross_bundle_free_function_applies_named_optional_defaults_between_supplied_args() {
    let lib_mod = || {
        proc_module(
            "LibMod",
            "Public Function Pack(ByVal text As String, Optional SpaceAsPlus As Boolean = False, Optional EncodeUnsafe As Boolean = True, Optional mode As Long = 7) As Long\n\
             Pack = Len(text)\n\
             If SpaceAsPlus Then Pack = Pack + 10\n\
             If EncodeUnsafe Then Pack = Pack + 20\n\
             Pack = Pack + mode * 100\n\
             End Function\n",
        )
    };
    let lib = project("Lib", vec![lib_mod()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\n\
             Sub Main()\n\
             r = Pack(text:=\"abc\", mode:=2)\n\
             End Sub\n",
        )],
        vec![referenced("Lib", vec![lib_mod()])],
    );
    // The named `mode` argument must not leave the middle optionals as Missing:
    // SpaceAsPlus=False and EncodeUnsafe=True still apply.
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(223));
}

#[test]
fn cross_bundle_free_function_keeps_missing_required_arg_omitted() {
    let lib_mod = || {
        proc_module(
            "LibMod",
            "Public Function Needs(ByVal required As Long, Optional bonus As Long = 5) As Long\n\
             Needs = required + bonus\n\
             End Function\n",
        )
    };
    let lib = project("Lib", vec![lib_mod()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\n\
             Sub Main()\n\
             r = Needs()\n\
             End Sub\n",
        )],
        vec![referenced("Lib", vec![lib_mod()])],
    );
    link_run_fails(&[lib, app]);
}

#[test]
fn cross_project_enum_type_qualifier_resolves_from_referenced_surface() {
    let lib_mod = || {
        proc_module(
            "LibMod",
            "Public Enum WebMethod\nHttpGet = 0\nHttpPost = 1\nEnd Enum\n\
             Public Function MethodToName(ByVal method As WebMethod) As String\n\
             If method = WebMethod.HttpPost Then\nMethodToName = \"POST\"\nElse\nMethodToName = \"GET\"\nEnd If\n\
             End Function\n",
        )
    };
    let lib = project("Lib", vec![lib_mod()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\n\
             Sub Main()\n\
             If MethodToName(WebMethod.HttpPost) = \"POST\" Then r = 42\n\
             End Sub\n",
        )],
        vec![referenced("Lib", vec![lib_mod()])],
    );
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(42));
}

#[test]
fn cross_project_enum_type_qualifier_can_be_project_qualified() {
    let lib_mod = || {
        proc_module(
            "LibMod",
            "Public Enum WebMethod\nHttpGet = 0\nHttpPost = 1\nEnd Enum\n\
             Public Function IsPost(ByVal method As WebMethod) As Boolean\n\
             IsPost = (method = WebMethod.HttpPost)\n\
             End Function\n",
        )
    };
    let lib = project("Lib", vec![lib_mod()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\n\
             Sub Main()\n\
             If IsPost(Lib.WebMethod.HttpPost) Then r = 42\n\
             End Sub\n",
        )],
        vec![referenced("Lib", vec![lib_mod()])],
    );
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(42));
}

#[test]
fn referenced_module_variable_is_not_cross_project_bindable() {
    // A referenced standard-module public VARIABLE has no callable export, so a
    // cross-project reference must fail cleanly at bind time under Option
    // Explicit rather than opaquely at link time.
    let lib_mod = || proc_module("LibMod", "Public gConfig As Long\n");
    let lib = project("Lib", vec![lib_mod()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Option Explicit\nPublic r As Long\nSub Main()\nr = gConfig\nEnd Sub\n",
        )],
        vec![referenced("Lib", vec![lib_mod()])],
    );
    let err = bind_projects_error(&[lib, app]);
    assert!(err.contains("VariableNotDefined"));
}

#[test]
fn referenced_module_variable_name_without_option_explicit_is_local() {
    // Without Option Explicit, VBA treats an otherwise unresolved name as an
    // implicit local variable. That still must not tunnel through the referenced
    // project's public module variable.
    let lib_mod = || proc_module("LibMod", "Public gConfig As Long\n");
    let lib = project("Lib", vec![lib_mod()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\nSub Main()\nr = gConfig\nEnd Sub\n",
        )],
        vec![referenced("Lib", vec![lib_mod()])],
    );
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(0));
}

#[test]
fn module_qualified_call_resolves_when_qualifier_also_names_a_sub() {
    // The entry-shim shape that made the SQLiteForExcel demo recurse infinitely: the
    // closure loader injects a `Sub Main` shim that does `Call Main.Main`, and the
    // project's own entry is module `Main`'s `Sub Main`. The shim's `Main.Main` must
    // resolve the module-qualified `Main.Main` proc — NOT bind the receiver `Main` as
    // the (shim's own) `Main` sub and self-recurse. `ids.entry()` picks the first
    // `Main` (the shim, module-first), so the shim is the entry here.
    let app = project(
        "App",
        vec![
            proc_module("Shim", "Public Sub Main()\n    Call Main.Main\nEnd Sub\n"),
            proc_module(
                "Main",
                "Public r As Long\nPublic Sub Main()\n    r = 7\nEnd Sub\n",
            ),
        ],
        vec![],
    );
    assert_eq!(link_run_global0_i32(&[app]), Some(7));
}

#[test]
fn module_qualified_call_with_conditional_compilation_resolves_named_proc() {
    // The SQLiteForExcel shape: the called module has `#If Win64`-wrapped proc
    // signatures (each proc declared twice, one branch blanked by cond-comp) before
    // and around the target. If cond-comp desyncs proc-decl ↔ ProcId alignment, the
    // qualified call lands on the wrong proc (the demo recursed into `Main`).
    let helper = "Public r As Long\n\
         #If Win64 Then\n\
         Public Sub First(ByVal a As LongPtr)\n\
         #Else\n\
         Public Sub First(ByVal a As Long)\n\
         #End If\n\
         End Sub\n\
         #If Win64 Then\n\
         Public Sub Go()\n\
         #Else\n\
         Public Sub Go()\n\
         #End If\n\
         \x20   r = 42\n\
         End Sub\n";
    let app = project(
        "App",
        vec![
            proc_module("Main", "Sub Main()\n    Call Helper.Go\nEnd Sub\n"),
            proc_module("Helper", helper),
        ],
        vec![],
    );
    assert_eq!(link_run_global0_i32(&[app]), Some(42));
}

#[test]
fn module_qualified_call_from_main_resolves_the_named_proc() {
    // `Call Helper.Go` from `Sub Main` (in module `Main`) must invoke `Helper.Go`,
    // NOT recurse into `Main` (proc 0). Regression for the SQLiteForExcel demo's
    // `Main` → `Call Sqlite3Demo.AllTests` infinite recursion.
    let app = project(
        "App",
        vec![
            proc_module(
                "Main",
                "Public r As Long\nSub Main()\n    Call Helper.Go\nEnd Sub\n",
            ),
            proc_module("Helper", "Public Sub Go()\n    r = 42\nEnd Sub\n"),
        ],
        vec![],
    );
    assert_eq!(link_run_global0_i32(&[app]), Some(42));
}

// ── Predeclared instances (VB_PredeclaredId) ─────────────────────────────────

#[test]
fn predeclared_instance_singleton_in_active_project() {
    // A `VB_PredeclaredId` class is reachable as a global singleton by its module
    // name; its per-instance state persists across accesses (it is one instance,
    // created on first use), distinct from `New`.
    let app = project(
        "App",
        vec![
            proc_module(
                "Main",
                "Public r As Long\n\
                 Sub Main()\n\
                 \x20   Counter.Bump\n\
                 \x20   Counter.Bump\n\
                 \x20   Counter.Bump\n\
                 \x20   r = Counter.Total\n\
                 End Sub\n",
            ),
            predeclared_class_module(
                "Counter",
                "Private n As Long\n\
                 Public Sub Bump()\nn = n + 1\nEnd Sub\n\
                 Public Property Get Total() As Long\nTotal = n\nEnd Property\n",
            ),
        ],
        vec![],
    );
    // Three Bump()s on the one singleton → Total = 3 (a fresh instance would be 0).
    assert_eq!(link_run_global0_i32(&[app]), Some(3));
}

#[test]
fn predeclared_new_makes_independent_instance() {
    // `New <predeclared class>` allocates a fresh instance, independent of the
    // global singleton — bumping the new one does not change the singleton's state.
    let app = project(
        "App",
        vec![
            proc_module(
                "Main",
                "Public r As Long\n\
                 Sub Main()\n\
                 \x20   Dim fresh As Counter\n\
                 \x20   Set fresh = New Counter\n\
                 \x20   fresh.Bump\n\
                 \x20   fresh.Bump\n\
                 \x20   Counter.Bump\n\
                 \x20   r = Counter.Total + fresh.Total * 10\n\
                 End Sub\n",
            ),
            predeclared_class_module(
                "Counter",
                "Private n As Long\n\
                 Public Sub Bump()\nn = n + 1\nEnd Sub\n\
                 Public Property Get Total() As Long\nTotal = n\nEnd Property\n",
            ),
        ],
        vec![],
    );
    // singleton.Total = 1, fresh.Total = 2 → 1 + 2*10 = 21.
    assert_eq!(link_run_global0_i32(&[app]), Some(21));
}

#[test]
fn cross_project_predeclared_instance_property() {
    // A referenced project exposes a `VB_PredeclaredId` class; the active project
    // reaches its singleton by bare name and reads a `Property Get` — exactly the
    // `ThisWorkbook.Path` shape. The instance lives in (and dispatches into) the
    // referenced project's bundle.
    let host = || {
        predeclared_class_module(
            "HostEnv",
            "Public Property Get Answer() As Long\nAnswer = 42\nEnd Property\n",
        )
    };
    let lib = project("Lib", vec![host()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\nSub Main()\nr = HostEnv.Answer\nEnd Sub\n",
        )],
        vec![referenced("Lib", vec![host()])],
    );
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(42));
}

#[test]
fn cross_project_predeclared_string_property_returns_intact() {
    // A cross-bundle `Property Get … As String` on a predeclared instance, then a
    // `+` string concat — exactly the SQLiteForExcel `ThisWorkbook.Path + "\x64"`
    // shape. The returned BStr must cross the bundle boundary intact (a corrupted
    // length would make the concat allocate gigabytes). Asserting on `Len` keeps the
    // i32 harness.
    let host = || {
        predeclared_class_module(
            "HostEnv",
            "Public Property Get Path() As String\nPath = \"abc\"\nEnd Property\n",
        )
    };
    let lib = project("Lib", vec![host()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\nSub Main()\nr = Len(HostEnv.Path + \"def\")\nEnd Sub\n",
        )],
        vec![referenced("Lib", vec![host()])],
    );
    // Len("abc" + "def") = 6.
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(6));
}

#[test]
fn cross_project_predeclared_string_through_optional_byval_param() {
    // The exact `SQLite3Initialize(ThisWorkbook.Path + "\x64")` shape: a cross-bundle
    // predeclared `Property Get … As String` result is passed through another proc's
    // `Optional ByVal … As String` parameter and then string-manipulated. A corrupted
    // BStr length here would make the inner concat allocate gigabytes.
    let host = || {
        predeclared_class_module(
            "HostEnv",
            "Public Property Get Path() As String\nPath = \"abc\"\nEnd Property\n",
        )
    };
    let lib = project("Lib", vec![host()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\n\
             Sub Main()\n\
             \x20   r = Init(HostEnv.Path + \"\\x64\")\n\
             End Sub\n\
             Function Init(Optional ByVal libDir As String) As Long\n\
             \x20   If libDir = \"\" Then libDir = \"default\"\n\
             \x20   If Right(libDir, 1) <> \"\\\" Then libDir = libDir & \"\\\"\n\
             \x20   Init = Len(libDir + \"SQLite3.dll\")\n\
             End Function\n",
        )],
        vec![referenced("Lib", vec![host()])],
    );
    // libDir = "abc\x64" -> "abc\x64\" (8); Len("abc\x64\SQLite3.dll") = 8 + 11 = 19.
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(19));
}

#[test]
fn cross_project_predeclared_instance_persists_state() {
    // The referenced predeclared singleton holds state across accesses from the
    // active project (one shared instance in the owning bundle).
    let host = || {
        predeclared_class_module(
            "HostEnv",
            "Private mHits As Long\n\
             Public Sub Touch()\nmHits = mHits + 1\nEnd Sub\n\
             Public Property Get Hits() As Long\nHits = mHits\nEnd Property\n",
        )
    };
    let lib = project("Lib", vec![host()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\n\
             Sub Main()\n\
             \x20   HostEnv.Touch\n\
             \x20   HostEnv.Touch\n\
             \x20   r = HostEnv.Hits\n\
             End Sub\n",
        )],
        vec![referenced("Lib", vec![host()])],
    );
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(2));
}

#[test]
fn cross_project_default_member_bare_let_get_preserves_object_reference() {
    // A referenced coclass publishes a VB_UserMemId=0 scalar property. The active
    // project should use that default member in Let/value contexts while `Set`
    // assignments keep the cross-bundle object reference.
    let widget = || {
        class_module(
            "Widget",
            "Private mV As Long\n\
             Public Property Get Value() As Long\nValue = mV\nEnd Property\n\
             Attribute Value.VB_UserMemId = 0\n\
             Public Property Let Value(ByVal v As Long)\nmV = v\nEnd Property\n\
             Attribute Value.VB_UserMemId = 0\n",
            true,
        )
    };
    let lib = project("Lib", vec![widget()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Public r As Long\n\
             Sub Main()\n\
             \x20   Dim src As Widget\n\
             \x20   Dim dst As Widget\n\
             \x20   Dim mirror As Widget\n\
             \x20   Set src = New Lib.Widget\n\
             \x20   Set dst = New Lib.Widget\n\
             \x20   src = 7\n\
             \x20   dst = src\n\
             \x20   Set mirror = dst\n\
             \x20   mirror = 9\n\
             \x20   r = dst\n\
             End Sub\n",
        )],
        vec![referenced("Lib", vec![widget()])],
    );
    assert_eq!(link_run_global0_i32(&[lib, app]), Some(9));
}

#[test]
fn cross_project_property_let_does_not_fallback_to_getter() {
    let widget = || {
        class_module(
            "Widget",
            "Public Property Get Value() As Long\nValue = 1\nEnd Property\n",
            true,
        )
    };
    let lib = project("Lib", vec![widget()], vec![]);
    let app = project(
        "App",
        vec![proc_module(
            "Main",
            "Sub Main()\n\
             \x20   Dim w As Widget\n\
             \x20   Set w = New Lib.Widget\n\
             \x20   w.Value = 10\n\
             End Sub\n",
        )],
        vec![referenced("Lib", vec![widget()])],
    );
    let err = bind_projects_error(&[lib, app]);
    assert!(
        err.contains("Value") || err.contains("PropertyLet"),
        "get-only cross-project property assignment should not bind through the getter: {err}"
    );
}

// ── Multi-level chain + diamond ──────────────────────────────────────────────

#[test]
fn multi_level_chain_a_b_c() {
    // A → B → C: A calls a B function that itself calls a C function. Each project
    // is bound from its own manifest (carrying its transitive reference source); the
    // transitive imports compose at link time.
    let c = || {
        referenced(
            "C",
            vec![proc_module(
                "CMod",
                "Public Function CVal() As Long\nCVal = 100\nEnd Function\n",
            )],
        )
    };
    let b_modules = vec![proc_module(
        "BMod",
        "Public Function BVal() As Long\nBVal = CVal() + 1\nEnd Function\n",
    )];
    let c_proj = project("C", c().modules, vec![]);
    let b_proj = project("B", b_modules.clone(), vec![c()]);
    let a_proj = project(
        "A",
        vec![proc_module(
            "Main",
            "Public r As Long\nSub Main()\nr = BVal()\nEnd Sub\n",
        )],
        vec![referenced("B", b_modules)],
    );
    // r = B.BVal() = C.CVal() + 1 = 101, computed across three bundles.
    assert_eq!(link_run_global0_i32(&[c_proj, b_proj, a_proj]), Some(101));
}

#[test]
fn diamond_a_b_d_a_c_d_links_d_once() {
    // A → B → D and A → C → D: D is referenced via two paths. `Vm3::link` resolves
    // both B's and C's import of D to the single loaded D bundle (D links once).
    let d = || {
        referenced(
            "D",
            vec![proc_module(
                "DMod",
                "Public Function DVal() As Long\nDVal = 50\nEnd Function\n",
            )],
        )
    };
    let b_modules = vec![proc_module(
        "BMod",
        "Public Function BFromD() As Long\nBFromD = DVal()\nEnd Function\n",
    )];
    let c_modules = vec![proc_module(
        "CMod",
        "Public Function CFromD() As Long\nCFromD = DVal()\nEnd Function\n",
    )];
    let d_proj = project("D", d().modules, vec![]);
    let b_proj = project("B", b_modules.clone(), vec![d()]);
    let c_proj = project("C", c_modules.clone(), vec![d()]);
    let a_proj = project(
        "A",
        vec![proc_module(
            "Main",
            "Public r As Long\nSub Main()\nr = BFromD() + CFromD()\nEnd Sub\n",
        )],
        vec![referenced("B", b_modules), referenced("C", c_modules)],
    );
    // r = D.DVal() + D.DVal() = 100; both paths resolve to the one D bundle.
    assert_eq!(
        link_run_global0_i32(&[d_proj, b_proj, c_proj, a_proj]),
        Some(100)
    );
}
