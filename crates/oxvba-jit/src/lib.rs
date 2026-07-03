//! oxvba-jit: Cranelift backend boundary.
//!
//! M4-3 lands the first real JIT slice: a deliberately narrow Cranelift compiler that
//! runs straight-line Long arithmetic and cleanly declines everything else. The decline
//! path is explicit and whole-program; there is no VM fallback in this crate.

use cranelift_codegen::ir::{self, AbiParam, InstBuilder, UserFuncName, Value, types};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId as ClifFuncId, Linkage, Module, default_libcall_names};
use oxvba_bundle::{NumericCoerceTarget, NumericMode};
use oxvba_hal::HostServices;
use oxvba_oxir::{
    ArithOp, BlockId, FuncId, OxCoerceTarget, OxConst, OxFunc, OxInst, OxOperand, OxPlace,
    OxProgram, OxTerminator, OxTy,
};
use oxvba_rt_abi::{
    ExecState, LoadedProgram, RawExecState, ST_FAULT, ST_HALT, ST_OK, exec_state_as_raw,
    rt_add_i32, rt_currency_add, rt_currency_mul, rt_currency_sub, rt_maybe_drain, rt_mul_i32,
    rt_sub_i32,
};
use oxvba_runtime::Variant;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use thiserror::Error;

pub const JIT_NOT_IMPLEMENTED_MESSAGE: &str =
    "JIT execution is not implemented for this OxIR shape";

const AREA_GLOBAL: u32 = 0;
const AREA_LOCAL: u32 = 1;
const AREA_TEMP: u32 = 2;

type JitEntryFn = unsafe extern "C" fn(*mut JitRun, *mut RawExecState) -> i32;

/// Final `Err` state surfaced by the JIT backend without depending on `oxvba-host`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JitFinalErr {
    pub number: i32,
    pub source: String,
    pub description: String,
    pub last_dll_error: i32,
}

/// Observable result of a JIT run.
#[derive(Debug, Clone)]
pub struct JitOutcome {
    pub values: Vec<Variant>,
    pub err: JitFinalErr,
    pub raised: bool,
}

#[derive(Debug, Error)]
pub enum JitError {
    #[error("jit: unsupported: {0}")]
    Unsupported(String),
    #[error("jit compile: {0}")]
    Compile(String),
    #[error("jit runtime: {0}")]
    Runtime(String),
}

impl JitError {
    pub fn unsupported(what: impl Into<String>) -> Self {
        Self::Unsupported(what.into())
    }

