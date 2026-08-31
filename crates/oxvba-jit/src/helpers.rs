//! Helper registration, Cranelift imports, and compiled-entry shims.

use super::*;

#[derive(Clone, Copy)]
pub(crate) struct Imports {
    pub(crate) load_i32: ClifFuncId,
    pub(crate) load_i64: ClifFuncId,
    pub(crate) load_f32: ClifFuncId,
    pub(crate) load_f64: ClifFuncId,
    pub(crate) pack_f32_arg: ClifFuncId,
    pub(crate) pack_f64_arg: ClifFuncId,
    pub(crate) store_i32: ClifFuncId,
    pub(crate) store_i64: ClifFuncId,
    pub(crate) store_f32: ClifFuncId,
    pub(crate) store_f64: ClifFuncId,
    pub(crate) store_currency_i64: ClifFuncId,
    pub(crate) store_date_f64: ClifFuncId,
    pub(crate) store_u8: ClifFuncId,
    pub(crate) store_i16: ClifFuncId,
    pub(crate) load_bool: ClifFuncId,
    pub(crate) store_bool: ClifFuncId,
    pub(crate) store_proc_ref: ClifFuncId,
    pub(crate) store_variant: ClifFuncId,
    pub(crate) stmt_boundary: ClifFuncId,
    pub(crate) drain_terminations: ClifFuncId,
    pub(crate) add_ref: ClifFuncId,
    pub(crate) release: ClifFuncId,
    pub(crate) as_new_project_class_slot: ClifFuncId,
    pub(crate) as_new_collection_slot: ClifFuncId,
    pub(crate) new_collection_slot: ClifFuncId,
    pub(crate) new_object_slot: ClifFuncId,
    pub(crate) predeclared_slot: ClifFuncId,
    pub(crate) predeclared_set: ClifFuncId,
    pub(crate) field_get_slot: ClifFuncId,
    pub(crate) field_set_slot: ClifFuncId,
    pub(crate) withevents_get_slot: ClifFuncId,
    pub(crate) withevents_set_slot: ClifFuncId,
    pub(crate) withevents_clear_owner_slot: ClifFuncId,
    pub(crate) withevents_first_owner_slot: ClifFuncId,
    pub(crate) withevents_next_owner_slot: ClifFuncId,
    pub(crate) raise_event: ClifFuncId,
    pub(crate) project_member_get_slot: ClifFuncId,
    pub(crate) call_by_name_slot: ClifFuncId,
    pub(crate) project_type_name_slot: ClifFuncId,
    pub(crate) new_record_slot: ClifFuncId,
    pub(crate) record_get_slot: ClifFuncId,
    pub(crate) record_set_slot: ClifFuncId,
    pub(crate) record_lset_slot: ClifFuncId,
    pub(crate) record_array_get_slot: ClifFuncId,
    pub(crate) record_array_set_slot: ClifFuncId,
    pub(crate) field_array_get_slot: ClifFuncId,
    pub(crate) field_array_set_slot: ClifFuncId,
    pub(crate) validate_assignment: ClifFuncId,
    pub(crate) err_clear: ClifFuncId,
    pub(crate) err_i32_field: ClifFuncId,
    pub(crate) err_string_field_utf8: ClifFuncId,
    pub(crate) erl_get: ClifFuncId,
    pub(crate) err_set_field: ClifFuncId,
    pub(crate) set_line_number: ClifFuncId,
    pub(crate) current_line: ClifFuncId,
    pub(crate) set_error_handler: ClifFuncId,
    pub(crate) resume: ClifFuncId,
    pub(crate) route_fault: ClifFuncId,
    pub(crate) raise_error_number: ClifFuncId,
    pub(crate) gosub_push: ClifFuncId,
    pub(crate) gosub_pop: ClifFuncId,
    pub(crate) direct_enter_noarg_sub: ClifFuncId,
    pub(crate) direct_exit_noarg_sub: ClifFuncId,
    pub(crate) direct_enter_noarg_func: ClifFuncId,
    pub(crate) direct_exit_noarg_func: ClifFuncId,
    pub(crate) direct_enter_one_i32_sub: ClifFuncId,
    pub(crate) direct_enter_one_i32_func: ClifFuncId,
    pub(crate) direct_enter_one_i32_byref_sub: ClifFuncId,
    pub(crate) direct_enter_one_i32_byref_func: ClifFuncId,
    pub(crate) direct_enter_two_i32_sub: ClifFuncId,
    pub(crate) direct_enter_two_i32_func: ClifFuncId,
    pub(crate) direct_enter_proc_i32: ClifFuncId,
    pub(crate) expect_proc_ref_i32: ClifFuncId,
    pub(crate) array_literal_slot: ClifFuncId,
    pub(crate) array_redim_slot: ClifFuncId,
    pub(crate) array_erase_slot: ClifFuncId,
    pub(crate) array_get_slot: ClifFuncId,
    pub(crate) array_get_i32_1d_slot: ClifFuncId,
    pub(crate) array_set_slot: ClifFuncId,
    pub(crate) array_set_i32_1d_slot: ClifFuncId,
    pub(crate) bound_slot: ClifFuncId,
    pub(crate) for_each_init_slot: ClifFuncId,
    pub(crate) for_each_next_slot: ClifFuncId,
    pub(crate) call_extern_proc_i32: ClifFuncId,
    pub(crate) call_proc_ref_i32: ClifFuncId,
    pub(crate) lib_invoke_slot: ClifFuncId,
    pub(crate) declare_call_slot: ClifFuncId,
    pub(crate) arith_v_slot: ClifFuncId,
    pub(crate) concat_v_slot: ClifFuncId,
    pub(crate) neg_v_slot: ClifFuncId,
    pub(crate) compare_v_slot: ClifFuncId,
    pub(crate) compare_object_is_slot: ClifFuncId,
    pub(crate) type_of_is_slot: ClifFuncId,
    pub(crate) logical_v_slot: ClifFuncId,
    pub(crate) not_v_slot: ClifFuncId,
    pub(crate) truthy_v_slot: ClifFuncId,
    pub(crate) variant_changed_slot: ClifFuncId,
    pub(crate) coerce_numeric_v_slot: ClifFuncId,
    pub(crate) coerce_string_v_slot: ClifFuncId,
    pub(crate) coerce_fixed_string_v_slot: ClifFuncId,
    pub(crate) unbox_slot: ClifFuncId,
    pub(crate) add_i32_slot: ClifFuncId,
    pub(crate) sub_i32_slot: ClifFuncId,
    pub(crate) mul_i32_slot: ClifFuncId,
    pub(crate) div_i32_slot: ClifFuncId,
    pub(crate) rem_i32_slot: ClifFuncId,
    pub(crate) add_i16_slot: ClifFuncId,
    pub(crate) sub_i16_slot: ClifFuncId,
    pub(crate) mul_i16_slot: ClifFuncId,
    pub(crate) add_u8_slot: ClifFuncId,
    pub(crate) sub_u8_slot: ClifFuncId,
    pub(crate) mul_u8_slot: ClifFuncId,
    pub(crate) add_i64_slot: ClifFuncId,
    pub(crate) sub_i64_slot: ClifFuncId,
    pub(crate) mul_i64_slot: ClifFuncId,
    pub(crate) div_i64_slot: ClifFuncId,
    pub(crate) rem_i64_slot: ClifFuncId,
    pub(crate) add_currency_slot: ClifFuncId,
    pub(crate) sub_currency_slot: ClifFuncId,
    pub(crate) mul_currency_slot: ClifFuncId,
}

