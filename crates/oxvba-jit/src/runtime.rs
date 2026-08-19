//! JIT engine, compiled image, frames, and run-state helpers.

use super::*;

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
    pub(crate) module: JITModule,
    pub(crate) programs: Vec<&'p OxProgram>,
    pub(crate) functions: Vec<Vec<JitEntryFn>>,
}

impl<'p> CompiledImage<'p> {
    pub fn run<'a>(&'a self, host: &'a dyn HostServices) -> Result<JitOutcome, JitError> {
        let Some(entry_program_index) = self.programs.len().checked_sub(1) else {
            return Err(JitError::Runtime(
                "compiled image has no programs".to_string(),
            ));
        };
        let entry_program = self.programs[entry_program_index];
        let mut exec = ExecState::new(host);
        exec.default_error_source = entry_program.unit_name.clone();
        exec.programs = self
            .programs
            .iter()
            .map(|program| build_loaded(program))
            .collect::<Result<Vec<_>, _>>()?;
        let mut globals = exec
            .programs
            .iter_mut()
            .map(|loaded| &mut loaded.globals as *mut Vec<Variant>)
            .collect::<Vec<_>>();
        let program_images = self
            .programs
            .iter()
            .zip(self.functions.iter())
            .map(|(program, functions)| JitProgramImage {
                program: *program as *const OxProgram,
                functions: functions.as_ptr(),
                function_count: functions.len(),
            })
            .collect::<Vec<_>>();
        let mut run = JitRun {
            globals: globals.as_mut_ptr(),
            global_count: globals.len(),
            frames: Vec::new(),
            explicit_refs: Vec::new(),
            for_each: HashMap::new(),
            as_new_slots: HashMap::new(),
            param_array_aliases: HashMap::new(),
            next_collection_instance_id: i32::MIN + 1,
            programs: program_images.as_ptr(),
            program_count: program_images.len(),
        };
        let run_root = &raw mut run;
        let state = exec_state_as_raw(&mut exec);
        let mut bridge_ctx = JitProcInvokeCtx {
            run: run_root,
            state,
        };
        // SAFETY: `state` is the live same-thread run state, and `bridge_ctx`
        // remains live until the registration guard clears the bridge.
        let install_status = unsafe {
            rt_install_proc_invoker(
                state,
                (&raw mut bridge_ctx).cast::<c_void>(),
                Some(jit_proc_invoke),
            )
        };
        if install_status != ST_OK {
            return Err(JitError::Runtime(
                "failed to install the JIT procedure-invocation bridge".to_string(),
            ));
        }
        let _proc_invoker_registration = ProcInvokerRegistration { state };