    pub fn unsupported_message(&self) -> Option<&str> {
        match self {
            Self::Unsupported(what) => Some(what.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct JitEngine;

impl JitEngine {
    /// Legacy probe retained for callers that only want a boundary health check.
    pub fn compile_function(&self, _symbol: &str) -> Result<(), JitError> {
        Err(JitError::Unsupported(
            "function-symbol compilation is superseded by compile_image".to_string(),
        ))
    }

    /// Compile one link image. M4-3 supports the single-program case and compiles every
    /// function in that program up front.
    pub fn compile_image<'p>(
        &self,
        programs: &[&'p OxProgram],
    ) -> Result<CompiledImage<'p>, JitError> {
        Compiler::compile_image(programs)
    }
}

/// A compiled single-program image. Function pointers stay valid because the JITModule is
/// owned by this value.
pub struct CompiledImage<'p> {
    #[allow(dead_code)]
    module: JITModule,
    program: &'p OxProgram,
    functions: Vec<JitEntryFn>,
}

impl<'p> CompiledImage<'p> {
    pub fn run<'a>(&'a self, host: &'a dyn HostServices) -> Result<JitOutcome, JitError> {
        let mut exec = ExecState::new(host);
        exec.programs = vec![build_loaded(self.program)];
        let globals_ptr = &mut exec.programs[0].globals as *mut Vec<Variant>;
        let mut run = JitRun {
            globals: globals_ptr,
            locals: Vec::new(),
            temps: Vec::new(),
        };
        let state = exec_state_as_raw(&mut exec);

        oxvba_runtime::reset_pending_terminations();
        if let Some(init) = self.program.global_initializer {
            let status = self.invoke_func(init, &mut run, state)?;
            if status == ST_FAULT {
                return Ok(JitOutcome {
                    values: Vec::new(),
                    err: err_from_exec(&exec),
                    raised: true,
                });
            }
            if status == ST_HALT {
                return Ok(JitOutcome {
                    values: snapshot_values(&exec, &run),
                    err: err_from_exec(&exec),
                    raised: false,
                });
            }
        }

        if let Some(entry) = self.program.entry {
            let status = self.invoke_func(entry, &mut run, state)?;
            if status == ST_FAULT {
                return Ok(JitOutcome {
                    values: Vec::new(),
                    err: err_from_exec(&exec),
                    raised: true,
                });
            }
            if status == ST_HALT {
                return Ok(JitOutcome {
                    values: snapshot_values(&exec, &run),
                    err: err_from_exec(&exec),
                    raised: false,
                });
            }
        }

        let drain_status = rt_maybe_drain(state);
        if drain_status == ST_FAULT {
            return Ok(JitOutcome {
                values: Vec::new(),
                err: err_from_exec(&exec),
                raised: true,
            });
        }

        Ok(JitOutcome {
            values: snapshot_values(&exec, &run),
            err: err_from_exec(&exec),
            raised: false,
        })
    }

    fn invoke_func(
        &self,
        func: FuncId,
        run: &mut JitRun,
        state: *mut RawExecState,
    ) -> Result<i32, JitError> {
        let f = self
            .program
            .funcs
            .get(func.0)
            .ok_or_else(|| JitError::Runtime(format!("function {} out of range", func.0)))?;
        run.locals = f
            .locals
            .iter()
            .map(|_| Variant::empty())
            .collect::<Vec<_>>();
        run.temps = f.temps.iter().map(|_| Variant::empty()).collect::<Vec<_>>();
        let entry = *self
            .functions
            .get(func.0)
            .ok_or_else(|| JitError::Runtime(format!("function {} not compiled", func.0)))?;
        // SAFETY: `entry` was produced by Cranelift for the exact `JitEntryFn`
        // signature in `Compiler::entry_signature`; `run` and `state` live for the call.
        Ok(unsafe { entry(run, state) })
    }
}

struct JitRun {
    globals: *mut Vec<Variant>,
    locals: Vec<Variant>,
    temps: Vec<Variant>,
}

fn build_loaded<'p>(program: &'p OxProgram) -> LoadedProgram<'p> {
    LoadedProgram {
        program,
        globals: program.globals.iter().map(|_| Variant::empty()).collect(),
        class_descriptors: Vec::new(),
        predeclared_singletons: HashMap::new(),
        event_routes: HashMap::new(),
    }
}

fn err_from_exec(exec: &ExecState<'_>) -> JitFinalErr {
    JitFinalErr {
        number: exec.err_engine.err.number,
        source: exec.err_engine.err.source.clone(),
        description: exec.err_engine.err.description.clone(),
        last_dll_error: exec.err_engine.last_dll_error,
    }
}

fn snapshot_values(exec: &ExecState<'_>, run: &JitRun) -> Vec<Variant> {
    let mut values = exec
        .programs
        .first()
        .map(|loaded| loaded.globals.clone())
        .unwrap_or_default();
    values.extend(run.locals.iter().cloned());
    values
}

#[derive(Clone, Copy)]
struct Imports {
    load_i32: ClifFuncId,
    store_i32: ClifFuncId,
    add_i32_slot: ClifFuncId,
    sub_i32_slot: ClifFuncId,
    mul_i32_slot: ClifFuncId,
}

struct Compiler<'p> {
    module: JITModule,
    imports: Imports,
    program: &'p OxProgram,
    clif_ids: Vec<ClifFuncId>,
}

