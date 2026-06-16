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

    let dll_path = output
        .dll_target_path
        .display()
        .to_string()
        .replace('\'', "''");
    let script = format!(
        r#"
$dll = '{}'
$register = Start-Process -FilePath regsvr32.exe -ArgumentList @('/s', $dll) -Wait -PassThru -WindowStyle Hidden
if ($register.ExitCode -ne 0) {{ throw "regsvr32 register failed with exit code $($register.ExitCode)" }}
try {{
    $obj = New-Object -ComObject DemoServer.Calculator
    $result = $obj.Add(2, 3)
    if ($result -ne 5) {{ throw "expected Add(2,3)=5, got $result" }}
}} finally {{
    Start-Process -FilePath regsvr32.exe -ArgumentList @('/u', '/s', $dll) -Wait -PassThru -WindowStyle Hidden | Out-Null
}}
"#,
        dll_path
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
