#![cfg(target_os = "windows")]
#![allow(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering, fence};
use std::sync::{Arc, Mutex};

use windows_sys::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CLSCTX_LOCAL_SERVER, CLSIDFromProgID, COINIT_APARTMENTTHREADED,
    CoCreateInstance, CoInitializeEx, CoUninitialize, DISPATCH_METHOD, DISPATCH_PROPERTYGET,
    DISPATCH_PROPERTYPUT, SAFEARRAY, TYPEATTR,
};
use windows_sys::Win32::System::Ole::SafeArrayDestroy;
use windows_sys::Win32::System::Variant::{VARIANT, VT_DISPATCH, VT_I4, VT_R8, VariantClear};
use windows_sys::core::GUID;

type SeenEvents = Arc<Mutex<Vec<(i32, Option<i32>)>>>;

const IID_IDTEXTENSIBILITY2: GUID = GUID {
    data1: 0xB65A_D801,
    data2: 0xABAF,
    data3: 0x11D0,
    data4: [0xBB, 0x8B, 0x00, 0xA0, 0xC9, 0x0F, 0x27, 0x44],
};
const IID_IRTDSERVER: GUID = GUID {
    data1: 0xEC0E_6191,
    data2: 0xDB51,
    data3: 0x11D3,
    data4: [0x8F, 0x3E, 0x00, 0xC0, 0x4F, 0x36, 0x51, 0xB8],
};

#[test]
#[ignore = "builds/registers an in-process COM DLL; run manually on Windows"]
fn wrapped_com_server_dll_registers_and_dispatches_late_bound() {
    let temp = TestDir::new("wrapped_com_server_dll_registers_and_dispatches_late_bound");
    let project_path = temp.path.join("Demo.basproj");
    let class_path = temp.path.join("Calculator.cls");
    let pinger_path = temp.path.join("Pinger.cls");
    let counter_path = temp.path.join("Counter.cls");
    let returner_path = temp.path.join("Returner.cls");
    let object_relay_path = temp.path.join("ObjectRelay.cls");
    let addin_path = temp.path.join("OfficeAddin.cls");
    let rtd_path = temp.path.join("RtdTicker.cls");
    let out_dir = temp.path.join("out");

    write(
        &class_path,
        r#"
Public Event Changed(ByVal value As Long)
Private mValue As Long

Public Function Add(ByVal a As Long, ByVal b As Long) As Long
    Add = a + b
End Function

Public Property Get Value() As Long
    Value = mValue
End Property

Public Property Let Value(ByVal newValue As Long)
    mValue = newValue
End Property

Public Function ReturnSelf() As Object
    Set ReturnSelf = Me
End Function

Public Function Numbers() As Variant
    Dim values(0 To 1) As Long
    values(0) = 7
    values(1) = 8
    Numbers = values
End Function

Public Function Boom() As Long
    Err.Raise 5, , "boom"
End Function

Public Sub FireChanged(ByVal value As Long)
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
Implements IDTExtensibility2

Private mApplicationNameLength As Long

Private Sub IDTExtensibility2_OnConnection(ByVal HostApplication As Excel.Application, ByVal ConnectMode As Long, ByVal AddInInst As Variant, ByVal custom As Variant)
    mApplicationNameLength = Len(HostApplication.Name)
End Sub

Private Sub IDTExtensibility2_OnDisconnection(ByVal RemoveMode As Long, ByVal custom As Variant)
End Sub

Private Sub IDTExtensibility2_OnAddInsUpdate(ByVal custom As Variant)
End Sub

Private Sub IDTExtensibility2_OnStartupComplete(ByVal custom As Variant)
End Sub

Private Sub IDTExtensibility2_OnBeginShutdown(ByVal custom As Variant)
End Sub

Public Function ApplicationNameLength() As Long
    ApplicationNameLength = mApplicationNameLength
End Function
"#,
    );
    write(
        &rtd_path,
        r#"
Implements IRtdServer

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
    <ClassModule Include="Calculator.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.Calculator</ProgId>
    </ClassModule>
    <ClassModule Include="Pinger.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.Pinger</ProgId>
    </ClassModule>
    <ClassModule Include="Counter.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.Counter</ProgId>
    </ClassModule>
    <ClassModule Include="Returner.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.Returner</ProgId>
    </ClassModule>
    <ClassModule Include="ObjectRelay.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.ObjectRelay</ProgId>
    </ClassModule>
    <ClassModule Include="OfficeAddin.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.OfficeAddin</ProgId>
    </ClassModule>
    <ClassModule Include="RtdTicker.cls">
      <VBExposed>True</VBExposed>
      <VBCreatable>True</VBCreatable>
      <Instancing>MultiUse</Instancing>
      <ProgId>DemoServer.RtdTicker</ProgId>
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
    let pinger_class = descriptor
        .classes
        .iter()
        .find(|class| class.class_name == "Pinger")
        .expect("Pinger descriptor");
    let counter_class = descriptor
        .classes
        .iter()
        .find(|class| class.class_name == "Counter")
        .expect("Counter descriptor");
    let returner_class = descriptor
        .classes
        .iter()
        .find(|class| class.class_name == "Returner")
        .expect("Returner descriptor");
    let object_relay_class = descriptor
        .classes
        .iter()
        .find(|class| class.class_name == "ObjectRelay")
        .expect("ObjectRelay descriptor");
    let addin_class = descriptor
        .classes
        .iter()
        .find(|class| class.class_name == "OfficeAddin")
        .expect("OfficeAddin descriptor");
    let rtd_class = descriptor
        .classes
        .iter()
        .find(|class| class.class_name == "RtdTicker")
        .expect("RtdTicker descriptor");

    let registration = RegisteredDll::register(&output.dll_target_path);
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
$tlb = '{}'
$libid = '{}'
$clsid = '{}'
$version = '{}'
$wsh = New-Object -ComObject WScript.Shell
$classTypeLib = $wsh.RegRead("HKCU\Software\Classes\CLSID\{{$clsid}}\TypeLib\")
if ($classTypeLib -ne "{{$libid}}") {{ throw "expected CLSID TypeLib {{$libid}}, got $classTypeLib" }}
$registeredTlb = $wsh.RegRead("HKCU\Software\Classes\TypeLib\{{$libid}}\$version\0\win64\")
if ($registeredTlb -ne $tlb) {{ throw "expected registered TLB $tlb, got $registeredTlb" }}
$obj = New-Object -ComObject DemoServer.Calculator
$result = $obj.Add(2, 3)
if ($result -ne 5) {{ throw "expected Add(2,3)=5, got $result" }}
$pinger = New-Object -ComObject DemoServer.Pinger
$ping = $pinger.Ping()
if ($ping -ne 42) {{ throw "expected Ping()=42, got $ping" }}
$pair = $pinger.AddPair(19, 23)
if ($pair -ne 42) {{ throw "expected AddPair(19,23)=42, got $pair" }}
$average = $pinger.Average(10.5, 21.5)
if ([Math]::Abs($average - 16.0) -gt 0.0000001) {{ throw "expected Average(10.5,21.5)=16, got $average" }}
$counter = New-Object -ComObject DemoServer.Counter
$counter.Value = 314
if ($counter.Value -ne 314) {{ throw "expected Counter.Value=314, got $($counter.Value)" }}
$returner = New-Object -ComObject DemoServer.Returner
$returned = $returner.ReturnSelf()
$returnedPing = $returned.Ping()
if ($returnedPing -ne 42) {{ throw "expected Returner.ReturnSelf().Ping()=42, got $returnedPing" }}
$relay = New-Object -ComObject DemoServer.ObjectRelay
$relayEcho = $relay.EchoPing($relay)
if ($relayEcho -ne 42) {{ throw "expected ObjectRelay.EchoPing(ObjectRelay)=42, got $relayEcho" }}
"#,
        tlb_path, libid, clsid, version
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

    // SAFETY: the smoke owns the registered in-process COM object lifetime in this
    // STA and releases every interface pointer obtained by the raw COM helper.
    unsafe { controlled_connection_point_smoke(&descriptor, class) };
    // SAFETY: this raw COM helper creates the same wrapped object through COM, then
    // releases the dispatch and custom interface pointers it obtains.
    unsafe { controlled_dual_vtable_smoke(pinger_class) };
    // SAFETY: this raw COM helper creates and releases the Counter dispatch and
    // default-interface pointers it obtains.
    unsafe { controlled_property_vtable_smoke(counter_class) };
    // SAFETY: this raw COM helper creates and releases the Returner dispatch,
    // default-interface, and returned dispatch pointers it obtains.
    unsafe { controlled_object_return_vtable_smoke(returner_class) };
    // SAFETY: this raw COM helper creates and releases the ObjectRelay dispatch
    // and default-interface pointers it obtains.
    unsafe { controlled_object_argument_vtable_smoke(object_relay_class) };
    // SAFETY: this raw COM helper creates and releases the Office profile
    // interface pointers and returned RTD SAFEARRAY it obtains.
    unsafe { controlled_office_profile_vtable_smoke(addin_class, rtd_class) };
    excel_vba_early_bound_and_connection_point_smoke(&temp.path, &output.tlb_target_path);
    drop(registration);
}

fn excel_vba_early_bound_and_connection_point_smoke(work_dir: &Path, tlb_path: &Path) {
    let class_source_path = work_dir.join("EventSink.cls");
    let module_source_path = work_dir.join("ExcelClient.bas");
    let script_path = work_dir.join("excel_client_smoke.ps1");
    write(
        &class_source_path,
        r#"
Public WithEvents Calc As Calculator
Public LastValue As Long

Private Sub Calc_Changed(ByVal arg0 As Variant)
    LastValue = CLng(arg0)
End Sub
"#,
    );
    write(
        &module_source_path,
        r#"
Public Function RunOxVbaWrappedComServerSmoke() As String
    On Error GoTo Fail
    Dim sink As EventSink
    Dim calc As Calculator
    Dim returned As Object
    Dim values As Variant
    Dim ignored As Long
    Dim pinger As Pinger
    Dim counter As Counter
    Dim returner As Returner
    Dim returnedFromReturner As Object
    Dim relay As ObjectRelay

    Set sink = New EventSink
    Set calc = New Calculator
    Set pinger = New Pinger
    Set counter = New Counter
    Set returner = New Returner
    Set relay = New ObjectRelay
    Set sink.Calc = calc

    If sink.Calc.Add(20, 22) <> 42 Then
        Err.Raise 5, , "Add returned wrong value"
    End If
    calc.Value = 123
    If calc.Value <> 123 Then
        Err.Raise 5, , "Value property returned wrong value"
    End If
    Set returned = calc.ReturnSelf()
    If returned.Add(3, 4) <> 7 Then
        Err.Raise 5, , "ReturnSelf returned wrong object"
    End If
    values = calc.Numbers()
    If values(0) <> 7 Or values(1) <> 8 Then
        Err.Raise 5, , "Numbers returned wrong array"
    End If
    If pinger.Ping() <> 42 Then
        Err.Raise 5, , "Pinger.Ping returned wrong value"
    End If
    If pinger.AddPair(19, 23) <> 42 Then
        Err.Raise 5, , "Pinger.AddPair returned wrong value"
    End If
    If Abs(pinger.Average(10.5, 21.5) - 16#) > 0.0000001 Then
        Err.Raise 5, , "Pinger.Average returned wrong value"
    End If
    counter.Value = 271
    If counter.Value <> 271 Then
        Err.Raise 5, , "Counter.Value returned wrong value"
    End If
    Set returnedFromReturner = returner.ReturnSelf()
    If returnedFromReturner.Ping() <> 42 Then
        Err.Raise 5, , "Returner.ReturnSelf returned wrong object"
    End If
    If relay.EchoPing(relay) <> 42 Then
        Err.Raise 5, , "ObjectRelay.EchoPing returned wrong value"
    End If
    On Error Resume Next
    ignored = calc.Boom()
    If Err.Number <> 440 Then
        RunOxVbaWrappedComServerSmoke = "ERR:expected Boom Automation error 440, got " & CStr(Err.Number)
        Exit Function
    End If
    Err.Clear
    On Error GoTo Fail

    sink.Calc.FireChanged 77
    RunOxVbaWrappedComServerSmoke = "OK:" & CStr(sink.LastValue) & ":" & CStr(calc.Value)
    Exit Function
Fail:
    RunOxVbaWrappedComServerSmoke = "ERR:" & CStr(Err.Number) & ":" & Err.Description
End Function
"#,
    );
    write(
        &script_path,
        r#"
param(
    [string]$TlbPath,
    [string]$ClassSourcePath,
    [string]$ModuleSourcePath
)
$ErrorActionPreference = "Stop"
$excel = $null
$wb = $null
$skip = $null
$failure = $null
$stage = "create Excel.Application"
try {
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false
    $excel.DisplayAlerts = $false
    $stage = "create workbook"
    $wb = $excel.Workbooks.Add()
    $stage = "add generated type-library reference"
    [void]$wb.VBProject.References.AddFromFile($TlbPath)
    $stage = "add EventSink class"
    $classComponent = $wb.VBProject.VBComponents.Add(2)
    $classComponent.Name = "EventSink"
    [void]$classComponent.CodeModule.AddFromString((Get-Content -Raw $ClassSourcePath))
    $stage = "add Excel client module"
    $moduleComponent = $wb.VBProject.VBComponents.Add(1)
    $moduleComponent.Name = "ExcelClient"
    [void]$moduleComponent.CodeModule.AddFromString((Get-Content -Raw $ModuleSourcePath))
    $stage = "run Excel VBA client"
    $macroName = "'" + $wb.Name + "'!ExcelClient.RunOxVbaWrappedComServerSmoke"
    $result = $excel.Run($macroName)
    if ([string]$result -ne "OK:77:123") {
        throw "expected early-bound/WithEvents result OK:77:123, got $result"
    }
    "ok"
} catch {
    $message = $_.Exception.Message
    if ($stage -eq "create Excel.Application") {
        $skip = "Excel.Application unavailable: $message"
    } elseif ($message -match "programmatic access|Visual Basic Project|VBProject|VBA project") {
        $skip = "Excel VBOM unavailable: $message"
    } else {
        $failure = $_
    }
} finally {
    if ($wb -ne $null) {
        try { $wb.Close($false) } catch {}
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
    }
    if ($excel -ne $null) {
        try { $excel.Quit() } catch {}
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    }
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
}
if ($skip -ne $null) {
    "SKIP: $skip"
    exit 0
}
if ($failure -ne $null) {
    Write-Error $failure
    exit 1
}
"#,
    );

    let output = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&script_path)
        .arg("-TlbPath")
        .arg(tlb_path)
        .arg("-ClassSourcePath")
        .arg(&class_source_path)
        .arg("-ModuleSourcePath")
        .arg(&module_source_path)
        .output()
        .expect("PowerShell should run Excel smoke script");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "Excel VBA COM server event smoke failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    if stdout.contains("SKIP:") {
        eprintln!("{}", stdout.trim());
    } else {
        assert!(
            stdout.contains("ok"),
            "Excel VBA COM server event smoke returned unexpected output: {stdout}"
        );
    }
}

struct RegisteredDll {
    path: PathBuf,
}

impl RegisteredDll {
    fn register(path: &Path) -> Self {
        let status = Command::new("regsvr32.exe")
            .arg("/s")
            .arg(path)
            .status()
            .expect("regsvr32 register should run");
        assert!(status.success(), "regsvr32 register failed: {status:?}");
        Self {
            path: path.to_path_buf(),
        }
    }
}

impl Drop for RegisteredDll {
    fn drop(&mut self) {
        let _ = Command::new("regsvr32.exe")
            .arg("/u")
            .arg("/s")
            .arg(&self.path)
            .status();
    }
}

unsafe fn controlled_connection_point_smoke(
    descriptor: &oxvba_build::ComServerDescriptor,
    class: &oxvba_build::ComClassDescriptor,
) {
    let hr = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED as u32);
    let should_uninitialize = hr >= 0;
    let result = controlled_connection_point_smoke_inner(descriptor, class);
    if should_uninitialize {
        CoUninitialize();
    }
    result.expect("controlled connection-point smoke");
}

unsafe fn controlled_connection_point_smoke_inner(
    descriptor: &oxvba_build::ComServerDescriptor,
    class: &oxvba_build::ComClassDescriptor,
) -> Result<(), String> {
    let mut clsid = GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    let progid = wide_null(&class.prog_id);
    let hr = CLSIDFromProgID(progid.as_ptr(), &mut clsid);
    if hr < 0 {
        return Err(format!("CLSIDFromProgID failed: 0x{:08X}", hr as u32));
    }

    let mut object: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = CoCreateInstance(
        &clsid,
        std::ptr::null_mut(),
        CLSCTX_INPROC_SERVER,
        &IID_IDISPATCH,
        &mut object,
    );
    if hr < 0 || object.is_null() {
        return Err(format!("CoCreateInstance failed: 0x{:08X}", hr as u32));
    }

    let result = (|| {
        assert_dispatch_type_info(object, &parse_guid(&class.default_interface_iid)?)?;
        let mut cpc: *mut std::ffi::c_void = std::ptr::null_mut();
        let hr = query_interface(object, &IID_ICONNECTIONPOINTCONTAINER, &mut cpc);
        if hr < 0 || cpc.is_null() {
            return Err(format!(
                "QueryInterface(IConnectionPointContainer) failed: 0x{:08X}",
                hr as u32
            ));
        }
        let result = controlled_connection_point_with_container(descriptor, class, object, cpc);
        release_unknown(cpc);
        result
    })();
    release_unknown(object);
    result
}

unsafe fn controlled_dual_vtable_smoke(class: &oxvba_build::ComClassDescriptor) {
    let hr = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED as u32);
    let should_uninitialize = hr >= 0;
    let result = controlled_dual_vtable_smoke_inner(class);
    if should_uninitialize {
        CoUninitialize();
    }
    result.expect("controlled dual-interface vtable smoke");
}

