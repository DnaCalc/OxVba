mod _shared;

#[test]
fn fixture_thin_slice_has_expected_statement_lines() {
    let text = _shared::read_fixture("thin_slice", "Module1.bas");
    assert_eq!("    Dim answer As Long", _shared::line_at(&text, 6));
    assert_eq!("    answer = 42", _shared::line_at(&text, 7));
    assert_eq!("    Debug.Print answer", _shared::line_at(&text, 8));
}

#[test]
fn fixture_multi_module_walkthrough_loads() {
    assert!(_shared::fixture_file("multi_module_walkthrough", "Module1.bas").exists());
    assert!(_shared::fixture_file("multi_module_walkthrough", "Module2.bas").exists());
    assert!(
        _shared::read_fixture("multi_module_walkthrough", "Module1.bas").contains("HelperValue")
    );
}

#[test]
fn fixture_bare_no_preamble_loads() {
    let text = _shared::read_fixture("bare_no_preamble", "Module1.bas");
    assert!(text.starts_with("Public Sub Main()"));
}

#[test]
fn fixture_com_dispatch_smoke_declared() {
    let text = _shared::read_fixture("com_dispatch_smoke", "Module1.bas");
    assert!(text.contains("CreateObject"));
    assert!(_shared::fixture_file("com_dispatch_smoke", "project.basproj").exists());
}