impl<'p> Compiler<'p> {
    fn compile_image(programs: &[&'p OxProgram]) -> Result<CompiledImage<'p>, JitError> {
        let [program] = programs else {
            return Err(JitError::unsupported(
                "M4-3 supports one OxProgram image; cross-project images start in M4-4+",
            ));
        };
        validate_program_shape(program)?;

        let mut builder = jit_builder()?;
        register_symbols(&mut builder);
        let mut module = JITModule::new(builder);
        let imports = declare_imports(&mut module)?;
        let clif_ids = declare_program_functions(&mut module, program)?;
        let mut compiler = Self {
            module,
            imports,
            program,
            clif_ids,
        };
        compiler.define_program_functions()?;
        compiler.module.finalize_definitions().map_err(module_err)?;

        let functions = compiler
            .clif_ids
            .iter()
            .map(|id| {
                let ptr = compiler.module.get_finalized_function(*id);
                // SAFETY: every declared local function uses `entry_signature`, which is
                // exactly `unsafe extern "C" fn(*mut JitRun, *mut RawExecState) -> i32`.
                unsafe { std::mem::transmute::<*const u8, JitEntryFn>(ptr) }
            })
            .collect();

        Ok(CompiledImage {
            module: compiler.module,
            program,
            functions,
        })
    }

    fn define_program_functions(&mut self) -> Result<(), JitError> {
        let mut ctx = self.module.make_context();
        let mut func_ctx = FunctionBuilderContext::new();
        for index in 0..self.program.funcs.len() {
            self.define_function(index, &mut ctx, &mut func_ctx)?;
            self.module.clear_context(&mut ctx);
        }
        Ok(())
    }

    fn define_function(
        &mut self,
        index: usize,
        ctx: &mut cranelift_codegen::Context,
        func_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), JitError> {
        let func = &self.program.funcs[index];
        validate_func_shape(func)?;
        ctx.func.signature = entry_signature(&mut self.module);
        ctx.func.name = UserFuncName::user(0, self.clif_ids[index].as_u32());

        {
            let mut builder = FunctionBuilder::new(&mut ctx.func, func_ctx);
            let entry = builder.create_block();
            builder.switch_to_block(entry);
            builder.append_block_params_for_function_params(entry);
            let run = builder.block_params(entry)[0];
            let state = builder.block_params(entry)[1];

            let mut blocks = HashMap::new();
            for block in &func.blocks {
                blocks.insert(block.id, builder.create_block());
            }
            let Some(first) = blocks.get(&func.entry).copied() else {
                return Err(JitError::Compile(format!(
                    "entry block {} missing in {}",
                    func.entry.0, func.name
                )));
            };
            builder.ins().jump(first, &[]);

            let lower = LowerFunc {
                program: self.program,
                func,
                imports: self.imports,
                blocks,
                run,
                state,
            };
            lower.define_blocks(&mut builder, &mut self.module)?;
            builder.seal_all_blocks();
            builder.finalize();
        }

        self.module
            .define_function(self.clif_ids[index], ctx)
            .map_err(module_err)
    }
}

struct LowerFunc<'a> {
    program: &'a OxProgram,
    func: &'a OxFunc,
    imports: Imports,
    blocks: HashMap<BlockId, ir::Block>,
    run: Value,
    state: Value,
}

impl<'a> LowerFunc<'a> {
    fn define_blocks(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
    ) -> Result<(), JitError> {
        for block in &self.func.blocks {
            let clif_block = self.clif_block(block.id)?;
            builder.switch_to_block(clif_block);
            for inst in &block.instrs {
                self.lower_inst(builder, module, inst)?;
            }
            self.lower_terminator(builder, &block.terminator)?;
        }
        Ok(())
    }

    fn clif_block(&self, id: BlockId) -> Result<ir::Block, JitError> {
        self.blocks
            .get(&id)
            .copied()
            .ok_or_else(|| JitError::Compile(format!("block {} was not created", id.0)))
    }