unsafe fn controlled_dual_vtable_smoke_inner(
    class: &oxvba_build::ComClassDescriptor,
) -> Result<(), String> {
    let ping = class
        .members
        .iter()
        .find(|member| member.name == "Ping")
        .ok_or_else(|| "Ping descriptor missing".to_string())?;
    let add_pair = class
        .members
        .iter()
        .find(|member| member.name == "AddPair")
        .ok_or_else(|| "AddPair descriptor missing".to_string())?;
    let average = class
        .members
        .iter()
        .find(|member| member.name == "Average")
        .ok_or_else(|| "Average descriptor missing".to_string())?;

    let mut clsid = GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    let progid = wide_null(&class.prog_id);
    let hr = CLSIDFromProgID(progid.as_ptr(), &mut clsid);
    if hr < 0 {
        return Err(format!("CLSIDFromProgID failed: 0x{:08X}", hr as u32));
    }

    let mut object: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = CoCreateInstance(
        &clsid,
        std::ptr::null_mut(),
        CLSCTX_INPROC_SERVER,
        &IID_IDISPATCH,
        &mut object,
    );
    if hr < 0 || object.is_null() {
        return Err(format!("CoCreateInstance failed: 0x{:08X}", hr as u32));
    }

    let result = (|| {
        assert_dispatch_type_info(object, &parse_guid(&class.default_interface_iid)?)?;
        let dispatch_value = invoke_i4_method_result(object, ping.dispid)?;
        let dispatch_pair_value = invoke_i4_method_pair_result(object, add_pair.dispid, 19, 23)?;
        let dispatch_average_value =
            invoke_r8_method_pair_result(object, average.dispid, 10.5, 21.5)?;
        let iid = parse_guid(&class.default_interface_iid)?;
        let mut dual: *mut std::ffi::c_void = std::ptr::null_mut();
        let hr = query_interface(object, &iid, &mut dual);
        if hr < 0 || dual.is_null() {
            return Err(format!(
                "QueryInterface({}) failed: 0x{:08X}",
                class.default_interface_iid, hr as u32
            ));
        }

        let dual_result = (|| {
            let vtbl = *(dual.cast::<*const BoundedDualVtbl>());
            let mut vtable_value = 0i32;
            let hr = ((*vtbl).slot0)(dual, &mut vtable_value);
            if hr < 0 {
                return Err(format!("dual Ping vtable call failed: 0x{:08X}", hr as u32));
            }
            if dispatch_value != vtable_value {
                return Err(format!(
                    "dispatch Ping returned {dispatch_value}, vtable returned {vtable_value}"
                ));
            }
            if vtable_value != 42 {
                return Err(format!("expected vtable Ping()=42, got {vtable_value}"));
            }
            let mut vtable_pair_value = 0i32;
            let hr = ((*vtbl).slot1)(dual, 19, 23, &mut vtable_pair_value);
            if hr < 0 {
                return Err(format!(
                    "dual AddPair vtable call failed: 0x{:08X}",
                    hr as u32
                ));
            }
            if dispatch_pair_value != vtable_pair_value {
                return Err(format!(
                    "dispatch AddPair returned {dispatch_pair_value}, vtable returned {vtable_pair_value}"
                ));
            }
            if vtable_pair_value != 42 {
                return Err(format!(
                    "expected vtable AddPair(19,23)=42, got {vtable_pair_value}"
                ));
            }
            let mut vtable_average_value = 0.0f64;
            let hr = ((*vtbl).slot2)(dual, 10.5, 21.5, &mut vtable_average_value);
            if hr < 0 {
                return Err(format!(
                    "dual Average vtable call failed: 0x{:08X}",
                    hr as u32
                ));
            }
            if (dispatch_average_value - vtable_average_value).abs() > 0.0000001 {
                return Err(format!(
                    "dispatch Average returned {dispatch_average_value}, vtable returned {vtable_average_value}"
                ));
            }
            if (vtable_average_value - 16.0).abs() > 0.0000001 {
                return Err(format!(
                    "expected vtable Average(10.5,21.5)=16, got {vtable_average_value}"
                ));
            }
            Ok(())
        })();
        release_unknown(dual);
        dual_result
    })();
    release_unknown(object);
    result
}

