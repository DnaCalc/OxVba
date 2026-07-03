use oxvba_host::{Engine, HostConfig, RuntimeProfileId};

fn run_with_profile(profile: RuntimeProfileId, source: &str) -> i32 {
    let mut engine = Engine::new(HostConfig::vm3());
    engine.set_runtime_profile(profile);
    let snapshot = engine
        .execute_source_with_variant_snapshot_clean(source)
        .unwrap_or_else(|err| panic!("{err}"));
    snapshot[0].as_i32().expect("first public Long")
}

#[test]
fn conditional_compilation_uses_runtime_target_constants() {
    let source = "Public r As Long\n\
         Sub Main()\n\
         #If Mac Then\n\
             r = 1\n\
         #ElseIf Win64 Then\n\
             r = 2\n\
         #Else\n\
             r = 3\n\
         #End If\n\
         End Sub\n";

    assert_eq!(run_with_profile(RuntimeProfileId::MacOsHeadless, source), 1);
    if cfg!(target_pointer_width = "64") {
        assert_eq!(run_with_profile(RuntimeProfileId::WindowsStdio, source), 2);
    } else {
        assert_eq!(run_with_profile(RuntimeProfileId::WindowsStdio, source), 3);
    }
}

#[test]
fn explicit_project_constants_override_runtime_target_defaults() {
    use oxvba_symbol::manifest as sym;
    let mut constants = std::collections::BTreeMap::new();
    constants.insert("Mac".to_string(), 0);
    constants.insert("Win64".to_string(), 1);

    let manifest = sym::SymbolProjectManifest {
        project_name: "Main".to_string(),
        project_kind: sym::ProjectKind::Source,
        modules: vec![sym::ModuleUnit {
            module_name: "Main".to_string(),
            module_kind: sym::ModuleKind::Procedural,
            attributes: sym::ModuleAttributes::named("Main"),
            source: "Public r As Long\n\
                 Sub Main()\n\
                 #If Mac Then\n\
                     r = 1\n\
                 #ElseIf Win64 Then\n\
                     r = 2\n\
                 #Else\n\
                     r = 3\n\
                 #End If\n\
                 End Sub\n"
                .to_string(),
        }],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: constants,
        conditional_compilation_target: Default::default(),
    };

    let mut engine = Engine::new(HostConfig::vm3());
    engine.set_runtime_profile(RuntimeProfileId::MacOsHeadless);
    let snapshot = engine
        .execute_manifest_with_variant_snapshot(&manifest)
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(snapshot[0].as_i32(), Some(2));
}

#[test]
fn referenced_project_uses_runtime_target_predefines_without_active_project_constants() {
    use oxvba_symbol::manifest as sym;

    let lib_source = "#If Mac Then\n\
         Public Function LibPick() As Long\n\
             LibPick = 11\n\
         End Function\n\
         #Else\n\
         Public Function OtherPick() As Long\n\
             OtherPick = 22\n\
         End Function\n\
         #End If\n";
    let lib_module = || sym::ModuleUnit {
        module_name: "LibMod".to_string(),
        module_kind: sym::ModuleKind::Procedural,
        attributes: sym::ModuleAttributes::named("LibMod"),
        source: lib_source.to_string(),
    };

    let lib_manifest = sym::SymbolProjectManifest {
        project_name: "LibProj".to_string(),
        project_kind: sym::ProjectKind::Library,
        modules: vec![lib_module()],
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: std::collections::BTreeMap::new(),
        conditional_compilation_target: Default::default(),
    };

    let manifest = sym::SymbolProjectManifest {
        project_name: "Main".to_string(),
        project_kind: sym::ProjectKind::Source,
        modules: vec![sym::ModuleUnit {
            module_name: "Main".to_string(),
            module_kind: sym::ModuleKind::Procedural,
            attributes: sym::ModuleAttributes::named("Main"),
            source: "Public r As Long\n\
                 Sub Main()\n\
                     r = LibPick()\n\
                 End Sub\n"
                .to_string(),
        }],
        references: vec![sym::ProjectReference::Project {
            referenced_project_name: "LibProj".to_string(),
        }],
        reference_projects: vec![sym::ReferencedProjectManifest {
            project_name: "LibProj".to_string(),
            project_kind: sym::ProjectKind::Library,
            modules: vec![lib_module()],
        }],
        conditional_constants: {
            let mut constants = std::collections::BTreeMap::new();
            constants.insert("Mac".to_string(), 0);
            constants.insert("Win64".to_string(), 1);
            constants
        },
        conditional_compilation_target: Default::default(),
    };

    let mut engine = Engine::new(HostConfig::vm3());
    engine.set_runtime_profile(RuntimeProfileId::MacOsHeadless);
    let snapshot = engine
        .execute_project_closure_with_variant_snapshot(&[lib_manifest, manifest])
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(snapshot[0].as_i32(), Some(11));
}
