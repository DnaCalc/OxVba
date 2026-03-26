#![cfg(target_os = "windows")]

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use oxvba_hal::{
    callbacks::HostCallbacks,
    model::HostPolicy,
    project::{
        HostExtensionModuleChange, ProjectCallbackResult, ProjectDescriptor,
        ProjectDescriptorKind, ProjectReferenceDescriptor,
    },
};
use oxvba_host::{Engine, HostConfig};

struct ExcelVbideHostCallbacks {
    workbook_path: PathBuf,
    project_name: String,
}

impl ExcelVbideHostCallbacks {
    fn new(workbook_path: PathBuf, project_name: impl Into<String>) -> Self {
        Self {
            workbook_path,
            project_name: project_name.into(),
        }
    }

    fn workbook_path_str(&self) -> String {
        self.workbook_path.to_string_lossy().into_owned()
    }

    fn run_ps(
        &self,
        action: &str,
        module_name: Option<&str>,
        source: Option<&str>,
    ) -> Result<String, String> {
        fn quote(value: &str) -> String {
            format!("'{}'", value.replace('\'', "''"))
        }

        let module_expr = module_name
            .map(quote)
            .unwrap_or_else(|| "$null".to_string());
        let source_expr = source.map(quote).unwrap_or_else(|| "$null".to_string());
        let command = format!(
            r#"
$ErrorActionPreference = "Stop"
$Action = {action}
$WorkbookPath = {workbook_path}
$ModuleName = {module_name}
$Source = {source}
$excel = New-Object -ComObject Excel.Application
$excel.Visible = $false
$excel.DisplayAlerts = $false
$wb = $null
try {{
    if ($Action -eq "create") {{
        $wb = $excel.Workbooks.Add()
        $wb.VBProject.Name = "Workbook"
        $wb.SaveAs($WorkbookPath, 52)
        "ok"
        return
    }}

    $wb = $excel.Workbooks.Open($WorkbookPath)
    switch ($Action) {{
        "attach" {{
            $component = $wb.VBProject.VBComponents.Item($ModuleName)
            $codeModule = $component.CodeModule
            if ($codeModule.CountOfLines -gt 0) {{
                $codeModule.DeleteLines(1, $codeModule.CountOfLines)
            }}
            [void]$codeModule.AddFromString($Source)
            $wb.Save()
            "ok"
        }}
        "read" {{
            $component = $wb.VBProject.VBComponents.Item($ModuleName)
            $codeModule = $component.CodeModule
            if ($codeModule.CountOfLines -gt 0) {{
                $codeModule.Lines(1, $codeModule.CountOfLines)
            }}
        }}
        default {{
            throw "unknown action: $Action"
        }}
    }}
}}
finally {{
    if ($wb -ne $null) {{
        $wb.Close($false)
        [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($wb)
    }}
    $excel.Quit()
    [void][System.Runtime.InteropServices.Marshal]::ReleaseComObject($excel)
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
}}
"#,
            action = quote(action),
            workbook_path = quote(&self.workbook_path_str()),
            module_name = module_expr,
            source = source_expr
        );

        let mut cmd = Command::new("pwsh");
        cmd.args([
            "-NoLogo",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &command,
        ]);
        let output = cmd.output().map_err(|err| err.to_string())?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }

    fn create_workbook(&self) -> Result<(), String> {
        self.run_ps("create", None, None).map(|_| ())
    }

    fn read_module_source(&self, module_name: &str) -> Result<String, String> {
        self.run_ps("read", Some(module_name), None)
    }
}

impl HostCallbacks for ExcelVbideHostCallbacks {
    fn on_msg_box(&self, _prompt: &str, style: i32) -> i32 {
        style.max(1)
    }

    fn on_input_box(&self, _prompt: &str, default: &str) -> String {
        default.to_string()
    }

    fn on_status_bar(&self, _text: &str) {}

    fn on_debug_print(&self, _text: &str) {}

    fn supports_project_catalog(&self) -> bool {
        true
    }

    fn supports_project_references(&self) -> bool {
        true
    }

    fn supports_project_mutation(&self) -> bool {
        true
    }

    fn on_list_projects(&self) -> ProjectCallbackResult<Vec<ProjectDescriptor>> {
        Ok(vec![ProjectDescriptor {
            project_name: self.project_name.clone(),
            kind: ProjectDescriptorKind::Host,
            supports_extension_modules: true,
        }])
    }

    fn on_get_project(&self, project_name: &str) -> ProjectCallbackResult<ProjectDescriptor> {
        Ok(ProjectDescriptor {
            project_name: project_name.to_string(),
            kind: ProjectDescriptorKind::Host,
            supports_extension_modules: true,
        })
    }

    fn on_list_project_references(
        &self,
        project_name: &str,
    ) -> ProjectCallbackResult<Vec<ProjectReferenceDescriptor>> {
        Ok(vec![ProjectReferenceDescriptor {
            project_name: project_name.to_string(),
            referenced_name: "VBA".to_string(),
            kind: oxvba_hal::project::ProjectReferenceKind::TypeLibrary,
        }])
    }

    fn on_attach_host_extension_module(
        &self,
        change: &HostExtensionModuleChange,
    ) -> ProjectCallbackResult<()> {
        self.run_ps("attach", Some(&change.module_name), Some(&change.source))
            .map(|_| ())
            .map_err(|message| oxvba_hal::project::ProjectCallbackError::AdapterFault {
                message,
            })
    }
}

fn unique_workbook_path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("oxvba_host_project_{stamp}.xlsm"))
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

#[test]
#[ignore = "requires Windows Excel with AccessVBOM enabled"]
fn excel_vbide_host_callbacks_attach_source_to_thisworkbook() {
    let workbook_path = unique_workbook_path();
    let callbacks = Arc::new(ExcelVbideHostCallbacks::new(
        workbook_path.clone(),
        "Workbook",
    ));
    callbacks
        .create_workbook()
        .expect("workbook creation should succeed");

    let mut engine = Engine::new(HostConfig::default()).with_host_callbacks(callbacks.clone());
    engine.set_host_policy(HostPolicy::interactive_dev());

    let host = engine.host_services();
    let projects = host
        .project_catalog()
        .expect("project catalog should be exposed")
        .list_projects()
        .expect("project list should succeed");
    assert_eq!(projects[0].project_name, "Workbook");

    let refs = host
        .project_references()
        .expect("project references should be exposed")
        .list_references("Workbook")
        .expect("reference list should succeed");
    assert_eq!(refs[0].referenced_name, "VBA");

    let source = "Public Sub Sync()\nEnd Sub";
    host.project_mutation()
        .expect("project mutation should be exposed")
        .attach_host_extension_module(&HostExtensionModuleChange {
            project_name: "Workbook".to_string(),
            module_name: "ThisWorkbook".to_string(),
            source: source.to_string(),
        })
        .expect("host extension attach should succeed");

    let observed = callbacks
        .read_module_source("ThisWorkbook")
        .expect("code should be readable");
    assert!(
        observed.contains("Public Sub Sync()"),
        "expected ThisWorkbook code module to contain attached source, got: {observed}"
    );

    cleanup(&workbook_path);
}
