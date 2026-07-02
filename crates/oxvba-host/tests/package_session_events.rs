//! WithEvents event-dispatch through a host runtime session (the in-process COM-server
//! create-class / invoke-member / event-sink surface), running on vm3 via
//! `Engine::prepare_image_session` over an elaborated `OxImage`. The vm3 counterpart of the
//! retired vm2 session flow; complements `vm3_session.rs` (create + invoke)
//! with the parameterless-probe-event and native-FM20-named-event WithEvents fan-out.

use oxvba_bundle::ProjectMemberKind;
use oxvba_hal::model::HostPolicy;
use oxvba_host::{Engine, HostConfig, ProjectRuntimeSession};
use oxvba_oxir::OxImage;
use oxvba_runtime::Variant;
use oxvba_symbol::manifest::{
    ModuleAttributes, ModuleKind, ModuleUnit, ProjectKind, SymbolProjectManifest,
};

fn class_module(name: &str, source: &str) -> ModuleUnit {
    let mut attributes = ModuleAttributes::named(name);
    attributes.vb_exposed = true;
    attributes.vb_creatable = true;
    ModuleUnit {
        module_name: name.to_string(),
        module_kind: ModuleKind::Class,
        attributes,
        source: source.to_string(),
    }
}

fn package_session_for(modules: Vec<ModuleUnit>) -> ProjectRuntimeSession {
    let manifest = SymbolProjectManifest {
        project_name: "DemoServer".to_string(),
        project_kind: ProjectKind::Library,
        modules,
        references: Vec::new(),
        reference_projects: Vec::new(),
        conditional_constants: Default::default(),
        conditional_compilation_target: Default::default(),
    };
    let typelibs = oxvba_symbol::CatalogTypeLibResolver;
    let program = oxvba_bind::bind_program(&manifest, &typelibs).expect("bind package");
    let oxp = oxvba_oxir::elaborate::elaborate(&program).expect("elaborate package");
    let mut engine = Engine::new(HostConfig::default());
    engine.set_host_policy(HostPolicy::interactive_dev());
    engine
        .prepare_image_session(OxImage::new(vec![oxp]))
        .expect("prepare vm3 session")
}

fn invoke(
    session: &mut ProjectRuntimeSession,
    object: oxvba_runtime::ObjectRef,
    member: &str,
    args: Vec<Variant>,
) -> Variant {
    session
        .invoke_member_values(object, member, Some(ProjectMemberKind::Method), args)
        .unwrap_or_else(|err| panic!("invoke {member}: {err} ({:?})", err.diagnostic()))
}

#[test]
fn package_session_withevents_dispatches_parameterless_probe_event() {
    let source = r#"
Option Explicit

Public Event Click()
Public Event ProbeClick()

Public Sub RaiseClick()
    RaiseEvent ProbeClick
End Sub
"#;
    let sink = r#"
Option Explicit

Private WithEvents EventSource As W005ProbeEventSource
Private mLog As String

Public Sub Attach(ByVal source As W005ProbeEventSource)
    Set EventSource = source
End Sub

Private Sub EventSource_ProbeClick()
    mLog = mLog & "P"
End Sub

Public Property Get LogText() As String
    LogText = mLog
End Property
"#;
    let mut session = package_session_for(vec![
        class_module("W005ProbeEventSource", source),
        class_module("W005ProbeEventHarness", sink),
    ]);
    let source = session
        .create_class_instance("W005ProbeEventSource")
        .expect("create source");
    let sink = session
        .create_class_instance("W005ProbeEventHarness")
        .expect("create sink");

    invoke(
        &mut session,
        sink.clone(),
        "Attach",
        vec![Variant::from_object_ref(source.clone())],
    );
    invoke(&mut session, source, "RaiseClick", Vec::new());

    let log = session
        .invoke_member_values(
            sink,
            "LogText",
            Some(ProjectMemberKind::PropertyGet),
            Vec::new(),
        )
        .expect("read log");
    assert_eq!(log.as_bstr().map(|s| s.as_str()), Some("P".to_string()));
}

