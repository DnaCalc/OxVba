//! Smoke tests over hand-authored bundles, exercising the execution model:
//! arithmetic, the copy-in/copy-out calling convention (ByRef + function
//! return), a library built-in, a counted loop, and both `On Error` paths.

use oxvba_bundle::{
    Bundle, NativeCallee, NativeImplId, Op, ProcArg, ProcedureDescriptor, ProcedureKind,
    StringCompareMode, isa::CallArg,
};
use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;

use crate::run;

fn host() -> NullHostServices {
    NullHostServices::new(HostPolicy::deterministic_runtime())
}

fn bundle(ops: Vec<Op>, slot_count: usize, procedures: Vec<ProcedureDescriptor>) -> Bundle {
    Bundle {
        ops,
        procedures,
        entry_pc: 0,
        slot_count,
        user_slot_count: slot_count,
        external_calls: Vec::new(),
        source_map: Vec::new(),
        com_class_exports: Vec::new(),
    }
}

#[test]
fn arithmetic() {
    // (10 + 5) * 2 = 30
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 10 },
            Op::LoadI32 { slot: 1, value: 5 },
            Op::Add { dst: 2, lhs: 0, rhs: 1 },
            Op::LoadI32 { slot: 3, value: 2 },
            Op::Mul { dst: 4, lhs: 2, rhs: 3 },
            Op::Halt,
        ],
        5,
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(4).unwrap().as_f64(), Some(30.0));
}

#[test]
fn builtin_len() {
    let b = bundle(
        vec![
            Op::LoadString { slot: 0, value: "hello".to_string() },
            Op::CallNative {
                dst: Some(1),
                callee: NativeCallee::Builtin(NativeImplId::Len),
                args: vec![CallArg::Slot(0)],
            },
            Op::Halt,
        ],
        2,
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(1).unwrap().as_i32(), Some(5));
}

#[test]
fn proc_byref_writeback() {
    // Sub Inc(ByRef n): n = n + 1.  Caller passes slot 0 (= 41) ByRef → 42.
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 41 },                                   // 0
            Op::CallProc { proc: 0, dst: None, args: vec![ProcArg::ByRef(0)], member: None }, // 1
            Op::Halt,                                                             // 2
            Op::IncSlot { slot: 5 },                                              // 3 (Inc entry)
            Op::Return,                                                           // 4
        ],
        7,
        vec![ProcedureDescriptor {
            name: "Inc".to_string(),
            entry_pc: 3,
            kind: ProcedureKind::Sub,
            param_count: 1,
            frame_base: 5,
            frame_slots: 2,
            return_slot: None,
        }],
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(42));
}

#[test]
fn proc_function_return() {
    // Function Double(ByVal n) = n + n.  Double(21) → 42 into slot 1.
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 21 },                                          // 0
            Op::CallProc { proc: 0, dst: Some(1), args: vec![ProcArg::ByVal(0)], member: None }, // 1
            Op::Halt,                                                                    // 2
            Op::Add { dst: 7, lhs: 5, rhs: 5 },                                          // 3 (Double entry)
            Op::Return,                                                                  // 4
        ],
        8,
        vec![ProcedureDescriptor {
            name: "Double".to_string(),
            entry_pc: 3,
            kind: ProcedureKind::Function,
            param_count: 1,
            frame_base: 5,
            frame_slots: 3,
            return_slot: Some(7),
        }],
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(1).unwrap().as_i32(), Some(42));
}

#[test]
fn counted_loop_sum() {
    // acc = 0; For i = 1 To 5: acc = acc + i.  Sum = 15.
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 0 },                                       // 0 acc
            Op::LoadI32 { slot: 1, value: 1 },                                       // 1 i
            Op::LoadI32 { slot: 2, value: 6 },                                       // 2 limit
            Op::CmpLt { dst: 3, lhs: 1, rhs: 2, mode: StringCompareMode::Binary },   // 3 i<6
            Op::JumpIfZero { cond_slot: 3, target_pc: 8 },                           // 4 exit
            Op::Add { dst: 0, lhs: 0, rhs: 1 },                                      // 5 acc+=i
            Op::IncSlot { slot: 1 },                                                 // 6 i++
            Op::Jump { target_pc: 3 },                                               // 7 loop
            Op::Halt,                                                                // 8
        ],
        4,
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(15));
}

#[test]
fn on_error_resume_next() {
    let b = bundle(
        vec![
            Op::SetOnErrorResumeNext,         // 0
            Op::RaiseError { code: 11 },      // 1 (resume → 2)
            Op::LoadI32 { slot: 0, value: 99 }, // 2
            Op::Halt,                         // 3
        ],
        1,
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(99));
}

#[test]
fn on_error_goto_handler() {
    let b = bundle(
        vec![
            Op::SetOnErrorGotoLabel { target_pc: 4 }, // 0
            Op::RaiseError { code: 11 },              // 1 → handler
            Op::LoadI32 { slot: 0, value: 1 },        // 2 (skipped)
            Op::Halt,                                 // 3
            Op::LoadI32 { slot: 0, value: 7 },        // 4 handler
            Op::Halt,                                 // 5
        ],
        1,
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(7));
}

#[test]
fn uncaught_error_surfaces() {
    let b = bundle(vec![Op::RaiseError { code: 6 }, Op::Halt], 1, Vec::new());
    let h = host();
    assert!(matches!(run(&b, &h), Err(e) if e.code == 6));
}