    fn lower_inst(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        inst: &OxInst,
    ) -> Result<(), JitError> {
        match inst {
            OxInst::StmtBoundary { .. } | OxInst::SetLineNumber { .. } => Ok(()),
            OxInst::DrainTerminations => Ok(()),
            OxInst::Assign { dst, value } => {
                self.ensure_long_place(*dst)?;
                let value = self.lower_operand_i32(builder, module, value)?;
                self.emit_store_i32(builder, module, *dst, value)
            }
            OxInst::Coerce { dst, src, target } => {
                self.ensure_long_place(*dst)?;
                match target {
                    OxCoerceTarget::Numeric(NumericCoerceTarget::Long) => {
                        let value = self.lower_operand_i32(builder, module, src)?;
                        self.emit_store_i32(builder, module, *dst, value)
                    }
                    _ => Err(JitError::unsupported(format!(
                        "M4-3 lowers only Numeric(Long) coercions, got {target:?}"
                    ))),
                }
            }
            OxInst::Arith {
                dst,
                op,
                lhs,
                rhs,
                mode,
            } => {
                self.ensure_long_place(*dst)?;
                let helper = match (op, mode) {
                    (ArithOp::Add, NumericMode::Checked(NumericCoerceTarget::Long)) => {
                        self.imports.add_i32_slot
                    }
                    (ArithOp::Sub, NumericMode::Checked(NumericCoerceTarget::Long)) => {
                        self.imports.sub_i32_slot
                    }
                    (ArithOp::Mul, NumericMode::Checked(NumericCoerceTarget::Long)) => {
                        self.imports.mul_i32_slot
                    }
                    _ => {
                        return Err(JitError::unsupported(format!(
                            "M4-3 lowers only checked Long add/sub/mul, got {op:?} {mode:?}",
                        )));
                    }
                };
                let lhs = self.lower_operand_i32(builder, module, lhs)?;
                let rhs = self.lower_operand_i32(builder, module, rhs)?;
                let (area, index) = place_addr(*dst);
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                let callee = self.import(builder, module, helper);
                let call = builder
                    .ins()
                    .call(callee, &[self.state, self.run, lhs, rhs, area, index]);
                let status = builder.inst_results(call)[0];
                self.return_if_not_ok(builder, status);
                Ok(())
            }
            other => Err(JitError::unsupported(format!(
                "instruction not lowered in M4-3: {other:?}"
            ))),
        }
    }

    fn lower_terminator(
        &self,
        builder: &mut FunctionBuilder<'_>,
        term: &OxTerminator,
    ) -> Result<(), JitError> {
        match term {
            OxTerminator::Jump(target) => {
                builder.ins().jump(self.clif_block(*target)?, &[]);
                Ok(())
            }
            OxTerminator::Return => {
                let ok = builder.ins().iconst(types::I32, i64::from(ST_OK));
                builder.ins().return_(&[ok]);
                Ok(())
            }
            OxTerminator::Halt => {
                let halt = builder.ins().iconst(types::I32, i64::from(ST_HALT));
                builder.ins().return_(&[halt]);
                Ok(())
            }
            OxTerminator::FaultDispatch { .. } => {
                let fault = builder.ins().iconst(types::I32, i64::from(ST_FAULT));
                builder.ins().return_(&[fault]);
                Ok(())
            }
            other => Err(JitError::unsupported(format!(
                "terminator not lowered in M4-3: {other:?}"
            ))),
        }
    }

    fn lower_operand_i32(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        operand: &OxOperand,
    ) -> Result<Value, JitError> {
        match operand {
            OxOperand::Const(OxConst::I32(value)) => {
                Ok(builder.ins().iconst(types::I32, i64::from(*value)))
            }
            OxOperand::Const(OxConst::I16(value)) => {
                Ok(builder.ins().iconst(types::I32, i64::from(*value)))
            }
            OxOperand::Const(OxConst::Bool(value)) => {
                let raw = if *value { -1 } else { 0 };
                Ok(builder.ins().iconst(types::I32, raw))
            }
            OxOperand::Use(place) => {
                self.ensure_long_place(*place)?;
                let (area, index) = place_addr(*place);
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                let callee = self.import(builder, module, self.imports.load_i32);
                let call = builder.ins().call(callee, &[self.run, area, index]);
                Ok(builder.inst_results(call)[0])
            }
            other => Err(JitError::unsupported(format!(
                "i32 operand not lowered in M4-3: {other:?}"
            ))),
        }
    }

