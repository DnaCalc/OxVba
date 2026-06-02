#[cfg(target_os = "windows")]
mod windows_excel_office_oracle_lane {
    use oxvba_hal::model::HostPolicy;
    use oxvba_host::{Engine, HostConfig};

    const EXCEL_ACTIVATION_SMOKE: &str = include_str!(
        "../../../conformance/com/office/excel/excel_application_activation_smoke.bas"
    );
    const EXCEL_WORKBOOK_RANGE_SMOKE: &str =
        include_str!("../../../conformance/com/office/excel/excel_workbook_range_smoke.bas");
    const EXCEL_DISPATCHINVOKE_RANGE_SMOKE: &str =
        include_str!("../../../conformance/com/office/excel/excel_dispatchinvoke_range_smoke.bas");
    const EXCEL_NAMED_ARGUMENT_SMOKE: &str =
        include_str!("../../../conformance/com/office/excel/excel_named_argument_smoke.bas");
    const EXCEL_FIND_NULL_RESULT_SMOKE: &str =
        include_str!("../../../conformance/com/office/excel/excel_find_null_result_smoke.bas");

    fn execute_with_host_policy(source: &str) -> Result<(), String> {
        let mut engine = Engine::new(HostConfig { enable_jit: false });
        engine.set_host_policy(HostPolicy::interactive_dev());
        engine
            .execute_source_with_variant_snapshot_phased(source)
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    fn excel_application_available() -> bool {
        execute_with_host_policy(
            "Sub Main()\nDim app\napp = CreateObject(\"Excel.Application\")\napp.Quit\nEnd Sub\n",
        )
        .is_ok()
    }

    #[test]
    #[ignore = "requires Windows Excel.Application automation"]
    fn excel_application_activation_smoke_fixture_executes_when_available() {
        if !excel_application_available() {
            eprintln!("Excel oracle lane: Excel.Application is not available in this environment");
            return;
        }

        execute_with_host_policy(EXCEL_ACTIVATION_SMOKE)
            .expect("Excel activation smoke fixture should execute");
    }

    #[test]
    #[ignore = "requires Windows Excel.Application automation"]
    fn excel_workbook_range_object_smoke_fixture_executes_when_available() {
        if !excel_application_available() {
            eprintln!("Excel oracle lane: Excel.Application is not available in this environment");
            return;
        }

        execute_with_host_policy(EXCEL_WORKBOOK_RANGE_SMOKE)
            .expect("Excel workbook/range object smoke fixture should execute");
    }

    #[test]
    #[ignore = "requires Windows Excel.Application automation"]
    fn excel_dispatchinvoke_range_smoke_fixture_executes_when_available() {
        if !excel_application_available() {
            eprintln!("Excel oracle lane: Excel.Application is not available in this environment");
            return;
        }

        execute_with_host_policy(EXCEL_DISPATCHINVOKE_RANGE_SMOKE)
            .expect("Excel DispatchInvoke range smoke fixture should execute");
    }

    #[test]
    #[ignore = "requires Windows Excel.Application automation"]
    fn excel_named_argument_smoke_fixture_executes_when_available() {
        if !excel_application_available() {
            eprintln!("Excel oracle lane: Excel.Application is not available in this environment");
            return;
        }

        execute_with_host_policy(EXCEL_NAMED_ARGUMENT_SMOKE)
            .expect("Excel named-argument smoke fixture should execute");
    }

    #[test]
    #[ignore = "requires Windows Excel.Application automation"]
    fn excel_find_null_result_smoke_fixture_executes_when_available() {
        if !excel_application_available() {
            eprintln!("Excel oracle lane: Excel.Application is not available in this environment");
            return;
        }

        execute_with_host_policy(EXCEL_FIND_NULL_RESULT_SMOKE)
            .expect("Excel Find null-result smoke fixture should execute");
    }
}
