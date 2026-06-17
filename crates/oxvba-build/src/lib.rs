//! Clean build target orchestration for OxVBA wrapper outputs.
//!
//! `WrappedComServer` builds are intentionally package-first: the compiler emits
//! the canonical bundle package, derives COM descriptors from the symbol export
//! surface, then wrapper-specific tooling consumes those artifacts.

mod com_descriptor;
mod compile;
mod identity;
mod idl;
mod shim;

use std::fs;
use std::path::{Path, PathBuf};

pub use com_descriptor::{
    ComClassDescriptor, ComEventDescriptor, ComInvokeKind, ComMemberDescriptor, ComParamType,
    ComServerDescriptor,
};
pub use compile::{ShimCompileError, compile_shim_dll, compile_typelib};
pub use identity::deterministic_uuid;
pub use idl::generate_idl;
pub use shim::generate_shim_source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedComServerBuildOptions {
    pub project_path: PathBuf,
    pub out_dir: PathBuf,
    pub compile_dll: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedComServerBuildOutput {
    pub oxb_path: PathBuf,
    pub descriptor_path: PathBuf,
    pub idl_path: PathBuf,
    pub shim_source_path: PathBuf,
    pub dll_target_path: PathBuf,
    pub tlb_target_path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Project(#[from] oxvba_project::BasProjError),
    #[error("{0}")]
    Bind(#[from] oxvba_bind::BindError),
    #[error("{0}")]
    Linearize(String),
    #[error("{0}")]
    Symbol(#[from] oxvba_symbol::SymbolModelError),
    #[error("{0}")]
    BundlePackage(#[from] oxvba_bundle::BundlePackageError),
    #[error("WrappedComServer requires OutputType=ComServer, found {found:?}")]
    InvalidOutputType { found: oxvba_project::OutputType },
    #[error("WrappedComServer requires BuildTarget=WrappedComServer, found {found:?}")]
    InvalidBuildTarget { found: oxvba_project::BuildTarget },
    #[error("project closure was empty for {path}")]
    EmptyProjectClosure { path: String },
    #[error("ComServer project {project_name} has no exposed class surface")]
    NoComClasses { project_name: String },
    #[error("ComServer project {project_name} has no creatable classes")]
    NoCreatableClasses { project_name: String },
    #[error("failed to serialize COM server descriptor: {0}")]
    DescriptorSerialize(serde_json::Error),
    #[error("{0}")]
    ShimCompile(#[from] ShimCompileError),
}

pub fn build_wrapped_com_server(
    options: &WrappedComServerBuildOptions,
) -> Result<WrappedComServerBuildOutput, BuildError> {
    let loaded = load_single_project(&options.project_path)?;
    validate_wrapped_com_server_project(&loaded)?;

    fs::create_dir_all(&options.out_dir).map_err(|source| BuildError::Io {
        path: options.out_dir.display().to_string(),
        source,
    })?;

    let closure = oxvba_project::load_project_closure(&options.project_path)?;
    let root_manifest = closure
        .last()
        .ok_or_else(|| BuildError::EmptyProjectClosure {
            path: options.project_path.display().to_string(),
        })?;
    let project_name = root_manifest.project_name.clone();
    let artifact_stem = artifact_stem(&project_name);

    let package = build_bundle_package(&closure)?;
    let oxb_path = options.out_dir.join(format!("{artifact_stem}.oxb"));
    write_bytes(&oxb_path, &package.to_bytes()?)?;

    let descriptor = build_com_descriptor(root_manifest)?;
    if descriptor.classes.is_empty() {
        return Err(BuildError::NoComClasses { project_name });
    }
    if descriptor.creatable_classes().next().is_none() {
        return Err(BuildError::NoCreatableClasses { project_name });
    }

    let descriptor_path = options
        .out_dir
        .join(format!("{artifact_stem}.comserver.json"));
    let descriptor_bytes =
        serde_json::to_vec_pretty(&descriptor).map_err(BuildError::DescriptorSerialize)?;
    write_bytes(&descriptor_path, &descriptor_bytes)?;

    let idl_path = options.out_dir.join(format!("{artifact_stem}.idl"));
    write_text(&idl_path, &generate_idl(&descriptor))?;
    let tlb_target_path = options.out_dir.join(format!("{artifact_stem}.tlb"));

    let shim_source_path = options
        .out_dir
        .join(format!("{artifact_stem}_com_server.rs"));
    write_text(
        &shim_source_path,
        &generate_shim_source(&descriptor, &oxb_path, &descriptor_path, &tlb_target_path),
    )?;
    let dll_target_path = options.out_dir.join(format!("{artifact_stem}.dll"));
    if options.compile_dll {
        compile_typelib(&idl_path, &tlb_target_path)?;
        compile_shim_dll(&shim_source_path, &dll_target_path)?;
    }

    Ok(WrappedComServerBuildOutput {
        oxb_path,
        descriptor_path,
        idl_path,
        shim_source_path,
        dll_target_path,
        tlb_target_path,
    })
}

fn load_single_project(path: &Path) -> Result<oxvba_project::LoadedProject, BuildError> {
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("vbp"))
    {
        Ok(oxvba_project::load_vbp(path)?)
    } else {
        Ok(oxvba_project::load_basproj(path)?)
    }
}

fn validate_wrapped_com_server_project(
    loaded: &oxvba_project::LoadedProject,
) -> Result<(), BuildError> {
    if loaded.output_type != oxvba_project::OutputType::ComServer {
        return Err(BuildError::InvalidOutputType {
            found: loaded.output_type,
        });
    }
    if loaded.build_target != oxvba_project::BuildTarget::WrappedComServer {
        return Err(BuildError::InvalidBuildTarget {
            found: loaded.build_target,
        });
    }
    Ok(())
}

fn build_bundle_package(
    closure: &[oxvba_symbol::manifest::SymbolProjectManifest],
) -> Result<oxvba_bundle::BundlePackage, BuildError> {
    let typelibs = oxvba_symbol::CatalogTypeLibResolver;
    let programs = oxvba_bind::bind_projects(closure, &typelibs)?;
    let bundles: Vec<oxvba_bundle::Bundle> = programs
        .iter()
        .map(oxvba_bundle::linearize)
        .collect::<Result<_, _>>()
        .map_err(|err| BuildError::Linearize(err.to_string()))?;
    Ok(oxvba_bundle::BundlePackage::new(
        bundles,
        closure.len().saturating_sub(1),
    ))
}

fn build_com_descriptor(
    root_manifest: &oxvba_symbol::manifest::SymbolProjectManifest,
) -> Result<ComServerDescriptor, BuildError> {
    let typelibs = oxvba_symbol::CatalogTypeLibResolver;
    let env = oxvba_symbol::build_resolution_environment(root_manifest, &typelibs)?;
    let surface = env
        .export_surfaces()
        .first()
        .expect("resolution environment always includes active export surface");
    Ok(ComServerDescriptor::from_surface(surface))
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), BuildError> {
    fs::write(path, bytes).map_err(|source| BuildError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn write_text(path: &Path, text: &str) -> Result<(), BuildError> {
    write_bytes(path, text.as_bytes())
}

fn artifact_stem(project_name: &str) -> String {
    let mut out = String::new();
    for ch in project_name.chars() {
        if ch == '_' || ch == '-' || ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "OxVbaComServer".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_com_server_build_emits_package_descriptor_and_idl() {
        let temp = TestDir::new("wrapped_com_server_build_emits_package_descriptor_and_idl");
        let project_path = temp.path.join("Demo.basproj");
        let class_path = temp.path.join("Calculator.cls");
        let pinger_path = temp.path.join("Pinger.cls");
        let counter_path = temp.path.join("Counter.cls");
        let returner_path = temp.path.join("Returner.cls");
        let out_dir = temp.path.join("out");

        write(
            &class_path,
            r#"
Public Event Changed(ByVal value As Long)

Public Function Add(ByVal a As Long, ByVal b As Long) As Long
    Add = a + b
End Function

Public Sub Fire(ByVal value As Long)
    RaiseEvent Changed(value)
End Sub
"#,
        );
        write(
            &pinger_path,
            r#"
Public Function Ping() As Long
    Ping = 42
End Function

Public Function AddPair(ByVal a As Long, ByVal b As Long) As Long
    AddPair = a + b
End Function

Public Function Average(ByVal a As Double, ByVal b As Double) As Double
    Average = (a + b) / 2#
End Function
"#,
        );
        write(
            &counter_path,
            r#"
Private mValue As Long

Public Property Get Value() As Long
    Value = mValue
End Property

Public Property Let Value(ByVal newValue As Long)
    mValue = newValue
End Property
"#,
        );
        write(
            &returner_path,
            r#"
Public Function ReturnSelf() As Object
    Set ReturnSelf = Me
End Function

Public Function Ping() As Long
    Ping = 42
End Function
"#,
        );
        write(
            &project_path,
            r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>ComServer</OutputType>
    <BuildTarget>WrappedComServer</BuildTarget>
    <ProjectName>DemoServer</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <ClassModule Include="Calculator.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.Calculator</ProgId>
      <Description>Calculator class</Description>
    </ClassModule>
    <ClassModule Include="Pinger.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.Pinger</ProgId>
      <Description>Pinger class</Description>
    </ClassModule>
    <ClassModule Include="Counter.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.Counter</ProgId>
      <Description>Counter class</Description>
    </ClassModule>
    <ClassModule Include="Returner.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.Returner</ProgId>
      <Description>Returner class</Description>
    </ClassModule>
  </ItemGroup>
</Project>
"#,
        );

        let output = build_wrapped_com_server(&WrappedComServerBuildOptions {
            project_path,
            out_dir,
            compile_dll: false,
        })
        .expect("WrappedComServer build should emit artifacts");

        let package_bytes = std::fs::read(&output.oxb_path).expect("oxb should exist");
        let package =
            oxvba_bundle::BundlePackage::from_bytes(&package_bytes).expect("oxb should parse");
        assert_eq!(package.bundles.len(), 1);
        assert_eq!(package.entry_bundle, 0);

        let descriptor_text =
            std::fs::read_to_string(&output.descriptor_path).expect("descriptor should exist");
        let descriptor: ComServerDescriptor =
            serde_json::from_str(&descriptor_text).expect("descriptor should parse");
        assert_eq!(descriptor.project_name, "DemoServer");
        let class = descriptor
            .classes
            .iter()
            .find(|class| class.class_name == "Calculator")
            .expect("Calculator descriptor");
        assert!(class.creatable);
        assert_eq!(class.prog_id, "DemoServer.Calculator");
        assert_eq!(class.description.as_deref(), Some("Calculator class"));
        assert!(class.members.iter().any(|member| member.name == "Add"));
        assert!(class.events.iter().any(|event| event.name == "Changed"));
        let pinger = descriptor
            .classes
            .iter()
            .find(|class| class.class_name == "Pinger")
            .expect("Pinger descriptor");
        assert_eq!(pinger.members.len(), 3);
        assert_eq!(pinger.members[0].vtable_slot, Some(7));
        assert_eq!(pinger.members[1].vtable_slot, Some(8));
        assert_eq!(pinger.members[2].vtable_slot, Some(9));

        let idl = std::fs::read_to_string(&output.idl_path).expect("idl should exist");
        assert!(idl.contains("library DemoServerLib"));
        assert!(idl.contains("dispinterface ICalculator"));
        assert!(idl.contains("dispinterface _CalculatorEvents"));
        assert!(idl.contains("long Add([in] long a, [in] long b);"));
        assert!(idl.contains("void Fire([in] long value);"));
        assert!(idl.contains("void Changed"));
        assert!(idl.contains("interface IPinger : IDispatch"));
        assert!(idl.contains("HRESULT Ping([out, retval] long* result);"));
        assert!(
            idl.contains("HRESULT AddPair([in] long a, [in] long b, [out, retval] long* result);")
        );
        assert!(idl.contains(
            "HRESULT Average([in] double a, [in] double b, [out, retval] double* result);"
        ));
        assert!(idl.contains("[default] interface IPinger;"));
        let counter = descriptor
            .classes
            .iter()
            .find(|class| class.class_name == "Counter")
            .expect("Counter descriptor");
        assert_eq!(counter.members.len(), 2);
        assert_eq!(counter.members[0].vtable_slot, Some(7));
        assert_eq!(counter.members[0].invoke_kind, ComInvokeKind::PropertyGet);
        assert_eq!(counter.members[1].vtable_slot, Some(8));
        assert_eq!(counter.members[1].invoke_kind, ComInvokeKind::PropertyPut);
        assert!(idl.contains("interface ICounter : IDispatch"));
        assert!(idl.contains("HRESULT value([out, retval] long* result);"));
        assert!(idl.contains("HRESULT value([in] long newValue);"));
        let returner = descriptor
            .classes
            .iter()
            .find(|class| class.class_name == "Returner")
            .expect("Returner descriptor");
        assert_eq!(returner.members.len(), 2);
        assert_eq!(returner.members[0].vtable_slot, Some(7));
        assert_eq!(returner.members[0].return_type, Some(ComParamType::Object));
        assert_eq!(returner.members[1].vtable_slot, Some(8));
        assert_eq!(returner.members[1].return_type, Some(ComParamType::Long));
        assert!(idl.contains("interface IReturner : IDispatch"));
        assert!(idl.contains("HRESULT ReturnSelf([out, retval] IDispatch** result);"));
        assert!(idl.contains("HRESULT Ping([out, retval] long* result);"));

        let shim_source =
            std::fs::read_to_string(&output.shim_source_path).expect("shim source should exist");
        assert!(shim_source.contains("DllGetClassObject"));
        assert!(shim_source.contains("DllRegisterServer"));
        assert!(shim_source.contains("MS-OAUT"));
        assert!(shim_source.contains("BoundedDualInterface"));
        assert!(!output.dll_target_path.exists());
        assert!(!output.tlb_target_path.exists());
    }

    #[test]
    fn wrapped_com_server_rejects_non_comserver_output_type() {
        let temp = TestDir::new("wrapped_com_server_rejects_non_comserver_output_type");
        let project_path = temp.path.join("Demo.basproj");
        let class_path = temp.path.join("Calculator.cls");

        write(&class_path, "Public Sub Ping()\nEnd Sub\n");
        write(
            &project_path,
            r#"<Project Sdk="OxVba.Sdk/0.1.0">
  <PropertyGroup>
    <OutputType>Library</OutputType>
    <BuildTarget>WrappedComServer</BuildTarget>
    <ProjectName>DemoServer</ProjectName>
  </PropertyGroup>
  <ItemGroup>
    <ClassModule Include="Calculator.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
    </ClassModule>
  </ItemGroup>
</Project>
"#,
        );

        let err = build_wrapped_com_server(&WrappedComServerBuildOptions {
            project_path,
            out_dir: temp.path.join("out"),
            compile_dll: false,
        })
        .expect_err("invalid OutputType should be rejected");
        assert!(matches!(err, BuildError::InvalidOutputType { .. }));
    }

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let unique = format!(
                "oxvba_build_{name}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be after unix epoch")
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(&path).expect("create test dir");
            Self { path }
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn write(path: &Path, text: &str) {
        std::fs::write(path, text).expect("write test fixture");
    }
}