    fn emit_store_i32(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        place: OxPlace,
        value: Value,
    ) -> Result<(), JitError> {
        let (area, index) = place_addr(place);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.store_i32);
        let call = builder.ins().call(callee, &[self.run, area, index, value]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn return_if_not_ok(&self, builder: &mut FunctionBuilder<'_>, status: Value) {
        let ok = builder
            .ins()
            .icmp_imm(ir::condcodes::IntCC::Equal, status, i64::from(ST_OK));
        let cont = builder.create_block();
        let ret = builder.create_block();
        builder.append_block_param(ret, types::I32);
        builder.ins().brif(ok, cont, &[], ret, &[status.into()]);

        builder.switch_to_block(ret);
        let code = builder.block_params(ret)[0];
        builder.ins().return_(&[code]);

        builder.switch_to_block(cont);
    }

    fn import(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        id: ClifFuncId,
    ) -> ir::FuncRef {
        module.declare_func_in_func(id, &mut builder.func)
    }

    fn ensure_long_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::Long) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-3 lowers only Long places, got {ty:?} at {place:?}"
            )))
        }
    }
}

fn jit_builder() -> Result<JITBuilder, JitError> {
    let opt_level = if std::env::var_os("OXVBA_JIT_DEBUG_OPT_NONE").is_some() {
        "none"
    } else {
        "speed"
    };
    let flags = [
        ("opt_level", opt_level),
        ("enable_verifier", "true"),
        ("is_pic", "false"),
        ("use_colocated_libcalls", "false"),
    ];
    let mut flag_builder = settings::builder();
    for (name, value) in flags {
        flag_builder
            .set(name, value)
            .map_err(|err| JitError::Compile(err.to_string()))?;
    }
    let isa_builder = cranelift_native::builder()
        .map_err(|msg| JitError::Compile(format!("native ISA unavailable: {msg}")))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|err| JitError::Compile(err.to_string()))?;
    Ok(JITBuilder::with_isa(isa, default_libcall_names()))
}

fn register_symbols(builder: &mut JITBuilder) {
    builder.symbol("rt_jit_load_i32", rt_jit_load_i32 as *const u8);
    builder.symbol("rt_jit_store_i32", rt_jit_store_i32 as *const u8);
    builder.symbol(
        "rt_jit_add_i32_to_slot",
        rt_jit_add_i32_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_sub_i32_to_slot",
        rt_jit_sub_i32_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_mul_i32_to_slot",
        rt_jit_mul_i32_to_slot as *const u8,
    );
    builder.symbol("rt_add_i32", rt_add_i32 as *const u8);
    builder.symbol("rt_sub_i32", rt_sub_i32 as *const u8);
    builder.symbol("rt_mul_i32", rt_mul_i32 as *const u8);
    builder.symbol("rt_currency_add", rt_currency_add as *const u8);
    builder.symbol("rt_currency_sub", rt_currency_sub as *const u8);
    builder.symbol("rt_currency_mul", rt_currency_mul as *const u8);
}

fn declare_imports(module: &mut JITModule) -> Result<Imports, JitError> {
    let ptr_ty = module.target_config().pointer_type();

    let mut load_sig = module.make_signature();
    load_sig.params.push(AbiParam::new(ptr_ty));
    load_sig.params.push(AbiParam::new(types::I32));
    load_sig.params.push(AbiParam::new(types::I32));
    load_sig.returns.push(AbiParam::new(types::I32));
    let load_i32 = module
        .declare_function("rt_jit_load_i32", Linkage::Import, &load_sig)
        .map_err(module_err)?;

    let mut store_sig = module.make_signature();
    store_sig.params.push(AbiParam::new(ptr_ty));
    store_sig.params.push(AbiParam::new(types::I32));
    store_sig.params.push(AbiParam::new(types::I32));
    store_sig.params.push(AbiParam::new(types::I32));
    store_sig.returns.push(AbiParam::new(types::I32));
    let store_i32 = module
        .declare_function("rt_jit_store_i32", Linkage::Import, &store_sig)
        .map_err(module_err)?;

    let mut slot_sig = module.make_signature();
    slot_sig.params.push(AbiParam::new(ptr_ty));
    slot_sig.params.push(AbiParam::new(ptr_ty));
    slot_sig.params.push(AbiParam::new(types::I32));
    slot_sig.params.push(AbiParam::new(types::I32));
    slot_sig.params.push(AbiParam::new(types::I32));
    slot_sig.params.push(AbiParam::new(types::I32));
    slot_sig.returns.push(AbiParam::new(types::I32));
    let add_i32_slot = module
        .declare_function("rt_jit_add_i32_to_slot", Linkage::Import, &slot_sig)
        .map_err(module_err)?;
    let sub_i32_slot = module
        .declare_function("rt_jit_sub_i32_to_slot", Linkage::Import, &slot_sig)
        .map_err(module_err)?;
    let mul_i32_slot = module
        .declare_function("rt_jit_mul_i32_to_slot", Linkage::Import, &slot_sig)
        .map_err(module_err)?;

    Ok(Imports {
        load_i32,
        store_i32,
        add_i32_slot,
        sub_i32_slot,
        mul_i32_slot,
    })
}