unsafe fn controlled_property_vtable_smoke(class: &oxvba_build::ComClassDescriptor) {
    let hr = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED as u32);
    let should_uninitialize = hr >= 0;
    let result = controlled_property_vtable_smoke_inner(class);
    if should_uninitialize {
        CoUninitialize();
    }
    result.expect("controlled dual-interface property vtable smoke");
}

unsafe fn controlled_property_vtable_smoke_inner(
    class: &oxvba_build::ComClassDescriptor,
) -> Result<(), String> {
    let value_get = class
        .members
        .iter()
        .find(|member| {
            member.name.eq_ignore_ascii_case("Value")
                && member.invoke_kind == oxvba_build::ComInvokeKind::PropertyGet
        })
        .ok_or_else(|| "Value property get descriptor missing".to_string())?;
    let value_put = class
        .members
        .iter()
        .find(|member| {
            member.name.eq_ignore_ascii_case("Value")
                && member.invoke_kind == oxvba_build::ComInvokeKind::PropertyPut
        })
        .ok_or_else(|| "Value property put descriptor missing".to_string())?;

    let mut clsid = GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    let progid = wide_null(&class.prog_id);
    let hr = CLSIDFromProgID(progid.as_ptr(), &mut clsid);
    if hr < 0 {
        return Err(format!("CLSIDFromProgID failed: 0x{:08X}", hr as u32));
    }

    let mut object: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = CoCreateInstance(
        &clsid,
        std::ptr::null_mut(),
        CLSCTX_INPROC_SERVER,
        &IID_IDISPATCH,
        &mut object,
    );
    if hr < 0 || object.is_null() {
        return Err(format!("CoCreateInstance failed: 0x{:08X}", hr as u32));
    }

    let result = (|| {
        assert_dispatch_type_info(object, &parse_guid(&class.default_interface_iid)?)?;
        invoke_i4_property_put(object, value_put.dispid, 1234)?;
        let dispatch_value = invoke_i4_property_get(object, value_get.dispid)?;
        let iid = parse_guid(&class.default_interface_iid)?;
        let mut dual: *mut std::ffi::c_void = std::ptr::null_mut();
        let hr = query_interface(object, &iid, &mut dual);
        if hr < 0 || dual.is_null() {
            return Err(format!(
                "QueryInterface({}) failed: 0x{:08X}",
                class.default_interface_iid, hr as u32
            ));
        }

        let dual_result = (|| {
            let vtbl = *(dual.cast::<*const LongPropertyDualVtbl>());
            let mut vtable_value = 0i32;
            let hr = ((*vtbl).slot0)(dual, &mut vtable_value);
            if hr < 0 {
                return Err(format!(
                    "dual Value propget vtable call failed: 0x{:08X}",
                    hr as u32
                ));
            }
            if dispatch_value != vtable_value {
                return Err(format!(
                    "dispatch Value returned {dispatch_value}, vtable returned {vtable_value}"
                ));
            }
            let hr = ((*vtbl).slot1)(dual, 5678);
            if hr < 0 {
                return Err(format!(
                    "dual Value propput vtable call failed: 0x{:08X}",
                    hr as u32
                ));
            }
            let dispatch_after_put = invoke_i4_property_get(object, value_get.dispid)?;
            if dispatch_after_put != 5678 {
                return Err(format!(
                    "expected dispatch Value after vtable put to be 5678, got {dispatch_after_put}"
                ));
            }
            Ok(())
        })();
        release_unknown(dual);
        dual_result
    })();
    release_unknown(object);
    result
}

unsafe fn controlled_object_return_vtable_smoke(class: &oxvba_build::ComClassDescriptor) {
    let hr = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED as u32);
    let should_uninitialize = hr >= 0;
    let result = controlled_object_return_vtable_smoke_inner(class);
    if should_uninitialize {
        CoUninitialize();
    }
    result.expect("controlled dual-interface object-return vtable smoke");
}

unsafe fn controlled_object_return_vtable_smoke_inner(
    class: &oxvba_build::ComClassDescriptor,
) -> Result<(), String> {
    let return_self = class
        .members
        .iter()
        .find(|member| member.name == "ReturnSelf")
        .ok_or_else(|| "ReturnSelf descriptor missing".to_string())?;
    let ping = class
        .members
        .iter()
        .find(|member| member.name == "Ping")
        .ok_or_else(|| "Ping descriptor missing".to_string())?;

    let mut clsid = GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    let progid = wide_null(&class.prog_id);
    let hr = CLSIDFromProgID(progid.as_ptr(), &mut clsid);
    if hr < 0 {
        return Err(format!("CLSIDFromProgID failed: 0x{:08X}", hr as u32));
    }

    let mut object: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = CoCreateInstance(
        &clsid,
        std::ptr::null_mut(),
        CLSCTX_INPROC_SERVER,
        &IID_IDISPATCH,
        &mut object,
    );
    if hr < 0 || object.is_null() {
        return Err(format!("CoCreateInstance failed: 0x{:08X}", hr as u32));
    }

    let result = (|| {
        assert_dispatch_type_info(object, &parse_guid(&class.default_interface_iid)?)?;
        let dispatch_returned = invoke_dispatch_object_method_result(object, return_self.dispid)?;
        let dispatch_returned_result = (|| {
            let returned_ping = invoke_i4_method_result(dispatch_returned, ping.dispid)?;
            if returned_ping != 42 {
                return Err(format!(
                    "dispatch ReturnSelf().Ping() expected 42, got {returned_ping}"
                ));
            }
            Ok(())
        })();
        release_unknown(dispatch_returned);
        dispatch_returned_result?;

        let iid = parse_guid(&class.default_interface_iid)?;
        let mut dual: *mut std::ffi::c_void = std::ptr::null_mut();
        let hr = query_interface(object, &iid, &mut dual);
        if hr < 0 || dual.is_null() {
            return Err(format!(
                "QueryInterface({}) failed: 0x{:08X}",
                class.default_interface_iid, hr as u32
            ));
        }

        let dual_result = (|| {
            let vtbl = *(dual.cast::<*const ObjectReturnDualVtbl>());
            let mut vtable_returned: *mut std::ffi::c_void = std::ptr::null_mut();
            let hr = ((*vtbl).slot0)(dual, &mut vtable_returned);
            if hr < 0 || vtable_returned.is_null() {
                return Err(format!(
                    "dual ReturnSelf vtable call failed: 0x{:08X}, ptr={vtable_returned:p}",
                    hr as u32
                ));
            }
            let vtable_returned_result = (|| {
                let returned_ping = invoke_i4_method_result(vtable_returned, ping.dispid)?;
                if returned_ping != 42 {
                    return Err(format!(
                        "vtable ReturnSelf().Ping() expected 42, got {returned_ping}"
                    ));
                }
                Ok(())
            })();
            release_unknown(vtable_returned);
            vtable_returned_result?;

            let mut vtable_ping = 0i32;
            let hr = ((*vtbl).slot1)(dual, &mut vtable_ping);
            if hr < 0 {
                return Err(format!(
                    "dual object-return shape Ping vtable call failed: 0x{:08X}",
                    hr as u32
                ));
            }
            if vtable_ping != 42 {
                return Err(format!(
                    "expected object-return shape Ping()=42, got {vtable_ping}"
                ));
            }
            Ok(())
        })();
        release_unknown(dual);
        dual_result
    })();
    release_unknown(object);
    result
}

unsafe fn controlled_object_argument_vtable_smoke(class: &oxvba_build::ComClassDescriptor) {
    let hr = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED as u32);
    let should_uninitialize = hr >= 0;
    let result = controlled_object_argument_vtable_smoke_inner(class);
    if should_uninitialize {
        CoUninitialize();
    }
    result.expect("controlled dual-interface object-argument vtable smoke");
}

