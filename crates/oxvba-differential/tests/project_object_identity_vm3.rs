//! VM3 project-object identity regressions.

use oxvba_differential::{Canon, Executor, RunOutcome, run_modules};
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

fn assert_contains_string(outcome: RunOutcome, expected: &str) {
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined project-object identity case as unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome
            .handle_balance
            .is_some_and(|balance| balance.is_zero()),
        "vm3 project-object identity case leaked runtime handles: {:?}",
        outcome.handle_balance
    );
    let values = outcome
        .result
        .unwrap_or_else(|err| panic!("vm3 project-object identity case failed: {err}"));
    let expected = Canon::Str(expected.to_string());
    assert!(
        values.contains(&expected),
        "snapshot {values:?} did not contain {expected:?}"
    );
}

#[test]
fn project_class_set_rejects_incompatible_project_class() {
    let main = "Public result As Variant\n\
                Sub Main()\n\
                \x20   Dim a As Alpha\n\
                \x20   Dim b As Beta\n\
                \x20   Dim mismatchErr As Long\n\
                \x20   Dim okErr As Long\n\
                \x20   Set b = New Beta\n\
                \x20   On Error Resume Next\n\
                \x20   Set a = b\n\
                \x20   mismatchErr = Err.Number\n\
                \x20   Err.Clear\n\
                \x20   Set a = New Alpha\n\
                \x20   okErr = Err.Number\n\
                \x20   On Error GoTo 0\n\
                \x20   result = CStr(mismatchErr) & \"|\" & CStr(okErr) & \"|\" & CStr(a Is Nothing)\n\
                \x20   Set a = Nothing\n\
                \x20   Set b = Nothing\n\
                End Sub\n";
    let alpha = "Public Function Value() As Long\n\
                 \x20   Value = 1\n\
                 End Function\n";
    let beta = "Public Function Value() As Long\n\
                \x20   Value = 2\n\
                End Function\n";

    assert_contains_string(
        run_modules(
            Executor::Vm3,
            &[
                ("Main", Procedural, main),
                ("Alpha", Class, alpha),
                ("Beta", Class, beta),
            ],
            "VBAProject",
        ),
        "13|0|False",
    );
}

#[test]
fn project_interface_set_accepts_implementer_and_rejects_non_implementer() {
    let main = "Public result As Variant\n\
                Sub Main()\n\
                \x20   Dim a As IAnimal\n\
                \x20   Dim d As Dog\n\
                \x20   Dim r As Rock\n\
                \x20   Dim speak As Long\n\
                \x20   Dim mismatchErr As Long\n\
                \x20   Dim nothingErr As Long\n\
                \x20   Set d = New Dog\n\
                \x20   Set r = New Rock\n\
                \x20   Set a = d\n\
                \x20   speak = a.Speak()\n\
                \x20   On Error Resume Next\n\
                \x20   Set a = r\n\
                \x20   mismatchErr = Err.Number\n\
                \x20   Err.Clear\n\
                \x20   Set a = Nothing\n\
                \x20   nothingErr = Err.Number\n\
                \x20   On Error GoTo 0\n\
                \x20   result = CStr(speak) & \"|\" & CStr(mismatchErr) & \"|\" & CStr(nothingErr) & \"|\" & CStr(a Is Nothing)\n\
                \x20   Set d = Nothing\n\
                \x20   Set r = Nothing\n\
                End Sub\n";
    let ianimal = "Public Function Speak() As Long\n\
                   End Function\n";
    let dog = "Implements IAnimal\n\n\
               Private Function IAnimal_Speak() As Long\n\
               \x20   IAnimal_Speak = 42\n\
               End Function\n";
    let rock = "Public Function Weight() As Long\n\
                \x20   Weight = 9\n\
                End Function\n";

    assert_contains_string(
        run_modules(
            Executor::Vm3,
            &[
                ("Main", Procedural, main),
                ("IAnimal", Class, ianimal),
                ("Dog", Class, dog),
                ("Rock", Class, rock),
            ],
            "VBAProject",
        ),
        "42|13|0|True",
    );
}