fn declare_program_functions(
    module: &mut JITModule,
    program: &OxProgram,
) -> Result<Vec<ClifFuncId>, JitError> {
    let sig = entry_signature(module);
    program
        .funcs
        .iter()
        .enumerate()
        .map(|(index, func)| {
            let name = format!("ox$p0$f{index}${}", sanitize_symbol(&func.name));
            module
                .declare_function(&name, Linkage::Local, &sig)
                .map_err(module_err)
        })
        .collect()
}

fn entry_signature(module: &mut JITModule) -> ir::Signature {
    let ptr_ty = module.target_config().pointer_type();
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(ptr_ty));
    sig.params.push(AbiParam::new(ptr_ty));
    sig.returns.push(AbiParam::new(types::I32));
    sig
}

fn sanitize_symbol(name: &str) -> String {
    let mut out = String::with_capacity(name.len().max(1));
    for ch in name.chars() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { "_".to_string() } else { out }
}

fn validate_program_shape(program: &OxProgram) -> Result<(), JitError> {
    if !program.classes.is_empty() {
        return Err(JitError::unsupported("project classes start in M4-8"));
    }
    if !program.imports.is_empty() {
        return Err(JitError::unsupported(
            "cross-program/library imports start in M4-4/M4-9",
        ));
    }
    if !program.external_calls.is_empty() || !program.com_interfaces.is_empty() {
        return Err(JitError::unsupported("native/COM calls start in M4-9"));
    }
    for global in &program.globals {
        if !matches!(global.ty, OxTy::Long) {
            return Err(JitError::unsupported(format!(
                "M4-3 lowers only Long globals, got {:?}",
                global.ty
            )));
        }
    }
    Ok(())
}

fn validate_func_shape(func: &OxFunc) -> Result<(), JitError> {
    if func.param_count != 0 {
        return Err(JitError::unsupported(format!(
            "procedure parameters start in M4-4: {}",
            func.name
        )));
    }
    if func.return_local.is_some() {
        return Err(JitError::unsupported(format!(
            "function return locals start in M4-4: {}",
            func.name
        )));
    }
    for local in &func.locals {
        if !matches!(local.ty, OxTy::Long) {
            return Err(JitError::unsupported(format!(
                "M4-3 lowers only Long locals, got {:?} in {}",
                local.ty, func.name
            )));
        }
    }
    for ty in &func.temps {
        if !matches!(ty, OxTy::Long) {
            return Err(JitError::unsupported(format!(
                "M4-3 lowers only Long temps, got {ty:?} in {}",
                func.name
            )));
        }
    }
    Ok(())
}

fn place_ty<'a>(
    program: &'a OxProgram,
    func: &'a OxFunc,
    place: OxPlace,
) -> Result<&'a OxTy, JitError> {
    match place {
        OxPlace::Local(id) => func
            .locals
            .get(id.0)
            .map(|local| &local.ty)
            .ok_or_else(|| JitError::Compile(format!("local {} out of range", id.0))),
        OxPlace::Global(id) => program
            .globals
            .get(id.0)
            .map(|global| &global.ty)
            .ok_or_else(|| JitError::Compile(format!("global {} out of range", id.0))),
        OxPlace::Temp(id) => func
            .temps
            .get(id.0)
            .ok_or_else(|| JitError::Compile(format!("temp {} out of range", id.0))),
    }
}

fn place_addr(place: OxPlace) -> (u32, u32) {
    match place {
        OxPlace::Global(id) => (AREA_GLOBAL, id.0 as u32),
        OxPlace::Local(id) => (AREA_LOCAL, id.0 as u32),
        OxPlace::Temp(id) => (AREA_TEMP, id.0 as u32),
    }
}

