//! vm3 coverage for `GetSetting`/`SaveSetting`/`GetAllSettings`/`DeleteSetting`.

use oxvba_differential::{Canon, Executor, run};

fn snapshot(body: &str) -> Vec<Canon> {
    let source = format!("Sub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}",
        outcome.unsupported
    );
    outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 run failed: {err}\n{body}"))
}

fn assert_contains_str(values: &[Canon], expected: &str) {
    assert!(
        values.contains(&Canon::Str(expected.to_string())),
        "missing {expected:?} in {values:?}"
    );
}

#[test]
fn get_setting_missing_values_return_empty_or_default() {
    let snap = snapshot(
        "    Dim emptyDefault As String, explicitDefault As String\n\
             emptyDefault = \"[\" & GetSetting(\"OxVbaVm3\", \"Startup\", \"Left\") & \"]\"\n\
             explicitDefault = GetSetting(\"OxVbaVm3\", \"Startup\", \"Left\", \"fallback\")",
    );
    assert_contains_str(&snap, "[]");
    assert_contains_str(&snap, "fallback");
}

#[test]
fn save_setting_round_trips_as_string_and_case_insensitive() {
    let snap = snapshot(
        "    Dim saved As String\n\
             SaveSetting \"OxVbaVm3\", \"Startup\", \"Top\", 75\n\
             saved = GetSetting(\"oxvbavm3\", \"startup\", \"top\", \"fallback\")",
    );
    assert_contains_str(&snap, "75");
}

#[test]
fn get_all_settings_returns_two_column_bstr_array() {
    let snap = snapshot(
        "    Dim settings As Variant, summary As String\n\
             SaveSetting \"OxVbaVm3\", \"Startup\", \"Top\", 75\n\
             SaveSetting \"OxVbaVm3\", \"Startup\", \"Left\", \"50\"\n\
             settings = GetAllSettings(\"OxVbaVm3\", \"Startup\")\n\
             summary = CStr(LBound(settings, 1)) & \":\" & CStr(UBound(settings, 1)) & \":\" & _\n\
                       CStr(LBound(settings, 2)) & \":\" & CStr(UBound(settings, 2)) & \":\" & _\n\
                       CStr(settings(0, 0)) & \"=\" & CStr(settings(0, 1)) & \":\" & _\n\
                       CStr(settings(1, 0)) & \"=\" & CStr(settings(1, 1))",
    );
    assert_contains_str(&snap, "0:1:0:1:Left=50:Top=75");
}

#[test]
fn delete_setting_deletes_keys_and_sections() {
    let snap = snapshot(
        "    Dim afterKey As String, afterSection As String\n\
             SaveSetting \"OxVbaVm3\", \"Startup\", \"Top\", \"75\"\n\
             SaveSetting \"OxVbaVm3\", \"Startup\", \"Left\", \"50\"\n\
             DeleteSetting \"OxVbaVm3\", \"Startup\", \"Left\"\n\
             afterKey = GetSetting(\"OxVbaVm3\", \"Startup\", \"Left\", \"fallback\")\n\
             DeleteSetting \"OxVbaVm3\", \"Startup\"\n\
             afterSection = CStr(IsEmpty(GetAllSettings(\"OxVbaVm3\", \"Startup\")))",
    );
    assert_contains_str(&snap, "fallback");
    assert_contains_str(&snap, "True");
}

#[test]
fn delete_setting_missing_section_raises_error_five() {
    let snap = snapshot(
        "    Dim n As String\n\
             On Error Resume Next\n\
             DeleteSetting \"OxVbaVm3\", \"Missing\"\n\
             n = CStr(Err.Number)",
    );
    assert_contains_str(&snap, "5");
}