pub(crate) fn jit_builder() -> Result<JITBuilder, JitError> {
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

pub(crate) fn register_symbols(builder: &mut JITBuilder) {
    builder.symbol("rt_jit_load_i32", rt_jit_load_i32 as *const u8);
    builder.symbol("rt_jit_load_i64", rt_jit_load_i64 as *const u8);
    builder.symbol("rt_jit_load_f32", rt_jit_load_f32 as *const u8);
    builder.symbol("rt_jit_load_f64", rt_jit_load_f64 as *const u8);
    builder.symbol("rt_jit_pack_f32_arg", rt_jit_pack_f32_arg as *const u8);
    builder.symbol("rt_jit_pack_f64_arg", rt_jit_pack_f64_arg as *const u8);
    builder.symbol("rt_jit_store_i32", rt_jit_store_i32 as *const u8);
    builder.symbol("rt_jit_store_i64", rt_jit_store_i64 as *const u8);
    builder.symbol("rt_jit_store_f32", rt_jit_store_f32 as *const u8);
    builder.symbol("rt_jit_store_f64", rt_jit_store_f64 as *const u8);
    builder.symbol(
        "rt_jit_store_currency_i64",
        rt_jit_store_currency_i64 as *const u8,
    );
    builder.symbol("rt_jit_store_date_f64", rt_jit_store_date_f64 as *const u8);
    builder.symbol("rt_jit_store_u8", rt_jit_store_u8 as *const u8);
    builder.symbol("rt_jit_store_i16", rt_jit_store_i16 as *const u8);
    builder.symbol("rt_jit_load_bool", rt_jit_load_bool as *const u8);
    builder.symbol("rt_jit_store_bool", rt_jit_store_bool as *const u8);
    builder.symbol("rt_jit_store_proc_ref", rt_jit_store_proc_ref as *const u8);
    builder.symbol("rt_jit_store_variant", rt_jit_store_variant as *const u8);
    builder.symbol("rt_jit_stmt_boundary", rt_jit_stmt_boundary as *const u8);
    builder.symbol(
        "rt_jit_drain_terminations",
        rt_jit_drain_terminations as *const u8,
    );
    builder.symbol("rt_jit_add_ref", rt_jit_add_ref as *const u8);
    builder.symbol("rt_jit_release", rt_jit_release as *const u8);
    builder.symbol(
        "rt_jit_as_new_project_class_slot",
        rt_jit_as_new_project_class_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_as_new_collection_slot",
        rt_jit_as_new_collection_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_new_collection_to_slot",
        rt_jit_new_collection_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_new_object_to_slot",
        rt_jit_new_object_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_predeclared_to_slot",
        rt_jit_predeclared_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_predeclared_set",
        rt_jit_predeclared_set as *const u8,
    );
    builder.symbol(
        "rt_jit_project_field_get_to_slot",
        rt_jit_project_field_get_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_project_field_set",
        rt_jit_project_field_set as *const u8,
    );
    builder.symbol(
        "rt_jit_withevents_get_to_slot",
        rt_jit_withevents_get_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_withevents_set_to_slot",
        rt_jit_withevents_set_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_withevents_clear_owner_to_slot",
        rt_jit_withevents_clear_owner_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_withevents_first_owner_to_slot",
        rt_jit_withevents_first_owner_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_withevents_next_owner_to_slot",
        rt_jit_withevents_next_owner_to_slot as *const u8,
    );
    builder.symbol("rt_jit_raise_event", rt_jit_raise_event as *const u8);
    builder.symbol(
        "rt_jit_project_member_get_to_slot",
        rt_jit_project_member_get_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_call_by_name_to_slot",
        rt_jit_call_by_name_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_project_type_name_to_slot",
        rt_jit_project_type_name_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_new_record_to_slot",
        rt_jit_new_record_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_record_get_to_slot",
        rt_jit_record_get_to_slot as *const u8,
    );
    builder.symbol("rt_jit_record_set", rt_jit_record_set as *const u8);
    builder.symbol("rt_jit_record_lset", rt_jit_record_lset as *const u8);
    builder.symbol(
        "rt_jit_record_array_get_to_slot",
        rt_jit_record_array_get_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_record_array_set",
        rt_jit_record_array_set as *const u8,
    );
    builder.symbol(
        "rt_jit_project_field_array_get_to_slot",
        rt_jit_project_field_array_get_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_project_field_array_set",
        rt_jit_project_field_array_set as *const u8,
    );
    builder.symbol(
        "rt_jit_validate_assignment",
        rt_jit_validate_assignment as *const u8,
    );
    builder.symbol("rt_err_clear", rt_err_clear as *const u8);
    builder.symbol("rt_err_i32_field", rt_err_i32_field as *const u8);
    builder.symbol(
        "rt_err_string_field_utf8",
        rt_err_string_field_utf8 as *const u8,
    );
    builder.symbol("rt_erl_get", rt_erl_get as *const u8);
    builder.symbol("rt_jit_err_field_set", rt_jit_err_field_set as *const u8);
    builder.symbol(
        "rt_jit_set_line_number",
        rt_jit_set_line_number as *const u8,
    );
    builder.symbol("rt_jit_current_line", rt_jit_current_line as *const u8);
    builder.symbol("rt_resume", rt_resume as *const u8);
    builder.symbol("rt_set_error_handler", rt_set_error_handler as *const u8);
    builder.symbol("rt_route_fault", rt_route_fault as *const u8);
    builder.symbol("rt_raise_error_number", rt_raise_error_number as *const u8);
    builder.symbol("rt_jit_gosub_push", rt_jit_gosub_push as *const u8);
    builder.symbol("rt_jit_gosub_pop", rt_jit_gosub_pop as *const u8);
    builder.symbol(
        "rt_jit_direct_enter_noarg_sub",
        rt_jit_direct_enter_noarg_sub as *const u8,
    );
    builder.symbol(
        "rt_jit_direct_exit_noarg_sub",
        rt_jit_direct_exit_noarg_sub as *const u8,
    );
    builder.symbol(
        "rt_jit_direct_enter_noarg_func",
        rt_jit_direct_enter_noarg_func as *const u8,
    );
    builder.symbol(
        "rt_jit_direct_exit_noarg_func",
        rt_jit_direct_exit_noarg_func as *const u8,
    );
    builder.symbol(
        "rt_jit_direct_enter_one_i32_sub",
        rt_jit_direct_enter_one_i32_sub as *const u8,
    );
    builder.symbol(
        "rt_jit_direct_enter_one_i32_func",
        rt_jit_direct_enter_one_i32_func as *const u8,
    );
    builder.symbol(
        "rt_jit_direct_enter_one_i32_byref_sub",
        rt_jit_direct_enter_one_i32_byref_sub as *const u8,
    );
    builder.symbol(
        "rt_jit_direct_enter_one_i32_byref_func",
        rt_jit_direct_enter_one_i32_byref_func as *const u8,
    );
    builder.symbol(
        "rt_jit_direct_enter_two_i32_sub",
        rt_jit_direct_enter_two_i32_sub as *const u8,
    );
    builder.symbol(
        "rt_jit_direct_enter_two_i32_func",
        rt_jit_direct_enter_two_i32_func as *const u8,
    );
    builder.symbol(
        "rt_jit_direct_enter_proc_i32",
        rt_jit_direct_enter_proc_i32 as *const u8,
    );
    builder.symbol(
        "rt_jit_expect_proc_ref_i32",
        rt_jit_expect_proc_ref_i32 as *const u8,
    );
    builder.symbol(
        "rt_jit_array_literal_to_slot",
        rt_jit_array_literal_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_array_redim_to_slot",
        rt_jit_array_redim_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_array_erase_variant_slot",
        rt_jit_array_erase_variant_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_array_get_variant_to_slot",
        rt_jit_array_get_variant_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_array_get_i32_1d_to_slot",
        rt_jit_array_get_i32_1d_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_array_set_variant_slot",
        rt_jit_array_set_variant_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_array_set_i32_1d_slot",
        rt_jit_array_set_i32_1d_slot as *const u8,
    );
    builder.symbol("rt_jit_bound_to_slot", rt_jit_bound_to_slot as *const u8);
    builder.symbol(
        "rt_jit_for_each_init_variant_array",
        rt_jit_for_each_init_variant_array as *const u8,
    );
    builder.symbol(
        "rt_jit_for_each_next_variant_array",
        rt_jit_for_each_next_variant_array as *const u8,
    );
    builder.symbol("rt_jit_unbox_to_slot", rt_jit_unbox_to_slot as *const u8);
    builder.symbol(
        "rt_jit_call_extern_proc_i32",
        rt_jit_call_extern_proc_i32 as *const u8,
    );
    builder.symbol("rt_jit_call_proc_i32", rt_jit_call_proc_i32 as *const u8);
    builder.symbol(
        "rt_jit_call_proc_ref_i32",
        rt_jit_call_proc_ref_i32 as *const u8,
    );
    builder.symbol(
        "rt_jit_lib_invoke_to_slot",
        rt_jit_lib_invoke_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_declare_call_to_slot",
        rt_jit_declare_call_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_arith_v_to_slot",
        rt_jit_arith_v_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_concat_v_to_slot",
        rt_jit_concat_v_to_slot as *const u8,
    );
    builder.symbol("rt_jit_neg_v_to_slot", rt_jit_neg_v_to_slot as *const u8);
    builder.symbol(
        "rt_jit_compare_v_to_slot",
        rt_jit_compare_v_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_compare_object_is_to_bool_slot",
        rt_jit_compare_object_is_to_bool_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_type_of_is_to_bool_slot",
        rt_jit_type_of_is_to_bool_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_logical_v_to_slot",
        rt_jit_logical_v_to_slot as *const u8,
    );
    builder.symbol("rt_jit_not_v_to_slot", rt_jit_not_v_to_slot as *const u8);
    builder.symbol(
        "rt_jit_truthy_v_to_bool_slot",
        rt_jit_truthy_v_to_bool_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_variant_changed_to_bool_slot",
        rt_jit_variant_changed_to_bool_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_coerce_numeric_v_to_slot",
        rt_jit_coerce_numeric_v_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_coerce_string_v_to_slot",
        rt_jit_coerce_string_v_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_coerce_fixed_string_v_to_slot",
        rt_jit_coerce_fixed_string_v_to_slot as *const u8,
    );
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
    builder.symbol(
        "rt_jit_div_i32_to_slot",
        rt_jit_div_i32_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_rem_i32_to_slot",
        rt_jit_rem_i32_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_add_i16_to_slot",
        rt_jit_add_i16_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_sub_i16_to_slot",
        rt_jit_sub_i16_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_mul_i16_to_slot",
        rt_jit_mul_i16_to_slot as *const u8,
    );
    builder.symbol("rt_jit_add_u8_to_slot", rt_jit_add_u8_to_slot as *const u8);
    builder.symbol("rt_jit_sub_u8_to_slot", rt_jit_sub_u8_to_slot as *const u8);
    builder.symbol("rt_jit_mul_u8_to_slot", rt_jit_mul_u8_to_slot as *const u8);
    builder.symbol(
        "rt_jit_add_i64_to_slot",
        rt_jit_add_i64_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_sub_i64_to_slot",
        rt_jit_sub_i64_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_mul_i64_to_slot",
        rt_jit_mul_i64_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_div_i64_to_slot",
        rt_jit_div_i64_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_rem_i64_to_slot",
        rt_jit_rem_i64_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_add_currency_to_slot",
        rt_jit_add_currency_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_sub_currency_to_slot",
        rt_jit_sub_currency_to_slot as *const u8,
    );
    builder.symbol(
        "rt_jit_mul_currency_to_slot",
        rt_jit_mul_currency_to_slot as *const u8,
    );
    builder.symbol("rt_add_i32", rt_add_i32 as *const u8);
    builder.symbol("rt_sub_i32", rt_sub_i32 as *const u8);
    builder.symbol("rt_mul_i32", rt_mul_i32 as *const u8);
    builder.symbol("rt_currency_add", rt_currency_add as *const u8);
    builder.symbol("rt_currency_sub", rt_currency_sub as *const u8);
    builder.symbol("rt_currency_mul", rt_currency_mul as *const u8);
}

pub(crate) fn declare_imports(module: &mut JITModule) -> Result<Imports, JitError> {
    let ptr_ty = module.target_config().pointer_type();

    let mut load_sig = module.make_signature();
    load_sig.params.push(AbiParam::new(ptr_ty));
    load_sig.params.push(AbiParam::new(types::I32));
    load_sig.params.push(AbiParam::new(types::I32));
    load_sig.returns.push(AbiParam::new(types::I32));
    let load_i32 = module
        .declare_function("rt_jit_load_i32", Linkage::Import, &load_sig)
        .map_err(module_err)?;
    let mut load_i64_sig = module.make_signature();
    load_i64_sig.params.push(AbiParam::new(ptr_ty));
    load_i64_sig.params.push(AbiParam::new(types::I32));
    load_i64_sig.params.push(AbiParam::new(types::I32));
    load_i64_sig.returns.push(AbiParam::new(types::I64));
    let load_i64 = module
        .declare_function("rt_jit_load_i64", Linkage::Import, &load_i64_sig)
        .map_err(module_err)?;
    let mut load_f32_sig = module.make_signature();
    load_f32_sig.params.push(AbiParam::new(ptr_ty));
    load_f32_sig.params.push(AbiParam::new(types::I32));
    load_f32_sig.params.push(AbiParam::new(types::I32));
    load_f32_sig.returns.push(AbiParam::new(types::F32));
    let load_f32 = module
        .declare_function("rt_jit_load_f32", Linkage::Import, &load_f32_sig)
        .map_err(module_err)?;
    let mut load_f64_sig = module.make_signature();
    load_f64_sig.params.push(AbiParam::new(ptr_ty));
    load_f64_sig.params.push(AbiParam::new(types::I32));
    load_f64_sig.params.push(AbiParam::new(types::I32));
    load_f64_sig.returns.push(AbiParam::new(types::F64));
    let load_f64 = module
        .declare_function("rt_jit_load_f64", Linkage::Import, &load_f64_sig)
        .map_err(module_err)?;
    let mut pack_f32_sig = module.make_signature();
    pack_f32_sig.params.push(AbiParam::new(types::F32));
    pack_f32_sig.returns.push(AbiParam::new(types::I64));
    let pack_f32_arg = module
        .declare_function("rt_jit_pack_f32_arg", Linkage::Import, &pack_f32_sig)
        .map_err(module_err)?;
    let mut pack_f64_sig = module.make_signature();
    pack_f64_sig.params.push(AbiParam::new(types::F64));
    pack_f64_sig.returns.push(AbiParam::new(types::I64));
    let pack_f64_arg = module
        .declare_function("rt_jit_pack_f64_arg", Linkage::Import, &pack_f64_sig)
        .map_err(module_err)?;
    let load_bool = module
        .declare_function("rt_jit_load_bool", Linkage::Import, &load_sig)
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
    let mut store_i64_sig = module.make_signature();
    store_i64_sig.params.push(AbiParam::new(ptr_ty));
    store_i64_sig.params.push(AbiParam::new(types::I32));
    store_i64_sig.params.push(AbiParam::new(types::I32));
    store_i64_sig.params.push(AbiParam::new(types::I64));
    store_i64_sig.returns.push(AbiParam::new(types::I32));
    let store_i64 = module
        .declare_function("rt_jit_store_i64", Linkage::Import, &store_i64_sig)
        .map_err(module_err)?;
    let store_currency_i64 = module
        .declare_function("rt_jit_store_currency_i64", Linkage::Import, &store_i64_sig)
        .map_err(module_err)?;
    let mut store_f32_sig = module.make_signature();
    store_f32_sig.params.push(AbiParam::new(ptr_ty));
    store_f32_sig.params.push(AbiParam::new(types::I32));
    store_f32_sig.params.push(AbiParam::new(types::I32));
    store_f32_sig.params.push(AbiParam::new(types::F32));
    store_f32_sig.returns.push(AbiParam::new(types::I32));
    let store_f32 = module
        .declare_function("rt_jit_store_f32", Linkage::Import, &store_f32_sig)
        .map_err(module_err)?;
    let mut store_f64_sig = module.make_signature();
    store_f64_sig.params.push(AbiParam::new(ptr_ty));
    store_f64_sig.params.push(AbiParam::new(types::I32));
    store_f64_sig.params.push(AbiParam::new(types::I32));
    store_f64_sig.params.push(AbiParam::new(types::F64));
    store_f64_sig.returns.push(AbiParam::new(types::I32));
    let store_f64 = module
        .declare_function("rt_jit_store_f64", Linkage::Import, &store_f64_sig)
        .map_err(module_err)?;
    let store_date_f64 = module
        .declare_function("rt_jit_store_date_f64", Linkage::Import, &store_f64_sig)
        .map_err(module_err)?;
    let store_u8 = module
        .declare_function("rt_jit_store_u8", Linkage::Import, &store_sig)
        .map_err(module_err)?;
    let store_i16 = module
        .declare_function("rt_jit_store_i16", Linkage::Import, &store_sig)
        .map_err(module_err)?;
    let store_bool = module
        .declare_function("rt_jit_store_bool", Linkage::Import, &store_sig)
        .map_err(module_err)?;
    let store_proc_ref = module
        .declare_function("rt_jit_store_proc_ref", Linkage::Import, &store_sig)
        .map_err(module_err)?;
    let mut store_variant_sig = module.make_signature();
    store_variant_sig.params.push(AbiParam::new(ptr_ty));
    store_variant_sig.params.push(AbiParam::new(ptr_ty));
    store_variant_sig.params.push(AbiParam::new(ptr_ty));
    store_variant_sig.params.push(AbiParam::new(types::I32));
    store_variant_sig.params.push(AbiParam::new(types::I32));
    store_variant_sig.returns.push(AbiParam::new(types::I32));
    let store_variant = module
        .declare_function("rt_jit_store_variant", Linkage::Import, &store_variant_sig)
        .map_err(module_err)?;

    let mut stmt_boundary_sig = module.make_signature();
    stmt_boundary_sig.params.push(AbiParam::new(ptr_ty));
    stmt_boundary_sig.params.push(AbiParam::new(ptr_ty));
    stmt_boundary_sig.params.push(AbiParam::new(types::I32));
    stmt_boundary_sig.returns.push(AbiParam::new(types::I32));
    let stmt_boundary = module
        .declare_function("rt_jit_stmt_boundary", Linkage::Import, &stmt_boundary_sig)
        .map_err(module_err)?;

    let mut set_line_sig = module.make_signature();
    set_line_sig.params.push(AbiParam::new(ptr_ty));
    set_line_sig.params.push(AbiParam::new(types::I32));
    set_line_sig.returns.push(AbiParam::new(types::I32));
    let set_line_number = module
        .declare_function("rt_jit_set_line_number", Linkage::Import, &set_line_sig)
        .map_err(module_err)?;

    let mut current_line_sig = module.make_signature();
    current_line_sig.params.push(AbiParam::new(ptr_ty));
    current_line_sig.returns.push(AbiParam::new(types::I32));
    let current_line = module
        .declare_function("rt_jit_current_line", Linkage::Import, &current_line_sig)
        .map_err(module_err)?;

    let mut erl_get_sig = module.make_signature();
    erl_get_sig.params.push(AbiParam::new(ptr_ty));
    erl_get_sig.params.push(AbiParam::new(ptr_ty));
    erl_get_sig.returns.push(AbiParam::new(types::I32));
    let erl_get = module
        .declare_function("rt_erl_get", Linkage::Import, &erl_get_sig)
        .map_err(module_err)?;

    let mut err_set_field_sig = module.make_signature();
    err_set_field_sig.params.push(AbiParam::new(ptr_ty));
    err_set_field_sig.params.push(AbiParam::new(ptr_ty));
    err_set_field_sig.params.push(AbiParam::new(types::I32));
    err_set_field_sig.params.push(AbiParam::new(types::I32));
    err_set_field_sig.params.push(AbiParam::new(types::I64));
    err_set_field_sig.params.push(AbiParam::new(types::I32));
    err_set_field_sig.params.push(AbiParam::new(types::I32));
    err_set_field_sig.returns.push(AbiParam::new(types::I32));
    let err_set_field = module
        .declare_function("rt_jit_err_field_set", Linkage::Import, &err_set_field_sig)
        .map_err(module_err)?;

    let mut drain_terminations_sig = module.make_signature();
    drain_terminations_sig.params.push(AbiParam::new(ptr_ty));
    drain_terminations_sig.params.push(AbiParam::new(ptr_ty));
    drain_terminations_sig
        .returns
        .push(AbiParam::new(types::I32));
    let drain_terminations = module
        .declare_function(
            "rt_jit_drain_terminations",
            Linkage::Import,
            &drain_terminations_sig,
        )
        .map_err(module_err)?;

    let mut ref_effect_sig = module.make_signature();
    ref_effect_sig.params.push(AbiParam::new(ptr_ty));
    ref_effect_sig.params.push(AbiParam::new(ptr_ty));
    ref_effect_sig.params.push(AbiParam::new(ptr_ty));
    ref_effect_sig.returns.push(AbiParam::new(types::I32));
    let add_ref = module
        .declare_function("rt_jit_add_ref", Linkage::Import, &ref_effect_sig)
        .map_err(module_err)?;
    let release = module
        .declare_function("rt_jit_release", Linkage::Import, &ref_effect_sig)
        .map_err(module_err)?;

    let mut as_new_project_class_slot_sig = module.make_signature();
    as_new_project_class_slot_sig
        .params
        .push(AbiParam::new(ptr_ty));
    as_new_project_class_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    as_new_project_class_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    as_new_project_class_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    as_new_project_class_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let as_new_project_class_slot = module
        .declare_function(
            "rt_jit_as_new_project_class_slot",
            Linkage::Import,
            &as_new_project_class_slot_sig,
        )
        .map_err(module_err)?;
    let as_new_collection_slot = module
        .declare_function(
            "rt_jit_as_new_collection_slot",
            Linkage::Import,
            &as_new_project_class_slot_sig,
        )
        .map_err(module_err)?;

    let mut new_collection_slot_sig = module.make_signature();
    for param in [ptr_ty, ptr_ty, types::I32, types::I32, types::I32] {
        new_collection_slot_sig.params.push(AbiParam::new(param));
    }
    new_collection_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let new_collection_slot = module
        .declare_function(
            "rt_jit_new_collection_to_slot",
            Linkage::Import,
            &new_collection_slot_sig,
        )
        .map_err(module_err)?;

    let mut new_object_slot_sig = module.make_signature();
    new_object_slot_sig.params.push(AbiParam::new(ptr_ty));
    new_object_slot_sig.params.push(AbiParam::new(ptr_ty));
    new_object_slot_sig.params.push(AbiParam::new(types::I32));
    new_object_slot_sig.params.push(AbiParam::new(types::I32));
    new_object_slot_sig.params.push(AbiParam::new(types::I32));
    new_object_slot_sig.params.push(AbiParam::new(types::I32));
    new_object_slot_sig.returns.push(AbiParam::new(types::I32));
    let new_object_slot = module
        .declare_function(
            "rt_jit_new_object_to_slot",
            Linkage::Import,
            &new_object_slot_sig,
        )
        .map_err(module_err)?;

    let mut predeclared_slot_sig = module.make_signature();
    predeclared_slot_sig.params.push(AbiParam::new(ptr_ty));
    predeclared_slot_sig.params.push(AbiParam::new(ptr_ty));
    predeclared_slot_sig.params.push(AbiParam::new(types::I32));
    predeclared_slot_sig.params.push(AbiParam::new(types::I32));
    predeclared_slot_sig.params.push(AbiParam::new(types::I32));
    predeclared_slot_sig.params.push(AbiParam::new(types::I32));
    predeclared_slot_sig.returns.push(AbiParam::new(types::I32));
    let predeclared_slot = module
        .declare_function(
            "rt_jit_predeclared_to_slot",
            Linkage::Import,
            &predeclared_slot_sig,
        )
        .map_err(module_err)?;

    let mut predeclared_set_sig = module.make_signature();
    predeclared_set_sig.params.push(AbiParam::new(ptr_ty));
    predeclared_set_sig.params.push(AbiParam::new(ptr_ty));
    predeclared_set_sig.params.push(AbiParam::new(types::I32));
    predeclared_set_sig.params.push(AbiParam::new(types::I32));
    predeclared_set_sig.params.push(AbiParam::new(ptr_ty));
    predeclared_set_sig.returns.push(AbiParam::new(types::I32));
    let predeclared_set = module
        .declare_function(
            "rt_jit_predeclared_set",
            Linkage::Import,
            &predeclared_set_sig,
        )
        .map_err(module_err)?;

    let mut field_get_slot_sig = module.make_signature();
    field_get_slot_sig.params.push(AbiParam::new(ptr_ty));
    field_get_slot_sig.params.push(AbiParam::new(ptr_ty));
    field_get_slot_sig.params.push(AbiParam::new(ptr_ty));
    field_get_slot_sig.params.push(AbiParam::new(types::I32));
    field_get_slot_sig.params.push(AbiParam::new(types::I32));
    field_get_slot_sig.params.push(AbiParam::new(types::I32));
    field_get_slot_sig.returns.push(AbiParam::new(types::I32));
    let field_get_slot = module
        .declare_function(
            "rt_jit_project_field_get_to_slot",
            Linkage::Import,
            &field_get_slot_sig,
        )
        .map_err(module_err)?;

    let mut field_set_slot_sig = module.make_signature();
    field_set_slot_sig.params.push(AbiParam::new(ptr_ty));
    field_set_slot_sig.params.push(AbiParam::new(ptr_ty));
    field_set_slot_sig.params.push(AbiParam::new(ptr_ty));
    field_set_slot_sig.params.push(AbiParam::new(types::I32));
    field_set_slot_sig.returns.push(AbiParam::new(types::I32));
    let field_set_slot = module
        .declare_function(
            "rt_jit_project_field_set",
            Linkage::Import,
            &field_set_slot_sig,
        )
        .map_err(module_err)?;

    let mut withevents_get_slot_sig = module.make_signature();
    for param in [ptr_ty, ptr_ty, ptr_ty, types::I32, types::I32, types::I32] {
        withevents_get_slot_sig.params.push(AbiParam::new(param));
    }
    withevents_get_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let withevents_get_slot = module
        .declare_function(
            "rt_jit_withevents_get_to_slot",
            Linkage::Import,
            &withevents_get_slot_sig,
        )
        .map_err(module_err)?;

    let mut withevents_set_slot_sig = module.make_signature();
    for param in [ptr_ty, ptr_ty, ptr_ty, types::I32, types::I32, types::I32] {
        withevents_set_slot_sig.params.push(AbiParam::new(param));
    }
    withevents_set_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let withevents_set_slot = module
        .declare_function(
            "rt_jit_withevents_set_to_slot",
            Linkage::Import,
            &withevents_set_slot_sig,
        )
        .map_err(module_err)?;

    let mut withevents_clear_owner_slot_sig = module.make_signature();
    for param in [ptr_ty, ptr_ty, ptr_ty, types::I32, types::I32] {
        withevents_clear_owner_slot_sig
            .params
            .push(AbiParam::new(param));
    }
    withevents_clear_owner_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let withevents_clear_owner_slot = module
        .declare_function(
            "rt_jit_withevents_clear_owner_to_slot",
            Linkage::Import,
            &withevents_clear_owner_slot_sig,
        )
        .map_err(module_err)?;

    let mut withevents_next_owner_slot_sig = module.make_signature();
    for param in [ptr_ty, ptr_ty, types::I32, types::I32] {
        withevents_next_owner_slot_sig
            .params
            .push(AbiParam::new(param));
    }
    withevents_next_owner_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let withevents_next_owner_slot = module
        .declare_function(
            "rt_jit_withevents_next_owner_to_slot",
            Linkage::Import,
            &withevents_next_owner_slot_sig,
        )
        .map_err(module_err)?;

    let withevents_first_owner_slot = module
        .declare_function(
            "rt_jit_withevents_first_owner_to_slot",
            Linkage::Import,
            &withevents_get_slot_sig,
        )
        .map_err(module_err)?;

    let mut raise_event_sig = module.make_signature();
    for param in [ptr_ty, ptr_ty, ptr_ty, types::I32, ptr_ty, types::I32] {
        raise_event_sig.params.push(AbiParam::new(param));
    }
    raise_event_sig.returns.push(AbiParam::new(types::I32));
    let raise_event = module
        .declare_function("rt_jit_raise_event", Linkage::Import, &raise_event_sig)
        .map_err(module_err)?;

    let mut project_member_get_slot_sig = module.make_signature();
    for param in [
        ptr_ty,
        ptr_ty,
        ptr_ty,
        ptr_ty,
        types::I32,
        types::I32,
        types::I32,
        ptr_ty,
        ptr_ty,
        types::I32,
        types::I32,
    ] {
        project_member_get_slot_sig
            .params
            .push(AbiParam::new(param));
    }
    project_member_get_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let project_member_get_slot = module
        .declare_function(
            "rt_jit_project_member_get_to_slot",
            Linkage::Import,
            &project_member_get_slot_sig,
        )
        .map_err(module_err)?;

    let mut call_by_name_slot_sig = module.make_signature();
    for param in [
        ptr_ty,
        ptr_ty,
        ptr_ty,
        types::I32,
        ptr_ty,
        ptr_ty,
        types::I32,
        types::I32,
    ] {
        call_by_name_slot_sig.params.push(AbiParam::new(param));
    }
    call_by_name_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let call_by_name_slot = module
        .declare_function(
            "rt_jit_call_by_name_to_slot",
            Linkage::Import,
            &call_by_name_slot_sig,
        )
        .map_err(module_err)?;

    let mut project_type_name_slot_sig = module.make_signature();
    project_type_name_slot_sig
        .params
        .push(AbiParam::new(ptr_ty));
    project_type_name_slot_sig
        .params
        .push(AbiParam::new(ptr_ty));
    project_type_name_slot_sig
        .params
        .push(AbiParam::new(ptr_ty));
    project_type_name_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    project_type_name_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    project_type_name_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let project_type_name_slot = module
        .declare_function(
            "rt_jit_project_type_name_to_slot",
            Linkage::Import,
            &project_type_name_slot_sig,
        )
        .map_err(module_err)?;

    let mut new_record_slot_sig = module.make_signature();
    new_record_slot_sig.params.push(AbiParam::new(ptr_ty));
    new_record_slot_sig.params.push(AbiParam::new(ptr_ty));
    new_record_slot_sig.params.push(AbiParam::new(ptr_ty));
    new_record_slot_sig.params.push(AbiParam::new(types::I32));
    new_record_slot_sig.params.push(AbiParam::new(types::I32));
    new_record_slot_sig.params.push(AbiParam::new(types::I32));
    new_record_slot_sig.returns.push(AbiParam::new(types::I32));
    let new_record_slot = module
        .declare_function(
            "rt_jit_new_record_to_slot",
            Linkage::Import,
            &new_record_slot_sig,
        )
        .map_err(module_err)?;

    let mut record_get_slot_sig = module.make_signature();
    for param in [ptr_ty, ptr_ty, ptr_ty, types::I32, types::I32, types::I32] {
        record_get_slot_sig.params.push(AbiParam::new(param));
    }
    record_get_slot_sig.returns.push(AbiParam::new(types::I32));
    let record_get_slot = module
        .declare_function(
            "rt_jit_record_get_to_slot",
            Linkage::Import,
            &record_get_slot_sig,
        )
        .map_err(module_err)?;

    let mut record_set_slot_sig = module.make_signature();
    for param in [ptr_ty, ptr_ty, types::I32, types::I32, types::I32, ptr_ty] {
        record_set_slot_sig.params.push(AbiParam::new(param));
    }
    record_set_slot_sig.returns.push(AbiParam::new(types::I32));
    let record_set_slot = module
        .declare_function("rt_jit_record_set", Linkage::Import, &record_set_slot_sig)
        .map_err(module_err)?;

    let mut record_lset_slot_sig = module.make_signature();
    for param in [ptr_ty, ptr_ty, types::I32, types::I32, ptr_ty] {
        record_lset_slot_sig.params.push(AbiParam::new(param));
    }
    record_lset_slot_sig.returns.push(AbiParam::new(types::I32));
    let record_lset_slot = module
        .declare_function("rt_jit_record_lset", Linkage::Import, &record_lset_slot_sig)
        .map_err(module_err)?;

    let mut record_array_get_slot_sig = module.make_signature();
    for param in [
        ptr_ty,
        ptr_ty,
        ptr_ty,
        types::I32,
        ptr_ty,
        types::I32,
        types::I32,
        types::I32,
    ] {
        record_array_get_slot_sig.params.push(AbiParam::new(param));
    }
    record_array_get_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let record_array_get_slot = module
        .declare_function(
            "rt_jit_record_array_get_to_slot",
            Linkage::Import,
            &record_array_get_slot_sig,
        )
        .map_err(module_err)?;

    let mut record_array_set_slot_sig = module.make_signature();
    for param in [
        ptr_ty,
        ptr_ty,
        types::I32,
        types::I32,
        types::I32,
        ptr_ty,
        types::I32,
        ptr_ty,
    ] {
        record_array_set_slot_sig.params.push(AbiParam::new(param));
    }
    record_array_set_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let record_array_set_slot = module
        .declare_function(
            "rt_jit_record_array_set",
            Linkage::Import,
            &record_array_set_slot_sig,
        )
        .map_err(module_err)?;

    let mut field_array_get_slot_sig = module.make_signature();
    for param in [
        ptr_ty,
        ptr_ty,
        ptr_ty,
        types::I32,
        ptr_ty,
        types::I32,
        types::I32,
        types::I32,
    ] {
        field_array_get_slot_sig.params.push(AbiParam::new(param));
    }
    field_array_get_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let field_array_get_slot = module
        .declare_function(
            "rt_jit_project_field_array_get_to_slot",
            Linkage::Import,
            &field_array_get_slot_sig,
        )
        .map_err(module_err)?;

    let mut field_array_set_slot_sig = module.make_signature();
    for param in [
        ptr_ty,
        ptr_ty,
        ptr_ty,
        types::I32,
        ptr_ty,
        types::I32,
        ptr_ty,
    ] {
        field_array_set_slot_sig.params.push(AbiParam::new(param));
    }
    field_array_set_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let field_array_set_slot = module
        .declare_function(
            "rt_jit_project_field_array_set",
            Linkage::Import,
            &field_array_set_slot_sig,
        )
        .map_err(module_err)?;

    let mut validate_assignment_sig = module.make_signature();
    validate_assignment_sig.params.push(AbiParam::new(ptr_ty));
    validate_assignment_sig.params.push(AbiParam::new(ptr_ty));
    validate_assignment_sig.params.push(AbiParam::new(ptr_ty));
    validate_assignment_sig
        .params
        .push(AbiParam::new(types::I32));
    validate_assignment_sig
        .params
        .push(AbiParam::new(types::I32));
    validate_assignment_sig.params.push(AbiParam::new(ptr_ty));
    validate_assignment_sig
        .params
        .push(AbiParam::new(types::I32));
    validate_assignment_sig
        .returns
        .push(AbiParam::new(types::I32));
    let validate_assignment = module
        .declare_function(
            "rt_jit_validate_assignment",
            Linkage::Import,
            &validate_assignment_sig,
        )
        .map_err(module_err)?;

    let mut err_clear_sig = module.make_signature();
    err_clear_sig.params.push(AbiParam::new(ptr_ty));
    err_clear_sig.returns.push(AbiParam::new(types::I32));
    let err_clear = module
        .declare_function("rt_err_clear", Linkage::Import, &err_clear_sig)
        .map_err(module_err)?;

    let mut err_i32_field_sig = module.make_signature();
    err_i32_field_sig.params.push(AbiParam::new(ptr_ty));
    err_i32_field_sig.params.push(AbiParam::new(types::I32));
    err_i32_field_sig.params.push(AbiParam::new(ptr_ty));
    err_i32_field_sig.returns.push(AbiParam::new(types::I32));
    let err_i32_field = module
        .declare_function("rt_err_i32_field", Linkage::Import, &err_i32_field_sig)
        .map_err(module_err)?;

    let mut err_string_field_utf8_sig = module.make_signature();
    err_string_field_utf8_sig.params.push(AbiParam::new(ptr_ty));
    err_string_field_utf8_sig
        .params
        .push(AbiParam::new(types::I32));
    err_string_field_utf8_sig.params.push(AbiParam::new(ptr_ty));
    err_string_field_utf8_sig.params.push(AbiParam::new(ptr_ty));
    err_string_field_utf8_sig
        .returns
        .push(AbiParam::new(types::I32));
    let err_string_field_utf8 = module
        .declare_function(
            "rt_err_string_field_utf8",
            Linkage::Import,
            &err_string_field_utf8_sig,
        )
        .map_err(module_err)?;

    let mut resume_sig = module.make_signature();
    resume_sig.params.push(AbiParam::new(ptr_ty));
    resume_sig.params.push(AbiParam::new(types::I32));
    resume_sig.params.push(AbiParam::new(types::I32));
    resume_sig.params.push(AbiParam::new(ptr_ty));
    resume_sig.returns.push(AbiParam::new(types::I32));
    let resume = module
        .declare_function("rt_resume", Linkage::Import, &resume_sig)
        .map_err(module_err)?;

    let mut set_error_handler_sig = module.make_signature();
    set_error_handler_sig.params.push(AbiParam::new(ptr_ty));
    set_error_handler_sig.params.push(AbiParam::new(types::I32));
    set_error_handler_sig.params.push(AbiParam::new(types::I32));
    set_error_handler_sig
        .returns
        .push(AbiParam::new(types::I32));
    let set_error_handler = module
        .declare_function(
            "rt_set_error_handler",
            Linkage::Import,
            &set_error_handler_sig,
        )
        .map_err(module_err)?;

    let mut route_fault_sig = module.make_signature();
    route_fault_sig.params.push(AbiParam::new(ptr_ty));
    route_fault_sig.params.push(AbiParam::new(types::I32));
    route_fault_sig.params.push(AbiParam::new(types::I32));
    route_fault_sig.params.push(AbiParam::new(types::I32));
    route_fault_sig.params.push(AbiParam::new(ptr_ty));
    route_fault_sig.params.push(AbiParam::new(ptr_ty));
    route_fault_sig.returns.push(AbiParam::new(types::I32));
    let route_fault = module
        .declare_function("rt_route_fault", Linkage::Import, &route_fault_sig)
        .map_err(module_err)?;

    let mut raise_error_number_sig = module.make_signature();
    raise_error_number_sig.params.push(AbiParam::new(ptr_ty));
    raise_error_number_sig
        .params
        .push(AbiParam::new(types::I32));
    raise_error_number_sig
        .params
        .push(AbiParam::new(types::I32));
    raise_error_number_sig.params.push(AbiParam::new(ptr_ty));
    raise_error_number_sig
        .params
        .push(AbiParam::new(types::I32));
    raise_error_number_sig
        .returns
        .push(AbiParam::new(types::I32));
    let raise_error_number = module
        .declare_function(
            "rt_raise_error_number",
            Linkage::Import,
            &raise_error_number_sig,
        )
        .map_err(module_err)?;

    let mut gosub_push_sig = module.make_signature();
    gosub_push_sig.params.push(AbiParam::new(ptr_ty));
    gosub_push_sig.params.push(AbiParam::new(types::I32));
    gosub_push_sig.returns.push(AbiParam::new(types::I32));
    let gosub_push = module
        .declare_function("rt_jit_gosub_push", Linkage::Import, &gosub_push_sig)
        .map_err(module_err)?;

    let mut gosub_pop_sig = module.make_signature();
    gosub_pop_sig.params.push(AbiParam::new(ptr_ty));
    gosub_pop_sig.params.push(AbiParam::new(ptr_ty));
    gosub_pop_sig.params.push(AbiParam::new(ptr_ty));
    gosub_pop_sig.returns.push(AbiParam::new(types::I32));
    let gosub_pop = module
        .declare_function("rt_jit_gosub_pop", Linkage::Import, &gosub_pop_sig)
        .map_err(module_err)?;

    let mut direct_enter_noarg_sub_sig = module.make_signature();
    direct_enter_noarg_sub_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_noarg_sub_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_noarg_sub_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_noarg_sub_sig
        .returns
        .push(AbiParam::new(types::I32));
    let direct_enter_noarg_sub = module
        .declare_function(
            "rt_jit_direct_enter_noarg_sub",
            Linkage::Import,
            &direct_enter_noarg_sub_sig,
        )
        .map_err(module_err)?;

    let mut direct_exit_noarg_sub_sig = module.make_signature();
    direct_exit_noarg_sub_sig.params.push(AbiParam::new(ptr_ty));
    direct_exit_noarg_sub_sig.params.push(AbiParam::new(ptr_ty));
    direct_exit_noarg_sub_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_exit_noarg_sub_sig
        .returns
        .push(AbiParam::new(types::I32));
    let direct_exit_noarg_sub = module
        .declare_function(
            "rt_jit_direct_exit_noarg_sub",
            Linkage::Import,
            &direct_exit_noarg_sub_sig,
        )
        .map_err(module_err)?;

    let direct_enter_noarg_func = module
        .declare_function(
            "rt_jit_direct_enter_noarg_func",
            Linkage::Import,
            &direct_enter_noarg_sub_sig,
        )
        .map_err(module_err)?;

    let mut direct_exit_noarg_func_sig = module.make_signature();
    direct_exit_noarg_func_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_exit_noarg_func_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_exit_noarg_func_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_exit_noarg_func_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_exit_noarg_func_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_exit_noarg_func_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_exit_noarg_func_sig
        .returns
        .push(AbiParam::new(types::I32));
    let direct_exit_noarg_func = module
        .declare_function(
            "rt_jit_direct_exit_noarg_func",
            Linkage::Import,
            &direct_exit_noarg_func_sig,
        )
        .map_err(module_err)?;

    let mut direct_enter_one_i32_sub_sig = module.make_signature();
    direct_enter_one_i32_sub_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_one_i32_sub_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_one_i32_sub_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_one_i32_sub_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_one_i32_sub_sig
        .returns
        .push(AbiParam::new(types::I32));
    let direct_enter_one_i32_sub = module
        .declare_function(
            "rt_jit_direct_enter_one_i32_sub",
            Linkage::Import,
            &direct_enter_one_i32_sub_sig,
        )
        .map_err(module_err)?;

    let mut direct_enter_one_i32_func_sig = module.make_signature();
    direct_enter_one_i32_func_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_one_i32_func_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_one_i32_func_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_one_i32_func_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_one_i32_func_sig
        .returns
        .push(AbiParam::new(types::I32));
    let direct_enter_one_i32_func = module
        .declare_function(
            "rt_jit_direct_enter_one_i32_func",
            Linkage::Import,
            &direct_enter_one_i32_func_sig,
        )
        .map_err(module_err)?;

    let mut direct_enter_one_i32_byref_sub_sig = module.make_signature();
    direct_enter_one_i32_byref_sub_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_one_i32_byref_sub_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_one_i32_byref_sub_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_one_i32_byref_sub_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_one_i32_byref_sub_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_one_i32_byref_sub_sig
        .returns
        .push(AbiParam::new(types::I32));
    let direct_enter_one_i32_byref_sub = module
        .declare_function(
            "rt_jit_direct_enter_one_i32_byref_sub",
            Linkage::Import,
            &direct_enter_one_i32_byref_sub_sig,
        )
        .map_err(module_err)?;

    let mut direct_enter_one_i32_byref_func_sig = module.make_signature();
    direct_enter_one_i32_byref_func_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_one_i32_byref_func_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_one_i32_byref_func_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_one_i32_byref_func_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_one_i32_byref_func_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_one_i32_byref_func_sig
        .returns
        .push(AbiParam::new(types::I32));
    let direct_enter_one_i32_byref_func = module
        .declare_function(
            "rt_jit_direct_enter_one_i32_byref_func",
            Linkage::Import,
            &direct_enter_one_i32_byref_func_sig,
        )
        .map_err(module_err)?;

    let mut direct_enter_two_i32_sub_sig = module.make_signature();
    direct_enter_two_i32_sub_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_two_i32_sub_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_two_i32_sub_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_two_i32_sub_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_two_i32_sub_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_two_i32_sub_sig
        .returns
        .push(AbiParam::new(types::I32));
    let direct_enter_two_i32_sub = module
        .declare_function(
            "rt_jit_direct_enter_two_i32_sub",
            Linkage::Import,
            &direct_enter_two_i32_sub_sig,
        )
        .map_err(module_err)?;

    let mut direct_enter_two_i32_func_sig = module.make_signature();
    direct_enter_two_i32_func_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_two_i32_func_sig
        .params
        .push(AbiParam::new(ptr_ty));
    direct_enter_two_i32_func_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_two_i32_func_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_two_i32_func_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_two_i32_func_sig
        .returns
        .push(AbiParam::new(types::I32));
    let direct_enter_two_i32_func = module
        .declare_function(
            "rt_jit_direct_enter_two_i32_func",
            Linkage::Import,
            &direct_enter_two_i32_func_sig,
        )
        .map_err(module_err)?;

    let mut direct_enter_proc_i32_sig = module.make_signature();
    direct_enter_proc_i32_sig.params.push(AbiParam::new(ptr_ty));
    direct_enter_proc_i32_sig.params.push(AbiParam::new(ptr_ty));
    direct_enter_proc_i32_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_proc_i32_sig
        .params
        .push(AbiParam::new(types::I32));
    direct_enter_proc_i32_sig.params.push(AbiParam::new(ptr_ty));
    direct_enter_proc_i32_sig
        .returns
        .push(AbiParam::new(types::I32));
    let direct_enter_proc_i32 = module
        .declare_function(
            "rt_jit_direct_enter_proc_i32",
            Linkage::Import,
            &direct_enter_proc_i32_sig,
        )
        .map_err(module_err)?;

    let mut expect_proc_ref_i32_sig = module.make_signature();
    expect_proc_ref_i32_sig.params.push(AbiParam::new(ptr_ty));
    expect_proc_ref_i32_sig.params.push(AbiParam::new(ptr_ty));
    expect_proc_ref_i32_sig
        .params
        .push(AbiParam::new(types::I32));
    expect_proc_ref_i32_sig
        .params
        .push(AbiParam::new(types::I32));
    expect_proc_ref_i32_sig
        .params
        .push(AbiParam::new(types::I32));
    expect_proc_ref_i32_sig
        .returns
        .push(AbiParam::new(types::I32));
    let expect_proc_ref_i32 = module
        .declare_function(
            "rt_jit_expect_proc_ref_i32",
            Linkage::Import,
            &expect_proc_ref_i32_sig,
        )
        .map_err(module_err)?;

    let mut array_literal_slot_sig = module.make_signature();
    array_literal_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_literal_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_literal_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_literal_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_literal_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_literal_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_literal_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_literal_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let array_literal_slot = module
        .declare_function(
            "rt_jit_array_literal_to_slot",
            Linkage::Import,
            &array_literal_slot_sig,
        )
        .map_err(module_err)?;

    let mut array_redim_slot_sig = module.make_signature();
    array_redim_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_redim_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_redim_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_redim_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_redim_slot_sig.params.push(AbiParam::new(types::I32));
    array_redim_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_redim_slot_sig.params.push(AbiParam::new(types::I32));
    array_redim_slot_sig.params.push(AbiParam::new(types::I32));
    array_redim_slot_sig.params.push(AbiParam::new(types::I32));
    array_redim_slot_sig.params.push(AbiParam::new(types::I32));
    array_redim_slot_sig.returns.push(AbiParam::new(types::I32));
    let array_redim_slot = module
        .declare_function(
            "rt_jit_array_redim_to_slot",
            Linkage::Import,
            &array_redim_slot_sig,
        )
        .map_err(module_err)?;

    let mut array_erase_slot_sig = module.make_signature();
    array_erase_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_erase_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_erase_slot_sig.params.push(AbiParam::new(types::I32));
    array_erase_slot_sig.params.push(AbiParam::new(types::I32));
    array_erase_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_erase_slot_sig.returns.push(AbiParam::new(types::I32));
    let array_erase_slot = module
        .declare_function(
            "rt_jit_array_erase_variant_slot",
            Linkage::Import,
            &array_erase_slot_sig,
        )
        .map_err(module_err)?;

    let mut array_get_slot_sig = module.make_signature();
    array_get_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_get_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_get_slot_sig.params.push(AbiParam::new(types::I32));
    array_get_slot_sig.params.push(AbiParam::new(types::I32));
    array_get_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_get_slot_sig.params.push(AbiParam::new(types::I32));
    array_get_slot_sig.params.push(AbiParam::new(types::I32));
    array_get_slot_sig.params.push(AbiParam::new(types::I32));
    array_get_slot_sig.returns.push(AbiParam::new(types::I32));
    let array_get_slot = module
        .declare_function(
            "rt_jit_array_get_variant_to_slot",
            Linkage::Import,
            &array_get_slot_sig,
        )
        .map_err(module_err)?;

    let mut array_get_i32_1d_slot_sig = module.make_signature();
    array_get_i32_1d_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_get_i32_1d_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_get_i32_1d_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_get_i32_1d_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_get_i32_1d_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_get_i32_1d_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_get_i32_1d_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_get_i32_1d_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let array_get_i32_1d_slot = module
        .declare_function(
            "rt_jit_array_get_i32_1d_to_slot",
            Linkage::Import,
            &array_get_i32_1d_slot_sig,
        )
        .map_err(module_err)?;

    let mut array_set_slot_sig = module.make_signature();
    array_set_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_set_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_set_slot_sig.params.push(AbiParam::new(types::I32));
    array_set_slot_sig.params.push(AbiParam::new(types::I32));
    array_set_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_set_slot_sig.params.push(AbiParam::new(types::I32));
    array_set_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_set_slot_sig.returns.push(AbiParam::new(types::I32));
    let array_set_slot = module
        .declare_function(
            "rt_jit_array_set_variant_slot",
            Linkage::Import,
            &array_set_slot_sig,
        )
        .map_err(module_err)?;

    let mut array_set_i32_1d_slot_sig = module.make_signature();
    array_set_i32_1d_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_set_i32_1d_slot_sig.params.push(AbiParam::new(ptr_ty));
    array_set_i32_1d_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_set_i32_1d_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_set_i32_1d_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_set_i32_1d_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    array_set_i32_1d_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let array_set_i32_1d_slot = module
        .declare_function(
            "rt_jit_array_set_i32_1d_slot",
            Linkage::Import,
            &array_set_i32_1d_slot_sig,
        )
        .map_err(module_err)?;

    let mut bound_slot_sig = module.make_signature();
    bound_slot_sig.params.push(AbiParam::new(ptr_ty));
    bound_slot_sig.params.push(AbiParam::new(ptr_ty));
    bound_slot_sig.params.push(AbiParam::new(ptr_ty));
    bound_slot_sig.params.push(AbiParam::new(types::I32));
    bound_slot_sig.params.push(AbiParam::new(types::I32));
    bound_slot_sig.params.push(AbiParam::new(types::I32));
    bound_slot_sig.params.push(AbiParam::new(types::I32));
    bound_slot_sig.returns.push(AbiParam::new(types::I32));
    let bound_slot = module
        .declare_function("rt_jit_bound_to_slot", Linkage::Import, &bound_slot_sig)
        .map_err(module_err)?;

    let mut for_each_init_slot_sig = module.make_signature();
    for_each_init_slot_sig.params.push(AbiParam::new(ptr_ty));
    for_each_init_slot_sig.params.push(AbiParam::new(ptr_ty));
    for_each_init_slot_sig.params.push(AbiParam::new(ptr_ty));
    for_each_init_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    for_each_init_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    for_each_init_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let for_each_init_slot = module
        .declare_function(
            "rt_jit_for_each_init_variant_array",
            Linkage::Import,
            &for_each_init_slot_sig,
        )
        .map_err(module_err)?;

    let mut for_each_next_slot_sig = module.make_signature();
    for_each_next_slot_sig.params.push(AbiParam::new(ptr_ty));
    for_each_next_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    for_each_next_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    for_each_next_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    for_each_next_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    for_each_next_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    for_each_next_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    for_each_next_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let for_each_next_slot = module
        .declare_function(
            "rt_jit_for_each_next_variant_array",
            Linkage::Import,
            &for_each_next_slot_sig,
        )
        .map_err(module_err)?;

    let mut unbox_slot_sig = module.make_signature();
    unbox_slot_sig.params.push(AbiParam::new(ptr_ty));
    unbox_slot_sig.params.push(AbiParam::new(ptr_ty));
    unbox_slot_sig.params.push(AbiParam::new(types::I32));
    unbox_slot_sig.params.push(AbiParam::new(types::I32));
    unbox_slot_sig.params.push(AbiParam::new(ptr_ty));
    unbox_slot_sig.params.push(AbiParam::new(types::I32));
    unbox_slot_sig.params.push(AbiParam::new(types::I32));
    unbox_slot_sig.returns.push(AbiParam::new(types::I32));
    let unbox_slot = module
        .declare_function("rt_jit_unbox_to_slot", Linkage::Import, &unbox_slot_sig)
        .map_err(module_err)?;

    let mut call_extern_proc_sig = module.make_signature();
    call_extern_proc_sig.params.push(AbiParam::new(ptr_ty));
    call_extern_proc_sig.params.push(AbiParam::new(ptr_ty));
    call_extern_proc_sig.params.push(AbiParam::new(types::I32));
    call_extern_proc_sig.params.push(AbiParam::new(types::I32));
    call_extern_proc_sig.params.push(AbiParam::new(types::I32));
    call_extern_proc_sig.params.push(AbiParam::new(ptr_ty));
    call_extern_proc_sig.params.push(AbiParam::new(types::I32));
    call_extern_proc_sig.params.push(AbiParam::new(types::I32));
    call_extern_proc_sig.returns.push(AbiParam::new(types::I32));
    let call_extern_proc_i32 = module
        .declare_function(
            "rt_jit_call_extern_proc_i32",
            Linkage::Import,
            &call_extern_proc_sig,
        )
        .map_err(module_err)?;

    let mut call_proc_ref_sig = module.make_signature();
    call_proc_ref_sig.params.push(AbiParam::new(ptr_ty));
    call_proc_ref_sig.params.push(AbiParam::new(ptr_ty));
    call_proc_ref_sig.params.push(AbiParam::new(types::I32));
    call_proc_ref_sig.params.push(AbiParam::new(types::I32));
    call_proc_ref_sig.params.push(AbiParam::new(types::I32));
    call_proc_ref_sig.params.push(AbiParam::new(types::I32));
    call_proc_ref_sig.params.push(AbiParam::new(types::I32));
    call_proc_ref_sig.params.push(AbiParam::new(ptr_ty));
    call_proc_ref_sig.params.push(AbiParam::new(types::I32));
    call_proc_ref_sig.params.push(AbiParam::new(types::I32));
    call_proc_ref_sig.returns.push(AbiParam::new(types::I32));
    let call_proc_ref_i32 = module
        .declare_function(
            "rt_jit_call_proc_ref_i32",
            Linkage::Import,
            &call_proc_ref_sig,
        )
        .map_err(module_err)?;

    let mut lib_invoke_slot_sig = module.make_signature();
    lib_invoke_slot_sig.params.push(AbiParam::new(ptr_ty));
    lib_invoke_slot_sig.params.push(AbiParam::new(ptr_ty));
    lib_invoke_slot_sig.params.push(AbiParam::new(types::I32));
    lib_invoke_slot_sig.params.push(AbiParam::new(types::I32));
    lib_invoke_slot_sig.params.push(AbiParam::new(ptr_ty));
    lib_invoke_slot_sig.params.push(AbiParam::new(types::I32));
    lib_invoke_slot_sig.params.push(AbiParam::new(types::I32));
    lib_invoke_slot_sig.params.push(AbiParam::new(types::I32));
    lib_invoke_slot_sig.returns.push(AbiParam::new(types::I32));
    let lib_invoke_slot = module
        .declare_function(
            "rt_jit_lib_invoke_to_slot",
            Linkage::Import,
            &lib_invoke_slot_sig,
        )
        .map_err(module_err)?;
    let mut declare_call_slot_sig = module.make_signature();
    declare_call_slot_sig.params.push(AbiParam::new(ptr_ty));
    declare_call_slot_sig.params.push(AbiParam::new(ptr_ty));
    declare_call_slot_sig.params.push(AbiParam::new(types::I32));
    declare_call_slot_sig.params.push(AbiParam::new(types::I32));
    declare_call_slot_sig.params.push(AbiParam::new(ptr_ty));
    declare_call_slot_sig.params.push(AbiParam::new(types::I32));
    declare_call_slot_sig.params.push(AbiParam::new(types::I32));
    declare_call_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let declare_call_slot = module
        .declare_function(
            "rt_jit_declare_call_to_slot",
            Linkage::Import,
            &declare_call_slot_sig,
        )
        .map_err(module_err)?;

    let mut arith_v_slot_sig = module.make_signature();
    arith_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    arith_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    arith_v_slot_sig.params.push(AbiParam::new(types::I32));
    arith_v_slot_sig.params.push(AbiParam::new(types::I32));
    arith_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    arith_v_slot_sig.params.push(AbiParam::new(types::I32));
    arith_v_slot_sig.params.push(AbiParam::new(types::I32));
    arith_v_slot_sig.returns.push(AbiParam::new(types::I32));
    let arith_v_slot = module
        .declare_function("rt_jit_arith_v_to_slot", Linkage::Import, &arith_v_slot_sig)
        .map_err(module_err)?;

    let mut concat_v_slot_sig = module.make_signature();
    concat_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    concat_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    concat_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    concat_v_slot_sig.params.push(AbiParam::new(types::I32));
    concat_v_slot_sig.params.push(AbiParam::new(types::I32));
    concat_v_slot_sig.returns.push(AbiParam::new(types::I32));
    let concat_v_slot = module
        .declare_function(
            "rt_jit_concat_v_to_slot",
            Linkage::Import,
            &concat_v_slot_sig,
        )
        .map_err(module_err)?;

    let mut neg_v_slot_sig = module.make_signature();
    neg_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    neg_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    neg_v_slot_sig.params.push(AbiParam::new(types::I32));
    neg_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    neg_v_slot_sig.params.push(AbiParam::new(types::I32));
    neg_v_slot_sig.params.push(AbiParam::new(types::I32));
    neg_v_slot_sig.returns.push(AbiParam::new(types::I32));
    let neg_v_slot = module
        .declare_function("rt_jit_neg_v_to_slot", Linkage::Import, &neg_v_slot_sig)
        .map_err(module_err)?;

    let mut compare_v_slot_sig = module.make_signature();
    compare_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    compare_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    compare_v_slot_sig.params.push(AbiParam::new(types::I32));
    compare_v_slot_sig.params.push(AbiParam::new(types::I32));
    compare_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    compare_v_slot_sig.params.push(AbiParam::new(types::I32));
    compare_v_slot_sig.params.push(AbiParam::new(types::I32));
    compare_v_slot_sig.returns.push(AbiParam::new(types::I32));
    let compare_v_slot = module
        .declare_function(
            "rt_jit_compare_v_to_slot",
            Linkage::Import,
            &compare_v_slot_sig,
        )
        .map_err(module_err)?;

    let mut compare_object_is_slot_sig = module.make_signature();
    compare_object_is_slot_sig
        .params
        .push(AbiParam::new(ptr_ty));
    compare_object_is_slot_sig
        .params
        .push(AbiParam::new(ptr_ty));
    compare_object_is_slot_sig
        .params
        .push(AbiParam::new(ptr_ty));
    compare_object_is_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    compare_object_is_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    compare_object_is_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let compare_object_is_slot = module
        .declare_function(
            "rt_jit_compare_object_is_to_bool_slot",
            Linkage::Import,
            &compare_object_is_slot_sig,
        )
        .map_err(module_err)?;

    let mut type_of_is_slot_sig = module.make_signature();
    type_of_is_slot_sig.params.push(AbiParam::new(ptr_ty));
    type_of_is_slot_sig.params.push(AbiParam::new(ptr_ty));
    type_of_is_slot_sig.params.push(AbiParam::new(ptr_ty));
    type_of_is_slot_sig.params.push(AbiParam::new(ptr_ty));
    type_of_is_slot_sig.params.push(AbiParam::new(types::I32));
    type_of_is_slot_sig.params.push(AbiParam::new(types::I32));
    type_of_is_slot_sig.params.push(AbiParam::new(types::I32));
    type_of_is_slot_sig.returns.push(AbiParam::new(types::I32));
    let type_of_is_slot = module
        .declare_function(
            "rt_jit_type_of_is_to_bool_slot",
            Linkage::Import,
            &type_of_is_slot_sig,
        )
        .map_err(module_err)?;

    let mut logical_v_slot_sig = module.make_signature();
    logical_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    logical_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    logical_v_slot_sig.params.push(AbiParam::new(types::I32));
    logical_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    logical_v_slot_sig.params.push(AbiParam::new(types::I32));
    logical_v_slot_sig.params.push(AbiParam::new(types::I32));
    logical_v_slot_sig.returns.push(AbiParam::new(types::I32));
    let logical_v_slot = module
        .declare_function(
            "rt_jit_logical_v_to_slot",
            Linkage::Import,
            &logical_v_slot_sig,
        )
        .map_err(module_err)?;

    let mut not_v_slot_sig = module.make_signature();
    not_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    not_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    not_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    not_v_slot_sig.params.push(AbiParam::new(types::I32));
    not_v_slot_sig.params.push(AbiParam::new(types::I32));
    not_v_slot_sig.returns.push(AbiParam::new(types::I32));
    let not_v_slot = module
        .declare_function("rt_jit_not_v_to_slot", Linkage::Import, &not_v_slot_sig)
        .map_err(module_err)?;

    let mut truthy_v_slot_sig = module.make_signature();
    truthy_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    truthy_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    truthy_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    truthy_v_slot_sig.params.push(AbiParam::new(types::I32));
    truthy_v_slot_sig.params.push(AbiParam::new(types::I32));
    truthy_v_slot_sig.returns.push(AbiParam::new(types::I32));
    let truthy_v_slot = module
        .declare_function(
            "rt_jit_truthy_v_to_bool_slot",
            Linkage::Import,
            &truthy_v_slot_sig,
        )
        .map_err(module_err)?;

    let mut variant_changed_slot_sig = module.make_signature();
    variant_changed_slot_sig.params.push(AbiParam::new(ptr_ty));
    variant_changed_slot_sig.params.push(AbiParam::new(ptr_ty));
    variant_changed_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    variant_changed_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    variant_changed_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let variant_changed_slot = module
        .declare_function(
            "rt_jit_variant_changed_to_bool_slot",
            Linkage::Import,
            &variant_changed_slot_sig,
        )
        .map_err(module_err)?;

    let mut coerce_numeric_v_slot_sig = module.make_signature();
    coerce_numeric_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    coerce_numeric_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    coerce_numeric_v_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    coerce_numeric_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    coerce_numeric_v_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    coerce_numeric_v_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    coerce_numeric_v_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let coerce_numeric_v_slot = module
        .declare_function(
            "rt_jit_coerce_numeric_v_to_slot",
            Linkage::Import,
            &coerce_numeric_v_slot_sig,
        )
        .map_err(module_err)?;

    let mut coerce_string_v_slot_sig = module.make_signature();
    coerce_string_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    coerce_string_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    coerce_string_v_slot_sig.params.push(AbiParam::new(ptr_ty));
    coerce_string_v_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    coerce_string_v_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    coerce_string_v_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let coerce_string_v_slot = module
        .declare_function(
            "rt_jit_coerce_string_v_to_slot",
            Linkage::Import,
            &coerce_string_v_slot_sig,
        )
        .map_err(module_err)?;

    let mut coerce_fixed_string_v_slot_sig = module.make_signature();
    coerce_fixed_string_v_slot_sig
        .params
        .push(AbiParam::new(ptr_ty));
    coerce_fixed_string_v_slot_sig
        .params
        .push(AbiParam::new(ptr_ty));
    coerce_fixed_string_v_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    coerce_fixed_string_v_slot_sig
        .params
        .push(AbiParam::new(ptr_ty));
    coerce_fixed_string_v_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    coerce_fixed_string_v_slot_sig
        .params
        .push(AbiParam::new(types::I32));
    coerce_fixed_string_v_slot_sig
        .returns
        .push(AbiParam::new(types::I32));
    let coerce_fixed_string_v_slot = module
        .declare_function(
            "rt_jit_coerce_fixed_string_v_to_slot",
            Linkage::Import,
            &coerce_fixed_string_v_slot_sig,
        )
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
    let div_i32_slot = module
        .declare_function("rt_jit_div_i32_to_slot", Linkage::Import, &slot_sig)
        .map_err(module_err)?;
    let rem_i32_slot = module
        .declare_function("rt_jit_rem_i32_to_slot", Linkage::Import, &slot_sig)
        .map_err(module_err)?;
    let add_i16_slot = module
        .declare_function("rt_jit_add_i16_to_slot", Linkage::Import, &slot_sig)
        .map_err(module_err)?;
    let sub_i16_slot = module
        .declare_function("rt_jit_sub_i16_to_slot", Linkage::Import, &slot_sig)
        .map_err(module_err)?;
    let mul_i16_slot = module
        .declare_function("rt_jit_mul_i16_to_slot", Linkage::Import, &slot_sig)
        .map_err(module_err)?;
    let add_u8_slot = module
        .declare_function("rt_jit_add_u8_to_slot", Linkage::Import, &slot_sig)
        .map_err(module_err)?;
    let sub_u8_slot = module
        .declare_function("rt_jit_sub_u8_to_slot", Linkage::Import, &slot_sig)
        .map_err(module_err)?;
    let mul_u8_slot = module
        .declare_function("rt_jit_mul_u8_to_slot", Linkage::Import, &slot_sig)
        .map_err(module_err)?;
    let mut slot_i64_sig = module.make_signature();
    slot_i64_sig.params.push(AbiParam::new(ptr_ty));
    slot_i64_sig.params.push(AbiParam::new(ptr_ty));
    slot_i64_sig.params.push(AbiParam::new(types::I64));
    slot_i64_sig.params.push(AbiParam::new(types::I64));
    slot_i64_sig.params.push(AbiParam::new(types::I32));
    slot_i64_sig.params.push(AbiParam::new(types::I32));
    slot_i64_sig.returns.push(AbiParam::new(types::I32));
    let add_i64_slot = module
        .declare_function("rt_jit_add_i64_to_slot", Linkage::Import, &slot_i64_sig)
        .map_err(module_err)?;
    let sub_i64_slot = module
        .declare_function("rt_jit_sub_i64_to_slot", Linkage::Import, &slot_i64_sig)
        .map_err(module_err)?;
    let mul_i64_slot = module
        .declare_function("rt_jit_mul_i64_to_slot", Linkage::Import, &slot_i64_sig)
        .map_err(module_err)?;
    let div_i64_slot = module
        .declare_function("rt_jit_div_i64_to_slot", Linkage::Import, &slot_i64_sig)
        .map_err(module_err)?;
    let rem_i64_slot = module
        .declare_function("rt_jit_rem_i64_to_slot", Linkage::Import, &slot_i64_sig)
        .map_err(module_err)?;
    let add_currency_slot = module
        .declare_function(
            "rt_jit_add_currency_to_slot",
            Linkage::Import,
            &slot_i64_sig,
        )
        .map_err(module_err)?;
    let sub_currency_slot = module
        .declare_function(
            "rt_jit_sub_currency_to_slot",
            Linkage::Import,
            &slot_i64_sig,
        )
        .map_err(module_err)?;
    let mul_currency_slot = module
        .declare_function(
            "rt_jit_mul_currency_to_slot",
            Linkage::Import,
            &slot_i64_sig,
        )
        .map_err(module_err)?;

    Ok(Imports {
        load_i32,
        load_i64,
        load_f32,
        load_f64,
        pack_f32_arg,
        pack_f64_arg,
        store_i32,
        store_i64,
        store_f32,
        store_f64,
        store_currency_i64,
        store_date_f64,
        store_u8,
        store_i16,
        load_bool,
        store_bool,
        store_proc_ref,
        store_variant,
        stmt_boundary,
        drain_terminations,
        add_ref,
        release,
        as_new_project_class_slot,
        as_new_collection_slot,
        new_collection_slot,
        new_object_slot,
        predeclared_slot,
        predeclared_set,
        field_get_slot,
        field_set_slot,
        withevents_get_slot,
        withevents_set_slot,
        withevents_clear_owner_slot,
        withevents_first_owner_slot,
        withevents_next_owner_slot,
        raise_event,
        project_member_get_slot,
        call_by_name_slot,
        project_type_name_slot,
        new_record_slot,
        record_get_slot,
        record_set_slot,
        record_lset_slot,
        record_array_get_slot,
        record_array_set_slot,
        field_array_get_slot,
        field_array_set_slot,
        validate_assignment,
        err_clear,
        err_i32_field,
        err_string_field_utf8,
        erl_get,
        err_set_field,
        set_line_number,
        current_line,
        set_error_handler,
        resume,
        route_fault,
        raise_error_number,
        gosub_push,
        gosub_pop,
        direct_enter_noarg_sub,
        direct_exit_noarg_sub,
        direct_enter_noarg_func,
        direct_exit_noarg_func,
        direct_enter_one_i32_sub,
        direct_enter_one_i32_func,
        direct_enter_one_i32_byref_sub,
        direct_enter_one_i32_byref_func,
        direct_enter_two_i32_sub,
        direct_enter_two_i32_func,
        direct_enter_proc_i32,
        expect_proc_ref_i32,
        array_literal_slot,
        array_redim_slot,
        array_erase_slot,
        array_get_slot,
        array_get_i32_1d_slot,
        array_set_slot,
        array_set_i32_1d_slot,
        bound_slot,
        for_each_init_slot,
        for_each_next_slot,
        call_extern_proc_i32,
        call_proc_ref_i32,
        lib_invoke_slot,
        declare_call_slot,
        arith_v_slot,
        concat_v_slot,
        neg_v_slot,
        compare_v_slot,
        compare_object_is_slot,
        type_of_is_slot,
        logical_v_slot,
        not_v_slot,
        truthy_v_slot,
        variant_changed_slot,
        coerce_numeric_v_slot,
        coerce_string_v_slot,
        coerce_fixed_string_v_slot,
        unbox_slot,
        add_i32_slot,
        sub_i32_slot,
        mul_i32_slot,
        div_i32_slot,
        rem_i32_slot,
        add_i16_slot,
        sub_i16_slot,
        mul_i16_slot,
        add_u8_slot,
        sub_u8_slot,
        mul_u8_slot,
        add_i64_slot,
        sub_i64_slot,
        mul_i64_slot,
        div_i64_slot,
        rem_i64_slot,
        add_currency_slot,
        sub_currency_slot,
        mul_currency_slot,
    })
}

pub(crate) unsafe extern "C" fn rt_jit_load_i32(run: *mut JitRun, area: u32, index: u32) -> i32 {
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
                .or_else(|| value.as_u8().map(i32::from))
                .or_else(|| value.as_bool().map(|value| if value { -1 } else { 0 }))
                .unwrap_or(0),
            None => 0,
        }
    }))
    .unwrap_or(0)
}