fn module_err(err: impl std::fmt::Display) -> JitError {
    JitError::Compile(err.to_string())
}

fn slot_mut(run: &mut JitRun, area: u32, index: u32) -> Option<&mut Variant> {
    let index = index as usize;
    match area {
        AREA_GLOBAL => {
            if run.globals.is_null() {
                None
            } else {
                // SAFETY: `globals` is installed from the live ExecState global vector for
                // the duration of a compiled function invocation.
                unsafe { (&mut *run.globals).get_mut(index) }
            }
        }
        AREA_LOCAL => run.locals.get_mut(index),
        AREA_TEMP => run.temps.get_mut(index),
        _ => None,
    }
}

fn slot_ref(run: &JitRun, area: u32, index: u32) -> Option<&Variant> {
    let index = index as usize;
    match area {
        AREA_GLOBAL => {
            if run.globals.is_null() {
                None
            } else {
                // SAFETY: `globals` is installed from the live ExecState global vector for
                // the duration of a compiled function invocation.
                unsafe { (&*run.globals).get(index) }
            }
        }
        AREA_LOCAL => run.locals.get(index),
        AREA_TEMP => run.temps.get(index),
        _ => None,
    }
}

fn status_guard(work: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or(ST_FAULT)
}

unsafe extern "C" fn rt_jit_load_i32(run: *mut JitRun, area: u32, index: u32) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if run.is_null() {
            return 0;
        }
        // SAFETY: null was rejected and the compiled call gives unique run ownership.
        let run = unsafe { &*run };
        match slot_ref(run, area, index) {
            Some(value) => value
                .as_i32()
                .or_else(|| value.as_i16().map(i32::from))
                .unwrap_or(0),
            None => 0,
        }
    }))
    .unwrap_or(0)
}

unsafe extern "C" fn rt_jit_store_i32(run: *mut JitRun, area: u32, index: u32, value: i32) -> i32 {
    status_guard(|| {
        if run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled call gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, area, index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_i32(value);
        ST_OK
    })
}

unsafe extern "C" fn rt_jit_add_i32_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    checked_i32_to_slot(state, run, lhs, rhs, area, index, rt_add_i32)
}

unsafe extern "C" fn rt_jit_sub_i32_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    checked_i32_to_slot(state, run, lhs, rhs, area, index, rt_sub_i32)
}

unsafe extern "C" fn rt_jit_mul_i32_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    checked_i32_to_slot(state, run, lhs, rhs, area, index, rt_mul_i32)
}

