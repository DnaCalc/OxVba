#[cfg(target_os = "windows")]
mod windows_vba_attribute_oracle_lane {
    use oxvba_host::{Engine, HostConfig};
    use oxvba_project::load_basproj_from_str;
    use oxvba_runtime::{RuntimeValue, bstr::BStr};

    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "{prefix}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("unix epoch")
                .as_nanos()
        ))
    }

    fn run_project_with_widget(
        main_source: &str,
        widget_source: &str,
    ) -> Result<Vec<oxvba_runtime::RuntimeValue>, String> {
        let temp_root = unique_temp_dir("oxvba_vba_attribute_oracle");
        std::fs::create_dir_all(&temp_root).expect("create temp project root");
        std::fs::write(temp_root.join("Main.bas"), main_source).expect("write main module");
        std::fs::write(temp_root.join("Widget.cls"), widget_source).expect("write widget class");

        let loaded = load_basproj_from_str(
            "\
<Project Sdk=\"OxVba.Sdk/0.1.0\">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <ProjectName>AttributeOracleProject</ProjectName>
    <EntryPoint>Main.Main</EntryPoint>
  </PropertyGroup>
  <ItemGroup>
    <Module Include=\"Main.bas\" />
    <ClassModule Include=\"Widget.cls\" />
  </ItemGroup>
</Project>
",
            &temp_root,
        )
        .map_err(|err| err.to_string())?;

        let result = Engine::new(HostConfig {
            enable_jit: false,
            root_object_name: None,
        })
        .execute_project_with_snapshot_phased(&loaded.manifest)
        .map_err(|err| err.to_string());

        std::fs::remove_dir_all(&temp_root).expect("cleanup temp project root");
        result
    }

    fn emit_observed(case_id: &str, value: &RuntimeValue) {
        let rendered = match value {
            RuntimeValue::String(BStr(text)) => text.clone(),
            RuntimeValue::I32(n) => n.to_string(),
            other => format!("{other:?}"),
        };
        println!("ODGATTR-OBSERVED[{case_id}]={rendered}");
    }

    fn emit_observed_text(case_id: &str, value: &str) {
        println!("ODGATTR-OBSERVED[{case_id}]={value}");
    }

    #[test]
    #[ignore = "requires Windows Excel-backed oracle lane"]
    fn windows_defaultprop_vb_usermemid_zero_bare_assignment_matches_excel() {
        let values = run_project_with_widget(
            "Attribute VB_Name = \"Main\"\nPublic Sub Main()\nDim widget As New Widget\nDim valueOut\nvalueOut = widget\nEnd Sub\n",
            concat!(
                "Attribute VB_Name = \"Widget\"\n",
                "Option Explicit\n",
                "Private stored As Long\n",
                "Public Sub Class_Initialize()\n",
                "stored = 41\n",
                "End Sub\n",
                "Public Property Get Value() As Long\n",
                "Value = stored + 1\n",
                "End Property\n",
                "Attribute Value.VB_UserMemId = 0\n"
            ),
        )
        .expect("attribute oracle project should execute");
        emit_observed("CCT-049-DEFAULTPROP-001", &values[0]);
        assert_eq!(values[0], RuntimeValue::I32(42));
    }

    #[test]
    #[ignore = "requires Windows Excel-backed oracle lane"]
    fn windows_newenum_vb_usermemid_minus4_for_each_matches_excel() {
        let result = run_project_with_widget(
            concat!(
                "Attribute VB_Name = \"Main\"\n",
                "Public Sub Main()\n",
                "Dim widget As New Widget\n",
                "Dim item\n",
                "Dim valueOut\n",
                "For Each item In widget\n",
                "    valueOut = valueOut & CStr(item) & \",\"\n",
                "Next item\n",
                "End Sub\n"
            ),
            concat!(
                "Attribute VB_Name = \"Widget\"\n",
                "Option Explicit\n",
                "Private items As New Collection\n",
                "Public Sub Class_Initialize()\n",
                "items.Add 41\n",
                "items.Add 42\n",
                "End Sub\n",
                "Public Property Get NewEnum() As IUnknown\n",
                "Set NewEnum = items.[_NewEnum]\n",
                "End Property\n",
                "Attribute NewEnum.VB_UserMemId = -4\n",
                "Attribute NewEnum.VB_MemberFlags = \"40\"\n"
            ),
        );

        match result {
            Ok(values) => {
                emit_observed("CCT-050-NEWENUM-001", &values[0]);
                assert_eq!(values[0], RuntimeValue::String(BStr("41,42,".to_string())));
            }
            Err(err) => {
                emit_observed_text("CCT-050-NEWENUM-001", &format!("error:{err}"));
                panic!("NewEnum oracle project should execute: {err}");
            }
        }
    }
}