pub(crate) unsafe extern "C" fn rt_jit_load_i64(run: *mut JitRun, area: u32, index: u32) -> i64 {
    catch_unwind(AssertUnwindSafe(|| {
        if run.is_null() {
            return 0;
        }
        // SAFETY: null was rejected and the compiled call gives shared run access.
        let run = unsafe { &*run };
        match slot_ref(run, area, index) {
            Some(value) => value
                .as_i64()
                .or_else(|| value.as_currency_scaled_i64())
                .or_else(|| value.as_i32().map(i64::from))
                .or_else(|| value.as_i16().map(i64::from))
                .or_else(|| value.as_u8().map(i64::from))
                .unwrap_or(0),
            None => 0,
        }
    }))
    .unwrap_or(0)
}

pub(crate) unsafe extern "C" fn rt_jit_pack_f32_arg(value: f32) -> i64 {
    i64::from(value.to_bits())
}

pub(crate) unsafe extern "C" fn rt_jit_pack_f64_arg(value: f64) -> i64 {
    value.to_bits() as i64
}

pub(crate) unsafe extern "C" fn rt_jit_load_f64(run: *mut JitRun, area: u32, index: u32) -> f64 {
    catch_unwind(AssertUnwindSafe(|| {
        if run.is_null() {
            return 0.0;
        }
        // SAFETY: null was rejected and the compiled call gives shared run access.
        let run = unsafe { &*run };
        slot_ref(run, area, index)
            .and_then(|value| value.as_f64().or_else(|| value.as_date_f64()))
            .unwrap_or(0.0)
    }))
    .unwrap_or(0.0)
}

pub(crate) unsafe extern "C" fn rt_jit_load_f32(run: *mut JitRun, area: u32, index: u32) -> f32 {
    catch_unwind(AssertUnwindSafe(|| {
        if run.is_null() {
            return 0.0;
        }
        // SAFETY: null was rejected and the compiled call gives shared run access.
        let run = unsafe { &*run };
        slot_ref(run, area, index)
            .and_then(Variant::as_f32)
            .unwrap_or(0.0)
    }))
    .unwrap_or(0.0)
}

