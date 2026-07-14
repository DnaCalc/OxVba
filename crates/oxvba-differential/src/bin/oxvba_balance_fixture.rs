use oxvba_differential::balance_protocol::{
    ALL_BALANCE_FIXTURES, BalanceFixtureReport, POLICY_ERROR_BALANCE_FIXTURE,
};
use oxvba_differential::{Executor, RunOutcome, run, run_modules, run_with_project};
use oxvba_runtime::{HandleBalance, current_thread_live_handle_counts, live_handle_counts};
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

fn main() {
    let fixture = match parse_fixture_arg() {
        Ok(fixture) => fixture,
        Err(message) => exit_with_error(&message),
    };
    if let Err(message) = verify_counter_scopes() {
        exit_with_error(&message);
    }
    let process_before = live_handle_counts();
    let outcome = match run_named_fixture(&fixture) {
        Ok(outcome) => outcome,
        Err(message) => exit_with_error(&message),
    };
    let process_handle_balance = process_before.balance_to(live_handle_counts());
    let report = match BalanceFixtureReport::from_process_balanced_outcome(
        fixture,
        outcome,
        process_handle_balance,
    ) {
        Ok(report) => report,
        Err(message) => exit_with_error(&message),
    };
    match report.to_protocol_line() {
        Ok(line) => println!("{line}"),
        Err(message) => exit_with_error(&message),
    }
}

fn verify_counter_scopes() -> Result<(), String> {
    let parent_thread_before = current_thread_live_handle_counts();
    let process_before = live_handle_counts();
    let (allocated_tx, allocated_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();

    let child = std::thread::spawn(move || -> Result<(), String> {
        let child_thread_before = current_thread_live_handle_counts();
        let value = oxvba_runtime::bstr::BStr::from("balance-scope-probe");
        let child_thread_balance =
            child_thread_before.balance_to(current_thread_live_handle_counts());
        allocated_tx
            .send(child_thread_balance)
            .map_err(|_| "balance scope parent disappeared before allocation proof".to_string())?;
        release_rx
            .recv()
            .map_err(|_| "balance scope parent did not release the child handle".to_string())?;
        drop(value);
        if current_thread_live_handle_counts() != child_thread_before {
            return Err("child-thread counter did not balance after its local free".to_string());
        }
        Ok(())
    });

    let published = allocated_rx.recv();
    let parent_thread_during = current_thread_live_handle_counts();
    let process_during = process_before.balance_to(live_handle_counts());
    let release = release_tx.send(());
    let child_result = child.join();

    let child_thread_balance =
        published.map_err(|_| "balance scope child did not publish its allocation".to_string())?;
    release.map_err(|_| "balance scope child disappeared before release".to_string())?;
    child_result.map_err(|_| "balance scope child panicked".to_string())??;

    let one_bstr = HandleBalance {
        bstrs: 1,
        ..HandleBalance::default()
    };
    if child_thread_balance != one_bstr {
        return Err(format!(
            "child-thread scope did not observe exactly its BSTR: {child_thread_balance:?}"
        ));
    }
    if parent_thread_during != parent_thread_before {
        return Err(format!(
            "sibling allocation contaminated parent-thread counters: before={parent_thread_before:?} during={parent_thread_during:?}"
        ));
    }
    if process_during != one_bstr {
        return Err(format!(
            "process scope did not observe the sibling BSTR: {process_during:?}"
        ));
    }
    if current_thread_live_handle_counts() != parent_thread_before {
        return Err("sibling free contaminated parent-thread counters".to_string());
    }
    if live_handle_counts() != process_before {
        return Err("process counter did not balance after sibling free".to_string());
    }
    Ok(())
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
