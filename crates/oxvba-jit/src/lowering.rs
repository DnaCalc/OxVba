//! Cranelift procedure lowering and compile-time call planning.

use super::*;

pub(crate) struct Compiler<'p> {
    pub(crate) module: JITModule,
    pub(crate) imports: Imports,
    pub(crate) programs: Vec<&'p OxProgram>,
    pub(crate) clif_ids: Vec<Vec<ClifFuncId>>,
}

impl<'p> Compiler<'p> {
    pub(crate) fn compile_image(programs: &[&'p OxProgram]) -> Result<CompiledImage<'p>, JitError> {
        if programs.is_empty() {
            return Err(JitError::unsupported(
                "JIT compile_image requires at least one OxProgram",
            ));
        }
        for program in programs {
            validate_program_shape(program)?;
        }

        let mut builder = jit_builder()?;
        register_symbols(&mut builder);
        let mut module = JITModule::new(builder);
        let imports = declare_imports(&mut module)?;
        let clif_ids = programs
            .iter()
            .enumerate()
            .map(|(program_index, program)| {
                declare_program_functions(&mut module, program_index, program)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut compiler = Self {
            module,
            imports,
            programs: programs.to_vec(),
            clif_ids,
        };
        compiler.define_program_functions()?;
        compiler.module.finalize_definitions().map_err(module_err)?;

        let functions = compiler
            .clif_ids
            .iter()
            .map(|program_ids| {
                program_ids
                    .iter()
                    .map(|id| {
                        let ptr = compiler.module.get_finalized_function(*id);
                        // SAFETY: every declared local function uses `entry_signature`, which is
                        // exactly `unsafe extern "C" fn(*mut JitRun, *mut RawExecState) -> i32`.
                        unsafe { std::mem::transmute::<*const u8, JitEntryFn>(ptr) }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        Ok(CompiledImage {
            module: compiler.module,
            programs: programs.to_vec(),
            functions,
        })
    }

    fn define_program_functions(&mut self) -> Result<(), JitError> {
        let mut ctx = self.module.make_context();
        let mut func_ctx = FunctionBuilderContext::new();
        for program_index in 0..self.programs.len() {
            for index in 0..self.programs[program_index].funcs.len() {
                self.define_function(program_index, index, &mut ctx, &mut func_ctx)?;
                self.module.clear_context(&mut ctx);
            }
        }
        Ok(())
    }

    fn define_function(
        &mut self,
        program_index: usize,
        index: usize,
        ctx: &mut cranelift_codegen::Context,
        func_ctx: &mut FunctionBuilderContext,
    ) -> Result<(), JitError> {
        let program = self.programs[program_index];
        let func = &program.funcs[index];
        validate_func_shape(func)?;
        ctx.func.signature = entry_signature(&mut self.module);
        ctx.func.name = UserFuncName::user(
            program_index as u32,
            self.clif_ids[program_index][index].as_u32(),
        );

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

            let mut lower = LowerFunc {
                programs: &self.programs,
                program_index,
                program,
                func,
                imports: self.imports,
                clif_ids: &self.clif_ids[program_index],
                blocks,
                static_proc_refs: collect_static_proc_refs(program, func),
                has_label_error_handler: func_has_label_error_handler(func),
                run,
                state,
                current_fault_target: None,
            };
            lower.define_blocks(&mut builder, &mut self.module)?;
            builder.seal_all_blocks();
            builder.finalize();
        }

        self.module
            .define_function(self.clif_ids[program_index][index], ctx)
            .map_err(module_err)
    }
}

pub(crate) struct LowerFunc<'a> {
    pub(crate) programs: &'a [&'a OxProgram],
    pub(crate) program_index: usize,
    pub(crate) program: &'a OxProgram,
    pub(crate) func: &'a OxFunc,
    pub(crate) imports: Imports,
    pub(crate) clif_ids: &'a [ClifFuncId],
    pub(crate) blocks: HashMap<BlockId, ir::Block>,
    pub(crate) static_proc_refs: HashMap<OxPlace, ProcRefStaticTarget>,
    pub(crate) has_label_error_handler: bool,
    pub(crate) run: Value,
    pub(crate) state: Value,
    pub(crate) current_fault_target: Option<ir::Block>,
}

#[derive(Clone, Copy)]
pub(crate) struct LoweredCallArg {
    pub(crate) kind: Value,
    pub(crate) aux: Value,
    pub(crate) value: Value,
    pub(crate) area: Value,
    pub(crate) index: Value,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnknownProcRefArgShape {
    LongOnly,
    StringByValOnly,
    VariantByValOnly,
    StringByValCandidate,
    StringCandidate,
}

#[derive(Clone, Copy)]
pub(crate) struct LoweredVariantOperand {
    pub(crate) kind: Value,
    pub(crate) value: Value,
    pub(crate) area: Value,
    pub(crate) index: Value,
}

pub(crate) struct BinaryVariantOperands<'a> {
    pub(crate) lhs: &'a OxOperand,
    pub(crate) rhs: &'a OxOperand,
}

pub(crate) struct ProjectClassTarget {
    pub(crate) program_index: usize,
    pub(crate) class_index: usize,
}

pub(crate) struct ProjectMemberCallInputs<'a> {
    pub(crate) recv: &'a OxOperand,
    pub(crate) name: &'a str,
    pub(crate) default_member: bool,
    pub(crate) invoke_kind: TypeLibMemberInvokeKind,
    pub(crate) args: &'a [OxCallArg],
}

pub(crate) struct CallByNameInputs<'a> {
    pub(crate) object: &'a OxOperand,
    pub(crate) name: &'a OxOperand,
    pub(crate) calltype: &'a OxOperand,
    pub(crate) args: &'a [OxCallArg],
}

pub(crate) struct ArrayRedimInputs<'a> {
    pub(crate) upper_bounds: &'a [OxOperand],
    pub(crate) lower_bounds: &'a [OxOperand],
    pub(crate) element: &'a ArrayElementType,
    pub(crate) preserve: bool,
    pub(crate) fixed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcRefStaticTarget {
    Unique(FuncId),
    SameSignature(FuncId),
    Unknown,
}

impl ProcRefStaticTarget {
    fn signature_proc(self) -> Option<FuncId> {
        match self {
            Self::Unique(proc) | Self::SameSignature(proc) => Some(proc),
            Self::Unknown => None,
        }
    }

    fn expected_marker(self) -> Result<i32, JitError> {
        match self {
            Self::Unique(proc) => i32::try_from(proc.0).map_err(|_| {
                JitError::unsupported("M4-4 ProcRef target index does not fit helper marker")
            }),
            Self::SameSignature(proc) => {
                let marker = i32::try_from(proc.0)
                    .ok()
                    .and_then(|value| value.checked_add(2))
                    .and_then(|value| value.checked_neg())
                    .ok_or_else(|| {
                        JitError::unsupported(
                            "M4-4 ProcRef signature target index does not fit helper marker",
                        )
                    })?;
                Ok(marker)
            }
            Self::Unknown => Ok(-1),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum I64CompareLane {
    Plain,
    Currency,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FloatCompareLane {
    Single,
    Double,
    Date,
}

impl<'a> LowerFunc<'a> {
    fn define_blocks(
        &mut self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
    ) -> Result<(), JitError> {
        for block in &self.func.blocks {
            self.current_fault_target = block
                .fault_target
                .map(|target| self.clif_block(target))
                .transpose()?;
            let clif_block = self.clif_block(block.id)?;
            builder.switch_to_block(clif_block);
            for inst in &block.instrs {
                self.lower_inst(builder, module, inst)?;
            }
            self.lower_terminator(builder, module, &block.terminator)?;
        }
        self.current_fault_target = None;
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
        if let Some(message) = unsupported_project_object_inst_message(inst) {
            return Err(JitError::unsupported(message));
        }
        match inst {
            OxInst::StmtBoundary {
                clear_temps_from, ..
            } => self.emit_stmt_boundary(builder, module, *clear_temps_from),
            OxInst::SetLineNumber { line } => self.emit_set_line_number(builder, module, *line),
            OxInst::DrainTerminations => self.emit_drain_terminations(builder, module),
            OxInst::AddRef { object } => self.emit_add_ref(builder, module, object),
            OxInst::Release { object, .. } => self.emit_release(builder, module, object),
            OxInst::Assign { dst, value } => match place_ty(self.program, self.func, *dst)? {
                OxTy::Long => {
                    let value = self.lower_operand_i32(builder, module, value)?;
                    self.emit_store_i32(builder, module, *dst, value)
                }
                OxTy::LongLong => {
                    let value = self.lower_operand_i64(builder, module, value)?;
                    self.emit_store_i64(builder, module, *dst, value)
                }
                OxTy::Currency => {
                    let value = self.lower_operand_currency_i64(builder, module, value)?;
                    self.emit_store_currency_i64(builder, module, *dst, value)
                }
                OxTy::Double => {
                    let value = self.lower_operand_f64(builder, module, value)?;
                    self.emit_store_f64(builder, module, *dst, value)
                }
                OxTy::Date => {
                    let value = self.lower_operand_date_f64(builder, module, value)?;
                    self.emit_store_date_f64(builder, module, *dst, value)
                }
                OxTy::Single => {
                    let value = self.lower_operand_f32(builder, module, value)?;
                    self.emit_store_f32(builder, module, *dst, value)
                }
                OxTy::Byte => {
                    let value = self.lower_operand_u8_i32(builder, module, value)?;
                    self.emit_store_u8(builder, module, *dst, value)
                }
                OxTy::Integer => {
                    let value = self.lower_operand_i16_i32(builder, module, value)?;
                    self.emit_store_i16(builder, module, *dst, value)
                }
                OxTy::Bool => {
                    let value = self.lower_operand_bool_i32(builder, module, value)?;
                    self.emit_store_bool(builder, module, *dst, value)
                }
                ty if is_jit_variant_carrier_ty(ty) => {
                    self.emit_store_variant(builder, module, *dst, value)
                }
                ty => Err(JitError::unsupported(format!(
                    "JIT Assign lowering supports fast scalars and Variant-backed carriers, got {ty:?} at {dst:?}"
                ))),
            },
            OxInst::ValidateAssignment {
                src,
                intent,
                target_kind,
                target_type_name,
                ..
            } => self.emit_validate_assignment(
                builder,
                module,
                src,
                *intent,
                *target_kind,
                target_type_name,
            ),
            OxInst::AsNew { place, binding } => {
                self.emit_as_new_register(builder, module, *place, binding)
            }
            OxInst::NewRecord { dst, fields } => {
                self.emit_new_record_to_slot(builder, module, *dst, fields)
            }
            OxInst::RecordGet { dst, record, index } => {
                self.emit_record_get_to_slot(builder, module, *dst, record, *index)
            }
            OxInst::RecordSet {
                record,
                index,
                value,
            } => self.emit_record_set(builder, module, *record, *index, value),
            OxInst::RecordLSet { record, value } => {
                self.emit_record_lset(builder, module, *record, value)
            }
            OxInst::RecordArrayGet {
                dst,
                record,
                index,
                indices,
            } => self.emit_record_array_get_to_slot(builder, module, *dst, record, *index, indices),
            OxInst::RecordArraySet {
                record,
                index,
                indices,
                value,
            } => self.emit_record_array_set(builder, module, *record, *index, indices, value),
            OxInst::NewObject { dst, class } => {
                self.emit_new_object_to_slot(builder, module, *dst, class.0)
            }
            OxInst::NewExtern { dst, import } => {
                self.emit_new_extern_to_slot(builder, module, *dst, *import)
            }
            OxInst::Predeclared { dst, class } => {
                self.emit_predeclared_to_slot(builder, module, *dst, class.0)
            }
            OxInst::PredeclaredExtern { dst, import } => {
                self.emit_predeclared_extern_to_slot(builder, module, *dst, *import)
            }
            OxInst::PredeclaredSet { class, value } => {
                self.emit_predeclared_set(builder, module, class.0, value)
            }
            OxInst::PredeclaredExternSet { import, value } => {
                self.emit_predeclared_extern_set(builder, module, *import, value)
            }
            OxInst::FieldGet { dst, object, field } => {
                self.emit_field_get_to_slot(builder, module, *dst, object, *field)
            }
            OxInst::FieldSet {
                object,
                field,
                value,
            } => self.emit_field_set(builder, module, object, *field, value),
            OxInst::FieldArrayGet {
                dst,
                object,
                field,
                indices,
            } => self.emit_field_array_get_to_slot(builder, module, *dst, object, *field, indices),
            OxInst::FieldArraySet {
                object,
                field,
                indices,
                value,
            } => self.emit_field_array_set(builder, module, object, *field, indices, value),
            OxInst::WithEventsGet {
                dst,
                owner,
                binding,
            } => self.emit_withevents_get_to_slot(builder, module, *dst, owner, *binding),
            OxInst::WithEventsSet {
                dst,
                owner,
                binding,
                value,
            } => self.emit_withevents_set_to_slot(builder, module, *dst, owner, *binding, value),
            OxInst::WithEventsClearOwner { dst, owner } => {
                self.emit_withevents_clear_owner_to_slot(builder, module, *dst, owner)
            }
            OxInst::WithEventsFirstOwner {
                dst,
                source,
                binding,
            } => self.emit_withevents_first_owner_to_slot(builder, module, *dst, source, *binding),
            OxInst::WithEventsNextOwner { dst } => {
                self.emit_withevents_next_owner_to_slot(builder, module, *dst)
            }
            OxInst::RaiseEvent {
                source,
                event,
                args,
            } => self.emit_raise_event(builder, module, source, *event, args),
            OxInst::ComCallLate {
                dst,
                recv,
                name,
                default_member,
                invoke_kind,
                args,
            } => self.emit_project_member_call_to_slot(
                builder,
                module,
                *dst,
                ProjectMemberCallInputs {
                    recv,
                    name,
                    default_member: *default_member,
                    invoke_kind: *invoke_kind,
                    args,
                },
            ),
            OxInst::CallByName {
                dst,
                object,
                name,
                calltype,
                args,
            } => self.emit_call_by_name_to_slot(
                builder,
                module,
                *dst,
                CallByNameInputs {
                    object,
                    name,
                    calltype,
                    args,
                },
            ),
            OxInst::Box { dst, src, from } => {
                self.ensure_variant_place(*dst)?;
                if !is_jit_supported_slot_ty(from) {
                    return Err(JitError::unsupported(format!(
                        "JIT Box lowering supports only supported scalar/carrier sources, got {from:?}"
                    )));
                }
                self.emit_store_variant(builder, module, *dst, src)
            }
            OxInst::ArrayLiteral {
                dst,
                values,
                aliases,
                lower_bound,
            } => self.lower_array_literal(builder, module, *dst, values, aliases, *lower_bound),
            OxInst::ArrayRedim {
                dst,
                upper_bounds,
                lower_bounds,
                element,
                preserve,
                fixed,
            } => self.lower_array_redim(
                builder,
                module,
                *dst,
                ArrayRedimInputs {
                    upper_bounds,
                    lower_bounds,
                    element,
                    preserve: *preserve,
                    fixed: *fixed,
                },
            ),
            OxInst::ArrayGet {
                dst,
                array,
                indices,
            } => self.lower_array_get(builder, module, *dst, array, indices),
            OxInst::ArraySet {
                array,
                indices,
                value,
            } => self.lower_array_set(builder, module, *array, indices, value),
            OxInst::ArrayErase { array, element } => {
                self.lower_array_erase(builder, module, *array, element)
            }
            OxInst::Bound {
                dst,
                which,
                array,
                dimension,
            } => self.lower_bound(builder, module, *dst, *which, array, dimension.as_ref()),
            OxInst::ForEachInit { iter, source } => {
                self.lower_for_each_init(builder, module, *iter, source)
            }
            OxInst::ForEachNext {
                iter,
                item,
                has_value,
            } => self.lower_for_each_next(builder, module, *iter, *item, *has_value),
            OxInst::Unbox {
                dst,
                src,
                to,
                checked,
            } => {
                self.ensure_unbox_target_place(*dst, to)?;
                if raw_unbox_target(to).is_none() {
                    return Err(JitError::unsupported(format!(
                        "M4-4 Unbox lowering supports only JIT scalar/carrier targets, got {to:?}"
                    )));
                }
                self.emit_unbox_to_slot(builder, module, *dst, src, to, *checked)
            }
            OxInst::VariantChanged {
                dst,
                current,
                original,
            } => {
                self.ensure_bool_place(*dst)?;
                self.emit_variant_changed_slot_call(builder, module, *dst, current, original)
            }
            OxInst::Coerce { dst, src, target } => {
                if let OxCoerceTarget::Numeric(target_ty) = target {
                    if matches!(place_ty(self.program, self.func, *dst)?, OxTy::Variant) {
                        return self.emit_variant_numeric_coerce_slot_call(
                            builder, module, *dst, *target_ty, src,
                        );
                    }
                    self.ensure_numeric_target_place(*dst, *target_ty)?;
                    if self.numeric_coerce_requires_variant_helper(src, *target_ty)? {
                        return self.emit_variant_numeric_coerce_slot_call(
                            builder, module, *dst, *target_ty, src,
                        );
                    }
                }
                match target {
                    OxCoerceTarget::Numeric(NumericCoerceTarget::Long) => {
                        self.ensure_long_place(*dst)?;
                        let value = self.lower_operand_i32(builder, module, src)?;
                        self.emit_store_i32(builder, module, *dst, value)
                    }
                    OxCoerceTarget::Numeric(NumericCoerceTarget::LongLong) => {
                        self.ensure_longlong_place(*dst)?;
                        let value = self.lower_operand_i64(builder, module, src)?;
                        self.emit_store_i64(builder, module, *dst, value)
                    }
                    OxCoerceTarget::Numeric(NumericCoerceTarget::Currency) => {
                        self.ensure_currency_place(*dst)?;
                        let value = self.lower_operand_currency_i64(builder, module, src)?;
                        self.emit_store_currency_i64(builder, module, *dst, value)
                    }
                    OxCoerceTarget::Numeric(NumericCoerceTarget::Double) => {
                        self.ensure_double_place(*dst)?;
                        let value = self.lower_operand_f64(builder, module, src)?;
                        self.emit_store_f64(builder, module, *dst, value)
                    }
                    OxCoerceTarget::Numeric(NumericCoerceTarget::Date) => {
                        self.ensure_date_place(*dst)?;
                        let value = self.lower_operand_date_f64(builder, module, src)?;
                        self.emit_store_date_f64(builder, module, *dst, value)
                    }
                    OxCoerceTarget::Numeric(NumericCoerceTarget::Single) => {
                        self.ensure_single_place(*dst)?;
                        let value = self.lower_operand_f32(builder, module, src)?;
                        self.emit_store_f32(builder, module, *dst, value)
                    }
                    OxCoerceTarget::Numeric(NumericCoerceTarget::Byte) => {
                        self.ensure_byte_place(*dst)?;
                        let value = self.lower_operand_i32(builder, module, src)?;
                        let zero = builder.ins().iconst(types::I32, 0);
                        self.emit_checked_slot_call(
                            builder,
                            module,
                            *dst,
                            self.imports.add_u8_slot,
                            value,
                            zero,
                        )
                    }
                    OxCoerceTarget::Numeric(NumericCoerceTarget::Integer) => {
                        self.ensure_integer_place(*dst)?;
                        let value = self.lower_operand_i32(builder, module, src)?;
                        let zero = builder.ins().iconst(types::I32, 0);
                        self.emit_checked_slot_call(
                            builder,
                            module,
                            *dst,
                            self.imports.add_i16_slot,
                            value,
                            zero,
                        )
                    }
                    OxCoerceTarget::Numeric(NumericCoerceTarget::Boolean) => {
                        self.ensure_bool_place(*dst)?;
                        let value = self.lower_operand_bool_i32(builder, module, src)?;
                        self.emit_store_bool(builder, module, *dst, value)
                    }
                    OxCoerceTarget::Str => {
                        self.ensure_string_place(*dst)?;
                        if self.operand_is_static_string_source(src)? {
                            return self.emit_store_variant(builder, module, *dst, src);
                        }
                        self.emit_variant_string_coerce_slot_call(builder, module, *dst, src)
                    }
                    OxCoerceTarget::FixedStr(len) => {
                        self.ensure_fixed_string_place(*dst, *len)?;
                        self.emit_variant_fixed_string_coerce_slot_call(
                            builder, module, *dst, *len, src,
                        )
                    }
                    _ => Err(JitError::unsupported(format!(
                        "M4-4 lowers only same-type Numeric(Long/LongLong/Currency/Single/Double/Date/Byte/Integer/Boolean), String, and fixed-length String coercions, got {target:?}"
                    ))),
                }
            }
            OxInst::Arith {
                dst,
                op,
                lhs,
                rhs,
                mode,
            } => match place_ty(self.program, self.func, *dst)? {
                OxTy::Variant => self.emit_variant_arith_slot_call(
                    builder,
                    module,
                    *dst,
                    *op,
                    *mode,
                    BinaryVariantOperands { lhs, rhs },
                ),
                OxTy::Long => {
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
                        (ArithOp::IntDiv, NumericMode::Checked(NumericCoerceTarget::Long)) => {
                            self.imports.div_i32_slot
                        }
                        (ArithOp::Mod, NumericMode::Checked(NumericCoerceTarget::Long)) => {
                            self.imports.rem_i32_slot
                        }
                        _ => {
                            return Err(JitError::unsupported(format!(
                                "M4-4 lowers only checked Long add/sub/mul/intdiv/mod, got {op:?} {mode:?}",
                            )));
                        }
                    };
                    if self
                        .numeric_coerce_requires_variant_helper(lhs, NumericCoerceTarget::Long)?
                        || self.numeric_coerce_requires_variant_helper(
                            rhs,
                            NumericCoerceTarget::Long,
                        )?
                    {
                        return self.emit_variant_arith_slot_call(
                            builder,
                            module,
                            *dst,
                            *op,
                            *mode,
                            BinaryVariantOperands { lhs, rhs },
                        );
                    }
                    let lhs = self.lower_operand_i32(builder, module, lhs)?;
                    let rhs = self.lower_operand_i32(builder, module, rhs)?;
                    self.emit_checked_slot_call(builder, module, *dst, helper, lhs, rhs)
                }
                OxTy::Integer => {
                    let helper = match (op, mode) {
                        (ArithOp::Add, NumericMode::Checked(NumericCoerceTarget::Integer)) => {
                            self.imports.add_i16_slot
                        }
                        (ArithOp::Sub, NumericMode::Checked(NumericCoerceTarget::Integer)) => {
                            self.imports.sub_i16_slot
                        }
                        (ArithOp::Mul, NumericMode::Checked(NumericCoerceTarget::Integer)) => {
                            self.imports.mul_i16_slot
                        }
                        _ => {
                            return Err(JitError::unsupported(format!(
                                "M4-4 lowers only checked Long/Integer/Byte/LongLong/Currency add/sub/mul, got {op:?} {mode:?}",
                            )));
                        }
                    };
                    if self
                        .numeric_coerce_requires_variant_helper(lhs, NumericCoerceTarget::Integer)?
                        || self.numeric_coerce_requires_variant_helper(
                            rhs,
                            NumericCoerceTarget::Integer,
                        )?
                    {
                        return self.emit_variant_arith_slot_call(
                            builder,
                            module,
                            *dst,
                            *op,
                            *mode,
                            BinaryVariantOperands { lhs, rhs },
                        );
                    }
                    let lhs = self.lower_operand_i16_i32(builder, module, lhs)?;
                    let rhs = self.lower_operand_i16_i32(builder, module, rhs)?;
                    self.emit_checked_slot_call(builder, module, *dst, helper, lhs, rhs)
                }
                OxTy::Byte => {
                    let helper = match (op, mode) {
                        (ArithOp::Add, NumericMode::Checked(NumericCoerceTarget::Byte)) => {
                            self.imports.add_u8_slot
                        }
                        (ArithOp::Sub, NumericMode::Checked(NumericCoerceTarget::Byte)) => {
                            self.imports.sub_u8_slot
                        }
                        (ArithOp::Mul, NumericMode::Checked(NumericCoerceTarget::Byte)) => {
                            self.imports.mul_u8_slot
                        }
                        _ => {
                            return Err(JitError::unsupported(format!(
                                "M4-4 lowers only checked Long/Integer/Byte/LongLong/Currency add/sub/mul, got {op:?} {mode:?}",
                            )));
                        }
                    };
                    if self
                        .numeric_coerce_requires_variant_helper(lhs, NumericCoerceTarget::Byte)?
                        || self.numeric_coerce_requires_variant_helper(
                            rhs,
                            NumericCoerceTarget::Byte,
                        )?
                    {
                        return self.emit_variant_arith_slot_call(
                            builder,
                            module,
                            *dst,
                            *op,
                            *mode,
                            BinaryVariantOperands { lhs, rhs },
                        );
                    }
                    let lhs = self.lower_operand_u8_i32(builder, module, lhs)?;
                    let rhs = self.lower_operand_u8_i32(builder, module, rhs)?;
                    self.emit_checked_slot_call(builder, module, *dst, helper, lhs, rhs)
                }
                OxTy::LongLong => {
                    let helper = match (op, mode) {
                        (ArithOp::Add, NumericMode::Checked(NumericCoerceTarget::LongLong)) => {
                            self.imports.add_i64_slot
                        }
                        (ArithOp::Sub, NumericMode::Checked(NumericCoerceTarget::LongLong)) => {
                            self.imports.sub_i64_slot
                        }
                        (ArithOp::Mul, NumericMode::Checked(NumericCoerceTarget::LongLong)) => {
                            self.imports.mul_i64_slot
                        }
                        (ArithOp::IntDiv, NumericMode::Checked(NumericCoerceTarget::LongLong)) => {
                            self.imports.div_i64_slot
                        }
                        (ArithOp::Mod, NumericMode::Checked(NumericCoerceTarget::LongLong)) => {
                            self.imports.rem_i64_slot
                        }
                        _ => {
                            return Err(JitError::unsupported(format!(
                                "M4-4 lowers only checked LongLong add/sub/mul/intdiv/mod, got {op:?} {mode:?}",
                            )));
                        }
                    };
                    if self.numeric_coerce_requires_variant_helper(
                        lhs,
                        NumericCoerceTarget::LongLong,
                    )? || self.numeric_coerce_requires_variant_helper(
                        rhs,
                        NumericCoerceTarget::LongLong,
                    )? {
                        return self.emit_variant_arith_slot_call(
                            builder,
                            module,
                            *dst,
                            *op,
                            *mode,
                            BinaryVariantOperands { lhs, rhs },
                        );
                    }
                    let lhs = self.lower_operand_i64(builder, module, lhs)?;
                    let rhs = self.lower_operand_i64(builder, module, rhs)?;
                    self.emit_checked_slot_call(builder, module, *dst, helper, lhs, rhs)
                }
                OxTy::Currency => {
                    let helper = match (op, mode) {
                        (ArithOp::Add, NumericMode::Checked(NumericCoerceTarget::Currency)) => {
                            self.imports.add_currency_slot
                        }
                        (ArithOp::Sub, NumericMode::Checked(NumericCoerceTarget::Currency)) => {
                            self.imports.sub_currency_slot
                        }
                        (ArithOp::Mul, NumericMode::Checked(NumericCoerceTarget::Currency)) => {
                            self.imports.mul_currency_slot
                        }
                        _ => {
                            return Err(JitError::unsupported(format!(
                                "M4-4 lowers only checked Long/LongLong/Currency add/sub/mul, got {op:?} {mode:?}",
                            )));
                        }
                    };
                    if self.numeric_coerce_requires_variant_helper(
                        lhs,
                        NumericCoerceTarget::Currency,
                    )? || self.numeric_coerce_requires_variant_helper(
                        rhs,
                        NumericCoerceTarget::Currency,
                    )? {
                        return self.emit_variant_arith_slot_call(
                            builder,
                            module,
                            *dst,
                            *op,
                            *mode,
                            BinaryVariantOperands { lhs, rhs },
                        );
                    }
                    let lhs = self.lower_operand_currency_i64(builder, module, lhs)?;
                    let rhs = self.lower_operand_currency_i64(builder, module, rhs)?;
                    self.emit_checked_slot_call(builder, module, *dst, helper, lhs, rhs)
                }
                OxTy::Single => {
                    if !matches!(mode, NumericMode::Checked(NumericCoerceTarget::Single)) {
                        return Err(JitError::unsupported(format!(
                            "M4-4 lowers only checked Single add/sub/mul, got {op:?} {mode:?}",
                        )));
                    }
                    match op {
                        ArithOp::Add | ArithOp::Sub | ArithOp::Mul => {}
                        _ => {
                            return Err(JitError::unsupported(format!(
                                "M4-4 lowers only checked Single add/sub/mul, got {op:?} {mode:?}",
                            )));
                        }
                    }
                    if self
                        .numeric_coerce_requires_variant_helper(lhs, NumericCoerceTarget::Single)?
                        || self.numeric_coerce_requires_variant_helper(
                            rhs,
                            NumericCoerceTarget::Single,
                        )?
                    {
                        return self.emit_variant_arith_slot_call(
                            builder,
                            module,
                            *dst,
                            *op,
                            *mode,
                            BinaryVariantOperands { lhs, rhs },
                        );
                    }
                    let lhs = self.lower_operand_f32(builder, module, lhs)?;
                    let rhs = self.lower_operand_f32(builder, module, rhs)?;
                    let value = match op {
                        ArithOp::Add => builder.ins().fadd(lhs, rhs),
                        ArithOp::Sub => builder.ins().fsub(lhs, rhs),
                        ArithOp::Mul => builder.ins().fmul(lhs, rhs),
                        _ => unreachable!("checked Single op above"),
                    };
                    self.emit_overflow_if_not_finite(builder, module, value, true)?;
                    self.emit_store_f32(builder, module, *dst, value)
                }
                OxTy::Double => {
                    if !matches!(mode, NumericMode::Checked(NumericCoerceTarget::Double)) {
                        return Err(JitError::unsupported(format!(
                            "M4-4 lowers only checked Double add/sub/mul, got {op:?} {mode:?}",
                        )));
                    }
                    match op {
                        ArithOp::Add | ArithOp::Sub | ArithOp::Mul => {}
                        _ => {
                            return Err(JitError::unsupported(format!(
                                "M4-4 lowers only checked Double add/sub/mul, got {op:?} {mode:?}",
                            )));
                        }
                    }
                    if self
                        .numeric_coerce_requires_variant_helper(lhs, NumericCoerceTarget::Double)?
                        || self.numeric_coerce_requires_variant_helper(
                            rhs,
                            NumericCoerceTarget::Double,
                        )?
                    {
                        return self.emit_variant_arith_slot_call(
                            builder,
                            module,
                            *dst,
                            *op,
                            *mode,
                            BinaryVariantOperands { lhs, rhs },
                        );
                    }
                    let lhs = self.lower_operand_f64(builder, module, lhs)?;
                    let rhs = self.lower_operand_f64(builder, module, rhs)?;
                    let value = match op {
                        ArithOp::Add => builder.ins().fadd(lhs, rhs),
                        ArithOp::Sub => builder.ins().fsub(lhs, rhs),
                        ArithOp::Mul => builder.ins().fmul(lhs, rhs),
                        _ => unreachable!("checked Double op above"),
                    };
                    // NB: unlike Single, vm3 does NOT raise Overflow on a
                    // non-finite Double result (it yields ±Inf), so the JIT must
                    // match that for parity — no finite check here. (vm3's Double
                    // overflow conformance vs VBA is tracked separately.)
                    self.emit_store_f64(builder, module, *dst, value)
                }
                ty => Err(JitError::unsupported(format!(
                    "M4-4 lowers checked arithmetic only for current Long/Integer/Byte/LongLong/Currency/Single/Double destinations, got {ty:?} at {dst:?}"
                ))),
            },
            OxInst::Div { dst, lhs, rhs } => {
                self.ensure_double_place(*dst)?;
                self.emit_variant_arith_raw_slot_call(
                    builder,
                    module,
                    *dst,
                    RT_ARITH_DIV,
                    RT_NUMERIC_WIDENING,
                    BinaryVariantOperands { lhs, rhs },
                )
            }
            OxInst::Pow { dst, lhs, rhs } => {
                self.ensure_double_place(*dst)?;
                self.emit_variant_arith_raw_slot_call(
                    builder,
                    module,
                    *dst,
                    RT_ARITH_POW,
                    RT_NUMERIC_WIDENING,
                    BinaryVariantOperands { lhs, rhs },
                )
            }
            OxInst::Concat { dst, lhs, rhs } => {
                self.ensure_concat_destination_place(*dst)?;
                self.emit_variant_concat_slot_call(builder, module, *dst, lhs, rhs)
            }
            OxInst::Neg { dst, src, mode } => match place_ty(self.program, self.func, *dst)? {
                OxTy::Long => {
                    if !matches!(mode, NumericMode::Checked(NumericCoerceTarget::Long)) {
                        return Err(JitError::unsupported(format!(
                            "M4-4 lowers only checked Long negation, got {mode:?}",
                        )));
                    }
                    let zero = builder.ins().iconst(types::I32, 0);
                    let src = self.lower_operand_i32(builder, module, src)?;
                    self.emit_checked_slot_call(
                        builder,
                        module,
                        *dst,
                        self.imports.sub_i32_slot,
                        zero,
                        src,
                    )
                }
                OxTy::Integer => {
                    if !matches!(mode, NumericMode::Checked(NumericCoerceTarget::Integer)) {
                        return Err(JitError::unsupported(format!(
                            "M4-4 lowers only checked Integer negation, got {mode:?}",
                        )));
                    }
                    let zero = builder.ins().iconst(types::I32, 0);
                    let src = self.lower_operand_i16_i32(builder, module, src)?;
                    self.emit_checked_slot_call(
                        builder,
                        module,
                        *dst,
                        self.imports.sub_i16_slot,
                        zero,
                        src,
                    )
                }
                OxTy::Byte => {
                    if !matches!(mode, NumericMode::Checked(NumericCoerceTarget::Byte)) {
                        return Err(JitError::unsupported(format!(
                            "M4-4 lowers only checked Byte negation, got {mode:?}",
                        )));
                    }
                    let zero = builder.ins().iconst(types::I32, 0);
                    let src = self.lower_operand_u8_i32(builder, module, src)?;
                    self.emit_checked_slot_call(
                        builder,
                        module,
                        *dst,
                        self.imports.sub_u8_slot,
                        zero,
                        src,
                    )
                }
                OxTy::LongLong => {
                    if !matches!(mode, NumericMode::Checked(NumericCoerceTarget::LongLong)) {
                        return Err(JitError::unsupported(format!(
                            "M4-4 lowers only checked LongLong negation, got {mode:?}",
                        )));
                    }
                    let zero = builder.ins().iconst(types::I64, 0);
                    let src = self.lower_operand_i64(builder, module, src)?;
                    self.emit_checked_slot_call(
                        builder,
                        module,
                        *dst,
                        self.imports.sub_i64_slot,
                        zero,
                        src,
                    )
                }
                OxTy::Currency => {
                    if !matches!(mode, NumericMode::Checked(NumericCoerceTarget::Currency)) {
                        return Err(JitError::unsupported(format!(
                            "M4-4 lowers only checked Currency negation, got {mode:?}",
                        )));
                    }
                    let zero = builder.ins().iconst(types::I64, 0);
                    let src = self.lower_operand_currency_i64(builder, module, src)?;
                    self.emit_checked_slot_call(
                        builder,
                        module,
                        *dst,
                        self.imports.sub_currency_slot,
                        zero,
                        src,
                    )
                }
                OxTy::Single => {
                    if !matches!(mode, NumericMode::Checked(NumericCoerceTarget::Single)) {
                        return Err(JitError::unsupported(format!(
                            "M4-4 lowers only checked Single negation, got {mode:?}",
                        )));
                    }
                    let src = self.lower_operand_f32(builder, module, src)?;
                    let value = builder.ins().fneg(src);
                    self.emit_store_f32(builder, module, *dst, value)
                }
                OxTy::Double => {
                    if !matches!(mode, NumericMode::Checked(NumericCoerceTarget::Double)) {
                        return Err(JitError::unsupported(format!(
                            "M4-4 lowers only checked Double negation, got {mode:?}",
                        )));
                    }
                    let src = self.lower_operand_f64(builder, module, src)?;
                    let value = builder.ins().fneg(src);
                    self.emit_store_f64(builder, module, *dst, value)
                }
                OxTy::Variant => {
                    if !matches!(mode, NumericMode::Widening) {
                        return Err(JitError::unsupported(format!(
                            "M4-4 Variant negation lowering requires Widening mode, got {mode:?}",
                        )));
                    }
                    self.emit_variant_neg_slot_call(builder, module, *dst, *mode, src)
                }
                ty => Err(JitError::unsupported(format!(
                    "M4-4 lowers checked scalar negation or Widening Variant negation only for Long/Integer/Byte/LongLong/Currency/Single/Double/Variant destinations, got {ty:?} at {dst:?}"
                ))),
            },
            OxInst::CallProc { dst, proc, args } => {
                self.lower_call_proc(builder, module, *dst, *proc, args)
            }
            OxInst::CallExtern { dst, import, args } => {
                self.lower_call_extern(builder, module, *dst, *import, args)
            }
            OxInst::CallNative { dst, callee, args } => {
                self.lower_call_native(builder, module, *dst, callee, args)
            }
            OxInst::LoadProcRef { dst, proc } => {
                self.ensure_proc_ref_place(*dst)?;
                let (area, index) = place_addr(*dst);
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                let proc = builder.ins().iconst(types::I32, proc.0 as i64);
                let callee = self.import(builder, module, self.imports.store_proc_ref);
                let call = builder.ins().call(callee, &[self.run, area, index, proc]);
                let status = builder.inst_results(call)[0];
                self.return_if_not_ok(builder, status);
                Ok(())
            }
            OxInst::CallProcRef { dst, target, args } => {
                self.lower_call_proc_ref(builder, module, *dst, target, args)
            }
            OxInst::CompareObjectIs { dst, lhs, rhs } => {
                self.emit_compare_object_is_slot_call(builder, module, *dst, lhs, rhs)
            }
            OxInst::TypeOfIs {
                dst,
                object,
                type_name,
            } => self.lower_type_of_is(builder, module, *dst, object, type_name),
            OxInst::Compare {
                dst,
                op,
                lhs,
                rhs,
                mode,
            } => match place_ty(self.program, self.func, *dst)? {
                OxTy::Variant => self.emit_variant_compare_slot_call(
                    builder,
                    module,
                    *dst,
                    *op,
                    *mode,
                    BinaryVariantOperands { lhs, rhs },
                ),
                OxTy::Bool => {
                    let lhs_float_lane = self.compare_float_lane(lhs)?;
                    let rhs_float_lane = self.compare_float_lane(rhs)?;
                    let lhs_lane = self.compare_i64_lane(lhs)?;
                    let rhs_lane = self.compare_i64_lane(rhs)?;
                    let cmp = if lhs_float_lane.is_some() || rhs_float_lane.is_some() {
                        match (lhs_float_lane, rhs_float_lane) {
                            (Some(FloatCompareLane::Single), Some(FloatCompareLane::Single)) => {
                                let lhs = self.lower_operand_f32(builder, module, lhs)?;
                                let rhs = self.lower_operand_f32(builder, module, rhs)?;
                                builder.ins().fcmp(float_compare_cc(*op), lhs, rhs)
                            }
                            (Some(FloatCompareLane::Double), Some(FloatCompareLane::Double)) => {
                                let lhs = self.lower_operand_f64(builder, module, lhs)?;
                                let rhs = self.lower_operand_f64(builder, module, rhs)?;
                                builder.ins().fcmp(float_compare_cc(*op), lhs, rhs)
                            }
                            (Some(FloatCompareLane::Date), Some(FloatCompareLane::Date)) => {
                                let lhs = self.lower_operand_date_f64(builder, module, lhs)?;
                                let rhs = self.lower_operand_date_f64(builder, module, rhs)?;
                                builder.ins().fcmp(float_compare_cc(*op), lhs, rhs)
                            }
                            _ => {
                                return Err(JitError::unsupported(
                                    "M4-4 floating compare lowering requires both operands in the same Single, Double, or Date lane",
                                ));
                            }
                        }
                    } else if matches!(
                        (lhs_lane, rhs_lane),
                        (Some(I64CompareLane::Currency), _) | (_, Some(I64CompareLane::Currency))
                    ) {
                        if lhs_lane != Some(I64CompareLane::Currency)
                            || rhs_lane != Some(I64CompareLane::Currency)
                        {
                            return Err(JitError::unsupported(
                                "M4-4 Currency compare lowering requires both operands in the Currency lane",
                            ));
                        }
                        let lhs = self.lower_operand_currency_i64(builder, module, lhs)?;
                        let rhs = self.lower_operand_currency_i64(builder, module, rhs)?;
                        builder.ins().icmp(int_compare_cc(*op), lhs, rhs)
                    } else if lhs_lane == Some(I64CompareLane::Plain)
                        || rhs_lane == Some(I64CompareLane::Plain)
                    {
                        let lhs = self.lower_operand_i64(builder, module, lhs)?;
                        let rhs = self.lower_operand_i64(builder, module, rhs)?;
                        builder.ins().icmp(int_compare_cc(*op), lhs, rhs)
                    } else {
                        let lhs = self.lower_operand_i32(builder, module, lhs)?;
                        let rhs = self.lower_operand_i32(builder, module, rhs)?;
                        builder.ins().icmp(int_compare_cc(*op), lhs, rhs)
                    };
                    let one = builder.ins().iconst(types::I32, 1);
                    let zero = builder.ins().iconst(types::I32, 0);
                    let value = builder.ins().select(cmp, one, zero);
                    self.emit_store_bool(builder, module, *dst, value)
                }
                ty => Err(JitError::unsupported(format!(
                    "M4-4 lowers only Variant/Bool Compare places, got {ty:?} at {dst:?}"
                ))),
            },
            OxInst::Logical { dst, op, lhs, rhs } => {
                match place_ty(self.program, self.func, *dst)? {
                    OxTy::Variant => {
                        self.emit_variant_logical_slot_call(builder, module, *dst, *op, lhs, rhs)
                    }
                    OxTy::Bool => {
                        let lhs = self.lower_operand_bool_i32(builder, module, lhs)?;
                        let rhs = self.lower_operand_bool_i32(builder, module, rhs)?;
                        let value = self.emit_bool_logical(builder, *op, lhs, rhs);
                        self.emit_store_bool(builder, module, *dst, value)
                    }
                    OxTy::Long => {
                        let lhs = self.lower_operand_i32(builder, module, lhs)?;
                        let rhs = self.lower_operand_i32(builder, module, rhs)?;
                        let value = self.emit_numeric_logical(builder, *op, lhs, rhs, types::I32);
                        self.emit_store_i32(builder, module, *dst, value)
                    }
                    OxTy::Integer => {
                        let lhs = self.lower_operand_i16_i32(builder, module, lhs)?;
                        let rhs = self.lower_operand_i16_i32(builder, module, rhs)?;
                        let value = self.emit_numeric_logical(builder, *op, lhs, rhs, types::I32);
                        self.emit_store_i16(builder, module, *dst, value)
                    }
                    OxTy::LongLong => {
                        let lhs = self.lower_operand_i64(builder, module, lhs)?;
                        let rhs = self.lower_operand_i64(builder, module, rhs)?;
                        let value = self.emit_numeric_logical(builder, *op, lhs, rhs, types::I64);
                        self.emit_store_i64(builder, module, *dst, value)
                    }
                    ty => Err(JitError::unsupported(format!(
                        "M4-4 lowers only Variant/Bool/Long/Integer/LongLong Logical places, got {ty:?} at {dst:?}"
                    ))),
                }
            }
            OxInst::Not { dst, src } => match place_ty(self.program, self.func, *dst)? {
                OxTy::Variant => self.emit_variant_not_slot_call(builder, module, *dst, src),
                OxTy::Bool => {
                    let value = self.lower_operand_bool_i32(builder, module, src)?;
                    let zero = builder.ins().iconst(types::I32, 0);
                    let is_false = builder.ins().icmp(ir::condcodes::IntCC::Equal, value, zero);
                    let one = builder.ins().iconst(types::I32, 1);
                    let value = builder.ins().select(is_false, one, zero);
                    self.emit_store_bool(builder, module, *dst, value)
                }
                OxTy::Long => {
                    let value = self.lower_operand_i32(builder, module, src)?;
                    let value = self.emit_numeric_not(builder, value, types::I32);
                    self.emit_store_i32(builder, module, *dst, value)
                }
                OxTy::Integer => {
                    let value = self.lower_operand_i16_i32(builder, module, src)?;
                    let value = self.emit_numeric_not(builder, value, types::I32);
                    self.emit_store_i16(builder, module, *dst, value)
                }
                OxTy::LongLong => {
                    let value = self.lower_operand_i64(builder, module, src)?;
                    let value = self.emit_numeric_not(builder, value, types::I64);
                    self.emit_store_i64(builder, module, *dst, value)
                }
                ty => Err(JitError::unsupported(format!(
                    "M4-4 lowers only Variant/Bool/Long/Integer/LongLong Not places, got {ty:?} at {dst:?}"
                ))),
            },
            OxInst::Truthy { dst, src } => {
                self.ensure_bool_place(*dst)?;
                let use_variant_truthy = match src {
                    OxOperand::Use(place) => {
                        matches!(place_ty(self.program, self.func, *place)?, OxTy::Variant)
                    }
                    OxOperand::Const(OxConst::Empty | OxConst::Null) => true,
                    _ => false,
                };
                if use_variant_truthy {
                    return self.emit_variant_truthy_slot_call(builder, module, *dst, src);
                }
                let raw = match src {
                    OxOperand::Use(place)
                        if matches!(place_ty(self.program, self.func, *place)?, OxTy::Bool) =>
                    {
                        self.lower_operand_bool_i32(builder, module, src)?
                    }
                    OxOperand::Use(place)
                        if matches!(place_ty(self.program, self.func, *place)?, OxTy::LongLong) =>
                    {
                        let value = self.lower_operand_i64(builder, module, src)?;
                        let zero = builder.ins().iconst(types::I64, 0);
                        let truthy =
                            builder
                                .ins()
                                .icmp(ir::condcodes::IntCC::NotEqual, value, zero);
                        let one = builder.ins().iconst(types::I32, 1);
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().select(truthy, one, zero)
                    }
                    OxOperand::Use(place)
                        if matches!(place_ty(self.program, self.func, *place)?, OxTy::Currency) =>
                    {
                        let value = self.lower_operand_currency_i64(builder, module, src)?;
                        let zero = builder.ins().iconst(types::I64, 0);
                        let truthy =
                            builder
                                .ins()
                                .icmp(ir::condcodes::IntCC::NotEqual, value, zero);
                        let one = builder.ins().iconst(types::I32, 1);
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().select(truthy, one, zero)
                    }
                    OxOperand::Use(place)
                        if matches!(place_ty(self.program, self.func, *place)?, OxTy::Single) =>
                    {
                        let value = self.lower_operand_f32(builder, module, src)?;
                        let zero = builder.ins().f32const(Ieee32::with_float(0.0));
                        let truthy =
                            builder
                                .ins()
                                .fcmp(ir::condcodes::FloatCC::NotEqual, value, zero);
                        let one = builder.ins().iconst(types::I32, 1);
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().select(truthy, one, zero)
                    }
                    OxOperand::Use(place)
                        if matches!(place_ty(self.program, self.func, *place)?, OxTy::Double) =>
                    {
                        let value = self.lower_operand_f64(builder, module, src)?;
                        let zero = builder.ins().f64const(Ieee64::with_float(0.0));
                        let truthy =
                            builder
                                .ins()
                                .fcmp(ir::condcodes::FloatCC::NotEqual, value, zero);
                        let one = builder.ins().iconst(types::I32, 1);
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().select(truthy, one, zero)
                    }
                    OxOperand::Use(place)
                        if matches!(place_ty(self.program, self.func, *place)?, OxTy::Date) =>
                    {
                        let value = self.lower_operand_date_f64(builder, module, src)?;
                        let zero = builder.ins().f64const(Ieee64::with_float(0.0));
                        let truthy =
                            builder
                                .ins()
                                .fcmp(ir::condcodes::FloatCC::NotEqual, value, zero);
                        let one = builder.ins().iconst(types::I32, 1);
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().select(truthy, one, zero)
                    }
                    OxOperand::Const(OxConst::I64(_)) => {
                        let value = self.lower_operand_i64(builder, module, src)?;
                        let zero = builder.ins().iconst(types::I64, 0);
                        let truthy =
                            builder
                                .ins()
                                .icmp(ir::condcodes::IntCC::NotEqual, value, zero);
                        let one = builder.ins().iconst(types::I32, 1);
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().select(truthy, one, zero)
                    }
                    OxOperand::Const(OxConst::Currency(_)) => {
                        let value = self.lower_operand_currency_i64(builder, module, src)?;
                        let zero = builder.ins().iconst(types::I64, 0);
                        let truthy =
                            builder
                                .ins()
                                .icmp(ir::condcodes::IntCC::NotEqual, value, zero);
                        let one = builder.ins().iconst(types::I32, 1);
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().select(truthy, one, zero)
                    }
                    OxOperand::Const(OxConst::F32(_)) => {
                        let value = self.lower_operand_f32(builder, module, src)?;
                        let zero = builder.ins().f32const(Ieee32::with_float(0.0));
                        let truthy =
                            builder
                                .ins()
                                .fcmp(ir::condcodes::FloatCC::NotEqual, value, zero);
                        let one = builder.ins().iconst(types::I32, 1);
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().select(truthy, one, zero)
                    }
                    OxOperand::Const(OxConst::F64(_)) => {
                        let value = self.lower_operand_f64(builder, module, src)?;
                        let zero = builder.ins().f64const(Ieee64::with_float(0.0));
                        let truthy =
                            builder
                                .ins()
                                .fcmp(ir::condcodes::FloatCC::NotEqual, value, zero);
                        let one = builder.ins().iconst(types::I32, 1);
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().select(truthy, one, zero)
                    }
                    OxOperand::Const(OxConst::Date(_)) => {
                        let value = self.lower_operand_date_f64(builder, module, src)?;
                        let zero = builder.ins().f64const(Ieee64::with_float(0.0));
                        let truthy =
                            builder
                                .ins()
                                .fcmp(ir::condcodes::FloatCC::NotEqual, value, zero);
                        let one = builder.ins().iconst(types::I32, 1);
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().select(truthy, one, zero)
                    }
                    _ => {
                        let value = self.lower_operand_i32(builder, module, src)?;
                        let zero = builder.ins().iconst(types::I32, 0);
                        let truthy =
                            builder
                                .ins()
                                .icmp(ir::condcodes::IntCC::NotEqual, value, zero);
                        let one = builder.ins().iconst(types::I32, 1);
                        builder.ins().select(truthy, one, zero)
                    }
                };
                self.emit_store_bool(builder, module, *dst, raw)
            }
            OxInst::SetErrorHandler(handler) => {
                self.emit_set_error_handler(builder, module, handler)
            }
            OxInst::ClearErr => self.emit_err_clear(builder, module),
            OxInst::ErrFieldGet { dst, field } => {
                self.emit_err_field_get(builder, module, *dst, *field)
            }
            OxInst::ErlGet { dst } => self.emit_erl_get(builder, module, *dst),
            OxInst::ErrFieldSet { field, src } => {
                self.emit_err_field_set(builder, module, *field, src)
            }
            other => Err(JitError::unsupported(format!(
                "instruction not lowered in M4-4: {other:?}"
            ))),
        }
    }

    fn emit_set_error_handler(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        handler: &ErrorHandler,
    ) -> Result<(), JitError> {
        let (kind, block) = match handler {
            ErrorHandler::ResumeNext => (RT_ERROR_HANDLER_RESUME_NEXT, 0),
            ErrorHandler::Goto0 => (RT_ERROR_HANDLER_GOTO_0, 0),
            ErrorHandler::GotoMinus1 => (RT_ERROR_HANDLER_GOTO_MINUS_1, 0),
            ErrorHandler::GotoLabel(target) => (RT_ERROR_HANDLER_GOTO_LABEL, target.0),
        };
        let kind = builder.ins().iconst(types::I32, i64::from(kind));
        let block = builder.ins().iconst(types::I32, block as i64);
        let callee = self.import(builder, module, self.imports.set_error_handler);
        let call = builder.ins().call(callee, &[self.state, kind, block]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_err_clear(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
    ) -> Result<(), JitError> {
        let callee = self.import(builder, module, self.imports.err_clear);
        let call = builder.ins().call(callee, &[self.state]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_err_field_get(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        field: ErrField,
    ) -> Result<(), JitError> {
        let raw_field = raw_err_field(field);
        let slot =
            builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 4, 2));
        let ptr_ty = module.target_config().pointer_type();
        let out = builder.ins().stack_addr(ptr_ty, slot, 0);
        match field {
            ErrField::Number | ErrField::HelpContext | ErrField::LastDllError => {
                let raw_field = builder.ins().iconst(types::I32, i64::from(raw_field));
                let callee = self.import(builder, module, self.imports.err_i32_field);
                let call = builder.ins().call(callee, &[self.state, raw_field, out]);
                let status = builder.inst_results(call)[0];
                self.return_if_not_ok(builder, status);
                let value = builder.ins().stack_load(types::I32, slot, 0);
                match place_ty(self.program, self.func, dst)? {
                    OxTy::Long => self.emit_store_i32(builder, module, dst, value),
                    OxTy::Variant => self.emit_store_i32_variant_value(builder, module, dst, value),
                    ty => Err(JitError::unsupported(format!(
                        "M4-4 Err numeric field read lowers only Long/Variant destinations, got {ty:?}"
                    ))),
                }
            }
            ErrField::Description | ErrField::Source | ErrField::HelpFile => {
                let ptr_slot = builder.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    8,
                    3,
                ));
                let len_slot = builder.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    4,
                    2,
                ));
                let ptr_out = builder.ins().stack_addr(ptr_ty, ptr_slot, 0);
                let len_out = builder.ins().stack_addr(ptr_ty, len_slot, 0);
                let raw_field = builder.ins().iconst(types::I32, i64::from(raw_field));
                let callee = self.import(builder, module, self.imports.err_string_field_utf8);
                let call = builder
                    .ins()
                    .call(callee, &[self.state, raw_field, ptr_out, len_out]);
                let status = builder.inst_results(call)[0];
                self.return_if_not_ok(builder, status);
                let ptr = builder.ins().stack_load(ptr_ty, ptr_slot, 0);
                let len = builder.ins().stack_load(types::I32, len_slot, 0);
                match place_ty(self.program, self.func, dst)? {
                    OxTy::Str | OxTy::Variant => {
                        self.emit_store_utf8_variant_value(builder, module, dst, ptr, len)
                    }
                    ty => Err(JitError::unsupported(format!(
                        "M4-4 Err string field read lowers only String/Variant destinations, got {ty:?}"
                    ))),
                }
            }
        }
    }

    fn emit_set_line_number(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        line: i32,
    ) -> Result<(), JitError> {
        let line = builder.ins().iconst(types::I32, i64::from(line));
        let callee = self.import(builder, module, self.imports.set_line_number);
        let call = builder.ins().call(callee, &[self.run, line]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_erl_get(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
    ) -> Result<(), JitError> {
        let slot =
            builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 4, 2));
        let ptr_ty = module.target_config().pointer_type();
        let out = builder.ins().stack_addr(ptr_ty, slot, 0);
        let callee = self.import(builder, module, self.imports.erl_get);
        let call = builder.ins().call(callee, &[self.state, out]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        let value = builder.ins().stack_load(types::I32, slot, 0);
        match place_ty(self.program, self.func, dst)? {
            OxTy::Long => self.emit_store_i32(builder, module, dst, value),
            OxTy::Variant => self.emit_store_i32_variant_value(builder, module, dst, value),
            ty => Err(JitError::unsupported(format!(
                "JIT Erl read lowers only Long/Variant destinations, got {ty:?}"
            ))),
        }
    }

    fn emit_err_field_set(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        field: ErrField,
        src: &OxOperand,
    ) -> Result<(), JitError> {
        let operand = self.lower_variant_operand(builder, src)?;
        let callee = self.import(builder, module, self.imports.err_set_field);
        let field = builder
            .ins()
            .iconst(types::I32, i64::from(raw_err_field(field)));
        let call = builder.ins().call(
            callee,
            &[
                self.run,
                self.state,
                field,
                operand.kind,
                operand.value,
                operand.area,
                operand.index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_store_i32_variant_value(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        value: Value,
    ) -> Result<(), JitError> {
        let zero = builder.ins().iconst(types::I32, 0);
        let operand = LoweredVariantOperand {
            kind: builder
                .ins()
                .iconst(types::I32, i64::from(JIT_VARIANT_OPERAND_I32)),
            value: builder.ins().sextend(types::I64, value),
            area: zero,
            index: zero,
        };
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &[operand])?;
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.store_variant);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, operand_ptr, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_store_utf8_variant_value(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        ptr: Value,
        len: Value,
    ) -> Result<(), JitError> {
        let zero = builder.ins().iconst(types::I32, 0);
        let ptr_ty = module.target_config().pointer_type();
        let value = if ptr_ty == types::I64 {
            ptr
        } else {
            builder.ins().uextend(types::I64, ptr)
        };
        let operand = LoweredVariantOperand {
            kind: builder
                .ins()
                .iconst(types::I32, i64::from(JIT_VARIANT_OPERAND_STR_UTF8)),
            value,
            area: zero,
            index: len,
        };
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &[operand])?;
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.store_variant);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, operand_ptr, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_bool_logical(
        &self,
        builder: &mut FunctionBuilder<'_>,
        op: LogicalOp,
        lhs: Value,
        rhs: Value,
    ) -> Value {
        match op {
            LogicalOp::And => builder.ins().band(lhs, rhs),
            LogicalOp::Or => builder.ins().bor(lhs, rhs),
            LogicalOp::Xor => builder.ins().bxor(lhs, rhs),
            LogicalOp::Eqv => {
                let eq = builder.ins().icmp(ir::condcodes::IntCC::Equal, lhs, rhs);
                let one = builder.ins().iconst(types::I32, 1);
                let zero = builder.ins().iconst(types::I32, 0);
                builder.ins().select(eq, one, zero)
            }
            LogicalOp::Imp => {
                let zero = builder.ins().iconst(types::I32, 0);
                let lhs_false = builder.ins().icmp(ir::condcodes::IntCC::Equal, lhs, zero);
                let one = builder.ins().iconst(types::I32, 1);
                builder.ins().select(lhs_false, one, rhs)
            }
        }
    }

    fn emit_numeric_logical(
        &self,
        builder: &mut FunctionBuilder<'_>,
        op: LogicalOp,
        lhs: Value,
        rhs: Value,
        ty: ir::Type,
    ) -> Value {
        match op {
            LogicalOp::And => builder.ins().band(lhs, rhs),
            LogicalOp::Or => builder.ins().bor(lhs, rhs),
            LogicalOp::Xor => builder.ins().bxor(lhs, rhs),
            LogicalOp::Eqv => {
                let xor = builder.ins().bxor(lhs, rhs);
                self.emit_numeric_not(builder, xor, ty)
            }
            LogicalOp::Imp => {
                let not_lhs = self.emit_numeric_not(builder, lhs, ty);
                builder.ins().bor(not_lhs, rhs)
            }
        }
    }

    fn emit_numeric_not(
        &self,
        builder: &mut FunctionBuilder<'_>,
        value: Value,
        ty: ir::Type,
    ) -> Value {
        let all_ones = builder.ins().iconst(ty, -1);
        builder.ins().bxor(value, all_ones)
    }

    fn compare_i64_lane(&self, operand: &OxOperand) -> Result<Option<I64CompareLane>, JitError> {
        match operand {
            OxOperand::Const(OxConst::I64(_)) => Ok(Some(I64CompareLane::Plain)),
            OxOperand::Const(OxConst::Currency(_)) => Ok(Some(I64CompareLane::Currency)),
            OxOperand::Use(place) => match place_ty(self.program, self.func, *place)? {
                OxTy::LongLong => Ok(Some(I64CompareLane::Plain)),
                OxTy::Currency => Ok(Some(I64CompareLane::Currency)),
                _ => Ok(None),
            },
            _ => Ok(None),
        }
    }

    fn compare_float_lane(
        &self,
        operand: &OxOperand,
    ) -> Result<Option<FloatCompareLane>, JitError> {
        match operand {
            OxOperand::Const(OxConst::F32(_)) => Ok(Some(FloatCompareLane::Single)),
            OxOperand::Const(OxConst::F64(_)) => Ok(Some(FloatCompareLane::Double)),
            OxOperand::Const(OxConst::Date(_)) => Ok(Some(FloatCompareLane::Date)),
            OxOperand::Use(place) => match place_ty(self.program, self.func, *place)? {
                OxTy::Single => Ok(Some(FloatCompareLane::Single)),
                OxTy::Double => Ok(Some(FloatCompareLane::Double)),
                OxTy::Date => Ok(Some(FloatCompareLane::Date)),
                _ => Ok(None),
            },
            _ => Ok(None),
        }
    }

    fn numeric_coerce_requires_variant_helper(
        &self,
        operand: &OxOperand,
        target: NumericCoerceTarget,
    ) -> Result<bool, JitError> {
        let direct = match target {
            NumericCoerceTarget::Long => match operand {
                OxOperand::Const(OxConst::I32(_))
                | OxOperand::Const(OxConst::I16(_))
                | OxOperand::Const(OxConst::Bool(_)) => true,
                OxOperand::Use(place) => matches!(
                    place_ty(self.program, self.func, *place)?,
                    OxTy::Long | OxTy::Byte | OxTy::Integer
                ),
                _ => false,
            },
            NumericCoerceTarget::LongLong => match operand {
                OxOperand::Const(OxConst::I64(_))
                | OxOperand::Const(OxConst::I32(_))
                | OxOperand::Const(OxConst::I16(_))
                | OxOperand::Const(OxConst::Bool(_)) => true,
                OxOperand::Use(place) => matches!(
                    place_ty(self.program, self.func, *place)?,
                    OxTy::LongLong | OxTy::Long | OxTy::Byte | OxTy::Integer
                ),
                _ => false,
            },
            NumericCoerceTarget::Currency => match operand {
                OxOperand::Const(OxConst::Currency(_))
                | OxOperand::Const(OxConst::I32(_))
                | OxOperand::Const(OxConst::I16(_)) => true,
                OxOperand::Use(place) => {
                    matches!(place_ty(self.program, self.func, *place)?, OxTy::Currency)
                }
                _ => false,
            },
            NumericCoerceTarget::Double => match operand {
                OxOperand::Const(OxConst::F64(_))
                | OxOperand::Const(OxConst::I32(_))
                | OxOperand::Const(OxConst::I16(_)) => true,
                OxOperand::Use(place) => {
                    matches!(place_ty(self.program, self.func, *place)?, OxTy::Double)
                }
                _ => false,
            },
            NumericCoerceTarget::Date => match operand {
                OxOperand::Const(OxConst::Date(_))
                | OxOperand::Const(OxConst::I32(_))
                | OxOperand::Const(OxConst::I16(_)) => true,
                OxOperand::Use(place) => {
                    matches!(place_ty(self.program, self.func, *place)?, OxTy::Date)
                }
                _ => false,
            },
            NumericCoerceTarget::Single => match operand {
                OxOperand::Const(OxConst::F32(_))
                | OxOperand::Const(OxConst::F64(_))
                | OxOperand::Const(OxConst::I32(_))
                | OxOperand::Const(OxConst::I16(_)) => true,
                OxOperand::Use(place) => {
                    matches!(place_ty(self.program, self.func, *place)?, OxTy::Single)
                }
                _ => false,
            },
            NumericCoerceTarget::Byte | NumericCoerceTarget::Integer => match operand {
                OxOperand::Const(OxConst::I32(_))
                | OxOperand::Const(OxConst::I16(_))
                | OxOperand::Const(OxConst::Bool(_)) => true,
                OxOperand::Use(place) => matches!(
                    place_ty(self.program, self.func, *place)?,
                    OxTy::Long | OxTy::Byte | OxTy::Integer
                ),
                _ => false,
            },
            NumericCoerceTarget::Boolean => match operand {
                OxOperand::Const(OxConst::Bool(_))
                | OxOperand::Const(OxConst::I32(_))
                | OxOperand::Const(OxConst::I16(_)) => true,
                OxOperand::Use(place) => {
                    matches!(place_ty(self.program, self.func, *place)?, OxTy::Bool)
                }
                _ => false,
            },
        };
        Ok(!direct)
    }

    fn operand_is_static_string_source(&self, operand: &OxOperand) -> Result<bool, JitError> {
        match operand {
            OxOperand::Const(OxConst::Str(_)) => Ok(true),
            OxOperand::Use(place) => Ok(matches!(
                place_ty(self.program, self.func, *place)?,
                OxTy::Str | OxTy::FixedStr(_)
            )),
            OxOperand::Const(_) => Ok(false),
        }
    }

    fn operand_is_variant_source(&self, operand: &OxOperand) -> Result<bool, JitError> {
        match operand {
            OxOperand::Use(place) => Ok(matches!(
                place_ty(self.program, self.func, *place)?,
                OxTy::Variant
            )),
            OxOperand::Const(_) => Ok(false),
        }
    }

    fn ensure_numeric_target_place(
        &self,
        place: OxPlace,
        target: NumericCoerceTarget,
    ) -> Result<(), JitError> {
        match target {
            NumericCoerceTarget::Byte => self.ensure_byte_place(place),
            NumericCoerceTarget::Integer => self.ensure_integer_place(place),
            NumericCoerceTarget::Long => self.ensure_long_place(place),
            NumericCoerceTarget::LongLong => self.ensure_longlong_place(place),
            NumericCoerceTarget::Single => self.ensure_single_place(place),
            NumericCoerceTarget::Double => self.ensure_double_place(place),
            NumericCoerceTarget::Currency => self.ensure_currency_place(place),
            NumericCoerceTarget::Date => self.ensure_date_place(place),
            NumericCoerceTarget::Boolean => self.ensure_bool_place(place),
        }
    }

    fn ensure_unbox_target_place(&self, place: OxPlace, target: &OxTy) -> Result<(), JitError> {
        match target {
            OxTy::Long => self.ensure_long_place(place),
            OxTy::LongLong => self.ensure_longlong_place(place),
            OxTy::Currency => self.ensure_currency_place(place),
            OxTy::Single => self.ensure_single_place(place),
            OxTy::Double => self.ensure_double_place(place),
            OxTy::Date => self.ensure_date_place(place),
            OxTy::Byte => self.ensure_byte_place(place),
            OxTy::Integer => self.ensure_integer_place(place),
            OxTy::Bool => self.ensure_bool_place(place),
            OxTy::Variant => self.ensure_variant_place(place),
            OxTy::ProcRef => self.ensure_proc_ref_place(place),
            OxTy::Decimal
            | OxTy::Str
            | OxTy::FixedStr(_)
            | OxTy::Object(_)
            | OxTy::Record(_)
            | OxTy::Array(_, _) => {
                let actual = place_ty(self.program, self.func, place)?;
                if actual == target {
                    Ok(())
                } else {
                    Err(JitError::unsupported(format!(
                        "JIT Unbox destination type mismatch: got {actual:?} for {target:?} at {place:?}"
                    )))
                }
            }
        }
    }

    fn lower_variant_operand(
        &self,
        builder: &mut FunctionBuilder<'_>,
        operand: &OxOperand,
    ) -> Result<LoweredVariantOperand, JitError> {
        let zero_i32 = builder.ins().iconst(types::I32, 0);
        let zero_i64 = builder.ins().iconst(types::I64, 0);
        let const_operand =
            |builder: &mut FunctionBuilder<'_>, kind: i32, value: i64| LoweredVariantOperand {
                kind: builder.ins().iconst(types::I32, i64::from(kind)),
                value: builder.ins().iconst(types::I64, value),
                area: zero_i32,
                index: zero_i32,
            };
        match operand {
            OxOperand::Use(place) => {
                let ty = place_ty(self.program, self.func, *place)?;
                if !is_m4_4_variant_descriptor_operand_ty(ty) {
                    return Err(JitError::unsupported(format!(
                        "M4-4 Variant arithmetic operand does not support {ty:?} at {place:?}"
                    )));
                }
                let (area, index) = place_addr(*place);
                Ok(LoweredVariantOperand {
                    kind: builder
                        .ins()
                        .iconst(types::I32, i64::from(JIT_VARIANT_OPERAND_PLACE)),
                    value: zero_i64,
                    area: builder.ins().iconst(types::I32, i64::from(area)),
                    index: builder.ins().iconst(types::I32, index as i64),
                })
            }
            OxOperand::Const(OxConst::Empty) => {
                Ok(const_operand(builder, JIT_VARIANT_OPERAND_EMPTY, 0))
            }
            OxOperand::Const(OxConst::Null) => {
                Ok(const_operand(builder, JIT_VARIANT_OPERAND_NULL, 0))
            }
            OxOperand::Const(OxConst::Nothing) => {
                Ok(const_operand(builder, JIT_VARIANT_OPERAND_NOTHING, 0))
            }
            OxOperand::Const(OxConst::Bool(value)) => Ok(const_operand(
                builder,
                JIT_VARIANT_OPERAND_BOOL,
                i64::from(u8::from(*value)),
            )),
            OxOperand::Const(OxConst::I16(value)) => Ok(const_operand(
                builder,
                JIT_VARIANT_OPERAND_I16,
                i64::from(*value),
            )),
            OxOperand::Const(OxConst::I32(value)) => Ok(const_operand(
                builder,
                JIT_VARIANT_OPERAND_I32,
                i64::from(*value),
            )),
            OxOperand::Const(OxConst::I64(value)) => {
                Ok(const_operand(builder, JIT_VARIANT_OPERAND_I64, *value))
            }
            OxOperand::Const(OxConst::F32(bits)) => Ok(const_operand(
                builder,
                JIT_VARIANT_OPERAND_F32,
                i64::from(*bits),
            )),
            OxOperand::Const(OxConst::F64(bits)) => Ok(const_operand(
                builder,
                JIT_VARIANT_OPERAND_F64,
                *bits as i64,
            )),
            OxOperand::Const(OxConst::Currency(value)) => {
                Ok(const_operand(builder, JIT_VARIANT_OPERAND_CURRENCY, *value))
            }
            OxOperand::Const(OxConst::Date(bits)) => Ok(const_operand(
                builder,
                JIT_VARIANT_OPERAND_DATE,
                *bits as i64,
            )),
            OxOperand::Const(OxConst::Str(value)) => {
                let len = i32::try_from(value.len()).map_err(|_| {
                    JitError::unsupported("M4-4 string literal operand length exceeds i32")
                })?;
                let ptr = i64::try_from(value.as_ptr() as usize).map_err(|_| {
                    JitError::unsupported("M4-4 string literal operand pointer exceeds i64")
                })?;
                Ok(LoweredVariantOperand {
                    kind: builder
                        .ins()
                        .iconst(types::I32, i64::from(JIT_VARIANT_OPERAND_STR_UTF8)),
                    value: builder.ins().iconst(types::I64, ptr),
                    area: zero_i32,
                    index: builder.ins().iconst(types::I32, i64::from(len)),
                })
            }
        }
    }

    fn emit_variant_operand_descriptors(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &JITModule,
        operands: &[LoweredVariantOperand],
    ) -> Result<Value, JitError> {
        let ptr_ty = module.target_config().pointer_type();
        if operands.is_empty() {
            return Ok(builder.ins().iconst(ptr_ty, 0));
        }
        let byte_size = operands
            .len()
            .checked_mul(JIT_CALL_ARG_DESC_SIZE as usize)
            .and_then(|size| i32::try_from(size).ok())
            .and_then(|size| u32::try_from(size).ok())
            .ok_or_else(|| JitError::unsupported("M4-4 Variant operand stack is too large"))?;
        let slot = builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            byte_size,
            3,
        ));
        for (index, operand) in operands.iter().enumerate() {
            let base = i32::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(JIT_CALL_ARG_DESC_SIZE as i32))
                .ok_or_else(|| JitError::unsupported("M4-4 Variant operand offset overflow"))?;
            builder.ins().stack_store(operand.kind, slot, base);
            builder
                .ins()
                .stack_store(operand.value, slot, base + JIT_CALL_ARG_VALUE_OFFSET);
            builder
                .ins()
                .stack_store(operand.area, slot, base + JIT_CALL_ARG_AREA_OFFSET);
            builder
                .ins()
                .stack_store(operand.index, slot, base + JIT_CALL_ARG_INDEX_OFFSET);
        }
        Ok(builder.ins().stack_addr(ptr_ty, slot, 0))
    }

    fn emit_i32_stack_values(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &JITModule,
        values: &[Value],
        label: &'static str,
    ) -> Result<Value, JitError> {
        let ptr_ty = module.target_config().pointer_type();
        if values.is_empty() {
            return Ok(builder.ins().iconst(ptr_ty, 0));
        }
        let byte_size = values
            .len()
            .checked_mul(JIT_I32_STACK_ELEM_SIZE as usize)
            .and_then(|size| i32::try_from(size).ok())
            .and_then(|size| u32::try_from(size).ok())
            .ok_or_else(|| JitError::unsupported(format!("M4-4 {label} stack is too large")))?;
        let slot = builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            byte_size,
            2,
        ));
        for (index, value) in values.iter().enumerate() {
            let base = i32::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(JIT_I32_STACK_ELEM_SIZE as i32))
                .ok_or_else(|| JitError::unsupported(format!("M4-4 {label} offset overflow")))?;
            builder.ins().stack_store(*value, slot, base);
        }
        Ok(builder.ins().stack_addr(ptr_ty, slot, 0))
    }

    fn emit_param_array_alias_descriptors(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &JITModule,
        aliases: &[Option<OxPlace>],
    ) -> Result<Value, JitError> {
        let ptr_ty = module.target_config().pointer_type();
        if aliases.iter().all(Option::is_none) {
            return Ok(builder.ins().iconst(ptr_ty, 0));
        }
        let byte_size = aliases
            .len()
            .checked_mul(JIT_SLOT_ALIAS_DESC_SIZE as usize)
            .and_then(|size| i32::try_from(size).ok())
            .and_then(|size| u32::try_from(size).ok())
            .ok_or_else(|| JitError::unsupported("M4-4 ParamArray alias stack is too large"))?;
        let slot = builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            byte_size,
            3,
        ));
        for (index, alias) in aliases.iter().enumerate() {
            let base = i32::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(JIT_SLOT_ALIAS_DESC_SIZE as i32))
                .ok_or_else(|| JitError::unsupported("M4-4 ParamArray alias offset overflow"))?;
            let (area, slot_index) = match alias {
                Some(place) => {
                    let (area, index) = place_addr(*place);
                    (
                        builder.ins().iconst(types::I32, i64::from(area)),
                        builder.ins().iconst(types::I32, index as i64),
                    )
                }
                None => (
                    builder.ins().iconst(types::I32, -1),
                    builder.ins().iconst(types::I32, -1),
                ),
            };
            builder.ins().stack_store(area, slot, base);
            builder
                .ins()
                .stack_store(slot_index, slot, base + JIT_SLOT_ALIAS_INDEX_OFFSET);
        }
        Ok(builder.ins().stack_addr(ptr_ty, slot, 0))
    }

    fn lower_extern_arg(
        &self,
        builder: &mut FunctionBuilder<'_>,
        arg: &OxArg,
    ) -> Result<LoweredVariantOperand, JitError> {
        let zero_i32 = builder.ins().iconst(types::I32, 0);
        let zero_i64 = builder.ins().iconst(types::I64, 0);
        match arg {
            OxArg::ByVal(arg) => self.lower_variant_operand(builder, arg),
            OxArg::ByRef(place) => self.lower_variant_operand(builder, &OxOperand::Use(*place)),
            OxArg::Omitted => Ok(LoweredVariantOperand {
                kind: builder
                    .ins()
                    .iconst(types::I32, i64::from(JIT_VARIANT_OPERAND_EMPTY)),
                value: zero_i64,
                area: zero_i32,
                index: zero_i32,
            }),
        }
    }

    fn lower_extern_args(
        &self,
        builder: &mut FunctionBuilder<'_>,
        args: &[OxArg],
    ) -> Result<Vec<LoweredVariantOperand>, JitError> {
        let mut lowered = Vec::with_capacity(args.len());
        for arg in args {
            lowered.push(self.lower_extern_arg(builder, arg)?);
        }
        Ok(lowered)
    }

    fn lower_call_native_arg(
        &self,
        builder: &mut FunctionBuilder<'_>,
        arg: &OxCallArg,
    ) -> Result<LoweredVariantOperand, JitError> {
        match arg {
            OxCallArg::Operand(operand) => self.lower_variant_operand(builder, operand),
            OxCallArg::Const(value) => {
                self.lower_variant_operand(builder, &OxOperand::Const(OxConst::I32(*value)))
            }
            other => Err(JitError::unsupported(format!(
                "M4-4 CallNative built-in subset lowers only ordinary operands and compiler-inserted integer constants, got {other:?}"
            ))),
        }
    }

    fn lower_call_native_args(
        &self,
        builder: &mut FunctionBuilder<'_>,
        args: &[OxCallArg],
    ) -> Result<Vec<LoweredVariantOperand>, JitError> {
        let mut lowered = Vec::with_capacity(args.len());
        for arg in args {
            lowered.push(self.lower_call_native_arg(builder, arg)?);
        }
        Ok(lowered)
    }

    fn lower_project_call_arg<'b>(
        &self,
        builder: &mut FunctionBuilder<'_>,
        arg: &'b OxCallArg,
    ) -> Result<(LoweredCallArg, Option<&'b str>), JitError> {
        let zero_i32 = builder.ins().iconst(types::I32, 0);
        let zero_i64 = builder.ins().iconst(types::I64, 0);
        match arg {
            OxCallArg::Operand(operand) => {
                let operand = self.lower_variant_operand(builder, operand)?;
                Ok((
                    LoweredCallArg {
                        kind: builder
                            .ins()
                            .iconst(types::I32, i64::from(JIT_CALL_ARG_BYVAL_VARIANT)),
                        aux: operand.kind,
                        value: operand.value,
                        area: operand.area,
                        index: operand.index,
                    },
                    None,
                ))
            }
            OxCallArg::Named { name, value } => {
                let operand = self.lower_variant_operand(builder, value)?;
                Ok((
                    LoweredCallArg {
                        kind: builder
                            .ins()
                            .iconst(types::I32, i64::from(JIT_CALL_ARG_BYVAL_VARIANT)),
                        aux: operand.kind,
                        value: operand.value,
                        area: operand.area,
                        index: operand.index,
                    },
                    Some(name.as_str()),
                ))
            }
            OxCallArg::ByRef(place) => {
                let (area, index) = place_addr(*place);
                Ok((
                    LoweredCallArg {
                        kind: builder
                            .ins()
                            .iconst(types::I32, i64::from(JIT_CALL_ARG_BYREF_ALIAS)),
                        aux: zero_i32,
                        value: zero_i64,
                        area: builder.ins().iconst(types::I32, i64::from(area)),
                        index: builder.ins().iconst(types::I32, index as i64),
                    },
                    None,
                ))
            }
            OxCallArg::Omitted => Ok((
                LoweredCallArg {
                    kind: builder
                        .ins()
                        .iconst(types::I32, i64::from(JIT_CALL_ARG_OMITTED)),
                    aux: zero_i32,
                    value: zero_i64,
                    area: zero_i32,
                    index: zero_i32,
                },
                None,
            )),
            OxCallArg::Const(_) => Err(JitError::unsupported(
                "JIT project ComCallLate does not support compiler-inserted Const call arguments",
            )),
        }
    }

    fn lower_project_call_args<'b>(
        &self,
        builder: &mut FunctionBuilder<'_>,
        args: &'b [OxCallArg],
    ) -> Result<(Vec<LoweredCallArg>, Vec<Option<&'b str>>), JitError> {
        let mut lowered = Vec::with_capacity(args.len());
        let mut names = Vec::with_capacity(args.len());
        for arg in args {
            let (lowered_arg, name) = self.lower_project_call_arg(builder, arg)?;
            lowered.push(lowered_arg);
            names.push(name);
        }
        Ok((lowered, names))
    }

    fn emit_variant_arith_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        op: ArithOp,
        mode: NumericMode,
        operands: BinaryVariantOperands<'_>,
    ) -> Result<(), JitError> {
        let op = raw_arith_op(op).ok_or_else(|| {
            JitError::unsupported(format!("M4-4 Variant arithmetic does not lower {op:?}"))
        })?;
        let mode = raw_numeric_mode(mode)?;
        self.emit_variant_arith_raw_slot_call(builder, module, dst, op, mode, operands)
    }

    fn emit_variant_arith_raw_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        op: u32,
        mode: u32,
        operands: BinaryVariantOperands<'_>,
    ) -> Result<(), JitError> {
        let operands = [
            self.lower_variant_operand(builder, operands.lhs)?,
            self.lower_variant_operand(builder, operands.rhs)?,
        ];
        let operands_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let op = builder.ins().iconst(types::I32, i64::from(op));
        let mode = builder.ins().iconst(types::I32, i64::from(mode));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.arith_v_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, op, mode, operands_ptr, area, index],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn ensure_concat_destination_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if !matches!(ty, OxTy::Str | OxTy::FixedStr(_) | OxTy::Variant) {
            return Err(JitError::unsupported(format!(
                "M4-4 Concat lowering supports only String/FixedString/Variant destinations, got {ty:?} at {place:?}"
            )));
        }
        Ok(())
    }

    fn emit_variant_concat_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        lhs: &OxOperand,
        rhs: &OxOperand,
    ) -> Result<(), JitError> {
        let operands = [
            self.lower_variant_operand(builder, lhs)?,
            self.lower_variant_operand(builder, rhs)?,
        ];
        let operands_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.concat_v_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, operands_ptr, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_variant_neg_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        mode: NumericMode,
        src: &OxOperand,
    ) -> Result<(), JitError> {
        let mode = raw_numeric_mode(mode)?;
        let operands = [self.lower_variant_operand(builder, src)?];
        let operands_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let mode = builder.ins().iconst(types::I32, i64::from(mode));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.neg_v_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, mode, operands_ptr, area, index],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_variant_compare_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        op: CmpOp,
        mode: StringCompareMode,
        operands: BinaryVariantOperands<'_>,
    ) -> Result<(), JitError> {
        let operands = [
            self.lower_variant_operand(builder, operands.lhs)?,
            self.lower_variant_operand(builder, operands.rhs)?,
        ];
        let operands_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let op = builder
            .ins()
            .iconst(types::I32, i64::from(raw_compare_op(op)));
        let mode = builder
            .ins()
            .iconst(types::I32, i64::from(raw_string_compare_mode(mode)));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.compare_v_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, op, mode, operands_ptr, area, index],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_compare_object_is_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        lhs: &OxOperand,
        rhs: &OxOperand,
    ) -> Result<(), JitError> {
        self.ensure_bool_place(dst)?;
        let operands = [
            self.lower_variant_operand(builder, lhs)?,
            self.lower_variant_operand(builder, rhs)?,
        ];
        let operands_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.compare_object_is_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, operands_ptr, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn lower_type_of_is(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        object: &OxOperand,
        type_name: &str,
    ) -> Result<(), JitError> {
        self.ensure_bool_place(dst)?;
        if matches!(object, OxOperand::Const(OxConst::Nothing)) {
            let false_value = builder.ins().iconst(types::I32, 0);
            return self.emit_store_bool(builder, module, dst, false_value);
        }
        if !is_project_object_static_ty(&operand_static_ty(self.program, self.func, object)?)
            && self.programs.len() == 1
        {
            return Err(JitError::unsupported(format!(
                "JIT project object/class instruction TypeOfIs is unsupported for {type_name}: currently supports only statically typed active-project class/interface receivers"
            )));
        }
        let operand = self.lower_variant_operand(builder, object)?;
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &[operand])?;
        let ptr_ty = module.target_config().pointer_type();
        let name_ptr = builder.ins().iconst(ptr_ty, type_name.as_ptr() as i64);
        let name_len = i32::try_from(type_name.len())
            .map_err(|_| JitError::unsupported("JIT TypeOf type name is too long"))?;
        let name_len = builder.ins().iconst(types::I32, i64::from(name_len));
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.type_of_is_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                operand_ptr,
                name_ptr,
                name_len,
                area,
                index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_variant_logical_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        op: LogicalOp,
        lhs: &OxOperand,
        rhs: &OxOperand,
    ) -> Result<(), JitError> {
        let op = raw_logical_op(op);
        let operands = [
            self.lower_variant_operand(builder, lhs)?,
            self.lower_variant_operand(builder, rhs)?,
        ];
        let operands_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let op = builder.ins().iconst(types::I32, i64::from(op));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.logical_v_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, op, operands_ptr, area, index],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_variant_not_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        src: &OxOperand,
    ) -> Result<(), JitError> {
        let operands = [self.lower_variant_operand(builder, src)?];
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.not_v_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, operand_ptr, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_variant_truthy_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        src: &OxOperand,
    ) -> Result<(), JitError> {
        let operands = [self.lower_variant_operand(builder, src)?];
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.truthy_v_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, operand_ptr, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_variant_changed_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        current: &OxOperand,
        original: &OxOperand,
    ) -> Result<(), JitError> {
        let operands = [
            self.lower_variant_operand(builder, current)?,
            self.lower_variant_operand(builder, original)?,
        ];
        let operands_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.variant_changed_slot);
        let call = builder
            .ins()
            .call(callee, &[self.run, operands_ptr, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_variant_numeric_coerce_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        target: NumericCoerceTarget,
        src: &OxOperand,
    ) -> Result<(), JitError> {
        let target = raw_numeric_target(target)?;
        let operands = [self.lower_variant_operand(builder, src)?];
        let operands_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let target = builder.ins().iconst(types::I32, i64::from(target));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.coerce_numeric_v_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, target, operands_ptr, area, index],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_variant_string_coerce_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        src: &OxOperand,
    ) -> Result<(), JitError> {
        let operands = [self.lower_variant_operand(builder, src)?];
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.coerce_string_v_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, operand_ptr, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_variant_fixed_string_coerce_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        len: u32,
        src: &OxOperand,
    ) -> Result<(), JitError> {
        let operands = [self.lower_variant_operand(builder, src)?];
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let len = builder.ins().iconst(types::I32, i64::from(len));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.coerce_fixed_string_v_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, len, operand_ptr, area, index],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_store_variant(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        value: &OxOperand,
    ) -> Result<(), JitError> {
        let operands = [self.lower_variant_operand(builder, value)?];
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.store_variant);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, operand_ptr, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_as_new_register(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        place: OxPlace,
        binding: &OxAsNew,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(place)?;
        match binding {
            OxAsNew::ProjectClass { class } => {
                let class = i32::try_from(class.0)
                    .map_err(|_| JitError::unsupported("JIT AsNew class index is too large"))?;
                let (area, index) = place_addr(place);
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                let class = builder.ins().iconst(types::I32, i64::from(class));
                let callee = self.import(builder, module, self.imports.as_new_project_class_slot);
                let call = builder.ins().call(callee, &[self.run, area, index, class]);
                let status = builder.inst_results(call)[0];
                self.return_if_not_ok(builder, status);
                Ok(())
            }
            OxAsNew::ExternClass { import } if self.is_vba_collection_import(*import)? => {
                let (area, index) = place_addr(place);
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                let program = i32::try_from(self.program_index).map_err(|_| {
                    JitError::unsupported("JIT AsNew Collection program index is too large")
                })?;
                let program = builder.ins().iconst(types::I32, i64::from(program));
                let callee = self.import(builder, module, self.imports.as_new_collection_slot);
                let call = builder
                    .ins()
                    .call(callee, &[self.run, area, index, program]);
                let status = builder.inst_results(call)[0];
                self.return_if_not_ok(builder, status);
                Ok(())
            }
            OxAsNew::ExternClass { .. } | OxAsNew::ComClass { .. } => Err(JitError::unsupported(
                "JIT AsNew supports active-project classes and built-in VBA.Collection only",
            )),
        }
    }

    fn emit_new_object_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        class_index: usize,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        let class_index = i32::try_from(class_index)
            .map_err(|_| JitError::unsupported("JIT NewObject class index is too large"))?;
        let (area, index) = place_addr(dst);
        let program = i32::try_from(self.program_index)
            .map_err(|_| JitError::unsupported("JIT program index is too large"))?;
        let program = builder.ins().iconst(types::I32, i64::from(program));
        let class = builder.ins().iconst(types::I32, i64::from(class_index));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.new_object_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, program, class, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_new_extern_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        import: oxvba_oxir::ImportId,
    ) -> Result<(), JitError> {
        if self.is_vba_collection_import(import)? {
            self.ensure_variant_carrier_place(dst)?;
            let (area, index) = place_addr(dst);
            let area = builder.ins().iconst(types::I32, i64::from(area));
            let index = builder.ins().iconst(types::I32, index as i64);
            let program = i32::try_from(self.program_index).map_err(|_| {
                JitError::unsupported("JIT NewExtern Collection program index is too large")
            })?;
            let program = builder.ins().iconst(types::I32, i64::from(program));
            let callee = self.import(builder, module, self.imports.new_collection_slot);
            let call = builder
                .ins()
                .call(callee, &[self.state, self.run, area, index, program]);
            let status = builder.inst_results(call)[0];
            self.return_if_not_ok(builder, status);
            return Ok(());
        }
        let (program_index, class_index) = self.resolve_cross_project_class(import)?;
        self.emit_project_class_to_slot(
            builder,
            module,
            dst,
            ProjectClassTarget {
                program_index,
                class_index,
            },
            self.imports.new_object_slot,
            "NewExtern",
        )
    }

    fn emit_predeclared_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        class_index: usize,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        let class_index = i32::try_from(class_index)
            .map_err(|_| JitError::unsupported("JIT Predeclared class index is too large"))?;
        let (area, index) = place_addr(dst);
        let program = i32::try_from(self.program_index)
            .map_err(|_| JitError::unsupported("JIT program index is too large"))?;
        let program = builder.ins().iconst(types::I32, i64::from(program));
        let class = builder.ins().iconst(types::I32, i64::from(class_index));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.predeclared_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, program, class, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_predeclared_extern_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        import: oxvba_oxir::ImportId,
    ) -> Result<(), JitError> {
        let (program_index, class_index) = self.resolve_cross_project_class(import)?;
        self.emit_project_class_to_slot(
            builder,
            module,
            dst,
            ProjectClassTarget {
                program_index,
                class_index,
            },
            self.imports.predeclared_slot,
            "PredeclaredExtern",
        )
    }

    fn emit_project_class_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        target: ProjectClassTarget,
        callee_id: ClifFuncId,
        label: &'static str,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        let program_index = i32::try_from(target.program_index).map_err(|_| {
            JitError::unsupported(format!("JIT {label} program index is too large"))
        })?;
        let class_index = i32::try_from(target.class_index)
            .map_err(|_| JitError::unsupported(format!("JIT {label} class index is too large")))?;
        let (area, index) = place_addr(dst);
        let program = builder.ins().iconst(types::I32, i64::from(program_index));
        let class = builder.ins().iconst(types::I32, i64::from(class_index));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, callee_id);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, program, class, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_predeclared_set(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        class_index: usize,
        value: &OxOperand,
    ) -> Result<(), JitError> {
        let class_index = i32::try_from(class_index)
            .map_err(|_| JitError::unsupported("JIT PredeclaredSet class index is too large"))?;
        let operand = self.lower_variant_operand(builder, value)?;
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &[operand])?;
        let program = i32::try_from(self.program_index)
            .map_err(|_| JitError::unsupported("JIT program index is too large"))?;
        let program = builder.ins().iconst(types::I32, i64::from(program));
        let class = builder.ins().iconst(types::I32, i64::from(class_index));
        let callee = self.import(builder, module, self.imports.predeclared_set);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, program, class, operand_ptr]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_predeclared_extern_set(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        import: oxvba_oxir::ImportId,
        value: &OxOperand,
    ) -> Result<(), JitError> {
        let (program_index, class_index) = self.resolve_cross_project_class(import)?;
        let program_index = i32::try_from(program_index).map_err(|_| {
            JitError::unsupported("JIT PredeclaredExternSet program index is too large")
        })?;
        let class_index = i32::try_from(class_index).map_err(|_| {
            JitError::unsupported("JIT PredeclaredExternSet class index is too large")
        })?;
        let operand = self.lower_variant_operand(builder, value)?;
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &[operand])?;
        let program = builder.ins().iconst(types::I32, i64::from(program_index));
        let class = builder.ins().iconst(types::I32, i64::from(class_index));
        let callee = self.import(builder, module, self.imports.predeclared_set);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, program, class, operand_ptr]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn is_vba_collection_import(&self, import: oxvba_oxir::ImportId) -> Result<bool, JitError> {
        let imp = self.program.imports.get(import.0).ok_or_else(|| {
            JitError::Compile(format!("NewExtern import {} out of range", import.0))
        })?;
        if !imp.unit.eq_ignore_ascii_case("VBA") {
            return Ok(false);
        }
        let lib = vba_library_bundle();
        let Some(export) = lib
            .exports
            .iter()
            .find(|export| export.token.matches(&imp.token))
        else {
            return Ok(false);
        };
        let ExportTarget::Class(class_index) = export.target else {
            return Ok(false);
        };
        Ok(lib
            .classes
            .get(class_index)
            .is_some_and(|class| class.name.eq_ignore_ascii_case("Collection")))
    }

    fn resolve_cross_project_class(
        &self,
        import: oxvba_oxir::ImportId,
    ) -> Result<(usize, usize), JitError> {
        let imp = self.program.imports.get(import.0).ok_or_else(|| {
            JitError::Compile(format!("NewExtern import {} out of range", import.0))
        })?;
        if imp.unit.eq_ignore_ascii_case("VBA") {
            return Err(JitError::unsupported(
                "JIT NewExtern for imported VBA/COM library classes remains unsupported; referenced-project classes are supported",
            ));
        }
        let program_index = self
            .programs
            .iter()
            .position(|program| program.unit_name.eq_ignore_ascii_case(&imp.unit))
            .ok_or_else(|| {
                JitError::unsupported(format!(
                    "JIT cross-project class import names unresolved unit '{}'",
                    imp.unit
                ))
            })?;
        let program = self.programs[program_index];
        let export = program
            .exports
            .iter()
            .find(|export| export.token.matches(&imp.token))
            .ok_or_else(|| {
                JitError::unsupported(format!(
                    "JIT cross-project class import {} has no matching export in '{}'",
                    import.0, imp.unit
                ))
            })?;
        let ExportTarget::Class(class_index) = export.target else {
            return Err(JitError::unsupported(
                "JIT cross-project class import resolved to a non-class export",
            ));
        };
        if class_index >= program.classes.len() {
            return Err(JitError::unsupported(format!(
                "JIT cross-project class import {} resolved to out-of-range class {}",
                import.0, class_index
            )));
        }
        Ok((program_index, class_index))
    }

    fn emit_field_get_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        object: &OxOperand,
        field: i32,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        let object = self.lower_variant_operand(builder, object)?;
        let object_ptr = self.emit_variant_operand_descriptors(builder, module, &[object])?;
        let (area, index) = place_addr(dst);
        let field = builder.ins().iconst(types::I32, i64::from(field));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.field_get_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, object_ptr, field, area, index],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_field_set(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        object: &OxOperand,
        field: i32,
        value: &OxOperand,
    ) -> Result<(), JitError> {
        let object = self.lower_variant_operand(builder, object)?;
        let value = self.lower_variant_operand(builder, value)?;
        let operands = [object, value];
        let operands_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let field = builder.ins().iconst(types::I32, i64::from(field));
        let callee = self.import(builder, module, self.imports.field_set_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, operands_ptr, field]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_withevents_get_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        owner: &OxOperand,
        binding: i32,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        let owner = self.lower_variant_operand(builder, owner)?;
        let owner_ptr = self.emit_variant_operand_descriptors(builder, module, &[owner])?;
        let (area, index) = place_addr(dst);
        let binding = builder.ins().iconst(types::I32, i64::from(binding));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.withevents_get_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, owner_ptr, binding, area, index],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_withevents_set_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        owner: &OxOperand,
        binding: i32,
        value: &OxOperand,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        let owner = self.lower_variant_operand(builder, owner)?;
        let value = self.lower_variant_operand(builder, value)?;
        let operands = [owner, value];
        let operands_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let binding = builder.ins().iconst(types::I32, i64::from(binding));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.withevents_set_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, operands_ptr, binding, area, index],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_withevents_clear_owner_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        owner: &OxOperand,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        let owner = self.lower_variant_operand(builder, owner)?;
        let owner_ptr = self.emit_variant_operand_descriptors(builder, module, &[owner])?;
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.withevents_clear_owner_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, owner_ptr, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_withevents_first_owner_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        source: &OxOperand,
        binding: i32,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        let source = self.lower_variant_operand(builder, source)?;
        let source_ptr = self.emit_variant_operand_descriptors(builder, module, &[source])?;
        let (area, index) = place_addr(dst);
        let binding = builder.ins().iconst(types::I32, i64::from(binding));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.withevents_first_owner_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, source_ptr, binding, area, index],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_withevents_next_owner_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.withevents_next_owner_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn lower_event_args(
        &self,
        builder: &mut FunctionBuilder<'_>,
        args: &[OxArg],
    ) -> Result<Vec<LoweredCallArg>, JitError> {
        let mut lowered = Vec::with_capacity(args.len());
        let zero_i32 = builder.ins().iconst(types::I32, 0);
        let zero_i64 = builder.ins().iconst(types::I64, 0);
        for arg in args {
            match arg {
                OxArg::ByVal(arg) => {
                    let operand = self.lower_variant_operand(builder, arg)?;
                    lowered.push(LoweredCallArg {
                        kind: builder
                            .ins()
                            .iconst(types::I32, i64::from(JIT_CALL_ARG_BYVAL_VARIANT)),
                        aux: operand.kind,
                        value: operand.value,
                        area: operand.area,
                        index: operand.index,
                    });
                }
                OxArg::ByRef(place) => {
                    let (area, index) = place_addr(*place);
                    lowered.push(LoweredCallArg {
                        kind: builder
                            .ins()
                            .iconst(types::I32, i64::from(JIT_CALL_ARG_BYREF_ALIAS)),
                        aux: zero_i32,
                        value: zero_i64,
                        area: builder.ins().iconst(types::I32, i64::from(area)),
                        index: builder.ins().iconst(types::I32, index as i64),
                    });
                }
                OxArg::Omitted => {
                    lowered.push(LoweredCallArg {
                        kind: builder
                            .ins()
                            .iconst(types::I32, i64::from(JIT_CALL_ARG_OMITTED)),
                        aux: zero_i32,
                        value: zero_i64,
                        area: zero_i32,
                        index: zero_i32,
                    });
                }
            }
        }
        Ok(lowered)
    }

    fn emit_raise_event(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        source: &OxOperand,
        event: i32,
        args: &[OxArg],
    ) -> Result<(), JitError> {
        let source = self.lower_variant_operand(builder, source)?;
        let source_ptr = self.emit_variant_operand_descriptors(builder, module, &[source])?;
        let lowered_args = self.lower_event_args(builder, args)?;
        let args_ptr = self.emit_call_arg_descriptors(builder, module, &lowered_args)?;
        let argc = i32::try_from(args.len())
            .map_err(|_| JitError::unsupported("JIT RaiseEvent argument count is too large"))?;
        let event = builder.ins().iconst(types::I32, i64::from(event));
        let argc = builder.ins().iconst(types::I32, i64::from(argc));
        let callee = self.import(builder, module, self.imports.raise_event);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, source_ptr, event, args_ptr, argc],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_project_member_call_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: Option<OxPlace>,
        inputs: ProjectMemberCallInputs<'_>,
    ) -> Result<(), JitError> {
        let ProjectMemberCallInputs {
            recv,
            name,
            default_member,
            invoke_kind,
            args,
        } = inputs;
        if let Some(dst) = dst {
            self.ensure_variant_carrier_place(dst)?;
        }
        let recv = self.lower_variant_operand(builder, recv)?;
        let recv_ptr = self.emit_variant_operand_descriptors(builder, module, &[recv])?;
        let (lowered_args, arg_names) = self.lower_project_call_args(builder, args)?;
        let args_ptr = self.emit_call_arg_descriptors(builder, module, &lowered_args)?;
        let arg_names_ptr = self.emit_call_arg_name_descriptors(builder, module, &arg_names)?;
        let argc = i32::try_from(args.len())
            .map_err(|_| JitError::unsupported("JIT project member argument count is too large"))?;
        let argc = builder.ins().iconst(types::I32, i64::from(argc));
        let ptr_ty = module.target_config().pointer_type();
        let member_name = if default_member { "" } else { name };
        let name_ptr = builder.ins().iconst(ptr_ty, member_name.as_ptr() as i64);
        let name_len = i32::try_from(member_name.len())
            .map_err(|_| JitError::unsupported("JIT project member name is too long"))?;
        let name_len = builder.ins().iconst(types::I32, i64::from(name_len));
        let invoke_kind = builder
            .ins()
            .iconst(types::I32, i64::from(raw_member_invoke_kind(invoke_kind)));
        let (area, index) = if let Some(dst) = dst {
            let (area, index) = place_addr(dst);
            (
                builder.ins().iconst(types::I32, i64::from(area)),
                builder.ins().iconst(types::I32, index as i64),
            )
        } else {
            (
                builder.ins().iconst(types::I32, -1),
                builder.ins().iconst(types::I32, -1),
            )
        };
        let callee = self.import(builder, module, self.imports.project_member_get_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                recv_ptr,
                name_ptr,
                name_len,
                invoke_kind,
                argc,
                args_ptr,
                arg_names_ptr,
                area,
                index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_call_by_name_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: Option<OxPlace>,
        inputs: CallByNameInputs<'_>,
    ) -> Result<(), JitError> {
        let CallByNameInputs {
            object,
            name,
            calltype,
            args,
        } = inputs;
        if let Some(dst) = dst {
            self.ensure_variant_carrier_place(dst)?;
        }
        let object = self.lower_variant_operand(builder, object)?;
        let name = self.lower_variant_operand(builder, name)?;
        let calltype = self.lower_variant_operand(builder, calltype)?;
        let operands_ptr =
            self.emit_variant_operand_descriptors(builder, module, &[object, name, calltype])?;
        let (lowered_args, arg_names) = self.lower_project_call_args(builder, args)?;
        let args_ptr = self.emit_call_arg_descriptors(builder, module, &lowered_args)?;
        let arg_names_ptr = self.emit_call_arg_name_descriptors(builder, module, &arg_names)?;
        let argc = i32::try_from(args.len())
            .map_err(|_| JitError::unsupported("JIT CallByName argument count is too large"))?;
        let argc = builder.ins().iconst(types::I32, i64::from(argc));
        let (area, index) = if let Some(dst) = dst {
            let (area, index) = place_addr(dst);
            (
                builder.ins().iconst(types::I32, i64::from(area)),
                builder.ins().iconst(types::I32, index as i64),
            )
        } else {
            (
                builder.ins().iconst(types::I32, -1),
                builder.ins().iconst(types::I32, -1),
            )
        };
        let callee = self.import(builder, module, self.imports.call_by_name_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                operands_ptr,
                argc,
                args_ptr,
                arg_names_ptr,
                area,
                index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_project_type_name_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        object: &OxOperand,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        let operand = self.lower_variant_operand(builder, object)?;
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &[operand])?;
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.project_type_name_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, operand_ptr, area, index]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn should_route_project_type_name(&self, object: &OxOperand) -> Result<bool, JitError> {
        let ty = operand_static_ty(self.program, self.func, object)?;
        Ok(is_project_object_static_ty(&ty)
            || (self.programs.len() > 1 && matches!(ty, OxTy::Object(_))))
    }

    fn emit_validate_assignment(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        src: &OxOperand,
        intent: AssignmentIntent,
        target_kind: AssignmentTargetKind,
        target_type_name: &str,
    ) -> Result<(), JitError> {
        let operands = [self.lower_variant_operand(builder, src)?];
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let intent = builder
            .ins()
            .iconst(types::I32, i64::from(raw_assignment_intent(intent)));
        let target_kind = builder.ins().iconst(
            types::I32,
            i64::from(raw_assignment_target_kind(target_kind)),
        );
        let ptr_ty = module.target_config().pointer_type();
        let target_type_ptr = if target_type_name.is_empty() {
            builder.ins().iconst(ptr_ty, 0)
        } else {
            builder
                .ins()
                .iconst(ptr_ty, target_type_name.as_ptr() as i64)
        };
        let target_type_len = i32::try_from(target_type_name.len())
            .map_err(|_| JitError::unsupported("JIT assignment target type name is too long"))?;
        let target_type_len = builder.ins().iconst(types::I32, i64::from(target_type_len));
        let callee = self.import(builder, module, self.imports.validate_assignment);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                operand_ptr,
                intent,
                target_kind,
                target_type_ptr,
                target_type_len,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_new_record_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        fields: &[ArrayElementType],
    ) -> Result<(), JitError> {
        let dst_ty = place_ty(self.program, self.func, dst)?;
        if !matches!(dst_ty, OxTy::Variant | OxTy::Record(_)) {
            return Err(JitError::unsupported(format!(
                "M4-4 NewRecord lowers only Variant/Record destinations, got {dst_ty:?} at {dst:?}"
            )));
        }
        let fields_len = i32::try_from(fields.len())
            .map_err(|_| JitError::unsupported("M4-4 NewRecord field count exceeds i32"))?;
        let ptr_ty = module.target_config().pointer_type();
        let fields_ptr = if fields.is_empty() {
            0
        } else {
            i64::try_from(fields.as_ptr() as usize)
                .map_err(|_| JitError::unsupported("M4-4 NewRecord fields pointer exceeds i64"))?
        };
        let fields_ptr = builder.ins().iconst(ptr_ty, fields_ptr);
        let fields_len = builder.ins().iconst(types::I32, i64::from(fields_len));
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.new_record_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, fields_ptr, fields_len, area, index],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_record_get_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        record: &OxOperand,
        index: usize,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        let index = i32::try_from(index)
            .map_err(|_| JitError::unsupported("M4-4 RecordGet field index exceeds i32"))?;
        let record = self.lower_variant_operand(builder, record)?;
        let record_ptr = self.emit_variant_operand_descriptors(builder, module, &[record])?;
        let (area, slot) = place_addr(dst);
        let index = builder.ins().iconst(types::I32, i64::from(index));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let slot = builder.ins().iconst(types::I32, i64::from(slot));
        let callee = self.import(builder, module, self.imports.record_get_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, record_ptr, index, area, slot],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_record_set(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        record: OxPlace,
        index: usize,
        value: &OxOperand,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(record)?;
        let index = i32::try_from(index)
            .map_err(|_| JitError::unsupported("M4-4 RecordSet field index exceeds i32"))?;
        let value = self.lower_variant_operand(builder, value)?;
        let value_ptr = self.emit_variant_operand_descriptors(builder, module, &[value])?;
        let (area, slot) = place_addr(record);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let slot = builder.ins().iconst(types::I32, i64::from(slot));
        let index = builder.ins().iconst(types::I32, i64::from(index));
        let callee = self.import(builder, module, self.imports.record_set_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, area, slot, index, value_ptr],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_record_lset(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        record: OxPlace,
        value: &OxOperand,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(record)?;
        let value = self.lower_variant_operand(builder, value)?;
        let value_ptr = self.emit_variant_operand_descriptors(builder, module, &[value])?;
        let (area, slot) = place_addr(record);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let slot = builder.ins().iconst(types::I32, slot as i64);
        let callee = self.import(builder, module, self.imports.record_lset_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, area, slot, value_ptr]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_record_array_get_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        record: &OxOperand,
        index: usize,
        indices: &[OxOperand],
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        if indices.is_empty() {
            return Err(JitError::unsupported(
                "M4-7 RecordArrayGet lowering requires at least one subscript",
            ));
        }
        let index = i32::try_from(index)
            .map_err(|_| JitError::unsupported("M4-7 RecordArrayGet field index exceeds i32"))?;
        let record = self.lower_variant_operand(builder, record)?;
        let record_ptr = self.emit_variant_operand_descriptors(builder, module, &[record])?;
        let indices_ptr =
            self.emit_lowered_i32_indices(builder, module, indices, "RecordArrayGet subscript")?;
        let dimensions = i32::try_from(indices.len()).map_err(|_| {
            JitError::unsupported("M4-7 RecordArrayGet subscript count is too large")
        })?;
        let dimensions = builder.ins().iconst(types::I32, i64::from(dimensions));
        let (dst_area, dst_slot) = place_addr(dst);
        let dst_area = builder.ins().iconst(types::I32, i64::from(dst_area));
        let dst_slot = builder.ins().iconst(types::I32, dst_slot as i64);
        let index = builder.ins().iconst(types::I32, i64::from(index));
        let callee = self.import(builder, module, self.imports.record_array_get_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                record_ptr,
                index,
                indices_ptr,
                dimensions,
                dst_area,
                dst_slot,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_record_array_set(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        record: OxPlace,
        index: usize,
        indices: &[OxOperand],
        value: &OxOperand,
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(record)?;
        if indices.is_empty() {
            return Err(JitError::unsupported(
                "M4-7 RecordArraySet lowering requires at least one subscript",
            ));
        }
        let index = i32::try_from(index)
            .map_err(|_| JitError::unsupported("M4-7 RecordArraySet field index exceeds i32"))?;
        let indices_ptr =
            self.emit_lowered_i32_indices(builder, module, indices, "RecordArraySet subscript")?;
        let dimensions = i32::try_from(indices.len()).map_err(|_| {
            JitError::unsupported("M4-7 RecordArraySet subscript count is too large")
        })?;
        let dimensions = builder.ins().iconst(types::I32, i64::from(dimensions));
        let value = self.lower_variant_operand(builder, value)?;
        let value_ptr = self.emit_variant_operand_descriptors(builder, module, &[value])?;
        let (area, slot) = place_addr(record);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let slot = builder.ins().iconst(types::I32, slot as i64);
        let index = builder.ins().iconst(types::I32, i64::from(index));
        let callee = self.import(builder, module, self.imports.record_array_set_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                area,
                slot,
                index,
                indices_ptr,
                dimensions,
                value_ptr,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_field_array_get_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        object: &OxOperand,
        field: i32,
        indices: &[OxOperand],
    ) -> Result<(), JitError> {
        self.ensure_variant_carrier_place(dst)?;
        if indices.is_empty() {
            return Err(JitError::unsupported(
                "M4-8 FieldArrayGet lowering requires at least one subscript",
            ));
        }
        let object = self.lower_variant_operand(builder, object)?;
        let object_ptr = self.emit_variant_operand_descriptors(builder, module, &[object])?;
        let indices_ptr =
            self.emit_lowered_i32_indices(builder, module, indices, "FieldArrayGet subscript")?;
        let dimensions = i32::try_from(indices.len()).map_err(|_| {
            JitError::unsupported("M4-8 FieldArrayGet subscript count is too large")
        })?;
        let dimensions = builder.ins().iconst(types::I32, i64::from(dimensions));
        let field = builder.ins().iconst(types::I32, i64::from(field));
        let (dst_area, dst_slot) = place_addr(dst);
        let dst_area = builder.ins().iconst(types::I32, i64::from(dst_area));
        let dst_slot = builder.ins().iconst(types::I32, dst_slot as i64);
        let callee = self.import(builder, module, self.imports.field_array_get_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                object_ptr,
                field,
                indices_ptr,
                dimensions,
                dst_area,
                dst_slot,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_field_array_set(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        object: &OxOperand,
        field: i32,
        indices: &[OxOperand],
        value: &OxOperand,
    ) -> Result<(), JitError> {
        if indices.is_empty() {
            return Err(JitError::unsupported(
                "M4-8 FieldArraySet lowering requires at least one subscript",
            ));
        }
        let object = self.lower_variant_operand(builder, object)?;
        let object_ptr = self.emit_variant_operand_descriptors(builder, module, &[object])?;
        let indices_ptr =
            self.emit_lowered_i32_indices(builder, module, indices, "FieldArraySet subscript")?;
        let dimensions = i32::try_from(indices.len()).map_err(|_| {
            JitError::unsupported("M4-8 FieldArraySet subscript count is too large")
        })?;
        let dimensions = builder.ins().iconst(types::I32, i64::from(dimensions));
        let value = self.lower_variant_operand(builder, value)?;
        let value_ptr = self.emit_variant_operand_descriptors(builder, module, &[value])?;
        let field = builder.ins().iconst(types::I32, i64::from(field));
        let callee = self.import(builder, module, self.imports.field_array_set_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                object_ptr,
                field,
                indices_ptr,
                dimensions,
                value_ptr,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_lowered_i32_indices(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        indices: &[OxOperand],
        label: &'static str,
    ) -> Result<Value, JitError> {
        let mut index_values = Vec::with_capacity(indices.len());
        for index in indices {
            index_values.push(self.lower_operand_i32(builder, module, index)?);
        }
        self.emit_i32_stack_values(builder, module, &index_values, label)
    }

    fn emit_unbox_to_slot(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        src: &OxOperand,
        to: &OxTy,
        checked: bool,
    ) -> Result<(), JitError> {
        let target = raw_unbox_target(to).ok_or_else(|| {
            JitError::unsupported(format!("M4-4 Unbox target not lowered: {to:?}"))
        })?;
        let operands = [self.lower_variant_operand(builder, src)?];
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &operands)?;
        let (area, index) = place_addr(dst);
        let target = builder.ins().iconst(types::I32, i64::from(target));
        let checked = builder
            .ins()
            .iconst(types::I32, i64::from(u8::from(checked)));
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.unbox_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                target,
                checked,
                operand_ptr,
                area,
                index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_checked_slot_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        helper: ClifFuncId,
        lhs: Value,
        rhs: Value,
    ) -> Result<(), JitError> {
        let (area, index) = place_addr(dst);
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

    /// Seat Overflow (error 6) when a checked Single/Double arithmetic result is
    /// not finite. The f32/f64 fast paths compute with raw `fadd`/`fsub`/`fmul`,
    /// so an overflow produced ±Inf silently; vm3 raises error 6 (its Single/Double
    /// coercion rejects a non-finite result). This restores parity: a finite
    /// result flows through; a non-finite one seats error 6 and routes to the
    /// active On Error handler (or returns the fault) via `return_if_not_ok`.
    fn emit_overflow_if_not_finite(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        value: Value,
        is_single: bool,
    ) -> Result<(), JitError> {
        let abs = builder.ins().fabs(value);
        // An ordered `|x| <= MAX` is false for both Inf and NaN, so it is exactly
        // the finiteness test.
        let max = if is_single {
            builder.ins().f32const(f32::MAX)
        } else {
            builder.ins().f64const(f64::MAX)
        };
        let is_finite = builder
            .ins()
            .fcmp(ir::condcodes::FloatCC::LessThanOrEqual, abs, max);

        let seat_block = builder.create_block();
        let merge_block = builder.create_block();
        builder.append_block_param(merge_block, types::I32);
        let ok_status = builder.ins().iconst(types::I32, i64::from(ST_OK));
        builder
            .ins()
            .brif(is_finite, merge_block, &[ok_status.into()], seat_block, &[]);

        builder.switch_to_block(seat_block);
        builder.seal_block(seat_block);
        let callee = self.import(builder, module, self.imports.raise_error_number);
        let number = builder.ins().iconst(types::I32, 6);
        let inherit = builder.ins().iconst(types::I32, 0);
        let source_ptr_val = i64::try_from(self.program.unit_name.as_ptr() as usize)
            .map_err(|_| JitError::unsupported("unit name pointer exceeds i64"))?;
        let source_len_val = i32::try_from(self.program.unit_name.len())
            .map_err(|_| JitError::unsupported("unit name length exceeds i32"))?;
        let ptr_ty = module.target_config().pointer_type();
        let source_ptr = builder.ins().iconst(ptr_ty, source_ptr_val);
        let source_len = builder.ins().iconst(types::I32, i64::from(source_len_val));
        let call = builder.ins().call(
            callee,
            &[self.state, number, inherit, source_ptr, source_len],
        );
        let fault_status = builder.inst_results(call)[0];
        builder.ins().jump(merge_block, &[fault_status.into()]);

        builder.switch_to_block(merge_block);
        builder.seal_block(merge_block);
        let status = builder.block_params(merge_block)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn lower_long_call_arg(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        arg: &OxArg,
    ) -> Result<LoweredCallArg, JitError> {
        let zero_i32 = builder.ins().iconst(types::I32, 0);
        let zero_i64 = builder.ins().iconst(types::I64, 0);
        match arg {
            OxArg::ByVal(arg) => {
                let kind = builder
                    .ins()
                    .iconst(types::I32, i64::from(JIT_CALL_ARG_BYVAL_SCALAR));
                let value = self.lower_operand_i32(builder, module, arg)?;
                let value = builder.ins().sextend(types::I64, value);
                Ok(LoweredCallArg {
                    kind,
                    aux: zero_i32,
                    value,
                    area: zero_i32,
                    index: zero_i32,
                })
            }
            OxArg::ByRef(place) => {
                self.ensure_long_place(*place)?;
                let (area, index) = place_addr(*place);
                let kind = builder
                    .ins()
                    .iconst(types::I32, i64::from(JIT_CALL_ARG_BYREF_ALIAS));
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                Ok(LoweredCallArg {
                    kind,
                    aux: zero_i32,
                    value: zero_i64,
                    area,
                    index,
                })
            }
            other => Err(JitError::unsupported(format!(
                "M4-4 call subset lowers only ByVal/ByRef Long args, got {other:?}"
            ))),
        }
    }

    fn lower_unknown_long_call_arg(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        arg: &OxArg,
    ) -> Result<LoweredCallArg, JitError> {
        if let OxArg::ByVal(operand) = arg
            && self.operand_is_variant_source(operand)?
        {
            let operand = self.lower_variant_operand(builder, operand)?;
            let kind = builder
                .ins()
                .iconst(types::I32, i64::from(JIT_CALL_ARG_BYVAL_VARIANT));
            return Ok(LoweredCallArg {
                kind,
                aux: operand.kind,
                value: operand.value,
                area: operand.area,
                index: operand.index,
            });
        }
        self.lower_long_call_arg(builder, module, arg)
    }

    fn lower_long_call_args(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        args: &[OxArg],
    ) -> Result<Vec<LoweredCallArg>, JitError> {
        let mut lowered = Vec::with_capacity(args.len());
        for arg in args {
            lowered.push(self.lower_unknown_long_call_arg(builder, module, arg)?);
        }
        Ok(lowered)
    }

    fn lower_static_scalar_call_arg_value(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        param_ty: &OxTy,
        arg: &OxOperand,
    ) -> Result<Value, JitError> {
        match param_ty {
            OxTy::Long => {
                let value = self.lower_operand_i32(builder, module, arg)?;
                Ok(builder.ins().sextend(types::I64, value))
            }
            OxTy::LongLong => self.lower_operand_i64(builder, module, arg),
            OxTy::Currency => self.lower_operand_currency_i64(builder, module, arg),
            OxTy::Single => {
                let value = self.lower_operand_f32(builder, module, arg)?;
                let callee = self.import(builder, module, self.imports.pack_f32_arg);
                let call = builder.ins().call(callee, &[value]);
                Ok(builder.inst_results(call)[0])
            }
            OxTy::Double => {
                let value = self.lower_operand_f64(builder, module, arg)?;
                let callee = self.import(builder, module, self.imports.pack_f64_arg);
                let call = builder.ins().call(callee, &[value]);
                Ok(builder.inst_results(call)[0])
            }
            OxTy::Date => {
                let value = self.lower_operand_date_f64(builder, module, arg)?;
                let callee = self.import(builder, module, self.imports.pack_f64_arg);
                let call = builder.ins().call(callee, &[value]);
                Ok(builder.inst_results(call)[0])
            }
            OxTy::Byte => {
                let value = self.lower_operand_u8_i32(builder, module, arg)?;
                Ok(builder.ins().sextend(types::I64, value))
            }
            OxTy::Integer => {
                let value = self.lower_operand_i16_i32(builder, module, arg)?;
                Ok(builder.ins().sextend(types::I64, value))
            }
            OxTy::Bool => {
                let value = self.lower_operand_bool_i32(builder, module, arg)?;
                Ok(builder.ins().sextend(types::I64, value))
            }
            ty => Err(JitError::unsupported(format!(
                "JIT static scalar call path cannot lower {ty:?}"
            ))),
        }
    }

    fn lower_string_byval_call_arg(
        &self,
        builder: &mut FunctionBuilder<'_>,
        arg: &OxOperand,
    ) -> Result<LoweredCallArg, JitError> {
        if !self.operand_is_static_string_source(arg)? {
            return Err(JitError::unsupported(format!(
                "M4-4 unknown-signature CallProcRef String ByVal subset lowers only String carriers, got {arg:?}"
            )));
        }
        let operand = self.lower_variant_operand(builder, arg)?;
        let kind = builder
            .ins()
            .iconst(types::I32, i64::from(JIT_CALL_ARG_BYVAL_VARIANT));
        Ok(LoweredCallArg {
            kind,
            aux: operand.kind,
            value: operand.value,
            area: operand.area,
            index: operand.index,
        })
    }

    fn lower_string_byval_candidate_call_arg(
        &self,
        builder: &mut FunctionBuilder<'_>,
        arg: &OxOperand,
    ) -> Result<LoweredCallArg, JitError> {
        if !self.operand_is_static_string_source(arg)? && !self.operand_is_variant_source(arg)? {
            return Err(JitError::unsupported(format!(
                "M4-4 unknown-signature CallProcRef String ByVal candidate subset lowers only String or Variant carriers, got {arg:?}"
            )));
        }
        let operand = self.lower_variant_operand(builder, arg)?;
        let kind = builder
            .ins()
            .iconst(types::I32, i64::from(JIT_CALL_ARG_BYVAL_VARIANT));
        Ok(LoweredCallArg {
            kind,
            aux: operand.kind,
            value: operand.value,
            area: operand.area,
            index: operand.index,
        })
    }

    fn lower_string_candidate_call_arg(
        &self,
        builder: &mut FunctionBuilder<'_>,
        arg: &OxArg,
    ) -> Result<LoweredCallArg, JitError> {
        let zero_i32 = builder.ins().iconst(types::I32, 0);
        let zero_i64 = builder.ins().iconst(types::I64, 0);
        match arg {
            OxArg::ByVal(operand) => self.lower_string_byval_candidate_call_arg(builder, operand),
            OxArg::ByRef(place) => {
                self.ensure_string_place(*place)?;
                let (area, index) = place_addr(*place);
                let kind = builder
                    .ins()
                    .iconst(types::I32, i64::from(JIT_CALL_ARG_BYREF_ALIAS));
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                Ok(LoweredCallArg {
                    kind,
                    aux: zero_i32,
                    value: zero_i64,
                    area,
                    index,
                })
            }
            other => Err(JitError::unsupported(format!(
                "M4-4 unknown-signature CallProcRef String candidate subset lowers only ByVal String/Variant carriers or exact ByRef String aliases, got {other:?}"
            ))),
        }
    }

    fn lower_unknown_proc_ref_call_args(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        args: &[OxArg],
    ) -> Result<(Vec<LoweredCallArg>, UnknownProcRefArgShape), JitError> {
        if args.is_empty() {
            return Ok((Vec::new(), UnknownProcRefArgShape::LongOnly));
        }
        let mut string_byval_only = true;
        for arg in args {
            let OxArg::ByVal(operand) = arg else {
                string_byval_only = false;
                break;
            };
            if !self.operand_is_static_string_source(operand)? {
                string_byval_only = false;
                break;
            }
        }
        if string_byval_only {
            let mut lowered = Vec::with_capacity(args.len());
            for arg in args {
                let OxArg::ByVal(operand) = arg else {
                    unreachable!("checked string ByVal-only shape above");
                };
                lowered.push(self.lower_string_byval_call_arg(builder, operand)?);
            }
            Ok((lowered, UnknownProcRefArgShape::StringByValOnly))
        } else {
            let mut variant_byval_only = true;
            for arg in args {
                let OxArg::ByVal(operand) = arg else {
                    variant_byval_only = false;
                    break;
                };
                if !self.operand_is_variant_source(operand)? {
                    variant_byval_only = false;
                    break;
                }
            }
            if variant_byval_only {
                return self
                    .lower_long_call_args(builder, module, args)
                    .map(|args| (args, UnknownProcRefArgShape::VariantByValOnly));
            }
            let mut string_byval_candidate = true;
            for arg in args {
                let OxArg::ByVal(operand) = arg else {
                    string_byval_candidate = false;
                    break;
                };
                if !self.operand_is_static_string_source(operand)?
                    && !self.operand_is_variant_source(operand)?
                {
                    string_byval_candidate = false;
                    break;
                }
            }
            if string_byval_candidate {
                let mut lowered = Vec::with_capacity(args.len());
                for arg in args {
                    let OxArg::ByVal(operand) = arg else {
                        unreachable!("checked string ByVal candidate shape above");
                    };
                    lowered.push(self.lower_string_byval_candidate_call_arg(builder, operand)?);
                }
                return Ok((lowered, UnknownProcRefArgShape::StringByValCandidate));
            }
            let mut string_candidate = true;
            for arg in args {
                match arg {
                    OxArg::ByVal(operand) => {
                        if !self.operand_is_static_string_source(operand)?
                            && !self.operand_is_variant_source(operand)?
                        {
                            string_candidate = false;
                            break;
                        }
                    }
                    OxArg::ByRef(place) => {
                        if !matches!(place_ty(self.program, self.func, *place)?, OxTy::Str) {
                            string_candidate = false;
                            break;
                        }
                    }
                    _ => {
                        string_candidate = false;
                        break;
                    }
                }
            }
            if string_candidate {
                let mut lowered = Vec::with_capacity(args.len());
                for arg in args {
                    lowered.push(self.lower_string_candidate_call_arg(builder, arg)?);
                }
                return Ok((lowered, UnknownProcRefArgShape::StringCandidate));
            }
            self.lower_long_call_args(builder, module, args)
                .map(|args| (args, UnknownProcRefArgShape::LongOnly))
        }
    }

    fn lower_array_literal(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        values: &[OxOperand],
        aliases: &[Option<OxPlace>],
        lower_bound: i32,
    ) -> Result<(), JitError> {
        if !aliases.is_empty() && aliases.len() != values.len() {
            return Err(JitError::Compile(format!(
                "ArrayLiteral alias count {} does not match value count {}",
                aliases.len(),
                values.len()
            )));
        }
        let dst_ty = place_ty(self.program, self.func, dst)?;
        if !is_m4_4_variant_array_carrier_ty(dst_ty) {
            return Err(JitError::unsupported(format!(
                "M4-4 ArrayLiteral lowers only Variant or dynamic Variant-array destinations, got {dst_ty:?} at {dst:?}"
            )));
        }
        let mut lowered = Vec::with_capacity(values.len());
        for value in values {
            lowered.push(self.lower_variant_operand(builder, value)?);
        }
        let operands_ptr = self.emit_variant_operand_descriptors(builder, module, &lowered)?;
        let aliases_ptr = self.emit_param_array_alias_descriptors(builder, module, aliases)?;
        let argc = i32::try_from(values.len())
            .map_err(|_| JitError::unsupported("M4-4 ArrayLiteral element count is too large"))?;
        let argc = builder.ins().iconst(types::I32, i64::from(argc));
        let lower_bound = builder.ins().iconst(types::I32, i64::from(lower_bound));
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, i64::from(index));
        let callee = self.import(builder, module, self.imports.array_literal_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.run,
                operands_ptr,
                aliases_ptr,
                argc,
                lower_bound,
                area,
                index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn lower_array_redim(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        inputs: ArrayRedimInputs<'_>,
    ) -> Result<(), JitError> {
        let ArrayRedimInputs {
            upper_bounds,
            lower_bounds,
            element,
            preserve,
            fixed,
        } = inputs;
        let dst_ty = place_ty(self.program, self.func, dst)?;
        let supported_dst = if matches!(dst_ty, OxTy::Variant) {
            true
        } else if fixed {
            is_m4_4_fixed_array_ty_for_element(dst_ty, element)
        } else {
            is_m4_4_array_ty_for_element(dst_ty, element)
        };
        if !supported_dst {
            return Err(JitError::unsupported(format!(
                "M4-4 ArrayRedim lowers only selected array destinations matching the ReDim form, got {dst_ty:?} with {element:?} at {dst:?}"
            )));
        }
        if preserve && fixed {
            return Err(JitError::unsupported(
                "M4-4 ArrayRedim Preserve lowering supports only dynamic selected arrays",
            ));
        }
        if upper_bounds.is_empty() || lower_bounds.len() > upper_bounds.len() {
            return Err(JitError::unsupported(
                "M4-4 ArrayRedim lowering requires at least one upper bound and no extra lower bounds",
            ));
        }

        let mut upper_values = Vec::with_capacity(upper_bounds.len());
        let mut lower_values = Vec::with_capacity(upper_bounds.len());
        for (index, upper) in upper_bounds.iter().enumerate() {
            upper_values.push(self.lower_operand_i32(builder, module, upper)?);
            lower_values.push(match lower_bounds.get(index) {
                Some(lower) => self.lower_operand_i32(builder, module, lower)?,
                None => builder.ins().iconst(types::I32, 0),
            });
        }
        let upper_ptr =
            self.emit_i32_stack_values(builder, module, &upper_values, "ArrayRedim upper-bound")?;
        let lower_ptr =
            self.emit_i32_stack_values(builder, module, &lower_values, "ArrayRedim lower-bound")?;
        let dimensions = i32::try_from(upper_bounds.len())
            .map_err(|_| JitError::unsupported("M4-4 ArrayRedim dimension count is too large"))?;
        let dimensions = builder.ins().iconst(types::I32, i64::from(dimensions));
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let fixed = builder.ins().iconst(types::I32, if fixed { 1 } else { 0 });
        let preserve = builder
            .ins()
            .iconst(types::I32, if preserve { 1 } else { 0 });
        let element_ptr = self.emit_array_element_metadata_ptr(builder, module, element)?;
        let callee = self.import(builder, module, self.imports.array_redim_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                lower_ptr,
                upper_ptr,
                dimensions,
                element_ptr,
                fixed,
                preserve,
                area,
                index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn lower_array_get(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        array: &OxOperand,
        indices: &[OxOperand],
    ) -> Result<(), JitError> {
        let OxOperand::Use(array_place) = array else {
            return Err(JitError::unsupported(format!(
                "M4-4 ArrayGet lowering requires a place-resident array operand, got {array:?}"
            )));
        };
        let array_ty = place_ty(self.program, self.func, *array_place)?;
        if !is_m4_4_array_index_carrier_ty(array_ty) {
            return Err(JitError::unsupported(format!(
                "M4-4 ArrayGet lowers only Variant or Variant-array receivers, got {array_ty:?} at {array_place:?}"
            )));
        }
        let dst_ty = place_ty(self.program, self.func, dst)?;
        if !is_m4_4_array_get_dst_ty_for_array(dst_ty, array_ty) {
            return Err(JitError::unsupported(format!(
                "M4-4 ArrayGet lowers only matching scalar/String/Variant destinations, got destination {dst_ty:?} for array {array_ty:?} at {dst:?}"
            )));
        }
        if indices.is_empty() {
            return Err(JitError::unsupported(
                "M4-4 ArrayGet lowering requires at least one subscript",
            ));
        }
        if indices.len() == 1 && is_m4_4_long_array_ty(array_ty) {
            let index_value = self.lower_operand_i32(builder, module, &indices[0])?;
            let (array_area, array_index) = place_addr(*array_place);
            let array_area = builder.ins().iconst(types::I32, i64::from(array_area));
            let array_index = builder.ins().iconst(types::I32, array_index as i64);
            let (dst_area, dst_index) = place_addr(dst);
            let dst_area = builder.ins().iconst(types::I32, i64::from(dst_area));
            let dst_index = builder.ins().iconst(types::I32, dst_index as i64);
            let callee = self.import(builder, module, self.imports.array_get_i32_1d_slot);
            let call = builder.ins().call(
                callee,
                &[
                    self.state,
                    self.run,
                    array_area,
                    array_index,
                    index_value,
                    dst_area,
                    dst_index,
                ],
            );
            let status = builder.inst_results(call)[0];
            self.return_if_not_ok(builder, status);
            return Ok(());
        }

        let mut index_values = Vec::with_capacity(indices.len());
        for index in indices {
            index_values.push(self.lower_operand_i32(builder, module, index)?);
        }
        let indices_ptr =
            self.emit_i32_stack_values(builder, module, &index_values, "ArrayGet subscript")?;
        let dimensions = i32::try_from(indices.len())
            .map_err(|_| JitError::unsupported("M4-4 ArrayGet subscript count is too large"))?;
        let dimensions = builder.ins().iconst(types::I32, i64::from(dimensions));
        let (array_area, array_index) = place_addr(*array_place);
        let array_area = builder.ins().iconst(types::I32, i64::from(array_area));
        let array_index = builder.ins().iconst(types::I32, array_index as i64);
        let (dst_area, dst_index) = place_addr(dst);
        let dst_area = builder.ins().iconst(types::I32, i64::from(dst_area));
        let dst_index = builder.ins().iconst(types::I32, dst_index as i64);
        let callee = self.import(builder, module, self.imports.array_get_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                array_area,
                array_index,
                indices_ptr,
                dimensions,
                dst_area,
                dst_index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn lower_array_erase(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        array: OxPlace,
        element: &ArrayElementType,
    ) -> Result<(), JitError> {
        let array_ty = place_ty(self.program, self.func, array)?;
        if !matches!(array_ty, OxTy::Variant) && !is_m4_4_array_ty_for_element(array_ty, element) {
            return Err(JitError::unsupported(format!(
                "M4-4 ArrayErase lowers only supported array places, got {array_ty:?} with element {element:?} at {array:?}"
            )));
        }

        let (area, index) = place_addr(array);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let element_ptr = self.emit_array_element_metadata_ptr(builder, module, element)?;
        let callee = self.import(builder, module, self.imports.array_erase_slot);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, area, index, element_ptr]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_array_element_metadata_ptr(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &JITModule,
        element: &ArrayElementType,
    ) -> Result<Value, JitError> {
        let ptr = i64::try_from(element as *const ArrayElementType as usize).map_err(|_| {
            JitError::unsupported("M4-4 ArrayElementType metadata pointer exceeds i64")
        })?;
        let ptr_ty = module.target_config().pointer_type();
        Ok(builder.ins().iconst(ptr_ty, ptr))
    }

    fn lower_array_set(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        array: OxPlace,
        indices: &[OxOperand],
        value: &OxOperand,
    ) -> Result<(), JitError> {
        let array_ty = place_ty(self.program, self.func, array)?;
        if !is_m4_4_array_index_carrier_ty(array_ty) {
            return Err(JitError::unsupported(format!(
                "M4-4 ArraySet lowers only Variant or Variant-array receivers, got {array_ty:?} at {array:?}"
            )));
        }
        if indices.is_empty() {
            return Err(JitError::unsupported(
                "M4-4 ArraySet lowering requires at least one subscript",
            ));
        }
        if indices.len() == 1
            && is_m4_4_long_array_ty(array_ty)
            && self.can_lower_direct_i32_operand(value)
        {
            let index_value = self.lower_operand_i32(builder, module, &indices[0])?;
            let value = self.lower_operand_i32(builder, module, value)?;
            let (array_area, array_index) = place_addr(array);
            let array_area = builder.ins().iconst(types::I32, i64::from(array_area));
            let array_index = builder.ins().iconst(types::I32, array_index as i64);
            let callee = self.import(builder, module, self.imports.array_set_i32_1d_slot);
            let call = builder.ins().call(
                callee,
                &[
                    self.state,
                    self.run,
                    array_area,
                    array_index,
                    index_value,
                    value,
                ],
            );
            let status = builder.inst_results(call)[0];
            self.return_if_not_ok(builder, status);
            return Ok(());
        }

        let mut index_values = Vec::with_capacity(indices.len());
        for index in indices {
            index_values.push(self.lower_operand_i32(builder, module, index)?);
        }
        let indices_ptr =
            self.emit_i32_stack_values(builder, module, &index_values, "ArraySet subscript")?;
        let dimensions = i32::try_from(indices.len())
            .map_err(|_| JitError::unsupported("M4-4 ArraySet subscript count is too large"))?;
        let dimensions = builder.ins().iconst(types::I32, i64::from(dimensions));
        let value = self.lower_variant_operand(builder, value)?;
        let value_ptr = self.emit_variant_operand_descriptors(builder, module, &[value])?;
        let (array_area, array_index) = place_addr(array);
        let array_area = builder.ins().iconst(types::I32, i64::from(array_area));
        let array_index = builder.ins().iconst(types::I32, array_index as i64);
        let callee = self.import(builder, module, self.imports.array_set_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                array_area,
                array_index,
                indices_ptr,
                dimensions,
                value_ptr,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn lower_bound(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: OxPlace,
        which: BoundWhich,
        array: &OxOperand,
        dimension: Option<&OxOperand>,
    ) -> Result<(), JitError> {
        self.ensure_long_place(dst)?;
        let OxOperand::Use(place) = array else {
            return Err(JitError::unsupported(format!(
                "M4-4 Bound lowering requires a Variant array carrier place operand, got {array:?}"
            )));
        };
        let ty = place_ty(self.program, self.func, *place)?;
        if !is_m4_4_array_index_carrier_ty(ty) {
            return Err(JitError::unsupported(format!(
                "M4-4 Bound lowering requires a Variant array carrier place operand, got {ty:?}"
            )));
        }
        let zero_i64 = builder.ins().iconst(types::I64, 0);
        let (operand_area, operand_index) = place_addr(*place);
        let operand = LoweredVariantOperand {
            kind: builder
                .ins()
                .iconst(types::I32, i64::from(JIT_VARIANT_OPERAND_PLACE)),
            value: zero_i64,
            area: builder.ins().iconst(types::I32, i64::from(operand_area)),
            index: builder.ins().iconst(types::I32, operand_index as i64),
        };
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &[operand])?;
        let dimension = match dimension {
            Some(dimension) => self.lower_operand_i32(builder, module, dimension)?,
            None => builder.ins().iconst(types::I32, 1),
        };
        let which = match which {
            BoundWhich::Lower => 0,
            BoundWhich::Upper => 1,
        };
        let which = builder.ins().iconst(types::I32, which);
        let (area, index) = place_addr(dst);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.bound_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                operand_ptr,
                which,
                dimension,
                area,
                index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn lower_for_each_init(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        iter: OxPlace,
        source: &OxOperand,
    ) -> Result<(), JitError> {
        self.ensure_variant_place(iter)?;
        let OxOperand::Use(source_place) = source else {
            return Err(JitError::unsupported(format!(
                "M4-4 ForEachInit lowers only place-resident array sources, got {source:?}"
            )));
        };
        let source_ty = place_ty(self.program, self.func, *source_place)?;
        if !is_m4_4_for_each_source_ty(source_ty) {
            return Err(JitError::unsupported(format!(
                "M4-4 ForEachInit lowers only descriptor-supported scalar, Variant, or Variant-array sources, got {source_ty:?} at {source_place:?}"
            )));
        }
        let source = self.lower_variant_operand(builder, source)?;
        let source_ptr = self.emit_variant_operand_descriptors(builder, module, &[source])?;
        let (iter_area, iter_index) = place_addr(iter);
        let iter_area = builder.ins().iconst(types::I32, i64::from(iter_area));
        let iter_index = builder.ins().iconst(types::I32, iter_index as i64);
        let callee = self.import(builder, module, self.imports.for_each_init_slot);
        let call = builder.ins().call(
            callee,
            &[self.state, self.run, source_ptr, iter_area, iter_index],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn lower_for_each_next(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        iter: OxPlace,
        item: OxPlace,
        has_value: OxPlace,
    ) -> Result<(), JitError> {
        self.ensure_variant_place(iter)?;
        self.ensure_variant_place(item)?;
        self.ensure_bool_place(has_value)?;
        let (iter_area, iter_index) = place_addr(iter);
        let iter_area = builder.ins().iconst(types::I32, i64::from(iter_area));
        let iter_index = builder.ins().iconst(types::I32, iter_index as i64);
        let (item_area, item_index) = place_addr(item);
        let item_area = builder.ins().iconst(types::I32, i64::from(item_area));
        let item_index = builder.ins().iconst(types::I32, item_index as i64);
        let (has_area, has_index) = place_addr(has_value);
        let has_area = builder.ins().iconst(types::I32, i64::from(has_area));
        let has_index = builder.ins().iconst(types::I32, has_index as i64);
        let callee = self.import(builder, module, self.imports.for_each_next_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.run, iter_area, iter_index, item_area, item_index, has_area, has_index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn lower_static_call_arg(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        arg: &OxArg,
        param: &OxLocal,
    ) -> Result<LoweredCallArg, JitError> {
        let param_ty = &param.ty;
        let zero_i32 = builder.ins().iconst(types::I32, 0);
        let zero_i64 = builder.ins().iconst(types::I64, 0);
        match arg {
            OxArg::ByVal(arg) => {
                if param.param.as_ref().is_some_and(|info| info.by_ref) {
                    if is_jit_variant_carrier_ty(param_ty) {
                        let operand = self.lower_variant_operand(builder, arg)?;
                        let kind = builder
                            .ins()
                            .iconst(types::I32, i64::from(JIT_CALL_ARG_BYREF_COPY));
                        return Ok(LoweredCallArg {
                            kind,
                            aux: operand.kind,
                            value: operand.value,
                            area: operand.area,
                            index: operand.index,
                        });
                    }
                    let kind = builder
                        .ins()
                        .iconst(types::I32, i64::from(JIT_CALL_ARG_BYREF_COPY));
                    let value =
                        self.lower_static_scalar_call_arg_value(builder, module, param_ty, arg)?;
                    return Ok(LoweredCallArg {
                        kind,
                        aux: zero_i32,
                        value,
                        area: zero_i32,
                        index: zero_i32,
                    });
                }
                if param.param.as_ref().is_some_and(|info| info.variadic) {
                    let OxOperand::Use(place) = arg else {
                        return Err(JitError::unsupported(format!(
                            "M4-4 ParamArray support requires a packed dynamic Variant-array carrier for {}, got {arg:?}",
                            param.name
                        )));
                    };
                    let actual = place_ty(self.program, self.func, *place)?;
                    if !is_m4_4_dynamic_variant_array_ty(actual) {
                        return Err(JitError::unsupported(format!(
                            "M4-4 ParamArray support requires a packed dynamic Variant-array carrier for {}, got {actual:?}",
                            param.name
                        )));
                    }
                    let operand = self.lower_variant_operand(builder, arg)?;
                    let kind = builder
                        .ins()
                        .iconst(types::I32, i64::from(JIT_CALL_ARG_BYVAL_VARIANT));
                    return Ok(LoweredCallArg {
                        kind,
                        aux: operand.kind,
                        value: operand.value,
                        area: operand.area,
                        index: operand.index,
                    });
                }
                if is_jit_variant_carrier_ty(param_ty) {
                    let operand = self.lower_variant_operand(builder, arg)?;
                    let kind = builder
                        .ins()
                        .iconst(types::I32, i64::from(JIT_CALL_ARG_BYVAL_VARIANT));
                    return Ok(LoweredCallArg {
                        kind,
                        aux: operand.kind,
                        value: operand.value,
                        area: operand.area,
                        index: operand.index,
                    });
                }
                let kind = builder
                    .ins()
                    .iconst(types::I32, i64::from(JIT_CALL_ARG_BYVAL_SCALAR));
                let value =
                    self.lower_static_scalar_call_arg_value(builder, module, param_ty, arg)?;
                Ok(LoweredCallArg {
                    kind,
                    aux: zero_i32,
                    value,
                    area: zero_i32,
                    index: zero_i32,
                })
            }
            OxArg::ByRef(place) => {
                let actual = place_ty(self.program, self.func, *place)?;
                if actual != param_ty {
                    return Err(JitError::unsupported(format!(
                        "M4-4 static call subset requires exact ByRef type match, got {actual:?} for {param_ty:?} at {place:?}"
                    )));
                }
                let (area, index) = place_addr(*place);
                let kind = builder
                    .ins()
                    .iconst(types::I32, i64::from(JIT_CALL_ARG_BYREF_ALIAS));
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                Ok(LoweredCallArg {
                    kind,
                    aux: zero_i32,
                    value: zero_i64,
                    area,
                    index,
                })
            }
            OxArg::Omitted if matches!(param_ty, OxTy::Variant) => {
                let kind = builder
                    .ins()
                    .iconst(types::I32, i64::from(JIT_CALL_ARG_OMITTED));
                Ok(LoweredCallArg {
                    kind,
                    aux: zero_i32,
                    value: zero_i64,
                    area: zero_i32,
                    index: zero_i32,
                })
            }
            other => Err(JitError::unsupported(format!(
                "M4-4 static call subset lowers only ByVal/ByRef args, got {other:?}"
            ))),
        }
    }

    fn lower_static_call_args(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        callee: &OxFunc,
        args: &[OxArg],
    ) -> Result<Vec<LoweredCallArg>, JitError> {
        let mut lowered = Vec::with_capacity(args.len());
        for (index, arg) in args.iter().enumerate() {
            let Some(param) = callee.locals.get(index) else {
                return Err(JitError::Compile(format!(
                    "callee {} has param_count without local {index}",
                    callee.name
                )));
            };
            lowered.push(self.lower_static_call_arg(builder, module, arg, param)?);
        }
        Ok(lowered)
    }

    fn emit_call_arg_descriptors(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &JITModule,
        args: &[LoweredCallArg],
    ) -> Result<Value, JitError> {
        let ptr_ty = module.target_config().pointer_type();
        if args.is_empty() {
            return Ok(builder.ins().iconst(ptr_ty, 0));
        }
        let byte_size = args
            .len()
            .checked_mul(JIT_CALL_ARG_DESC_SIZE as usize)
            .and_then(|size| i32::try_from(size).ok())
            .and_then(|size| u32::try_from(size).ok())
            .ok_or_else(|| JitError::unsupported("M4-4 call descriptor stack is too large"))?;
        let slot = builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            byte_size,
            3,
        ));
        for (index, arg) in args.iter().enumerate() {
            let base = i32::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(JIT_CALL_ARG_DESC_SIZE as i32))
                .ok_or_else(|| JitError::unsupported("M4-4 call descriptor offset overflow"))?;
            builder.ins().stack_store(arg.kind, slot, base);
            builder
                .ins()
                .stack_store(arg.aux, slot, base + JIT_CALL_ARG_AUX_OFFSET);
            builder
                .ins()
                .stack_store(arg.value, slot, base + JIT_CALL_ARG_VALUE_OFFSET);
            builder
                .ins()
                .stack_store(arg.area, slot, base + JIT_CALL_ARG_AREA_OFFSET);
            builder
                .ins()
                .stack_store(arg.index, slot, base + JIT_CALL_ARG_INDEX_OFFSET);
        }
        Ok(builder.ins().stack_addr(ptr_ty, slot, 0))
    }

    fn emit_call_arg_name_descriptors(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &JITModule,
        names: &[Option<&str>],
    ) -> Result<Value, JitError> {
        let ptr_ty = module.target_config().pointer_type();
        if names.is_empty() {
            return Ok(builder.ins().iconst(ptr_ty, 0));
        }
        let byte_size = names
            .len()
            .checked_mul(JIT_CALL_ARG_NAME_DESC_SIZE as usize)
            .and_then(|size| i32::try_from(size).ok())
            .and_then(|size| u32::try_from(size).ok())
            .ok_or_else(|| JitError::unsupported("M4-8 project call name stack is too large"))?;
        let slot = builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            byte_size,
            3,
        ));
        for (index, name) in names.iter().enumerate() {
            let base = i32::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(JIT_CALL_ARG_NAME_DESC_SIZE as i32))
                .ok_or_else(|| JitError::unsupported("M4-8 project call name offset overflow"))?;
            let (ptr, len) = if let Some(name) = name {
                let len = i32::try_from(name.len()).map_err(|_| {
                    JitError::unsupported("JIT project call argument name is too long")
                })?;
                (
                    builder.ins().iconst(ptr_ty, name.as_ptr() as i64),
                    builder.ins().iconst(types::I32, i64::from(len)),
                )
            } else {
                (
                    builder.ins().iconst(ptr_ty, 0),
                    builder.ins().iconst(types::I32, -1),
                )
            };
            builder.ins().stack_store(ptr, slot, base);
            builder
                .ins()
                .stack_store(len, slot, base + JIT_CALL_ARG_NAME_LEN_OFFSET);
        }
        Ok(builder.ins().stack_addr(ptr_ty, slot, 0))
    }

    fn lower_call_proc_ref(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: Option<OxPlace>,
        target: &OxOperand,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<(), JitError> {
        let OxOperand::Use(target_place) = target else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallProcRef subset requires a ProcRef place target, got {target:?}"
            )));
        };
        self.ensure_proc_ref_place(*target_place)?;
        let (target_area, target_index) = place_addr(*target_place);
        let target_area = builder.ins().iconst(types::I32, i64::from(target_area));
        let target_index = builder.ins().iconst(types::I32, target_index as i64);
        let target_info = self
            .static_proc_refs
            .get(target_place)
            .copied()
            .unwrap_or(ProcRefStaticTarget::Unknown);
        if let ProcRefStaticTarget::Unique(proc) = target_info {
            let callee = self.program.funcs.get(proc.0).ok_or_else(|| {
                JitError::Compile(format!("proc-ref target {} out of range", proc.0))
            })?;
            self.validate_signature_known_proc_ref_call(callee, dst, args)?;
            let lowered_args = self.lower_static_call_args(builder, module, callee, args)?;
            let args_ptr = self.emit_call_arg_descriptors(builder, module, &lowered_args)?;
            self.lower_expect_proc_ref_target(builder, module, target_area, target_index, proc)?;
            return self.lower_direct_descriptor_static_call(
                builder,
                module,
                proc,
                args.len(),
                args_ptr,
                dst,
            );
        }
        let signature_proc = target_info.signature_proc();
        let expected_proc = target_info.expected_marker()?;
        let expected_proc_value = builder.ins().iconst(types::I32, i64::from(expected_proc));
        let (dst_area, dst_index, dynamic_ret_kind, lowered_args) = if let Some(proc) =
            signature_proc
        {
            let callee = self.program.funcs.get(proc.0).ok_or_else(|| {
                JitError::Compile(format!("proc-ref target {} out of range", proc.0))
            })?;
            self.validate_signature_known_proc_ref_call(callee, dst, args)?;
            let (dst_area, dst_index) = if let Some(dst) = dst {
                let (area, index) = place_addr(dst);
                (
                    builder.ins().iconst(types::I32, i64::from(area)),
                    builder.ins().iconst(types::I32, i64::from(index)),
                )
            } else {
                (
                    builder.ins().iconst(types::I32, -1),
                    builder.ins().iconst(types::I32, -1),
                )
            };
            let lowered_args = self.lower_static_call_args(builder, module, callee, args)?;
            (
                dst_area,
                dst_index,
                builder
                    .ins()
                    .iconst(types::I32, i64::from(JIT_PROC_REF_RET_NONE)),
                lowered_args,
            )
        } else {
            let (dst_area, dst_index, ret_kind) = if let Some(dst) = dst {
                let dst_ty = place_ty(self.program, self.func, dst)?;
                let ret_kind = match dst_ty {
                    OxTy::Long => {
                        self.ensure_long_place(dst)?;
                        JIT_PROC_REF_RET_LONG
                    }
                    OxTy::Str => {
                        self.ensure_string_place(dst)?;
                        JIT_PROC_REF_RET_STRING
                    }
                    OxTy::Variant => {
                        self.ensure_variant_place(dst)?;
                        JIT_PROC_REF_RET_VARIANT
                    }
                    OxTy::LongLong
                    | OxTy::Currency
                    | OxTy::Single
                    | OxTy::Double
                    | OxTy::Date
                    | OxTy::Byte
                    | OxTy::Integer
                    | OxTy::Bool => unknown_proc_ref_exact_return_kind(dst_ty).ok_or_else(
                        || {
                            JitError::unsupported(format!(
                                "M4-4 unknown-signature CallProcRef cannot encode exact return kind for {dst_ty:?}"
                            ))
                        },
                    )?,
                    _ => {
                        return Err(JitError::unsupported(format!(
                            "M4-4 unknown-signature CallProcRef returns only Long into exact destinations with arguments, concrete scalar/String no-argument returns into exact destinations, or the current concrete/actual Variant return-to-Variant subset, got {dst_ty:?}"
                        )));
                    }
                };
                let (area, index) = place_addr(dst);
                (
                    builder.ins().iconst(types::I32, i64::from(area)),
                    builder.ins().iconst(types::I32, index as i64),
                    ret_kind,
                )
            } else {
                (
                    builder.ins().iconst(types::I32, -1),
                    builder.ins().iconst(types::I32, -1),
                    JIT_PROC_REF_RET_NONE,
                )
            };
            let (lowered_args, arg_shape) =
                self.lower_unknown_proc_ref_call_args(builder, module, args)?;
            let allowed = args.is_empty()
                || matches!(
                    (ret_kind, arg_shape),
                    (JIT_PROC_REF_RET_LONG, UnknownProcRefArgShape::LongOnly)
                        | (JIT_PROC_REF_RET_NONE, UnknownProcRefArgShape::LongOnly)
                        | (JIT_PROC_REF_RET_VARIANT, UnknownProcRefArgShape::LongOnly)
                        | (
                            JIT_PROC_REF_RET_NONE,
                            UnknownProcRefArgShape::StringByValOnly
                        )
                        | (
                            JIT_PROC_REF_RET_STRING,
                            UnknownProcRefArgShape::StringByValOnly
                        )
                        | (
                            JIT_PROC_REF_RET_VARIANT,
                            UnknownProcRefArgShape::StringByValOnly
                        )
                        | (
                            JIT_PROC_REF_RET_NONE,
                            UnknownProcRefArgShape::VariantByValOnly
                        )
                        | (
                            JIT_PROC_REF_RET_LONG,
                            UnknownProcRefArgShape::VariantByValOnly
                        )
                        | (
                            JIT_PROC_REF_RET_STRING,
                            UnknownProcRefArgShape::VariantByValOnly
                        )
                        | (
                            JIT_PROC_REF_RET_VARIANT,
                            UnknownProcRefArgShape::VariantByValOnly
                        )
                        | (
                            JIT_PROC_REF_RET_NONE,
                            UnknownProcRefArgShape::StringByValCandidate
                        )
                        | (
                            JIT_PROC_REF_RET_STRING,
                            UnknownProcRefArgShape::StringByValCandidate
                        )
                        | (
                            JIT_PROC_REF_RET_VARIANT,
                            UnknownProcRefArgShape::StringByValCandidate
                        )
                        | (
                            JIT_PROC_REF_RET_NONE,
                            UnknownProcRefArgShape::StringCandidate
                        )
                        | (
                            JIT_PROC_REF_RET_STRING,
                            UnknownProcRefArgShape::StringCandidate
                        )
                        | (
                            JIT_PROC_REF_RET_VARIANT,
                            UnknownProcRefArgShape::StringCandidate
                        )
                );
            if !allowed {
                return Err(JitError::unsupported(
                    "M4-4 unknown-signature CallProcRef non-empty fallback supports only Long-only calls or exact String ByVal-only calls with no destination, a matching concrete return, or an actual Variant return to a Variant destination",
                ));
            }
            let dynamic_ret_kind = builder.ins().iconst(types::I32, i64::from(ret_kind));
            (dst_area, dst_index, dynamic_ret_kind, lowered_args)
        };
        let args_ptr = self.emit_call_arg_descriptors(builder, module, &lowered_args)?;
        let argc = i32::try_from(args.len())
            .map_err(|_| JitError::unsupported("M4-4 call argument count is too large"))?;
        let argc = builder.ins().iconst(types::I32, i64::from(argc));
        let callee = self.import(builder, module, self.imports.call_proc_ref_i32);
        let call = builder.ins().call(
            callee,
            &[
                self.run,
                self.state,
                target_area,
                target_index,
                expected_proc_value,
                dynamic_ret_kind,
                argc,
                args_ptr,
                dst_area,
                dst_index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn validate_signature_known_proc_ref_call(
        &self,
        callee: &OxFunc,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<(), JitError> {
        if let Some(dst) = dst {
            let Some(ret) = callee.return_local else {
                return Err(JitError::unsupported(format!(
                    "CallProcRef destination was requested for Sub {}",
                    callee.name
                )));
            };
            let Some(ret_ty) = callee.locals.get(ret.0).map(|local| &local.ty) else {
                return Err(JitError::Compile(format!(
                    "return local {} out of range in {}",
                    ret.0, callee.name
                )));
            };
            if !is_jit_static_call_ty(ret_ty) {
                return Err(JitError::unsupported(format!(
                    "JIT CallProcRef signature-known path accepts every supported scalar/carrier return in {}, got {ret_ty:?}",
                    callee.name
                )));
            }
            let dst_ty = place_ty(self.program, self.func, dst)?;
            if !is_m4_4_call_return_destination_ty(dst_ty, ret_ty) {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallProcRef signature-known subset requires exact return destination type match or a Variant destination, got {dst_ty:?} for {ret_ty:?}"
                )));
            }
        }
        if args.len() != callee.param_count {
            return Err(JitError::unsupported(format!(
                "CallProcRef argument count {} does not match callee param_count {} for {}",
                args.len(),
                callee.param_count,
                callee.name
            )));
        }
        for (index, arg) in args.iter().enumerate() {
            validate_static_scalar_call_arg(callee, index, arg)?;
        }
        Ok(())
    }

    fn lower_expect_proc_ref_target(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        target_area: Value,
        target_index: Value,
        proc: FuncId,
    ) -> Result<(), JitError> {
        let expected_proc = builder.ins().iconst(types::I32, proc.0 as i64);
        let callee = self.import(builder, module, self.imports.expect_proc_ref_i32);
        let call = builder.ins().call(
            callee,
            &[
                self.run,
                self.state,
                target_area,
                target_index,
                expected_proc,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn can_lower_direct_noarg_sub_call(
        &self,
        callee: &OxFunc,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
    ) -> bool {
        dst.is_none() && args.is_empty() && callee.param_count == 0 && callee.return_local.is_none()
    }

    fn can_lower_direct_noarg_function_call(
        &self,
        callee: &OxFunc,
        dst: OxPlace,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<bool, JitError> {
        if !args.is_empty() || callee.param_count != 0 {
            return Ok(false);
        }
        let Some(ret) = callee.return_local else {
            return Ok(false);
        };
        let Some(ret_ty) = callee.locals.get(ret.0).map(|local| &local.ty) else {
            return Err(JitError::Compile(format!(
                "return local {} out of range in {}",
                ret.0, callee.name
            )));
        };
        if !is_m4_4_call_scalar_ty(ret_ty) {
            return Ok(false);
        }
        let dst_ty = place_ty(self.program, self.func, dst)?;
        Ok(is_m4_4_call_return_destination_ty(dst_ty, ret_ty))
    }

    fn can_lower_direct_ignored_noarg_function_call(
        &self,
        callee: &OxFunc,
        args: &[oxvba_oxir::OxArg],
    ) -> bool {
        args.is_empty() && callee.param_count == 0 && callee.return_local.is_some()
    }

    fn can_lower_direct_one_i32_sub_call(
        &self,
        callee: &OxFunc,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
    ) -> bool {
        if dst.is_some()
            || args.len() != 1
            || callee.param_count != 1
            || callee.return_local.is_some()
        {
            return false;
        }
        let Some(param) = callee.locals.first() else {
            return false;
        };
        matches!(param.ty, OxTy::Long)
            && param
                .param
                .as_ref()
                .is_some_and(|info| !info.by_ref && !info.variadic)
            && matches!(
                args.first(),
                Some(OxArg::ByVal(operand)) if self.can_lower_direct_i32_operand(operand)
            )
    }

    fn can_lower_direct_two_i32_sub_call(
        &self,
        callee: &OxFunc,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
    ) -> bool {
        dst.is_none()
            && callee.return_local.is_none()
            && self.can_lower_direct_i32_byval_args(callee, args, 2)
    }

    fn can_lower_direct_i32_operand(&self, operand: &OxOperand) -> bool {
        match operand {
            OxOperand::Const(OxConst::I32(_) | OxConst::I16(_) | OxConst::Bool(_)) => true,
            OxOperand::Use(place) => self.ensure_i32_numeric_place(*place).is_ok(),
            _ => false,
        }
    }

    fn can_lower_direct_i32_byval_args(
        &self,
        callee: &OxFunc,
        args: &[oxvba_oxir::OxArg],
        count: usize,
    ) -> bool {
        if args.len() != count || callee.param_count != count {
            return false;
        }
        for (index, arg) in args.iter().enumerate() {
            let Some(param) = callee.locals.get(index) else {
                return false;
            };
            if !matches!(param.ty, OxTy::Long)
                || !param
                    .param
                    .as_ref()
                    .is_some_and(|info| !info.by_ref && !info.variadic)
                || !matches!(arg, OxArg::ByVal(operand) if self.can_lower_direct_i32_operand(operand))
            {
                return false;
            }
        }
        true
    }

    fn can_lower_direct_one_i32_byref_arg(
        &self,
        callee: &OxFunc,
        args: &[oxvba_oxir::OxArg],
    ) -> bool {
        if args.len() != 1 || callee.param_count != 1 {
            return false;
        }
        let Some(param) = callee.locals.first() else {
            return false;
        };
        matches!(param.ty, OxTy::Long)
            && param
                .param
                .as_ref()
                .is_some_and(|info| info.by_ref && !info.variadic)
            && matches!(
                args.first(),
                Some(OxArg::ByRef(place)) if self.ensure_long_place(*place).is_ok()
            )
    }

    fn can_lower_direct_one_i32_byref_sub_call(
        &self,
        callee: &OxFunc,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
    ) -> bool {
        dst.is_none()
            && callee.return_local.is_none()
            && self.can_lower_direct_one_i32_byref_arg(callee, args)
    }

    fn can_lower_direct_one_i32_function_call(
        &self,
        callee: &OxFunc,
        dst: OxPlace,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<bool, JitError> {
        if args.len() != 1 || callee.param_count != 1 {
            return Ok(false);
        }
        let Some(param) = callee.locals.first() else {
            return Ok(false);
        };
        if !matches!(param.ty, OxTy::Long)
            || !param
                .param
                .as_ref()
                .is_some_and(|info| !info.by_ref && !info.variadic)
            || !matches!(
                args.first(),
                Some(OxArg::ByVal(operand)) if self.can_lower_direct_i32_operand(operand)
            )
        {
            return Ok(false);
        }
        let Some(ret) = callee.return_local else {
            return Ok(false);
        };
        let Some(ret_ty) = callee.locals.get(ret.0).map(|local| &local.ty) else {
            return Err(JitError::Compile(format!(
                "return local {} out of range in {}",
                ret.0, callee.name
            )));
        };
        if !is_m4_4_call_scalar_ty(ret_ty) {
            return Ok(false);
        }
        let dst_ty = place_ty(self.program, self.func, dst)?;
        Ok(is_m4_4_call_return_destination_ty(dst_ty, ret_ty))
    }

    fn can_lower_direct_one_i32_byref_function_call(
        &self,
        callee: &OxFunc,
        dst: OxPlace,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<bool, JitError> {
        if !self.can_lower_direct_one_i32_byref_arg(callee, args) {
            return Ok(false);
        }
        let Some(ret) = callee.return_local else {
            return Ok(false);
        };
        let Some(ret_ty) = callee.locals.get(ret.0).map(|local| &local.ty) else {
            return Err(JitError::Compile(format!(
                "return local {} out of range in {}",
                ret.0, callee.name
            )));
        };
        if !is_m4_4_call_scalar_ty(ret_ty) {
            return Ok(false);
        }
        let dst_ty = place_ty(self.program, self.func, dst)?;
        Ok(is_m4_4_call_return_destination_ty(dst_ty, ret_ty))
    }

    fn can_lower_direct_two_i32_function_call(
        &self,
        callee: &OxFunc,
        dst: OxPlace,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<bool, JitError> {
        if !self.can_lower_direct_i32_byval_args(callee, args, 2) {
            return Ok(false);
        }
        let Some(ret) = callee.return_local else {
            return Ok(false);
        };
        let Some(ret_ty) = callee.locals.get(ret.0).map(|local| &local.ty) else {
            return Err(JitError::Compile(format!(
                "return local {} out of range in {}",
                ret.0, callee.name
            )));
        };
        if !is_m4_4_call_scalar_ty(ret_ty) {
            return Ok(false);
        }
        let dst_ty = place_ty(self.program, self.func, dst)?;
        Ok(is_m4_4_call_return_destination_ty(dst_ty, ret_ty))
    }

    fn can_lower_direct_ignored_one_i32_function_call(
        &self,
        callee: &OxFunc,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<bool, JitError> {
        if args.len() != 1 || callee.param_count != 1 {
            return Ok(false);
        }
        let Some(param) = callee.locals.first() else {
            return Ok(false);
        };
        if !matches!(param.ty, OxTy::Long)
            || !param
                .param
                .as_ref()
                .is_some_and(|info| !info.by_ref && !info.variadic)
            || !matches!(
                args.first(),
                Some(OxArg::ByVal(operand)) if self.can_lower_direct_i32_operand(operand)
            )
        {
            return Ok(false);
        }
        let Some(ret) = callee.return_local else {
            return Ok(false);
        };
        let Some(ret_ty) = callee.locals.get(ret.0).map(|local| &local.ty) else {
            return Err(JitError::Compile(format!(
                "return local {} out of range in {}",
                ret.0, callee.name
            )));
        };
        Ok(is_m4_4_call_scalar_ty(ret_ty))
    }

    fn can_lower_direct_ignored_one_i32_byref_function_call(
        &self,
        callee: &OxFunc,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<bool, JitError> {
        if !self.can_lower_direct_one_i32_byref_arg(callee, args) {
            return Ok(false);
        }
        let Some(ret) = callee.return_local else {
            return Ok(false);
        };
        let Some(ret_ty) = callee.locals.get(ret.0).map(|local| &local.ty) else {
            return Err(JitError::Compile(format!(
                "return local {} out of range in {}",
                ret.0, callee.name
            )));
        };
        Ok(is_m4_4_call_scalar_ty(ret_ty))
    }

    fn can_lower_direct_ignored_two_i32_function_call(
        &self,
        callee: &OxFunc,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<bool, JitError> {
        if !self.can_lower_direct_i32_byval_args(callee, args, 2) {
            return Ok(false);
        }
        let Some(ret) = callee.return_local else {
            return Ok(false);
        };
        let Some(ret_ty) = callee.locals.get(ret.0).map(|local| &local.ty) else {
            return Err(JitError::Compile(format!(
                "return local {} out of range in {}",
                ret.0, callee.name
            )));
        };
        Ok(is_m4_4_call_scalar_ty(ret_ty))
    }

    fn lower_direct_noarg_sub_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let enter = self.import(builder, module, self.imports.direct_enter_noarg_sub);
        let enter_call = builder
            .ins()
            .call(enter, &[self.run, self.state, proc_value]);
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        let exit = self.import(builder, module, self.imports.direct_exit_noarg_sub);
        let exit_call = builder
            .ins()
            .call(exit, &[self.run, self.state, body_status]);
        let exit_status = builder.inst_results(exit_call)[0];
        self.return_if_not_ok(builder, exit_status);
        Ok(())
    }

    fn lower_direct_one_i32_sub_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let Some(OxArg::ByVal(arg)) = args.first() else {
            return Err(JitError::Compile(
                "direct one-i32 Sub call was selected without a ByVal arg".to_string(),
            ));
        };
        let arg0 = self.lower_operand_i32(builder, module, arg)?;
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let enter = self.import(builder, module, self.imports.direct_enter_one_i32_sub);
        let enter_call = builder
            .ins()
            .call(enter, &[self.run, self.state, proc_value, arg0]);
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        let exit = self.import(builder, module, self.imports.direct_exit_noarg_sub);
        let exit_call = builder
            .ins()
            .call(exit, &[self.run, self.state, body_status]);
        let exit_status = builder.inst_results(exit_call)[0];
        self.return_if_not_ok(builder, exit_status);
        Ok(())
    }

    fn lower_direct_one_i32_byref_sub_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let [OxArg::ByRef(place)] = args else {
            return Err(JitError::Compile(
                "direct one-i32 ByRef Sub call was selected without one ByRef arg".to_string(),
            ));
        };
        let (arg_area, arg_index) = place_addr(*place);
        let arg_area = builder.ins().iconst(types::I32, i64::from(arg_area));
        let arg_index = builder.ins().iconst(types::I32, arg_index as i64);
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let enter = self.import(builder, module, self.imports.direct_enter_one_i32_byref_sub);
        let enter_call = builder.ins().call(
            enter,
            &[self.run, self.state, proc_value, arg_area, arg_index],
        );
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        let exit = self.import(builder, module, self.imports.direct_exit_noarg_sub);
        let exit_call = builder
            .ins()
            .call(exit, &[self.run, self.state, body_status]);
        let exit_status = builder.inst_results(exit_call)[0];
        self.return_if_not_ok(builder, exit_status);
        Ok(())
    }

    fn lower_direct_two_i32_sub_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let [OxArg::ByVal(arg0), OxArg::ByVal(arg1)] = args else {
            return Err(JitError::Compile(
                "direct two-i32 Sub call was selected without two ByVal args".to_string(),
            ));
        };
        let arg0 = self.lower_operand_i32(builder, module, arg0)?;
        let arg1 = self.lower_operand_i32(builder, module, arg1)?;
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let enter = self.import(builder, module, self.imports.direct_enter_two_i32_sub);
        let enter_call = builder
            .ins()
            .call(enter, &[self.run, self.state, proc_value, arg0, arg1]);
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        let exit = self.import(builder, module, self.imports.direct_exit_noarg_sub);
        let exit_call = builder
            .ins()
            .call(exit, &[self.run, self.state, body_status]);
        let exit_status = builder.inst_results(exit_call)[0];
        self.return_if_not_ok(builder, exit_status);
        Ok(())
    }

    fn lower_direct_one_i32_function_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
        args: &[oxvba_oxir::OxArg],
        dst: OxPlace,
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let Some(OxArg::ByVal(arg)) = args.first() else {
            return Err(JitError::Compile(
                "direct one-i32 Function call was selected without a ByVal arg".to_string(),
            ));
        };
        let arg0 = self.lower_operand_i32(builder, module, arg)?;
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let enter = self.import(builder, module, self.imports.direct_enter_one_i32_func);
        let enter_call = builder
            .ins()
            .call(enter, &[self.run, self.state, proc_value, arg0]);
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        let (dst_area, dst_index) = place_addr(dst);
        let dst_area = builder.ins().iconst(types::I32, i64::from(dst_area));
        let dst_index = builder.ins().iconst(types::I32, dst_index as i64);
        let exit = self.import(builder, module, self.imports.direct_exit_noarg_func);
        let exit_call = builder.ins().call(
            exit,
            &[
                self.run,
                self.state,
                proc_value,
                body_status,
                dst_area,
                dst_index,
            ],
        );
        let exit_status = builder.inst_results(exit_call)[0];
        self.return_if_not_ok(builder, exit_status);
        Ok(())
    }

    fn lower_direct_one_i32_byref_function_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
        args: &[oxvba_oxir::OxArg],
        dst: OxPlace,
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let [OxArg::ByRef(place)] = args else {
            return Err(JitError::Compile(
                "direct one-i32 ByRef Function call was selected without one ByRef arg".to_string(),
            ));
        };
        let (arg_area, arg_index) = place_addr(*place);
        let arg_area = builder.ins().iconst(types::I32, i64::from(arg_area));
        let arg_index = builder.ins().iconst(types::I32, arg_index as i64);
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let enter = self.import(
            builder,
            module,
            self.imports.direct_enter_one_i32_byref_func,
        );
        let enter_call = builder.ins().call(
            enter,
            &[self.run, self.state, proc_value, arg_area, arg_index],
        );
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        let (dst_area, dst_index) = place_addr(dst);
        let dst_area = builder.ins().iconst(types::I32, i64::from(dst_area));
        let dst_index = builder.ins().iconst(types::I32, dst_index as i64);
        let exit = self.import(builder, module, self.imports.direct_exit_noarg_func);
        let exit_call = builder.ins().call(
            exit,
            &[
                self.run,
                self.state,
                proc_value,
                body_status,
                dst_area,
                dst_index,
            ],
        );
        let exit_status = builder.inst_results(exit_call)[0];
        self.return_if_not_ok(builder, exit_status);
        Ok(())
    }

    fn lower_direct_two_i32_function_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
        args: &[oxvba_oxir::OxArg],
        dst: OxPlace,
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let [OxArg::ByVal(arg0), OxArg::ByVal(arg1)] = args else {
            return Err(JitError::Compile(
                "direct two-i32 Function call was selected without two ByVal args".to_string(),
            ));
        };
        let arg0 = self.lower_operand_i32(builder, module, arg0)?;
        let arg1 = self.lower_operand_i32(builder, module, arg1)?;
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let enter = self.import(builder, module, self.imports.direct_enter_two_i32_func);
        let enter_call = builder
            .ins()
            .call(enter, &[self.run, self.state, proc_value, arg0, arg1]);
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        let (dst_area, dst_index) = place_addr(dst);
        let dst_area = builder.ins().iconst(types::I32, i64::from(dst_area));
        let dst_index = builder.ins().iconst(types::I32, dst_index as i64);
        let exit = self.import(builder, module, self.imports.direct_exit_noarg_func);
        let exit_call = builder.ins().call(
            exit,
            &[
                self.run,
                self.state,
                proc_value,
                body_status,
                dst_area,
                dst_index,
            ],
        );
        let exit_status = builder.inst_results(exit_call)[0];
        self.return_if_not_ok(builder, exit_status);
        Ok(())
    }

    fn lower_direct_ignored_noarg_function_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let enter = self.import(builder, module, self.imports.direct_enter_noarg_func);
        let enter_call = builder
            .ins()
            .call(enter, &[self.run, self.state, proc_value]);
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        let exit = self.import(builder, module, self.imports.direct_exit_noarg_sub);
        let exit_call = builder
            .ins()
            .call(exit, &[self.run, self.state, body_status]);
        let exit_status = builder.inst_results(exit_call)[0];
        self.return_if_not_ok(builder, exit_status);
        Ok(())
    }

    fn lower_direct_ignored_one_i32_function_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let Some(OxArg::ByVal(arg)) = args.first() else {
            return Err(JitError::Compile(
                "direct ignored one-i32 Function call was selected without a ByVal arg".to_string(),
            ));
        };
        let arg0 = self.lower_operand_i32(builder, module, arg)?;
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let enter = self.import(builder, module, self.imports.direct_enter_one_i32_func);
        let enter_call = builder
            .ins()
            .call(enter, &[self.run, self.state, proc_value, arg0]);
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        let exit = self.import(builder, module, self.imports.direct_exit_noarg_sub);
        let exit_call = builder
            .ins()
            .call(exit, &[self.run, self.state, body_status]);
        let exit_status = builder.inst_results(exit_call)[0];
        self.return_if_not_ok(builder, exit_status);
        Ok(())
    }

    fn lower_direct_ignored_one_i32_byref_function_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let [OxArg::ByRef(place)] = args else {
            return Err(JitError::Compile(
                "direct ignored one-i32 ByRef Function call was selected without one ByRef arg"
                    .to_string(),
            ));
        };
        let (arg_area, arg_index) = place_addr(*place);
        let arg_area = builder.ins().iconst(types::I32, i64::from(arg_area));
        let arg_index = builder.ins().iconst(types::I32, arg_index as i64);
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let enter = self.import(
            builder,
            module,
            self.imports.direct_enter_one_i32_byref_func,
        );
        let enter_call = builder.ins().call(
            enter,
            &[self.run, self.state, proc_value, arg_area, arg_index],
        );
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        let exit = self.import(builder, module, self.imports.direct_exit_noarg_sub);
        let exit_call = builder
            .ins()
            .call(exit, &[self.run, self.state, body_status]);
        let exit_status = builder.inst_results(exit_call)[0];
        self.return_if_not_ok(builder, exit_status);
        Ok(())
    }

    fn lower_direct_ignored_two_i32_function_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let [OxArg::ByVal(arg0), OxArg::ByVal(arg1)] = args else {
            return Err(JitError::Compile(
                "direct ignored two-i32 Function call was selected without two ByVal args"
                    .to_string(),
            ));
        };
        let arg0 = self.lower_operand_i32(builder, module, arg0)?;
        let arg1 = self.lower_operand_i32(builder, module, arg1)?;
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let enter = self.import(builder, module, self.imports.direct_enter_two_i32_func);
        let enter_call = builder
            .ins()
            .call(enter, &[self.run, self.state, proc_value, arg0, arg1]);
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        let exit = self.import(builder, module, self.imports.direct_exit_noarg_sub);
        let exit_call = builder
            .ins()
            .call(exit, &[self.run, self.state, body_status]);
        let exit_status = builder.inst_results(exit_call)[0];
        self.return_if_not_ok(builder, exit_status);
        Ok(())
    }

    fn lower_direct_noarg_function_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
        dst: OxPlace,
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let enter = self.import(builder, module, self.imports.direct_enter_noarg_func);
        let enter_call = builder
            .ins()
            .call(enter, &[self.run, self.state, proc_value]);
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        let (dst_area, dst_index) = place_addr(dst);
        let dst_area = builder.ins().iconst(types::I32, i64::from(dst_area));
        let dst_index = builder.ins().iconst(types::I32, dst_index as i64);
        let exit = self.import(builder, module, self.imports.direct_exit_noarg_func);
        let exit_call = builder.ins().call(
            exit,
            &[
                self.run,
                self.state,
                proc_value,
                body_status,
                dst_area,
                dst_index,
            ],
        );
        let exit_status = builder.inst_results(exit_call)[0];
        self.return_if_not_ok(builder, exit_status);
        Ok(())
    }

    fn lower_direct_descriptor_static_call(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        proc: FuncId,
        args_len: usize,
        args_ptr: Value,
        dst: Option<OxPlace>,
    ) -> Result<(), JitError> {
        let Some(callee_id) = self.clif_ids.get(proc.0).copied() else {
            return Err(JitError::Compile(format!(
                "direct call target {} out of range",
                proc.0
            )));
        };
        let proc_value = builder.ins().iconst(types::I32, proc.0 as i64);
        let argc = i32::try_from(args_len)
            .map_err(|_| JitError::unsupported("M4-4 call argument count is too large"))?;
        let argc = builder.ins().iconst(types::I32, i64::from(argc));
        let enter = self.import(builder, module, self.imports.direct_enter_proc_i32);
        let enter_call = builder
            .ins()
            .call(enter, &[self.run, self.state, proc_value, argc, args_ptr]);
        let enter_status = builder.inst_results(enter_call)[0];
        self.return_if_not_ok(builder, enter_status);

        let callee = self.import(builder, module, callee_id);
        let body_call = builder.ins().call(callee, &[self.run, self.state]);
        let body_status = builder.inst_results(body_call)[0];

        if let Some(dst) = dst {
            let (dst_area, dst_index) = place_addr(dst);
            let dst_area = builder.ins().iconst(types::I32, i64::from(dst_area));
            let dst_index = builder.ins().iconst(types::I32, dst_index as i64);
            let exit = self.import(builder, module, self.imports.direct_exit_noarg_func);
            let exit_call = builder.ins().call(
                exit,
                &[
                    self.run,
                    self.state,
                    proc_value,
                    body_status,
                    dst_area,
                    dst_index,
                ],
            );
            let exit_status = builder.inst_results(exit_call)[0];
            self.return_if_not_ok(builder, exit_status);
        } else {
            let exit = self.import(builder, module, self.imports.direct_exit_noarg_sub);
            let exit_call = builder
                .ins()
                .call(exit, &[self.run, self.state, body_status]);
            let exit_status = builder.inst_results(exit_call)[0];
            self.return_if_not_ok(builder, exit_status);
        }
        Ok(())
    }

    fn lower_call_proc(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: Option<OxPlace>,
        proc: FuncId,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<(), JitError> {
        let callee = self
            .program
            .funcs
            .get(proc.0)
            .ok_or_else(|| JitError::Compile(format!("call target {} out of range", proc.0)))?;
        if self.can_lower_direct_noarg_sub_call(callee, dst, args) {
            return self.lower_direct_noarg_sub_call(builder, module, proc);
        }
        if self.can_lower_direct_one_i32_sub_call(callee, dst, args) {
            return self.lower_direct_one_i32_sub_call(builder, module, proc, args);
        }
        if self.can_lower_direct_one_i32_byref_sub_call(callee, dst, args) {
            return self.lower_direct_one_i32_byref_sub_call(builder, module, proc, args);
        }
        if self.can_lower_direct_two_i32_sub_call(callee, dst, args) {
            return self.lower_direct_two_i32_sub_call(builder, module, proc, args);
        }
        if dst.is_none() && self.can_lower_direct_ignored_noarg_function_call(callee, args) {
            return self.lower_direct_ignored_noarg_function_call(builder, module, proc);
        }
        if dst.is_none() && self.can_lower_direct_ignored_one_i32_function_call(callee, args)? {
            return self.lower_direct_ignored_one_i32_function_call(builder, module, proc, args);
        }
        if dst.is_none()
            && self.can_lower_direct_ignored_one_i32_byref_function_call(callee, args)?
        {
            return self
                .lower_direct_ignored_one_i32_byref_function_call(builder, module, proc, args);
        }
        if dst.is_none() && self.can_lower_direct_ignored_two_i32_function_call(callee, args)? {
            return self.lower_direct_ignored_two_i32_function_call(builder, module, proc, args);
        }
        if let Some(dst) = dst
            && self.can_lower_direct_noarg_function_call(callee, dst, args)?
        {
            return self.lower_direct_noarg_function_call(builder, module, proc, dst);
        }
        if let Some(dst) = dst
            && self.can_lower_direct_one_i32_function_call(callee, dst, args)?
        {
            return self.lower_direct_one_i32_function_call(builder, module, proc, args, dst);
        }
        if let Some(dst) = dst
            && self.can_lower_direct_one_i32_byref_function_call(callee, dst, args)?
        {
            return self.lower_direct_one_i32_byref_function_call(builder, module, proc, args, dst);
        }
        if let Some(dst) = dst
            && self.can_lower_direct_two_i32_function_call(callee, dst, args)?
        {
            return self.lower_direct_two_i32_function_call(builder, module, proc, args, dst);
        }
        if let Some(dst) = dst {
            let Some(ret) = callee.return_local else {
                return Err(JitError::unsupported(format!(
                    "call destination was requested for Sub {}",
                    callee.name
                )));
            };
            let Some(ret_ty) = callee.locals.get(ret.0).map(|local| &local.ty) else {
                return Err(JitError::Compile(format!(
                    "return local {} out of range in {}",
                    ret.0, callee.name
                )));
            };
            if !is_jit_static_call_ty(ret_ty) {
                return Err(JitError::unsupported(format!(
                    "JIT static call path accepts every supported scalar/carrier return in {}, got {ret_ty:?}",
                    callee.name
                )));
            }
            let dst_ty = place_ty(self.program, self.func, dst)?;
            if !is_m4_4_call_return_destination_ty(dst_ty, ret_ty) {
                return Err(JitError::unsupported(format!(
                    "M4-4 static call subset requires exact return destination type match or a Variant destination, got {dst_ty:?} for {ret_ty:?}"
                )));
            }
        }
        if args.len() != callee.param_count {
            return Err(JitError::unsupported(format!(
                "call argument count {} does not match callee param_count {} for {}",
                args.len(),
                callee.param_count,
                callee.name
            )));
        }
        for (index, arg) in args.iter().enumerate() {
            validate_static_scalar_call_arg(callee, index, arg)?;
        }
        let lowered_args = self.lower_static_call_args(builder, module, callee, args)?;
        let args_ptr = self.emit_call_arg_descriptors(builder, module, &lowered_args)?;

        self.lower_direct_descriptor_static_call(builder, module, proc, args.len(), args_ptr, dst)
    }

    fn lower_call_extern(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: Option<OxPlace>,
        import: oxvba_oxir::ImportId,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<(), JitError> {
        if let Some((target_program, target_proc)) =
            resolve_cross_project_proc_import(self.programs, self.program, import)?
        {
            return self.lower_cross_project_call_extern(
                builder,
                module,
                dst,
                target_program,
                target_proc,
                args,
            );
        }
        let (native_id, native_impl, string_typed_alias) =
            resolve_vba_library_import(self.program, import)?;
        if native_impl == NativeImplId::TypeName
            && let Some(dst) = dst
            && let [OxArg::ByVal(object)] = args
            && self.should_route_project_type_name(object)?
        {
            return self.emit_project_type_name_to_slot(builder, module, dst, object);
        }
        self.validate_call_extern_library_shape(dst, args, native_impl)?;
        if let Some(dst) = dst {
            let dst_ty = place_ty(self.program, self.func, dst)?;
            if !is_m4_4_call_extern_destination_ty(dst_ty) {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern library subset lowers only scalar/Variant destinations, got {dst_ty:?}"
                )));
            }
        }
        let lowered_args = self.lower_extern_args(builder, args)?;
        let args_ptr = self.emit_variant_operand_descriptors(builder, module, &lowered_args)?;
        let argc = i32::try_from(args.len())
            .map_err(|_| JitError::unsupported("M4-4 CallExtern argument count is too large"))?;
        let (dst_area, dst_index) = if let Some(dst) = dst {
            let (area, index) = place_addr(dst);
            (
                builder.ins().iconst(types::I32, i64::from(area)),
                builder.ins().iconst(types::I32, i64::from(index)),
            )
        } else {
            (
                builder.ins().iconst(types::I32, -1),
                builder.ins().iconst(types::I32, -1),
            )
        };
        let native_id = builder.ins().iconst(types::I32, i64::from(native_id));
        let string_typed_alias = builder.ins().iconst(
            types::I32,
            i64::from(if string_typed_alias { 1 } else { 0 }),
        );
        let argc = builder.ins().iconst(types::I32, i64::from(argc));
        let callee = self.import(builder, module, self.imports.lib_invoke_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                native_id,
                string_typed_alias,
                args_ptr,
                argc,
                dst_area,
                dst_index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn lower_cross_project_call_extern(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: Option<OxPlace>,
        target_program: usize,
        target_proc: FuncId,
        args: &[oxvba_oxir::OxArg],
    ) -> Result<(), JitError> {
        let callee = self
            .programs
            .get(target_program)
            .and_then(|program| program.funcs.get(target_proc.0))
            .ok_or_else(|| {
                JitError::Compile(format!(
                    "cross-project CallExtern target {}:{} out of range",
                    target_program, target_proc.0
                ))
            })?;
        if let Some(dst) = dst {
            let Some(ret) = callee.return_local else {
                return Err(JitError::unsupported(format!(
                    "cross-project CallExtern destination was requested for Sub {}",
                    callee.name
                )));
            };
            let Some(ret_ty) = callee.locals.get(ret.0).map(|local| &local.ty) else {
                return Err(JitError::Compile(format!(
                    "return local {} out of range in {}",
                    ret.0, callee.name
                )));
            };
            if !is_jit_static_call_ty(ret_ty) {
                return Err(JitError::unsupported(format!(
                    "JIT cross-project CallExtern accepts supported scalar/carrier returns in {}, got {ret_ty:?}",
                    callee.name
                )));
            }
            let dst_ty = place_ty(self.program, self.func, dst)?;
            if !is_m4_4_call_return_destination_ty(dst_ty, ret_ty) {
                return Err(JitError::unsupported(format!(
                    "M4-4 cross-project CallExtern requires exact return destination type match or a Variant destination, got {dst_ty:?} for {ret_ty:?}"
                )));
            }
        }
        if args.len() != callee.param_count {
            return Err(JitError::unsupported(format!(
                "cross-project CallExtern argument count {} does not match callee param_count {} for {}",
                args.len(),
                callee.param_count,
                callee.name
            )));
        }
        for (index, arg) in args.iter().enumerate() {
            validate_static_scalar_call_arg(callee, index, arg)?;
        }
        let lowered_args = self.lower_static_call_args(builder, module, callee, args)?;
        let args_ptr = self.emit_call_arg_descriptors(builder, module, &lowered_args)?;
        let argc = i32::try_from(args.len()).map_err(|_| {
            JitError::unsupported("M4-4 cross-project CallExtern argument count is too large")
        })?;
        let target_program = i32::try_from(target_program)
            .map_err(|_| JitError::unsupported("JIT target program index is too large"))?;
        let target_proc = i32::try_from(target_proc.0)
            .map_err(|_| JitError::unsupported("JIT target proc index is too large"))?;
        let target_program = builder.ins().iconst(types::I32, i64::from(target_program));
        let target_proc = builder.ins().iconst(types::I32, i64::from(target_proc));
        let argc = builder.ins().iconst(types::I32, i64::from(argc));
        let (dst_area, dst_index) = if let Some(dst) = dst {
            let (area, index) = place_addr(dst);
            (
                builder.ins().iconst(types::I32, i64::from(area)),
                builder.ins().iconst(types::I32, i64::from(index)),
            )
        } else {
            (
                builder.ins().iconst(types::I32, -1),
                builder.ins().iconst(types::I32, -1),
            )
        };
        let callee = self.import(builder, module, self.imports.call_extern_proc_i32);
        let call = builder.ins().call(
            callee,
            &[
                self.run,
                self.state,
                target_program,
                target_proc,
                argc,
                args_ptr,
                dst_area,
                dst_index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn lower_call_native(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        dst: Option<OxPlace>,
        callee: &OxNativeCallee,
        args: &[OxCallArg],
    ) -> Result<(), JitError> {
        let OxNativeCallee::Builtin(native_impl) = callee else {
            return Err(JitError::unsupported(
                "M4-4 CallNative lowers only a narrow built-in subset; Declare/native external calls remain unsupported",
            ));
        };
        if *native_impl == NativeImplId::TypeName
            && let Some(dst) = dst
            && let [OxCallArg::Operand(object)] = args
            && self.should_route_project_type_name(object)?
        {
            return self.emit_project_type_name_to_slot(builder, module, dst, object);
        }
        self.validate_call_native_builtin_shape(dst, args, *native_impl)?;
        if let Some(dst) = dst {
            let dst_ty = place_ty(self.program, self.func, dst)?;
            if !is_m4_4_call_extern_destination_ty(dst_ty) {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallNative built-in subset lowers only scalar/String/Variant destinations, got {dst_ty:?}"
                )));
            }
        }
        let lowered_args = self.lower_call_native_args(builder, args)?;
        let args_ptr = self.emit_variant_operand_descriptors(builder, module, &lowered_args)?;
        let argc = i32::try_from(args.len())
            .map_err(|_| JitError::unsupported("M4-4 CallNative argument count is too large"))?;
        let (dst_area, dst_index) = if let Some(dst) = dst {
            let (area, index) = place_addr(dst);
            (
                builder.ins().iconst(types::I32, i64::from(area)),
                builder.ins().iconst(types::I32, i64::from(index)),
            )
        } else {
            (
                builder.ins().iconst(types::I32, -1),
                builder.ins().iconst(types::I32, -1),
            )
        };
        let native_id = builder
            .ins()
            .iconst(types::I32, i64::from(native_impl_index(*native_impl)?));
        let string_typed_alias = builder.ins().iconst(types::I32, 0);
        let argc = builder.ins().iconst(types::I32, i64::from(argc));
        let callee = self.import(builder, module, self.imports.lib_invoke_slot);
        let call = builder.ins().call(
            callee,
            &[
                self.state,
                self.run,
                native_id,
                string_typed_alias,
                args_ptr,
                argc,
                dst_area,
                dst_index,
            ],
        );
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn validate_call_extern_library_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
    ) -> Result<(), JitError> {
        if let Some(shape) = scalar_optional_fixed_call_extern_shape(native_impl)
            && args.len() > 1
        {
            return self.validate_call_extern_scalar_optional_fixed_shape(
                dst,
                args,
                native_impl,
                shape,
            );
        }
        if let Some(shape) = date_part_optional_fixed_call_extern_shape(native_impl)
            && args.len() > 1
        {
            return self.validate_call_extern_date_part_optional_fixed_shape(
                dst,
                args,
                native_impl,
                shape,
            );
        }
        if let Some(shape) = date_name_optional_call_extern_shape(native_impl)
            && args.len() > 1
        {
            return self.validate_call_extern_date_name_optional_shape(
                dst,
                args,
                native_impl,
                shape,
            );
        }
        if let Some(shape) = random_call_extern_shape(native_impl) {
            return self.validate_call_extern_random_shape(dst, args, native_impl, shape);
        }
        if let Some(shape) = scalar_unary_call_extern_shape(native_impl) {
            return self.validate_call_extern_scalar_unary_shape(dst, args, native_impl, shape);
        }
        if let Some(shape) = scalar_double_call_extern_shape(native_impl) {
            return self.validate_call_extern_scalar_double_shape(dst, args, native_impl, shape);
        }
        if args.len() == 2
            && let Some(shape) = variant_double_call_extern_shape(native_impl)
        {
            return self.validate_call_extern_variant_double_shape(dst, args, shape);
        }
        if args.len() == 3
            && let Some(shape) = variant_triple_call_extern_shape(native_impl)
        {
            return self.validate_call_extern_variant_triple_shape(dst, args, native_impl, shape);
        }
        if args.len() == 4
            && let Some(shape) = variant_quad_call_extern_shape(native_impl)
        {
            return self.validate_call_extern_variant_quad_shape(dst, args, shape);
        }
        if let Some(shape) = variant_string_optional_call_extern_shape(native_impl) {
            return self.validate_call_extern_variant_string_optional_shape(
                dst,
                args,
                native_impl,
                shape,
            );
        }
        if args.len() == 2
            && let Some(shape) = variant_fixed_double_call_extern_shape(native_impl)
        {
            return self.validate_call_extern_variant_fixed_double_shape(
                dst,
                args,
                native_impl,
                shape,
            );
        }
        if let Some(shape) = variant_fixed_triple_call_extern_shape(native_impl) {
            return self.validate_call_extern_variant_fixed_triple_shape(
                dst,
                args,
                native_impl,
                shape,
            );
        }
        if let Some(shape) = date_interval_call_extern_shape(native_impl) {
            return self.validate_call_extern_date_interval_shape(dst, args, native_impl, shape);
        }
        if let Some(shape) = scalar_triple_call_extern_shape(native_impl) {
            return self.validate_call_extern_scalar_triple_shape(dst, args, native_impl, shape);
        }
        Err(JitError::unsupported(format!(
            "M4-4 CallExtern library subset currently lowers only scalar one-argument, fixed-integer two-argument, scalar/fixed-integer two-argument, selected optional date-part, selected optional date-name, selected random, variant two-argument, variant three-argument, selected variant four-argument, selected optional string-search/replacement, variant/fixed-integer two-argument, variant/fixed-integer three-argument, selected DateAdd/DateDiff, or fixed-integer three-argument built-ins, got {native_impl:?}"
        )))
    }

    fn validate_call_native_builtin_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[OxCallArg],
        native_impl: NativeImplId,
    ) -> Result<(), JitError> {
        let shape = match native_impl {
            NativeImplId::Like => "Like(Variant, Variant, compare-const)",
            NativeImplId::MidStmt => "MidStmt(target, start[, count], value)",
            NativeImplId::CreateObject => "CreateObject(\"OxVba.TestDispatch\")",
            NativeImplId::DebugPrint => "Debug.Print(value)",
            _ => {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallNative built-in subset currently lowers only Like, MidStmt, fixture-backed CreateObject, and Debug.Print, got {native_impl:?}"
                )));
            }
        };
        match native_impl {
            NativeImplId::Like => {
                self.validate_call_extern_variant_destination(dst, shape)?;
                if args.len() != 3 {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallNative {shape} subset requires exactly three arguments, got {}",
                        args.len()
                    )));
                }
                for arg in &args[..2] {
                    let OxCallArg::Operand(operand) = arg else {
                        return Err(JitError::unsupported(format!(
                            "M4-4 CallNative {shape} subset lowers only Variant operands followed by a compare constant"
                        )));
                    };
                    if !self.is_supported_variant_extern_operand(operand)? {
                        return Err(JitError::unsupported(format!(
                            "M4-4 CallNative {shape} subset lowers only supported Variant operands, got {operand:?}"
                        )));
                    }
                }
                match args[2] {
                    OxCallArg::Const(0 | 1) => Ok(()),
                    ref other => Err(JitError::unsupported(format!(
                        "M4-4 CallNative {shape} subset requires a binary/text compare constant, got {other:?}"
                    ))),
                }
            }
            NativeImplId::MidStmt => {
                self.validate_call_native_mid_stmt_destination(dst, shape)?;
                if !(args.len() == 3 || args.len() == 4) {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallNative {shape} subset requires three or four arguments, got {}",
                        args.len()
                    )));
                }
                for arg in args {
                    let OxCallArg::Operand(operand) = arg else {
                        return Err(JitError::unsupported(format!(
                            "M4-4 CallNative {shape} subset lowers only ordinary operands"
                        )));
                    };
                    if !self.is_supported_call_native_mid_stmt_operand(operand)? {
                        return Err(JitError::unsupported(format!(
                            "M4-4 CallNative {shape} subset lowers only supported string/scalar operands, got {operand:?}"
                        )));
                    }
                }
                Ok(())
            }
            NativeImplId::CreateObject => {
                self.validate_call_extern_variant_destination(dst, shape)?;
                if args.len() != 1 {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallNative {shape} subset requires exactly one argument, got {}",
                        args.len()
                    )));
                }
                let OxCallArg::Operand(OxOperand::Const(OxConst::Str(prog_id))) = &args[0] else {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallNative {shape} subset requires a literal ProgID"
                    )));
                };
                if prog_id.eq_ignore_ascii_case("OxVba.TestDispatch") {
                    Ok(())
                } else {
                    Err(JitError::unsupported(format!(
                        "M4-4 CallNative {shape} subset is limited to OxVba.TestDispatch, got {prog_id:?}"
                    )))
                }
            }
            NativeImplId::DebugPrint => {
                if dst.is_some() {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallNative {shape} subset lowers only statement destinations"
                    )));
                }
                for arg in args {
                    let OxCallArg::Operand(operand) = arg else {
                        return Err(JitError::unsupported(format!(
                            "M4-4 CallNative {shape} subset lowers only ordinary operands"
                        )));
                    };
                    if !self.is_supported_variant_extern_operand(operand)? {
                        return Err(JitError::unsupported(format!(
                            "M4-4 CallNative {shape} subset lowers only supported scalar/String/Variant operands, got {operand:?}"
                        )));
                    }
                }
                Ok(())
            }
            _ => unreachable!("validated CallNative built-in shape above"),
        }
    }

    fn validate_call_native_mid_stmt_destination(
        &self,
        dst: Option<OxPlace>,
        shape: &'static str,
    ) -> Result<(), JitError> {
        let Some(dst) = dst else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallNative {shape} subset requires a destination"
            )));
        };
        let dst_ty = place_ty(self.program, self.func, dst)?;
        if !matches!(dst_ty, OxTy::Str | OxTy::FixedStr(_) | OxTy::Variant) {
            return Err(JitError::unsupported(format!(
                "M4-4 CallNative {shape} subset requires a String/FixedString/Variant destination, got {dst_ty:?}"
            )));
        }
        Ok(())
    }

    fn is_supported_call_native_mid_stmt_operand(
        &self,
        operand: &OxOperand,
    ) -> Result<bool, JitError> {
        match operand {
            OxOperand::Const(
                OxConst::Empty
                | OxConst::Null
                | OxConst::Bool(_)
                | OxConst::I16(_)
                | OxConst::I32(_)
                | OxConst::I64(_)
                | OxConst::F32(_)
                | OxConst::F64(_)
                | OxConst::Currency(_)
                | OxConst::Date(_),
            ) => Ok(true),
            OxOperand::Const(OxConst::Str(value)) => Ok(i32::try_from(value.len()).is_ok()),
            OxOperand::Use(place) => Ok(is_m4_4_variant_descriptor_operand_ty(place_ty(
                self.program,
                self.func,
                *place,
            )?)),
            _ => Ok(false),
        }
    }

    fn validate_call_extern_variant_destination(
        &self,
        dst: Option<OxPlace>,
        shape: &'static str,
    ) -> Result<(), JitError> {
        let Some(dst) = dst else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset requires a Variant destination"
            )));
        };
        let dst_ty = place_ty(self.program, self.func, dst)?;
        if !matches!(dst_ty, OxTy::Variant) {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset requires a Variant destination, got {dst_ty:?}"
            )));
        }
        Ok(())
    }

    fn validate_call_extern_scalar_unary_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        if args.len() != 1 {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset requires exactly one argument, got {}",
                args.len()
            )));
        }
        let OxArg::ByVal(arg) = &args[0] else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only supported ByVal scalar arguments"
            )));
        };
        if !self.is_supported_scalar_unary_extern_operand(arg, native_impl)? {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only supported ByVal scalar arguments, got {arg:?}"
            )));
        }
        Ok(())
    }

    fn validate_call_extern_scalar_double_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        if args.len() != 2 {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset requires exactly two arguments, got {}",
                args.len()
            )));
        }
        for arg in args {
            let OxArg::ByVal(operand) = arg else {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only supported ByVal fixed-integer scalar arguments"
                )));
            };
            if !self.is_supported_scalar_double_extern_operand(operand, native_impl)? {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only supported ByVal fixed-integer scalar arguments, got {operand:?}"
                )));
            }
        }
        Ok(())
    }

    fn validate_call_extern_scalar_optional_fixed_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        if args.len() != 2 {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset requires exactly two arguments, got {}",
                args.len()
            )));
        }
        let OxArg::ByVal(value) = &args[0] else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only a ByVal scalar numeric argument"
            )));
        };
        if !self.is_supported_scalar_optional_fixed_value(value, native_impl)? {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only supported ByVal scalar numeric arguments, got {value:?}"
            )));
        }
        self.validate_call_extern_byval_fixed_integer_operand(&args[1], shape, "optional digits")
    }

    fn validate_call_extern_date_name_optional_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        match native_impl {
            NativeImplId::MonthName => {
                if args.len() != 2 {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallExtern {shape} subset requires exactly two arguments, got {}",
                        args.len()
                    )));
                }
                self.validate_call_extern_byval_fixed_integer_operand(&args[0], shape, "month")?;
                self.validate_call_extern_byval_bool_operand(&args[1], shape, "abbreviate")?;
            }
            NativeImplId::WeekdayName => {
                if args.len() != 3 {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallExtern {shape} subset requires exactly three arguments, got {}",
                        args.len()
                    )));
                }
                self.validate_call_extern_byval_fixed_integer_operand(&args[0], shape, "weekday")?;
                self.validate_call_extern_byval_bool_operand(&args[1], shape, "abbreviate")?;
                self.validate_call_extern_byval_fixed_integer_operand(&args[2], shape, "firstday")?;
            }
            _ => {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern optional date-name subset does not cover {native_impl:?}"
                )));
            }
        }
        Ok(())
    }

    fn validate_call_extern_date_part_optional_fixed_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        if args.len() != 2 {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset requires exactly two arguments, got {}",
                args.len()
            )));
        }
        if !matches!(native_impl, NativeImplId::Weekday) {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern optional date-part subset does not cover {native_impl:?}"
            )));
        }
        let OxArg::ByVal(date) = &args[0] else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only a ByVal Date first argument"
            )));
        };
        if !self.is_supported_date_part_extern_operand(date)? {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only supported Date first arguments, got {date:?}"
            )));
        }
        self.validate_call_extern_byval_fixed_integer_operand(&args[1], shape, "firstday")
    }

    fn validate_call_extern_random_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
        shape: &'static str,
    ) -> Result<(), JitError> {
        if let Some(dst) = dst {
            self.validate_call_extern_variant_destination(Some(dst), shape)?;
        }
        if args.len() > 1 {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset accepts at most one argument, got {}",
                args.len()
            )));
        }
        if let Some(arg) = args.first() {
            let OxArg::ByVal(operand) = arg else {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only an optional ByVal numeric argument"
                )));
            };
            if !self.is_supported_random_extern_operand(operand, native_impl)? {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only supported optional ByVal numeric arguments, got {operand:?}"
                )));
            }
        }
        Ok(())
    }

    fn validate_call_extern_variant_double_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        if args.len() != 2 {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset requires exactly two arguments, got {}",
                args.len()
            )));
        }
        for arg in args {
            let OxArg::ByVal(operand) = arg else {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only supported ByVal Variant arguments"
                )));
            };
            if !self.is_supported_variant_extern_operand(operand)? {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only supported ByVal Variant arguments, got {operand:?}"
                )));
            }
        }
        Ok(())
    }

    fn validate_call_extern_variant_triple_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        if args.len() != 3 {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset requires exactly three arguments, got {}",
                args.len()
            )));
        }
        for arg in args {
            let OxArg::ByVal(operand) = arg else {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only supported ByVal Variant arguments"
                )));
            };
            if !self.is_supported_variant_triple_extern_operand(operand, native_impl)? {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only supported ByVal Variant/scalar arguments, got {operand:?}"
                )));
            }
        }
        Ok(())
    }

    fn validate_call_extern_variant_quad_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        if args.len() != 4 {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset requires exactly four arguments, got {}",
                args.len()
            )));
        }
        for arg in args {
            let OxArg::ByVal(operand) = arg else {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only supported ByVal Variant arguments"
                )));
            };
            if !self.is_supported_variant_extern_operand(operand)? {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only supported ByVal Variant arguments, got {operand:?}"
                )));
            }
        }
        Ok(())
    }

    fn validate_call_extern_variant_string_optional_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        match native_impl {
            NativeImplId::InStr => {
                if !(3..=4).contains(&args.len()) {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallExtern {shape} subset requires three or four arguments, got {}",
                        args.len()
                    )));
                }
                self.validate_call_extern_byval_fixed_integer_operand(&args[0], shape, "start")?;
                self.validate_call_extern_byval_string_operand(&args[1], shape, "string1")?;
                self.validate_call_extern_byval_string_operand(&args[2], shape, "string2")?;
                if args.len() == 4 {
                    self.validate_call_extern_byval_fixed_integer_operand(
                        &args[3], shape, "compare",
                    )?;
                }
            }
            NativeImplId::InStrRev => {
                if !(3..=4).contains(&args.len()) {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallExtern {shape} subset requires three or four arguments, got {}",
                        args.len()
                    )));
                }
                self.validate_call_extern_byval_string_operand(&args[0], shape, "stringcheck")?;
                self.validate_call_extern_byval_string_operand(&args[1], shape, "stringmatch")?;
                self.validate_call_extern_optional_fixed_integer_operand(&args[2], shape, "start")?;
                if args.len() == 4 {
                    self.validate_call_extern_byval_fixed_integer_operand(
                        &args[3], shape, "compare",
                    )?;
                }
            }
            NativeImplId::StrComp => {
                if args.len() != 3 {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallExtern {shape} subset requires exactly three arguments, got {}",
                        args.len()
                    )));
                }
                self.validate_call_extern_byval_string_operand(&args[0], shape, "string1")?;
                self.validate_call_extern_byval_string_operand(&args[1], shape, "string2")?;
                self.validate_call_extern_byval_fixed_integer_operand(&args[2], shape, "compare")?;
            }
            NativeImplId::Replace => {
                if !(4..=6).contains(&args.len()) {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallExtern {shape} subset requires four to six arguments, got {}",
                        args.len()
                    )));
                }
                self.validate_call_extern_byval_string_operand(&args[0], shape, "expression")?;
                self.validate_call_extern_byval_string_operand(&args[1], shape, "find")?;
                self.validate_call_extern_byval_string_operand(&args[2], shape, "replace")?;
                self.validate_call_extern_optional_fixed_integer_operand(&args[3], shape, "start")?;
                if args.len() >= 5 {
                    self.validate_call_extern_optional_fixed_integer_operand(
                        &args[4], shape, "count",
                    )?;
                }
                if args.len() == 6 {
                    self.validate_call_extern_byval_fixed_integer_operand(
                        &args[5], shape, "compare",
                    )?;
                }
            }
            _ => {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern optional string subset does not cover {native_impl:?}"
                )));
            }
        }
        Ok(())
    }

    fn validate_call_extern_variant_fixed_double_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        if args.len() != 2 {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset requires exactly two arguments, got {}",
                args.len()
            )));
        }
        let OxArg::ByVal(source) = &args[0] else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only a ByVal descriptor-supported Variant/string source"
            )));
        };
        if !self.is_supported_variant_extern_operand(source)? {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only a ByVal descriptor-supported Variant/string source, got {source:?}"
            )));
        }
        let OxArg::ByVal(count) = &args[1] else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only a ByVal fixed-integer index/count operand"
            )));
        };
        if !self.is_supported_variant_fixed_double_extern_count(count, native_impl)? {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only a ByVal fixed-integer index/count operand, got {count:?}"
            )));
        }
        Ok(())
    }

    fn validate_call_extern_variant_fixed_triple_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        if args.len() != 3 {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset requires exactly three arguments, got {}",
                args.len()
            )));
        }
        let OxArg::ByVal(source) = &args[0] else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only a ByVal descriptor-supported Variant/string source"
            )));
        };
        if !self.is_supported_variant_extern_operand(source)? {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only a ByVal descriptor-supported Variant/string source, got {source:?}"
            )));
        }
        for arg in &args[1..] {
            let OxArg::ByVal(count) = arg else {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only ByVal fixed-integer index/count operands"
                )));
            };
            if !self.is_supported_variant_fixed_triple_extern_operand(count, native_impl)? {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only ByVal fixed-integer index/count operands, got {count:?}"
                )));
            }
        }
        Ok(())
    }

    fn validate_call_extern_date_interval_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        match native_impl {
            NativeImplId::DateAdd => {
                if args.len() != 3 {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallExtern {shape} subset requires exactly three arguments, got {}",
                        args.len()
                    )));
                }
                self.validate_call_extern_byval_string_operand(&args[0], shape, "interval")?;
                self.validate_call_extern_byval_variant_operand(&args[1], shape, "number")?;
                self.validate_call_extern_byval_date_operand(&args[2], shape, "date")?;
            }
            NativeImplId::DateDiff => {
                if args.len() != 3 {
                    return Err(JitError::unsupported(format!(
                        "M4-4 CallExtern {shape} subset requires exactly three arguments, got {}",
                        args.len()
                    )));
                }
                self.validate_call_extern_byval_string_operand(&args[0], shape, "interval")?;
                self.validate_call_extern_byval_date_operand(&args[1], shape, "date1")?;
                self.validate_call_extern_byval_date_operand(&args[2], shape, "date2")?;
            }
            _ => {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern date-interval subset does not cover {native_impl:?}"
                )));
            }
        }
        Ok(())
    }

    fn validate_call_extern_byval_string_operand(
        &self,
        arg: &OxArg,
        shape: &'static str,
        role: &'static str,
    ) -> Result<(), JitError> {
        let OxArg::ByVal(operand) = arg else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only ByVal descriptor-supported string operands for {role}"
            )));
        };
        if !self.is_supported_variant_extern_operand(operand)? {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only descriptor-supported string operands for {role}, got {operand:?}"
            )));
        }
        Ok(())
    }

    fn validate_call_extern_byval_variant_operand(
        &self,
        arg: &OxArg,
        shape: &'static str,
        role: &'static str,
    ) -> Result<(), JitError> {
        let OxArg::ByVal(operand) = arg else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only ByVal descriptor-supported Variant operands for {role}"
            )));
        };
        if !self.is_supported_variant_extern_operand(operand)? {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only descriptor-supported Variant operands for {role}, got {operand:?}"
            )));
        }
        Ok(())
    }

    fn validate_call_extern_byval_date_operand(
        &self,
        arg: &OxArg,
        shape: &'static str,
        role: &'static str,
    ) -> Result<(), JitError> {
        let OxArg::ByVal(operand) = arg else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only ByVal Date/Variant operands for {role}"
            )));
        };
        if !self.is_supported_date_part_extern_operand(operand)? {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only Date/Variant operands for {role}, got {operand:?}"
            )));
        }
        Ok(())
    }

    fn validate_call_extern_byval_fixed_integer_operand(
        &self,
        arg: &OxArg,
        shape: &'static str,
        role: &'static str,
    ) -> Result<(), JitError> {
        let OxArg::ByVal(operand) = arg else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only ByVal fixed-integer operands for {role}"
            )));
        };
        if !self.is_supported_fixed_integer_extern_operand(operand)? {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only fixed-integer operands for {role}, got {operand:?}"
            )));
        }
        Ok(())
    }

    fn validate_call_extern_byval_bool_operand(
        &self,
        arg: &OxArg,
        shape: &'static str,
        role: &'static str,
    ) -> Result<(), JitError> {
        let OxArg::ByVal(operand) = arg else {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only ByVal Boolean operands for {role}"
            )));
        };
        if !self.is_supported_bool_extern_operand(operand)? {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset lowers only Boolean operands for {role}, got {operand:?}"
            )));
        }
        Ok(())
    }

    fn validate_call_extern_optional_fixed_integer_operand(
        &self,
        arg: &OxArg,
        shape: &'static str,
        role: &'static str,
    ) -> Result<(), JitError> {
        if matches!(arg, OxArg::Omitted) {
            return Ok(());
        }
        self.validate_call_extern_byval_fixed_integer_operand(arg, shape, role)
    }

    fn validate_call_extern_scalar_triple_shape(
        &self,
        dst: Option<OxPlace>,
        args: &[oxvba_oxir::OxArg],
        native_impl: NativeImplId,
        shape: &'static str,
    ) -> Result<(), JitError> {
        self.validate_call_extern_variant_destination(dst, shape)?;
        if args.len() != 3 {
            return Err(JitError::unsupported(format!(
                "M4-4 CallExtern {shape} subset requires exactly three arguments, got {}",
                args.len()
            )));
        }
        for arg in args {
            let OxArg::ByVal(operand) = arg else {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only supported ByVal fixed-integer scalar arguments"
                )));
            };
            if !self.is_supported_scalar_triple_extern_operand(operand, native_impl)? {
                return Err(JitError::unsupported(format!(
                    "M4-4 CallExtern {shape} subset lowers only supported ByVal fixed-integer scalar arguments, got {operand:?}"
                )));
            }
        }
        Ok(())
    }

    fn is_supported_scalar_unary_extern_operand(
        &self,
        operand: &OxOperand,
        native_impl: NativeImplId,
    ) -> Result<bool, JitError> {
        match native_impl {
            NativeImplId::Abs => self.is_supported_numeric_unary_extern_operand(operand, false),
            NativeImplId::Int | NativeImplId::Fix => {
                self.is_supported_numeric_or_variant_unary_extern_operand(operand, true)
            }
            NativeImplId::Sgn => self.is_supported_numeric_unary_extern_operand(operand, false),
            NativeImplId::CVErr => self.is_supported_numeric_unary_extern_operand(operand, false),
            NativeImplId::Hex | NativeImplId::Oct | NativeImplId::Str => {
                self.is_supported_non_null_numeric_unary_extern_operand(operand)
            }
            NativeImplId::Round
            | NativeImplId::Sqr
            | NativeImplId::Sin
            | NativeImplId::Cos
            | NativeImplId::Log
            | NativeImplId::Exp
            | NativeImplId::Atn
            | NativeImplId::Tan => self.is_supported_non_null_numeric_unary_extern_operand(operand),
            NativeImplId::Year
            | NativeImplId::Month
            | NativeImplId::Day
            | NativeImplId::Weekday
            | NativeImplId::Hour
            | NativeImplId::Minute
            | NativeImplId::Second => self.is_supported_date_part_extern_operand(operand),
            NativeImplId::DateValue | NativeImplId::TimeValue => {
                self.is_supported_date_value_time_value_extern_operand(operand)
            }
            NativeImplId::MonthName | NativeImplId::WeekdayName => {
                self.is_supported_fixed_integer_extern_operand(operand)
            }
            NativeImplId::QbColor => self.is_supported_fixed_integer_extern_operand(operand),
            NativeImplId::ErrorText => self.is_supported_fixed_integer_extern_operand(operand),
            NativeImplId::Chr | NativeImplId::ChrW | NativeImplId::Space => {
                self.is_supported_fixed_integer_extern_operand(operand)
            }
            NativeImplId::Asc | NativeImplId::AscW => {
                self.is_supported_variant_extern_operand(operand)
            }
            NativeImplId::LCase | NativeImplId::UCase => {
                self.is_supported_variant_extern_operand(operand)
            }
            NativeImplId::Trim | NativeImplId::LTrim | NativeImplId::RTrim => {
                self.is_supported_variant_extern_operand(operand)
            }
            NativeImplId::StrReverse => self.is_supported_variant_extern_operand(operand),
            NativeImplId::Val => self.is_supported_variant_extern_operand(operand),
            NativeImplId::Len | NativeImplId::LenB => {
                self.is_supported_variant_extern_operand(operand)
            }
            NativeImplId::Format => self.is_supported_variant_extern_operand(operand),
            NativeImplId::IsArray
            | NativeImplId::VarType
            | NativeImplId::TypeName
            | NativeImplId::IsNumeric
            | NativeImplId::IsDate
            | NativeImplId::IsObject
            | NativeImplId::IsNull
            | NativeImplId::IsEmpty
            | NativeImplId::IsError
            | NativeImplId::IsMissing => self.is_supported_information_extern_operand(operand),
            NativeImplId::CStr
            | NativeImplId::CDate
            | NativeImplId::CBool
            | NativeImplId::CByte
            | NativeImplId::CInt
            | NativeImplId::CLng
            | NativeImplId::CLngLng
            | NativeImplId::CLngPtr
            | NativeImplId::CSng
            | NativeImplId::CDbl
            | NativeImplId::CCur
            | NativeImplId::CDec
            | NativeImplId::CVar => {
                self.is_supported_conversion_extern_operand(operand, native_impl)
            }
            _ => Ok(false),
        }
    }

    fn is_supported_scalar_double_extern_operand(
        &self,
        operand: &OxOperand,
        native_impl: NativeImplId,
    ) -> Result<bool, JitError> {
        match native_impl {
            NativeImplId::StringRepeat => self.is_supported_fixed_integer_extern_operand(operand),
            _ => Ok(false),
        }
    }

    fn is_supported_scalar_optional_fixed_value(
        &self,
        operand: &OxOperand,
        native_impl: NativeImplId,
    ) -> Result<bool, JitError> {
        match native_impl {
            NativeImplId::Round => self.is_supported_non_null_numeric_unary_extern_operand(operand),
            _ => Ok(false),
        }
    }

    fn is_supported_bool_extern_operand(&self, operand: &OxOperand) -> Result<bool, JitError> {
        match operand {
            OxOperand::Const(OxConst::Bool(_)) => Ok(true),
            OxOperand::Use(place) => Ok(matches!(
                place_ty(self.program, self.func, *place)?,
                OxTy::Bool
            )),
            _ => Ok(false),
        }
    }

    fn is_supported_variant_fixed_double_extern_count(
        &self,
        operand: &OxOperand,
        native_impl: NativeImplId,
    ) -> Result<bool, JitError> {
        match native_impl {
            NativeImplId::Left
            | NativeImplId::LeftB
            | NativeImplId::Right
            | NativeImplId::RightB
            | NativeImplId::Mid
            | NativeImplId::StrConv => self.is_supported_fixed_integer_extern_operand(operand),
            _ => Ok(false),
        }
    }

    fn is_supported_variant_fixed_triple_extern_operand(
        &self,
        operand: &OxOperand,
        native_impl: NativeImplId,
    ) -> Result<bool, JitError> {
        match native_impl {
            NativeImplId::Mid => self.is_supported_fixed_integer_extern_operand(operand),
            _ => Ok(false),
        }
    }

    fn is_supported_scalar_triple_extern_operand(
        &self,
        operand: &OxOperand,
        native_impl: NativeImplId,
    ) -> Result<bool, JitError> {
        match native_impl {
            NativeImplId::DateSerial | NativeImplId::TimeSerial | NativeImplId::Rgb => {
                self.is_supported_fixed_integer_extern_operand(operand)
            }
            _ => Ok(false),
        }
    }

    fn is_supported_variant_triple_extern_operand(
        &self,
        operand: &OxOperand,
        native_impl: NativeImplId,
    ) -> Result<bool, JitError> {
        match native_impl {
            NativeImplId::Rate | NativeImplId::NPer => {
                self.is_supported_non_null_numeric_unary_extern_operand(operand)
            }
            _ => self.is_supported_variant_extern_operand(operand),
        }
    }

    fn is_supported_random_extern_operand(
        &self,
        operand: &OxOperand,
        native_impl: NativeImplId,
    ) -> Result<bool, JitError> {
        match native_impl {
            NativeImplId::Rnd | NativeImplId::Randomize => {
                self.is_supported_non_null_numeric_unary_extern_operand(operand)
            }
            _ => Ok(false),
        }
    }

    fn is_supported_conversion_extern_operand(
        &self,
        operand: &OxOperand,
        native_impl: NativeImplId,
    ) -> Result<bool, JitError> {
        match operand {
            OxOperand::Const(OxConst::Str(_)) => Ok(matches!(native_impl, NativeImplId::CDate)),
            OxOperand::Const(
                OxConst::Null
                | OxConst::Empty
                | OxConst::Bool(_)
                | OxConst::I16(_)
                | OxConst::I32(_)
                | OxConst::I64(_)
                | OxConst::F32(_)
                | OxConst::F64(_)
                | OxConst::Currency(_)
                | OxConst::Date(_),
            ) => Ok(true),
            OxOperand::Use(place) => {
                let ty = place_ty(self.program, self.func, *place)?;
                Ok(is_m4_4_variant_operand_ty(ty)
                    || (matches!(native_impl, NativeImplId::CDate) && matches!(ty, OxTy::Str)))
            }
            _ => Ok(false),
        }
    }

    fn is_supported_numeric_unary_extern_operand(
        &self,
        operand: &OxOperand,
        allow_date: bool,
    ) -> Result<bool, JitError> {
        match operand {
            OxOperand::Const(
                OxConst::Null
                | OxConst::Empty
                | OxConst::Bool(_)
                | OxConst::I16(_)
                | OxConst::I32(_)
                | OxConst::I64(_)
                | OxConst::F32(_)
                | OxConst::F64(_)
                | OxConst::Currency(_),
            ) => Ok(true),
            OxOperand::Const(OxConst::Date(_)) => Ok(allow_date),
            OxOperand::Use(place) => {
                let ty = place_ty(self.program, self.func, *place)?;
                Ok(matches!(
                    ty,
                    OxTy::Bool
                        | OxTy::Integer
                        | OxTy::Long
                        | OxTy::LongLong
                        | OxTy::Single
                        | OxTy::Double
                        | OxTy::Currency
                ) || (allow_date && matches!(ty, OxTy::Date)))
            }
            _ => Ok(false),
        }
    }

    fn is_supported_non_null_numeric_unary_extern_operand(
        &self,
        operand: &OxOperand,
    ) -> Result<bool, JitError> {
        match operand {
            OxOperand::Const(
                OxConst::I16(_)
                | OxConst::I32(_)
                | OxConst::I64(_)
                | OxConst::F32(_)
                | OxConst::F64(_)
                | OxConst::Currency(_),
            ) => Ok(true),
            OxOperand::Use(place) => {
                let ty = place_ty(self.program, self.func, *place)?;
                Ok(matches!(
                    ty,
                    OxTy::Integer
                        | OxTy::Long
                        | OxTy::LongLong
                        | OxTy::Single
                        | OxTy::Double
                        | OxTy::Currency
                ))
            }
            _ => Ok(false),
        }
    }

    fn is_supported_numeric_or_variant_unary_extern_operand(
        &self,
        operand: &OxOperand,
        allow_date: bool,
    ) -> Result<bool, JitError> {
        if self.is_supported_numeric_unary_extern_operand(operand, allow_date)? {
            return Ok(true);
        }
        match operand {
            OxOperand::Use(place) => Ok(matches!(
                place_ty(self.program, self.func, *place)?,
                OxTy::Variant
            )),
            _ => Ok(false),
        }
    }

    fn is_supported_date_part_extern_operand(&self, operand: &OxOperand) -> Result<bool, JitError> {
        match operand {
            OxOperand::Const(OxConst::Date(_)) => Ok(true),
            OxOperand::Use(place) => Ok(matches!(
                place_ty(self.program, self.func, *place)?,
                OxTy::Date | OxTy::Variant
            )),
            _ => Ok(false),
        }
    }

    fn is_supported_date_value_time_value_extern_operand(
        &self,
        operand: &OxOperand,
    ) -> Result<bool, JitError> {
        match operand {
            OxOperand::Const(OxConst::Str(value)) => Ok(i32::try_from(value.len()).is_ok()),
            _ => self.is_supported_date_part_extern_operand(operand),
        }
    }

    fn is_supported_fixed_integer_extern_operand(
        &self,
        operand: &OxOperand,
    ) -> Result<bool, JitError> {
        match operand {
            OxOperand::Const(OxConst::I16(_) | OxConst::I32(_) | OxConst::I64(_)) => Ok(true),
            OxOperand::Use(place) => Ok(matches!(
                place_ty(self.program, self.func, *place)?,
                OxTy::Byte | OxTy::Integer | OxTy::Long | OxTy::LongLong
            )),
            _ => Ok(false),
        }
    }

    fn is_supported_variant_extern_operand(&self, operand: &OxOperand) -> Result<bool, JitError> {
        match operand {
            OxOperand::Const(
                OxConst::Empty
                | OxConst::Null
                | OxConst::Bool(_)
                | OxConst::I16(_)
                | OxConst::I32(_)
                | OxConst::I64(_)
                | OxConst::F32(_)
                | OxConst::F64(_)
                | OxConst::Currency(_)
                | OxConst::Date(_),
            ) => Ok(true),
            OxOperand::Const(OxConst::Str(value)) => Ok(i32::try_from(value.len()).is_ok()),
            OxOperand::Use(place) => Ok(matches!(
                place_ty(self.program, self.func, *place)?,
                OxTy::Variant | OxTy::Str
            )),
            _ => Ok(false),
        }
    }

    fn is_supported_information_extern_operand(
        &self,
        operand: &OxOperand,
    ) -> Result<bool, JitError> {
        if self.is_supported_conversion_extern_operand(operand, NativeImplId::CVar)? {
            return Ok(true);
        }
        match operand {
            OxOperand::Const(OxConst::Str(value)) => Ok(i32::try_from(value.len()).is_ok()),
            OxOperand::Use(place) => Ok(is_m4_4_fixed_variant_array_ty(place_ty(
                self.program,
                self.func,
                *place,
            )?)),
            _ => Ok(false),
        }
    }

    fn lower_terminator(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
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
            OxTerminator::Branch {
                cond,
                then_blk,
                else_blk,
            } => {
                let cond = self.lower_operand_bool_i32(builder, module, cond)?;
                let zero = builder.ins().iconst(types::I32, 0);
                let taken = builder
                    .ins()
                    .icmp(ir::condcodes::IntCC::NotEqual, cond, zero);
                builder.ins().brif(
                    taken,
                    self.clif_block(*then_blk)?,
                    &[],
                    self.clif_block(*else_blk)?,
                    &[],
                );
                Ok(())
            }
            OxTerminator::FaultDispatch {
                resume,
                resume_next,
            } => {
                let dispatch_slot = builder.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    4,
                    2,
                ));
                let block_slot = builder.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    4,
                    2,
                ));
                let ptr_ty = module.target_config().pointer_type();
                let dispatch_out = builder.ins().stack_addr(ptr_ty, dispatch_slot, 0);
                let block_out = builder.ins().stack_addr(ptr_ty, block_slot, 0);
                let resume = builder.ins().iconst(types::I32, resume.0 as i64);
                let resume_next_raw = builder.ins().iconst(types::I32, resume_next.0 as i64);
                let current_line_callee = self.import(builder, module, self.imports.current_line);
                let current_line_call = builder.ins().call(current_line_callee, &[self.run]);
                let current_line = builder.inst_results(current_line_call)[0];
                let callee = self.import(builder, module, self.imports.route_fault);
                let call = builder.ins().call(
                    callee,
                    &[
                        self.state,
                        resume,
                        resume_next_raw,
                        current_line,
                        dispatch_out,
                        block_out,
                    ],
                );
                let status = builder.inst_results(call)[0];
                let ok =
                    builder
                        .ins()
                        .icmp_imm(ir::condcodes::IntCC::Equal, status, i64::from(ST_OK));
                let read_dispatch_block = builder.create_block();
                let route_failed_block = builder.create_block();
                builder
                    .ins()
                    .brif(ok, read_dispatch_block, &[], route_failed_block, &[]);

                builder.switch_to_block(route_failed_block);
                builder.ins().return_(&[status]);

                builder.switch_to_block(read_dispatch_block);
                let dispatch = builder.ins().stack_load(types::I32, dispatch_slot, 0);
                if !self.has_label_error_handler {
                    let resume_next_dispatch = builder.ins().icmp_imm(
                        ir::condcodes::IntCC::Equal,
                        dispatch,
                        i64::from(RT_FAULT_DISP_RESUME_NEXT),
                    );
                    let resume_next_block = builder.create_block();
                    let fault_block = builder.create_block();
                    builder.ins().brif(
                        resume_next_dispatch,
                        resume_next_block,
                        &[],
                        fault_block,
                        &[],
                    );

                    builder.switch_to_block(resume_next_block);
                    builder.ins().jump(self.clif_block(*resume_next)?, &[]);

                    builder.switch_to_block(fault_block);
                    let fault = builder.ins().iconst(types::I32, i64::from(ST_FAULT));
                    builder.ins().return_(&[fault]);
                    return Ok(());
                }

                let unwind = builder.ins().icmp_imm(
                    ir::condcodes::IntCC::Equal,
                    dispatch,
                    i64::from(RT_FAULT_DISP_UNWIND),
                );
                let unwind_block = builder.create_block();
                let routed_block = builder.create_block();
                let check_handler_block = builder.create_block();
                let bad_dispatch_block = builder.create_block();
                builder
                    .ins()
                    .brif(unwind, unwind_block, &[], check_handler_block, &[]);

                builder.switch_to_block(unwind_block);
                let fault = builder.ins().iconst(types::I32, i64::from(ST_FAULT));
                builder.ins().return_(&[fault]);

                builder.switch_to_block(check_handler_block);
                let resume_next = builder.ins().icmp_imm(
                    ir::condcodes::IntCC::Equal,
                    dispatch,
                    i64::from(RT_FAULT_DISP_RESUME_NEXT),
                );
                builder
                    .ins()
                    .brif(resume_next, routed_block, &[], bad_dispatch_block, &[]);

                builder.switch_to_block(bad_dispatch_block);
                let handler = builder.ins().icmp_imm(
                    ir::condcodes::IntCC::Equal,
                    dispatch,
                    i64::from(RT_FAULT_DISP_HANDLER),
                );
                let invalid_block = builder.create_block();
                builder
                    .ins()
                    .brif(handler, routed_block, &[], invalid_block, &[]);

                builder.switch_to_block(invalid_block);
                let fault = builder.ins().iconst(types::I32, i64::from(ST_FAULT));
                builder.ins().return_(&[fault]);

                builder.switch_to_block(routed_block);
                let block = builder.ins().stack_load(types::I32, block_slot, 0);
                self.emit_jump_to_runtime_block(builder, block)
            }
            OxTerminator::Resume => self.emit_resume(builder, module, RT_RESUME_SAME, 0),
            OxTerminator::ResumeNext => self.emit_resume(builder, module, RT_RESUME_NEXT, 0),
            OxTerminator::ResumeLabel(label) => {
                self.emit_resume(builder, module, RT_RESUME_LABEL, label.0)
            }
            OxTerminator::GoSub { target, ret } => {
                let ret = builder.ins().iconst(types::I32, ret.0 as i64);
                let callee = self.import(builder, module, self.imports.gosub_push);
                let call = builder.ins().call(callee, &[self.run, ret]);
                let status = builder.inst_results(call)[0];
                self.return_if_not_ok(builder, status);
                builder.ins().jump(self.clif_block(*target)?, &[]);
                Ok(())
            }
            OxTerminator::GoSubReturn => self.emit_gosub_return(builder, module),
            OxTerminator::Raise {
                number,
                source,
                description,
                help_file,
                help_context,
                inherit,
            } => {
                if source.is_some()
                    || description.is_some()
                    || help_file.is_some()
                    || help_context.is_some()
                {
                    return Err(JitError::unsupported(format!(
                        "M4-4 lowers only bare Err.Raise/Error with a constant number, got {term:?}"
                    )));
                }
                let number = match number {
                    OxOperand::Const(OxConst::I16(value)) => i32::from(*value),
                    OxOperand::Const(OxConst::I32(value)) => *value,
                    other => {
                        return Err(JitError::unsupported(format!(
                            "M4-4 lowers only bare Err.Raise with a constant Integer/Long number, got {other:?}"
                        )));
                    }
                };
                let callee = self.import(builder, module, self.imports.raise_error_number);
                let number = builder.ins().iconst(types::I32, i64::from(number));
                let inherit = builder.ins().iconst(types::I32, i64::from(*inherit as i32));
                let source_ptr = i64::try_from(self.program.unit_name.as_ptr() as usize)
                    .map_err(|_| JitError::unsupported("M4-4 unit name pointer exceeds i64"))?;
                let source_len = i32::try_from(self.program.unit_name.len())
                    .map_err(|_| JitError::unsupported("M4-4 unit name length exceeds i32"))?;
                let ptr_ty = module.target_config().pointer_type();
                let source_ptr = builder.ins().iconst(ptr_ty, source_ptr);
                let source_len = builder.ins().iconst(types::I32, i64::from(source_len));
                let call = builder.ins().call(
                    callee,
                    &[self.state, number, inherit, source_ptr, source_len],
                );
                let status = builder.inst_results(call)[0];
                self.return_if_not_ok(builder, status);
                let ok = builder.ins().iconst(types::I32, i64::from(ST_OK));
                builder.ins().return_(&[ok]);
                Ok(())
            }
            other => Err(JitError::unsupported(format!(
                "terminator not lowered in M4-4: {other:?}"
            ))),
        }
    }

    fn emit_resume(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        target_kind: u32,
        label: usize,
    ) -> Result<(), JitError> {
        let block_slot =
            builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 4, 2));
        let ptr_ty = module.target_config().pointer_type();
        let block_out = builder.ins().stack_addr(ptr_ty, block_slot, 0);
        let target = builder.ins().iconst(types::I32, i64::from(target_kind));
        let label = builder.ins().iconst(types::I32, label as i64);
        let callee = self.import(builder, module, self.imports.resume);
        let call = builder
            .ins()
            .call(callee, &[self.state, target, label, block_out]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        let block = builder.ins().stack_load(types::I32, block_slot, 0);
        self.emit_jump_to_runtime_block(builder, block)
    }

    fn emit_gosub_return(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
    ) -> Result<(), JitError> {
        let block_slot =
            builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 4, 2));
        let ptr_ty = module.target_config().pointer_type();
        let block_out = builder.ins().stack_addr(ptr_ty, block_slot, 0);
        let callee = self.import(builder, module, self.imports.gosub_pop);
        let call = builder
            .ins()
            .call(callee, &[self.state, self.run, block_out]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        let block = builder.ins().stack_load(types::I32, block_slot, 0);
        self.emit_jump_to_runtime_block(builder, block)
    }

    fn emit_jump_to_runtime_block(
        &self,
        builder: &mut FunctionBuilder<'_>,
        block: Value,
    ) -> Result<(), JitError> {
        for idx in 0..self.func.blocks.len() {
            let is_target = builder
                .ins()
                .icmp_imm(ir::condcodes::IntCC::Equal, block, idx as i64);
            let next = builder.create_block();
            builder
                .ins()
                .brif(is_target, self.clif_block(BlockId(idx))?, &[], next, &[]);
            builder.switch_to_block(next);
        }
        let fault = builder.ins().iconst(types::I32, i64::from(ST_FAULT));
        builder.ins().return_(&[fault]);
        Ok(())
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
                self.ensure_i32_numeric_place(*place)?;
                let (area, index) = place_addr(*place);
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                let callee = self.import(builder, module, self.imports.load_i32);
                let call = builder.ins().call(callee, &[self.run, area, index]);
                Ok(builder.inst_results(call)[0])
            }
            other => Err(JitError::unsupported(format!(
                "i32 operand not lowered in M4-4: {other:?}"
            ))),
        }
    }

    fn lower_operand_i64(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        operand: &OxOperand,
    ) -> Result<Value, JitError> {
        match operand {
            OxOperand::Const(OxConst::I64(value)) => Ok(builder.ins().iconst(types::I64, *value)),
            OxOperand::Const(OxConst::I32(value)) => {
                Ok(builder.ins().iconst(types::I64, i64::from(*value)))
            }
            OxOperand::Const(OxConst::I16(value)) => {
                Ok(builder.ins().iconst(types::I64, i64::from(*value)))
            }
            OxOperand::Const(OxConst::Bool(value)) => {
                let raw = if *value { -1 } else { 0 };
                Ok(builder.ins().iconst(types::I64, raw))
            }
            OxOperand::Use(place) => {
                self.ensure_i64_numeric_place(*place)?;
                let (area, index) = place_addr(*place);
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                let callee = self.import(builder, module, self.imports.load_i64);
                let call = builder.ins().call(callee, &[self.run, area, index]);
                Ok(builder.inst_results(call)[0])
            }
            other => Err(JitError::unsupported(format!(
                "i64 operand not lowered in M4-4: {other:?}"
            ))),
        }
    }

    fn lower_operand_currency_i64(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        operand: &OxOperand,
    ) -> Result<Value, JitError> {
        match operand {
            OxOperand::Const(OxConst::Currency(value)) => {
                Ok(builder.ins().iconst(types::I64, *value))
            }
            OxOperand::Const(OxConst::I32(value)) => {
                Ok(builder.ins().iconst(types::I64, i64::from(*value) * 10_000))
            }
            OxOperand::Const(OxConst::I16(value)) => {
                Ok(builder.ins().iconst(types::I64, i64::from(*value) * 10_000))
            }
            OxOperand::Use(place) => {
                self.ensure_currency_place(*place)?;
                let (area, index) = place_addr(*place);
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                let callee = self.import(builder, module, self.imports.load_i64);
                let call = builder.ins().call(callee, &[self.run, area, index]);
                Ok(builder.inst_results(call)[0])
            }
            other => Err(JitError::unsupported(format!(
                "Currency operand not lowered in M4-4: {other:?}"
            ))),
        }
    }

    fn lower_operand_f64(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        operand: &OxOperand,
    ) -> Result<Value, JitError> {
        match operand {
            OxOperand::Const(OxConst::F64(bits)) => {
                Ok(builder.ins().f64const(Ieee64::with_bits(*bits)))
            }
            OxOperand::Const(OxConst::I32(value)) => Ok(builder
                .ins()
                .f64const(Ieee64::with_float(f64::from(*value)))),
            OxOperand::Const(OxConst::I16(value)) => Ok(builder
                .ins()
                .f64const(Ieee64::with_float(f64::from(*value)))),
            OxOperand::Use(place) => {
                self.ensure_double_place(*place)?;
                let (area, index) = place_addr(*place);
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                let callee = self.import(builder, module, self.imports.load_f64);
                let call = builder.ins().call(callee, &[self.run, area, index]);
                Ok(builder.inst_results(call)[0])
            }
            other => Err(JitError::unsupported(format!(
                "f64 operand not lowered in M4-4: {other:?}"
            ))),
        }
    }

    fn lower_operand_date_f64(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        operand: &OxOperand,
    ) -> Result<Value, JitError> {
        match operand {
            OxOperand::Const(OxConst::Date(bits)) => {
                Ok(builder.ins().f64const(Ieee64::with_bits(*bits)))
            }
            OxOperand::Const(OxConst::I32(value)) => Ok(builder
                .ins()
                .f64const(Ieee64::with_float(f64::from(*value)))),
            OxOperand::Const(OxConst::I16(value)) => Ok(builder
                .ins()
                .f64const(Ieee64::with_float(f64::from(*value)))),
            OxOperand::Use(place) => {
                self.ensure_date_place(*place)?;
                let (area, index) = place_addr(*place);
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                let callee = self.import(builder, module, self.imports.load_f64);
                let call = builder.ins().call(callee, &[self.run, area, index]);
                Ok(builder.inst_results(call)[0])
            }
            other => Err(JitError::unsupported(format!(
                "Date operand not lowered in M4-4: {other:?}"
            ))),
        }
    }

    fn lower_operand_f32(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        operand: &OxOperand,
    ) -> Result<Value, JitError> {
        match operand {
            OxOperand::Const(OxConst::F32(bits)) => {
                Ok(builder.ins().f32const(Ieee32::with_bits(*bits)))
            }
            OxOperand::Const(OxConst::F64(bits)) => {
                let value = f64::from_bits(*bits) as f32;
                Ok(builder.ins().f32const(Ieee32::with_float(value)))
            }
            OxOperand::Const(OxConst::I32(value)) => {
                Ok(builder.ins().f32const(Ieee32::with_float(*value as f32)))
            }
            OxOperand::Const(OxConst::I16(value)) => Ok(builder
                .ins()
                .f32const(Ieee32::with_float(f32::from(*value)))),
            OxOperand::Use(place) => {
                self.ensure_single_place(*place)?;
                let (area, index) = place_addr(*place);
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                let callee = self.import(builder, module, self.imports.load_f32);
                let call = builder.ins().call(callee, &[self.run, area, index]);
                Ok(builder.inst_results(call)[0])
            }
            other => Err(JitError::unsupported(format!(
                "f32 operand not lowered in M4-4: {other:?}"
            ))),
        }
    }

    fn lower_operand_i16_i32(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        operand: &OxOperand,
    ) -> Result<Value, JitError> {
        match operand {
            OxOperand::Const(OxConst::I16(value)) => {
                Ok(builder.ins().iconst(types::I32, i64::from(*value)))
            }
            OxOperand::Const(OxConst::I32(value)) => {
                Ok(builder.ins().iconst(types::I32, i64::from(*value)))
            }
            OxOperand::Use(place) => {
                match place_ty(self.program, self.func, *place)? {
                    OxTy::Byte | OxTy::Integer => {}
                    ty => {
                        return Err(JitError::unsupported(format!(
                            "M4-4 Integer operand lowering accepts only Byte/Integer places, got {ty:?} at {place:?}"
                        )));
                    }
                }
                self.lower_operand_i32(builder, module, operand)
            }
            other => Err(JitError::unsupported(format!(
                "Integer operand not lowered in M4-4: {other:?}"
            ))),
        }
    }

    fn lower_operand_u8_i32(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        operand: &OxOperand,
    ) -> Result<Value, JitError> {
        match operand {
            OxOperand::Const(OxConst::I16(value)) => {
                Ok(builder.ins().iconst(types::I32, i64::from(*value)))
            }
            OxOperand::Const(OxConst::I32(value)) => {
                Ok(builder.ins().iconst(types::I32, i64::from(*value)))
            }
            OxOperand::Use(place) => {
                self.ensure_byte_place(*place)?;
                self.lower_operand_i32(builder, module, operand)
            }
            other => Err(JitError::unsupported(format!(
                "Byte operand not lowered in M4-4: {other:?}"
            ))),
        }
    }

    fn lower_operand_bool_i32(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        operand: &OxOperand,
    ) -> Result<Value, JitError> {
        match operand {
            OxOperand::Const(OxConst::Bool(value)) => Ok(builder
                .ins()
                .iconst(types::I32, i64::from(u8::from(*value)))),
            OxOperand::Const(OxConst::I32(value)) => Ok(builder
                .ins()
                .iconst(types::I32, i64::from(u8::from(*value != 0)))),
            OxOperand::Const(OxConst::I16(value)) => Ok(builder
                .ins()
                .iconst(types::I32, i64::from(u8::from(*value != 0)))),
            OxOperand::Use(place) => {
                self.ensure_bool_place(*place)?;
                let (area, index) = place_addr(*place);
                let area = builder.ins().iconst(types::I32, i64::from(area));
                let index = builder.ins().iconst(types::I32, index as i64);
                let callee = self.import(builder, module, self.imports.load_bool);
                let call = builder.ins().call(callee, &[self.run, area, index]);
                Ok(builder.inst_results(call)[0])
            }
            other => Err(JitError::unsupported(format!(
                "bool operand not lowered in M4-4: {other:?}"
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

    fn emit_store_i64(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        place: OxPlace,
        value: Value,
    ) -> Result<(), JitError> {
        let (area, index) = place_addr(place);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.store_i64);
        let call = builder.ins().call(callee, &[self.run, area, index, value]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_store_currency_i64(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        place: OxPlace,
        value: Value,
    ) -> Result<(), JitError> {
        let (area, index) = place_addr(place);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.store_currency_i64);
        let call = builder.ins().call(callee, &[self.run, area, index, value]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_store_f64(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        place: OxPlace,
        value: Value,
    ) -> Result<(), JitError> {
        let (area, index) = place_addr(place);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.store_f64);
        let call = builder.ins().call(callee, &[self.run, area, index, value]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_store_date_f64(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        place: OxPlace,
        value: Value,
    ) -> Result<(), JitError> {
        let (area, index) = place_addr(place);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.store_date_f64);
        let call = builder.ins().call(callee, &[self.run, area, index, value]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_store_f32(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        place: OxPlace,
        value: Value,
    ) -> Result<(), JitError> {
        let (area, index) = place_addr(place);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.store_f32);
        let call = builder.ins().call(callee, &[self.run, area, index, value]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_store_i16(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        place: OxPlace,
        value: Value,
    ) -> Result<(), JitError> {
        let (area, index) = place_addr(place);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.store_i16);
        let call = builder.ins().call(callee, &[self.run, area, index, value]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_store_u8(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        place: OxPlace,
        value: Value,
    ) -> Result<(), JitError> {
        let (area, index) = place_addr(place);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.store_u8);
        let call = builder.ins().call(callee, &[self.run, area, index, value]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_store_bool(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        place: OxPlace,
        value: Value,
    ) -> Result<(), JitError> {
        let (area, index) = place_addr(place);
        let area = builder.ins().iconst(types::I32, i64::from(area));
        let index = builder.ins().iconst(types::I32, index as i64);
        let callee = self.import(builder, module, self.imports.store_bool);
        let call = builder.ins().call(callee, &[self.run, area, index, value]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_stmt_boundary(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        clear_temps_from: usize,
    ) -> Result<(), JitError> {
        let state = self.state;
        let clear_temps_from = builder.ins().iconst(types::I32, clear_temps_from as i64);
        let callee = self.import(builder, module, self.imports.stmt_boundary);
        let call = builder
            .ins()
            .call(callee, &[self.run, state, clear_temps_from]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_drain_terminations(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
    ) -> Result<(), JitError> {
        let callee = self.import(builder, module, self.imports.drain_terminations);
        let call = builder.ins().call(callee, &[self.run, self.state]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_add_ref(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        object: &OxOperand,
    ) -> Result<(), JitError> {
        let operand = self.lower_variant_operand(builder, object)?;
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &[operand])?;
        let callee = self.import(builder, module, self.imports.add_ref);
        let call = builder
            .ins()
            .call(callee, &[self.run, self.state, operand_ptr]);
        let status = builder.inst_results(call)[0];
        self.return_if_not_ok(builder, status);
        Ok(())
    }

    fn emit_release(
        &self,
        builder: &mut FunctionBuilder<'_>,
        module: &mut JITModule,
        object: &OxOperand,
    ) -> Result<(), JitError> {
        let operand = self.lower_variant_operand(builder, object)?;
        let operand_ptr = self.emit_variant_operand_descriptors(builder, module, &[operand])?;
        let callee = self.import(builder, module, self.imports.release);
        let call = builder
            .ins()
            .call(callee, &[self.run, self.state, operand_ptr]);
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
        if let Some(fault_target) = self.current_fault_target {
            let dispatch = builder.create_block();
            builder.ins().brif(ok, cont, &[], dispatch, &[]);
            builder.switch_to_block(dispatch);
            let is_fault =
                builder
                    .ins()
                    .icmp_imm(ir::condcodes::IntCC::Equal, status, i64::from(ST_FAULT));
            builder
                .ins()
                .brif(is_fault, fault_target, &[], ret, &[status.into()]);
        } else {
            builder.ins().brif(ok, cont, &[], ret, &[status.into()]);
        }

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
        module.declare_func_in_func(id, builder.func)
    }

    fn ensure_long_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::Long) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only Long places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_longlong_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::LongLong) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only LongLong places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_currency_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::Currency) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only Currency places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_double_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::Double) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only Double places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_date_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::Date) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only Date places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_single_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::Single) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only Single places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_integer_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::Integer) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only Integer places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_byte_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::Byte) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only Byte places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_i32_numeric_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::Long | OxTy::Byte | OxTy::Integer | OxTy::Bool) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only Long/Byte/Integer/Bool places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_i64_numeric_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::LongLong | OxTy::Long | OxTy::Byte | OxTy::Integer) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only LongLong/Long/Byte/Integer places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_bool_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::Bool) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only Bool places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_variant_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::Variant) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only Variant places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_variant_carrier_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if is_jit_variant_carrier_ty(ty) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "JIT object lowering requires a Variant-backed carrier place, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_string_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::Str) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only String places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_fixed_string_place(&self, place: OxPlace, len: u32) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::FixedStr(actual) if *actual == len) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only matching fixed-length String places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }

    fn ensure_proc_ref_place(&self, place: OxPlace) -> Result<(), JitError> {
        let ty = place_ty(self.program, self.func, place)?;
        if matches!(ty, OxTy::ProcRef) {
            Ok(())
        } else {
            Err(JitError::unsupported(format!(
                "M4-4 lowers only ProcRef places for this operation, got {ty:?} at {place:?}"
            )))
        }
    }
}

pub(crate) fn declare_program_functions(
    module: &mut JITModule,
    program_index: usize,
    program: &OxProgram,
) -> Result<Vec<ClifFuncId>, JitError> {
    let sig = entry_signature(module);
    program
        .funcs
        .iter()
        .enumerate()
        .map(|(index, func)| {
            let name = format!(
                "ox$p{program_index}$f{index}${}",
                sanitize_symbol(&func.name)
            );
            module
                .declare_function(&name, Linkage::Local, &sig)
                .map_err(module_err)
        })
        .collect()
}

pub(crate) fn entry_signature(module: &mut JITModule) -> ir::Signature {
    let ptr_ty = module.target_config().pointer_type();
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(ptr_ty));
    sig.params.push(AbiParam::new(ptr_ty));
    sig.returns.push(AbiParam::new(types::I32));
    sig
}

pub(crate) fn sanitize_symbol(name: &str) -> String {
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

pub(crate) fn collect_static_proc_refs(
    program: &OxProgram,
    func: &OxFunc,
) -> HashMap<OxPlace, ProcRefStaticTarget> {
    let mut refs = HashMap::new();
    for block in &func.blocks {
        for inst in &block.instrs {
            if let OxInst::LoadProcRef { dst, proc } = inst {
                match refs.get_mut(dst) {
                    None => {
                        refs.insert(*dst, ProcRefStaticTarget::Unique(*proc));
                    }
                    Some(slot) => {
                        *slot = merge_proc_ref_static_target(program, *slot, *proc);
                    }
                }
            }
        }
    }
    refs
}

pub(crate) fn func_has_label_error_handler(func: &OxFunc) -> bool {
    func.blocks.iter().any(|block| {
        block
            .instrs
            .iter()
            .any(|inst| matches!(inst, OxInst::SetErrorHandler(ErrorHandler::GotoLabel(_))))
    })
}

pub(crate) fn merge_proc_ref_static_target(
    program: &OxProgram,
    current: ProcRefStaticTarget,
    next: FuncId,
) -> ProcRefStaticTarget {
    match current {
        ProcRefStaticTarget::Unique(existing) if existing == next => current,
        ProcRefStaticTarget::Unique(existing) | ProcRefStaticTarget::SameSignature(existing)
            if proc_ref_signatures_match(program, existing, next) =>
        {
            ProcRefStaticTarget::SameSignature(existing)
        }
        ProcRefStaticTarget::SameSignature(_) | ProcRefStaticTarget::Unknown => {
            ProcRefStaticTarget::Unknown
        }
        ProcRefStaticTarget::Unique(_) => ProcRefStaticTarget::Unknown,
    }
}

pub(crate) fn proc_ref_signatures_match(program: &OxProgram, lhs: FuncId, rhs: FuncId) -> bool {
    let (Some(lhs), Some(rhs)) = (program.funcs.get(lhs.0), program.funcs.get(rhs.0)) else {
        return false;
    };
    if lhs.param_count != rhs.param_count {
        return false;
    }
    for index in 0..lhs.param_count {
        let (Some(lhs_param), Some(rhs_param)) = (lhs.locals.get(index), rhs.locals.get(index))
        else {
            return false;
        };
        if lhs_param.ty != rhs_param.ty {
            return false;
        }
        match (lhs_param.param.as_ref(), rhs_param.param.as_ref()) {
            (Some(lhs_info), Some(rhs_info)) => {
                if lhs_info.by_ref != rhs_info.by_ref || lhs_info.variadic != rhs_info.variadic {
                    return false;
                }
            }
            _ => return false,
        }
    }
    proc_return_ty(lhs) == proc_return_ty(rhs)
}

pub(crate) fn proc_return_ty(func: &OxFunc) -> Option<&OxTy> {
    func.return_local
        .and_then(|ret| func.locals.get(ret.0).map(|local| &local.ty))
}

pub(crate) fn resolve_vba_library_import(
    program: &OxProgram,
    import: oxvba_oxir::ImportId,
) -> Result<(u32, NativeImplId, bool), JitError> {
    let imp = program
        .imports
        .get(import.0)
        .ok_or_else(|| JitError::Compile(format!("CallExtern import {} out of range", import.0)))?;
    if !imp.unit.eq_ignore_ascii_case("VBA") {
        return Err(JitError::unsupported(format!(
            "JIT CallExtern could not resolve non-VBA import unit '{}'",
            imp.unit
        )));
    }
    let lib = vba_library_bundle();
    let export = lib
        .exports
        .iter()
        .find(|export| export.token.matches(&imp.token))
        .ok_or_else(|| {
            JitError::unsupported(format!(
                "M4-4 CallExtern could not resolve VBA library import {}",
                import.0
            ))
        })?;
    let ExportTarget::Proc(proc) = export.target else {
        return Err(JitError::unsupported(
            "M4-4 CallExtern lowers only VBA library procedure imports",
        ));
    };
    let Some(NativeBody::Library(id)) = lib.procedures.get(proc).and_then(|proc| proc.native)
    else {
        return Err(JitError::unsupported(
            "M4-4 CallExtern lowers only NativeBody::Library VBA imports",
        ));
    };
    let string_typed_alias = matches!(
        &imp.token,
        oxvba_bundle::ExportToken::ModuleFunc { member, .. }
            if id.is_string_typed_library_alias(member)
    );
    let native_index = native_impl_index(id)?;
    Ok((native_index, id, string_typed_alias))
}

pub(crate) fn resolve_cross_project_proc_import(
    programs: &[&OxProgram],
    program: &OxProgram,
    import: oxvba_oxir::ImportId,
) -> Result<Option<(usize, FuncId)>, JitError> {
    let imp = program
        .imports
        .get(import.0)
        .ok_or_else(|| JitError::Compile(format!("CallExtern import {} out of range", import.0)))?;
    if imp.unit.eq_ignore_ascii_case("VBA") {
        return Ok(None);
    }
    let Some((program_index, target_program)) = programs
        .iter()
        .enumerate()
        .find(|(_, candidate)| candidate.unit_name.eq_ignore_ascii_case(&imp.unit))
    else {
        return Err(JitError::unsupported(format!(
            "JIT cross-project CallExtern could not resolve unit '{}'",
            imp.unit
        )));
    };
    let export = target_program
        .exports
        .iter()
        .find(|export| export.token.matches(&imp.token))
        .ok_or_else(|| {
            JitError::unsupported(format!(
                "JIT cross-project CallExtern could not resolve export in unit '{}'",
                imp.unit
            ))
        })?;
    let ExportTarget::Proc(proc) = export.target else {
        return Err(JitError::unsupported(
            "JIT cross-project CallExtern resolved to a non-procedure export",
        ));
    };
    Ok(Some((program_index, FuncId(proc))))
}

pub(crate) fn native_impl_index(id: NativeImplId) -> Result<u32, JitError> {
    NativeImplId::ALL
        .iter()
        .position(|candidate| *candidate == id)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(|| {
            JitError::Compile(format!("VBA library id {id:?} is not in NativeImplId::ALL"))
        })
}

pub(crate) fn module_err(err: impl std::fmt::Display) -> JitError {
    JitError::Compile(err.to_string())
}

pub(crate) fn int_compare_cc(op: CmpOp) -> ir::condcodes::IntCC {
    match op {
        CmpOp::Eq => ir::condcodes::IntCC::Equal,
        CmpOp::Ne => ir::condcodes::IntCC::NotEqual,
        CmpOp::Lt => ir::condcodes::IntCC::SignedLessThan,
        CmpOp::Le => ir::condcodes::IntCC::SignedLessThanOrEqual,
        CmpOp::Gt => ir::condcodes::IntCC::SignedGreaterThan,
        CmpOp::Ge => ir::condcodes::IntCC::SignedGreaterThanOrEqual,
    }
}

pub(crate) fn float_compare_cc(op: CmpOp) -> ir::condcodes::FloatCC {
    match op {
        CmpOp::Eq => ir::condcodes::FloatCC::Equal,
        CmpOp::Ne => ir::condcodes::FloatCC::NotEqual,
        CmpOp::Lt => ir::condcodes::FloatCC::LessThan,
        CmpOp::Le => ir::condcodes::FloatCC::LessThanOrEqual,
        CmpOp::Gt => ir::condcodes::FloatCC::GreaterThan,
        CmpOp::Ge => ir::condcodes::FloatCC::GreaterThanOrEqual,
    }
}