pub(crate) unsafe extern "C" fn rt_jit_store_i32(
    run: *mut JitRun,
    area: u32,
    index: u32,
    value: i32,
) -> i32 {
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

pub(crate) unsafe extern "C" fn rt_jit_store_i64(
    run: *mut JitRun,
    area: u32,
    index: u32,
    value: i64,
) -> i32 {
    status_guard(|| {
        if run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled call gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, area, index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_i64(value);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_store_currency_i64(
    run: *mut JitRun,
    area: u32,
    index: u32,
    value: i64,
) -> i32 {
    status_guard(|| {
        if run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled call gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, area, index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_currency_scaled_i64(value);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_store_f64(
    run: *mut JitRun,
    area: u32,
    index: u32,
    value: f64,
) -> i32 {
    status_guard(|| {
        if run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled call gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, area, index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_f64(value);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_store_date_f64(
    run: *mut JitRun,
    area: u32,
    index: u32,
    value: f64,
) -> i32 {
    status_guard(|| {
        if run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled call gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, area, index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_date_f64(value);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_store_f32(
    run: *mut JitRun,
    area: u32,
    index: u32,
    value: f32,
) -> i32 {
    status_guard(|| {
        if run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled call gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, area, index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_f32(value);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_store_u8(
    run: *mut JitRun,
    area: u32,
    index: u32,
    value: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() {
            return ST_FAULT;
        }
        let Ok(value) = u8::try_from(value) else {
            return ST_FAULT;
        };
        // SAFETY: null was rejected and the compiled call gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, area, index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_u8(value);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_store_i16(
    run: *mut JitRun,
    area: u32,
    index: u32,
    value: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() {
            return ST_FAULT;
        }
        let Ok(value) = i16::try_from(value) else {
            return ST_FAULT;
        };
        // SAFETY: null was rejected and the compiled call gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, area, index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_i16(value);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_load_bool(run: *mut JitRun, area: u32, index: u32) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if run.is_null() {
            return 0;
        }
        // SAFETY: null was rejected and the compiled call gives shared run access.
        let run = unsafe { &*run };
        match slot_ref(run, area, index) {
            Some(value) => {
                if value.as_bool().unwrap_or(false)
                    || value.as_i32().is_some_and(|v| v != 0)
                    || value.as_i16().is_some_and(|v| v != 0)
                {
                    1
                } else {
                    0
                }
            }
            None => 0,
        }
    }))
    .unwrap_or(0)
}

pub(crate) unsafe extern "C" fn rt_jit_store_bool(
    run: *mut JitRun,
    area: u32,
    index: u32,
    value: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled call gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, area, index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_bool(value != 0);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_store_proc_ref(
    run: *mut JitRun,
    area: u32,
    index: u32,
    proc: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || proc < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled call gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, area, index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_proc_ref(proc as usize);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_store_variant(
    state: *mut RawExecState,
    run: *mut JitRun,
    operand: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || operand.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes one descriptor to
        // a stack slot that stays live for this helper call.
        let operand = unsafe { *operand };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans a possible As-New initializer callback.
        let src = match unsafe { variant_operand_value_with_as_new(run, state, operand) } {
            Ok(src) => src,
            Err(status) => return status,
        };
        // SAFETY: the callback-capable lookup returned and no typed run borrow is live.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        unsafe { replace_jit_slot_with_cleanup(state, slot, src) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_set_line_number(run: *mut JitRun, line: i32) -> i32 {
    status_guard(|| {
        if run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(frame) = run.frames.last_mut() else {
            return ST_FAULT;
        };
        frame.current_line = line;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_current_line(run: *mut JitRun) -> i32 {
    if run.is_null() {
        return 0;
    }
    // SAFETY: null was rejected and compiled code gives unique run ownership.
    let run = unsafe { &*run };
    run.frames
        .last()
        .map(|frame| frame.current_line)
        .unwrap_or(0)
}

pub(crate) unsafe extern "C" fn rt_jit_err_field_set(
    run: *mut JitRun,
    state: *mut RawExecState,
    field: u32,
    kind: i32,
    value: i64,
    area: i32,
    index: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() {
            return ST_FAULT;
        }
        let operand = JitVariantOperandDesc {
            kind,
            _pad: 0,
            value,
            area,
            index,
        };
        // SAFETY: the compiled entry owns the live run and the operand descriptor
        // storage for this synchronous call.
        let value = match unsafe { variant_operand_value_with_as_new(run, state, operand) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: `state` is the live uniquely owned execution state; `value` lives
        // for the complete helper call.
        unsafe { rt_err_set_field(state, field, &value) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_stmt_boundary(
    run: *mut JitRun,
    state: *mut RawExecState,
    clear_temps_from: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || clear_temps_from < 0 {
            return ST_FAULT;
        }
        {
            // SAFETY: null was rejected and compiled code gives unique run ownership.
            let run = unsafe { &mut *run };
            clear_current_statement_temps(run, clear_temps_from as usize);
        }
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        unsafe { rt_maybe_drain(state) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_drain_terminations(
    run: *mut JitRun,
    state: *mut RawExecState,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() {
            return ST_FAULT;
        }
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        unsafe { rt_maybe_drain(state) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_add_ref(
    run: *mut JitRun,
    state: *mut RawExecState,
    operand: *const JitVariantOperandDesc,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operand.is_null() {
            return ST_FAULT;
        }
        // SAFETY: descriptor pointer is owned by compiled stack state for this call.
        let operand = unsafe { *operand };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans a possible As-New initializer callback.
        let value = match unsafe { variant_operand_value_with_as_new(run, state, operand) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        if !matches!(value.vtype(), VarType::Object) {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_runtime_error_number(state, 424) };
        }
        // SAFETY: callback-capable lookup returned and no typed run borrow is live.
        unsafe { &mut *run }.explicit_refs.push(value);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_release(
    run: *mut JitRun,
    state: *mut RawExecState,
    operand: *const JitVariantOperandDesc,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operand.is_null() {
            return ST_FAULT;
        }
        // SAFETY: descriptor pointer is owned by compiled stack state for this call.
        let operand = unsafe { *operand };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans a possible As-New initializer callback.
        let value = match unsafe { variant_operand_value_with_as_new(run, state, operand) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let identity = match unsafe { object_identity_for_is(state, &value) } {
            Ok(identity) => identity,
            Err(status) => return status,
        };
        // SAFETY: all callback-capable work returned and no typed run borrow is live.
        let run = unsafe { &mut *run };
        if let Some(index) = run
            .explicit_refs
            .iter()
            // SAFETY: the current compiled-run boundary owns the live unique state handle;
            // typed references and owned values remain live and nonaliasing for this call.
            .rposition(|held| unsafe { object_identity_for_is(state, held) }.ok() == Some(identity))
        {
            run.explicit_refs.remove(index);
        }
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_as_new_project_class_slot(
    run: *mut JitRun,
    area: u32,
    index: u32,
    class_index: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || class_index < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(alias) =
            current_frame_slot(run, area, index).and_then(|alias| resolve_slot_alias(run, alias))
        else {
            return ST_FAULT;
        };
        run.as_new_slots.insert(
            alias,
            OxAsNew::ProjectClass {
                class: ClassId(class_index as usize),
            },
        );
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_as_new_collection_slot(
    run: *mut JitRun,
    area: u32,
    index: u32,
    program_index: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(alias) =
            current_frame_slot(run, area, index).and_then(|alias| resolve_slot_alias(run, alias))
        else {
            return ST_FAULT;
        };
        run.as_new_slots.insert(
            alias,
            OxAsNew::ComClass {
                prog_id: format!("__jit_vba_collection:{program_index}"),
            },
        );
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_new_collection_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    dst_area: i32,
    dst_index: i32,
    program_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || dst_area < 0 || dst_index < 0 || program_index < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled caller gives unique run ownership.
        let run = unsafe { &mut *run };
        let value = match new_collection_variant_for_jit(run, program_index) {
            Ok(value) => value,
            Err(status) => return status,
        };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        unsafe { replace_jit_slot_with_cleanup(state, slot, value) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_new_object_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    program_index: i32,
    class_index: i32,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || program_index < 0
            || class_index < 0
            || dst_area < 0
            || dst_index < 0
        {
            return ST_FAULT;
        }
        let mut value = Variant::empty();
        // SAFETY: null state was rejected and is live for this JIT boundary;
        // `value` is initialized, uniquely borrowed Variant output storage.
        let status = unsafe {
            rt_project_new_object(
                state,
                program_index as usize,
                class_index as usize,
                &mut value,
            )
        };
        if status != ST_OK {
            return status;
        }
        // SAFETY: null was rejected and the compiled caller gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        unsafe { replace_jit_slot_with_cleanup(state, slot, value) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_predeclared_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    program_index: i32,
    class_index: i32,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || program_index < 0
            || class_index < 0
            || dst_area < 0
            || dst_index < 0
        {
            return ST_FAULT;
        }
        let mut value = Variant::empty();
        // SAFETY: null state was rejected and is live for this JIT boundary;
        // `value` is initialized, uniquely borrowed Variant output storage.
        let status = unsafe {
            rt_project_predeclared_instance(
                state,
                program_index as usize,
                class_index as usize,
                &mut value,
            )
        };
        if status != ST_OK {
            return status;
        }
        // SAFETY: null was rejected and the compiled caller gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        unsafe { replace_jit_slot_with_cleanup(state, slot, value) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_predeclared_set(
    state: *mut RawExecState,
    run: *mut JitRun,
    program_index: i32,
    class_index: i32,
    operand: *const JitVariantOperandDesc,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || operand.is_null()
            || program_index < 0
            || class_index < 0
        {
            return ST_FAULT;
        }
        // SAFETY: the compiled caller provides one live descriptor.
        let operand = unsafe { *operand };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans a possible As-New initializer callback.
        let value = match unsafe { variant_operand_value_with_as_new(run, state, operand) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: null state was rejected and remains live; `value` is an owned,
        // initialized Variant that cannot alias runtime destination storage.
        unsafe {
            rt_project_set_predeclared_instance(
                state,
                program_index as usize,
                class_index as usize,
                &value,
            )
        }
    })
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn variant_to_project_object_for_jit(
    state: *mut RawExecState,
    value: &Variant,
) -> Result<oxvba_runtime::object_ref::ObjectRef, i32> {
    if let Some(object) = value.as_object_ref() {
        return Ok(object);
    }
    if matches!(
        value.vtype(),
        VarType::Object | VarType::Empty | VarType::Null
    ) {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return Err(unsafe { rt_raise_runtime_error_number(state, 91) });
    }
    // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
    Err(unsafe { rt_raise_runtime_error_number(state, 424) })
}

/// Reconstitutes the execution state behind the opaque runtime ABI handle.
///
/// # Safety
/// `state` must be null or the exact result of `exec_state_as_raw` for a live,
/// uniquely borrowed, same-thread `ExecState` that remains valid for `'a`.
/// No access to that state may overlap the returned mutable borrow.
pub(crate) unsafe fn jit_exec_state_mut<'a>(
    state: *mut RawExecState,
) -> Option<&'a mut ExecState<'a>> {
    if state.is_null() {
        None
    } else {
        // SAFETY: this is the conversion whose complete validity, lifetime,
        // alignment, same-thread, and exclusivity contract is imposed on callers.
        Some(unsafe { &mut *(state as *mut ExecState<'a>) })
    }
}

pub(crate) fn jit_object_identity(value: &Variant) -> i32 {
    value
        .as_object_ref()
        .map(|object| object.raw())
        .unwrap_or(0)
}

pub(crate) fn jit_withevents_key(owner: &ObjectRef, binding: i64) -> i64 {
    (i64::from(owner.raw()) << 32) | (binding & 0xFFFF_FFFF)
}

pub(crate) fn jit_withevents_owner_raw(key: i64) -> i32 {
    (key >> 32) as i32
}

pub(crate) fn jit_withevents_binding(key: i64) -> i64 {
    key & 0xFFFF_FFFF
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn clear_jit_withevents_owners_before_releasing_values<'a>(
    state: *mut RawExecState,
    values: impl IntoIterator<Item = &'a Variant>,
) -> i32 {
    // SAFETY: this helper inherits the exact live, uniquely borrowed, same-thread
    // execution-state handle contract from its checked compiled-run caller.
    let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
        return ST_FAULT;
    };
    let mut candidates: HashMap<i32, (ObjectRef, u32)> = HashMap::new();
    for value in values {
        let Some(owner) = value.as_object_ref() else {
            continue;
        };
        if !owner.is_project_instance() {
            continue;
        }
        let owner_raw = owner.raw();
        candidates
            .entry(owner_raw)
            .and_modify(|(_, count)| *count += 1)
            .or_insert((owner, 1));
    }

    for (owner_raw, (owner, releasing_refs)) in candidates {
        let event_binding_refs = exec
            .events
            .withevents
            .keys()
            .filter(|key| jit_withevents_owner_raw(**key) == owner_raw)
            .count() as u32;
        if event_binding_refs == 0 {
            continue;
        }
        let retained_event_refs = event_binding_refs;
        if owner.strong_count() == retained_event_refs + releasing_refs + 1 {
            exec.events
                .withevents
                .retain(|key, _| jit_withevents_owner_raw(*key) != owner_raw);
        }
    }
    ST_OK
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn replace_jit_slot_with_cleanup(
    state: *mut RawExecState,
    slot: &mut Variant,
    value: Variant,
) -> i32 {
    let status =
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        unsafe { clear_jit_withevents_owners_before_releasing_values(state, std::iter::once(&*slot)) };
    if status != ST_OK {
        return status;
    }
    *slot = value;
    ST_OK
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn cleanup_jit_frame_withevents_owners(
    state: *mut RawExecState,
    frame: &JitFrame,
) -> i32 {
    // SAFETY: this helper inherits the live unique state handle; the frame's
    // Variant storage remains initialized and immutably borrowed for the call.
    unsafe {
        clear_jit_withevents_owners_before_releasing_values(
            state,
            frame.locals.iter().chain(frame.temps.iter()),
        )
    }
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn after_jit_frame_pop(
    run: &mut JitRun,
    state: *mut RawExecState,
    frame: &JitFrame,
) -> i32 {
    // SAFETY: the current compiled-run boundary owns the live unique state handle;
    // typed references and owned values remain live and nonaliasing for this call.
    let status = unsafe { cleanup_jit_frame_withevents_owners(state, frame) };
    if status != ST_OK {
        return status;
    }
    prune_for_each_from_depth(run, run.frames.len());
    prune_as_new_slots_from_depth(run, run.frames.len());
    prune_param_array_aliases_from_depth(run, run.frames.len());
    ST_OK
}

pub(crate) unsafe extern "C" fn rt_jit_withevents_get_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    owner: *const JitVariantOperandDesc,
    binding: i32,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || owner.is_null() || dst_area < 0 || dst_index < 0 {
            return ST_FAULT;
        }
        // SAFETY: the compiled caller provides one live descriptor.
        let owner_operand = unsafe { *owner };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans a possible As-New initializer callback.
        let owner_value =
            match unsafe { variant_operand_value_with_as_new(run, state, owner_operand) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let owner_ref = match unsafe { variant_to_project_object_for_jit(state, &owner_value) } {
            Ok(owner) => owner,
            Err(status) => return status,
        };
        let key = jit_withevents_key(&owner_ref, binding as i64);
        // SAFETY: this helper inherits the exact live, uniquely borrowed, same-thread
        // execution-state handle contract from its checked compiled-run caller.
        let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
            return ST_FAULT;
        };
        let value = exec
            .events
            .withevents
            .get(&key)
            .map(|binding| binding.source.clone())
            .unwrap_or_else(|| Variant::from_i32(0));
        let _ = exec;
        // SAFETY: callback-capable lookup returned and no typed run borrow is live.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        *slot = value;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_withevents_set_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    operands: *const JitVariantOperandDesc,
    binding: i32,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || operands.is_null() || dst_area < 0 || dst_index < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes two live descriptors.
        let operands = unsafe { std::slice::from_raw_parts(operands, 2) };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans possible As-New initializer callbacks.
        let owner_value =
            match unsafe { variant_operand_value_with_as_new(run, state, operands[0]) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let value = match unsafe { variant_operand_value_with_as_new(run, state, operands[1]) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let owner_ref = match unsafe { variant_to_project_object_for_jit(state, &owner_value) } {
            Ok(owner) => owner,
            Err(status) => return status,
        };
        let key = jit_withevents_key(&owner_ref, binding as i64);
        // SAFETY: this helper inherits the exact live, uniquely borrowed, same-thread
        // execution-state handle contract from its checked compiled-run caller.
        let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
            return ST_FAULT;
        };
        if jit_is_nothing(&value) {
            exec.events.withevents.remove(&key);
        } else {
            let order = exec.events.next_withevents_order;
            exec.events.next_withevents_order = exec.events.next_withevents_order.wrapping_add(1);
            exec.events.withevents.insert(
                key,
                EventBinding {
                    owner: owner_value,
                    source: value.clone(),
                    order,
                },
            );
        }
        let _ = exec;
        // SAFETY: callback-capable lookups returned and no typed run borrow is live.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        *slot = value;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_withevents_clear_owner_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    owner: *const JitVariantOperandDesc,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || owner.is_null() || dst_area < 0 || dst_index < 0 {
            return ST_FAULT;
        }
        // SAFETY: the compiled caller provides one live descriptor.
        let owner_operand = unsafe { *owner };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans a possible As-New initializer callback.
        let owner_value =
            match unsafe { variant_operand_value_with_as_new(run, state, owner_operand) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let owner_ref = match unsafe { variant_to_project_object_for_jit(state, &owner_value) } {
            Ok(owner) => owner,
            Err(status) => return status,
        };
        let owner_raw = owner_ref.raw();
        // SAFETY: this helper inherits the exact live, uniquely borrowed, same-thread
        // execution-state handle contract from its checked compiled-run caller.
        let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
            return ST_FAULT;
        };
        exec.events
            .withevents
            .retain(|key, _| jit_withevents_owner_raw(*key) != owner_raw);
        let _ = exec;
        // SAFETY: callback-capable lookup returned and no typed run borrow is live.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        *slot = Variant::from_i32(0);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_withevents_first_owner_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    source: *const JitVariantOperandDesc,
    binding: i32,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || source.is_null() || dst_area < 0 || dst_index < 0 {
            return ST_FAULT;
        }
        // SAFETY: the compiled caller provides one live descriptor.
        let source_operand = unsafe { *source };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans a possible As-New initializer callback.
        let source_value =
            match unsafe { variant_operand_value_with_as_new(run, state, source_operand) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        let mut owners: Vec<(u64, ObjectRef)> = Vec::new();
        if !jit_is_nothing(&source_value) {
            // SAFETY: this helper inherits the exact live, uniquely borrowed, same-thread
            // execution-state handle contract from its checked compiled-run caller.
            let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
                return ST_FAULT;
            };
            let source_id = jit_object_identity(&source_value);
            for (key, binding_data) in &exec.events.withevents {
                if jit_withevents_binding(*key) == (binding as i64 & 0xFFFF_FFFF)
                    && jit_object_identity(&binding_data.source) == source_id
                    && let Some(owner) = binding_data.owner.as_object_ref()
                {
                    owners.push((binding_data.order, owner));
                }
            }
        }
        owners.sort_unstable_by_key(|(order, _)| *order);
        let owners: Vec<ObjectRef> = owners.into_iter().map(|(_, owner)| owner).collect();
        let value = match owners.first().cloned() {
            Some(first) => {
                // SAFETY: this helper inherits the exact live, uniquely borrowed, same-thread
                // execution-state handle contract from its checked compiled-run caller.
                let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
                    return ST_FAULT;
                };
                exec.events.withevents_iters.push((owners, 1));
                Variant::from_object_ref(first)
            }
            None => Variant::from_i32(0),
        };
        // SAFETY: callback-capable lookup returned and all state borrows are out of scope.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        *slot = value;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_withevents_next_owner_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || dst_area < 0 || dst_index < 0 {
            return ST_FAULT;
        }
        // SAFETY: this helper inherits the exact live, uniquely borrowed, same-thread
        // execution-state handle contract from its checked compiled-run caller.
        let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
            return ST_FAULT;
        };
        let next = exec
            .events
            .withevents_iters
            .last_mut()
            .and_then(|(owners, pos)| {
                let value = owners.get(*pos).cloned();
                if value.is_some() {
                    *pos += 1;
                }
                value
            });
        let value = match next {
            Some(owner) => Variant::from_object_ref(owner),
            None => {
                exec.events.withevents_iters.pop();
                Variant::from_i32(0)
            }
        };
        // SAFETY: null was rejected and this helper mutates only the destination slot.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        *slot = value;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_raise_event(
    state: *mut RawExecState,
    run: *mut JitRun,
    source: *const JitVariantOperandDesc,
    event: i32,
    args: *const JitCallArgDesc,
    argc: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || source.is_null()
            || argc < 0
            || (argc > 0 && args.is_null())
        {
            return ST_FAULT;
        }
        let argc = argc as usize;
        let args = if argc == 0 {
            &[]
        } else {
            // SAFETY: null was rejected and the compiled caller writes `argc` descriptors.
            unsafe { std::slice::from_raw_parts(args, argc) }
        };
        // SAFETY: the compiled caller provides one live descriptor.
        let source_operand = unsafe { *source };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans a possible As-New initializer callback.
        let source_value =
            match unsafe { variant_operand_value_with_as_new(run, state, source_operand) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let source_object = match unsafe { variant_to_project_object_for_jit(state, &source_value) }
        {
            Ok(object) => object,
            Err(status) => return status,
        };
        let source_id = source_object.raw();
        let current_program = {
            // SAFETY: callback-capable source lookup returned; this shared borrow is
            // bounded before any handler entry and supplies only a copied index.
            unsafe { &*run }
                .frames
                .last()
                .map(|frame| frame.program_index)
                .unwrap_or(0)
        };
        let targets: Vec<(u64, Variant, usize, usize)> = {
            // SAFETY: this helper inherits the exact live, uniquely borrowed, same-thread
            // execution-state handle contract from its checked compiled-run caller.
            let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
                return ST_FAULT;
            };
            let mut targets = Vec::new();
            for (key, binding) in &exec.events.withevents {
                if jit_object_identity(&binding.source) != source_id {
                    continue;
                }
                let token = jit_withevents_binding(*key) as i32;
                let owner_bundle = binding
                    .owner
                    .as_object_ref()
                    .map(|owner| owner.bundle_id() as usize)
                    .unwrap_or(current_program);
                if let Some(&handler) = exec
                    .programs
                    .get(owner_bundle)
                    .and_then(|program| program.event_routes.get(&(token, event)))
                {
                    targets.push((binding.order, binding.owner.clone(), handler, owner_bundle));
                }
            }
            targets.sort_by_key(|(order, ..)| *order);
            targets
        };
        let names = vec![
            JitCallArgNameDesc {
                ptr: 0,
                len: -1,
                _pad: 0,
            };
            args.len()
        ];
        for (_, sink, handler, owner_bundle) in targets {
            if let Err(status) =
                // SAFETY: the current compiled-run boundary owns the live unique state handle;
                // typed references and owned values remain live and nonaliasing for this call.
                unsafe {
                    invoke_project_member_with_me(
                        run,
                        state,
                        owner_bundle,
                        handler,
                        sink,
                        args,
                        &names,
                    )
                }
            {
                return status;
            }
        }
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_project_field_get_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    object: *const JitVariantOperandDesc,
    field: i32,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || object.is_null() || dst_area < 0 || dst_index < 0 {
            return ST_FAULT;
        }
        // SAFETY: the compiled caller provides one live descriptor.
        let object_operand = unsafe { *object };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans possible As-New initializer callbacks.
        let object_value =
            match unsafe { variant_operand_value_with_as_new(run, state, object_operand) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let object = match unsafe { variant_to_project_object_for_jit(state, &object_value) } {
            Ok(object) => object,
            Err(status) => return status,
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let value =
            match unsafe { project_field_get_with_as_new_for_jit(run, state, &object, field) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        // SAFETY: callback-capable field lookup returned and no typed run borrow is live.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        *slot = value;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_project_field_set(
    state: *mut RawExecState,
    run: *mut JitRun,
    operands: *const JitVariantOperandDesc,
    field: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || operands.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes two live descriptors.
        let operands = unsafe { std::slice::from_raw_parts(operands, 2) };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans possible As-New initializer callbacks.
        let object_value =
            match unsafe { variant_operand_value_with_as_new(run, state, operands[0]) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let value = match unsafe { variant_operand_value_with_as_new(run, state, operands[1]) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let object = match unsafe { variant_to_project_object_for_jit(state, &object_value) } {
            Ok(object) => object,
            Err(status) => return status,
        };
        if object.project_field_set(field, value) {
            ST_OK
        } else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            unsafe { rt_raise_runtime_error_number(state, 438) }
        }
    })
}

pub(crate) fn project_member_kind_from_raw(raw: i32) -> Option<ProjectMemberKind> {
    match raw {
        2 => Some(ProjectMemberKind::PropertyGet),
        1 => Some(ProjectMemberKind::Method),
        4 => Some(ProjectMemberKind::PropertyLet),
        8 => Some(ProjectMemberKind::PropertySet),
        _ => None,
    }
}

pub(crate) struct ProjectMemberInvocation<'a> {
    pub(crate) name: &'a str,
    pub(crate) kind: ProjectMemberKind,
    pub(crate) args: &'a [JitCallArgDesc],
    pub(crate) names: &'a [JitCallArgNameDesc],
    pub(crate) dst: Option<(u32, u32)>,
}

pub(crate) fn project_default_member_for_jit(
    class: &OxClass,
    kind: ProjectMemberKind,
    args_empty: bool,
) -> Option<&OxClassMethod> {
    let exact = class
        .methods
        .iter()
        .find(|method| method.is_default_member && method.kind == kind);
    exact.or_else(|| {
        if kind == ProjectMemberKind::PropertyGet && args_empty {
            class
                .methods
                .iter()
                .find(|method| method.is_default_member && method.kind == ProjectMemberKind::Method)
        } else if kind == ProjectMemberKind::Method {
            class.methods.iter().find(|method| {
                method.is_default_member && method.kind == ProjectMemberKind::PropertyGet
            })
        } else {
            None
        }
    })
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn invoke_project_member_with_me(
    run: *mut JitRun,
    state: *mut RawExecState,
    program_index: usize,
    proc: usize,
    me: Variant,
    args: &[JitCallArgDesc],
    names: &[JitCallArgNameDesc],
) -> Result<Variant, i32> {
    if run.is_null() {
        return Err(ST_FAULT);
    }
    let (image, return_local, frame, pending_param_array_aliases) = {
        // SAFETY: null was rejected. This preparation borrow ends before compiled entry.
        let run_ref = unsafe { &mut *run };
        let Some(image) = program_image(run_ref, program_index) else {
            return Err(ST_FAULT);
        };
        if image.program.is_null() || image.functions.is_null() || proc >= image.function_count {
            return Err(ST_FAULT);
        }
        // SAFETY: installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let Some(func) = program.funcs.get(proc) else {
            return Err(ST_FAULT);
        };
        if hidden_me_receiver_param_count(func) != 1 {
            return Err(ST_FAULT);
        }
        let return_local = if let Some(ret) = func.return_local {
            let Some(ret_ty) = func.locals.get(ret.0).map(|local| local.ty.clone()) else {
                return Err(ST_FAULT);
            };
            if !is_jit_static_call_ty(&ret_ty) {
                return Err(ST_FAULT);
            }
            Some((ret.0, ret_ty))
        } else {
            None
        };
        // SAFETY: descriptors and function metadata remain live for preparation.
        let ordered_args = unsafe { order_project_member_call_args(state, func, args, names) }?;
        if run_ref.frames.len() >= MAX_JIT_FRAMES {
            // SAFETY: the enclosing JIT boundary validated the live execution state.
            return Err(unsafe { rt_raise_out_of_stack(state) });
        }
        let Some(caller_frame) = run_ref.frames.len().checked_sub(1) else {
            return Err(ST_FAULT);
        };
        let mut frame = new_jit_frame(program, program_index, func).map_err(|_| ST_FAULT)?;
        frame.locals[0] = me;
        let mut pending_param_array_aliases = Vec::new();
        // SAFETY: preparation owns the bounded run borrow and disjoint frame storage.
        let seed_status = unsafe {
            seed_jit_member_frame_args(
                state,
                run_ref,
                func,
                &mut frame,
                caller_frame,
                &ordered_args,
                &mut pending_param_array_aliases,
            )
        };
        if seed_status != ST_OK {
            return Err(seed_status);
        }
        (image, return_local, frame, pending_param_array_aliases)
    };
    let mut saved_err = RtSavedErrState::default();
    // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
    let enter_status = unsafe { rt_err_enter_activation(state, &mut saved_err) };
    if enter_status != ST_OK {
        return Err(enter_status);
    }
    {
        // SAFETY: error activation completed and this push borrow ends before entry.
        let run_ref = unsafe { &mut *run };
        run_ref.frames.push(frame);
        let callee_frame = run_ref.frames.len() - 1;
        for (index, aliases) in pending_param_array_aliases {
            run_ref.param_array_aliases.insert(
                SlotAlias {
                    frame: Some(callee_frame),
                    area: AREA_LOCAL,
                    index: index as u32,
                },
                aliases,
            );
        }
    }
    // SAFETY: function pointer bounds were checked above.
    let entry = unsafe { *image.functions.add(proc) };
    // SAFETY: entry uses the JIT ABI and the stable raw run root/state remain live.
    let status = unsafe { entry(run, state) };
    let (return_value, cleanup_status) = {
        // SAFETY: entry returned; no typed run borrow spans it.
        let run_ref = unsafe { &mut *run };
        let return_value = if status == ST_OK {
            return_local.as_ref().and_then(|(local, ty)| {
                run_ref
                    .frames
                    .last()
                    .and_then(|frame| frame.locals.get(*local))
                    .and_then(|value| call_return_variant(ty, value))
            })
        } else {
            None
        };
        let Some(frame) = run_ref.frames.pop() else {
            // Restore the error activation below even if the entry corrupted the stack.
            // SAFETY: the activation was entered successfully and state remains live.
            let restore_status = unsafe { rt_err_restore_activation(state, &saved_err) };
            return Err(if restore_status == ST_OK {
                ST_FAULT
            } else {
                restore_status
            });
        };
        // SAFETY: post-entry cleanup owns the bounded run borrow.
        let cleanup_status = unsafe { after_jit_frame_pop(run_ref, state, &frame) };
        (return_value, cleanup_status)
    };
    // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
    let restore_status = unsafe { rt_err_restore_activation(state, &saved_err) };
    if restore_status != ST_OK {
        return Err(restore_status);
    }
    if cleanup_status != ST_OK {
        return Err(cleanup_status);
    }
    if status != ST_OK {
        return Err(status);
    }
    Ok(return_value.unwrap_or_else(Variant::empty))
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn invoke_project_default_member_values(
    run: *mut JitRun,
    state: *mut RawExecState,
    recv_value: Variant,
    kind: ProjectMemberKind,
    values: &[Variant],
) -> Result<Variant, i32> {
    if run.is_null() {
        return Err(ST_FAULT);
    }
    // SAFETY: the current compiled-run boundary owns the live unique state handle;
    // typed references and owned values remain live and nonaliasing for this call.
    let object = unsafe { variant_to_project_object_for_jit(state, &recv_value) }?;
    if !object.is_project_instance() {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return Err(unsafe { rt_raise_runtime_error_number(state, 438) });
    }
    let program_index = object.bundle_id() as usize;
    let Some(image) = ({
        // SAFETY: null was rejected and this shared metadata borrow is bounded.
        program_image(unsafe { &*run }, program_index)
    }) else {
        return Err(ST_FAULT);
    };
    let class_idx = object.route_key() as usize;
    // SAFETY: installed from the owning CompiledImage for this run.
    let program = unsafe { &*image.program };
    let class = program
        .classes
        .get(class_idx)
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        .ok_or_else(|| unsafe { rt_raise_runtime_error_number(state, 438) })?;
    let member = project_default_member_for_jit(class, kind, values.is_empty())
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        .ok_or_else(|| unsafe { rt_raise_runtime_error_number(state, 438) })?;
    let args: Vec<JitCallArgDesc> = values
        .iter()
        .enumerate()
        .map(|(index, _)| JitCallArgDesc {
            kind: JIT_CALL_ARG_BYVAL_VARIANT,
            aux: JIT_VARIANT_OPERAND_PLACE,
            value: 0,
            area: AREA_TEMP as i32,
            index: index as i32,
        })
        .collect();
    let names = vec![
        JitCallArgNameDesc {
            ptr: 0,
            len: -1,
            _pad: 0,
        };
        args.len()
    ];
    let frame = JitFrame {
        program_index,
        locals: Vec::new(),
        temps: values.to_vec(),
        aliases: Vec::new(),
        gosub_stack: Vec::new(),
        saved_err: RtSavedErrState::default(),
        current_line: 0,
    };
    // SAFETY: preparation is complete and this push borrow ends before member entry.
    unsafe { &mut *run }.frames.push(frame);
    // SAFETY: this helper inherits the live unique state handle; `run`, argument
    // descriptors, names, and the owned receiver remain live and nonaliasing.
    let result = unsafe {
        invoke_project_member_with_me(
            run,
            state,
            program_index,
            member.proc.0,
            recv_value,
            &args,
            &names,
        )
    };
    // SAFETY: member entry returned and no typed run borrow spans it.
    let run_ref = unsafe { &mut *run };
    let Some(frame) = run_ref.frames.pop() else {
        return Err(ST_FAULT);
    };
    // SAFETY: the current compiled-run boundary owns the live unique state handle;
    // typed references and owned values remain live and nonaliasing for this call.
    let cleanup_status = unsafe { after_jit_frame_pop(run_ref, state, &frame) };
    if cleanup_status != ST_OK {
        return Err(cleanup_status);
    }
    result
}

pub(crate) unsafe extern "C" fn rt_jit_project_member_get_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    recv: *const JitVariantOperandDesc,
    name_ptr: *const u8,
    name_len: i32,
    invoke_kind: i32,
    argc: i32,
    args: *const JitCallArgDesc,
    names: *const JitCallArgNameDesc,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || recv.is_null()
            || name_ptr.is_null()
            || name_len < 0
            || argc < 0
            || dst_area < -1
            || dst_index < -1
        {
            return ST_FAULT;
        }
        let dst = match (dst_area, dst_index) {
            (-1, -1) => None,
            (area, index) if area >= 0 && index >= 0 => Some((area as u32, index as u32)),
            _ => return ST_FAULT,
        };
        let Some(kind) = project_member_kind_from_raw(invoke_kind) else {
            return ST_FAULT;
        };
        let argc = argc as usize;
        let args = if argc == 0 {
            &[]
        } else if args.is_null() || names.is_null() {
            return ST_FAULT;
        } else {
            // SAFETY: compiled code provides exactly `argc` call and name descriptors.
            unsafe { std::slice::from_raw_parts(args, argc) }
        };
        let names = if argc == 0 {
            &[]
        } else {
            // SAFETY: null was rejected above and the descriptor count matches `args`.
            unsafe { std::slice::from_raw_parts(names, argc) }
        };
        // SAFETY: pointer/length were provided by compiled constants for a live member name.
        let name = match std::str::from_utf8(unsafe {
            std::slice::from_raw_parts(name_ptr, name_len as usize)
        }) {
            Ok(name) => name,
            Err(_) => return ST_FAULT,
        };
        // SAFETY: the compiled caller provides one live descriptor.
        let recv_operand = unsafe { *recv };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans a possible As-New initializer callback.
        let recv_value =
            match unsafe { variant_operand_value_with_as_new(run, state, recv_operand) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        unsafe {
            invoke_project_member_to_slot_for_jit(
                state,
                run,
                recv_value,
                ProjectMemberInvocation {
                    name,
                    kind,
                    args,
                    names,
                    dst,
                },
            )
        }
    })
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; `run` must be the matching
// stable run root, and the invocation's text and descriptor slices must remain
// initialized, live, and nonaliasing for the complete synchronous call.
pub(crate) unsafe fn invoke_project_member_to_slot_for_jit(
    state: *mut RawExecState,
    run: *mut JitRun,
    recv_value: Variant,
    invocation: ProjectMemberInvocation<'_>,
) -> i32 {
    let ProjectMemberInvocation {
        name,
        kind,
        args,
        names,
        dst,
    } = invocation;
    if run.is_null() {
        return ST_FAULT;
    }
    // SAFETY: the current compiled-run boundary owns the live unique state handle;
    // typed references and owned values remain live and nonaliasing for this call.
    let object = match unsafe { variant_to_project_object_for_jit(state, &recv_value) } {
        Ok(object) => object,
        Err(status) => return status,
    };
    if !object.is_project_instance() {
        // SAFETY: this helper inherits the live unique state handle; all other
        // inputs are owned values or live typed references for this call.
        return unsafe {
            invoke_foreign_member_to_slot_for_jit(
                state,
                run,
                object,
                ProjectMemberInvocation {
                    name,
                    kind,
                    args,
                    names,
                    dst,
                },
            )
        };
    }
    if object.route_key() == VBA_COLLECTION_ROUTE_KEY {
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let value = match unsafe {
            dispatch_collection_member_for_jit(state, &*run, &object, name, args)
        } {
            Ok(value) => value,
            Err(status) => return status,
        };
        if let Some((area, index)) = dst {
            // SAFETY: collection dispatch returned and no typed run borrow is live.
            let run = unsafe { &mut *run };
            let Some(slot) = slot_mut(run, area, index) else {
                return ST_FAULT;
            };
            *slot = value;
        }
        return ST_OK;
    }
    let program_index = object.bundle_id() as usize;
    let class_idx = object.route_key() as usize;
    let Some(image) = ({
        // SAFETY: this shared metadata borrow is bounded before member entry.
        program_image(unsafe { &*run }, program_index)
    }) else {
        return ST_FAULT;
    };
    if image.program.is_null() {
        return ST_FAULT;
    }
    // SAFETY: installed from the owning CompiledImage for this run.
    let program = unsafe { &*image.program };
    let Some(class) = program.classes.get(class_idx) else {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return unsafe { rt_raise_runtime_error_number(state, 438) };
    };
    let member = if name.is_empty() {
        project_default_member_for_jit(class, kind, args.is_empty())
    } else {
        let exact = class
            .methods
            .iter()
            .find(|method| method.name.eq_ignore_ascii_case(name) && method.kind == kind);
        exact.or_else(|| {
            if kind == ProjectMemberKind::PropertyGet && args.is_empty() {
                class.methods.iter().find(|method| {
                    method.name.eq_ignore_ascii_case(name)
                        && method.kind == ProjectMemberKind::Method
                })
            } else if kind == ProjectMemberKind::Method {
                class.methods.iter().find(|method| {
                    method.name.eq_ignore_ascii_case(name)
                        && method.kind == ProjectMemberKind::PropertyGet
                })
            } else {
                None
            }
        })
    };
    let Some(member) = member else {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return unsafe { rt_raise_runtime_error_number(state, 438) };
    };
    // SAFETY: this helper inherits the live unique state handle; `run`, argument
    // descriptors, names, and the owned receiver remain live and nonaliasing.
    let value = match unsafe {
        invoke_project_member_with_me(
            run,
            state,
            program_index,
            member.proc.0,
            recv_value,
            args,
            names,
        )
    } {
        Ok(value) => value,
        Err(status) => return status,
    };
    if let Some((area, index)) = dst {
        // SAFETY: member entry returned and no typed run borrow is live.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, area, index) else {
            return ST_FAULT;
        };
        *slot = value;
    }
    ST_OK
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn dispatch_collection_member_for_jit(
    state: *mut RawExecState,
    run: &JitRun,
    object: &ObjectRef,
    name: &str,
    args: &[JitCallArgDesc],
) -> Result<Variant, i32> {
    let native = vba_collection_native_method_for_jit(name)
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        .ok_or_else(|| unsafe { rt_raise_runtime_error_number(state, 438) })?;
    let method = match native {
        oxvba_bundle::NativeMethodId::CollectionAdd => CollectionMethod::Add,
        oxvba_bundle::NativeMethodId::CollectionItem => CollectionMethod::Item,
        oxvba_bundle::NativeMethodId::CollectionCount => CollectionMethod::Count,
        oxvba_bundle::NativeMethodId::CollectionRemove => CollectionMethod::Remove,
        oxvba_bundle::NativeMethodId::CollectionNewEnum => {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return Err(unsafe { rt_raise_runtime_error_number(state, 438) });
        }
    };
    let argv = collection_call_arg_values_for_jit(run, args)?;
    object
        .with_native_collection(|data| dispatch_collection(method, data, &argv))
        .ok_or(ST_FAULT)?
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        .map_err(|err| unsafe { raise_collection_error_for_jit(state, err) })
}

pub(crate) fn collection_call_arg_values_for_jit(
    run: &JitRun,
    args: &[JitCallArgDesc],
) -> Result<Vec<Variant>, i32> {
    args.iter()
        .copied()
        .map(|arg| call_arg_variant_value(run, arg).ok_or(ST_FAULT))
        .collect()
}

pub(crate) fn vba_collection_native_method_for_jit(
    member: &str,
) -> Option<oxvba_bundle::NativeMethodId> {
    let member = member.trim().trim_start_matches('[').trim_end_matches(']');
    let lib = vba_library_bundle();
    let class = lib.classes.first()?;
    let method = class
        .methods
        .iter()
        .find(|method| method.name.eq_ignore_ascii_case(member))?;
    match lib.procedures.get(method.proc).and_then(|proc| proc.native) {
        Some(NativeBody::Method(id)) => Some(id),
        _ => None,
    }
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn raise_collection_error_for_jit(
    state: *mut RawExecState,
    err: CollectionError,
) -> i32 {
    match err {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        CollectionError::NotFound => unsafe { rt_raise_subscript_out_of_range(state) },
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        CollectionError::DuplicateKey => unsafe { rt_raise_runtime_error_number(state, 457) },
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        CollectionError::BadArgument => unsafe { rt_raise_runtime_error_number(state, 5) },
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        CollectionError::ArgNotOptional => unsafe { rt_raise_runtime_error_number(state, 449) },
    }
}

pub(crate) fn project_member_kind_to_dynamic(kind: ProjectMemberKind) -> DynamicCallKind {
    match kind {
        ProjectMemberKind::Method => DynamicCallKind::Method,
        ProjectMemberKind::PropertyGet => DynamicCallKind::PropertyGet,
        ProjectMemberKind::PropertyLet => DynamicCallKind::PropertyLet,
        ProjectMemberKind::PropertySet => DynamicCallKind::PropertySet,
    }
}

fn interop_invoke_kind_from_project(kind: ProjectMemberKind) -> InteropInvokeKind {
    match kind {
        ProjectMemberKind::Method => InteropInvokeKind::Method,
        ProjectMemberKind::PropertyGet => InteropInvokeKind::PropertyGet,
        ProjectMemberKind::PropertyLet => InteropInvokeKind::PropertyPut,
        ProjectMemberKind::PropertySet => InteropInvokeKind::PropertyPutRef,
    }
}

fn verified_late_dispatch_plan_for_jit(
    name: &str,
    kind: ProjectMemberKind,
    args: &[JitCallArgDesc],
    names: &[JitCallArgNameDesc],
) -> Result<VerifiedInteropPlan, String> {
    let named_arg_count = names
        .iter()
        .filter(|name| name.ptr != 0 && name.len > 0)
        .count();
    let byref_slots = args
        .iter()
        .enumerate()
        .filter_map(|(index, arg)| (arg.kind == JIT_CALL_ARG_BYREF_ALIAS).then_some(index as u32))
        .collect();
    VerifiedInteropPlan::late_dispatch(
        name,
        name.is_empty(),
        interop_invoke_kind_from_project(kind),
        named_arg_count,
        byref_slots,
    )
    .map_err(|err| err.message)
}

fn verified_declare_plan_for_jit(
    descriptor: &oxvba_bundle::ExternalCallDescriptor,
) -> Result<VerifiedInteropPlan, String> {
    let entry = if descriptor.alias.is_empty() {
        descriptor.declared_name.clone()
    } else {
        descriptor.alias.clone()
    };
    VerifiedInteropPlan::declare_x64(
        descriptor.descriptor_id,
        descriptor.library.clone(),
        entry,
        descriptor.calling_convention.clone(),
        descriptor.param_by_ref.clone(),
        descriptor.return_type.as_ref().map(|ty| format!("{ty:?}")),
    )
    .map_err(|err| err.message)
}

fn current_unit_source(run: *mut JitRun) -> String {
    if run.is_null() {
        return "OxVba".to_string();
    }
    // SAFETY: caller provides the live compiled run for this helper.
    let run_ref = unsafe { &*run };
    current_program_image(run_ref)
        .and_then(|(_, image)| {
            if image.program.is_null() {
                None
            } else {
                // SAFETY: installed from the owning CompiledImage for this run.
                Some(unsafe { &*image.program }.unit_name.clone())
            }
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "OxVba".to_string())
}

fn raise_plan_error(state: *mut RawExecState, run: *mut JitRun, message: String) -> i32 {
    // SAFETY: the compiled caller supplies the live unique ExecState handle.
    let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
        return ST_FAULT;
    };
    exec.err_engine
        .raise(Fault::new(5, message), current_unit_source(run));
    ST_FAULT
}

pub(crate) fn foreign_member_selector_for_jit(name: &str) -> DynamicMemberSelector {
    if name.is_empty() {
        DynamicMemberSelector::DefaultMember
    } else {
        DynamicMemberSelector::Name(name.to_string())
    }
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; `run` must be the matching
// stable run root, and the invocation's text and descriptor slices must remain
// initialized, live, and nonaliasing for the complete synchronous call.
pub(crate) unsafe fn invoke_foreign_member_to_slot_for_jit(
    state: *mut RawExecState,
    run: *mut JitRun,
    object: ObjectRef,
    invocation: ProjectMemberInvocation<'_>,
) -> i32 {
    let ProjectMemberInvocation {
        name,
        kind,
        args,
        names,
        dst,
    } = invocation;
    if run.is_null() || args.len() != names.len() {
        return ST_FAULT;
    }
    let mut call_args = Vec::with_capacity(args.len());
    {
        // SAFETY: this read-only preparation borrow ends before the host call.
        let run_ref = unsafe { &*run };
        for (index, (arg, name)) in args.iter().copied().zip(names.iter().copied()).enumerate() {
            let value = if arg.kind == JIT_CALL_ARG_OMITTED {
                None
            } else {
                match call_arg_variant_value(run_ref, arg) {
                    Some(value) => Some(DynamicValue::from_variant(value)),
                    None => return ST_FAULT,
                }
            };
            // SAFETY: names are compiled descriptors backed by the live OxProgram.
            let name = match unsafe { call_arg_name_preserved(name) } {
                Ok(name) => name,
                Err(status) => return status,
            };
            call_args.push(DynamicCallArg {
                value,
                name,
                by_ref: (arg.kind == JIT_CALL_ARG_BYREF_ALIAS)
                    .then(|| RuntimeByRefSlot::new(index as u32, None)),
            });
        }
    }
    if let Err(message) = verified_late_dispatch_plan_for_jit(name, kind, args, names) {
        return raise_plan_error(state, run, message);
    }
    let request = DynamicCallRequest {
        object,
        member: foreign_member_selector_for_jit(name),
        args: call_args,
        call_kind_hint: Some(project_member_kind_to_dynamic(kind)),
    };
    let host = {
        // SAFETY: JIT helpers receive pointers produced by `exec_state_as_raw` for a
        // live `ExecState`. Copying the host reference ends this typed state borrow
        // before a host call that may synchronously re-enter VBA.
        let exec = match unsafe { jit_exec_state_mut(state) } {
            Some(exec) => exec,
            None => return ST_FAULT,
        };
        exec.host
    };
    let (value, writebacks) = match host
        .com()
        .dispatch_invoke_dynamic_variant_with_writebacks(&request)
    {
        Ok(result) => result,
        Err(err) => {
            // SAFETY: the host call returned, so no callback-owned state borrow
            // remains and the live execution state may be borrowed again.
            let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
                return ST_FAULT;
            };
            exec.err_engine
                .raise(Fault::from_hal(err), current_unit_source(run));
            return ST_FAULT;
        }
    };
    // SAFETY: the host call returned and no typed run borrow spans it.
    let run = unsafe { &mut *run };
    for (index, arg) in args.iter().copied().enumerate() {
        if arg.kind != JIT_CALL_ARG_BYREF_ALIAS {
            continue;
        }
        let Some(Some(value)) = writebacks.get(index) else {
            continue;
        };
        if arg.area < 0 || arg.index < 0 {
            return ST_FAULT;
        }
        let Some(slot) = slot_mut(run, arg.area as u32, arg.index as u32) else {
            return ST_FAULT;
        };
        *slot = value.clone();
    }
    if let Some((area, index)) = dst {
        let Some(slot) = slot_mut(run, area, index) else {
            return ST_FAULT;
        };
        *slot = value;
    }
    ST_OK
}

/// Execute a verified x64 Declare plan through the host dynlink HAL.
///
/// # Safety
/// `state` is the live unique ExecState handle; `run` is the matching compiled
/// run; `args` is `argc` live call descriptors for this call.
pub(crate) unsafe extern "C" fn rt_jit_declare_call_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    descriptor_id: u32,
    argc: i32,
    args: *const JitCallArgDesc,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || argc < 0 || dst_area < -1 || dst_index < -1 {
            return ST_FAULT;
        }
        let dst = match (dst_area, dst_index) {
            (-1, -1) => None,
            (area, index) if area >= 0 && index >= 0 => Some((area as u32, index as u32)),
            _ => return ST_FAULT,
        };
        let argc = argc as usize;
        let args = if argc == 0 {
            &[]
        } else if args.is_null() {
            return ST_FAULT;
        } else {
            // SAFETY: compiled code provides exactly `argc` call descriptors.
            unsafe { std::slice::from_raw_parts(args, argc) }
        };
        let descriptor = {
            // SAFETY: this metadata borrow ends before the host call.
            let run_ref = unsafe { &*run };
            let Some((_, image)) = current_program_image(run_ref) else {
                return ST_FAULT;
            };
            if image.program.is_null() {
                return ST_FAULT;
            }
            // SAFETY: installed from the owning CompiledImage for this run.
            let program = unsafe { &*image.program };
            match program
                .external_calls
                .iter()
                .find(|candidate| candidate.descriptor_id == descriptor_id)
            {
                Some(descriptor) => descriptor.clone(),
                None => {
                    return raise_plan_error(
                        state,
                        run,
                        format!("unknown Declare descriptor {descriptor_id}"),
                    );
                }
            }
        };
        if let Err(message) = verified_declare_plan_for_jit(&descriptor) {
            return raise_plan_error(state, run, message);
        }
        let arg_variants = {
            // SAFETY: this operand-read borrow is bounded before the host call.
            let run_ref = unsafe { &*run };
            let mut values = Vec::with_capacity(argc);
            for arg in args {
                match call_arg_variant_value(run_ref, *arg) {
                    Some(value) => values.push(value),
                    None => return ST_FAULT,
                }
            }
            values
        };
        let param_type_strings: Vec<String> = descriptor
            .param_types
            .iter()
            .map(|ty| format!("{ty:?}"))
            .collect();
        let return_type = descriptor
            .return_type
            .as_ref()
            .map(|ty| std::borrow::Cow::Owned(format!("{ty:?}")));
        let view = DynLinkDescriptorView {
            descriptor_id: descriptor.descriptor_id,
            declared_name: &descriptor.declared_name,
            library: &descriptor.library,
            alias: &descriptor.alias,
            ordinal_alias: descriptor.ordinal_alias,
            symbol: descriptor.symbol,
            marshal_lane: &descriptor.marshal_lane,
            calling_convention: &descriptor.calling_convention,
            selection_policy: &descriptor.selection_policy,
            param_count: descriptor.param_count,
            param_types: &param_type_strings,
            param_by_ref: &descriptor.param_by_ref,
            return_type,
        };
        let host = {
            // SAFETY: copy the host reference so the typed state borrow ends before FFI.
            let exec = match unsafe { jit_exec_state_mut(state) } {
                Some(exec) => exec,
                None => return ST_FAULT,
            };
            exec.host
        };
        let invoke = host
            .dynlink()
            .invoke_descriptor_variants(&view, &arg_variants);
        let last_dll_error = host.dynlink().last_dll_error();
        {
            // SAFETY: the host call returned, so the live execution state may be borrowed again.
            let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
                return ST_FAULT;
            };
            exec.err_engine.last_dll_error = last_dll_error;
        }
        let (ret, wb_values) = match invoke {
            Ok(pair) => pair,
            Err(err) => {
                // SAFETY: the host call returned, so the live execution state may be borrowed again.
                let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
                    return ST_FAULT;
                };
                exec.err_engine
                    .raise(Fault::from_hal(err), current_unit_source(run));
                return ST_FAULT;
            }
        };
        // SAFETY: the host call returned and no typed run borrow spans it.
        let run = unsafe { &mut *run };
        for (index, arg) in args.iter().copied().enumerate() {
            if arg.kind != JIT_CALL_ARG_BYREF_ALIAS {
                continue;
            }
            let Some(value) = wb_values.get(index) else {
                continue;
            };
            if arg.area < 0 || arg.index < 0 {
                return ST_FAULT;
            }
            let Some(slot) = slot_mut(run, arg.area as u32, arg.index as u32) else {
                return ST_FAULT;
            };
            *slot = value.clone();
        }
        if let Some((area, index)) = dst {
            let Some(slot) = slot_mut(run, area, index) else {
                return ST_FAULT;
            };
            *slot = ret;
        }
        ST_OK
    })
}

pub(crate) fn call_by_name_member_name(value: &Variant) -> String {
    variant_to_vba_string(value)
        .map(|text| text.as_str())
        .unwrap_or_default()
}

pub(crate) fn call_by_name_calltype(value: &Variant) -> Result<i64, i32> {
    if let Some(value) = value.as_i64() {
        return Ok(value);
    }
    if let Some(value) = value.as_i32() {
        return Ok(i64::from(value));
    }
    if let Some(value) = value.as_i16() {
        return Ok(i64::from(value));
    }
    if let Some(value) = value.as_u8() {
        return Ok(i64::from(value));
    }
    if let Some(value) = value.as_bool() {
        return Ok(if value { -1 } else { 0 });
    }
    if let Some(value) = value.as_f64() {
        if !value.is_finite() || value.abs() >= 9.223_372_036_854_775e18 {
            return Err(6);
        }
        return Ok(value.round_ties_even() as i64);
    }
    if let Some(value) = value.as_f32() {
        let value = f64::from(value);
        if !value.is_finite() || value.abs() >= 9.223_372_036_854_775e18 {
            return Err(6);
        }
        return Ok(value.round_ties_even() as i64);
    }
    if value.vtype() == VarType::Null {
        return Err(94);
    }
    Err(13)
}

pub(crate) unsafe extern "C" fn rt_jit_call_by_name_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    operands: *const JitVariantOperandDesc,
    argc: i32,
    args: *const JitCallArgDesc,
    names: *const JitCallArgNameDesc,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || operands.is_null()
            || argc < 0
            || dst_area < -1
            || dst_index < -1
        {
            return ST_FAULT;
        }
        let dst = match (dst_area, dst_index) {
            (-1, -1) => None,
            (area, index) if area >= 0 && index >= 0 => Some((area as u32, index as u32)),
            _ => return ST_FAULT,
        };
        let argc = argc as usize;
        let args = if argc == 0 {
            &[]
        } else if args.is_null() || names.is_null() {
            return ST_FAULT;
        } else {
            // SAFETY: compiled code provides exactly `argc` call and name descriptors.
            unsafe { std::slice::from_raw_parts(args, argc) }
        };
        let names = if argc == 0 {
            &[]
        } else {
            // SAFETY: null was rejected above and the descriptor count matches `args`.
            unsafe { std::slice::from_raw_parts(names, argc) }
        };
        // SAFETY: compiled code writes object/name/calltype descriptors in that order.
        let operands = unsafe { std::slice::from_raw_parts(operands, 3) };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans possible As-New initializer callbacks.
        let object = match unsafe { variant_operand_value_with_as_new(run, state, operands[0]) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let name_value = match unsafe { variant_operand_value_with_as_new(run, state, operands[1]) }
        {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let calltype_value =
            match unsafe { variant_operand_value_with_as_new(run, state, operands[2]) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        let member_name = call_by_name_member_name(&name_value);
        let calltype = match call_by_name_calltype(&calltype_value) {
            Ok(value) => value,
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(code) => return unsafe { rt_raise_runtime_error_number(state, code) },
        };
        let kind = match calltype {
            1 => ProjectMemberKind::Method,
            2 => ProjectMemberKind::PropertyGet,
            4 => ProjectMemberKind::PropertyLet,
            8 => ProjectMemberKind::PropertySet,
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            _ => return unsafe { rt_raise_runtime_error_number(state, 5) },
        };
        // SAFETY: the checked JIT entry owns the live unique state and run;
        // descriptors, member name, and destination metadata remain live.
        unsafe {
            invoke_project_member_to_slot_for_jit(
                state,
                run,
                object,
                ProjectMemberInvocation {
                    name: &member_name,
                    kind,
                    args,
                    names,
                    dst,
                },
            )
        }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_project_type_name_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    operand: *const JitVariantOperandDesc,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || operand.is_null() || dst_area < 0 || dst_index < 0 {
            return ST_FAULT;
        }
        // SAFETY: the compiled caller provides one live descriptor.
        let operand = unsafe { *operand };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans a possible As-New initializer callback.
        let value = match unsafe { variant_operand_value_with_as_new(run, state, operand) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        let name = if value.as_object_ref().is_none()
            && matches!(
                value.vtype(),
                VarType::Object | VarType::Empty | VarType::Null
            ) {
            "Nothing".to_string()
        } else {
            // SAFETY: the current compiled-run boundary owns the live unique state handle;
            // typed references and owned values remain live and nonaliasing for this call.
            let object = match unsafe { variant_to_project_object_for_jit(state, &value) } {
                Ok(object) => object,
                Err(status) => return status,
            };
            if !object.is_project_instance() {
                return ST_FAULT;
            }
            object.class_descriptor().name.to_string()
        };
        // SAFETY: callback-capable lookup returned and no typed run borrow is live.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        *slot = Variant::from_string(name);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_new_record_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    fields: *const ArrayElementType,
    fields_len: i32,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || fields_len < 0
            || (fields_len > 0 && fields.is_null())
        {
            return ST_FAULT;
        }
        let fields = if fields_len == 0 {
            &[]
        } else {
            // SAFETY: null was rejected and the compiled image passes a pointer into
            // the live OxProgram instruction's immutable field-layout vector.
            unsafe { std::slice::from_raw_parts(fields, fields_len as usize) }
        };
        let layout = match vba_record_layout_for_fields(fields) {
            Ok(layout) => layout,
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(_) => return unsafe { rt_raise_type_mismatch(state) },
        };
        let record = match VbaRecord::new_default(layout) {
            Ok(record) => record,
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(_) => return unsafe { rt_raise_type_mismatch(state) },
        };
        // SAFETY: null was rejected and the new owned record is independent of source
        // metadata before overwriting the destination slot.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_vba_record(record);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_record_get_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    record: *const JitVariantOperandDesc,
    field_index: i32,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || record.is_null()
            || field_index < 0
            || dst_area < 0
            || dst_index < 0
        {
            return ST_FAULT;
        }
        // SAFETY: compiled caller provides one live descriptor.
        let record = unsafe { *record };
        // SAFETY: null was rejected and source is cloned before destination write.
        let run_ref = unsafe { &*run };
        let Some(record) = variant_operand_value_from_compiled_desc!(run_ref, record) else {
            return ST_FAULT;
        };
        let value = match record.read_record_field_variant(field_index as usize) {
            Ok(value) => value,
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(_) => return unsafe { rt_raise_type_mismatch(state) },
        };
        // SAFETY: null was rejected and the value no longer borrows from source slots.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        *slot = value;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_record_set(
    state: *mut RawExecState,
    run: *mut JitRun,
    record_area: i32,
    record_index: i32,
    field_index: i32,
    value: *const JitVariantOperandDesc,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || value.is_null()
            || record_area < 0
            || record_index < 0
            || field_index < 0
        {
            return ST_FAULT;
        }
        // SAFETY: compiled caller provides one live descriptor.
        let value = unsafe { *value };
        // SAFETY: null was rejected and value is cloned before mutable record access.
        let run_ref = unsafe { &*run };
        let Some(value) = variant_operand_value_from_compiled_desc!(run_ref, value) else {
            return ST_FAULT;
        };
        // SAFETY: immutable borrows ended; compiled caller gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(record) = slot_mut(run, record_area as u32, record_index as u32) else {
            return ST_FAULT;
        };
        match record.write_record_field_variant(field_index as usize, &value) {
            Ok(()) => ST_OK,
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(_) => unsafe { rt_raise_type_mismatch(state) },
        }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_record_lset(
    state: *mut RawExecState,
    run: *mut JitRun,
    record_area: i32,
    record_index: i32,
    value: *const JitVariantOperandDesc,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || value.is_null()
            || record_area < 0
            || record_index < 0
        {
            return ST_FAULT;
        }
        // SAFETY: compiled caller provides one live descriptor.
        let value = unsafe { *value };
        // SAFETY: null was rejected and value is cloned before mutable record access.
        let run_ref = unsafe { &*run };
        let Some(value) = variant_operand_value_from_compiled_desc!(run_ref, value) else {
            return ST_FAULT;
        };
        // SAFETY: immutable borrows ended; compiled caller gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(record) = slot_mut(run, record_area as u32, record_index as u32) else {
            return ST_FAULT;
        };
        match record.lset_record_from(&value) {
            Ok(()) => ST_OK,
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(_) => unsafe { rt_raise_type_mismatch(state) },
        }
    })
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn jit_flat_index_from_bounds(
    state: *mut RawExecState,
    bounds: &[SafeArrayBound],
    len: usize,
    indices: &[i32],
) -> Result<usize, i32> {
    if indices.len() != bounds.len() {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return Err(unsafe { rt_raise_subscript_out_of_range(state) });
    }
    let mut flat = 0usize;
    for (&raw, bound) in indices.iter().zip(bounds) {
        let offset = i64::from(raw) - i64::from(bound.lower);
        if offset < 0 || offset >= i64::from(bound.count) {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return Err(unsafe { rt_raise_subscript_out_of_range(state) });
        }
        let Ok(offset) = usize::try_from(offset) else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return Err(unsafe { rt_raise_subscript_out_of_range(state) });
        };
        flat = flat
            .checked_mul(bound.count as usize)
            .and_then(|base| base.checked_add(offset))
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            .ok_or_else(|| unsafe { rt_raise_subscript_out_of_range(state) })?;
    }
    if flat >= len {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return Err(unsafe { rt_raise_subscript_out_of_range(state) });
    }
    Ok(flat)
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn jit_record_array_flat_index(
    state: *mut RawExecState,
    record: &Variant,
    field_index: usize,
    indices: &[i32],
) -> Result<usize, i32> {
    let bounds_len = match record.record_array_field_bounds_len(field_index) {
        Ok(Some(bounds_len)) => bounds_len,
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        Ok(None) => return Err(unsafe { rt_raise_expected_array(state) }),
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        Err(_) => return Err(unsafe { rt_raise_type_mismatch(state) }),
    };
    let (bounds, len) = bounds_len;
    // SAFETY: the current compiled-run boundary owns the live unique state handle;
    // typed references and owned values remain live and nonaliasing for this call.
    unsafe { jit_flat_index_from_bounds(state, &bounds, len, indices) }
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn jit_array_element_value(
    state: *mut RawExecState,
    array: &Variant,
    indices: &[i32],
) -> Result<Variant, i32> {
    if array.vtype() != VarType::ArrayVariant {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return Err(unsafe { rt_raise_expected_array(state) });
    }
    // SAFETY: the current compiled-run boundary owns the live unique state handle;
    // typed references and owned values remain live and nonaliasing for this call.
    let flat = unsafe { jit_flat_index(state, array, indices) }?;
    if let Some(result) = array.safearray_i32_element(flat) {
        match result {
            Ok(Some(value)) => return Ok(Variant::from_i32(value)),
            Ok(None) => {}
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(_) => return Err(unsafe { rt_raise_type_mismatch(state) }),
        }
    }
    match array.safearray_element(flat) {
        Some(Ok(value)) => Ok(value),
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        Some(Err(_)) => Err(unsafe { rt_raise_type_mismatch(state) }),
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        None => Err(unsafe { rt_raise_expected_array(state) }),
    }
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn jit_array_element_set(
    state: *mut RawExecState,
    array: &mut Variant,
    indices: &[i32],
    value: &Variant,
) -> Result<(), i32> {
    if array.vtype() != VarType::ArrayVariant {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return Err(unsafe { rt_raise_expected_array(state) });
    }
    // SAFETY: the current compiled-run boundary owns the live unique state handle;
    // typed references and owned values remain live and nonaliasing for this call.
    let flat = unsafe { jit_flat_index(state, array, indices) }?;
    if let Some(value_i32) = value.as_i32()
        && let Some(result) = array.set_safearray_i32_element(flat, value_i32)
    {
        return match result {
            Ok(true) => Ok(()),
            Ok(false) => array
                .set_safearray_element(flat, value)
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                .map_err(|_| unsafe { rt_raise_type_mismatch(state) }),
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(_) => Err(unsafe { rt_raise_type_mismatch(state) }),
        };
    }
    array
        .set_safearray_element(flat, value)
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        .map_err(|_| unsafe { rt_raise_type_mismatch(state) })
}

pub(crate) unsafe extern "C" fn rt_jit_record_array_get_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    record: *const JitVariantOperandDesc,
    field_index: i32,
    indices: *const i32,
    dimensions: i32,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || record.is_null()
            || field_index < 0
            || dimensions <= 0
            || indices.is_null()
            || dst_area < 0
            || dst_index < 0
        {
            return ST_FAULT;
        }
        let dimensions = dimensions as usize;
        // SAFETY: compiled caller writes exactly `dimensions` subscripts.
        let indices = unsafe { std::slice::from_raw_parts(indices, dimensions) };
        // SAFETY: compiled caller provides one live descriptor.
        let record = unsafe { *record };
        // SAFETY: null was rejected and source is cloned before destination write.
        let run_ref = unsafe { &*run };
        let Some(record) = variant_operand_value_from_compiled_desc!(run_ref, record) else {
            return ST_FAULT;
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let flat = match unsafe {
            jit_record_array_flat_index(state, &record, field_index as usize, indices)
        } {
            Ok(flat) => flat,
            Err(status) => return status,
        };
        let value = match record.record_array_field_element(field_index as usize, flat) {
            Ok(Some(value)) => value,
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Ok(None) => return unsafe { rt_raise_expected_array(state) },
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(_) => return unsafe { rt_raise_type_mismatch(state) },
        };
        // SAFETY: null was rejected and value no longer borrows from source slots.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        *slot = value;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_record_array_set(
    state: *mut RawExecState,
    run: *mut JitRun,
    record_area: i32,
    record_index: i32,
    field_index: i32,
    indices: *const i32,
    dimensions: i32,
    value: *const JitVariantOperandDesc,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || value.is_null()
            || record_area < 0
            || record_index < 0
            || field_index < 0
            || dimensions <= 0
            || indices.is_null()
        {
            return ST_FAULT;
        }
        let dimensions = dimensions as usize;
        // SAFETY: compiled caller writes exactly `dimensions` subscripts.
        let indices = unsafe { std::slice::from_raw_parts(indices, dimensions) };
        // SAFETY: compiled caller provides one live descriptor.
        let value = unsafe { *value };
        // SAFETY: null was rejected and all immutable values are cloned before mutation.
        let run_ref = unsafe { &*run };
        let Some(value) = variant_operand_value_from_compiled_desc!(run_ref, value) else {
            return ST_FAULT;
        };
        let Some(record) = slot_ref(run_ref, record_area as u32, record_index as u32) else {
            return ST_FAULT;
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let flat = match unsafe {
            jit_record_array_flat_index(state, record, field_index as usize, indices)
        } {
            Ok(flat) => flat,
            Err(status) => return status,
        };
        // SAFETY: immutable borrows ended; compiled caller gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(record) = slot_mut(run, record_area as u32, record_index as u32) else {
            return ST_FAULT;
        };
        match record.set_record_array_field_element(field_index as usize, flat, &value) {
            Ok(Some(())) => ST_OK,
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Ok(None) => unsafe { rt_raise_expected_array(state) },
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(_) => unsafe { rt_raise_type_mismatch(state) },
        }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_project_field_array_get_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    object: *const JitVariantOperandDesc,
    field: i32,
    indices: *const i32,
    dimensions: i32,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || object.is_null()
            || dimensions <= 0
            || indices.is_null()
            || dst_area < 0
            || dst_index < 0
        {
            return ST_FAULT;
        }
        let dimensions = dimensions as usize;
        // SAFETY: compiled caller writes exactly `dimensions` subscripts.
        let indices = unsafe { std::slice::from_raw_parts(indices, dimensions) };
        // SAFETY: compiled caller provides one live descriptor.
        let object_operand = unsafe { *object };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans possible As-New initializer callbacks.
        let object_value =
            match unsafe { variant_operand_value_with_as_new(run, state, object_operand) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let object = match unsafe { variant_to_project_object_for_jit(state, &object_value) } {
            Ok(object) => object,
            Err(status) => return status,
        };
        let fast = object
            .with_project_field(field, |stored| {
                let stored = stored?;
                if stored.as_object_ref().is_some() {
                    return None;
                }
                // SAFETY: the current compiled-run boundary owns the live unique state handle;
                // typed references and owned values remain live and nonaliasing for this call.
                Some(unsafe { jit_array_element_value(state, stored, indices) })
            })
            .flatten();
        let value = match fast {
            Some(Ok(value)) => value,
            Some(Err(status)) => return status,
            None => {
                let field_value =
                    // SAFETY: the current compiled-run boundary owns the live unique state handle;
                    // typed references and owned values remain live and nonaliasing for this call.
                    match unsafe { project_field_get_with_as_new_for_jit(run, state, &object, field) } {
                        Ok(value) => value,
                        Err(status) => return status,
                    };
                if field_value.as_object_ref().is_some() {
                    let args: Vec<Variant> =
                        indices.iter().copied().map(Variant::from_i32).collect();
                    // SAFETY: the checked JIT entry owns the live unique state;
                    // `run`, the owned receiver, and `args` remain live for the call.
                    match unsafe {
                        invoke_project_default_member_values(
                            run,
                            state,
                            field_value,
                            ProjectMemberKind::PropertyGet,
                            &args,
                        )
                    } {
                        Ok(value) => value,
                        Err(status) => return status,
                    }
                } else {
                    // SAFETY: the current compiled-run boundary owns the live unique state handle;
                    // typed references and owned values remain live and nonaliasing for this call.
                    match unsafe { jit_array_element_value(state, &field_value, indices) } {
                        Ok(value) => value,
                        Err(status) => return status,
                    }
                }
            }
        };
        // SAFETY: callback-capable lookups/member entry returned.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
            return ST_FAULT;
        };
        *slot = value;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_project_field_array_set(
    state: *mut RawExecState,
    run: *mut JitRun,
    object: *const JitVariantOperandDesc,
    field: i32,
    indices: *const i32,
    dimensions: i32,
    value: *const JitVariantOperandDesc,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || object.is_null()
            || value.is_null()
            || dimensions <= 0
            || indices.is_null()
        {
            return ST_FAULT;
        }
        let dimensions = dimensions as usize;
        // SAFETY: compiled caller writes exactly `dimensions` subscripts.
        let indices = unsafe { std::slice::from_raw_parts(indices, dimensions) };
        // SAFETY: compiled caller provides live descriptors.
        let object_operand = unsafe { *object };
        // SAFETY: the same checked descriptor pair keeps `value` live for this load.
        let value_operand = unsafe { *value };
        let value = {
            // SAFETY: this read-only value borrow is bounded before As-New callbacks.
            let run_ref = unsafe { &*run };
            match variant_operand_value_from_compiled_desc!(run_ref, value_operand) {
                Some(value) => value,
                None => return ST_FAULT,
            }
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans a possible As-New initializer callback.
        let object_value =
            match unsafe { variant_operand_value_with_as_new(run, state, object_operand) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let object = match unsafe { variant_to_project_object_for_jit(state, &object_value) } {
            Ok(object) => object,
            Err(status) => return status,
        };
        let fast = object
            .with_project_field_mut(field, |stored| {
                if stored.as_object_ref().is_some() {
                    return None;
                }
                // SAFETY: the current compiled-run boundary owns the live unique state handle;
                // typed references and owned values remain live and nonaliasing for this call.
                Some(unsafe { jit_array_element_set(state, stored, indices, &value) })
            })
            .flatten();
        match fast {
            Some(Ok(())) => return ST_OK,
            Some(Err(status)) => return status,
            None => {}
        }

        let mut field_value =
            // SAFETY: the current compiled-run boundary owns the live unique state handle;
            // typed references and owned values remain live and nonaliasing for this call.
            match unsafe { project_field_get_with_as_new_for_jit(run, state, &object, field) } {
                Ok(value) => value,
                Err(status) => return status,
            };
        if field_value.as_object_ref().is_some() {
            let mut args: Vec<Variant> = indices.iter().copied().map(Variant::from_i32).collect();
            args.push(value);
            let invoke_kind = if args.last().and_then(Variant::as_object_ref).is_some() {
                ProjectMemberKind::PropertySet
            } else {
                ProjectMemberKind::PropertyLet
            };
            // SAFETY: the checked JIT entry owns the live unique state; `run`,
            // the owned receiver, and `args` remain live for the call.
            return match unsafe {
                invoke_project_default_member_values(run, state, field_value, invoke_kind, &args)
            } {
                Ok(_) => ST_OK,
                Err(status) => status,
            };
        }
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        if let Err(status) =
            // SAFETY: `state` is the current live unique handle; `field_value`,
            // indices, and source value are initialized, live, and nonaliasing.
            unsafe { jit_array_element_set(state, &mut field_value, indices, &value) }
        {
            return status;
        }
        if object.project_field_set(field, field_value) {
            ST_OK
        } else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            unsafe { rt_raise_runtime_error_number(state, 438) }
        }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_validate_assignment(
    state: *mut RawExecState,
    run: *mut JitRun,
    operand: *const JitVariantOperandDesc,
    intent: i32,
    target_kind: i32,
    target_type_name_ptr: *const u8,
    target_type_name_len: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null()
            || state.is_null()
            || operand.is_null()
            || target_type_name_len < 0
            || (target_type_name_len > 0 && target_type_name_ptr.is_null())
        {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes one descriptor to
        // a stack slot that stays live for this helper call.
        let operand = unsafe { *operand };
        // SAFETY: null was rejected and this helper clones the source only.
        let run_ref = unsafe { &*run };
        let Some(value) = variant_operand_value_from_compiled_desc!(run_ref, operand) else {
            return ST_FAULT;
        };
        let is_object = matches!(value.vtype(), VarType::Object) || jit_is_nothing(&value);
        match intent {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            JIT_ASSIGN_INTENT_SET if !is_object => unsafe {
                rt_raise_runtime_error_number(state, 424)
            },
            JIT_ASSIGN_INTENT_LET
                if target_kind == JIT_ASSIGN_TARGET_VARIANT
                    && matches!(value.vtype(), VarType::Object)
                    && jit_is_nothing(&value) =>
            {
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                unsafe { rt_raise_runtime_error_number(state, 91) }
            }
            JIT_ASSIGN_INTENT_LET if target_kind == JIT_ASSIGN_TARGET_OBJECT && is_object => {
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                unsafe { rt_raise_runtime_error_number(state, 91) }
            }
            JIT_ASSIGN_INTENT_LET if target_kind == JIT_ASSIGN_TARGET_OBJECT => {
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                unsafe { rt_raise_runtime_error_number(state, 424) }
            }
            JIT_ASSIGN_INTENT_SET
                if matches!(value.vtype(), VarType::Object) && target_type_name_len > 0 =>
            {
                // SAFETY: the JIT entry contract supplies the live state and a
                // pointer/length pair for one initialized target-name byte range;
                // typed `run_ref` and `value` borrows remain live and nonaliasing.
                unsafe {
                    validate_jit_project_set_compatibility(
                        state,
                        run_ref,
                        &value,
                        target_type_name_ptr,
                        target_type_name_len,
                    )
                }
            }
            JIT_ASSIGN_INTENT_IMPLICIT | JIT_ASSIGN_INTENT_LET | JIT_ASSIGN_INTENT_SET => ST_OK,
            _ => ST_FAULT,
        }
    })
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn validate_jit_project_set_compatibility(
    state: *mut RawExecState,
    run: &JitRun,
    value: &Variant,
    target_type_name_ptr: *const u8,
    target_type_name_len: i32,
) -> i32 {
    if jit_is_nothing(value) {
        return ST_OK;
    }
    let Some(object) = value.as_object_ref() else {
        return ST_OK;
    };
    if !object.is_project_instance() {
        return ST_OK;
    }
    let target_type_name = if target_type_name_len == 0 {
        ""
    } else {
        // SAFETY: pointer and length were validated by the extern helper and point to
        // immutable OxIR metadata that outlives the compiled run.
        let bytes = unsafe {
            std::slice::from_raw_parts(target_type_name_ptr, target_type_name_len as usize)
        };
        match std::str::from_utf8(bytes) {
            Ok(name) => name,
            Err(_) => return ST_FAULT,
        }
    };
    let bare_target = target_type_name
        .rsplit('.')
        .next()
        .unwrap_or(target_type_name);
    if bare_target.is_empty() {
        return ST_OK;
    }
    let Some(image) = program_image(run, object.bundle_id() as usize) else {
        return ST_FAULT;
    };
    if image.program.is_null() {
        return ST_FAULT;
    }
    // SAFETY: the image was installed from CompiledImage for this run.
    let program = unsafe { &*image.program };
    let target_is_project = program.classes.iter().any(|class| {
        class.name.eq_ignore_ascii_case(bare_target)
            || class
                .implements
                .iter()
                .any(|interface| interface.eq_ignore_ascii_case(bare_target))
    });
    if !target_is_project {
        return ST_OK;
    }
    let Some(class) = program.classes.get(object.route_key() as usize) else {
        return ST_FAULT;
    };
    let compatible = class.name.eq_ignore_ascii_case(bare_target)
        || class
            .implements
            .iter()
            .any(|interface| interface.eq_ignore_ascii_case(bare_target));
    if compatible {
        ST_OK
    } else {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        unsafe { rt_raise_type_mismatch(state) }
    }
}

pub(crate) unsafe extern "C" fn rt_jit_array_literal_to_slot(
    run: *mut JitRun,
    operands: *const JitVariantOperandDesc,
    aliases: *const JitSlotAliasDesc,
    argc: i32,
    lower_bound: i32,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || argc < 0 || (argc > 0 && operands.is_null()) {
            return ST_FAULT;
        }
        let argc = argc as usize;
        let operands = if argc == 0 {
            &[]
        } else {
            // SAFETY: null was rejected and the compiled caller writes `argc`
            // descriptors to a stack slot that stays live for this helper call.
            unsafe { std::slice::from_raw_parts(operands, argc) }
        };
        // SAFETY: null was rejected and operand values are cloned before destination write.
        let run_ref = unsafe { &*run };
        let mut values = Vec::with_capacity(argc);
        for operand in operands {
            let Some(value) = variant_operand_value_from_compiled_desc!(run_ref, *operand) else {
                return ST_FAULT;
            };
            values.push(value);
        }
        let alias_targets = if aliases.is_null() {
            None
        } else {
            // SAFETY: the compiled caller writes `argc` alias descriptors to a stack slot
            // that stays live for this helper call when this pointer is non-null.
            let aliases = unsafe { std::slice::from_raw_parts(aliases, argc) };
            let mut targets = Vec::with_capacity(argc);
            for alias in aliases {
                let target = match (alias.area, alias.index) {
                    (-1, -1) => None,
                    (area, index) if area >= 0 && index >= 0 => {
                        let Some(alias) = current_frame_slot(run_ref, area as u32, index as u32)
                            .and_then(|alias| resolve_slot_alias(run_ref, alias))
                        else {
                            return ST_FAULT;
                        };
                        Some(alias)
                    }
                    _ => return ST_FAULT,
                };
                targets.push(target);
            }
            Some(targets)
        };
        let array = if lower_bound == 0 {
            SafeArray::from_variants(values)
        } else {
            let Ok(count) = u32::try_from(argc) else {
                return ST_FAULT;
            };
            SafeArray::from_variants_nd(
                vec![SafeArrayBound {
                    count,
                    lower: lower_bound,
                }],
                values,
            )
        };
        // SAFETY: null was rejected and operand clones no longer borrow from `run`.
        let run = unsafe { &mut *run };
        let Some(dst_alias) = current_frame_slot(run, dst_area, dst_index)
            .and_then(|alias| resolve_slot_alias(run, alias))
        else {
            return ST_FAULT;
        };
        let Some(slot) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_safearray(array);
        match alias_targets {
            Some(targets) if targets.iter().any(Option::is_some) => {
                run.param_array_aliases.insert(dst_alias, targets);
            }
            _ => {
                run.param_array_aliases.remove(&dst_alias);
            }
        }
        ST_OK
    })
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn jit_build_array_bounds(
    state: *mut RawExecState,
    lower_bounds: &[i32],
    upper_bounds: &[i32],
) -> Result<Vec<SafeArrayBound>, i32> {
    if upper_bounds.is_empty() || lower_bounds.len() != upper_bounds.len() {
        return Err(ST_FAULT);
    }
    let mut bounds = Vec::with_capacity(upper_bounds.len());
    for (&lower, &upper) in lower_bounds.iter().zip(upper_bounds) {
        if upper < lower {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return Err(unsafe { rt_raise_subscript_out_of_range(state) });
        }
        let span = i64::from(upper) - i64::from(lower) + 1;
        let Ok(count) = u32::try_from(span) else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return Err(unsafe { rt_raise_out_of_memory(state) });
        };
        bounds.push(SafeArrayBound { count, lower });
    }
    Ok(bounds)
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn jit_array_element_count(
    state: *mut RawExecState,
    bounds: &[SafeArrayBound],
) -> Result<usize, i32> {
    let mut count = 1usize;
    for bound in bounds {
        count = count
            .checked_mul(bound.count as usize)
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            .ok_or_else(|| unsafe { rt_raise_out_of_memory(state) })?;
    }
    Ok(count)
}

pub(crate) enum JitRedimDefaultArrayError {
    OutOfMemory,
    TypeMismatch,
}

pub(crate) fn jit_redim_element_supports_zeroed(element: &ArrayElementType) -> bool {
    matches!(
        element,
        ArrayElementType::Integer
            | ArrayElementType::Long
            | ArrayElementType::LongLong
            | ArrayElementType::Byte
            | ArrayElementType::Single
            | ArrayElementType::Double
            | ArrayElementType::Currency
            | ArrayElementType::Date
            | ArrayElementType::Boolean
    )
}

/// Fill a capacity-reserved vector with `count` default array elements,
/// propagating a UDT-record layout/allocation failure (guest-legal but
/// oversized/invalid record) instead of panicking inside `resize_with`.
pub(crate) fn jit_fill_default_array_elements(
    values: &mut Vec<Variant>,
    element: &ArrayElementType,
    count: usize,
) -> Result<(), String> {
    for _ in 0..count {
        values.push(default_array_element(element)?);
    }
    Ok(())
}

pub(crate) fn jit_redim_default_safearray(
    bounds: Vec<SafeArrayBound>,
    element: &ArrayElementType,
    count: usize,
    fixed: bool,
) -> Result<SafeArray, JitRedimDefaultArrayError> {
    if jit_redim_element_supports_zeroed(element) {
        return SafeArray::from_zeroed_typed_scalars_nd(
            bounds,
            safearray_vartype_for_element(element),
        )
        .map(|array| array.with_fixed_size(fixed))
        .map_err(|_| JitRedimDefaultArrayError::OutOfMemory);
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| JitRedimDefaultArrayError::OutOfMemory)?;
    jit_fill_default_array_elements(&mut values, element, count)
        .map_err(|_| JitRedimDefaultArrayError::TypeMismatch)?;
    redim_safearray_from_elements(bounds, element, values, fixed)
        .map_err(|_| JitRedimDefaultArrayError::TypeMismatch)
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn jit_flat_index(
    state: *mut RawExecState,
    value: &Variant,
    indices: &[i32],
) -> Result<usize, i32> {
    let Some((bounds, len)) = value.safearray_bounds_len() else {
        return if value.vtype() == VarType::ArrayVariant {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(unsafe { rt_raise_array_has_no_bounds(state) })
        } else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(unsafe { rt_raise_expected_array(state) })
        };
    };
    if indices.len() != bounds.len() {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return Err(unsafe { rt_raise_subscript_out_of_range(state) });
    }
    if let ([raw], [bound]) = (indices, bounds.as_slice()) {
        let offset = i64::from(*raw) - i64::from(bound.lower);
        if offset < 0 || offset >= i64::from(bound.count) {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return Err(unsafe { rt_raise_subscript_out_of_range(state) });
        }
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        let flat = usize::try_from(offset)
            .map_err(|_| unsafe { rt_raise_subscript_out_of_range(state) })?;
        if flat >= len {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return Err(unsafe { rt_raise_subscript_out_of_range(state) });
        }
        return Ok(flat);
    }
    let mut flat = 0usize;
    for (&raw, bound) in indices.iter().zip(&bounds) {
        let offset = i64::from(raw) - i64::from(bound.lower);
        if offset < 0 || offset >= i64::from(bound.count) {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return Err(unsafe { rt_raise_subscript_out_of_range(state) });
        }
        let Ok(offset) = usize::try_from(offset) else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return Err(unsafe { rt_raise_subscript_out_of_range(state) });
        };
        flat = flat
            .checked_mul(bound.count as usize)
            .and_then(|base| base.checked_add(offset))
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            .ok_or_else(|| unsafe { rt_raise_subscript_out_of_range(state) })?;
    }
    if flat >= len {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return Err(unsafe { rt_raise_subscript_out_of_range(state) });
    }
    Ok(flat)
}

// SAFETY CONTRACT: `state` must be null or the exact live, uniquely borrowed,
// same-thread handle produced by `exec_state_as_raw`; any additional raw pointer
// and length arguments must identify the initialized, nonaliasing storage described
// by their typed parameters for the complete synchronous call.
pub(crate) unsafe fn jit_flat_index_1d(
    state: *mut RawExecState,
    value: &Variant,
    index: i32,
) -> Result<usize, i32> {
    let Some((bounds, len)) = value.safearray_bounds_len() else {
        return if value.vtype() == VarType::ArrayVariant {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(unsafe { rt_raise_array_has_no_bounds(state) })
        } else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Err(unsafe { rt_raise_expected_array(state) })
        };
    };
    let [bound] = bounds.as_slice() else {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return Err(unsafe { rt_raise_subscript_out_of_range(state) });
    };
    let offset = i64::from(index) - i64::from(bound.lower);
    if offset < 0 || offset >= i64::from(bound.count) {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return Err(unsafe { rt_raise_subscript_out_of_range(state) });
    }
    // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
    let flat =
        usize::try_from(offset).map_err(|_| unsafe { rt_raise_subscript_out_of_range(state) })?;
    if flat >= len {
        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
        return Err(unsafe { rt_raise_subscript_out_of_range(state) });
    }
    Ok(flat)
}

pub(crate) fn jit_remap_preserve_index(
    old_flat: usize,
    old_bounds: &[SafeArrayBound],
    new_bounds: &[SafeArrayBound],
) -> Option<usize> {
    let rank = old_bounds.len();
    let mut offsets = vec![0usize; rank];
    let mut rem = old_flat;
    for dimension in (0..rank).rev() {
        let count = old_bounds[dimension].count as usize;
        if count == 0 {
            return None;
        }
        offsets[dimension] = rem % count;
        rem /= count;
    }
    let mut new_flat = 0usize;
    for dimension in 0..rank {
        let new_count = new_bounds[dimension].count as i64;
        let abs = i64::from(old_bounds[dimension].lower) + offsets[dimension] as i64;
        let new_offset = abs - i64::from(new_bounds[dimension].lower);
        if new_offset < 0 || new_offset >= new_count {
            return None;
        }
        new_flat = new_flat * new_count as usize + new_offset as usize;
    }
    Some(new_flat)
}

pub(crate) unsafe extern "C" fn rt_jit_array_redim_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lower_bounds: *const i32,
    upper_bounds: *const i32,
    dimensions: i32,
    element: *const ArrayElementType,
    fixed: i32,
    preserve: i32,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || element.is_null() {
            return ST_FAULT;
        }
        if dimensions <= 0 || lower_bounds.is_null() || upper_bounds.is_null() {
            return ST_FAULT;
        }
        let dimensions = dimensions as usize;
        // SAFETY: null was rejected and the compiled caller writes exactly `dimensions`
        // lower/upper values to stack slots that stay live for this helper call.
        let lower_bounds = unsafe { std::slice::from_raw_parts(lower_bounds, dimensions) };
        // SAFETY: the same checked pointer/length contract covers the upper-bound array.
        let upper_bounds = unsafe { std::slice::from_raw_parts(upper_bounds, dimensions) };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let bounds = match unsafe { jit_build_array_bounds(state, lower_bounds, upper_bounds) } {
            Ok(bounds) => bounds,
            Err(status) => return status,
        };
        if fixed == 0 {
            // SAFETY: null was rejected; this is a read-only guard before building the
            // replacement value or taking a mutable slot reference.
            let run_ref = unsafe { &*run };
            let Some(current) = slot_ref(run_ref, dst_area, dst_index) else {
                return ST_FAULT;
            };
            if current
                .as_safearray()
                .is_some_and(|array| array.is_fixed_size())
            {
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                return unsafe { rt_raise_fixed_or_temporarily_locked_array(state) };
            }
        }
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let count = match unsafe { jit_array_element_count(state, &bounds) } {
            Ok(count) => count,
            Err(status) => return status,
        };
        // SAFETY: null was rejected and the compiled image passes a pointer into
        // the live OxProgram instruction's immutable element metadata.
        let element = unsafe { (*element).clone() };
        let array = if preserve == 0 {
            match jit_redim_default_safearray(bounds, &element, count, fixed != 0) {
                Ok(array) => array,
                Err(JitRedimDefaultArrayError::OutOfMemory) => {
                    // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                    return unsafe { rt_raise_out_of_memory(state) };
                }
                Err(JitRedimDefaultArrayError::TypeMismatch) => {
                    // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                    return unsafe { rt_raise_type_mismatch(state) };
                }
            }
        } else {
            let mut values = Vec::new();
            if values.try_reserve_exact(count).is_err() {
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                return unsafe { rt_raise_out_of_memory(state) };
            }
            if jit_fill_default_array_elements(&mut values, &element, count).is_err() {
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                return unsafe { rt_raise_type_mismatch(state) };
            }
            // SAFETY: null was rejected; all element reads clone into `values` before the
            // destination slot is replaced below.
            let run_ref = unsafe { &*run };
            let Some(current) = slot_ref(run_ref, dst_area, dst_index) else {
                return ST_FAULT;
            };
            if let Some((old_bounds, old_len)) = current.safearray_bounds_len() {
                let rank = bounds.len();
                let illegal = old_bounds.len() != rank
                    || (0..rank.saturating_sub(1)).any(|index| old_bounds[index] != bounds[index])
                    || match (old_bounds.last(), bounds.last()) {
                        (Some(old), Some(new)) => old.lower != new.lower,
                        _ => false,
                    };
                if illegal {
                    // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                    return unsafe { rt_raise_subscript_out_of_range(state) };
                }
                for index in 0..old_len {
                    let Some(new_index) = jit_remap_preserve_index(index, &old_bounds, &bounds)
                    else {
                        continue;
                    };
                    if new_index >= values.len() {
                        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                        return unsafe { rt_raise_subscript_out_of_range(state) };
                    }
                    let value = match current.safearray_element(index) {
                        Some(Ok(value)) => value,
                        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                        Some(Err(_)) => return unsafe { rt_raise_type_mismatch(state) },
                        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                        None => return unsafe { rt_raise_expected_array(state) },
                    };
                    values[new_index] = value;
                }
            }
            match redim_safearray_from_elements(bounds, &element, values, fixed != 0) {
                Ok(array) => array,
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                Err(_) => return unsafe { rt_raise_type_mismatch(state) },
            }
        };
        // SAFETY: null was rejected and the freshly built array no longer borrows from `run`.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_safearray(array);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_array_erase_variant_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    array_area: u32,
    array_index: u32,
    element: *const ArrayElementType,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || element.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled image passes a pointer into
        // the live OxProgram instruction's immutable element metadata.
        let bind_element = unsafe { (*element).clone() };
        // SAFETY: null was rejected and the replacement value is built before the slot write.
        let run_ref = unsafe { &*run };
        let Some(current) = slot_ref(run_ref, array_area, array_index) else {
            return ST_FAULT;
        };
        let was_array = current.vtype() == VarType::ArrayVariant;
        let erased_element_vartype = current.array_element_vartype().unwrap_or(VT_VARIANT_VALUE);
        let replacement = match current.as_safearray().filter(|array| array.is_fixed_size()) {
            Some(array) => {
                let bounds = array.bounds().unwrap_or_default();
                let count = array.len();
                let element = match bind_element {
                    ArrayElementType::Variant => {
                        array_element_type_for_vartype(array.element_vartype())
                    }
                    other => other,
                };
                if jit_redim_element_supports_zeroed(&element) {
                    match SafeArray::from_zeroed_typed_scalars_nd(
                        bounds,
                        safearray_vartype_for_element(&element),
                    ) {
                        Ok(array) => Variant::from_safearray(array.with_fixed_size(true)),
                        Err(_) => return ST_FAULT,
                    }
                } else {
                    let mut values = Vec::new();
                    if values.try_reserve_exact(count).is_err() {
                        return ST_FAULT;
                    }
                    if jit_fill_default_array_elements(&mut values, &element, count).is_err() {
                        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                        return unsafe { rt_raise_type_mismatch(state) };
                    }
                    match redim_safearray_from_elements(bounds, &element, values, true) {
                        Ok(array) => Variant::from_safearray(array),
                        // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                        Err(_) => return unsafe { rt_raise_type_mismatch(state) },
                    }
                }
            }
            None if was_array => Variant::unallocated_array(erased_element_vartype),
            None => Variant::empty(),
        };
        // SAFETY: null was rejected and the replacement no longer borrows from `run`.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, array_area, array_index) else {
            return ST_FAULT;
        };
        *slot = replacement;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_array_get_i32_1d_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    array_area: u32,
    array_index: u32,
    index: i32,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the selected element is copied before the
        // destination write.
        let run_ref = unsafe { &*run };
        let Some(array) = slot_ref(run_ref, array_area, array_index) else {
            return ST_FAULT;
        };
        if array.vtype() != VarType::ArrayVariant {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_expected_array(state) };
        }
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let flat = match unsafe { jit_flat_index_1d(state, array, index) } {
            Ok(flat) => flat,
            Err(status) => return status,
        };
        let value = match array.safearray_i32_element(flat) {
            Some(Ok(Some(value))) => Variant::from_i32(value),
            Some(Ok(None)) => match array.safearray_element(flat) {
                Some(Ok(value)) => value,
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                Some(Err(_)) => return unsafe { rt_raise_type_mismatch(state) },
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                None => return unsafe { rt_raise_expected_array(state) },
            },
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Some(Err(_)) => return unsafe { rt_raise_type_mismatch(state) },
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            None => return unsafe { rt_raise_expected_array(state) },
        };
        // SAFETY: null was rejected and the element value no longer borrows from `run`.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        *slot = value;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_array_get_variant_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    array_area: u32,
    array_index: u32,
    indices: *const i32,
    dimensions: i32,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() {
            return ST_FAULT;
        }
        if dimensions <= 0 || indices.is_null() {
            return ST_FAULT;
        }
        let dimensions = dimensions as usize;
        // SAFETY: null was rejected and the compiled caller writes exactly `dimensions`
        // subscripts to a stack slot that stays live for this helper call.
        let indices = unsafe { std::slice::from_raw_parts(indices, dimensions) };
        let array = {
            // SAFETY: this source borrow is bounded before possible default-member entry.
            let run_ref = unsafe { &*run };
            let Some(array) = slot_ref(run_ref, array_area, array_index) else {
                return ST_FAULT;
            };
            array.clone()
        };
        if array.vtype() != VarType::ArrayVariant {
            if array.as_object_ref().is_some() {
                let recv_value = array.clone();
                let args: Vec<Variant> = indices.iter().copied().map(Variant::from_i32).collect();
                // SAFETY: the checked JIT entry owns the live unique state; `run`,
                // the owned receiver, and `args` remain live for the call.
                let value = match unsafe {
                    invoke_project_default_member_values(
                        run,
                        state,
                        recv_value,
                        ProjectMemberKind::PropertyGet,
                        &args,
                    )
                } {
                    Ok(value) => value,
                    Err(status) => return status,
                };
                // SAFETY: default-member entry returned and no typed run borrow spans it.
                let run = unsafe { &mut *run };
                let Some(slot) = slot_mut(run, dst_area, dst_index) else {
                    return ST_FAULT;
                };
                *slot = value;
                return ST_OK;
            }
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_expected_array(state) };
        }
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let flat = match unsafe { jit_flat_index(state, &array, indices) } {
            Ok(flat) => flat,
            Err(status) => return status,
        };
        if let Some(result) = array.safearray_i32_element(flat) {
            match result {
                Ok(Some(value)) => {
                    // SAFETY: null was rejected and the scalar element no longer borrows from `run`.
                    let run = unsafe { &mut *run };
                    let Some(slot) = slot_mut(run, dst_area, dst_index) else {
                        return ST_FAULT;
                    };
                    *slot = Variant::from_i32(value);
                    return ST_OK;
                }
                Ok(None) => {}
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                Err(_) => return unsafe { rt_raise_type_mismatch(state) },
            }
        }
        let value = match array.safearray_element(flat) {
            Some(Ok(value)) => value,
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Some(Err(_)) => return unsafe { rt_raise_type_mismatch(state) },
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            None => return unsafe { rt_raise_expected_array(state) },
        };
        // SAFETY: null was rejected and the element clone no longer borrows from `run`.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        *slot = value;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_array_set_i32_1d_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    array_area: u32,
    array_index: u32,
    index: i32,
    value: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and immutable reads finish before mutation.
        let run_ref = unsafe { &*run };
        let Some(array) = slot_ref(run_ref, array_area, array_index) else {
            return ST_FAULT;
        };
        if array.vtype() != VarType::ArrayVariant {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_expected_array(state) };
        }
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let flat = match unsafe { jit_flat_index_1d(state, array, index) } {
            Ok(flat) => flat,
            Err(status) => return status,
        };
        let Some(array_alias) = current_frame_slot(run_ref, array_area, array_index)
            .and_then(|alias| resolve_slot_alias(run_ref, alias))
        else {
            return ST_FAULT;
        };
        // SAFETY: null was rejected and all immutable borrows ended before mutation.
        let run = unsafe { &mut *run };
        let Some(array) = slot_mut(run, array_area, array_index) else {
            return ST_FAULT;
        };
        let value_variant = Variant::from_i32(value);
        match array.set_safearray_i32_element(flat, value) {
            Some(Ok(true)) => {}
            Some(Ok(false)) => {
                if array.set_safearray_element(flat, &value_variant).is_err() {
                    // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                    return unsafe { rt_raise_type_mismatch(state) };
                }
            }
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            Some(Err(_)) => return unsafe { rt_raise_type_mismatch(state) },
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            None => return unsafe { rt_raise_expected_array(state) },
        }
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        match unsafe {
            mirror_param_array_element_write(state, run, array_alias, flat, &value_variant)
        } {
            Ok(()) => ST_OK,
            Err(status) => status,
        }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_array_set_variant_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    array_area: u32,
    array_index: u32,
    indices: *const i32,
    dimensions: i32,
    value: *const JitVariantOperandDesc,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || value.is_null() {
            return ST_FAULT;
        }
        if dimensions <= 0 || indices.is_null() {
            return ST_FAULT;
        }
        let dimensions = dimensions as usize;
        // SAFETY: null was rejected and the compiled caller writes exactly `dimensions`
        // subscripts to a stack slot that stays live for this helper call.
        let indices = unsafe { std::slice::from_raw_parts(indices, dimensions) };
        // SAFETY: null was rejected and the compiled caller writes one descriptor to
        // a stack slot that stays live for this helper call.
        let value = unsafe { *value };
        let (value, array) = {
            // SAFETY: both source reads are bounded before possible default-member entry.
            let run_ref = unsafe { &*run };
            let Some(value) = variant_operand_value_from_compiled_desc!(run_ref, value) else {
                return ST_FAULT;
            };
            let Some(array) = slot_ref(run_ref, array_area, array_index) else {
                return ST_FAULT;
            };
            (value, array.clone())
        };
        if array.vtype() != VarType::ArrayVariant {
            if array.as_object_ref().is_some() {
                let recv_value = array.clone();
                let mut args: Vec<Variant> =
                    indices.iter().copied().map(Variant::from_i32).collect();
                args.push(value);
                let invoke_kind = if args.last().and_then(Variant::as_object_ref).is_some() {
                    ProjectMemberKind::PropertySet
                } else {
                    ProjectMemberKind::PropertyLet
                };
                // SAFETY: the checked JIT entry owns the live unique state; `run`,
                // the owned receiver, and `args` remain live for the call.
                return match unsafe {
                    invoke_project_default_member_values(run, state, recv_value, invoke_kind, &args)
                } {
                    Ok(_) => ST_OK,
                    Err(status) => status,
                };
            }
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_expected_array(state) };
        }
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let flat = match unsafe { jit_flat_index(state, &array, indices) } {
            Ok(flat) => flat,
            Err(status) => return status,
        };
        let array_alias = {
            // SAFETY: default-member path returned; this alias read is bounded before mutation.
            let run_ref = unsafe { &*run };
            let Some(array_alias) = current_frame_slot(run_ref, array_area, array_index)
                .and_then(|alias| resolve_slot_alias(run_ref, alias))
            else {
                return ST_FAULT;
            };
            array_alias
        };
        // SAFETY: null was rejected and all immutable borrows ended before the mutable slot write.
        let run = unsafe { &mut *run };
        let Some(array) = slot_mut(run, array_area, array_index) else {
            return ST_FAULT;
        };
        if let Some(value_i32) = value.as_i32()
            && let Some(result) = array.set_safearray_i32_element(flat, value_i32)
        {
            match result {
                Ok(true) => {
                    // SAFETY: the checked JIT entry owns the live unique state;
                    // `run` and the initialized source Variant remain live.
                    return match unsafe {
                        mirror_param_array_element_write(state, run, array_alias, flat, &value)
                    } {
                        Ok(()) => ST_OK,
                        Err(status) => status,
                    };
                }
                Ok(false) => {}
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                Err(_) => return unsafe { rt_raise_type_mismatch(state) },
            }
        }
        if array.set_safearray_element(flat, &value).is_err() {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_type_mismatch(state) };
        }
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        match unsafe { mirror_param_array_element_write(state, run, array_alias, flat, &value) } {
            Ok(()) => ST_OK,
            Err(status) => status,
        }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_bound_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    operand: *const JitVariantOperandDesc,
    which: i32,
    dimension: i32,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || operand.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes one descriptor
        // to a stack slot that stays live for this helper call.
        let operand = unsafe { *operand };
        // SAFETY: null was rejected and the operand is read before destination write.
        let run_ref = unsafe { &*run };
        let Some(src) = variant_operand_value_from_compiled_desc!(run_ref, operand) else {
            return ST_FAULT;
        };
        let Some((bounds, _len)) = src.safearray_bounds_len() else {
            return if src.vtype() == VarType::ArrayVariant {
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                unsafe { rt_raise_array_has_no_bounds(state) }
            } else {
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                unsafe { rt_raise_expected_array(state) }
            };
        };
        if dimension <= 0 {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_subscript_out_of_range(state) };
        }
        let Ok(index) = usize::try_from(dimension - 1) else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_subscript_out_of_range(state) };
        };
        let Some(bound) = bounds.get(index) else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_subscript_out_of_range(state) };
        };
        let value = match which {
            0 => bound.lower,
            1 => {
                let Ok(count) = i32::try_from(bound.count) else {
                    // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                    return unsafe { rt_raise_subscript_out_of_range(state) };
                };
                let Some(value) = bound
                    .lower
                    .checked_add(count)
                    .and_then(|value| value.checked_sub(1))
                else {
                    // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                    return unsafe { rt_raise_subscript_out_of_range(state) };
                };
                value
            }
            _ => return ST_FAULT,
        };
        // SAFETY: null was rejected and the source clone no longer borrows from `run`.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        *slot = Variant::from_i32(value);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_for_each_init_variant_array(
    state: *mut RawExecState,
    run: *mut JitRun,
    source: *const JitVariantOperandDesc,
    iter_area: u32,
    iter_index: u32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || source.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes one descriptor to
        // a stack slot that stays live for this helper call.
        let source = unsafe { *source };
        // SAFETY: null was rejected and the source is cloned before mutating iterator state.
        let run_ref = unsafe { &*run };
        let Some(source) = variant_operand_value_from_compiled_desc!(run_ref, source) else {
            return ST_FAULT;
        };
        let Some(array) = source.as_safearray() else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_type_mismatch(state) };
        };
        let elements = array.variant_elements().unwrap_or_default();
        let Some(iter_alias) = current_frame_slot(run_ref, iter_area, iter_index)
            .and_then(|alias| resolve_slot_alias(run_ref, alias))
        else {
            return ST_FAULT;
        };
        // SAFETY: null was rejected and immutable borrows ended before the map write.
        let run = unsafe { &mut *run };
        run.for_each.insert(
            iter_alias,
            JitForEachState {
                elements,
                position: 0,
            },
        );
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_for_each_next_variant_array(
    run: *mut JitRun,
    iter_area: u32,
    iter_index: u32,
    item_area: u32,
    item_index: u32,
    has_area: u32,
    has_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and this helper owns mutable access for the call.
        let run = unsafe { &mut *run };
        let Some(iter_alias) = current_frame_slot(run, iter_area, iter_index)
            .and_then(|alias| resolve_slot_alias(run, alias))
        else {
            return ST_FAULT;
        };
        let next = run.for_each.get_mut(&iter_alias).and_then(|state| {
            let value = state.elements.get(state.position).cloned();
            if value.is_some() {
                state.position += 1;
            }
            value
        });
        match next {
            Some(value) => {
                let Some(item_slot) = slot_mut(run, item_area, item_index) else {
                    return ST_FAULT;
                };
                *item_slot = value;
                let Some(has_slot) = slot_mut(run, has_area, has_index) else {
                    return ST_FAULT;
                };
                *has_slot = Variant::from_bool(true);
            }
            None => {
                let Some(has_slot) = slot_mut(run, has_area, has_index) else {
                    return ST_FAULT;
                };
                *has_slot = Variant::from_bool(false);
            }
        }
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_gosub_push(run: *mut JitRun, ret: i32) -> i32 {
    status_guard(|| {
        if run.is_null() || ret < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and this helper owns mutable access for the call.
        let run = unsafe { &mut *run };
        let Some(frame) = run.frames.last_mut() else {
            return ST_FAULT;
        };
        frame.gosub_stack.push(ret as u32);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_gosub_pop(
    state: *mut RawExecState,
    run: *mut JitRun,
    out_block: *mut u32,
) -> i32 {
    status_guard(|| {
        if state.is_null() || run.is_null() || out_block.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and this helper owns mutable access for the call.
        let run = unsafe { &mut *run };
        let Some(frame) = run.frames.last_mut() else {
            return ST_FAULT;
        };
        match frame.gosub_stack.pop() {
            Some(block) => {
                // SAFETY: null was rejected and the ABI requires writable storage.
                unsafe {
                    *out_block = block;
                }
                ST_OK
            }
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            None => unsafe { rt_raise_runtime_error_number(state, 3) },
        }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_unbox_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    target: i32,
    checked: i32,
    operand: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operand.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes one descriptor to
        // a stack slot that stays live for this helper call.
        let operand = unsafe { *operand };
        // SAFETY: null was rejected and the operand is cloned before destination write.
        let run_ref = unsafe { &*run };
        let Some(src) = variant_operand_value_from_compiled_desc!(run_ref, operand) else {
            return ST_FAULT;
        };
        if checked != 0 && !variant_matches_unbox_target(&src, target) {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_type_mismatch(state) };
        }
        // SAFETY: null was rejected and the source clone no longer borrows from `run`.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        *slot = src;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_arith_v_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    op: u32,
    mode: u32,
    operands: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operands.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes two descriptors to
        // a stack slot that stays live for this helper call.
        let operands = unsafe { std::slice::from_raw_parts(operands, 2) };
        // SAFETY: null was rejected and the compiled call gives shared access while
        // operand values are cloned out before the destination slot is borrowed mutably.
        let run_ref = unsafe { &*run };
        let Some(lhs) = variant_operand_value_from_compiled_desc!(run_ref, operands[0]) else {
            return ST_FAULT;
        };
        let Some(rhs) = variant_operand_value_from_compiled_desc!(run_ref, operands[1]) else {
            return ST_FAULT;
        };
        // SAFETY: null was rejected and operand clones no longer borrow from `run`.
        let run = unsafe { &mut *run };
        let Some(out) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        // SAFETY: the enclosing JIT boundary validated the live state and all Variant input/output pointers are initialized, live, and nonaliasing.
        unsafe { rt_arith_v(state, op, mode, &lhs, &rhs, out) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_concat_v_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    operands: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operands.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes two descriptors to
        // a stack slot that stays live for this helper call.
        let operands = unsafe { std::slice::from_raw_parts(operands, 2) };
        // SAFETY: null was rejected and operand values are cloned before destination write.
        let run_ref = unsafe { &*run };
        let Some(lhs) = variant_operand_value_from_compiled_desc!(run_ref, operands[0]) else {
            return ST_FAULT;
        };
        let Some(rhs) = variant_operand_value_from_compiled_desc!(run_ref, operands[1]) else {
            return ST_FAULT;
        };
        let out = match arith::concat(&lhs, &rhs) {
            Ok(value) => value,
            Err(err) => {
                // SAFETY: JIT helpers receive a live ExecState raw pointer during run.
                let Some(exec) = (unsafe { jit_exec_state_mut(state) }) else {
                    return ST_FAULT;
                };
                exec.err_engine.raise(Fault::from_arith(err), "OxVba");
                return ST_FAULT;
            }
        };
        // SAFETY: null was rejected and operand clones no longer borrow from `run`.
        let run = unsafe { &mut *run };
        let Some(slot) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        *slot = out;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_neg_v_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    mode: u32,
    operands: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operands.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes one descriptor to
        // a stack slot that stays live for this helper call.
        let operands = unsafe { std::slice::from_raw_parts(operands, 1) };
        // SAFETY: null was rejected and the source value is cloned before the
        // destination slot is borrowed mutably.
        let run_ref = unsafe { &*run };
        let Some(src) = variant_operand_value_from_compiled_desc!(run_ref, operands[0]) else {
            return ST_FAULT;
        };
        // SAFETY: null was rejected and the source clone no longer borrows from `run`.
        let run = unsafe { &mut *run };
        let Some(out) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        // SAFETY: the enclosing JIT boundary validated the live state and all Variant input/output pointers are initialized, live, and nonaliasing.
        unsafe { rt_neg_v(state, mode, &src, out) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_compare_v_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    op: u32,
    mode: u32,
    operands: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operands.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes two descriptors to
        // a stack slot that stays live for this helper call.
        let operands = unsafe { std::slice::from_raw_parts(operands, 2) };
        // SAFETY: null was rejected and operand values are cloned before `run` is
        // borrowed mutably for destination storage.
        let run_ref = unsafe { &*run };
        let Some(lhs) = variant_operand_value_from_compiled_desc!(run_ref, operands[0]) else {
            return ST_FAULT;
        };
        let Some(rhs) = variant_operand_value_from_compiled_desc!(run_ref, operands[1]) else {
            return ST_FAULT;
        };
        // SAFETY: null was rejected and operand clones no longer borrow from `run`.
        let run = unsafe { &mut *run };
        let Some(out) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        // SAFETY: the enclosing JIT boundary validated the live state and all Variant input/output pointers are initialized, live, and nonaliasing.
        unsafe { rt_compare_v(state, op, mode, &lhs, &rhs, out) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_compare_object_is_to_bool_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    operands: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operands.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes two descriptors to
        // a stack slot that stays live for this helper call.
        let operands = unsafe { std::slice::from_raw_parts(operands, 2) };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans possible As-New initializer callbacks.
        let lhs = match unsafe { variant_operand_value_with_as_new(run, state, operands[0]) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let rhs = match unsafe { variant_operand_value_with_as_new(run, state, operands[1]) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let lhs_id = match unsafe { object_identity_for_is(state, &lhs) } {
            Ok(id) => id,
            Err(status) => return status,
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let rhs_id = match unsafe { object_identity_for_is(state, &rhs) } {
            Ok(id) => id,
            Err(status) => return status,
        };
        // SAFETY: null run was rejected and the compiled destination identifies a
        // live Boolean carrier slot owned by this synchronous helper call.
        unsafe {
            rt_jit_store_bool(
                run,
                dst_area,
                dst_index,
                if lhs_id == rhs_id { 1 } else { 0 },
            )
        }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_type_of_is_to_bool_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    operand: *const JitVariantOperandDesc,
    type_name_ptr: *const u8,
    type_name_len: i32,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || operand.is_null()
            || type_name_ptr.is_null()
            || type_name_len < 0
        {
            return ST_FAULT;
        }
        // SAFETY: pointer/length come from compiled constants for a live type name.
        let type_name = match std::str::from_utf8(unsafe {
            std::slice::from_raw_parts(type_name_ptr, type_name_len as usize)
        }) {
            Ok(name) => name,
            Err(_) => return ST_FAULT,
        };
        // SAFETY: the compiled caller provides one live descriptor.
        let operand = unsafe { *operand };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // no typed run borrow spans a possible As-New initializer callback.
        let value = match unsafe { variant_operand_value_with_as_new(run, state, operand) } {
            Ok(value) => value,
            Err(status) => return status,
        };
        let matches = if value.as_object_ref().is_none()
            && matches!(
                value.vtype(),
                VarType::Object | VarType::Empty | VarType::Null
            ) {
            false
        } else {
            // SAFETY: the current compiled-run boundary owns the live unique state handle;
            // typed references and owned values remain live and nonaliasing for this call.
            let object = match unsafe { variant_to_project_object_for_jit(state, &value) } {
                Ok(object) => object,
                Err(status) => return status,
            };
            if !object.is_project_instance() {
                return ST_FAULT;
            }
            let descriptor = object.class_descriptor();
            let bare = type_name.rsplit('.').next().unwrap_or(type_name);
            descriptor.name.eq_ignore_ascii_case(bare)
                || descriptor
                    .implements
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(bare))
        };
        // SAFETY: null run was rejected and the compiled destination identifies a
        // live Boolean carrier slot owned by this synchronous helper call.
        unsafe { rt_jit_store_bool(run, dst_area, dst_index, if matches { 1 } else { 0 }) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_logical_v_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    op: u32,
    operands: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operands.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes two descriptors to
        // a stack slot that stays live for this helper call.
        let operands = unsafe { std::slice::from_raw_parts(operands, 2) };
        // SAFETY: null was rejected and operand values are cloned before `run` is
        // borrowed mutably for destination storage.
        let run_ref = unsafe { &*run };
        let Some(lhs) = variant_operand_value_from_compiled_desc!(run_ref, operands[0]) else {
            return ST_FAULT;
        };
        let Some(rhs) = variant_operand_value_from_compiled_desc!(run_ref, operands[1]) else {
            return ST_FAULT;
        };
        // SAFETY: null was rejected and operand clones no longer borrow from `run`.
        let run = unsafe { &mut *run };
        let Some(out) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        // SAFETY: the enclosing JIT boundary validated the live state and all Variant input/output pointers are initialized, live, and nonaliasing.
        unsafe { rt_logical_v(state, op, &lhs, &rhs, out) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_not_v_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    operand: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operand.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes one descriptor to
        // a stack slot that stays live for this helper call.
        let operand = unsafe { *operand };
        // SAFETY: null was rejected and the operand is cloned before destination write.
        let run_ref = unsafe { &*run };
        let Some(src) = variant_operand_value_from_compiled_desc!(run_ref, operand) else {
            return ST_FAULT;
        };
        // SAFETY: null was rejected and operand clone no longer borrows from `run`.
        let run = unsafe { &mut *run };
        let Some(out) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        // SAFETY: the enclosing JIT boundary validated the live state and all Variant input/output pointers are initialized, live, and nonaliasing.
        unsafe { rt_not_v(state, &src, out) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_truthy_v_to_bool_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    operand: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operand.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes one descriptor to
        // a stack slot that stays live for this helper call.
        let operand = unsafe { *operand };
        // SAFETY: null was rejected and the operand is cloned before destination write.
        let run_ref = unsafe { &*run };
        let Some(src) = variant_operand_value_from_compiled_desc!(run_ref, operand) else {
            return ST_FAULT;
        };
        let mut truthy = 0;
        // SAFETY: the enclosing JIT boundary validated the live state and all Variant input/output pointers are initialized, live, and nonaliasing.
        let status = unsafe { rt_truthy_v(state, &src, &mut truthy) };
        if status != ST_OK {
            return status;
        }
        // SAFETY: null was rejected and operand clone no longer borrows from `run`.
        let run = unsafe { &mut *run };
        let Some(out) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        *out = Variant::from_bool(truthy != 0);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_variant_changed_to_bool_slot(
    run: *mut JitRun,
    operands: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || operands.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes two descriptors to
        // a stack slot that stays live for this helper call.
        let operands = unsafe { std::slice::from_raw_parts(operands, 2) };
        // SAFETY: null was rejected and operand values are cloned before destination write.
        let run_ref = unsafe { &*run };
        let Some(current) = variant_operand_value_from_compiled_desc!(run_ref, operands[0]) else {
            return ST_FAULT;
        };
        let Some(original) = variant_operand_value_from_compiled_desc!(run_ref, operands[1]) else {
            return ST_FAULT;
        };
        let changed = variant_changed(&current, &original);
        // SAFETY: null was rejected and operand clones no longer borrow from `run`.
        unsafe { rt_jit_store_bool(run, dst_area, dst_index, if changed { 1 } else { 0 }) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_coerce_numeric_v_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    target: u32,
    operand: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operand.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes one descriptor to
        // a stack slot that stays live for this helper call.
        let operand = unsafe { *operand };
        // SAFETY: null was rejected and the operand is cloned before destination write.
        let run_ref = unsafe { &*run };
        let Some(src) = variant_operand_value_from_compiled_desc!(run_ref, operand) else {
            return ST_FAULT;
        };
        // SAFETY: null was rejected and operand clone no longer borrows from `run`.
        let run = unsafe { &mut *run };
        let Some(out) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        // SAFETY: the enclosing JIT boundary validated the live state and all Variant input/output pointers are initialized, live, and nonaliasing.
        unsafe { rt_coerce_numeric_v(state, target, &src, out) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_coerce_string_v_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    operand: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operand.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes one descriptor to
        // a stack slot that stays live for this helper call.
        let operand = unsafe { *operand };
        // SAFETY: null was rejected and the operand is cloned before destination write.
        let run_ref = unsafe { &*run };
        let Some(src) = variant_operand_value_from_compiled_desc!(run_ref, operand) else {
            return ST_FAULT;
        };
        let mut coerced = Variant::empty();
        // SAFETY: the enclosing JIT boundary validated the live state and all Variant input/output pointers are initialized, live, and nonaliasing.
        let status = unsafe { rt_coerce_string_v(state, &src, &mut coerced) };
        if status != ST_OK {
            return status;
        }
        // SAFETY: null was rejected and operand clone no longer borrows from `run`.
        let run = unsafe { &mut *run };
        let Some(out) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        *out = coerced;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_coerce_fixed_string_v_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    len: u32,
    operand: *const JitVariantOperandDesc,
    dst_area: u32,
    dst_index: u32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || operand.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled caller writes one descriptor to
        // a stack slot that stays live for this helper call.
        let operand = unsafe { *operand };
        // SAFETY: null was rejected and the operand is cloned before destination write.
        let run_ref = unsafe { &*run };
        let Some(src) = variant_operand_value_from_compiled_desc!(run_ref, operand) else {
            return ST_FAULT;
        };
        let mut coerced = Variant::empty();
        // SAFETY: the enclosing JIT boundary validated the live state and all Variant input/output pointers are initialized, live, and nonaliasing.
        let status = unsafe { rt_coerce_fixed_string_v(state, len, &src, &mut coerced) };
        if status != ST_OK {
            return status;
        }
        // SAFETY: null was rejected and operand clone no longer borrows from `run`.
        let run = unsafe { &mut *run };
        let Some(out) = slot_mut(run, dst_area, dst_index) else {
            return ST_FAULT;
        };
        *out = coerced;
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_direct_enter_noarg_sub(
    run: *mut JitRun,
    state: *mut RawExecState,
    proc: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || proc < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some((program_index, image)) = current_program_image(run) else {
            return ST_FAULT;
        };
        if image.program.is_null() || image.functions.is_null() {
            return ST_FAULT;
        }
        // SAFETY: `program` is installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let proc = proc as usize;
        if proc >= image.function_count {
            return ST_FAULT;
        }
        let Some(func) = program.funcs.get(proc) else {
            return ST_FAULT;
        };
        if func.param_count != 0 || func.return_local.is_some() {
            return ST_FAULT;
        }
        if run.frames.len() >= MAX_JIT_FRAMES {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_out_of_stack(state) };
        }

        let Ok(mut frame) = new_jit_frame(program, program_index, func) else {
            return ST_FAULT;
        };
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let enter_status = unsafe { rt_err_enter_activation(state, &mut frame.saved_err) };
        if enter_status != ST_OK {
            return enter_status;
        }
        run.frames.push(frame);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_direct_exit_noarg_sub(
    run: *mut JitRun,
    state: *mut RawExecState,
    status: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some(frame) = run.frames.pop() else {
            return ST_FAULT;
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let cleanup_status = unsafe { after_jit_frame_pop(run, state, &frame) };
        if cleanup_status != ST_OK {
            return cleanup_status;
        }
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let restore_status = unsafe { rt_err_restore_activation(state, &frame.saved_err) };
        if restore_status != ST_OK {
            restore_status
        } else {
            status
        }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_direct_enter_noarg_func(
    run: *mut JitRun,
    state: *mut RawExecState,
    proc: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || proc < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some((program_index, image)) = current_program_image(run) else {
            return ST_FAULT;
        };
        if image.program.is_null() || image.functions.is_null() {
            return ST_FAULT;
        }
        // SAFETY: `program` is installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let proc = proc as usize;
        if proc >= image.function_count {
            return ST_FAULT;
        }
        let Some(func) = program.funcs.get(proc) else {
            return ST_FAULT;
        };
        if func.param_count != 0 || func.return_local.is_none() {
            return ST_FAULT;
        }
        if run.frames.len() >= MAX_JIT_FRAMES {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_out_of_stack(state) };
        }

        let Ok(mut frame) = new_jit_frame(program, program_index, func) else {
            return ST_FAULT;
        };
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let enter_status = unsafe { rt_err_enter_activation(state, &mut frame.saved_err) };
        if enter_status != ST_OK {
            return enter_status;
        }
        run.frames.push(frame);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_direct_exit_noarg_func(
    run: *mut JitRun,
    state: *mut RawExecState,
    proc: i32,
    status: i32,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || proc < 0 || dst_area < 0 || dst_index < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some((_program_index, image)) = current_program_image(run) else {
            return ST_FAULT;
        };
        if image.program.is_null() {
            return ST_FAULT;
        }
        // SAFETY: `program` is installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let proc = proc as usize;
        let Some(func) = program.funcs.get(proc) else {
            return ST_FAULT;
        };
        let return_value = if status == ST_OK {
            let Some(ret) = func.return_local else {
                return ST_FAULT;
            };
            let Some(ret_ty) = func.locals.get(ret.0).map(|local| &local.ty) else {
                return ST_FAULT;
            };
            if !is_jit_static_call_ty(ret_ty) {
                return ST_FAULT;
            }
            let Some(value) = run
                .frames
                .last()
                .and_then(|frame| frame.locals.get(ret.0))
                .and_then(|value| call_return_variant(ret_ty, value))
            else {
                return ST_FAULT;
            };
            Some(value)
        } else {
            None
        };
        let Some(frame) = run.frames.pop() else {
            return ST_FAULT;
        };
        // SAFETY: the current compiled-run boundary owns the live unique state handle;
        // typed references and owned values remain live and nonaliasing for this call.
        let cleanup_status = unsafe { after_jit_frame_pop(run, state, &frame) };
        if cleanup_status != ST_OK {
            return cleanup_status;
        }
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let restore_status = unsafe { rt_err_restore_activation(state, &frame.saved_err) };
        if restore_status != ST_OK {
            return restore_status;
        }
        if status == ST_OK {
            let Some(slot) = slot_mut(run, dst_area as u32, dst_index as u32) else {
                return ST_FAULT;
            };
            let Some(value) = return_value else {
                return ST_FAULT;
            };
            *slot = value;
        }
        status
    })
}

pub(crate) unsafe extern "C" fn rt_jit_direct_enter_one_i32_sub(
    run: *mut JitRun,
    state: *mut RawExecState,
    proc: i32,
    arg0: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || proc < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some((program_index, image)) = current_program_image(run) else {
            return ST_FAULT;
        };
        if image.program.is_null() || image.functions.is_null() {
            return ST_FAULT;
        }
        // SAFETY: `program` is installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let proc = proc as usize;
        if proc >= image.function_count {
            return ST_FAULT;
        }
        let Some(func) = program.funcs.get(proc) else {
            return ST_FAULT;
        };
        let Some(param) = func.locals.first() else {
            return ST_FAULT;
        };
        let Some(param_info) = param.param.as_ref() else {
            return ST_FAULT;
        };
        if func.param_count != 1
            || func.return_local.is_some()
            || !matches!(param.ty, OxTy::Long)
            || param_info.by_ref
            || param_info.variadic
        {
            return ST_FAULT;
        }
        if run.frames.len() >= MAX_JIT_FRAMES {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_out_of_stack(state) };
        }

        let Ok(mut frame) = new_jit_frame(program, program_index, func) else {
            return ST_FAULT;
        };
        frame.locals[0] = Variant::from_i32(arg0);
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let enter_status = unsafe { rt_err_enter_activation(state, &mut frame.saved_err) };
        if enter_status != ST_OK {
            return enter_status;
        }
        run.frames.push(frame);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_direct_enter_one_i32_func(
    run: *mut JitRun,
    state: *mut RawExecState,
    proc: i32,
    arg0: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || proc < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some((program_index, image)) = current_program_image(run) else {
            return ST_FAULT;
        };
        if image.program.is_null() || image.functions.is_null() {
            return ST_FAULT;
        }
        // SAFETY: `program` is installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let proc = proc as usize;
        if proc >= image.function_count {
            return ST_FAULT;
        }
        let Some(func) = program.funcs.get(proc) else {
            return ST_FAULT;
        };
        let Some(param) = func.locals.first() else {
            return ST_FAULT;
        };
        let Some(param_info) = param.param.as_ref() else {
            return ST_FAULT;
        };
        let Some(ret) = func.return_local else {
            return ST_FAULT;
        };
        let Some(ret_ty) = func.locals.get(ret.0).map(|local| &local.ty) else {
            return ST_FAULT;
        };
        if func.param_count != 1
            || !is_m4_4_call_scalar_ty(ret_ty)
            || !matches!(param.ty, OxTy::Long)
            || param_info.by_ref
            || param_info.variadic
        {
            return ST_FAULT;
        }
        if run.frames.len() >= MAX_JIT_FRAMES {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_out_of_stack(state) };
        }

        let Ok(mut frame) = new_jit_frame(program, program_index, func) else {
            return ST_FAULT;
        };
        frame.locals[0] = Variant::from_i32(arg0);
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let enter_status = unsafe { rt_err_enter_activation(state, &mut frame.saved_err) };
        if enter_status != ST_OK {
            return enter_status;
        }
        run.frames.push(frame);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_direct_enter_one_i32_byref_sub(
    run: *mut JitRun,
    state: *mut RawExecState,
    proc: i32,
    arg_area: i32,
    arg_index: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || proc < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some((program_index, image)) = current_program_image(run) else {
            return ST_FAULT;
        };
        if image.program.is_null() || image.functions.is_null() {
            return ST_FAULT;
        }
        // SAFETY: `program` is installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let proc = proc as usize;
        if proc >= image.function_count {
            return ST_FAULT;
        }
        let Some(func) = program.funcs.get(proc) else {
            return ST_FAULT;
        };
        let Some(param) = func.locals.first() else {
            return ST_FAULT;
        };
        let Some(param_info) = param.param.as_ref() else {
            return ST_FAULT;
        };
        if func.param_count != 1
            || func.return_local.is_some()
            || !matches!(param.ty, OxTy::Long)
            || !param_info.by_ref
            || param_info.variadic
        {
            return ST_FAULT;
        }
        if run.frames.len() >= MAX_JIT_FRAMES {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_out_of_stack(state) };
        }
        let Some(alias) = direct_call_arg_alias(run, arg_area, arg_index) else {
            return ST_FAULT;
        };
        if slot_alias_ref(run, alias).is_none() {
            return ST_FAULT;
        }

        let Ok(mut frame) = new_jit_frame(program, program_index, func) else {
            return ST_FAULT;
        };
        frame.aliases[0] = Some(alias);
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let enter_status = unsafe { rt_err_enter_activation(state, &mut frame.saved_err) };
        if enter_status != ST_OK {
            return enter_status;
        }
        run.frames.push(frame);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_direct_enter_one_i32_byref_func(
    run: *mut JitRun,
    state: *mut RawExecState,
    proc: i32,
    arg_area: i32,
    arg_index: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || proc < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some((program_index, image)) = current_program_image(run) else {
            return ST_FAULT;
        };
        if image.program.is_null() || image.functions.is_null() {
            return ST_FAULT;
        }
        // SAFETY: `program` is installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let proc = proc as usize;
        if proc >= image.function_count {
            return ST_FAULT;
        }
        let Some(func) = program.funcs.get(proc) else {
            return ST_FAULT;
        };
        let Some(param) = func.locals.first() else {
            return ST_FAULT;
        };
        let Some(param_info) = param.param.as_ref() else {
            return ST_FAULT;
        };
        let Some(ret) = func.return_local else {
            return ST_FAULT;
        };
        let Some(ret_ty) = func.locals.get(ret.0).map(|local| &local.ty) else {
            return ST_FAULT;
        };
        if func.param_count != 1
            || !is_m4_4_call_scalar_ty(ret_ty)
            || !matches!(param.ty, OxTy::Long)
            || !param_info.by_ref
            || param_info.variadic
        {
            return ST_FAULT;
        }
        if run.frames.len() >= MAX_JIT_FRAMES {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_out_of_stack(state) };
        }
        let Some(alias) = direct_call_arg_alias(run, arg_area, arg_index) else {
            return ST_FAULT;
        };
        if slot_alias_ref(run, alias).is_none() {
            return ST_FAULT;
        }

        let Ok(mut frame) = new_jit_frame(program, program_index, func) else {
            return ST_FAULT;
        };
        frame.aliases[0] = Some(alias);
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let enter_status = unsafe { rt_err_enter_activation(state, &mut frame.saved_err) };
        if enter_status != ST_OK {
            return enter_status;
        }
        run.frames.push(frame);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_direct_enter_two_i32_sub(
    run: *mut JitRun,
    state: *mut RawExecState,
    proc: i32,
    arg0: i32,
    arg1: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || proc < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some((program_index, image)) = current_program_image(run) else {
            return ST_FAULT;
        };
        if image.program.is_null() || image.functions.is_null() {
            return ST_FAULT;
        }
        // SAFETY: `program` is installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let proc = proc as usize;
        if proc >= image.function_count {
            return ST_FAULT;
        }
        let Some(func) = program.funcs.get(proc) else {
            return ST_FAULT;
        };
        if func.param_count != 2 || func.return_local.is_some() {
            return ST_FAULT;
        }
        for index in 0..2 {
            let Some(param) = func.locals.get(index) else {
                return ST_FAULT;
            };
            let Some(param_info) = param.param.as_ref() else {
                return ST_FAULT;
            };
            if !matches!(param.ty, OxTy::Long) || param_info.by_ref || param_info.variadic {
                return ST_FAULT;
            }
        }
        if run.frames.len() >= MAX_JIT_FRAMES {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_out_of_stack(state) };
        }

        let Ok(mut frame) = new_jit_frame(program, program_index, func) else {
            return ST_FAULT;
        };
        frame.locals[0] = Variant::from_i32(arg0);
        frame.locals[1] = Variant::from_i32(arg1);
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let enter_status = unsafe { rt_err_enter_activation(state, &mut frame.saved_err) };
        if enter_status != ST_OK {
            return enter_status;
        }
        run.frames.push(frame);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_direct_enter_two_i32_func(
    run: *mut JitRun,
    state: *mut RawExecState,
    proc: i32,
    arg0: i32,
    arg1: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || proc < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some((program_index, image)) = current_program_image(run) else {
            return ST_FAULT;
        };
        if image.program.is_null() || image.functions.is_null() {
            return ST_FAULT;
        }
        // SAFETY: `program` is installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let proc = proc as usize;
        if proc >= image.function_count {
            return ST_FAULT;
        }
        let Some(func) = program.funcs.get(proc) else {
            return ST_FAULT;
        };
        let Some(ret) = func.return_local else {
            return ST_FAULT;
        };
        let Some(ret_ty) = func.locals.get(ret.0).map(|local| &local.ty) else {
            return ST_FAULT;
        };
        if func.param_count != 2 || !is_m4_4_call_scalar_ty(ret_ty) {
            return ST_FAULT;
        }
        for index in 0..2 {
            let Some(param) = func.locals.get(index) else {
                return ST_FAULT;
            };
            let Some(param_info) = param.param.as_ref() else {
                return ST_FAULT;
            };
            if !matches!(param.ty, OxTy::Long) || param_info.by_ref || param_info.variadic {
                return ST_FAULT;
            }
        }
        if run.frames.len() >= MAX_JIT_FRAMES {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_out_of_stack(state) };
        }

        let Ok(mut frame) = new_jit_frame(program, program_index, func) else {
            return ST_FAULT;
        };
        frame.locals[0] = Variant::from_i32(arg0);
        frame.locals[1] = Variant::from_i32(arg1);
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let enter_status = unsafe { rt_err_enter_activation(state, &mut frame.saved_err) };
        if enter_status != ST_OK {
            return enter_status;
        }
        run.frames.push(frame);
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_direct_enter_proc_i32(
    run: *mut JitRun,
    state: *mut RawExecState,
    proc: i32,
    argc: i32,
    args: *const JitCallArgDesc,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || proc < 0 || argc < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and compiled code gives unique run ownership.
        let run = unsafe { &mut *run };
        let Some((program_index, image)) = current_program_image(run) else {
            return ST_FAULT;
        };
        if image.program.is_null() || image.functions.is_null() {
            return ST_FAULT;
        }
        // SAFETY: `program` is installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let proc = proc as usize;
        let Some(func) = program.funcs.get(proc) else {
            return ST_FAULT;
        };
        let argc = argc as usize;
        if proc >= image.function_count || argc != func.param_count {
            return ST_FAULT;
        }
        let args = if argc == 0 {
            &[]
        } else if args.is_null() {
            return ST_FAULT;
        } else {
            // SAFETY: the compiled caller writes exactly `argc` descriptors to a stack
            // slot that stays live for this helper call.
            unsafe { std::slice::from_raw_parts(args, argc) }
        };

        let Some(caller_frame) = run.frames.len().checked_sub(1) else {
            return ST_FAULT;
        };
        if run.frames.len() >= MAX_JIT_FRAMES {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_out_of_stack(state) };
        }

        let Ok(mut frame) = new_jit_frame(program, program_index, func) else {
            return ST_FAULT;
        };
        let mut pending_param_array_aliases = Vec::new();
        for (index, arg) in args.iter().copied().enumerate() {
            let Some(param) = func.locals.get(index) else {
                return ST_FAULT;
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
                    frame.locals[index] = value;
                }
                JIT_CALL_ARG_BYVAL_VARIANT
                    if !param_info.by_ref && matches!(param.ty, OxTy::Long) =>
                {
                    let Some(value) = call_arg_long_i32_value(run, arg) else {
                        return ST_FAULT;
                    };
                    frame.locals[index] = Variant::from_i32(value);
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
                    // SAFETY: the current compiled-run boundary owns the live unique state handle;
                    // typed references and owned values remain live and nonaliasing for this call.
                    frame.locals[index] =
                        match unsafe { coerce_call_arg_for_param(state, &param.ty, &value) } {
                            Ok(value) => value,
                            Err(status) => return status,
                        };
                    if let Some(aliases) = param_array_aliases {
                        pending_param_array_aliases.push((index, aliases));
                    }
                }
                JIT_CALL_ARG_OMITTED if !param_info.by_ref && matches!(param.ty, OxTy::Variant) => {
                    let Some(value) = call_arg_variant_value(run, arg) else {
                        return ST_FAULT;
                    };
                    frame.locals[index] = value;
                }
                JIT_CALL_ARG_BYREF_COPY if param_info.by_ref => {
                    if is_jit_variant_carrier_ty(&param.ty) {
                        let Some(value) = call_arg_variant_value(run, arg) else {
                            return ST_FAULT;
                        };
                        frame.locals[index] =
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
                        frame.locals[index] = value;
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
                    frame.aliases[index] = Some(alias);
                }
                _ => return ST_FAULT,
            }
        }

        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let enter_status = unsafe { rt_err_enter_activation(state, &mut frame.saved_err) };
        if enter_status != ST_OK {
            return enter_status;
        }
        run.frames.push(frame);
        let callee_frame = run.frames.len() - 1;
        for (index, aliases) in pending_param_array_aliases {
            run.param_array_aliases.insert(
                SlotAlias {
                    frame: Some(callee_frame),
                    area: AREA_LOCAL,
                    index: index as u32,
                },
                aliases,
            );
        }
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_call_extern_proc_i32(
    run: *mut JitRun,
    state: *mut RawExecState,
    program_index: i32,
    proc: i32,
    argc: i32,
    args: *const JitCallArgDesc,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null()
            || state.is_null()
            || program_index < 0
            || proc < 0
            || argc < 0
            || dst_area < -1
            || dst_index < -1
        {
            return ST_FAULT;
        }
        let dst = match (dst_area, dst_index) {
            (-1, -1) => None,
            (area, index) if area >= 0 && index >= 0 => Some((area as u32, index as u32)),
            _ => return ST_FAULT,
        };
        let program_index = program_index as usize;
        let (image, proc, frame, return_local, pending_param_array_aliases) = {
            // SAFETY: null was rejected; this preparation borrow ends before compiled entry.
            let run = unsafe { &mut *run };
            let Some(image) = program_image(run, program_index) else {
                return ST_FAULT;
            };
            if image.program.is_null() || image.functions.is_null() {
                return ST_FAULT;
            }
            // SAFETY: `program` is installed from the owning CompiledImage for this run.
            let program = unsafe { &*image.program };
            let proc = proc as usize;
            let Some(func) = program.funcs.get(proc) else {
                return ST_FAULT;
            };
            let argc = argc as usize;
            if proc >= image.function_count || argc != func.param_count {
                return ST_FAULT;
            }
            let args = if argc == 0 {
                &[]
            } else if args.is_null() {
                return ST_FAULT;
            } else {
                // SAFETY: the compiled caller writes exactly `argc` descriptors to a stack slot
                // that stays live for this helper call.
                unsafe { std::slice::from_raw_parts(args, argc) }
            };

            let Some(caller_frame) = run.frames.len().checked_sub(1) else {
                return ST_FAULT;
            };
            if run.frames.len() >= MAX_JIT_FRAMES {
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                return unsafe { rt_raise_out_of_stack(state) };
            }
            let Ok(mut frame) = new_jit_frame(program, program_index, func) else {
                return ST_FAULT;
            };
            let return_local = if dst.is_some() {
                let Some(ret) = func.return_local else {
                    return ST_FAULT;
                };
                let Some(ret_ty) = func.locals.get(ret.0).map(|local| local.ty.clone()) else {
                    return ST_FAULT;
                };
                if !is_jit_static_call_ty(&ret_ty) {
                    return ST_FAULT;
                }
                Some((ret.0, ret_ty))
            } else {
                None
            };
            let mut pending_param_array_aliases = Vec::new();
            for (index, arg) in args.iter().copied().enumerate() {
                let Some(param) = func.locals.get(index) else {
                    return ST_FAULT;
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
                        frame.locals[index] = value;
                    }
                    JIT_CALL_ARG_BYVAL_VARIANT
                        if !param_info.by_ref && matches!(param.ty, OxTy::Long) =>
                    {
                        let Some(value) = call_arg_long_i32_value(run, arg) else {
                            return ST_FAULT;
                        };
                        frame.locals[index] = Variant::from_i32(value);
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
                        // SAFETY: the current compiled-run boundary owns the live unique state handle;
                        // typed references and owned values remain live and nonaliasing for this call.
                        frame.locals[index] =
                            match unsafe { coerce_call_arg_for_param(state, &param.ty, &value) } {
                                Ok(value) => value,
                                Err(status) => return status,
                            };
                        if let Some(aliases) = param_array_aliases {
                            pending_param_array_aliases.push((index, aliases));
                        }
                    }
                    JIT_CALL_ARG_OMITTED
                        if !param_info.by_ref && matches!(param.ty, OxTy::Variant) =>
                    {
                        let Some(value) = call_arg_variant_value(run, arg) else {
                            return ST_FAULT;
                        };
                        frame.locals[index] = value;
                    }
                    JIT_CALL_ARG_BYREF_COPY if param_info.by_ref => {
                        if is_jit_variant_carrier_ty(&param.ty) {
                            let Some(value) = call_arg_variant_value(run, arg) else {
                                return ST_FAULT;
                            };
                            frame.locals[index] =
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
                            frame.locals[index] = value;
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
                        frame.aliases[index] = Some(alias);
                    }
                    _ => return ST_FAULT,
                }
            }

            (
                image,
                proc,
                frame,
                return_local,
                pending_param_array_aliases,
            )
        };
        let mut saved_err = RtSavedErrState::default();
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let enter_status = unsafe { rt_err_enter_activation(state, &mut saved_err) };
        if enter_status != ST_OK {
            return enter_status;
        }
        {
            // SAFETY: error activation completed; this push borrow ends before entry.
            let run_ref = unsafe { &mut *run };
            run_ref.frames.push(frame);
            let callee_frame = run_ref.frames.len() - 1;
            for (index, aliases) in pending_param_array_aliases {
                run_ref.param_array_aliases.insert(
                    SlotAlias {
                        frame: Some(callee_frame),
                        area: AREA_LOCAL,
                        index: index as u32,
                    },
                    aliases,
                );
            }
        }
        // SAFETY: bounds and null checks above prove the function pointer exists.
        let entry = unsafe { *image.functions.add(proc) };
        // SAFETY: the function pointer uses the JIT entry ABI and raw `run`/`state`
        // remain live for the nested call.
        let status = unsafe { entry(run, state) };
        let (return_value, cleanup_status) = {
            // SAFETY: entry returned; this post-entry borrow is bounded.
            let run_ref = unsafe { &mut *run };
            let return_value = if status == ST_OK {
                return_local.as_ref().and_then(|(local, ty)| {
                    run_ref
                        .frames
                        .last()
                        .and_then(|frame| frame.locals.get(*local))
                        .and_then(|value| call_return_variant(ty, value))
                })
            } else {
                None
            };
            let Some(frame) = run_ref.frames.pop() else {
                // SAFETY: the activation was entered successfully and state remains live.
                let restore_status = unsafe { rt_err_restore_activation(state, &saved_err) };
                return if restore_status == ST_OK {
                    ST_FAULT
                } else {
                    restore_status
                };
            };
            // SAFETY: post-entry cleanup owns the bounded run borrow.
            let cleanup_status = unsafe { after_jit_frame_pop(run_ref, state, &frame) };
            (return_value, cleanup_status)
        };
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let restore_status = unsafe { rt_err_restore_activation(state, &saved_err) };
        if restore_status != ST_OK {
            return restore_status;
        }
        if cleanup_status != ST_OK {
            return cleanup_status;
        }
        if status == ST_OK
            && let Some((area, index)) = dst
        {
            let Some(value) = return_value else {
                return ST_FAULT;
            };
            // SAFETY: entry and activation restoration completed.
            let Some(slot) = slot_mut(unsafe { &mut *run }, area, index) else {
                return ST_FAULT;
            };
            *slot = value;
        }
        status
    })
}

pub(crate) unsafe extern "C" fn rt_jit_call_proc_i32(
    run: *mut JitRun,
    state: *mut RawExecState,
    proc: i32,
    argc: i32,
    args: *const JitCallArgDesc,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null()
            || state.is_null()
            || proc < 0
            || argc < 0
            || dst_area < -1
            || dst_index < -1
        {
            return ST_FAULT;
        }
        let dst = match (dst_area, dst_index) {
            (-1, -1) => None,
            (area, index) if area >= 0 && index >= 0 => Some((area as u32, index as u32)),
            _ => return ST_FAULT,
        };
        let (image, proc, frame, return_local, pending_param_array_aliases) = {
            // SAFETY: null was rejected; this preparation borrow ends before compiled entry.
            let run = unsafe { &mut *run };
            let Some((program_index, image)) = current_program_image(run) else {
                return ST_FAULT;
            };
            if image.program.is_null() || image.functions.is_null() {
                return ST_FAULT;
            }
            // SAFETY: `program` is installed from the owning CompiledImage for this run.
            let program = unsafe { &*image.program };
            let proc = proc as usize;
            let Some(func) = program.funcs.get(proc) else {
                return ST_FAULT;
            };
            let argc = argc as usize;
            if proc >= image.function_count || argc != func.param_count {
                return ST_FAULT;
            }
            let args = if argc == 0 {
                &[]
            } else if args.is_null() {
                return ST_FAULT;
            } else {
                // SAFETY: the compiled caller writes exactly `argc` descriptors to a stack slot
                // that stays live for this helper call.
                unsafe { std::slice::from_raw_parts(args, argc) }
            };

            let Some(caller_frame) = run.frames.len().checked_sub(1) else {
                return ST_FAULT;
            };
            if run.frames.len() >= MAX_JIT_FRAMES {
                // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
                return unsafe { rt_raise_out_of_stack(state) };
            }
            let Ok(mut frame) = new_jit_frame(program, program_index, func) else {
                return ST_FAULT;
            };
            let return_local = if dst.is_some() {
                let Some(ret) = func.return_local else {
                    return ST_FAULT;
                };
                let Some(ret_ty) = func.locals.get(ret.0).map(|local| local.ty.clone()) else {
                    return ST_FAULT;
                };
                if !is_jit_static_call_ty(&ret_ty) {
                    return ST_FAULT;
                }
                Some((ret.0, ret_ty))
            } else {
                None
            };
            let mut pending_param_array_aliases = Vec::new();
            for (index, arg) in args.iter().copied().enumerate() {
                let Some(param) = func.locals.get(index) else {
                    return ST_FAULT;
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
                        frame.locals[index] = value;
                    }
                    JIT_CALL_ARG_BYVAL_VARIANT
                        if !param_info.by_ref && matches!(param.ty, OxTy::Long) =>
                    {
                        let Some(value) = call_arg_long_i32_value(run, arg) else {
                            return ST_FAULT;
                        };
                        frame.locals[index] = Variant::from_i32(value);
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
                        // SAFETY: the current compiled-run boundary owns the live unique state handle;
                        // typed references and owned values remain live and nonaliasing for this call.
                        frame.locals[index] =
                            match unsafe { coerce_call_arg_for_param(state, &param.ty, &value) } {
                                Ok(value) => value,
                                Err(status) => return status,
                            };
                        if let Some(aliases) = param_array_aliases {
                            pending_param_array_aliases.push((index, aliases));
                        }
                    }
                    JIT_CALL_ARG_OMITTED
                        if !param_info.by_ref && matches!(param.ty, OxTy::Variant) =>
                    {
                        let Some(value) = call_arg_variant_value(run, arg) else {
                            return ST_FAULT;
                        };
                        frame.locals[index] = value;
                    }
                    JIT_CALL_ARG_BYREF_COPY if param_info.by_ref => {
                        if is_jit_variant_carrier_ty(&param.ty) {
                            let Some(value) = call_arg_variant_value(run, arg) else {
                                return ST_FAULT;
                            };
                            frame.locals[index] =
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
                            frame.locals[index] = value;
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
                        frame.aliases[index] = Some(alias);
                    }
                    _ => return ST_FAULT,
                }
            }

            (
                image,
                proc,
                frame,
                return_local,
                pending_param_array_aliases,
            )
        };
        let mut saved_err = RtSavedErrState::default();
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let enter_status = unsafe { rt_err_enter_activation(state, &mut saved_err) };
        if enter_status != ST_OK {
            return enter_status;
        }
        {
            // SAFETY: error activation completed; this push borrow ends before entry.
            let run_ref = unsafe { &mut *run };
            run_ref.frames.push(frame);
            let callee_frame = run_ref.frames.len() - 1;
            for (index, aliases) in pending_param_array_aliases {
                run_ref.param_array_aliases.insert(
                    SlotAlias {
                        frame: Some(callee_frame),
                        area: AREA_LOCAL,
                        index: index as u32,
                    },
                    aliases,
                );
            }
        }
        // SAFETY: bounds and null checks above prove the function pointer exists.
        let entry = unsafe { *image.functions.add(proc) };
        // SAFETY: the function pointer uses the JIT entry ABI and raw `run`/`state`
        // remain live for the nested call.
        let status = unsafe { entry(run, state) };
        let (return_value, cleanup_status) = {
            // SAFETY: entry returned; this post-entry borrow is bounded.
            let run_ref = unsafe { &mut *run };
            let return_value = if status == ST_OK {
                return_local.as_ref().and_then(|(local, ty)| {
                    run_ref
                        .frames
                        .last()
                        .and_then(|frame| frame.locals.get(*local))
                        .and_then(|value| call_return_variant(ty, value))
                })
            } else {
                None
            };
            let Some(frame) = run_ref.frames.pop() else {
                // SAFETY: the activation was entered successfully and state remains live.
                let restore_status = unsafe { rt_err_restore_activation(state, &saved_err) };
                return if restore_status == ST_OK {
                    ST_FAULT
                } else {
                    restore_status
                };
            };
            // SAFETY: post-entry cleanup owns the bounded run borrow.
            let cleanup_status = unsafe { after_jit_frame_pop(run_ref, state, &frame) };
            (return_value, cleanup_status)
        };
        // SAFETY: the enclosing JIT boundary validated the live state; saved-error storage is initialized and live for this call.
        let restore_status = unsafe { rt_err_restore_activation(state, &saved_err) };
        if restore_status != ST_OK {
            return restore_status;
        }
        if cleanup_status != ST_OK {
            return cleanup_status;
        }
        if status == ST_OK
            && let Some((area, index)) = dst
        {
            let Some(value) = return_value else {
                return ST_FAULT;
            };
            // SAFETY: entry and activation restoration completed.
            let Some(slot) = slot_mut(unsafe { &mut *run }, area, index) else {
                return ST_FAULT;
            };
            *slot = value;
        }
        status
    })
}

pub(crate) unsafe extern "C" fn rt_jit_expect_proc_ref_i32(
    run: *mut JitRun,
    state: *mut RawExecState,
    target_area: i32,
    target_index: i32,
    expected_proc: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null()
            || state.is_null()
            || target_area < 0
            || target_index < 0
            || expected_proc < 0
        {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and this helper only reads the target slot.
        let run_ref = unsafe { &*run };
        let resolved = slot_ref(run_ref, target_area as u32, target_index as u32)
            .and_then(Variant::as_proc_ref);
        let Some(proc) = resolved else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_invalid_proc_ref(state) };
        };
        let Some((_program_index, image)) = current_program_image(run_ref) else {
            return ST_FAULT;
        };
        if proc >= image.function_count || proc > i32::MAX as usize {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_invalid_proc_ref(state) };
        }
        if proc != expected_proc as usize {
            return ST_FAULT;
        }
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_call_proc_ref_i32(
    run: *mut JitRun,
    state: *mut RawExecState,
    target_area: i32,
    target_index: i32,
    expected_proc: i32,
    dynamic_ret_kind: i32,
    argc: i32,
    args: *const JitCallArgDesc,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if run.is_null() || state.is_null() || target_area < 0 || target_index < 0 {
            return ST_FAULT;
        }
        // SAFETY: null was rejected and the compiled call gives shared run access for
        // resolving the immutable ProcRef slot before the forwarded call mutates frames.
        let run_ref = unsafe { &*run };
        let resolved = slot_ref(run_ref, target_area as u32, target_index as u32)
            .and_then(Variant::as_proc_ref);
        let Some(proc) = resolved else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_invalid_proc_ref(state) };
        };
        let Some((_program_index, image)) = current_program_image(run_ref) else {
            return ST_FAULT;
        };
        if proc >= image.function_count || proc > i32::MAX as usize {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_invalid_proc_ref(state) };
        }
        let signature_proc = if expected_proc >= 0 {
            let expected = expected_proc as usize;
            if proc != expected {
                return ST_FAULT;
            }
            Some(expected)
        } else if expected_proc == -1 {
            None
        } else {
            let encoded = -(i64::from(expected_proc)) - 2;
            let Ok(signature_proc) = usize::try_from(encoded) else {
                return ST_FAULT;
            };
            Some(signature_proc)
        };
        if argc < 0 || image.program.is_null() {
            return ST_FAULT;
        }
        // SAFETY: `program` is installed from the owning CompiledImage for this run.
        let program = unsafe { &*image.program };
        let Some(func) = program.funcs.get(proc) else {
            // SAFETY: the enclosing JIT boundary validated `state` as the live, uniquely owned execution state.
            return unsafe { rt_raise_invalid_proc_ref(state) };
        };
        let argc_usize = argc as usize;
        if argc_usize != func.param_count {
            return ST_FAULT;
        }
        if argc_usize > 0 && args.is_null() {
            return ST_FAULT;
        }
        let arg_descs = if argc_usize == 0 {
            &[]
        } else {
            // SAFETY: the compiled caller writes exactly `argc_usize` descriptors to a
            // stack slot that stays live for this helper call.
            unsafe { std::slice::from_raw_parts(args, argc_usize) }
        };
        if let Some(signature_proc) = signature_proc {
            if !proc_ref_signatures_match(program, FuncId(signature_proc), FuncId(proc)) {
                return ST_FAULT;
            }
        } else {
            // SAFETY: the current compiled-run boundary owns the live unique state handle;
            // typed references and owned values remain live and nonaliasing for this call.
            let arg_shape =
                match unsafe { unknown_proc_ref_arg_shape(state, run_ref, func, arg_descs) } {
                    Ok(shape) => shape,
                    Err(status) => return status,
                };
            if matches!((dst_area, dst_index), (-1, -1)) {
                if dynamic_ret_kind != JIT_PROC_REF_RET_NONE
                    || !matches!(
                        arg_shape,
                        UnknownProcRefArgShape::LongOnly
                            | UnknownProcRefArgShape::StringByValOnly
                            | UnknownProcRefArgShape::StringCandidate
                    )
                {
                    return ST_FAULT;
                }
            } else {
                let Some(ret) = func.return_local else {
                    return ST_FAULT;
                };
                let Some(ret_ty) = func.locals.get(ret.0).map(|local| &local.ty) else {
                    return ST_FAULT;
                };
                let ret_matches = match dynamic_ret_kind {
                    JIT_PROC_REF_RET_LONG if arg_shape == UnknownProcRefArgShape::LongOnly => {
                        matches!(ret_ty, OxTy::Long)
                    }
                    JIT_PROC_REF_RET_STRING
                        if argc_usize == 0
                            || matches!(
                                arg_shape,
                                UnknownProcRefArgShape::StringByValOnly
                                    | UnknownProcRefArgShape::StringCandidate
                            ) =>
                    {
                        matches!(ret_ty, OxTy::Str)
                    }
                    JIT_PROC_REF_RET_VARIANT if argc_usize == 0 => {
                        is_m4_4_unknown_proc_ref_variant_return_ty(ret_ty)
                    }
                    JIT_PROC_REF_RET_VARIANT if arg_shape == UnknownProcRefArgShape::LongOnly => {
                        matches!(ret_ty, OxTy::Long | OxTy::Variant)
                    }
                    JIT_PROC_REF_RET_VARIANT
                        if matches!(
                            arg_shape,
                            UnknownProcRefArgShape::StringByValOnly
                                | UnknownProcRefArgShape::StringCandidate
                        ) =>
                    {
                        matches!(ret_ty, OxTy::Str | OxTy::Variant)
                    }
                    kind if argc_usize == 0 => unknown_proc_ref_exact_return_matches(kind, ret_ty),
                    _ => false,
                };
                if !ret_matches {
                    return ST_FAULT;
                }
            }
        }
        // SAFETY: forwarding the same validated run/state pointers received from compiled code.
        unsafe { rt_jit_call_proc_i32(run, state, proc as i32, argc, args, dst_area, dst_index) }
    })
}

pub(crate) unsafe extern "C" fn rt_jit_lib_invoke_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    native_id: u32,
    string_typed_alias: i32,
    operands: *const JitVariantOperandDesc,
    argc: i32,
    dst_area: i32,
    dst_index: i32,
) -> i32 {
    status_guard(|| {
        if state.is_null()
            || run.is_null()
            || argc < 0
            || dst_area < -1
            || dst_index < -1
            || (argc > 0 && operands.is_null())
        {
            return ST_FAULT;
        }
        let dst = match (dst_area, dst_index) {
            (-1, -1) => None,
            (area, index) if area >= 0 && index >= 0 => Some((area as u32, index as u32)),
            _ => return ST_FAULT,
        };
        let argc = argc as usize;
        let operands = if argc == 0 {
            &[]
        } else {
            // SAFETY: null was rejected and the compiled caller writes `argc` descriptors to
            // a stack slot that stays live for this helper call.
            unsafe { std::slice::from_raw_parts(operands, argc) }
        };
        let argv = {
            // SAFETY: this operand-read borrow is visibly bounded before library/host entry.
            let run_ref = unsafe { &*run };
            let mut argv = Vec::with_capacity(argc);
            for operand in operands {
                let Some(value) = variant_operand_value_from_compiled_desc!(run_ref, *operand)
                else {
                    return ST_FAULT;
                };
                argv.push(value);
            }
            argv
        };
        let mut out = Variant::empty();
        // SAFETY: null state was rejected; `argv` is one live allocation containing
        // exactly `argv.len()` initialized Variants, and `out` is unique initialized
        // Variant storage that does not alias the argument allocation.
        let status = unsafe {
            rt_lib_invoke_with_policy(
                state,
                native_id,
                argv.as_ptr(),
                argv.len(),
                string_typed_alias,
                &mut out,
            )
        };
        if status != ST_OK {
            return status;
        }
        if let Some((area, index)) = dst {
            // SAFETY: null was rejected and operand clones no longer borrow from `run`.
            let run = unsafe { &mut *run };
            let Some(slot) = slot_mut(run, area, index) else {
                return ST_FAULT;
            };
            *slot = out;
        }
        ST_OK
    })
}

pub(crate) unsafe extern "C" fn rt_jit_add_i32_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i32_to_slot(state, run, lhs, rhs, area, index, rt_add_i32) }
}

pub(crate) unsafe extern "C" fn rt_jit_sub_i32_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i32_to_slot(state, run, lhs, rhs, area, index, rt_sub_i32) }
}

pub(crate) unsafe extern "C" fn rt_jit_mul_i32_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i32_to_slot(state, run, lhs, rhs, area, index, rt_mul_i32) }
}

pub(crate) unsafe extern "C" fn rt_jit_div_i32_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i32_to_slot(state, run, lhs, rhs, area, index, rt_div_i32) }
}

pub(crate) unsafe extern "C" fn rt_jit_rem_i32_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i32_to_slot(state, run, lhs, rhs, area, index, rt_rem_i32) }
}

pub(crate) unsafe extern "C" fn rt_jit_add_i16_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i16_to_slot(state, run, lhs, rhs, area, index, rt_add_i16) }
}

pub(crate) unsafe extern "C" fn rt_jit_sub_i16_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i16_to_slot(state, run, lhs, rhs, area, index, rt_sub_i16) }
}

pub(crate) unsafe extern "C" fn rt_jit_mul_i16_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i16_to_slot(state, run, lhs, rhs, area, index, rt_mul_i16) }
}

pub(crate) unsafe extern "C" fn rt_jit_add_u8_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_u8_to_slot(state, run, lhs, rhs, area, index, rt_add_u8) }
}

pub(crate) unsafe extern "C" fn rt_jit_sub_u8_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_u8_to_slot(state, run, lhs, rhs, area, index, rt_sub_u8) }
}

pub(crate) unsafe extern "C" fn rt_jit_mul_u8_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_u8_to_slot(state, run, lhs, rhs, area, index, rt_mul_u8) }
}

