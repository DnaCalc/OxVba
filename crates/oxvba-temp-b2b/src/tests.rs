//! Bridge smoke tests: hand-built legacy bytecode → clean bundle → run on
//! oxvba-vm2, plus an end-to-end compile→lower→run of a real source.

use oxvba_compiler::bytecode::{Bytecode, Instruction};
use oxvba_hal::HostPolicy;
use oxvba_hal::adapters::null::NullHostServices;

use crate::{lower, lower_bytecode};

fn host() -> NullHostServices {
    NullHostServices::new(HostPolicy::deterministic_runtime())
}

fn bytecode(instructions: Vec<Instruction>, slot_count: usize) -> Bytecode {
    Bytecode {
        instructions,
        external_call_descriptors: Vec::new(),
        slot_count,
        user_slot_count: slot_count,
    }
}

#[test]
fn lowers_and_runs_arithmetic() {
    // x = 7 ; x = x + 1  →  8
    let bc = bytecode(
        vec![
            Instruction::LoadConstI32 { slot: 0, value: 7 },
            Instruction::AddConstI32 { slot: 0, value: 1 },
            Instruction::Halt,
        ],
        1,
    );
    let bundle = lower_bytecode(&bc);
    let h = host();
    let vm = oxvba_vm2::run(&bundle, &h).unwrap();
    assert_eq!(vm.slot(0).unwrap().as_i32(), Some(8));
}

#[test]
fn lowers_library_call_through_vm_and_lib() {
    // x = Len("hi")  →  2  (Intrinsic → CallNative{Builtin(Len)} → oxvba_lib)
    let bc = bytecode(
        vec![
            Instruction::LoadConstString { slot: 0, value: "hi".to_string() },
            Instruction::IntrinsicLenDigits { dst: 1, src: 0 },
            Instruction::Halt,
        ],
        2,
    );
    let bundle = lower_bytecode(&bc);
    let h = host();
    let vm = oxvba_vm2::run(&bundle, &h).unwrap();
    assert_eq!(vm.slot(1).unwrap().as_i32(), Some(2));
}

#[test]
fn end_to_end_compile_lower_run() {
    // The full Phase-1 pipeline: front-end → legacy bundle → b2b → clean bundle
    // → vm2. A call-free body runs to completion without error.
    let src = "Sub Main()\r\n    Dim x As Long\r\n    x = 1 + 2 * 3\r\nEnd Sub\r\n";
    let oxbundle = oxvba_compiler::compile_source_to_bundle(src).expect("compile");
    let bundle = lower(&oxbundle);
    let h = host();
    assert!(oxvba_vm2::run(&bundle, &h).is_ok(), "lowered bundle runs on vm2");
}
