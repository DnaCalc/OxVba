//! Active-object default-member indexed access for `bd-4ktq.48`.

use oxvba_differential::{Executor, RunOutcome, canon, run_modules};
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::ModuleKind::{Class, Procedural};

fn run_case(main: &str, classes: &[(&str, &str)]) -> RunOutcome {
    let mut modules = vec![("Main", Procedural, main)];
    modules.extend(classes.iter().map(|(name, source)| (*name, Class, *source)));
    run_modules(Executor::Vm3, &modules, "VBAProject")
}

fn assert_global_long(outcome: RunOutcome, expected: i32) {
    assert!(
        outcome.unsupported.is_none(),
        "vm3 declined default-member case as unsupported: {:?}",
        outcome.unsupported
    );
    assert!(
        outcome
            .handle_balance
            .is_some_and(|balance| balance.is_zero()),
        "vm3 default-member case leaked runtime handles: {:?}",
        outcome.handle_balance
    );
    let snapshot = outcome.result.expect("vm3 default-member case completed");
    assert!(
        snapshot.contains(&canon(&Variant::from_i32(expected))),
        "expected snapshot to contain {expected}, got {snapshot:?}"
    );
}

#[test]
fn object_typed_default_member_index_get_and_let() {
    let main = "Public r As Long\n\
        Sub Main()\n\
            Dim o As Object\n\
            Set o = New Widget\n\
            o(3) = 10\n\
            r = o(2)\n\
        End Sub\n";
    let widget = "Private mV As Long\n\n\
        Public Property Get Value(ByVal i As Long) As Long\n\
            Value = mV + i\n\
        End Property\n\
        Attribute Value.VB_UserMemId = 0\n\n\
        Public Property Let Value(ByVal i As Long, ByVal v As Long)\n\
            mV = v + i\n\
        End Property\n\
        Attribute Value.VB_UserMemId = 0\n";
    assert_global_long(run_case(main, &[("Widget", widget)]), 15);
}

#[test]
fn object_typed_default_member_named_index_get_and_let() {
    let main = "Public r As Long\n\
        Sub Main()\n\
            Dim o As Object\n\
            Set o = New Widget\n\
            o(i := 3) = 10\n\
            r = o(i := 2)\n\
        End Sub\n";
    let widget = "Private mV As Long\n\n\
        Public Property Get Value(ByVal i As Long) As Long\n\
            Value = mV + i\n\
        End Property\n\
        Attribute Value.VB_UserMemId = 0\n\n\
        Public Property Let Value(ByVal i As Long, ByVal v As Long)\n\
            mV = v + i\n\
        End Property\n\
        Attribute Value.VB_UserMemId = 0\n";
    assert_global_long(run_case(main, &[("Widget", widget)]), 15);
}

#[test]
fn object_typed_default_member_index_get_and_set() {
    let main = "Public r As Long\n\
        Sub Main()\n\
            Dim b As Object\n\
            Dim t As Thing\n\
            Dim got As Thing\n\
            Set b = New Box\n\
            Set t = New Thing\n\
            Set b(3) = t\n\
            Set got = b(2)\n\
            r = got.GetVal()\n\
        End Sub\n";
    let box_cls = "Private stored As Thing\n\n\
        Public Property Get Item(ByVal i As Long) As Thing\n\
            Set Item = stored\n\
        End Property\n\
        Attribute Item.VB_UserMemId = 0\n\n\
        Public Property Set Item(ByVal i As Long, ByVal v As Thing)\n\
            Set stored = v\n\
        End Property\n\
        Attribute Item.VB_UserMemId = 0\n";
    let thing = "Public Function GetVal() As Long\n\
            GetVal = 23\n\
        End Function\n";
    assert_global_long(run_case(main, &[("Box", box_cls), ("Thing", thing)]), 23);
}

#[test]
fn variant_held_object_default_member_index_get_and_let() {
    let main = "Public r As Long\n\
        Sub Main()\n\
            Dim v As Variant\n\
            Set v = New Widget\n\
            v(4) = 10\n\
            r = v(1)\n\
        End Sub\n";
    let widget = "Private mV As Long\n\n\
        Public Property Get Value(ByVal i As Long) As Long\n\
            Value = mV + i\n\
        End Property\n\
        Attribute Value.VB_UserMemId = 0\n\n\
        Public Property Let Value(ByVal i As Long, ByVal v As Long)\n\
            mV = v + i\n\
        End Property\n\
        Attribute Value.VB_UserMemId = 0\n";
    assert_global_long(run_case(main, &[("Widget", widget)]), 15);
}

#[test]
fn variant_held_object_default_member_index_get_and_set() {
    let main = "Public r As Long\n\
        Sub Main()\n\
            Dim v As Variant\n\
            Dim t As Thing\n\
            Dim got As Thing\n\
            Set v = New Box\n\
            Set t = New Thing\n\
            Set v(3) = t\n\
            Set got = v(2)\n\
            r = got.GetVal()\n\
        End Sub\n";
    let box_cls = "Private stored As Thing\n\n\
        Public Property Get Item(ByVal i As Long) As Thing\n\
            Set Item = stored\n\
        End Property\n\
        Attribute Item.VB_UserMemId = 0\n\n\
        Public Property Set Item(ByVal i As Long, ByVal v As Thing)\n\
            Set stored = v\n\
        End Property\n\
        Attribute Item.VB_UserMemId = 0\n";
    let thing = "Public Function GetVal() As Long\n\
            GetVal = 29\n\
        End Function\n";
    assert_global_long(run_case(main, &[("Box", box_cls), ("Thing", thing)]), 29);
}