pub(crate) unsafe extern "C" fn rt_jit_add_i64_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i64,
    rhs: i64,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i64_to_slot(state, run, lhs, rhs, area, index, rt_add_i64) }
}

pub(crate) unsafe extern "C" fn rt_jit_sub_i64_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i64,
    rhs: i64,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i64_to_slot(state, run, lhs, rhs, area, index, rt_sub_i64) }
}

pub(crate) unsafe extern "C" fn rt_jit_mul_i64_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i64,
    rhs: i64,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i64_to_slot(state, run, lhs, rhs, area, index, rt_mul_i64) }
}

pub(crate) unsafe extern "C" fn rt_jit_div_i64_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i64,
    rhs: i64,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i64_to_slot(state, run, lhs, rhs, area, index, rt_div_i64) }
}

pub(crate) unsafe extern "C" fn rt_jit_rem_i64_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i64,
    rhs: i64,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_i64_to_slot(state, run, lhs, rhs, area, index, rt_rem_i64) }
}

pub(crate) unsafe extern "C" fn rt_jit_add_currency_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i64,
    rhs: i64,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_currency_to_slot(state, run, lhs, rhs, area, index, rt_currency_add) }
}

pub(crate) unsafe extern "C" fn rt_jit_sub_currency_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i64,
    rhs: i64,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_currency_to_slot(state, run, lhs, rhs, area, index, rt_currency_sub) }
}