unsafe fn controlled_object_argument_vtable_smoke_inner(
    class: &oxvba_build::ComClassDescriptor,
) -> Result<(), String> {
    let ping = class
        .members
        .iter()
        .find(|member| member.name == "Ping")
        .ok_or_else(|| "Ping descriptor missing".to_string())?;
    let echo_ping = class
        .members
        .iter()
        .find(|member| member.name == "EchoPing")
        .ok_or_else(|| "EchoPing descriptor missing".to_string())?;

    let mut clsid = GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    let progid = wide_null(&class.prog_id);
    let hr = CLSIDFromProgID(progid.as_ptr(), &mut clsid);
    if hr < 0 {
        return Err(format!("CLSIDFromProgID failed: 0x{:08X}", hr as u32));
    }

    let mut object: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = CoCreateInstance(
        &clsid,
        std::ptr::null_mut(),
        CLSCTX_INPROC_SERVER,
        &IID_IDISPATCH,
        &mut object,
    );
    if hr < 0 || object.is_null() {
        return Err(format!("CoCreateInstance failed: 0x{:08X}", hr as u32));
    }

    let result = (|| {
        assert_dispatch_type_info(object, &parse_guid(&class.default_interface_iid)?)?;
        let dispatch_ping = invoke_i4_method_result(object, ping.dispid)?;
        let dispatch_echo = invoke_i4_method_object_result(object, echo_ping.dispid, object)?;
        if dispatch_echo != 42 {
            return Err(format!(
                "dispatch EchoPing(ObjectRelay) expected 42, got {dispatch_echo}"
            ));
        }
        let iid = parse_guid(&class.default_interface_iid)?;
        let mut dual: *mut std::ffi::c_void = std::ptr::null_mut();
        let hr = query_interface(object, &iid, &mut dual);
        if hr < 0 || dual.is_null() {
            return Err(format!(
                "QueryInterface({}) failed: 0x{:08X}",
                class.default_interface_iid, hr as u32
            ));
        }

        let dual_result = (|| {
            let vtbl = *(dual.cast::<*const ObjectArgumentDualVtbl>());
            let mut vtable_ping = 0i32;
            let hr = ((*vtbl).slot0)(dual, &mut vtable_ping);
            if hr < 0 {
                return Err(format!(
                    "dual object-argument shape Ping vtable call failed: 0x{:08X}",
                    hr as u32
                ));
            }
            if dispatch_ping != vtable_ping {
                return Err(format!(
                    "dispatch Ping returned {dispatch_ping}, vtable returned {vtable_ping}"
                ));
            }
            let mut vtable_echo = 0i32;
            let hr = ((*vtbl).slot1)(dual, object, &mut vtable_echo);
            if hr < 0 {
                return Err(format!(
                    "dual EchoPing vtable call failed: 0x{:08X}",
                    hr as u32
                ));
            }
            if dispatch_echo != vtable_echo {
                return Err(format!(
                    "dispatch EchoPing returned {dispatch_echo}, vtable returned {vtable_echo}"
                ));
            }
            if vtable_echo != 42 {
                return Err(format!(
                    "expected vtable EchoPing(ObjectRelay)=42, got {vtable_echo}"
                ));
            }
            let mut vtable_echo_from_dual_arg = 0i32;
            let hr = ((*vtbl).slot1)(dual, dual, &mut vtable_echo_from_dual_arg);
            if hr < 0 {
                return Err(format!(
                    "dual EchoPing(default interface) vtable call failed: 0x{:08X}",
                    hr as u32
                ));
            }
            if vtable_echo_from_dual_arg != 42 {
                return Err(format!(
                    "expected vtable EchoPing(default interface)=42, got {vtable_echo_from_dual_arg}"
                ));
            }
            Ok(())
        })();
        release_unknown(dual);
        dual_result
    })();
    release_unknown(object);
    result
}

unsafe fn controlled_office_profile_vtable_smoke(
    addin_class: &oxvba_build::ComClassDescriptor,
    rtd_class: &oxvba_build::ComClassDescriptor,
) {
    let hr = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED as u32);
    let should_uninitialize = hr >= 0;
    let result = controlled_office_profile_vtable_smoke_inner(addin_class, rtd_class);
    if should_uninitialize {
        CoUninitialize();
    }
    result.expect("controlled Office profile vtable smoke");
}

unsafe fn controlled_office_profile_vtable_smoke_inner(
    addin_class: &oxvba_build::ComClassDescriptor,
    rtd_class: &oxvba_build::ComClassDescriptor,
) -> Result<(), String> {
    let addin = activate_dispatch(addin_class)?;
    let addin_result = (|| {
        let mut idte: *mut std::ffi::c_void = std::ptr::null_mut();
        let hr = query_interface(addin, &IID_IDTEXTENSIBILITY2, &mut idte);
        if hr < 0 || idte.is_null() {
            return Err(format!(
                "QueryInterface(IDTExtensibility2) failed: 0x{:08X}",
                hr as u32
            ));
        }
        let result = controlled_idte_vtable(addin, idte, addin_class);
        release_unknown(idte);
        result
    })();
    release_unknown(addin);
    addin_result?;

    let rtd = activate_dispatch(rtd_class)?;
    let rtd_result = (|| {
        let mut server: *mut std::ffi::c_void = std::ptr::null_mut();
        let hr = query_interface(rtd, &IID_IRTDSERVER, &mut server);
        if hr < 0 || server.is_null() {
            return Err(format!(
                "QueryInterface(IRtdServer) failed: 0x{:08X}",
                hr as u32
            ));
        }
        let result = controlled_rtd_vtable(server);
        release_unknown(server);
        result
    })();
    release_unknown(rtd);
    rtd_result
}

unsafe fn activate_dispatch(
    class: &oxvba_build::ComClassDescriptor,
) -> Result<*mut std::ffi::c_void, String> {
    let mut clsid = GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    let progid = wide_null(&class.prog_id);
    let hr = CLSIDFromProgID(progid.as_ptr(), &mut clsid);
    if hr < 0 {
        return Err(format!("CLSIDFromProgID failed: 0x{:08X}", hr as u32));
    }
    let mut object: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = CoCreateInstance(
        &clsid,
        std::ptr::null_mut(),
        CLSCTX_INPROC_SERVER,
        &IID_IDISPATCH,
        &mut object,
    );
    if hr < 0 || object.is_null() {
        return Err(format!("CoCreateInstance failed: 0x{:08X}", hr as u32));
    }
    Ok(object)
}

unsafe fn activate_excel_application() -> Result<*mut std::ffi::c_void, String> {
    let mut clsid = GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    let progid = wide_null("Excel.Application");
    let hr = CLSIDFromProgID(progid.as_ptr(), &mut clsid);
    if hr < 0 {
        return Err(format!(
            "CLSIDFromProgID(Excel.Application) failed: 0x{:08X}",
            hr as u32
        ));
    }
    let mut object: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = CoCreateInstance(
        &clsid,
        std::ptr::null_mut(),
        CLSCTX_LOCAL_SERVER,
        &IID_IDISPATCH,
        &mut object,
    );
    if hr < 0 || object.is_null() {
        return Err(format!(
            "CoCreateInstance(Excel.Application) failed: 0x{:08X}",
            hr as u32
        ));
    }
    Ok(object)
}

unsafe fn controlled_idte_vtable(
    addin: *mut std::ffi::c_void,
    idte: *mut std::ffi::c_void,
    addin_class: &oxvba_build::ComClassDescriptor,
) -> Result<(), String> {
    let vtbl = *(idte.cast::<*const IdtExtensibility2Vtbl>());
    let mut count = 0u32;
    let hr = ((*vtbl).get_type_info_count)(idte, &mut count);
    if hr < 0 || count != 1 {
        return Err(format!(
            "IDTExtensibility2 GetTypeInfoCount failed/count mismatch: hr=0x{:08X}, count={count}",
            hr as u32
        ));
    }
    let application_name_length = addin_class
        .members
        .iter()
        .find(|member| member.name == "ApplicationNameLength")
        .map(|member| member.dispid)
        .ok_or_else(|| "ApplicationNameLength descriptor missing".to_string())?;
    let excel = activate_excel_application()?;
    let hr = ((*vtbl).on_connection)(idte, excel, 3, std::ptr::null_mut(), std::ptr::null_mut());
    let _ = invoke_dispatch_method_by_name(excel, "Quit");
    release_unknown(excel);
    if hr < 0 {
        return Err(format!(
            "IDTExtensibility2 OnConnection failed: 0x{:08X}",
            hr as u32
        ));
    }
    let length = invoke_i4_method_result(addin, application_name_length)?;
    if length <= 0 {
        return Err(format!(
            "expected OnConnection to read Excel.Application.Name, got length {length}"
        ));
    }
    let hr = ((*vtbl).on_disconnection)(idte, 2, std::ptr::null_mut());
    if hr < 0 {
        return Err(format!(
            "IDTExtensibility2 OnDisconnection failed: 0x{:08X}",
            hr as u32
        ));
    }
    let hr = ((*vtbl).on_addins_update)(idte, std::ptr::null_mut());
    if hr < 0 {
        return Err(format!(
            "IDTExtensibility2 OnAddInsUpdate failed: 0x{:08X}",
            hr as u32
        ));
    }
    let hr = ((*vtbl).on_startup_complete)(idte, std::ptr::null_mut());
    if hr < 0 {
        return Err(format!(
            "IDTExtensibility2 OnStartupComplete failed: 0x{:08X}",
            hr as u32
        ));
    }
    let hr = ((*vtbl).on_begin_shutdown)(idte, std::ptr::null_mut());
    if hr < 0 {
        return Err(format!(
            "IDTExtensibility2 OnBeginShutdown failed: 0x{:08X}",
            hr as u32
        ));
    }
    Ok(())
}

