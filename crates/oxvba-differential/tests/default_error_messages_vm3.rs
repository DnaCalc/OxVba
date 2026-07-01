//! vm3 should derive the standard VBA `Err.Description` text for common
//! built-in runtime error codes, rather than falling back to the generic
//! application/object-defined message for every unmapped code.
//! Live Excel/VBA 7.1 evidence:
//! `docs/evidence/conformance/vm3_default_error_message_oracle_20260701T1410Z/`.

use oxvba_differential::{Canon, Executor, run};

const EXPECTED: &[(i32, &str)] = &[
    (3, "Return without GoSub"),
    (5, "Invalid procedure call or argument"),
    (6, "Overflow"),
    (7, "Out of memory"),
    (9, "Subscript out of range"),
    (10, "This array is fixed or temporarily locked"),
    (11, "Division by zero"),
    (13, "Type mismatch"),
    (14, "Out of string space"),
    (16, "Expression too complex"),
    (17, "Can't perform requested operation"),
    (18, "User interrupt occurred"),
    (20, "Resume without error"),
    (28, "Out of stack space"),
    (35, "Sub or Function not defined"),
    (48, "Error in loading DLL"),
    (49, "Bad DLL calling convention"),
    (52, "Bad file name or number"),
    (53, "File not found"),
    (54, "Bad file mode"),
    (55, "File already open"),
    (57, "Device I/O error"),
    (58, "File already exists"),
    (61, "Disk full"),
    (62, "Input past end of file"),
    (67, "Too many files"),
    (68, "Device unavailable"),
    (70, "Permission denied"),
    (71, "Disk not ready"),
    (74, "Can't rename with different drive"),
    (75, "Path/File access error"),
    (76, "Path not found"),
    (91, "Object variable or With block variable not set"),
    (92, "For loop not initialized"),
    (93, "Invalid pattern string"),
    (94, "Invalid use of Null"),
    (
        97,
        "Can not call friend function on object which is not an instance of defining class",
    ),
    (
        98,
        "A property or method call cannot include a reference to a private object, either as an argument or as a return value",
    ),
    (380, "Invalid property value"),
    (381, "Invalid property array index"),
    (382, "Set not supported at runtime"),
    (383, "Set not supported (read-only property)"),
    (385, "Need property array index"),
    (387, "Set not permitted"),
    (393, "Get not supported at runtime"),
    (394, "Get not supported (write-only property)"),
    (422, "Property not found"),
    (423, "Property or method not found"),
    (424, "Object required"),
    (429, "ActiveX component can't create object"),
    (
        430,
        "Class does not support Automation or does not support expected interface",
    ),
    (
        432,
        "File name or class name not found during Automation operation",
    ),
    (438, "Object doesn't support this property or method"),
    (440, "Automation error"),
    (
        442,
        "Connection to type library or object library for remote process has been lost. Press OK for dialog to remove reference.",
    ),
    (443, "Automation object does not have a default value"),
    (445, "Object doesn't support this action"),
    (446, "Object doesn't support named arguments"),
    (447, "Object doesn't support current locale setting"),
    (448, "Named argument not found"),
    (449, "Argument not optional"),
    (
        450,
        "Wrong number of arguments or invalid property assignment",
    ),
    (
        451,
        "Property let procedure not defined and property get procedure did not return an object",
    ),
    (452, "Invalid ordinal"),
    (453, "Specified DLL function not found"),
    (454, "Code resource not found"),
    (455, "Code resource lock error"),
    (
        457,
        "This key is already associated with an element of this collection",
    ),
    (
        458,
        "Variable uses an Automation type not supported in Visual Basic",
    ),
    (459, "Object or class does not support the set of events"),
    (460, "Invalid clipboard format"),
    (461, "Method or data member not found"),
    (
        462,
        "The remote server machine does not exist or is unavailable",
    ),
    (463, "Class not registered on local machine"),
    (481, "Invalid picture"),
    (482, "Printer error"),
    (735, "Can't save file to TEMP"),
    (744, "Search text not found"),
    (746, "Replacements too long"),
];

fn run_description_probe(body: &str) -> String {
    let source = format!("Public result As String\nSub Main()\n{body}\nEnd Sub\n");
    let outcome = run(Executor::Vm3, &source);
    assert!(
        outcome.unsupported.is_none(),
        "unsupported: {:?}\n{source}",
        outcome.unsupported
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("default error-message probe failed: {err}\n{source}"));
    match values.first() {
        Some(Canon::Str(value)) => value.clone(),
        other => panic!("expected string result, got {other:?} from {values:?}"),
    }
}

fn expected_text(rows: &[(i32, &str)]) -> String {
    rows.iter()
        .map(|(code, message)| format!("{code}={message}\n"))
        .collect()
}

fn append_error_statement_rows(rows: &[(i32, &str)]) -> String {
    let mut body = String::from("    On Error Resume Next\n");
    for (code, _) in rows {
        body.push_str("    Err.Clear\n");
        body.push_str(&format!("    Error {code}\n"));
        body.push_str(&format!(
            "    result = result & \"{code}=\" & Err.Description & vbLf\n"
        ));
    }
    body
}

fn append_err_raise_rows(rows: &[(i32, &str)]) -> String {
    let mut body = String::from("    On Error Resume Next\n");
    for (code, _) in rows {
        body.push_str("    Err.Clear\n");
        body.push_str(&format!("    Err.Raise {code}\n"));
        body.push_str(&format!(
            "    result = result & \"{code}=\" & Err.Description & vbLf\n"
        ));
    }
    body
}

#[test]
fn error_statement_derives_standard_descriptions() {
    assert_eq!(
        run_description_probe(&append_error_statement_rows(EXPECTED)),
        expected_text(EXPECTED)
    );
}

#[test]
fn err_raise_omitted_description_derives_standard_descriptions() {
    assert_eq!(
        run_description_probe(&append_err_raise_rows(EXPECTED)),
        expected_text(EXPECTED)
    );
}

#[test]
fn unmapped_custom_error_code_keeps_generic_fallback() {
    assert_eq!(
        run_description_probe(
            "    On Error Resume Next\n    Err.Clear\n    Err.Raise 12345\n    result = Err.Description"
        ),
        "Application-defined or object-defined error"
    );
}