#[test]
fn package_session_withevents_dispatches_native_fm20_named_events() {
    let source = r#"
Option Explicit

Public Event Click()
Public Event MouseDown(ByVal Button As Integer, ByVal Shift As Integer, ByVal X As Single, ByVal Y As Single)
Public Event MouseUp(ByVal Button As Integer, ByVal Shift As Integer, ByVal X As Single, ByVal Y As Single)
Public Event MouseMove(ByVal Button As Integer, ByVal Shift As Integer, ByVal X As Single, ByVal Y As Single)

Public Sub RaiseClick()
    RaiseEvent Click
End Sub

Public Sub RaiseMouseDown(ByVal Button As Integer, ByVal Shift As Integer, ByVal X As Single, ByVal Y As Single)
    RaiseEvent MouseDown(Button, Shift, X, Y)
End Sub

Public Sub RaiseMouseUp(ByVal Button As Integer, ByVal Shift As Integer, ByVal X As Single, ByVal Y As Single)
    RaiseEvent MouseUp(Button, Shift, X, Y)
End Sub

Public Sub RaiseMouseMove(ByVal Button As Integer, ByVal Shift As Integer, ByVal X As Single, ByVal Y As Single)
    RaiseEvent MouseMove(Button, Shift, X, Y)
End Sub
"#;
    let sink = r#"
Option Explicit

Private WithEvents EventSource As W006NativeFm20EventSource
Private mLog As String
Private mDownScore As Long
Private mUpScore As Long
Private mMoveScore As Long

Public Sub Attach(ByVal source As W006NativeFm20EventSource)
    Set EventSource = source
End Sub

Private Sub EventSource_Click()
    mLog = mLog & "C"
End Sub

Private Sub EventSource_MouseDown(ByVal Button As Integer, ByVal Shift As Integer, ByVal X As Single, ByVal Y As Single)
    mLog = mLog & "D"
    mDownScore = CLng(Button) * 1000000 + CLng(Shift) * 10000 + CLng(X) * 100 + CLng(Y)
End Sub

Private Sub EventSource_MouseUp(ByVal Button As Integer, ByVal Shift As Integer, ByVal X As Single, ByVal Y As Single)
    mLog = mLog & "U"
    mUpScore = CLng(Button) * 1000000 + CLng(Shift) * 10000 + CLng(X) * 100 + CLng(Y)
End Sub

Private Sub EventSource_MouseMove(ByVal Button As Integer, ByVal Shift As Integer, ByVal X As Single, ByVal Y As Single)
    mLog = mLog & "M"
    mMoveScore = CLng(Button) * 1000000 + CLng(Shift) * 10000 + CLng(X) * 100 + CLng(Y)
End Sub

Public Function Log() As String
    Log = mLog
End Function

Public Function DownScore() As Long
    DownScore = mDownScore
End Function

Public Function UpScore() As Long
    UpScore = mUpScore
End Function

Public Function MoveScore() As Long
    MoveScore = mMoveScore
End Function
"#;
    let mut session = package_session_for(vec![
        class_module("W006NativeFm20EventSource", source),
        class_module("W006NativeFm20EventSink", sink),
    ]);
    let source = session
        .create_class_instance("W006NativeFm20EventSource")
        .expect("create source");
    let sink = session
        .create_class_instance("W006NativeFm20EventSink")
        .expect("create sink");

    invoke(
        &mut session,
        sink.clone(),
        "Attach",
        vec![Variant::from_object_ref(source.clone())],
    );
    invoke(
        &mut session,
        source.clone(),
        "RaiseMouseDown",
        vec![
            Variant::from_i32(1),
            Variant::from_i32(2),
            Variant::from_f64(30.0),
            Variant::from_f64(40.0),
        ],
    );
    invoke(&mut session, source.clone(), "RaiseClick", Vec::new());
    invoke(
        &mut session,
        source.clone(),
        "RaiseMouseUp",
        vec![
            Variant::from_i32(3),
            Variant::from_i32(4),
            Variant::from_f64(50.0),
            Variant::from_f64(60.0),
        ],
    );
    invoke(
        &mut session,
        source.clone(),
        "RaiseMouseMove",
        vec![
            Variant::from_i32(5),
            Variant::from_i32(6),
            Variant::from_f64(70.0),
            Variant::from_f64(80.0),
        ],
    );

    let log = invoke(&mut session, sink.clone(), "Log", Vec::new());
    assert_eq!(log.as_bstr().map(|s| s.as_str()), Some("DCUM".to_string()));
    assert_eq!(
        invoke(&mut session, sink.clone(), "DownScore", Vec::new()).as_i32(),
        Some(1_023_040)
    );
    assert_eq!(
        invoke(&mut session, sink.clone(), "UpScore", Vec::new()).as_i32(),
        Some(3_045_060)
    );
    assert_eq!(
        invoke(&mut session, sink, "MoveScore", Vec::new()).as_i32(),
        Some(5_067_080)
    );
}