unsafe fn controlled_rtd_vtable(server: *mut std::ffi::c_void) -> Result<(), String> {
    let vtbl = *(server.cast::<*const RtdServerVtbl>());
    let mut count = 0u32;
    let hr = ((*vtbl).get_type_info_count)(server, &mut count);
    if hr < 0 || count != 1 {
        return Err(format!(
            "IRtdServer GetTypeInfoCount failed/count mismatch: hr=0x{:08X}, count={count}",
            hr as u32
        ));
    }

    let mut start = 0i32;
    let hr = ((*vtbl).server_start)(server, std::ptr::null_mut(), &mut start);
    if hr < 0 || start != 1 {
        return Err(format!(
            "IRtdServer ServerStart failed/value mismatch: hr=0x{:08X}, value={start}",
            hr as u32
        ));
    }

    let mut new_values = 0i16;
    let mut connect_value: VARIANT = std::mem::zeroed();
    let hr = ((*vtbl).connect_data)(
        server,
        42,
        std::ptr::null_mut(),
        &mut new_values,
        &mut connect_value,
    );
    if hr < 0 {
        VariantClear(&mut connect_value);
        return Err(format!(
            "IRtdServer ConnectData failed: 0x{:08X}",
            hr as u32
        ));
    }
    let connected = if connect_value.Anonymous.Anonymous.vt == VT_I4 {
        connect_value.Anonymous.Anonymous.Anonymous.lVal
    } else {
        let vt = connect_value.Anonymous.Anonymous.vt;
        VariantClear(&mut connect_value);
        return Err(format!(
            "IRtdServer ConnectData returned VT {vt}, expected VT_I4"
        ));
    };
    VariantClear(&mut connect_value);
    if connected != 142 || new_values != -1 {
        return Err(format!(
            "IRtdServer ConnectData expected value/newValues 142/-1, got {connected}/{new_values}"
        ));
    }

    let mut topic_count = 0i32;
    let mut data: *mut SAFEARRAY = std::ptr::null_mut();
    let hr = ((*vtbl).refresh_data)(server, &mut topic_count, &mut data);
    if hr < 0 || data.is_null() {
        return Err(format!(
            "IRtdServer RefreshData failed: hr=0x{:08X}, data={data:p}",
            hr as u32
        ));
    }
    let destroy_hr = SafeArrayDestroy(data.cast_const());
    if destroy_hr < 0 {
        return Err(format!(
            "SafeArrayDestroy(RefreshData result) failed: 0x{:08X}",
            destroy_hr as u32
        ));
    }
    if topic_count != 1 {
        return Err(format!(
            "IRtdServer RefreshData expected TopicCount=1, got {topic_count}"
        ));
    }

    let mut heartbeat = 0i32;
    let hr = ((*vtbl).heartbeat)(server, &mut heartbeat);
    if hr < 0 || heartbeat != 1 {
        return Err(format!(
            "IRtdServer Heartbeat failed/value mismatch: hr=0x{:08X}, value={heartbeat}",
            hr as u32
        ));
    }
    let hr = ((*vtbl).disconnect_data)(server, 42);
    if hr < 0 {
        return Err(format!(
            "IRtdServer DisconnectData failed: 0x{:08X}",
            hr as u32
        ));
    }
    let hr = ((*vtbl).server_terminate)(server);
    if hr < 0 {
        return Err(format!(
            "IRtdServer ServerTerminate failed: 0x{:08X}",
            hr as u32
        ));
    }
    Ok(())
}

unsafe fn assert_dispatch_type_info(
    dispatch: *mut std::ffi::c_void,
    expected_iid: &GUID,
) -> Result<(), String> {
    let vtbl = *(dispatch.cast::<*const IDispatchVtbl>());
    let mut count = 0u32;
    let hr = ((*vtbl).get_type_info_count)(dispatch, &mut count);
    if hr < 0 || count != 1 {
        return Err(format!(
            "GetTypeInfoCount expected one typeinfo, hr=0x{:08X}, count={count}",
            hr as u32
        ));
    }

    let mut type_info: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = ((*vtbl).get_type_info)(dispatch, 0, 0, &mut type_info);
    if hr < 0 || type_info.is_null() {
        return Err(format!("GetTypeInfo(0) failed: 0x{:08X}", hr as u32));
    }
    let guid_result = assert_type_info_guid(type_info, expected_iid);
    release_unknown(type_info);
    guid_result?;

    let mut bad_index_info: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = ((*vtbl).get_type_info)(dispatch, 1, 0, &mut bad_index_info);
    if hr != DISP_E_BADINDEX || !bad_index_info.is_null() {
        if !bad_index_info.is_null() {
            release_unknown(bad_index_info);
        }
        return Err(format!(
            "GetTypeInfo(1) expected DISP_E_BADINDEX/null, hr=0x{:08X}, ptr={bad_index_info:p}",
            hr as u32
        ));
    }
    Ok(())
}

unsafe fn assert_type_info_guid(
    type_info: *mut std::ffi::c_void,
    expected: &GUID,
) -> Result<(), String> {
    let vtbl = *(type_info.cast::<*const ITypeInfoVtbl>());
    let mut attr: *mut TYPEATTR = std::ptr::null_mut();
    let hr = ((*vtbl).get_type_attr)(type_info, &mut attr);
    if hr < 0 || attr.is_null() {
        return Err(format!(
            "ITypeInfo::GetTypeAttr failed: 0x{:08X}",
            hr as u32
        ));
    }
    let actual = (*attr).guid;
    ((*vtbl).release_type_attr)(type_info, attr);
    if !guid_eq(&actual, expected) {
        return Err(format!(
            "GetTypeInfo(0) returned {}, expected {}",
            guid_to_string(&actual),
            guid_to_string(expected)
        ));
    }
    Ok(())
}

unsafe fn controlled_connection_point_with_container(
    _descriptor: &oxvba_build::ComServerDescriptor,
    class: &oxvba_build::ComClassDescriptor,
    object: *mut std::ffi::c_void,
    cpc: *mut std::ffi::c_void,
) -> Result<(), String> {
    let source_iid = parse_guid(
        class
            .source_interface_iid
            .as_deref()
            .ok_or_else(|| "class should have a source interface".to_string())?,
    )?;
    let fire_member = class
        .members
        .iter()
        .find(|member| member.name == "FireChanged")
        .ok_or_else(|| "FireChanged descriptor missing".to_string())?;
    let changed_event = class
        .events
        .iter()
        .find(|event| event.name == "Changed")
        .ok_or_else(|| "Changed descriptor missing".to_string())?;

    let cpc_vtbl = *(cpc.cast::<*const IConnectionPointContainerVtbl>());
    let mut cp: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = ((*cpc_vtbl).find_connection_point)(cpc, &source_iid, &mut cp);
    if hr < 0 || cp.is_null() {
        return Err(format!("FindConnectionPoint failed: 0x{:08X}", hr as u32));
    }
    if let Err(err) = assert_enum_connection_points(cpc, &source_iid) {
        release_unknown(cp);
        return Err(err);
    }

    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = TestSink::new(seen.clone());
    let cp_vtbl = *(cp.cast::<*const IConnectionPointVtbl>());
    let mut cookie = 0u32;
    let hr = ((*cp_vtbl).advise)(cp, sink.cast(), &mut cookie);
    if hr < 0 || cookie == 0 {
        release_unknown(cp);
        release_unknown(sink.cast());
        return Err(format!("Advise failed: 0x{:08X}", hr as u32));
    }
    if let Err(err) = assert_enum_connections(cp, cookie) {
        let _ = ((*cp_vtbl).unadvise)(cp, cookie);
        release_unknown(cp);
        release_unknown(sink.cast());
        return Err(err);
    }

    let invoke_result = invoke_i4_method(object, fire_member.dispid, 42);
    let unadvise_hr = ((*cp_vtbl).unadvise)(cp, cookie);
    release_unknown(cp);
    release_unknown(sink.cast());
    invoke_result?;
    if unadvise_hr < 0 {
        return Err(format!("Unadvise failed: 0x{:08X}", unadvise_hr as u32));
    }

    let seen = seen.lock().expect("seen lock").clone();
    if seen != vec![(changed_event.dispid, Some(42))] {
        return Err(format!("expected Changed event payload 42, got {seen:?}"));
    }
    Ok(())
}

unsafe fn assert_enum_connection_points(
    cpc: *mut std::ffi::c_void,
    expected_source_iid: &GUID,
) -> Result<(), String> {
    let cpc_vtbl = *(cpc.cast::<*const IConnectionPointContainerVtbl>());
    let mut enumerator: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = ((*cpc_vtbl).enum_connection_points)(cpc, &mut enumerator);
    if hr < 0 || enumerator.is_null() {
        return Err(format!("EnumConnectionPoints failed: 0x{:08X}", hr as u32));
    }

    let result = (|| {
        let enum_vtbl = *(enumerator.cast::<*const IEnumConnectionPointsVtbl>());
        let mut cp: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut fetched = 0u32;
        let hr = ((*enum_vtbl).next)(enumerator, 1, &mut cp, &mut fetched);
        if hr < 0 || fetched != 1 || cp.is_null() {
            return Err(format!(
                "IEnumConnectionPoints::Next expected one connection point, hr=0x{:08X}, fetched={fetched}",
                hr as u32
            ));
        }
        let cp_result = (|| {
            let cp_vtbl = *(cp.cast::<*const IConnectionPointVtbl>());
            let mut actual_iid = GUID {
                data1: 0,
                data2: 0,
                data3: 0,
                data4: [0; 8],
            };
            let hr = ((*cp_vtbl).get_connection_interface)(cp, &mut actual_iid);
            if hr < 0 {
                return Err(format!(
                    "IConnectionPoint::GetConnectionInterface failed: 0x{:08X}",
                    hr as u32
                ));
            }
            if !guid_eq(&actual_iid, expected_source_iid) {
                return Err(format!(
                    "EnumConnectionPoints returned IID {}, expected {}",
                    guid_to_string(&actual_iid),
                    guid_to_string(expected_source_iid)
                ));
            }
            Ok(())
        })();
        release_unknown(cp);
        cp_result?;

        let mut extra: *mut std::ffi::c_void = std::ptr::null_mut();
        fetched = u32::MAX;
        let hr = ((*enum_vtbl).next)(enumerator, 1, &mut extra, &mut fetched);
        if hr != S_FALSE || fetched != 0 || !extra.is_null() {
            if !extra.is_null() {
                release_unknown(extra);
            }
            return Err(format!(
                "IEnumConnectionPoints::Next past end expected S_FALSE/0/null, hr=0x{:08X}, fetched={fetched}, extra={extra:p}",
                hr as u32
            ));
        }

        let hr = ((*enum_vtbl).reset)(enumerator);
        if hr < 0 {
            return Err(format!(
                "IEnumConnectionPoints::Reset failed: 0x{:08X}",
                hr as u32
            ));
        }
        let mut reset_cp: *mut std::ffi::c_void = std::ptr::null_mut();
        fetched = 0;
        let hr = ((*enum_vtbl).next)(enumerator, 1, &mut reset_cp, &mut fetched);
        if hr < 0 || fetched != 1 || reset_cp.is_null() {
            return Err(format!(
                "IEnumConnectionPoints::Next after Reset failed: hr=0x{:08X}, fetched={fetched}",
                hr as u32
            ));
        }
        release_unknown(reset_cp);
        Ok(())
    })();
    release_unknown(enumerator);
    result
}