fn checked_i32_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
    shim: extern "C" fn(*mut RawExecState, i32, i32, *mut i32) -> i32,
) -> i32 {
    status_guard(|| {
        let mut out = 0;
        let status = shim(state, lhs, rhs, &mut out);
        if status != ST_OK {
            return status;
        }
        // SAFETY: forwarding the same run pointer received from compiled code.
        unsafe { rt_jit_store_i32(run, area, index, out) }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxvba_bundle::{ProcedureKind, ProjectMemberKind};
    use oxvba_hal::HostPolicy;
    use oxvba_hal::adapters::null::NullHostServices;
    use oxvba_oxir::{
        GlobalId, LocalId, OxBlock, OxGlobal, OxInst, OxLocal, TempId, verify_program,
    };

    fn straight_line_program() -> OxProgram {
        let n = LocalId(0);
        let t0 = TempId(0);
        let long = NumericMode::Checked(NumericCoerceTarget::Long);
        let entry = OxBlock {
            id: BlockId(0),
            instrs: vec![
                OxInst::Assign {
                    dst: OxPlace::Local(n),
                    value: OxOperand::Const(OxConst::I32(10)),
                },
                OxInst::Arith {
                    dst: OxPlace::Temp(t0),
                    op: ArithOp::Add,
                    lhs: OxOperand::Use(OxPlace::Local(n)),
                    rhs: OxOperand::Const(OxConst::I32(5)),
                    mode: long,
                },
                OxInst::Arith {
                    dst: OxPlace::Local(n),
                    op: ArithOp::Mul,
                    lhs: OxOperand::Use(OxPlace::Temp(t0)),
                    rhs: OxOperand::Const(OxConst::I32(2)),
                    mode: long,
                },
            ],
            fault_target: Some(BlockId(1)),
            terminator: OxTerminator::Return,
        };
        let fault = OxBlock {
            id: BlockId(1),
            instrs: Vec::new(),
            fault_target: None,
            terminator: OxTerminator::FaultDispatch {
                resume: BlockId(0),
                resume_next: BlockId(2),
            },
        };
        let exit = OxBlock {
            id: BlockId(2),
            instrs: Vec::new(),
            fault_target: None,
            terminator: OxTerminator::Return,
        };
        let main = OxFunc {
            name: "Main".to_string(),
            kind: ProcedureKind::Sub,
            locals: vec![OxLocal {
                name: "n".to_string(),
                ty: OxTy::Long,
                array_element: None,
                param: None,
                escaped: false,
            }],
            temps: vec![OxTy::Long],
            param_count: 0,
            return_local: None,
            blocks: vec![entry, fault, exit],
            entry: BlockId(0),
        };
        OxProgram {
            funcs: vec![main],
            globals: vec![OxGlobal {
                name: "g".to_string(),
                ty: OxTy::Long,
                array_element: None,
            }],
            entry: Some(FuncId(0)),
            unit_name: "VBAProject".to_string(),
            ..OxProgram::empty()
        }
    }

    #[test]
    fn jit_compiles_and_runs_straight_line_long_arithmetic() {
        let program = straight_line_program();
        assert_eq!(verify_program(&program), Ok(()));
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(!outcome.raised);
        assert_eq!(outcome.values.get(1).and_then(Variant::as_i32), Some(30));
    }

    #[test]
    fn jit_overflow_raises_through_rt_abi_shim() {
        let mut program = straight_line_program();
        if let OxInst::Assign { value, .. } = &mut program.funcs[0].blocks[0].instrs[0] {
            *value = OxOperand::Const(OxConst::I32(i32::MAX));
        }
        if let OxInst::Arith { rhs, .. } = &mut program.funcs[0].blocks[0].instrs[1] {
            *rhs = OxOperand::Const(OxConst::I32(1));
        }
        program.funcs[0].blocks[0].instrs.truncate(2);
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(outcome.raised);
        assert_eq!(outcome.err.number, 6);
    }

    #[test]
    fn jit_initializer_fault_status_is_observed() {
        let mut program = straight_line_program();
        program.global_initializer = Some(FuncId(0));
        program.entry = None;
        if let OxInst::Assign { value, .. } = &mut program.funcs[0].blocks[0].instrs[0] {
            *value = OxOperand::Const(OxConst::I32(i32::MAX));
        }
        if let OxInst::Arith { rhs, .. } = &mut program.funcs[0].blocks[0].instrs[1] {
            *rhs = OxOperand::Const(OxConst::I32(1));
        }
        program.funcs[0].blocks[0].instrs.truncate(2);
        let engine = JitEngine;
        let compiled = engine.compile_image(&[&program]).expect("compile");
        let host = NullHostServices::new(HostPolicy::default());
        let outcome = compiled.run(&host).expect("run");
        assert!(outcome.raised);
        assert_eq!(outcome.err.number, 6);
    }

    #[test]
    fn jit_declines_branch_until_control_flow_milestone() {
        let mut program = straight_line_program();
        program.funcs[0].blocks[0].terminator = OxTerminator::Branch {
            cond: OxOperand::Const(OxConst::Bool(true)),
            then_blk: BlockId(0),
            else_blk: BlockId(0),
        };
        let engine = JitEngine;
        match engine.compile_image(&[&program]) {
            Err(JitError::Unsupported(_)) => {}
            Err(other) => panic!("expected unsupported branch, got {other:?}"),
            Ok(_) => panic!("branch should be unsupported in M4-3"),
        }
    }

    #[test]
    fn symbol_sanitizer_keeps_stable_names() {
        let name = sanitize_symbol("Main.Worker$");
        assert_eq!(name, "Main_Worker_");
        let _ = ProjectMemberKind::Method;
        let _ = GlobalId(0);
    }
}