pub(crate) unsafe extern "C" fn rt_jit_mul_currency_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i64,
    rhs: i64,
    area: u32,
    index: u32,
) -> i32 {
    // SAFETY: compiled code supplies the live run/state pointers and typed scalar operands.
    unsafe { checked_currency_to_slot(state, run, lhs, rhs, area, index, rt_currency_mul) }
}

pub(crate) struct JitSlotAddress {
    pub(crate) area: u32,
    pub(crate) index: u32,
}

pub(crate) unsafe fn checked_i32_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
    shim: unsafe extern "C" fn(*mut RawExecState, i32, i32, *mut i32) -> i32,
) -> i32 {
    // SAFETY: callers uphold the raw execution-state and run-pointer contracts;
    // the selected runtime shim and store use the matching scalar representations.
    unsafe {
        checked_i32_to_slot_with_store(
            state,
            run,
            lhs,
            rhs,
            JitSlotAddress { area, index },
            shim,
            rt_jit_store_i32,
        )
    }
}

pub(crate) unsafe fn checked_i16_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
    shim: unsafe extern "C" fn(*mut RawExecState, i32, i32, *mut i32) -> i32,
) -> i32 {
    // SAFETY: callers uphold the raw execution-state and run-pointer contracts;
    // the selected runtime shim and store use the matching scalar representations.
    unsafe {
        checked_i32_to_slot_with_store(
            state,
            run,
            lhs,
            rhs,
            JitSlotAddress { area, index },
            shim,
            rt_jit_store_i16,
        )
    }
}