        oxvba_runtime::reset_pending_terminations();
        if let Some(init) = entry_program.global_initializer {
            // SAFETY: the current compiled-run boundary owns the live unique state handle;
            // typed references and owned values remain live and nonaliasing for this call.
            let status = unsafe { self.invoke_func(entry_program_index, init, run_root, state) }?;
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

        if let Some(entry) = entry_program.entry {
            // SAFETY: the current compiled-run boundary owns the live unique state handle;
            // typed references and owned values remain live and nonaliasing for this call.
            let status = unsafe { self.invoke_func(entry_program_index, entry, run_root, state) }?;
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

        // SAFETY: `state` was derived above from the uniquely borrowed, live `exec`
        // and remains valid for this synchronous runtime call.
        let drain_status = unsafe { rt_maybe_drain(state) };
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

    // SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
    // same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
    // and length arguments must identify the initialized, nonaliasing storage described
    // by their typed parameters for the complete synchronous call.
    unsafe fn invoke_func(
        &self,
        program_index: usize,
        func: FuncId,
        run: *mut JitRun,
        state: *mut RawExecState,
    ) -> Result<i32, JitError> {
        let program = self
            .programs
            .get(program_index)
            .ok_or_else(|| JitError::Runtime(format!("program {program_index} out of range")))?;
        let f = program
            .funcs
            .get(func.0)
            .ok_or_else(|| JitError::Runtime(format!("function {} out of range", func.0)))?;
        if run.is_null() {
            return Err(JitError::Runtime("JIT run root is null".to_string()));
        }
        let entry = *self
            .functions
            .get(program_index)
            .and_then(|functions| functions.get(func.0))
            .ok_or_else(|| {
                JitError::Runtime(format!(
                    "function {} not compiled in program {}",
                    func.0, program_index
                ))
            })?;
        let mut saved_err = RtSavedErrState::default();
        // SAFETY: `state` is the live execution state for this invocation and
        // `saved_err` is initialized, uniquely borrowed output storage.
        let enter_status = unsafe { rt_err_enter_activation(state, &mut saved_err) };
        if enter_status != ST_OK {
            return Ok(enter_status);
        }
        let frame = new_jit_frame(program, program_index, f)?;
        {
            // SAFETY: null was rejected and no other typed run borrow is live.
            // Keep this initialization borrow visibly bounded before compiled entry.
            let run_ref = unsafe { &mut *run };
            run_ref.frames.clear();
            run_ref.explicit_refs.clear();
            run_ref.param_array_aliases.clear();
            run_ref.frames.push(frame);
        }
        // SAFETY: `entry` was produced by Cranelift for the exact `JitEntryFn`
        // signature in `Compiler::entry_signature`; `run` and `state` live for the call.
        let status = unsafe { entry(run, state) };
        // SAFETY: `state` remains live and uniquely owned by the invocation;
        // `saved_err` was initialized by the successful enter call above.
        let restore_status = unsafe { rt_err_restore_activation(state, &saved_err) };
        if restore_status != ST_OK {
            Ok(restore_status)
        } else {
            Ok(status)
        }
    }
}

pub(crate) struct JitRun {
    pub(crate) globals: *mut *mut Vec<Variant>,
    pub(crate) global_count: usize,
    pub(crate) frames: Vec<JitFrame>,
    pub(crate) explicit_refs: Vec<Variant>,
    pub(crate) for_each: HashMap<SlotAlias, JitForEachState>,
    pub(crate) as_new_slots: HashMap<SlotAlias, OxAsNew>,
    pub(crate) param_array_aliases: HashMap<SlotAlias, Vec<Option<SlotAlias>>>,
    pub(crate) next_collection_instance_id: i32,
    pub(crate) programs: *const JitProgramImage,
    pub(crate) program_count: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct JitProcInvokeCtx {
    pub(crate) run: *mut JitRun,
    pub(crate) state: *mut RawExecState,
}

pub(crate) struct ProcInvokerRegistration {
    pub(crate) state: *mut RawExecState,
}

impl Drop for ProcInvokerRegistration {
    fn drop(&mut self) {
        // Clear the runtime-held opaque pointer before its stack storage expires.
        // SAFETY: registration and guard are confined to the same live run state.
        let _ = unsafe { rt_clear_proc_invoker(self.state) };
    }
}

#[derive(Clone, Copy)]
pub(crate) struct JitProgramImage {
    pub(crate) program: *const OxProgram,
    pub(crate) functions: *const JitEntryFn,
    pub(crate) function_count: usize,
}

pub(crate) unsafe extern "C" fn jit_proc_invoke(
    ctx: *mut c_void,
    target_prog: usize,
    proc: usize,
    me: *const Variant,
    _suppress: i32,
) -> i32 {
    status_guard(|| {
        if ctx.is_null() || me.is_null() {
            return ST_FAULT;
        }
        // SAFETY: `ctx` is installed from `CompiledImage::run` and remains live while
        // runtime callbacks can synchronously enter compiled procedures. Copying the
        // two raw handles avoids retaining a typed context borrow across recursion.
        let ctx = unsafe { std::ptr::read(ctx.cast::<JitProcInvokeCtx>()) };
        if ctx.run.is_null() || ctx.state.is_null() {
            return ST_FAULT;
        }
        let image = {
            // SAFETY: null was rejected; this read-only borrow ends before entry.
            let run = unsafe { &*ctx.run };
            if run.programs.is_null() || target_prog >= run.program_count {
                return ST_FAULT;
            }
            // SAFETY: target_prog is bounds-checked and the table is live for the run.
            unsafe { *run.programs.add(target_prog) }
        };
        if image.program.is_null() || image.functions.is_null() || proc >= image.function_count {
            return ST_FAULT;
        }
        // SAFETY: installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let Some(func) = program.funcs.get(proc) else {
            return ST_FAULT;
        };
        if func.param_count != 1 {
            return ST_FAULT;
        }
        let Some(param) = func.locals.first() else {
            return ST_FAULT;
        };
        if param.param.as_ref().is_none_or(|param| param.by_ref)
            || !is_jit_variant_carrier_ty(&param.ty)
        {
            return ST_FAULT;
        }
        // SAFETY: the short read-only borrow ends before any runtime/entry call.
        let frame_limit_reached = {
            // SAFETY: null was rejected and this shared borrow ends before entry.
            unsafe { (&*ctx.run).frames.len() >= MAX_JIT_FRAMES }
        };
        if frame_limit_reached {
            // SAFETY: the callback context validation above established that
            // `ctx.state` is the live execution state for this synchronous call.
            return unsafe { rt_raise_out_of_stack(ctx.state) };
        }
        let Ok(mut frame) = new_jit_frame(program, target_prog, func) else {
            return ST_FAULT;
        };
        // SAFETY: `me` was checked non-null and is borrowed only for this synchronous call.
        frame.locals[0] = unsafe { (*me).clone() };
        let mut saved_err = RtSavedErrState::default();
        // SAFETY: the validated callback state is live for the call and
        // `saved_err` is initialized, uniquely borrowed output storage.
        let enter_status = unsafe { rt_err_enter_activation(ctx.state, &mut saved_err) };
        if enter_status != ST_OK {
            return enter_status;
        }
        {
            // SAFETY: no other typed run borrow is live; the borrow ends before entry.
            let run = unsafe { &mut *ctx.run };
            run.frames.push(frame);
        }
        // SAFETY: function pointer bounds were checked above.
        let entry = unsafe { *image.functions.add(proc) };
        // SAFETY: the function pointer uses the JIT entry ABI and the stable raw
        // run root/state remain live. No typed run borrow spans this call.
        let status = unsafe { entry(ctx.run, ctx.state) };
        let cleanup_status = {
            // SAFETY: entry returned and no typed run borrow is live.
            let run = unsafe { &mut *ctx.run };
            let Some(frame) = run.frames.pop() else {
                // SAFETY: the activation was entered successfully and the live
                // callback state remains valid even if the run stack is corrupt.
                let restore_status = unsafe { rt_err_restore_activation(ctx.state, &saved_err) };
                return if restore_status == ST_OK {
                    ST_FAULT
                } else {
                    restore_status
                };
            };
            // SAFETY: state helpers used by frame cleanup do not invoke entries;
            // this typed run borrow is confined to the post-entry phase.
            let cleanup_status = unsafe { after_jit_frame_pop(run, ctx.state, &frame) };
            drop(frame);
            cleanup_status
        };
        // SAFETY: the callback state remains live and `saved_err` was initialized
        // by the successful enter call above.
        let restore_status = unsafe { rt_err_restore_activation(ctx.state, &saved_err) };
        if restore_status != ST_OK {
            restore_status
        } else if cleanup_status != ST_OK {
            cleanup_status
        } else {
            status
        }
    })
}

pub(crate) struct JitForEachState {
    pub(crate) elements: Vec<Variant>,
    pub(crate) position: usize,
}

pub(crate) struct JitFrame {
    pub(crate) program_index: usize,
    pub(crate) locals: Vec<Variant>,
    pub(crate) temps: Vec<Variant>,
    pub(crate) aliases: Vec<Option<SlotAlias>>,
    pub(crate) gosub_stack: Vec<u32>,
    pub(crate) saved_err: RtSavedErrState,
    pub(crate) current_line: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SlotAlias {
    pub(crate) frame: Option<usize>,
    pub(crate) area: u32,
    pub(crate) index: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct JitCallArgDesc {
    pub(crate) kind: i32,
    pub(crate) aux: i32,
    pub(crate) value: i64,
    pub(crate) area: i32,
    pub(crate) index: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct JitCallArgNameDesc {
    pub(crate) ptr: i64,
    pub(crate) len: i32,
    pub(crate) _pad: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct JitVariantOperandDesc {
    pub(crate) kind: i32,
    pub(crate) _pad: i32,
    pub(crate) value: i64,
    pub(crate) area: i32,
    pub(crate) index: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct JitSlotAliasDesc {
    pub(crate) area: i32,
    pub(crate) index: i32,
}

pub(crate) fn new_jit_frame(
    program: &OxProgram,
    program_index: usize,
    func: &OxFunc,
) -> Result<JitFrame, JitError> {
    let locals = func
        .locals
        .iter()
        .map(|local| {
            default_slot_value_with_array_element(program, &local.ty, local.array_element.as_ref())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let temps = func
        .temps
        .iter()
        .map(|ty| default_slot_value(program, ty))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(JitFrame {
        program_index,
        locals,
        temps,
        aliases: vec![None; func.locals.len()],
        gosub_stack: Vec::new(),
        saved_err: RtSavedErrState::default(),
        current_line: 0,
    })
}

pub(crate) fn build_loaded<'p>(program: &'p OxProgram) -> Result<LoadedProgram<'p>, JitError> {
    let globals = program
        .globals
        .iter()
        .map(|global| {
            default_slot_value_with_array_element(
                program,
                &global.ty,
                global.array_element.as_ref(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let event_routes = program
        .event_routes
        .iter()
        .map(|route| ((route.binding, route.event), route.handler))
        .collect();
    Ok(LoadedProgram {
        program,
        globals,
        class_descriptors: runtime_class_descriptors_for_program(program),
        predeclared_singletons: HashMap::new(),
        event_routes,
    })
}

pub(crate) fn default_slot_value(program: &OxProgram, ty: &OxTy) -> Result<Variant, JitError> {
    default_slot_value_with_array_element(program, ty, None)
}

pub(crate) fn default_slot_value_with_array_element(
    program: &OxProgram,
    ty: &OxTy,
    array_element: Option<&ArrayElementType>,
) -> Result<Variant, JitError> {
    match ty {
        OxTy::Bool => Ok(Variant::from_bool(false)),
        OxTy::Byte => Ok(Variant::from_u8(0)),
        OxTy::Integer => Ok(Variant::from_i16(0)),
        OxTy::Long => Ok(Variant::from_i32(0)),
        OxTy::LongLong => Ok(Variant::from_i64(0)),
        OxTy::Currency => Ok(Variant::from_currency_scaled_i64(0)),
        OxTy::Single => Ok(Variant::from_f32(0.0)),
        OxTy::Double => Ok(Variant::from_f64(0.0)),
        OxTy::Date => Ok(Variant::from_date_f64(0.0)),
        OxTy::Decimal => Ok(Variant::from_decimal96(Decimal96::default())),
        OxTy::Variant | OxTy::ProcRef => Ok(Variant::empty()),
        OxTy::Str => Ok(Variant::from_string(String::new())),
        OxTy::FixedStr(len) => Ok(Variant::from_string(" ".repeat(*len as usize))),
        OxTy::Object(_) => Ok(Variant::nothing()),
        OxTy::Record(id) => {
            let fields = program.record_layout(*id).ok_or_else(|| {
                JitError::unsupported(format!(
                    "JIT record slot {:?} has no record layout metadata",
                    id
                ))
            })?;
            let layout = vba_record_layout_for_fields(fields).map_err(|err| {
                JitError::unsupported(format!("JIT record slot layout is invalid: {err}"))
            })?;
            let record = VbaRecord::new_default(layout).map_err(|err| {
                JitError::Runtime(format!("default record allocation failed: {err}"))
            })?;
            Ok(Variant::from_vba_record(record))
        }
        OxTy::Array(element, _) => Ok(Variant::unallocated_array(array_slot_vartype_for_slot(
            element,
            array_element,
        ))),
    }
}

pub(crate) fn array_slot_vartype_for_slot(
    element: &OxTy,
    metadata: Option<&ArrayElementType>,
) -> u16 {
    match metadata {
        Some(ArrayElementType::Variant) | None => array_slot_vartype(element),
        Some(element) => safearray_vartype_for_element(element),
    }
}

pub(crate) fn array_slot_vartype(element: &OxTy) -> u16 {
    match element {
        OxTy::Bool => VT_BOOL_VALUE,
        OxTy::Byte => VT_UI1_VALUE,
        OxTy::Integer => VT_I2_VALUE,
        OxTy::Long => VT_I4_VALUE,
        OxTy::LongLong => VT_I8_VALUE,
        OxTy::Currency => VT_CY_VALUE,
        OxTy::Single => VT_R4_VALUE,
        OxTy::Double => VT_R8_VALUE,
        OxTy::Date => VT_DATE_VALUE,
        OxTy::Decimal => VT_DECIMAL_VALUE,
        OxTy::Str | OxTy::FixedStr(_) => VT_BSTR_VALUE,
        OxTy::Object(_) => VT_DISPATCH_VALUE,
        OxTy::Record(_) => VT_RECORD_VALUE,
        OxTy::Variant | OxTy::ProcRef | OxTy::Array(_, _) => VT_VARIANT_VALUE,
    }
}

pub(crate) fn err_from_exec(exec: &ExecState<'_>) -> JitFinalErr {
    JitFinalErr {
        number: exec.err_engine.err.number,
        source: exec.err_engine.err.source.clone(),
        description: exec.err_engine.err.description.clone(),
        last_dll_error: exec.err_engine.last_dll_error,
    }
}

pub(crate) fn snapshot_values(exec: &ExecState<'_>, run: &JitRun) -> Vec<Variant> {
    let entry_program_index = run.program_count.saturating_sub(1);
    let mut values = exec
        .programs
        .get(entry_program_index)
        .map(|loaded| loaded.globals.clone())
        .unwrap_or_default();
    if let Some(frame) = run.frames.last() {
        values.extend(frame.locals.iter().cloned());
    }
    values
}

pub(crate) fn current_frame_slot(run: &JitRun, area: u32, index: u32) -> Option<SlotAlias> {
    match area {
        AREA_GLOBAL => Some(SlotAlias {
            frame: Some(run.frames.len().checked_sub(1)?),
            area,
            index,
        }),
        AREA_LOCAL | AREA_TEMP => Some(SlotAlias {
            frame: Some(run.frames.len().checked_sub(1)?),
            area,
            index,
        }),
        _ => None,
    }
}

pub(crate) fn direct_call_arg_alias(run: &JitRun, area: i32, index: i32) -> Option<SlotAlias> {
    if area < 0 || index < 0 || run.frames.is_empty() {
        return None;
    }
    current_frame_slot(run, area as u32, index as u32)
}

pub(crate) fn program_image(run: &JitRun, program_index: usize) -> Option<JitProgramImage> {
    if run.programs.is_null() || program_index >= run.program_count {
        return None;
    }
    // SAFETY: program_index is bounds-checked and the table is live for the run.
    Some(unsafe { *run.programs.add(program_index) })
}

pub(crate) fn current_program_image(run: &JitRun) -> Option<(usize, JitProgramImage)> {
    let program_index = run.frames.last()?.program_index;
    Some((program_index, program_image(run, program_index)?))
}

pub(crate) fn frame_local_alias(run: &JitRun, frame: usize, index: usize) -> Option<SlotAlias> {
    run.frames.get(frame)?.aliases.get(index).copied().flatten()
}

pub(crate) fn resolve_slot_alias(run: &JitRun, mut alias: SlotAlias) -> Option<SlotAlias> {
    let max_alias_hops = run.frames.len().saturating_add(1);
    for _ in 0..=max_alias_hops {
        match alias.area {
            AREA_GLOBAL => {
                alias.frame?;
                return Some(SlotAlias {
                    frame: alias.frame,
                    area: AREA_GLOBAL,
                    index: alias.index,
                });
            }
            AREA_LOCAL => {
                let frame = alias.frame?;
                if let Some(next) = frame_local_alias(run, frame, alias.index as usize) {
                    alias = next;
                } else {
                    return Some(alias);
                }
            }
            AREA_TEMP => {
                alias.frame?;
                return Some(alias);
            }
            _ => return None,
        }
    }
    None
}

pub(crate) fn slot_mut(run: &mut JitRun, area: u32, index: u32) -> Option<&mut Variant> {
    let alias = current_frame_slot(run, area, index)?;
    slot_alias_mut(run, alias)
}

pub(crate) fn slot_ref(run: &JitRun, area: u32, index: u32) -> Option<&Variant> {
    let alias = current_frame_slot(run, area, index)?;
    slot_alias_ref(run, alias)
}

pub(crate) fn slot_alias_mut(run: &mut JitRun, alias: SlotAlias) -> Option<&mut Variant> {
    let alias = resolve_slot_alias(run, alias)?;
    let index = alias.index as usize;
    match alias.area {
        AREA_GLOBAL => {
            let program_index = run.frames.get(alias.frame?)?.program_index;
            if run.globals.is_null() || program_index >= run.global_count {
                None
            } else {
                // SAFETY: globals points at the live ExecState global vectors for the run;
                // program_index was bounds-checked above.
                let globals = unsafe { *run.globals.add(program_index) };
                if globals.is_null() {
                    None
                } else {
                    // SAFETY: selected global vector belongs to the live ExecState.
                    unsafe { (&mut *globals).get_mut(index) }
                }
            }
        }
        AREA_LOCAL => run.frames.get_mut(alias.frame?)?.locals.get_mut(index),
        AREA_TEMP => run.frames.get_mut(alias.frame?)?.temps.get_mut(index),
        _ => None,
    }
}

pub(crate) fn slot_alias_ref(run: &JitRun, alias: SlotAlias) -> Option<&Variant> {
    let alias = resolve_slot_alias(run, alias)?;
    let index = alias.index as usize;
    match alias.area {
        AREA_GLOBAL => {
            let program_index = run.frames.get(alias.frame?)?.program_index;
            if run.globals.is_null() || program_index >= run.global_count {
                None
            } else {
                // SAFETY: globals points at the live ExecState global vectors for the run;
                // program_index was bounds-checked above.
                let globals = unsafe { *run.globals.add(program_index) };
                if globals.is_null() {
                    None
                } else {
                    // SAFETY: selected global vector belongs to the live ExecState.
                    unsafe { (&*globals).get(index) }
                }
            }
        }
        AREA_LOCAL => run.frames.get(alias.frame?)?.locals.get(index),
        AREA_TEMP => run.frames.get(alias.frame?)?.temps.get(index),
        _ => None,
    }
}

pub(crate) fn param_array_aliases_for_call_arg(
    run: &JitRun,
    arg: JitCallArgDesc,
) -> Option<Vec<Option<SlotAlias>>> {
    if arg.area < 0 || arg.index < 0 {
        return None;
    }
    let alias = current_frame_slot(run, arg.area as u32, arg.index as u32)
        .and_then(|alias| resolve_slot_alias(run, alias))?;
    run.param_array_aliases.get(&alias).cloned()
}

pub(crate) fn prune_param_array_aliases_from_depth(run: &mut JitRun, depth: usize) {
    run.param_array_aliases.retain(|array, aliases| {
        array.frame.is_none_or(|frame| frame < depth)
            && aliases
                .iter()
                .flatten()
                .all(|alias| alias.frame.is_none_or(|frame| frame < depth))
    });
}

pub(crate) fn prune_as_new_slots_from_depth(run: &mut JitRun, depth: usize) {
    run.as_new_slots
        .retain(|slot, _| slot.frame.is_none_or(|frame| frame < depth));
}

pub(crate) fn prune_for_each_from_depth(run: &mut JitRun, depth: usize) {
    run.for_each
        .retain(|iter, _| iter.frame.is_none_or(|frame| frame < depth));
}

pub(crate) fn is_current_frame_temp_at_or_after(
    alias: SlotAlias,
    frame: usize,
    first_temp: usize,
) -> bool {
    alias.frame == Some(frame) && alias.area == AREA_TEMP && alias.index as usize >= first_temp
}

pub(crate) fn prune_param_array_aliases_for_cleared_temps(
    run: &mut JitRun,
    frame: usize,
    first_temp: usize,
) {
    run.param_array_aliases.retain(|array, aliases| {
        !is_current_frame_temp_at_or_after(*array, frame, first_temp)
            && aliases
                .iter()
                .flatten()
                .all(|alias| !is_current_frame_temp_at_or_after(*alias, frame, first_temp))
    });
}

pub(crate) fn prune_as_new_slots_for_cleared_temps(
    run: &mut JitRun,
    frame: usize,
    first_temp: usize,
) {
    run.as_new_slots
        .retain(|slot, _| !is_current_frame_temp_at_or_after(*slot, frame, first_temp));
}

pub(crate) fn prune_for_each_for_cleared_temps(run: &mut JitRun, frame: usize, first_temp: usize) {
    run.for_each
        .retain(|iter, _| !is_current_frame_temp_at_or_after(*iter, frame, first_temp));
}

pub(crate) fn clear_current_statement_temps(run: &mut JitRun, first_temp: usize) {
    let Some(frame_index) = run.frames.len().checked_sub(1) else {
        return;
    };
    if let Some(frame) = run.frames.get_mut(frame_index) {
        for slot in frame.temps.iter_mut().skip(first_temp) {
            *slot = Variant::empty();
        }
    }
    prune_for_each_for_cleared_temps(run, frame_index, first_temp);
    prune_as_new_slots_for_cleared_temps(run, frame_index, first_temp);
    prune_param_array_aliases_for_cleared_temps(run, frame_index, first_temp);
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn mirror_param_array_element_write(
    state: *mut RawExecState,
    run: &mut JitRun,
    array_alias: SlotAlias,
    flat: usize,
    value: &Variant,
) -> Result<(), i32> {
    let Some(aliases) = run.param_array_aliases.get(&array_alias).cloned() else {
        return Ok(());
    };
    let Some(Some(target)) = aliases.get(flat).copied() else {
        return Ok(());
    };
    let Some(slot) = slot_alias_mut(run, target) else {
        return Err(ST_FAULT);
    };
    *slot = value.clone();

    for (index, alias) in aliases.iter().enumerate() {
        if index == flat || *alias != Some(target) {
            continue;
        }
        let Some(array) = slot_alias_mut(run, array_alias) else {
            return Err(ST_FAULT);
        };
        array
            .set_safearray_element(index, value)
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            .map_err(|_| unsafe { rt_raise_type_mismatch(state) })?;
    }
    Ok(())
}

pub(crate) fn status_guard(work: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(work)).unwrap_or(ST_FAULT)
}

// SAFETY CONTRACT: a UTF-8 operand's integer-carried pointer must identify `index`
// readable bytes for this call (unless `index == 0`). Other operand kinds carry no
// dereferenceable integer. The run and all referenced slots remain live for the call.
pub(crate) unsafe fn variant_operand_value(
    run: &JitRun,
    operand: JitVariantOperandDesc,
) -> Option<Variant> {
    match operand.kind {
        JIT_VARIANT_OPERAND_PLACE => {
            if operand.area < 0 || operand.index < 0 {
                return None;
            }
            slot_ref(run, operand.area as u32, operand.index as u32).cloned()
        }
        JIT_VARIANT_OPERAND_EMPTY => Some(Variant::empty()),
        JIT_VARIANT_OPERAND_NULL => Some(Variant::null()),
        JIT_VARIANT_OPERAND_BOOL => Some(Variant::from_bool(operand.value != 0)),
        JIT_VARIANT_OPERAND_I16 => i16::try_from(operand.value).ok().map(Variant::from_i16),
        JIT_VARIANT_OPERAND_I32 => i32::try_from(operand.value).ok().map(Variant::from_i32),
        JIT_VARIANT_OPERAND_I64 => Some(Variant::from_i64(operand.value)),
        JIT_VARIANT_OPERAND_F32 => u32::try_from(operand.value)
            .ok()
            .map(|bits| Variant::from_f32(f32::from_bits(bits))),
        JIT_VARIANT_OPERAND_F64 => Some(Variant::from_f64(f64::from_bits(operand.value as u64))),
        JIT_VARIANT_OPERAND_CURRENCY => Some(Variant::from_currency_scaled_i64(operand.value)),
        JIT_VARIANT_OPERAND_DATE => {
            Some(Variant::from_date_f64(f64::from_bits(operand.value as u64)))
        }
        JIT_VARIANT_OPERAND_NOTHING => Some(Variant::nothing()),
        JIT_VARIANT_OPERAND_STR_UTF8 => {
            let len = usize::try_from(operand.index).ok()?;
            let ptr = usize::try_from(operand.value).ok()? as *const u8;
            let bytes = if len == 0 {
                &[]
            } else {
                if ptr.is_null() {
                    return None;
                }
                // SAFETY: upheld by this private decoder's descriptor contract.
                unsafe { std::slice::from_raw_parts(ptr, len) }
            };
            let text = std::str::from_utf8(bytes).ok()?;
            Some(Variant::from_string(text.to_owned()))
        }
        _ => None,
    }
}

macro_rules! variant_operand_value_from_compiled_desc {
    ($run:expr, $operand:expr) => {{
        // SAFETY: this macro is confined to generated-entry helpers whose raw
        // boundary guarantees any UTF-8 descriptor storage for the synchronous call.
        unsafe { variant_operand_value($run, $operand) }
    }};
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn variant_operand_value_with_as_new(
    run: *mut JitRun,
    state: *mut RawExecState,
    operand: JitVariantOperandDesc,
) -> Result<Variant, i32> {
    if run.is_null() {
        return Err(ST_FAULT);
    }
    if operand.kind != JIT_VARIANT_OPERAND_PLACE {
        // SAFETY: null was rejected and this borrow cannot reach a callback.
        let run = unsafe { &*run };
        return variant_operand_value_from_compiled_desc!(run, operand).ok_or(ST_FAULT);
    }
    if operand.area < 0 || operand.index < 0 {
        return Err(ST_FAULT);
    }
    let (alias, value, binding) = {
        // SAFETY: null was rejected and the read-only borrow ends before activation.
        let run = unsafe { &*run };
        let Some(alias) = current_frame_slot(run, operand.area as u32, operand.index as u32)
            .and_then(|alias| resolve_slot_alias(run, alias))
        else {
            return Err(ST_FAULT);
        };
        let Some(value) = slot_alias_ref(run, alias).cloned() else {
            return Err(ST_FAULT);
        };
        (alias, value, run.as_new_slots.get(&alias).cloned())
    };
    let Some(binding) = binding else {
        return Ok(value);
    };
    if !jit_is_nothing(&value) {
        return Ok(value);
    }
    // SAFETY: the current compiled-run boundary owns the live unique state handle;
    // typed references and owned values remain live and nonaliasing for this call.
    let object = unsafe { instantiate_as_new_for_jit(run, state, 0, binding) }?;
    {
        // SAFETY: activation returned and no typed run borrow is live.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_alias_mut(run, alias) else {
            return Err(ST_FAULT);
        };
        *slot = object.clone();
    }
    Ok(object)
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn instantiate_as_new_for_jit(
    run: *mut JitRun,
    state: *mut RawExecState,
    program_index: i32,
    binding: OxAsNew,
) -> Result<Variant, i32> {
    if run.is_null() {
        return Err(ST_FAULT);
    }
    match binding {
        OxAsNew::ProjectClass { class } => {
            let mut value = Variant::empty();
            // SAFETY: the enclosing JIT boundary validated the live state; the Variant output is initialized, uniquely borrowed storage.
            let status = unsafe {
                rt_project_new_object(state, program_index as usize, class.0, &mut value)
            };
            if status == ST_OK {
                Ok(value)
            } else {
                Err(status)
            }
        }
        OxAsNew::ComClass { prog_id } => {
            let Some(program_index) = prog_id
                .strip_prefix("__jit_vba_collection:")
                .and_then(|raw| raw.parse::<i32>().ok())
            else {
                return Err(ST_FAULT);
            };
            // SAFETY: collection construction cannot invoke the host or an entry;
            // this short mutable borrow is confined to local ID allocation.
            new_collection_variant_for_jit(unsafe { &mut *run }, program_index)
        }
        OxAsNew::ExternClass { .. } => Err(ST_FAULT),
    }
}

pub(crate) fn new_collection_variant_for_jit(
    run: &mut JitRun,
    program_index: i32,
) -> Result<Variant, i32> {
    if program_index < 0 {
        return Err(ST_FAULT);
    }
    let instance_id = run.next_collection_instance_id;
    run.next_collection_instance_id = run
        .next_collection_instance_id
        .checked_add(1)
        .unwrap_or(i32::MIN + 1);
    let object = ObjectRef::from_project_instance(
        instance_id,
        VBA_COLLECTION_ROUTE_KEY,
        program_index,
        false,
        &VBA_COLLECTION_DESCRIPTOR,
    );
    Ok(Variant::from_object_ref(object))
}

pub(crate) fn class_field_as_new_binding_for_jit(
    run: &JitRun,
    object: &oxvba_runtime::object_ref::ObjectRef,
    field: i32,
) -> Option<OxAsNew> {
    if !object.is_project_instance() {
        return None;
    }
    let image = program_image(run, object.bundle_id() as usize)?;
    if image.program.is_null() {
        return None;
    }
    // SAFETY: installed from the owning CompiledImage for this run.
    let program = unsafe { &*image.program };
    program
        .classes
        .get(object.route_key() as usize)?
        .as_new_fields
        .iter()
        .find(|candidate| candidate.field == field)
        .map(|candidate| candidate.binding.clone())
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn project_field_get_with_as_new_for_jit(
    run: *mut JitRun,
    state: *mut RawExecState,
    object: &oxvba_runtime::object_ref::ObjectRef,
    field: i32,
) -> Result<Variant, i32> {
    if run.is_null() {
        return Err(ST_FAULT);
    }
    let value = object
        .project_field_get(field)
        .unwrap_or_else(Variant::empty);
    // SAFETY: null was rejected and this read-only borrow ends before activation.
    let binding = class_field_as_new_binding_for_jit(unsafe { &*run }, object, field);
    let Some(binding) = binding else {
        return Ok(value);
    };
    if !jit_is_nothing(&value) {
        return Ok(value);
    }
    // SAFETY: the current compiled-run boundary owns the live unique state handle;
    // typed references and owned values remain live and nonaliasing for this call.
    let object_value =
        unsafe { instantiate_as_new_for_jit(run, state, object.bundle_id(), binding) }?;
    if object.project_field_set(field, object_value.clone()) {
        Ok(object_value)
    } else {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        Err(unsafe { rt_raise_runtime_error_number(state, 438) })
    }
}

pub(crate) fn scalar_arg_variant(ty: &OxTy, value: i64) -> Option<Variant> {
    match ty {
        OxTy::Long => i32::try_from(value).ok().map(Variant::from_i32),
        OxTy::LongLong => Some(Variant::from_i64(value)),
        OxTy::Currency => Some(Variant::from_currency_scaled_i64(value)),
        OxTy::Single => u32::try_from(value)
            .ok()
            .map(|bits| Variant::from_f32(f32::from_bits(bits))),
        OxTy::Double => Some(Variant::from_f64(f64::from_bits(value as u64))),
        OxTy::Date => Some(Variant::from_date_f64(f64::from_bits(value as u64))),
        OxTy::Byte => u8::try_from(value).ok().map(Variant::from_u8),
        OxTy::Integer => i16::try_from(value).ok().map(Variant::from_i16),
        OxTy::Bool => Some(Variant::from_bool(value != 0)),
        _ => None,
    }
}

pub(crate) fn call_arg_variant_value(run: &JitRun, arg: JitCallArgDesc) -> Option<Variant> {
    match arg.kind {
        JIT_CALL_ARG_BYVAL_VARIANT | JIT_CALL_ARG_BYREF_COPY => {
            variant_operand_value_from_compiled_desc!(
                run,
                JitVariantOperandDesc {
                    kind: arg.aux,
                    _pad: 0,
                    value: arg.value,
                    area: arg.area,
                    index: arg.index,
                }
            )
        }
        JIT_CALL_ARG_BYREF_ALIAS => {
            if arg.area < 0 || arg.index < 0 {
                None
            } else {
                slot_ref(run, arg.area as u32, arg.index as u32).cloned()
            }
        }
        JIT_CALL_ARG_OMITTED => Some(Variant::from_error_code(MISSING_ARG)),
        _ => None,
    }
}

pub(crate) fn variant_long_i32_value(value: &Variant) -> Option<i32> {
    value
        .as_i32()
        .or_else(|| value.as_i16().map(i32::from))
        .or_else(|| value.as_u8().map(i32::from))
        .or_else(|| value.as_bool().map(|value| if value { -1 } else { 0 }))
}

pub(crate) fn call_arg_long_i32_value(run: &JitRun, arg: JitCallArgDesc) -> Option<i32> {
    match arg.kind {
        JIT_CALL_ARG_BYVAL_SCALAR => i32::try_from(arg.value).ok(),
        JIT_CALL_ARG_BYVAL_VARIANT => {
            call_arg_variant_value(run, arg).and_then(|value| variant_long_i32_value(&value))
        }
        _ => None,
    }
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn coerce_string_call_arg(
    state: *mut RawExecState,
    value: &Variant,
) -> Result<Variant, i32> {
    let mut coerced = Variant::empty();
    // SAFETY: the enclosing JIT boundary validated the live state and all Variant input/output pointers are initialized, live, and nonaliasing.
    let status = unsafe { rt_coerce_string_v(state, value, &mut coerced) };
    if status == ST_OK {
        Ok(coerced)
    } else {
        Err(status)
    }
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn coerce_fixed_string_call_arg(
    state: *mut RawExecState,
    len: u32,
    value: &Variant,
) -> Result<Variant, i32> {
    let mut coerced = Variant::empty();
    // SAFETY: the enclosing JIT boundary validated the live state and all Variant input/output pointers are initialized, live, and nonaliasing.
    let status = unsafe { rt_coerce_fixed_string_v(state, len, value, &mut coerced) };
    if status == ST_OK {
        Ok(coerced)
    } else {
        Err(status)
    }
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn coerce_call_arg_for_param(
    state: *mut RawExecState,
    param_ty: &OxTy,
    value: &Variant,
) -> Result<Variant, i32> {
    match param_ty {
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        OxTy::Str => unsafe { coerce_string_call_arg(state, value) },
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        OxTy::FixedStr(len) => unsafe { coerce_fixed_string_call_arg(state, *len, value) },
        _ => Ok(value.clone()),
    }
}

pub(crate) fn hidden_me_receiver_param_count(func: &OxFunc) -> usize {
    usize::from(
        func.param_count > 0
            && func.locals.first().is_some_and(|local| {
                local.param.is_some() && local.name.eq_ignore_ascii_case("Me")
            }),
    )
}

pub(crate) fn omitted_call_arg_desc() -> JitCallArgDesc {
    JitCallArgDesc {
        kind: JIT_CALL_ARG_OMITTED,
        aux: 0,
        value: 0,
        area: 0,
        index: 0,
    }
}

// SAFETY CONTRACT: for nonnegative lengths, the integer-carried pointer identifies
// `len` readable UTF-8 bytes for this call (unless `len == 0`).
pub(crate) unsafe fn call_arg_name(name: JitCallArgNameDesc) -> Result<Option<String>, i32> {
    if name.len < 0 {
        return Ok(None);
    }
    let len = usize::try_from(name.len).map_err(|_| ST_FAULT)?;
    let ptr = usize::try_from(name.ptr).map_err(|_| ST_FAULT)? as *const u8;
    let bytes = if len == 0 {
        &[]
    } else {
        if ptr.is_null() {
            return Err(ST_FAULT);
        }
        // SAFETY: upheld by this private decoder's descriptor contract.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    let name = std::str::from_utf8(bytes).map_err(|_| ST_FAULT)?;
    Ok(Some(name.to_ascii_lowercase()))
}

// SAFETY CONTRACT: for nonnegative lengths, the integer-carried pointer identifies
// `len` readable UTF-8 bytes for this call (unless `len == 0`).
pub(crate) unsafe fn call_arg_name_preserved(
    name: JitCallArgNameDesc,
) -> Result<Option<String>, i32> {
    if name.len < 0 {
        return Ok(None);
    }
    let len = usize::try_from(name.len).map_err(|_| ST_FAULT)?;
    let ptr = usize::try_from(name.ptr).map_err(|_| ST_FAULT)? as *const u8;
    let bytes = if len == 0 {
        &[]
    } else {
        if ptr.is_null() {
            return Err(ST_FAULT);
        }
        // SAFETY: upheld by this private decoder's descriptor contract.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    let name = std::str::from_utf8(bytes).map_err(|_| ST_FAULT)?;
    Ok(Some(name.to_string()))
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn order_project_member_call_args(
    state: *mut RawExecState,
    func: &OxFunc,
    args: &[JitCallArgDesc],
    names: &[JitCallArgNameDesc],
) -> Result<Vec<JitCallArgDesc>, i32> {
    if args.len() != names.len() {
        return Err(ST_FAULT);
    }
    let hidden = hidden_me_receiver_param_count(func);
    if hidden != 1 {
        return Err(ST_FAULT);
    }
    let param_names: Vec<String> = func
        .locals
        .iter()
        .take(func.param_count)
        .skip(hidden)
        .map(|local| local.name.to_ascii_lowercase())
        .collect();
    let mut ordered: Vec<Option<JitCallArgDesc>> = Vec::new();
    let mut next_positional = 0usize;
    for (arg, name) in args.iter().copied().zip(names.iter().copied()) {
        // SAFETY: names are compiled descriptors backed by the live OxProgram.
        if let Some(name) = unsafe { call_arg_name(name) }? {
            let Some(index) = param_names.iter().position(|param| param == &name) else {
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                return Err(unsafe { rt_raise_runtime_error_number(state, 448) });
            };
            if ordered.len() <= index {
                ordered.resize_with(index + 1, || None);
            }
            if ordered[index].is_some() {
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                return Err(unsafe { rt_raise_runtime_error_number(state, 448) });
            }
            ordered[index] = Some(arg);
        } else {
            while ordered.get(next_positional).is_some_and(Option::is_some) {
                next_positional += 1;
            }
            if ordered.len() <= next_positional {
                ordered.resize_with(next_positional + 1, || None);
            }
            ordered[next_positional] = Some(arg);
            next_positional += 1;
        }
    }
    Ok(ordered
        .into_iter()
        .map(|arg| arg.unwrap_or_else(omitted_call_arg_desc))
        .collect())
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn seed_jit_member_frame_args(
    state: *mut RawExecState,
    run: &mut JitRun,
    func: &OxFunc,
    frame: &mut JitFrame,
    caller_frame: usize,
    args: &[JitCallArgDesc],
    pending_param_array_aliases: &mut Vec<(usize, Vec<Option<SlotAlias>>)>,
) -> i32 {
    let hidden = hidden_me_receiver_param_count(func);
    if hidden != 1 {
        return ST_FAULT;
    }
    for (arg_index, arg) in args.iter().copied().enumerate() {
        let param_index = hidden + arg_index;
        let Some(param) = func.locals.get(param_index) else {
            continue;
        };
        let Some(param_info) = param.param.as_ref() else {
            return ST_FAULT;
        };
        if param_info.variadic {
            if !is_m4_4_supported_paramarray_param(&param.ty, *param_info)
                || arg.kind != JIT_CALL_ARG_BYVAL_VARIANT
            {
                return ST_FAULT;
            }
        } else if !is_jit_static_call_ty(&param.ty) {
            return ST_FAULT;
        }
        match arg.kind {
            JIT_CALL_ARG_BYVAL_SCALAR
                if !param_info.by_ref
                    && matches!(classify_jit_ty(&param.ty), JitTypeSupport::FastScalar) =>
            {
                let Some(value) = scalar_arg_variant(&param.ty, arg.value) else {
                    return ST_FAULT;
                };
                frame.locals[param_index] = value;
            }
            JIT_CALL_ARG_BYVAL_VARIANT if !param_info.by_ref && matches!(param.ty, OxTy::Long) => {
                let Some(value) = call_arg_long_i32_value(run, arg) else {
                    return ST_FAULT;
                };
                frame.locals[param_index] = Variant::from_i32(value);
            }
            JIT_CALL_ARG_BYVAL_VARIANT
                if !param_info.by_ref
                    && (is_jit_variant_carrier_ty(&param.ty) || param_info.variadic) =>
            {
                let param_array_aliases = if param_info.variadic {
                    param_array_aliases_for_call_arg(run, arg)
                } else {
                    None
                };
                let Some(value) = call_arg_variant_value(run, arg) else {
                    return ST_FAULT;
                };
                if param_info.variadic && value.safearray_bounds_len().is_none() {
                    return ST_FAULT;
                }
                frame.locals[param_index] =
                    // SAFETY: the current compiled-run boundary owns the live unique state handle;
                    // typed references and owned values remain live and nonaliasing for this call.
                    match unsafe { coerce_call_arg_for_param(state, &param.ty, &value) } {
                        Ok(value) => value,
                        Err(status) => return status,
                    };
                if let Some(aliases) = param_array_aliases {
                    pending_param_array_aliases.push((param_index, aliases));
                }
            }
            JIT_CALL_ARG_OMITTED if !param_info.by_ref && matches!(param.ty, OxTy::Variant) => {
                let Some(value) = call_arg_variant_value(run, arg) else {
                    return ST_FAULT;
                };
                frame.locals[param_index] = value;
            }
            JIT_CALL_ARG_BYREF_COPY if param_info.by_ref => {
                if is_jit_variant_carrier_ty(&param.ty) {
                    let Some(value) = call_arg_variant_value(run, arg) else {
                        return ST_FAULT;
                    };
                    frame.locals[param_index] =
                        // SAFETY: the current compiled-run boundary owns the live unique state handle;
                        // typed references and owned values remain live and nonaliasing for this call.
                        match unsafe { coerce_call_arg_for_param(state, &param.ty, &value) } {
                            Ok(value) => value,
                            Err(status) => return status,
                        };
                } else {
                    let Some(value) = scalar_arg_variant(&param.ty, arg.value) else {
                        return ST_FAULT;
                    };
                    frame.locals[param_index] = value;
                }
            }
            JIT_CALL_ARG_BYREF_ALIAS if param_info.by_ref => {
                if arg.area < 0 || arg.index < 0 {
                    return ST_FAULT;
                }
                let frame_index = match arg.area as u32 {
                    AREA_GLOBAL | AREA_LOCAL | AREA_TEMP => Some(caller_frame),
                    _ => return ST_FAULT,
                };
                let alias = SlotAlias {
                    frame: frame_index,
                    area: arg.area as u32,
                    index: arg.index as u32,
                };
                if slot_alias_ref(run, alias).is_none() {
                    return ST_FAULT;
                }
                frame.aliases[param_index] = Some(alias);
            }
            _ => return ST_FAULT,
        }
    }
    ST_OK
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn unknown_proc_ref_arg_shape(
    state: *mut RawExecState,
    run: &JitRun,
    func: &OxFunc,
    args: &[JitCallArgDesc],
) -> Result<UnknownProcRefArgShape, i32> {
    if args.len() != func.param_count {
        return Err(ST_FAULT);
    }
    if args.is_empty() {
        return Ok(UnknownProcRefArgShape::LongOnly);
    }

    let long_only = args.iter().copied().enumerate().all(|(index, arg)| {
        let Some(param) = func.locals.get(index) else {
            return false;
        };
        let Some(param_info) = param.param.as_ref() else {
            return false;
        };
        matches!(param.ty, OxTy::Long)
            && !param_info.variadic
            && if param_info.by_ref {
                arg.kind == JIT_CALL_ARG_BYREF_ALIAS
            } else {
                call_arg_long_i32_value(run, arg).is_some()
            }
    });
    if long_only {
        return Ok(UnknownProcRefArgShape::LongOnly);
    }

    let mut string_byval_values = Vec::with_capacity(args.len());
    let string_byval_only = args.iter().copied().enumerate().all(|(index, arg)| {
        let Some(param) = func.locals.get(index) else {
            return false;
        };
        let Some(param_info) = param.param.as_ref() else {
            return false;
        };
        if !matches!(param.ty, OxTy::Str)
            || param_info.by_ref
            || param_info.variadic
            || arg.kind != JIT_CALL_ARG_BYVAL_VARIANT
        {
            return false;
        }
        let Some(value) = call_arg_variant_value(run, arg) else {
            return false;
        };
        string_byval_values.push(value);
        true
    });
    if string_byval_only {
        for value in &string_byval_values {
            // SAFETY: the current compiled-run boundary owns the live unique state handle;
            // typed references and owned values remain live and nonaliasing for this call.
            unsafe { coerce_string_call_arg(state, value) }?;
        }
        Ok(UnknownProcRefArgShape::StringByValOnly)
    } else {
        let mut string_candidate_values = Vec::with_capacity(args.len());
        let string_candidate = args.iter().copied().enumerate().all(|(index, arg)| {
            let Some(param) = func.locals.get(index) else {
                return false;
            };
            let Some(param_info) = param.param.as_ref() else {
                return false;
            };
            if !matches!(param.ty, OxTy::Str) || param_info.variadic {
                return false;
            }
            if param_info.by_ref {
                if arg.kind != JIT_CALL_ARG_BYREF_ALIAS || arg.area < 0 || arg.index < 0 {
                    return false;
                }
                slot_ref(run, arg.area as u32, arg.index as u32).is_some()
            } else {
                if arg.kind != JIT_CALL_ARG_BYVAL_VARIANT {
                    return false;
                }
                let Some(value) = call_arg_variant_value(run, arg) else {
                    return false;
                };
                string_candidate_values.push(value);
                true
            }
        });
        if string_candidate {
            for value in &string_candidate_values {
                // SAFETY: the current compiled-run boundary owns the live unique state handle;
                // typed references and owned values remain live and nonaliasing for this call.
                unsafe { coerce_string_call_arg(state, value) }?;
            }
            Ok(UnknownProcRefArgShape::StringCandidate)
        } else {
            Err(ST_FAULT)
        }
    }
}

pub(crate) fn variant_matches_unbox_target(value: &Variant, target: i32) -> bool {
    target == -1 || target == value.vtype() as i32
}

pub(crate) fn jit_is_nothing(value: &Variant) -> bool {
    match value.vtype() {
        VarType::Object => {
            value
                .as_object_ref()
                .map(|object| object.raw())
                .unwrap_or(0)
                == 0
        }
        VarType::Empty | VarType::Null => true,
        _ => value.as_i16() == Some(0) || value.as_i32() == Some(0),
    }
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn object_identity_for_is(
    state: *mut RawExecState,
    value: &Variant,
) -> Result<i32, i32> {
    if !matches!(value.vtype(), VarType::Object) {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return Err(unsafe { rt_raise_runtime_error_number(state, 424) });
    }
    Ok(value
        .as_object_ref()
        .map(|object| object.raw())
        .unwrap_or(0))
}

pub(crate) fn call_return_variant(ty: &OxTy, value: &Variant) -> Option<Variant> {
    match ty {
        OxTy::Variant => Some(value.clone()),
        OxTy::Long => value
            .as_i32()
            .or_else(|| value.as_i16().map(i32::from))
            .or_else(|| value.as_u8().map(i32::from))
            .map(Variant::from_i32),
        OxTy::LongLong => value
            .as_i64()
            .or_else(|| value.as_i32().map(i64::from))
            .or_else(|| value.as_i16().map(i64::from))
            .or_else(|| value.as_u8().map(i64::from))
            .map(Variant::from_i64),
        OxTy::Currency => value
            .as_currency_scaled_i64()
            .map(Variant::from_currency_scaled_i64),
        OxTy::Single => value.as_f32().map(Variant::from_f32),
        OxTy::Double => value.as_f64().map(Variant::from_f64),
        OxTy::Date => value.as_date_f64().map(Variant::from_date_f64),
        OxTy::Byte => value.as_u8().map(Variant::from_u8),
        OxTy::Integer => value.as_i16().map(Variant::from_i16),
        OxTy::Bool => value.as_bool().map(Variant::from_bool),
        OxTy::Str | OxTy::FixedStr(_) => value.as_bstr().map(Variant::from_string),
        OxTy::Decimal => value.as_decimal96().map(Variant::from_decimal96),
        OxTy::Object(_) if matches!(value.vtype(), VarType::Object) => Some(value.clone()),
        OxTy::Record(_) if matches!(value.vtype(), VarType::Record) => Some(value.clone()),
        OxTy::Array(_, _) if matches!(value.vtype(), VarType::ArrayVariant) => Some(value.clone()),
        OxTy::ProcRef => value.as_proc_ref().map(Variant::from_proc_ref),
        _ => None,
    }
}