unsafe fn assert_enum_connections(
    cp: *mut std::ffi::c_void,
    expected_cookie: u32,
) -> Result<(), String> {
    let cp_vtbl = *(cp.cast::<*const IConnectionPointVtbl>());
    let mut enumerator: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = ((*cp_vtbl).enum_connections)(cp, &mut enumerator);
    if hr < 0 || enumerator.is_null() {
        return Err(format!("EnumConnections failed: 0x{:08X}", hr as u32));
    }

    let result = (|| {
        let enum_vtbl = *(enumerator.cast::<*const IEnumConnectionsVtbl>());
        let mut data = ConnectData {
            p_unk: std::ptr::null_mut(),
            cookie: 0,
        };
        let mut fetched = 0u32;
        let hr = ((*enum_vtbl).next)(enumerator, 1, &mut data, &mut fetched);
        if hr < 0 || fetched != 1 || data.p_unk.is_null() {
            return Err(format!(
                "IEnumConnections::Next expected one connection, hr=0x{:08X}, fetched={fetched}",
                hr as u32
            ));
        }
        let connection_result = (|| {
            if data.cookie != expected_cookie {
                return Err(format!(
                    "IEnumConnections returned cookie {}, expected {expected_cookie}",
                    data.cookie
                ));
            }
            let mut dispatch: *mut std::ffi::c_void = std::ptr::null_mut();
            let hr = query_interface(data.p_unk, &IID_IDISPATCH, &mut dispatch);
            if hr < 0 || dispatch.is_null() {
                return Err(format!(
                    "IEnumConnections returned non-dispatch sink: 0x{:08X}",
                    hr as u32
                ));
            }
            release_unknown(dispatch);
            Ok(())
        })();
        release_unknown(data.p_unk);
        connection_result?;

        let mut extra = ConnectData {
            p_unk: std::ptr::null_mut(),
            cookie: 0,
        };
        fetched = u32::MAX;
        let hr = ((*enum_vtbl).next)(enumerator, 1, &mut extra, &mut fetched);
        if hr != S_FALSE || fetched != 0 || !extra.p_unk.is_null() || extra.cookie != 0 {
            if !extra.p_unk.is_null() {
                release_unknown(extra.p_unk);
            }
            return Err(format!(
                "IEnumConnections::Next past end expected S_FALSE/0/null, hr=0x{:08X}, fetched={fetched}, cookie={}",
                hr as u32, extra.cookie
            ));
        }

        let hr = ((*enum_vtbl).reset)(enumerator);
        if hr < 0 {
            return Err(format!(
                "IEnumConnections::Reset failed: 0x{:08X}",
                hr as u32
            ));
        }
        let mut reset_data = ConnectData {
            p_unk: std::ptr::null_mut(),
            cookie: 0,
        };
        fetched = 0;
        let hr = ((*enum_vtbl).next)(enumerator, 1, &mut reset_data, &mut fetched);
        if hr < 0 || fetched != 1 || reset_data.p_unk.is_null() {
            return Err(format!(
                "IEnumConnections::Next after Reset failed: hr=0x{:08X}, fetched={fetched}",
                hr as u32
            ));
        }
        release_unknown(reset_data.p_unk);
        Ok(())
    })();
    release_unknown(enumerator);
    result
}

unsafe fn invoke_i4_method(
    dispatch: *mut std::ffi::c_void,
    dispid: i32,
    value: i32,
) -> Result<(), String> {
    let vtbl = *(dispatch.cast::<*const IDispatchVtbl>());
    let mut arg: VARIANT = std::mem::zeroed();
    arg.Anonymous.Anonymous.vt = VT_I4;
    arg.Anonymous.Anonymous.Anonymous.lVal = value;
    let mut params = windows_sys::Win32::System::Com::DISPPARAMS {
        rgvarg: &mut arg,
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 1,
        cNamedArgs: 0,
    };
    let hr = ((*vtbl).invoke)(
        dispatch,
        dispid,
        &IID_NULL,
        0,
        DISPATCH_METHOD,
        &mut params,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    VariantClear(&mut arg);
    if hr < 0 {
        Err(format!("Invoke(FireChanged) failed: 0x{:08X}", hr as u32))
    } else {
        Ok(())
    }
}

unsafe fn invoke_i4_method_result(
    dispatch: *mut std::ffi::c_void,
    dispid: i32,
) -> Result<i32, String> {
    let vtbl = *(dispatch.cast::<*const IDispatchVtbl>());
    let mut params = windows_sys::Win32::System::Com::DISPPARAMS {
        rgvarg: std::ptr::null_mut(),
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 0,
        cNamedArgs: 0,
    };
    let mut result: VARIANT = std::mem::zeroed();
    let hr = ((*vtbl).invoke)(
        dispatch,
        dispid,
        &IID_NULL,
        0,
        DISPATCH_METHOD,
        &mut params,
        &mut result,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    if hr < 0 {
        VariantClear(&mut result);
        return Err(format!("Invoke(Ping) failed: 0x{:08X}", hr as u32));
    }
    let value = if result.Anonymous.Anonymous.vt == VT_I4 {
        Ok(result.Anonymous.Anonymous.Anonymous.lVal)
    } else {
        Err(format!(
            "Invoke(Ping) returned VT {}, expected VT_I4",
            result.Anonymous.Anonymous.vt
        ))
    };
    VariantClear(&mut result);
    value
}

unsafe fn invoke_dispatch_method_by_name(
    dispatch: *mut std::ffi::c_void,
    name: &str,
) -> Result<(), String> {
    let dispid = get_dispid_by_name(dispatch, name)?;
    let vtbl = *(dispatch.cast::<*const IDispatchVtbl>());
    let mut params = windows_sys::Win32::System::Com::DISPPARAMS {
        rgvarg: std::ptr::null_mut(),
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 0,
        cNamedArgs: 0,
    };
    let hr = ((*vtbl).invoke)(
        dispatch,
        dispid,
        &IID_NULL,
        0,
        DISPATCH_METHOD,
        &mut params,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    if hr < 0 {
        Err(format!("Invoke({name}) failed: 0x{:08X}", hr as u32))
    } else {
        Ok(())
    }
}

unsafe fn get_dispid_by_name(dispatch: *mut std::ffi::c_void, name: &str) -> Result<i32, String> {
    let vtbl = *(dispatch.cast::<*const IDispatchVtbl>());
    let wide = wide_null(name);
    let name_ptr = wide.as_ptr();
    let mut dispid = 0i32;
    let hr = ((*vtbl).get_ids_of_names)(dispatch, &IID_NULL, &name_ptr, 1, 0, &mut dispid);
    if hr < 0 {
        Err(format!("GetIDsOfNames({name}) failed: 0x{:08X}", hr as u32))
    } else {
        Ok(dispid)
    }
}

unsafe fn invoke_dispatch_object_method_result(
    dispatch: *mut std::ffi::c_void,
    dispid: i32,
) -> Result<*mut std::ffi::c_void, String> {
    let vtbl = *(dispatch.cast::<*const IDispatchVtbl>());
    let mut params = windows_sys::Win32::System::Com::DISPPARAMS {
        rgvarg: std::ptr::null_mut(),
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 0,
        cNamedArgs: 0,
    };
    let mut result: VARIANT = std::mem::zeroed();
    let hr = ((*vtbl).invoke)(
        dispatch,
        dispid,
        &IID_NULL,
        0,
        DISPATCH_METHOD,
        &mut params,
        &mut result,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    if hr < 0 {
        VariantClear(&mut result);
        return Err(format!(
            "Invoke(object-return method) failed: 0x{:08X}",
            hr as u32
        ));
    }
    if result.Anonymous.Anonymous.vt == VT_DISPATCH {
        let returned = result
            .Anonymous
            .Anonymous
            .Anonymous
            .pdispVal
            .cast::<std::ffi::c_void>();
        if returned.is_null() {
            VariantClear(&mut result);
            return Err("Invoke(object-return method) returned null VT_DISPATCH".to_string());
        }
        result.Anonymous.Anonymous.Anonymous.pdispVal = std::ptr::null_mut();
        VariantClear(&mut result);
        return Ok(returned);
    }
    let vt = result.Anonymous.Anonymous.vt;
    VariantClear(&mut result);
    Err(format!(
        "Invoke(object-return method) returned VT {vt}, expected VT_DISPATCH"
    ))
}

unsafe fn invoke_i4_method_object_result(
    dispatch: *mut std::ffi::c_void,
    dispid: i32,
    object_arg: *mut std::ffi::c_void,
) -> Result<i32, String> {
    let vtbl = *(dispatch.cast::<*const IDispatchVtbl>());
    let mut arg: VARIANT = std::mem::zeroed();
    arg.Anonymous.Anonymous.vt = VT_DISPATCH;
    add_ref_unknown(object_arg);
    arg.Anonymous.Anonymous.Anonymous.pdispVal = object_arg.cast();
    let mut params = windows_sys::Win32::System::Com::DISPPARAMS {
        rgvarg: &mut arg,
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 1,
        cNamedArgs: 0,
    };
    let mut result: VARIANT = std::mem::zeroed();
    let hr = ((*vtbl).invoke)(
        dispatch,
        dispid,
        &IID_NULL,
        0,
        DISPATCH_METHOD,
        &mut params,
        &mut result,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    VariantClear(&mut arg);
    if hr < 0 {
        VariantClear(&mut result);
        return Err(format!(
            "Invoke(object-argument method) failed: 0x{:08X}",
            hr as u32
        ));
    }
    let value = if result.Anonymous.Anonymous.vt == VT_I4 {
        Ok(result.Anonymous.Anonymous.Anonymous.lVal)
    } else {
        Err(format!(
            "Invoke(object-argument method) returned VT {}, expected VT_I4",
            result.Anonymous.Anonymous.vt
        ))
    };
    VariantClear(&mut result);
    value
}

unsafe fn invoke_i4_property_get(
    dispatch: *mut std::ffi::c_void,
    dispid: i32,
) -> Result<i32, String> {
    let vtbl = *(dispatch.cast::<*const IDispatchVtbl>());
    let mut params = windows_sys::Win32::System::Com::DISPPARAMS {
        rgvarg: std::ptr::null_mut(),
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 0,
        cNamedArgs: 0,
    };
    let mut result: VARIANT = std::mem::zeroed();
    let hr = ((*vtbl).invoke)(
        dispatch,
        dispid,
        &IID_NULL,
        0,
        DISPATCH_PROPERTYGET,
        &mut params,
        &mut result,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    if hr < 0 {
        VariantClear(&mut result);
        return Err(format!("Invoke(Value get) failed: 0x{:08X}", hr as u32));
    }
    let value = if result.Anonymous.Anonymous.vt == VT_I4 {
        Ok(result.Anonymous.Anonymous.Anonymous.lVal)
    } else {
        Err(format!(
            "Invoke(Value get) returned VT {}, expected VT_I4",
            result.Anonymous.Anonymous.vt
        ))
    };
    VariantClear(&mut result);
    value
}

unsafe fn invoke_i4_property_put(
    dispatch: *mut std::ffi::c_void,
    dispid: i32,
    value: i32,
) -> Result<(), String> {
    let vtbl = *(dispatch.cast::<*const IDispatchVtbl>());
    let mut arg: VARIANT = std::mem::zeroed();
    arg.Anonymous.Anonymous.vt = VT_I4;
    arg.Anonymous.Anonymous.Anonymous.lVal = value;
    let mut named_arg = oxvba_com::COM_DISPID_PROPERTYPUT;
    let mut params = windows_sys::Win32::System::Com::DISPPARAMS {
        rgvarg: &mut arg,
        rgdispidNamedArgs: &mut named_arg,
        cArgs: 1,
        cNamedArgs: 1,
    };
    let hr = ((*vtbl).invoke)(
        dispatch,
        dispid,
        &IID_NULL,
        0,
        DISPATCH_PROPERTYPUT,
        &mut params,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    VariantClear(&mut arg);
    if hr < 0 {
        Err(format!("Invoke(Value put) failed: 0x{:08X}", hr as u32))
    } else {
        Ok(())
    }
}

unsafe fn invoke_i4_method_pair_result(
    dispatch: *mut std::ffi::c_void,
    dispid: i32,
    left: i32,
    right: i32,
) -> Result<i32, String> {
    let vtbl = *(dispatch.cast::<*const IDispatchVtbl>());
    let mut args: [VARIANT; 2] = [std::mem::zeroed(), std::mem::zeroed()];
    args[0].Anonymous.Anonymous.vt = VT_I4;
    args[0].Anonymous.Anonymous.Anonymous.lVal = right;
    args[1].Anonymous.Anonymous.vt = VT_I4;
    args[1].Anonymous.Anonymous.Anonymous.lVal = left;
    let mut params = windows_sys::Win32::System::Com::DISPPARAMS {
        rgvarg: args.as_mut_ptr(),
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 2,
        cNamedArgs: 0,
    };
    let mut result: VARIANT = std::mem::zeroed();
    let hr = ((*vtbl).invoke)(
        dispatch,
        dispid,
        &IID_NULL,
        0,
        DISPATCH_METHOD,
        &mut params,
        &mut result,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    for arg in &mut args {
        VariantClear(arg);
    }
    if hr < 0 {
        VariantClear(&mut result);
        return Err(format!("Invoke(AddPair) failed: 0x{:08X}", hr as u32));
    }
    let value = if result.Anonymous.Anonymous.vt == VT_I4 {
        Ok(result.Anonymous.Anonymous.Anonymous.lVal)
    } else {
        Err(format!(
            "Invoke(AddPair) returned VT {}, expected VT_I4",
            result.Anonymous.Anonymous.vt
        ))
    };
    VariantClear(&mut result);
    value
}

unsafe fn invoke_r8_method_pair_result(
    dispatch: *mut std::ffi::c_void,
    dispid: i32,
    left: f64,
    right: f64,
) -> Result<f64, String> {
    let vtbl = *(dispatch.cast::<*const IDispatchVtbl>());
    let mut args: [VARIANT; 2] = [std::mem::zeroed(), std::mem::zeroed()];
    args[0].Anonymous.Anonymous.vt = VT_R8;
    args[0].Anonymous.Anonymous.Anonymous.dblVal = right;
    args[1].Anonymous.Anonymous.vt = VT_R8;
    args[1].Anonymous.Anonymous.Anonymous.dblVal = left;
    let mut params = windows_sys::Win32::System::Com::DISPPARAMS {
        rgvarg: args.as_mut_ptr(),
        rgdispidNamedArgs: std::ptr::null_mut(),
        cArgs: 2,
        cNamedArgs: 0,
    };
    let mut result: VARIANT = std::mem::zeroed();
    let hr = ((*vtbl).invoke)(
        dispatch,
        dispid,
        &IID_NULL,
        0,
        DISPATCH_METHOD,
        &mut params,
        &mut result,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    for arg in &mut args {
        VariantClear(arg);
    }
    if hr < 0 {
        VariantClear(&mut result);
        return Err(format!("Invoke(Average) failed: 0x{:08X}", hr as u32));
    }
    let value = if result.Anonymous.Anonymous.vt == VT_R8 {
        Ok(result.Anonymous.Anonymous.Anonymous.dblVal)
    } else {
        Err(format!(
            "Invoke(Average) returned VT {}, expected VT_R8",
            result.Anonymous.Anonymous.vt
        ))
    };
    VariantClear(&mut result);
    value
}

#[repr(C)]
struct IUnknownVtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
}

#[repr(C)]
struct IDispatchVtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    get_type_info_count: unsafe extern "system" fn(*mut std::ffi::c_void, *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        u32,
        u32,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *const *const u16,
        u32,
        u32,
        *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        i32,
        *const GUID,
        u32,
        u16,
        *mut windows_sys::Win32::System::Com::DISPPARAMS,
        *mut VARIANT,
        *mut windows_sys::Win32::System::Com::EXCEPINFO,
        *mut u32,
    ) -> i32,
}

#[repr(C)]
struct ITypeInfoVtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    get_type_attr: unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut TYPEATTR) -> i32,
    get_type_comp:
        unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> i32,
    get_func_desc:
        unsafe extern "system" fn(*mut std::ffi::c_void, u32, *mut *mut std::ffi::c_void) -> i32,
    get_var_desc:
        unsafe extern "system" fn(*mut std::ffi::c_void, u32, *mut *mut std::ffi::c_void) -> i32,
    get_names:
        unsafe extern "system" fn(*mut std::ffi::c_void, i32, *mut *mut u16, u32, *mut u32) -> i32,
    get_ref_type_of_impl_type:
        unsafe extern "system" fn(*mut std::ffi::c_void, u32, *mut u32) -> i32,
    get_impl_type_flags: unsafe extern "system" fn(*mut std::ffi::c_void, u32, *mut i32) -> i32,
    get_ids_of_names:
        unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut u16, u32, *mut i32) -> i32,
    invoke: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
        i32,
        u16,
        *mut windows_sys::Win32::System::Com::DISPPARAMS,
        *mut VARIANT,
        *mut windows_sys::Win32::System::Com::EXCEPINFO,
        *mut u32,
    ) -> i32,
    get_documentation: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        i32,
        *mut *mut u16,
        *mut *mut u16,
        *mut u32,
        *mut *mut u16,
    ) -> i32,
    get_dll_entry: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        i32,
        u16,
        *mut *mut u16,
        *mut *mut u16,
        *mut u16,
    ) -> i32,
    get_ref_type_info:
        unsafe extern "system" fn(*mut std::ffi::c_void, u32, *mut *mut std::ffi::c_void) -> i32,
    address_of_member: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        i32,
        u16,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    create_instance: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    get_mops: unsafe extern "system" fn(*mut std::ffi::c_void, i32, *mut *mut u16) -> i32,
    get_containing_type_lib: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *mut *mut std::ffi::c_void,
        *mut u32,
    ) -> i32,
    release_type_attr: unsafe extern "system" fn(*mut std::ffi::c_void, *mut TYPEATTR),
    release_func_desc: unsafe extern "system" fn(*mut std::ffi::c_void, *mut std::ffi::c_void),
    release_var_desc: unsafe extern "system" fn(*mut std::ffi::c_void, *mut std::ffi::c_void),
}

