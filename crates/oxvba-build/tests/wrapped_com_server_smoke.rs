#![cfg(target_os = "windows")]

use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
#[ignore = "builds/registers an in-process COM DLL; run manually on Windows"]
fn wrapped_com_server_dll_registers_and_dispatches_late_bound() {
    let temp = TestDir::new("wrapped_com_server_dll_registers_and_dispatches_late_bound");
    let project_path = temp.path.join("Demo.basproj");
    let class_path = temp.path.join("Calculator.cls");
    let out_dir = temp.path.join("out");

    write(
        &class_path,
        r#"
Public Function Add(ByVal a As Long, ByVal b As Long) As Long
    Add = a + b
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
    </ClassModule>
  </ItemGroup>
</Project>
"#,
    );

    let output =
        oxvba_build::build_wrapped_com_server(&oxvba_build::WrappedComServerBuildOptions {
            project_path,
            out_dir,
            compile_dll: true,
        })
        .expect("WrappedComServer build should compile a DLL");
    assert!(output.dll_target_path.exists());
    assert!(output.tlb_target_path.exists());

    let descriptor_text =
        std::fs::read_to_string(&output.descriptor_path).expect("descriptor should exist");
    let descriptor: oxvba_build::ComServerDescriptor =
        serde_json::from_str(&descriptor_text).expect("descriptor should parse");
    let class = descriptor
        .classes
        .iter()
        .find(|class| class.class_name == "Calculator")
        .expect("Calculator descriptor");

    let dll_path = output
        .dll_target_path
        .display()
        .to_string()
        .replace('\'', "''");
    let tlb_path = output
        .tlb_target_path
        .display()
        .to_string()
        .replace('\'', "''");
    let libid = descriptor.libid.replace('\'', "''");
    let clsid = class.clsid.replace('\'', "''");
    let version = format!("{}.{}", descriptor.version_major, descriptor.version_minor);
    let script = format!(
        r#"
$dll = '{}'
$tlb = '{}'
$libid = '{}'
$clsid = '{}'
$version = '{}'
$register = Start-Process -FilePath regsvr32.exe -ArgumentList @('/s', $dll) -Wait -PassThru -WindowStyle Hidden
if ($register.ExitCode -ne 0) {{ throw "regsvr32 register failed with exit code $($register.ExitCode)" }}
try {{
    $wsh = New-Object -ComObject WScript.Shell
    $classTypeLib = $wsh.RegRead("HKCU\Software\Classes\CLSID\{{$clsid}}\TypeLib\")
    if ($classTypeLib -ne "{{$libid}}") {{ throw "expected CLSID TypeLib {{$libid}}, got $classTypeLib" }}
    $registeredTlb = $wsh.RegRead("HKCU\Software\Classes\TypeLib\{{$libid}}\$version\0\win64\")
    if ($registeredTlb -ne $tlb) {{ throw "expected registered TLB $tlb, got $registeredTlb" }}
    $obj = New-Object -ComObject DemoServer.Calculator
    $result = $obj.Add(2, 3)
    if ($result -ne 5) {{ throw "expected Add(2,3)=5, got $result" }}
}} finally {{
    Start-Process -FilePath regsvr32.exe -ArgumentList @('/u', '/s', $dll) -Wait -PassThru -WindowStyle Hidden | Out-Null
}}
"#,
        dll_path, tlb_path, libid, clsid, version
    );
    let status = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script)
        .status()
        .expect("PowerShell should run COM smoke script");
    assert!(status.success(), "COM smoke script failed: {status:?}");
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
