//! Clean build target orchestration for OxVBA wrapper outputs.
//!
//! `WrappedComServer` builds are intentionally package-first: the compiler emits
//! the canonical bundle package, derives COM descriptors from the symbol export
//! surface, then wrapper-specific tooling consumes those artifacts.

mod com_descriptor;
mod compile;
mod identity;
mod idl;
#[cfg(target_os = "windows")]
mod oleaut_typelib;

use std::fs;
use std::path::{Path, PathBuf};

pub use com_descriptor::{
    ComClassDescriptor, ComEventDescriptor, ComImplementedInterfaceDescriptor,
    ComImplementedInterfaceMethodDescriptor, ComInvokeKind, ComMemberDescriptor,
    ComNativeInterfaceRequirement, ComParamType, ComServerDescriptor, ComWireType,
};
pub use compile::{
    ShimCompileError, compile_shim_dll, compile_typelib, copy_prebuilt_comhost_dll,
    find_prebuilt_comhost_dll,
};
pub use identity::deterministic_uuid;
pub use idl::generate_idl;
#[cfg(target_os = "windows")]
pub use oleaut_typelib::emit_typelib_with_oleaut;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedComServerBuildOptions {
    pub project_path: PathBuf,
    pub out_dir: PathBuf,
    pub compile_dll: bool,
    pub comhost_dll_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedComServerBuildOutput {
    pub oxb_path: PathBuf,
    pub descriptor_path: PathBuf,
    pub idl_path: PathBuf,
    pub dll_target_path: PathBuf,
    pub comhost_source_path: Option<PathBuf>,
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
    #[error("class `{class_name}` implements `{interface}` but does not implement `{method_name}`")]
    MissingImplementedInterfaceMember {
        class_name: String,
        interface: String,
        method_name: String,
    },
    #[error(
        "class `{class_name}` implements `{interface}` but `{method_name}` has {found_params} parameter(s); expected {expected_params}"
    )]
    InvalidImplementedInterfaceMemberArity {
        class_name: String,
        interface: String,
        method_name: String,
        expected_params: usize,
        found_params: usize,
    },
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

    let dll_target_path = options.out_dir.join(format!("{artifact_stem}.dll"));
    let mut comhost_source_path = options.comhost_dll_path.clone();
    if options.compile_dll {
        compile_typelib(&idl_path, &tlb_target_path)?;
        let copied_from =
            copy_prebuilt_comhost_dll(options.comhost_dll_path.as_deref(), &dll_target_path)?;
        comhost_source_path = Some(copied_from);
    }

    Ok(WrappedComServerBuildOutput {
        oxb_path,
        descriptor_path,
        idl_path,
        dll_target_path,
        comhost_source_path,
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
    let descriptor =
        ComServerDescriptor::from_surface_with_references(surface, &root_manifest.references);
    validate_implemented_interfaces(&descriptor, &env)?;
    Ok(descriptor)
}

fn validate_implemented_interfaces(
    descriptor: &ComServerDescriptor,
    env: &oxvba_symbol::provider::ResolutionEnvironment,
) -> Result<(), BuildError> {
    use oxvba_symbol::model::{SymbolImpl, SymbolNamespace};

    for class in &descriptor.classes {
        let Some(class_scope) = env.module_scope(&class.class_name) else {
            continue;
        };
        for interface in &class.implemented_interfaces {
            for method in &interface.methods {
                let symbol = match env.symbols.find_in_scope(
                    class_scope,
                    SymbolNamespace::Procedure,
                    &method.vba_name,
                ) {
                    Ok(Some(symbol)) => symbol,
                    _ => {
                        return Err(BuildError::MissingImplementedInterfaceMember {
                            class_name: class.class_name.clone(),
                            interface: interface.name.clone(),
                            method_name: method.vba_name.clone(),
                        });
                    }
                };
                let found_params =
                    env.symbols
                        .symbol(symbol)
                        .and_then(|symbol| match &symbol.imp {
                            SymbolImpl::Signature(signature_id) => env
                                .signatures
                                .get(*signature_id)
                                .map(|signature| signature.params.len()),
                            SymbolImpl::Property(group) => {
                                let signature_id = match method.invoke_kind {
                                    ComInvokeKind::PropertyGet => group.get,
                                    ComInvokeKind::PropertyPut => group.let_,
                                    ComInvokeKind::PropertyPutRef => group.set,
                                    ComInvokeKind::Method => None,
                                }?;
                                env.signatures
                                    .get(signature_id)
                                    .map(|signature| signature.params.len())
                            }
                            _ => None,
                        });
                let Some(found_params) = found_params else {
                    return Err(BuildError::MissingImplementedInterfaceMember {
                        class_name: class.class_name.clone(),
                        interface: interface.name.clone(),
                        method_name: method.vba_name.clone(),
                    });
                };
                if found_params != method.parameter_types.len() {
                    return Err(BuildError::InvalidImplementedInterfaceMemberArity {
                        class_name: class.class_name.clone(),
                        interface: interface.name.clone(),
                        method_name: method.vba_name.clone(),
                        expected_params: method.parameter_types.len(),
                        found_params,
                    });
                }
            }
        }
    }
    Ok(())
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
        let object_relay_path = temp.path.join("ObjectRelay.cls");
        let addin_path = temp.path.join("OfficeAddin.cls");
        let ribbon_path = temp.path.join("RibbonAddin.cls");
        let rtd_path = temp.path.join("RtdTicker.cls");
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
            &object_relay_path,
            r#"
Public Function Ping() As Long
    Ping = 42
End Function

Public Function EchoPing(ByVal other As Object) As Long
    EchoPing = other.Ping()
End Function
"#,
        );
        write(
            &addin_path,
            r#"
Implements AddInDesignerObjects.IDTExtensibility2

Private Sub IDTExtensibility2_OnConnection(ByVal Application As Variant, ByVal ConnectMode As Long, ByVal AddInInst As Variant, ByVal custom As Variant)
End Sub

Private Sub IDTExtensibility2_OnDisconnection(ByVal RemoveMode As Long, ByVal custom As Variant)
End Sub

Private Sub IDTExtensibility2_OnAddInsUpdate(ByVal custom As Variant)
End Sub

Private Sub IDTExtensibility2_OnStartupComplete(ByVal custom As Variant)
End Sub

Private Sub IDTExtensibility2_OnBeginShutdown(ByVal custom As Variant)
End Sub
"#,
        );
        write(
            &ribbon_path,
            r#"
Implements Office.IRibbonExtensibility

Private Function IRibbonExtensibility_GetCustomUI(ByVal RibbonID As String) As String
    IRibbonExtensibility_GetCustomUI = "<customUI xmlns=""http://schemas.microsoft.com/office/2009/07/customui""></customUI>"
End Function
"#,
        );
        write(
            &rtd_path,
            r#"
Implements Excel.IRtdServer

Private Function IRtdServer_ServerStart(ByVal CallbackObject As Variant) As Long
    IRtdServer_ServerStart = 1
End Function

Private Function IRtdServer_ConnectData(ByVal TopicID As Long, ByVal Strings As Variant, ByVal GetNewValues As Boolean) As Variant
    IRtdServer_ConnectData = TopicID + 100
End Function

Private Function IRtdServer_RefreshData(ByVal TopicCount As Long) As Variant
    IRtdServer_RefreshData = Array(7, "value")
End Function

Private Sub IRtdServer_DisconnectData(ByVal TopicID As Long)
End Sub

Private Function IRtdServer_Heartbeat() As Long
    IRtdServer_Heartbeat = 1
End Function

Private Sub IRtdServer_ServerTerminate()
End Sub
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
    <COMReference Include="Excel">
      <Guid>{00020813-0000-0000-C000-000000000046}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>9</VersionMinor>
      <Lcid>0</Lcid>
    </COMReference>
    <COMReference Include="AddInDesignerObjects">
      <Guid>{AC0714F2-3D04-11D1-AE7D-00A0C90F26F4}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>0</VersionMinor>
      <Lcid>0</Lcid>
    </COMReference>
    <COMReference Include="Office">
      <Guid>{2DF8D04C-5BFA-101B-BDE5-00AA0044DE52}</Guid>
      <VersionMajor>2</VersionMajor>
      <VersionMinor>8</VersionMinor>
      <Lcid>0</Lcid>
    </COMReference>
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
    <ClassModule Include="ObjectRelay.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.ObjectRelay</ProgId>
      <Description>ObjectRelay class</Description>
    </ClassModule>
    <ClassModule Include="OfficeAddin.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.OfficeAddin</ProgId>
      <Description>Office add-in class</Description>
    </ClassModule>
    <ClassModule Include="RibbonAddin.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.RibbonAddin</ProgId>
      <Description>Ribbon add-in class</Description>
    </ClassModule>
    <ClassModule Include="RtdTicker.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.RtdTicker</ProgId>
      <Description>RTD ticker class</Description>
    </ClassModule>
  </ItemGroup>
</Project>
"#,
        );

        let output = build_wrapped_com_server(&WrappedComServerBuildOptions {
            project_path,
            out_dir,
            compile_dll: false,
            comhost_dll_path: None,
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
        let object_relay = descriptor
            .classes
            .iter()
            .find(|class| class.class_name == "ObjectRelay")
            .expect("ObjectRelay descriptor");
        assert_eq!(object_relay.members.len(), 2);
        assert_eq!(object_relay.members[0].vtable_slot, Some(7));
        assert_eq!(
            object_relay.members[0].return_type,
            Some(ComParamType::Long)
        );
        assert_eq!(object_relay.members[1].vtable_slot, Some(8));
        assert_eq!(
            object_relay.members[1].return_type,
            Some(ComParamType::Long)
        );
        assert_eq!(
            object_relay.members[1].parameter_types.as_slice(),
            [ComParamType::Object]
        );
        assert!(idl.contains("interface IObjectRelay : IDispatch"));
        assert!(
            idl.contains("HRESULT EchoPing([in] IDispatch* other, [out, retval] long* result);")
        );
        let addin = descriptor
            .classes
            .iter()
            .find(|class| class.class_name == "OfficeAddin")
            .expect("OfficeAddin descriptor");
        assert_eq!(addin.implemented_interfaces.len(), 1);
        assert_eq!(addin.implemented_interfaces[0].name, "IDTExtensibility2");
        assert_eq!(addin.implemented_interfaces[0].methods.len(), 5);
        let on_connection = addin.implemented_interfaces[0]
            .methods
            .iter()
            .find(|method| method.name == "OnConnection")
            .expect("IDTExtensibility2.OnConnection metadata");
        assert_eq!(
            on_connection.parameter_wire_types[3],
            ComWireType::SafeArrayVariant
        );
        assert_eq!(
            addin.implemented_interfaces[0].native_requirements(),
            vec![
                ComNativeInterfaceRequirement::AutomationValue,
                ComNativeInterfaceRequirement::TypedInterfacePointer,
                ComNativeInterfaceRequirement::SafeArrayVariant,
            ]
        );
        let ribbon = descriptor
            .classes
            .iter()
            .find(|class| class.class_name == "RibbonAddin")
            .expect("RibbonAddin descriptor");
        assert_eq!(ribbon.implemented_interfaces.len(), 1);
        assert_eq!(
            ribbon.implemented_interfaces[0].name,
            "IRibbonExtensibility"
        );
        assert_eq!(ribbon.implemented_interfaces[0].methods.len(), 1);
        let get_custom_ui = ribbon.implemented_interfaces[0]
            .methods
            .iter()
            .find(|method| method.name == "GetCustomUI")
            .expect("IRibbonExtensibility.GetCustomUI metadata");
        assert_eq!(get_custom_ui.vtable_slot, Some(7));
        assert_eq!(
            get_custom_ui.parameter_wire_types.as_slice(),
            [ComWireType::Automation(ComParamType::String)]
        );
        assert_eq!(
            get_custom_ui.return_wire_type,
            Some(ComWireType::Automation(ComParamType::String))
        );
        assert_eq!(
            ribbon.implemented_interfaces[0].native_requirements(),
            vec![ComNativeInterfaceRequirement::AutomationValue]
        );
        let rtd = descriptor
            .classes
            .iter()
            .find(|class| class.class_name == "RtdTicker")
            .expect("RtdTicker descriptor");
        assert_eq!(rtd.implemented_interfaces.len(), 1);
        assert_eq!(rtd.implemented_interfaces[0].name, "IRtdServer");
        assert_eq!(rtd.implemented_interfaces[0].methods.len(), 6);
        let server_start = rtd.implemented_interfaces[0]
            .methods
            .iter()
            .find(|method| method.name == "ServerStart")
            .expect("IRtdServer.ServerStart metadata");
        assert!(matches!(
            &server_start.parameter_wire_types[0],
            ComWireType::InterfacePointer { iid, .. } if !iid.trim().is_empty()
        ));
        let connect_data = rtd.implemented_interfaces[0]
            .methods
            .iter()
            .find(|method| method.name == "ConnectData")
            .expect("IRtdServer.ConnectData metadata");
        assert_eq!(
            connect_data.parameter_wire_types.as_slice(),
            [
                ComWireType::Automation(ComParamType::Long),
                ComWireType::SafeArrayVariant,
                ComWireType::Automation(ComParamType::ByRefBoolean),
            ]
        );
        let refresh_data = rtd.implemented_interfaces[0]
            .methods
            .iter()
            .find(|method| method.name == "RefreshData")
            .expect("IRtdServer.RefreshData metadata");
        assert_eq!(
            refresh_data.return_wire_type,
            Some(ComWireType::ByRefSafeArrayVariant)
        );
        assert_eq!(
            rtd.implemented_interfaces[0].native_requirements(),
            vec![
                ComNativeInterfaceRequirement::AutomationValue,
                ComNativeInterfaceRequirement::ScalarByRefWriteback,
                ComNativeInterfaceRequirement::TypedInterfacePointer,
                ComNativeInterfaceRequirement::SafeArrayVariant,
                ComNativeInterfaceRequirement::SafeArrayByRefWriteback,
            ]
        );
        assert!(idl.contains("interface IDTExtensibility2 : IDispatch"));
        assert!(idl.contains("interface IRibbonExtensibility : IDispatch"));
        assert!(idl.contains("interface IRtdServer : IDispatch"));
        assert!(idl.contains("interface IDTExtensibility2;"));
        assert!(idl.contains("interface IRibbonExtensibility;"));
        assert!(idl.contains("interface IRtdServer;"));

        assert!(!output.dll_target_path.exists());
        assert!(!output.tlb_target_path.exists());
    }

    #[test]
    fn wrapped_com_server_rejects_incomplete_office_profile_implementation() {
        let temp =
            TestDir::new("wrapped_com_server_rejects_incomplete_office_profile_implementation");
        let project_path = temp.path.join("Demo.basproj");
        let class_path = temp.path.join("OfficeAddin.cls");

        write(
            &class_path,
            r#"
Implements AddInDesignerObjects.IDTExtensibility2

Private Sub IDTExtensibility2_OnConnection(ByVal Application As Variant, ByVal ConnectMode As Long, ByVal AddInInst As Variant, ByVal custom As Variant)
End Sub
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
    <COMReference Include="AddInDesignerObjects">
      <Guid>{AC0714F2-3D04-11D1-AE7D-00A0C90F26F4}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>0</VersionMinor>
      <Lcid>0</Lcid>
    </COMReference>
    <ClassModule Include="OfficeAddin.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.OfficeAddin</ProgId>
    </ClassModule>
  </ItemGroup>
</Project>
"#,
        );

        let err = build_wrapped_com_server(&WrappedComServerBuildOptions {
            project_path,
            out_dir: temp.path.join("out"),
            compile_dll: false,
            comhost_dll_path: None,
        })
        .expect_err("incomplete IDTExtensibility2 implementation should be rejected");
        match err {
            BuildError::MissingImplementedInterfaceMember {
                class_name,
                interface,
                method_name,
            } => {
                assert_eq!(class_name, "OfficeAddin");
                assert_eq!(interface, "IDTExtensibility2");
                assert_eq!(method_name, "IDTExtensibility2_OnDisconnection");
            }
            other => panic!("expected missing implemented-interface member, got {other:?}"),
        }
    }

    #[test]
    fn wrapped_com_server_rejects_wrong_office_profile_method_arity() {
        let temp = TestDir::new("wrapped_com_server_rejects_wrong_office_profile_method_arity");
        let project_path = temp.path.join("Demo.basproj");
        let class_path = temp.path.join("OfficeAddin.cls");

        write(
            &class_path,
            r#"
Implements AddInDesignerObjects.IDTExtensibility2

Private Sub IDTExtensibility2_OnConnection(ByVal Application As Variant, ByVal ConnectMode As Long, ByVal AddInInst As Variant, ByVal custom As Variant)
End Sub

Private Sub IDTExtensibility2_OnDisconnection(ByVal RemoveMode As Long, ByVal custom As Variant)
End Sub

Private Sub IDTExtensibility2_OnAddInsUpdate()
End Sub

Private Sub IDTExtensibility2_OnStartupComplete(ByVal custom As Variant)
End Sub

Private Sub IDTExtensibility2_OnBeginShutdown(ByVal custom As Variant)
End Sub
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
    <COMReference Include="AddInDesignerObjects">
      <Guid>{AC0714F2-3D04-11D1-AE7D-00A0C90F26F4}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>0</VersionMinor>
      <Lcid>0</Lcid>
    </COMReference>
    <ClassModule Include="OfficeAddin.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.OfficeAddin</ProgId>
    </ClassModule>
  </ItemGroup>
</Project>
"#,
        );

        let err = build_wrapped_com_server(&WrappedComServerBuildOptions {
            project_path,
            out_dir: temp.path.join("out"),
            compile_dll: false,
            comhost_dll_path: None,
        })
        .expect_err("wrong IDTExtensibility2 method arity should be rejected");
        match err {
            BuildError::InvalidImplementedInterfaceMemberArity {
                class_name,
                interface,
                method_name,
                expected_params,
                found_params,
            } => {
                assert_eq!(class_name, "OfficeAddin");
                assert_eq!(interface, "IDTExtensibility2");
                assert_eq!(method_name, "IDTExtensibility2_OnAddInsUpdate");
                assert_eq!(expected_params, 1);
                assert_eq!(found_params, 0);
            }
            other => panic!("expected invalid implemented-interface arity, got {other:?}"),
        }
    }

    #[test]
    #[ignore = "requires the registered Excel type library"]
    fn wrapped_com_server_resolves_imported_com_interface_from_typelib() {
        let temp = TestDir::new("wrapped_com_server_resolves_imported_com_interface_from_typelib");
        let project_path = temp.path.join("Demo.basproj");
        let class_path = temp.path.join("UpdateEventSink.cls");

        write(
            &class_path,
            r#"
Implements IRTDUpdateEvent

Private Sub IRTDUpdateEvent_UpdateNotify()
End Sub

Private Property Get IRTDUpdateEvent_HeartbeatInterval() As Long
    IRTDUpdateEvent_HeartbeatInterval = 1000
End Property

Private Property Let IRTDUpdateEvent_HeartbeatInterval(ByVal value As Long)
End Property

Private Sub IRTDUpdateEvent_Disconnect()
End Sub
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
    <COMReference Include="Excel">
      <Guid>{00020813-0000-0000-C000-000000000046}</Guid>
      <VersionMajor>1</VersionMajor>
      <VersionMinor>9</VersionMinor>
      <Lcid>0</Lcid>
    </COMReference>
    <ClassModule Include="UpdateEventSink.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.UpdateEventSink</ProgId>
    </ClassModule>
  </ItemGroup>
</Project>
"#,
        );

        let output = build_wrapped_com_server(&WrappedComServerBuildOptions {
            project_path,
            out_dir: temp.path.join("out"),
            compile_dll: false,
            comhost_dll_path: None,
        })
        .expect("WrappedComServer build should resolve imported COM interface");
        let descriptor_text =
            std::fs::read_to_string(&output.descriptor_path).expect("descriptor should exist");
        let descriptor: ComServerDescriptor =
            serde_json::from_str(&descriptor_text).expect("descriptor should parse");
        let sink = descriptor
            .classes
            .iter()
            .find(|class| class.class_name == "UpdateEventSink")
            .expect("UpdateEventSink descriptor");
        assert_eq!(sink.implemented_interfaces.len(), 1);
        let interface = &sink.implemented_interfaces[0];
        assert_eq!(interface.name, "IRTDUpdateEvent");
        assert_eq!(interface.iid, "A43788C1-D91B-11D3-8F39-00C04F3651B8");
        assert!(interface.methods.iter().any(|method| {
            method.name == "HeartbeatInterval"
                && method.invoke_kind == ComInvokeKind::PropertyGet
                && method.vba_name == "IRTDUpdateEvent_HeartbeatInterval"
        }));
        assert!(interface.methods.iter().any(|method| {
            method.name == "HeartbeatInterval"
                && method.invoke_kind == ComInvokeKind::PropertyPut
                && method.vba_name == "IRTDUpdateEvent_HeartbeatInterval"
        }));
        assert_eq!(
            interface
                .methods
                .iter()
                .filter(|method| method.vtable_slot.is_some())
                .count(),
            3
        );
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
            comhost_dll_path: None,
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