pub(crate) unsafe fn checked_u8_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    area: u32,
    index: u32,
    shim: unsafe extern "C" fn(*mut RawExecState, i32, i32, *mut i32) -> i32,
) -> i32 {
    // SAFETY: callers uphold the raw execution-state and run-pointer contracts;
    // the selected runtime shim and store use the matching scalar representations.
    unsafe {
        checked_i32_to_slot_with_store(
            state,
            run,
            lhs,
            rhs,
            JitSlotAddress { area, index },
            shim,
            rt_jit_store_u8,
        )
    }
}

pub(crate) unsafe fn checked_i32_to_slot_with_store(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i32,
    rhs: i32,
    destination: JitSlotAddress,
    shim: unsafe extern "C" fn(*mut RawExecState, i32, i32, *mut i32) -> i32,
    store: unsafe extern "C" fn(*mut JitRun, u32, u32, i32) -> i32,
) -> i32 {
    status_guard(|| {
        let mut out = 0;
        // SAFETY: the caller supplies a live execution state and this initialized,
        // uniquely borrowed local has the scalar output type required by `shim`.
        let status = unsafe { shim(state, lhs, rhs, &mut out) };
        if status != ST_OK {
            return status;
        }
        // SAFETY: forwarding the same run pointer received from compiled code.
        unsafe { store(run, destination.area, destination.index, out) }
    })
}