#[test]
fn project_property_set_rejects_incompatible_project_class_value() {
    let main = "Public result As Variant\n\
                Sub Main()\n\
                \x20   Dim box As Box\n\
                \x20   Dim b As Beta\n\
                \x20   Dim got As Alpha\n\
                \x20   Dim mismatchErr As Long\n\
                \x20   Dim okErr As Long\n\
                \x20   Set box = New Box\n\
                \x20   Set b = New Beta\n\
                \x20   On Error Resume Next\n\
                \x20   Set box.Item = b\n\
                \x20   mismatchErr = Err.Number\n\
                \x20   Err.Clear\n\
                \x20   Set box.Item = New Alpha\n\
                \x20   okErr = Err.Number\n\
                \x20   On Error GoTo 0\n\
                \x20   Set got = box.Item\n\
                \x20   result = CStr(mismatchErr) & \"|\" & CStr(okErr) & \"|\" & CStr(got Is Nothing)\n\
                \x20   Set got = Nothing\n\
                \x20   Set b = Nothing\n\
                \x20   Set box = Nothing\n\
                End Sub\n";
    let box_cls = "Private stored As Alpha\n\n\
                   Public Property Get Item() As Alpha\n\
                   \x20   Set Item = stored\n\
                   End Property\n\n\
                   Public Property Set Item(ByVal value As Alpha)\n\
                   \x20   Set stored = value\n\
                   End Property\n";
    let alpha = "Public Function Value() As Long\n\
                 \x20   Value = 1\n\
                 End Function\n";
    let beta = "Public Function Value() As Long\n\
                \x20   Value = 2\n\
                End Function\n";

    assert_contains_string(
        run_modules(
            Executor::Vm3,
            &[
                ("Main", Procedural, main),
                ("Box", Class, box_cls),
                ("Alpha", Class, alpha),
                ("Beta", Class, beta),
            ],
            "VBAProject",
        ),
        "13|0|False",
    );
}

#[test]
fn project_class_is_compares_object_identity_for_non_null_instances() {
    let main = "Public result As Variant\n\
                Sub Main()\n\
                \x20   Dim a As Widget\n\
                \x20   Dim b As Widget\n\
                \x20   Dim c As Widget\n\
                \x20   Set a = New Widget\n\
                \x20   Set b = a\n\
                \x20   Set c = New Widget\n\
                \x20   result = CStr(a Is b) & \"|\" & CStr(a Is c) & \"|\" & CStr(a Is Nothing) & \"|\" & CStr(c Is Nothing)\n\
                \x20   Set b = Nothing\n\
                \x20   Set a = Nothing\n\
                \x20   Set c = Nothing\n\
                End Sub\n";
    let widget = "Private n As Long\n\
                  Private Sub Class_Initialize()\n\
                  \x20   n = 1\n\
                  End Sub\n\
                  Public Property Get Value() As Long\n\
                  \x20   Value = n\n\
                  End Property\n";

    assert_contains_string(
        run_modules(
            Executor::Vm3,
            &[("Main", Procedural, main), ("Widget", Class, widget)],
            "VBAProject",
        ),
        "True|False|False|False",
    );
}

#[test]
fn project_class_typeof_uses_runtime_descriptor_for_object_variables() {
    let main = "Public result As Variant\n\
                Sub Main()\n\
                \x20   Dim o As Object\n\
                \x20   Set o = New Widget\n\
                \x20   Dim isWidget As Boolean\n\
                \x20   Dim isGadget As Boolean\n\
                \x20   Dim isNothingWidget As Boolean\n\
                \x20   isWidget = TypeOf o Is Widget\n\
                \x20   isGadget = TypeOf o Is Gadget\n\
                \x20   Set o = Nothing\n\
                \x20   isNothingWidget = TypeOf o Is Widget\n\
                \x20   result = CStr(isWidget) & \"|\" & CStr(isGadget) & \"|\" & CStr(isNothingWidget)\n\
                End Sub\n";
    let widget = "Private n As Long\n\
                  Private Sub Class_Initialize()\n\
                  \x20   n = 1\n\
                  End Sub\n";
    let gadget = "Private n As Long\n\
                  Private Sub Class_Initialize()\n\
                  \x20   n = 2\n\
                  End Sub\n";

    assert_contains_string(
        run_modules(
            Executor::Vm3,
            &[
                ("Main", Procedural, main),
                ("Widget", Class, widget),
                ("Gadget", Class, gadget),
            ],
            "VBAProject",
        ),
        "True|False|False",
    );
}

#[test]
fn project_class_typename_uses_runtime_descriptor_for_object_variables() {
    let main = "Public result As Variant\n\
                Sub Main()\n\
                \x20   Dim o As Object\n\
                \x20   Set o = New Widget\n\
                \x20   Dim liveName As String\n\
                \x20   liveName = TypeName(o)\n\
                \x20   Set o = Nothing\n\
                \x20   result = liveName & \"|\" & TypeName(o)\n\
                End Sub\n";
    let widget = "Private n As Long\n\
                  Private Sub Class_Initialize()\n\
                  \x20   n = 1\n\
                  End Sub\n";

    assert_contains_string(
        run_modules(
            Executor::Vm3,
            &[("Main", Procedural, main), ("Widget", Class, widget)],
            "VBAProject",
        ),
        "Widget|Nothing",
    );
}