#[repr(C)]
struct BoundedDualVtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    get_type_info_count: unsafe extern "system" fn(*mut std::ffi::c_void, *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        u32,
        u32,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *const *const u16,
        u32,
        u32,
        *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        i32,
        *const GUID,
        u32,
        u16,
        *mut windows_sys::Win32::System::Com::DISPPARAMS,
        *mut VARIANT,
        *mut windows_sys::Win32::System::Com::EXCEPINFO,
        *mut u32,
    ) -> i32,
    slot0: unsafe extern "system" fn(*mut std::ffi::c_void, *mut i32) -> i32,
    slot1: unsafe extern "system" fn(*mut std::ffi::c_void, i32, i32, *mut i32) -> i32,
    slot2: unsafe extern "system" fn(*mut std::ffi::c_void, f64, f64, *mut f64) -> i32,
}

#[repr(C)]
struct LongPropertyDualVtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    get_type_info_count: unsafe extern "system" fn(*mut std::ffi::c_void, *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        u32,
        u32,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *const *const u16,
        u32,
        u32,
        *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        i32,
        *const GUID,
        u32,
        u16,
        *mut windows_sys::Win32::System::Com::DISPPARAMS,
        *mut VARIANT,
        *mut windows_sys::Win32::System::Com::EXCEPINFO,
        *mut u32,
    ) -> i32,
    slot0: unsafe extern "system" fn(*mut std::ffi::c_void, *mut i32) -> i32,
    slot1: unsafe extern "system" fn(*mut std::ffi::c_void, i32) -> i32,
}

#[repr(C)]
struct ObjectReturnDualVtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    get_type_info_count: unsafe extern "system" fn(*mut std::ffi::c_void, *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        u32,
        u32,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *const *const u16,
        u32,
        u32,
        *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        i32,
        *const GUID,
        u32,
        u16,
        *mut windows_sys::Win32::System::Com::DISPPARAMS,
        *mut VARIANT,
        *mut windows_sys::Win32::System::Com::EXCEPINFO,
        *mut u32,
    ) -> i32,
    slot0: unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> i32,
    slot1: unsafe extern "system" fn(*mut std::ffi::c_void, *mut i32) -> i32,
}

#[repr(C)]
struct ObjectArgumentDualVtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    get_type_info_count: unsafe extern "system" fn(*mut std::ffi::c_void, *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        u32,
        u32,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *const *const u16,
        u32,
        u32,
        *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        i32,
        *const GUID,
        u32,
        u16,
        *mut windows_sys::Win32::System::Com::DISPPARAMS,
        *mut VARIANT,
        *mut windows_sys::Win32::System::Com::EXCEPINFO,
        *mut u32,
    ) -> i32,
    slot0: unsafe extern "system" fn(*mut std::ffi::c_void, *mut i32) -> i32,
    slot1: unsafe extern "system" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, *mut i32) -> i32,
}

