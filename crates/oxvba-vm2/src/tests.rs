//! Smoke tests over hand-authored bundles, exercising the execution model:
//! arithmetic, the frame-based calling convention (true ByRef aliasing,
//! function return, recursion), module globals, a library built-in, a counted
//! loop, and the `On Error` paths including statement-granular `Resume Next`.
//!
//! With `global_count = 0`, every slot operand is a current-frame local, so a
//! procedure's slot `n` and the caller's slot `n` are distinct storage.

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

fn bundle_full(
    ops: Vec<Op>,
    global_count: usize,
    entry_frame_slots: usize,
    statement_starts: Vec<usize>,
    procedures: Vec<ProcedureDescriptor>,
) -> Bundle {
    Bundle {
        ops,
        procedures,
        entry_pc: 0,
        global_count,
        entry_frame_slots,
        statement_starts,
        external_calls: Vec::new(),
        source_map: Vec::new(),
        com_class_exports: Vec::new(),
    }
}

fn bundle(ops: Vec<Op>, entry_frame_slots: usize, procedures: Vec<ProcedureDescriptor>) -> Bundle {
    bundle_full(ops, 0, entry_frame_slots, Vec::new(), procedures)
}

fn func(name: &str, entry_pc: usize, frame_slots: usize, return_slot: Option<usize>) -> ProcedureDescriptor {
    ProcedureDescriptor {
        name: name.to_string(),
        entry_pc,
        kind: if return_slot.is_some() { ProcedureKind::Function } else { ProcedureKind::Sub },
        param_count: 1,
        frame_slots,
        return_slot,
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
fn proc_byref_aliasing() {
    // Sub Inc(ByRef n): n = n + 1.  Caller passes local 0 (= 41) ByRef → 42.
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 41 },                                            // 0
            Op::CallProc { proc: 0, dst: None, args: vec![ProcArg::ByRef(0)], member: None }, // 1
            Op::Halt,                                                                      // 2
            Op::IncSlot { slot: 0 },                                                       // 3 (Inc entry; local 0 = aliased param)
            Op::Return,                                                                    // 4
        ],
        1,
        vec![func("Inc", 3, 1, None)],
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(42));
}

#[test]
fn proc_function_return() {
    // Function Double(ByVal n) = n + n.  Double(21) → 42 into caller local 1.
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 21 },                                                  // 0
            Op::CallProc { proc: 0, dst: Some(1), args: vec![ProcArg::ByVal(0)], member: None }, // 1
            Op::Halt,                                                                            // 2
            Op::Add { dst: 1, lhs: 0, rhs: 0 },                                                  // 3 (Double entry; local 1 = return)
            Op::Return,                                                                          // 4
        ],
        2,
        vec![func("Double", 3, 2, Some(1))],
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(1).unwrap().as_i32(), Some(42));
}

#[test]
fn recursion_factorial() {
    // Function Fact(n) = If n <= 1 Then 1 Else n * Fact(n - 1).  Fact(5) = 120.
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 5 },                                                   // 0
            Op::CallProc { proc: 0, dst: Some(1), args: vec![ProcArg::ByVal(0)], member: None }, // 1
            Op::Halt,                                                                            // 2
            // Fact entry (pc 3): locals 0=n, 1=const1, 2=cond, 3=n-1, 4=Fact(n-1), 5=result
            Op::LoadI32 { slot: 1, value: 1 },                                                   // 3
            Op::CmpLe { dst: 2, lhs: 0, rhs: 1, mode: StringCompareMode::Binary },               // 4
            Op::JumpIfZero { cond_slot: 2, target_pc: 8 },                                       // 5 -> recurse
            Op::LoadI32 { slot: 5, value: 1 },                                                   // 6 base: result = 1
            Op::Return,                                                                          // 7
            Op::Copy { dst: 3, src: 0 },                                                         // 8 n-1 = n
            Op::SubConstI32 { slot: 3, value: 1 },                                               // 9 n-1
            Op::CallProc { proc: 0, dst: Some(4), args: vec![ProcArg::ByVal(3)], member: None }, // 10 Fact(n-1)
            Op::Mul { dst: 5, lhs: 0, rhs: 4 },                                                  // 11 result = n * Fact(n-1)
            Op::Return,                                                                          // 12
        ],
        2,
        vec![func("Fact", 3, 6, Some(5))],
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(1).unwrap().as_f64(), Some(120.0));
}

#[test]
fn global_persists_across_calls() {
    // global 0 (slot 0, since global_count = 1); Sub Bump increments it; call twice → 2.
    let b = bundle_full(
        vec![
            Op::CallProc { proc: 0, dst: None, args: vec![], member: None }, // 0
            Op::CallProc { proc: 0, dst: None, args: vec![], member: None }, // 1
            Op::Halt,                                                        // 2
            Op::IncSlot { slot: 0 },                                         // 3 (Bump entry; slot 0 = global)
            Op::Return,                                                      // 4
        ],
        1, // global_count
        0, // entry_frame_slots
        Vec::new(),
        vec![ProcedureDescriptor {
            name: "Bump".to_string(),
            entry_pc: 3,
            kind: ProcedureKind::Sub,
            param_count: 0,
            frame_slots: 0,
            return_slot: None,
        }],
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_f64(), Some(2.0));
}

#[test]
fn counted_loop_sum() {
    // acc = 0; For i = 1 To 5: acc = acc + i.  Sum = 15.
    let b = bundle(
        vec![
            Op::LoadI32 { slot: 0, value: 0 },                                     // 0 acc
            Op::LoadI32 { slot: 1, value: 1 },                                     // 1 i
            Op::LoadI32 { slot: 2, value: 6 },                                     // 2 limit
            Op::CmpLt { dst: 3, lhs: 1, rhs: 2, mode: StringCompareMode::Binary }, // 3 i<6
            Op::JumpIfZero { cond_slot: 3, target_pc: 8 },                         // 4 exit
            Op::Add { dst: 0, lhs: 0, rhs: 1 },                                    // 5 acc+=i
            Op::IncSlot { slot: 1 },                                               // 6 i++
            Op::Jump { target_pc: 3 },                                            // 7 loop
            Op::Halt,                                                              // 8
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
            Op::SetOnErrorResumeNext,           // 0
            Op::RaiseError { code: 11 },        // 1 (resume → 2)
            Op::LoadI32 { slot: 0, value: 99 }, // 2
            Op::Halt,                           // 3
        ],
        1,
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(99));
}

#[test]
fn resume_next_is_statement_granular() {
    // Statement at pc 1..3 errors mid-way; Resume Next skips the *rest of the
    // statement* (pc 3) and continues at the next statement (pc 4).
    let b = bundle_full(
        vec![
            Op::SetOnErrorResumeNext,           // 0
            Op::LoadI32 { slot: 0, value: 1 },  // 1 (statement start)
            Op::RaiseError { code: 5 },         // 2 (errors → next statement)
            Op::LoadI32 { slot: 0, value: 2 },  // 3 (same statement — must be skipped)
            Op::LoadI32 { slot: 1, value: 99 }, // 4 (next statement)
            Op::Halt,                           // 5
        ],
        0,
        2,
        vec![0, 1, 4, 5],
        Vec::new(),
    );
    let h = host();
    let vm = run(&b, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(1), "pc 3 must be skipped");
    assert_eq!(vm.slot(1).unwrap().as_i32(), Some(99));
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