pub(crate) unsafe fn checked_i64_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i64,
    rhs: i64,
    area: u32,
    index: u32,
    shim: unsafe extern "C" fn(*mut RawExecState, i64, i64, *mut i64) -> i32,
) -> i32 {
    status_guard(|| {
        let mut out = 0;
        // SAFETY: the caller supplies a live execution state and this initialized,
        // uniquely borrowed local has the scalar output type required by `shim`.
        let status = unsafe { shim(state, lhs, rhs, &mut out) };
        if status != ST_OK {
            return status;
        }
        // SAFETY: forwarding the same run pointer received from compiled code.
        unsafe { rt_jit_store_i64(run, area, index, out) }
    })
}

pub(crate) unsafe fn checked_currency_to_slot(
    state: *mut RawExecState,
    run: *mut JitRun,
    lhs: i64,
    rhs: i64,
    area: u32,
    index: u32,
    shim: unsafe extern "C" fn(*mut RawExecState, i64, i64, *mut i64) -> i32,
) -> i32 {
    status_guard(|| {
        let mut out = 0;
        // SAFETY: the caller supplies a live execution state and this initialized,
        // uniquely borrowed local has the scalar output type required by `shim`.
        let status = unsafe { shim(state, lhs, rhs, &mut out) };
        if status != ST_OK {
            return status;
        }
        // SAFETY: forwarding the same run pointer received from compiled code.
        unsafe { rt_jit_store_currency_i64(run, area, index, out) }
    })
}