#[repr(C)]
struct IdtExtensibility2Vtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    get_type_info_count: unsafe extern "system" fn(*mut std::ffi::c_void, *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        u32,
        u32,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *const *const u16,
        u32,
        u32,
        *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        i32,
        *const GUID,
        u32,
        u16,
        *mut windows_sys::Win32::System::Com::DISPPARAMS,
        *mut VARIANT,
        *mut windows_sys::Win32::System::Com::EXCEPINFO,
        *mut u32,
    ) -> i32,
    on_connection: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *mut std::ffi::c_void,
        i32,
        *mut std::ffi::c_void,
        *mut *mut SAFEARRAY,
    ) -> i32,
    on_disconnection:
        unsafe extern "system" fn(*mut std::ffi::c_void, i32, *mut *mut SAFEARRAY) -> i32,
    on_addins_update: unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut SAFEARRAY) -> i32,
    on_startup_complete:
        unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut SAFEARRAY) -> i32,
    on_begin_shutdown: unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut SAFEARRAY) -> i32,
}

#[repr(C)]
struct RtdServerVtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    get_type_info_count: unsafe extern "system" fn(*mut std::ffi::c_void, *mut u32) -> i32,
    get_type_info: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        u32,
        u32,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    get_ids_of_names: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *const *const u16,
        u32,
        u32,
        *mut i32,
    ) -> i32,
    invoke: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        i32,
        *const GUID,
        u32,
        u16,
        *mut windows_sys::Win32::System::Com::DISPPARAMS,
        *mut VARIANT,
        *mut windows_sys::Win32::System::Com::EXCEPINFO,
        *mut u32,
    ) -> i32,
    server_start:
        unsafe extern "system" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, *mut i32) -> i32,
    connect_data: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        i32,
        *mut SAFEARRAY,
        *mut i16,
        *mut VARIANT,
    ) -> i32,
    refresh_data:
        unsafe extern "system" fn(*mut std::ffi::c_void, *mut i32, *mut *mut SAFEARRAY) -> i32,
    disconnect_data: unsafe extern "system" fn(*mut std::ffi::c_void, i32) -> i32,
    heartbeat: unsafe extern "system" fn(*mut std::ffi::c_void, *mut i32) -> i32,
    server_terminate: unsafe extern "system" fn(*mut std::ffi::c_void) -> i32,
}

#[repr(C)]
struct IConnectionPointContainerVtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    enum_connection_points:
        unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> i32,
    find_connection_point: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
}

#[repr(C)]
struct IConnectionPointVtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    get_connection_interface: unsafe extern "system" fn(*mut std::ffi::c_void, *mut GUID) -> i32,
    get_connection_point_container:
        unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> i32,
    advise:
        unsafe extern "system" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, *mut u32) -> i32,
    unadvise: unsafe extern "system" fn(*mut std::ffi::c_void, u32) -> i32,
    enum_connections:
        unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> i32,
}

#[repr(C)]
struct IEnumConnectionPointsVtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    next: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        u32,
        *mut *mut std::ffi::c_void,
        *mut u32,
    ) -> i32,
    skip: unsafe extern "system" fn(*mut std::ffi::c_void, u32) -> i32,
    reset: unsafe extern "system" fn(*mut std::ffi::c_void) -> i32,
    clone: unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> i32,
}

#[repr(C)]
struct IEnumConnectionsVtbl {
    query_interface: unsafe extern "system" fn(
        *mut std::ffi::c_void,
        *const GUID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(*mut std::ffi::c_void) -> u32,
    next: unsafe extern "system" fn(*mut std::ffi::c_void, u32, *mut ConnectData, *mut u32) -> i32,
    skip: unsafe extern "system" fn(*mut std::ffi::c_void, u32) -> i32,
    reset: unsafe extern "system" fn(*mut std::ffi::c_void) -> i32,
    clone: unsafe extern "system" fn(*mut std::ffi::c_void, *mut *mut std::ffi::c_void) -> i32,
}

#[repr(C)]
struct ConnectData {
    p_unk: *mut std::ffi::c_void,
    cookie: u32,
}

#[repr(C)]
struct TestSink {
    vtbl: *const IDispatchVtbl,
    ref_count: AtomicU32,
    seen: SeenEvents,
}

impl TestSink {
    unsafe fn new(seen: SeenEvents) -> *mut Self {
        Box::into_raw(Box::new(Self {
            vtbl: &TEST_SINK_VTBL,
            ref_count: AtomicU32::new(1),
            seen,
        }))
    }
}

static TEST_SINK_VTBL: IDispatchVtbl = IDispatchVtbl {
    query_interface: sink_query_interface,
    add_ref: sink_add_ref,
    release: sink_release,
    get_type_info_count: sink_get_type_info_count,
    get_type_info: sink_get_type_info,
    get_ids_of_names: sink_get_ids_of_names,
    invoke: sink_invoke,
};

unsafe extern "system" fn sink_query_interface(
    this: *mut std::ffi::c_void,
    riid: *const GUID,
    out: *mut *mut std::ffi::c_void,
) -> i32 {
    if this.is_null() || riid.is_null() || out.is_null() {
        return 0x8000_4003u32 as i32;
    }
    *out = std::ptr::null_mut();
    if guid_eq(&*riid, &IID_IUNKNOWN) || guid_eq(&*riid, &IID_IDISPATCH) {
        sink_add_ref(this);
        *out = this;
        0
    } else {
        0x8000_4002u32 as i32
    }
}

unsafe extern "system" fn sink_add_ref(this: *mut std::ffi::c_void) -> u32 {
    let sink = this.cast::<TestSink>();
    (*sink).ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn sink_release(this: *mut std::ffi::c_void) -> u32 {
    let sink = this.cast::<TestSink>();
    let previous = (*sink).ref_count.fetch_sub(1, Ordering::Release);
    let remaining = previous.saturating_sub(1);
    if remaining == 0 {
        fence(Ordering::Acquire);
        drop(Box::from_raw(sink));
    }
    remaining
}

unsafe extern "system" fn sink_get_type_info_count(
    _this: *mut std::ffi::c_void,
    count: *mut u32,
) -> i32 {
    if !count.is_null() {
        *count = 0;
    }
    0
}

unsafe extern "system" fn sink_get_type_info(
    _this: *mut std::ffi::c_void,
    _index: u32,
    _lcid: u32,
    out: *mut *mut std::ffi::c_void,
) -> i32 {
    if !out.is_null() {
        *out = std::ptr::null_mut();
    }
    0x8000_4001u32 as i32
}

unsafe extern "system" fn sink_get_ids_of_names(
    _this: *mut std::ffi::c_void,
    _riid: *const GUID,
    _names: *const *const u16,
    _name_count: u32,
    _lcid: u32,
    _dispids: *mut i32,
) -> i32 {
    0x8002_0006u32 as i32
}

unsafe extern "system" fn sink_invoke(
    this: *mut std::ffi::c_void,
    dispid: i32,
    _riid: *const GUID,
    _lcid: u32,
    _flags: u16,
    params: *mut windows_sys::Win32::System::Com::DISPPARAMS,
    _result: *mut VARIANT,
    _excep_info: *mut windows_sys::Win32::System::Com::EXCEPINFO,
    _arg_err: *mut u32,
) -> i32 {
    let value = if params.is_null() || (*params).cArgs == 0 || (*params).rgvarg.is_null() {
        None
    } else {
        let variant = &*(*params).rgvarg;
        if variant.Anonymous.Anonymous.vt == VT_I4 {
            Some(variant.Anonymous.Anonymous.Anonymous.lVal)
        } else {
            None
        }
    };
    (*this.cast::<TestSink>())
        .seen
        .lock()
        .expect("seen lock")
        .push((dispid, value));
    0
}

unsafe fn query_interface(
    ptr: *mut std::ffi::c_void,
    iid: &GUID,
    out: *mut *mut std::ffi::c_void,
) -> i32 {
    let vtbl = *(ptr.cast::<*const IUnknownVtbl>());
    ((*vtbl).query_interface)(ptr, iid, out)
}

unsafe fn add_ref_unknown(ptr: *mut std::ffi::c_void) {
    let vtbl = *(ptr.cast::<*const IUnknownVtbl>());
    ((*vtbl).add_ref)(ptr);
}

unsafe fn release_unknown(ptr: *mut std::ffi::c_void) {
    let vtbl = *(ptr.cast::<*const IUnknownVtbl>());
    ((*vtbl).release)(ptr);
}

fn parse_guid(text: &str) -> Result<GUID, String> {
    let text = text.trim_matches(|ch| ch == '{' || ch == '}');
    let parts: Vec<&str> = text.split('-').collect();
    if parts.len() != 5 || parts[3].len() != 4 || parts[4].len() != 12 {
        return Err(format!("invalid GUID `{text}`"));
    }
    let data1 = u32::from_str_radix(parts[0], 16).map_err(|err| err.to_string())?;
    let data2 = u16::from_str_radix(parts[1], 16).map_err(|err| err.to_string())?;
    let data3 = u16::from_str_radix(parts[2], 16).map_err(|err| err.to_string())?;
    let tail = format!("{}{}", parts[3], parts[4]);
    let mut data4 = [0u8; 8];
    for index in 0..8 {
        data4[index] = u8::from_str_radix(&tail[index * 2..index * 2 + 2], 16)
            .map_err(|err| err.to_string())?;
    }
    Ok(GUID {
        data1,
        data2,
        data3,
        data4,
    })
}

fn guid_eq(left: &GUID, right: &GUID) -> bool {
    left.data1 == right.data1
        && left.data2 == right.data2
        && left.data3 == right.data3
        && left.data4 == right.data4
}

fn guid_to_string(guid: &GUID) -> String {
    format!(
        "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7],
    )
}

const S_FALSE: i32 = 1;
const DISP_E_BADINDEX: i32 = 0x8002_000Bu32 as i32;

const IID_IUNKNOWN: GUID = GUID {
    data1: 0x0000_0000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};
const IID_IDISPATCH: GUID = GUID {
    data1: 0x0002_0400,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};
const IID_ICONNECTIONPOINTCONTAINER: GUID = GUID {
    data1: 0xB196_B284,
    data2: 0xBAB4,
    data3: 0x101A,
    data4: [0xB6, 0x9C, 0x00, 0xAA, 0x00, 0x34, 0x1D, 0x07],
};
const IID_NULL: GUID = GUID {
    data1: 0,
    data2: 0,
    data3: 0,
    data4: [0; 8],
};

fn wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
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
