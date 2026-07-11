use oxvba_differential::balance_protocol::{
    ALL_BALANCE_FIXTURES, BalanceFixtureReport, POLICY_ERROR_BALANCE_FIXTURE,
};
use oxvba_differential::{Executor, RunOutcome, run, run_modules, run_with_project};
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

fn main() {
    let fixture = match parse_fixture_arg() {
        Ok(fixture) => fixture,
        Err(message) => exit_with_error(&message),
    };
    let outcome = match run_named_fixture(&fixture) {
        Ok(outcome) => outcome,
        Err(message) => exit_with_error(&message),
    };
    let report = match BalanceFixtureReport::from_run_outcome(fixture, outcome) {
        Ok(report) => report,
        Err(message) => exit_with_error(&message),
    };
    match report.to_protocol_line() {
        Ok(line) => println!("{line}"),
        Err(message) => exit_with_error(&message),
    }
}

fn parse_fixture_arg() -> Result<String, String> {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--fixture"), Some(fixture), None) => Ok(fixture),
        _ => Err(format!(
            "usage: oxvba_balance_fixture --fixture <id>; known fixtures: {}",
            ALL_BALANCE_FIXTURES.join(", ")
        )),
    }
}

fn exit_with_error(message: &str) -> ! {
    eprintln!("balance fixture child failed: {message}");
    std::process::exit(2)
}

fn run_named_fixture(fixture: &str) -> Result<RunOutcome, String> {
    match fixture {
        "carrier-string" => Ok(run(
            Executor::Vm3,
            r#"
Public gResult As String

Sub Main()
    Dim value As String
    value = "alpha" & "-beta"
    gResult = value
End Sub
"#,
        )),
        "carrier-array" => Ok(run(
            Executor::Vm3,
            r#"
Public gResult As Long

Sub Main()
    Dim values() As Long
    ReDim values(1 To 3)
    values(1) = 10
    values(2) = 20
    values(3) = 30
    gResult = values(1) + values(2) + values(3)
End Sub
"#,
        )),
        "carrier-object" => Ok(run_modules(
            Executor::Vm3,
            &[
                (
                    "Main",
                    Procedural,
                    r#"
Public gResult As Long

Sub Main()
    Dim value As Box
    Set value = New Box
    value.Number = 41
    gResult = value.Number + 1
    Set value = Nothing
End Sub
"#,
                ),
                (
                    "Box",
                    Class,
                    r#"
Public Number As Long
"#,
                ),
            ],
            "BalanceFixture",
        )),
        "carrier-record" => Ok(run(
            Executor::Vm3,
            r#"
Private Type Pair
    Number As Long
    Text As String
End Type

Public gResult As Long

Sub Main()
    Dim value As Pair
    value.Number = 35
    value.Text = "record"
    gResult = value.Number + Len(value.Text)
End Sub
"#,
        )),
        POLICY_ERROR_BALANCE_FIXTURE => Ok(run_with_project(
            Executor::Vm3,
            include_str!(
                "../../../../conformance/jit_v2/tracer_bullets/\
                 tb08_native_declare_shared_abi.bas"
            ),
            "VBAProject",
        )),
        _ => Err(format!(
            "unknown fixture `{fixture}`; known fixtures: {}",
            ALL_BALANCE_FIXTURES.join(", ")
        )),
    }
}
