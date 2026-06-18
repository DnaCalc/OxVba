#![cfg(target_os = "windows")]
#![allow(non_snake_case)]
#![allow(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::mem::zeroed;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::null_mut;

use windows_sys::Win32::System::Com::{
    CC_STDCALL, ELEMDESC, ELEMDESC_0, FUNC_DISPATCH, FUNC_PUREVIRTUAL, FUNCDESC, IDLDESC,
    IDLFLAG_FIN, IDLFLAG_FOUT, IDLFLAG_FRETVAL, IMPLTYPEFLAG_FDEFAULT, IMPLTYPEFLAG_FSOURCE,
    INVOKE_FUNC, INVOKE_PROPERTYGET, INVOKE_PROPERTYPUT, INVOKE_PROPERTYPUTREF, SYS_WIN64,
    TKIND_COCLASS, TKIND_DISPATCH, TKIND_INTERFACE, TYPEDESC, TYPEDESC_0,
};
use windows_sys::Win32::System::Ole::{
    CreateTypeLib2, PARAMDESC, PARAMFLAG_FIN, PARAMFLAG_FOUT, PARAMFLAG_FRETVAL,
    TYPEFLAG_FDISPATCHABLE, TYPEFLAG_FDUAL, TYPEFLAG_FNONEXTENSIBLE, TYPEFLAG_FOLEAUTOMATION,
};
use windows_sys::Win32::System::Variant::{
    VT_BOOL, VT_BSTR, VT_CY, VT_DATE, VT_DECIMAL, VT_DISPATCH, VT_EMPTY, VT_HRESULT, VT_I2, VT_I4,
    VT_I8, VT_PTR, VT_R4, VT_R8, VT_SAFEARRAY, VT_UI1, VT_VARIANT, VT_VOID,
};
use windows_sys::core::GUID;

use crate::com_descriptor::{
    ComClassDescriptor, ComEventDescriptor, ComImplementedInterfaceDescriptor,
    ComImplementedInterfaceMethodDescriptor, ComInvokeKind, ComMemberDescriptor, ComParamType,
    ComServerDescriptor, ComWireType,
};
use crate::compile::ShimCompileError;

const S_OK: i32 = 0;
const IID_ITYPEINFO: GUID = GUID {
    data1: 0x00020401,
    data2: 0,
    data3: 0,
    data4: [0xC0, 0, 0, 0, 0, 0, 0, 0x46],
};

pub fn emit_typelib_with_oleaut(
    descriptor: &ComServerDescriptor,
    tlb_target_path: &Path,
) -> Result<(), ShimCompileError> {
    if let Some(parent) = tlb_target_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ShimCompileError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }
    // SAFETY: The emitter owns the OleAut COM builder pointer returned by CreateTypeLib2
    // and validates all HRESULTs before exposing the pointer through typed wrappers.
    let mut emitter = unsafe { OleAutEmitter::create(descriptor, tlb_target_path)? };
    // SAFETY: Emission only passes descriptor-owned metadata converted to stable wide-string
    // and FUNCDESC storage that is kept alive for each OleAut call.
    unsafe { emitter.emit(descriptor) }
}

#[repr(C)]
struct IUnknownVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
}

#[repr(C)]
struct ICreateTypeLib2Vtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    create_type_info:
        unsafe extern "system" fn(*mut c_void, *const u16, i32, *mut *mut c_void) -> i32,
    set_name: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
    set_version: unsafe extern "system" fn(*mut c_void, u16, u16) -> i32,
    set_guid: unsafe extern "system" fn(*mut c_void, *const GUID) -> i32,
    set_doc_string: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
    set_help_file_name: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
    set_help_context: unsafe extern "system" fn(*mut c_void, u32) -> i32,
    set_lcid: unsafe extern "system" fn(*mut c_void, u32) -> i32,
    set_lib_flags: unsafe extern "system" fn(*mut c_void, u32) -> i32,
    save_all_changes: unsafe extern "system" fn(*mut c_void) -> i32,
}

#[repr(C)]
struct ICreateTypeInfoVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    set_guid: unsafe extern "system" fn(*mut c_void, *const GUID) -> i32,
    set_type_flags: unsafe extern "system" fn(*mut c_void, u32) -> i32,
    set_doc_string: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
    set_help_context: unsafe extern "system" fn(*mut c_void, u32) -> i32,
    set_version: unsafe extern "system" fn(*mut c_void, u16, u16) -> i32,
    add_ref_type_info: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut u32) -> i32,
    add_func_desc: unsafe extern "system" fn(*mut c_void, u32, *mut FUNCDESC) -> i32,
    add_impl_type: unsafe extern "system" fn(*mut c_void, u32, u32) -> i32,
    set_impl_type_flags: unsafe extern "system" fn(*mut c_void, u32, i32) -> i32,
    set_alignment: unsafe extern "system" fn(*mut c_void, u16) -> i32,
    set_schema: unsafe extern "system" fn(*mut c_void, *const u16) -> i32,
    add_var_desc: unsafe extern "system" fn(*mut c_void, u32, *mut c_void) -> i32,
    set_func_and_param_names:
        unsafe extern "system" fn(*mut c_void, u32, *mut *mut u16, u32) -> i32,
    set_var_name: unsafe extern "system" fn(*mut c_void, u32, *const u16) -> i32,
    set_type_desc_alias: unsafe extern "system" fn(*mut c_void, *mut TYPEDESC) -> i32,
    define_func_as_dll_entry:
        unsafe extern "system" fn(*mut c_void, u32, *const u16, *const u16) -> i32,
    set_func_doc_string: unsafe extern "system" fn(*mut c_void, u32, *const u16) -> i32,
    set_var_doc_string: unsafe extern "system" fn(*mut c_void, u32, *const u16) -> i32,
    set_func_help_context: unsafe extern "system" fn(*mut c_void, u32, u32) -> i32,
    set_var_help_context: unsafe extern "system" fn(*mut c_void, u32, u32) -> i32,
    set_mops: unsafe extern "system" fn(*mut c_void, u32, *const u16) -> i32,
    set_type_idldesc: unsafe extern "system" fn(*mut c_void, *mut IDLDESC) -> i32,
    lay_out: unsafe extern "system" fn(*mut c_void) -> i32,
}

struct OleAutEmitter {
    typelib: ComPtr<ICreateTypeLib2Vtbl>,
}

impl OleAutEmitter {
    unsafe fn create(
        descriptor: &ComServerDescriptor,
        tlb_target_path: &Path,
    ) -> Result<Self, ShimCompileError> {
        let wide_path = wide_path(tlb_target_path);
        let mut typelib = null_mut();
        let hr = CreateTypeLib2(SYS_WIN64, wide_path.as_ptr(), &mut typelib);
        check_hr("CreateTypeLib2", hr)?;
        let typelib = ComPtr::<ICreateTypeLib2Vtbl>::new(typelib);
        let vtbl = typelib.vtbl();
        let libid = parse_guid(&descriptor.libid)?;
        let library_name_w = wide(&library_name(descriptor));
        check_hr(
            "ICreateTypeLib2::SetGuid",
            (vtbl.set_guid)(typelib.ptr, &libid),
        )?;
        check_hr(
            "ICreateTypeLib2::SetName",
            (vtbl.set_name)(typelib.ptr, library_name_w.as_ptr()),
        )?;
        check_hr(
            "ICreateTypeLib2::SetVersion",
            (vtbl.set_version)(
                typelib.ptr,
                descriptor.version_major,
                descriptor.version_minor,
            ),
        )?;
        let doc = wide(&format!("{} Type Library", descriptor.project_name));
        check_hr(
            "ICreateTypeLib2::SetDocString",
            (vtbl.set_doc_string)(typelib.ptr, doc.as_ptr()),
        )?;
        check_hr("ICreateTypeLib2::SetLcid", (vtbl.set_lcid)(typelib.ptr, 0))?;
        Ok(Self { typelib })
    }

    unsafe fn emit(&mut self, descriptor: &ComServerDescriptor) -> Result<(), ShimCompileError> {
        let mut refs = BTreeMap::new();
        for interface in implemented_interfaces(descriptor) {
            let created = self.create_implemented_interface(interface)?;
            refs.insert(interface.name.clone(), created);
        }
        for class in &descriptor.classes {
            let created = self.create_class_interfaces(class)?;
            refs.insert(
                class.default_interface_name.clone(),
                created.default_interface,
            );
            if let Some(source) = created.source_interface {
                refs.insert(
                    class
                        .source_interface_name
                        .clone()
                        .expect("source name exists"),
                    source,
                );
            }
        }
        for class in &descriptor.classes {
            self.create_coclass(class, &refs)?;
        }
        let hr = (self.typelib.vtbl().save_all_changes)(self.typelib.ptr);
        check_hr("ICreateTypeLib2::SaveAllChanges", hr)
    }

    unsafe fn create_class_interfaces(
        &self,
        class: &ComClassDescriptor,
    ) -> Result<CreatedClassInterfaces, ShimCompileError> {
        let default_interface = if class_supports_bounded_dual_interface(class) {
            self.create_dual_default_interface(class)?
        } else {
            self.create_dispatch_interface(
                &class.default_interface_name,
                &class.default_interface_iid,
                &format!("{} Interface", class.class_name),
                class
                    .members
                    .iter()
                    .map(DispatchMember::Default)
                    .collect::<Vec<_>>()
                    .as_slice(),
            )?
        };
        let source_interface = if let (Some(name), Some(iid)) = (
            class.source_interface_name.as_ref(),
            class.source_interface_iid.as_ref(),
        ) {
            Some(
                self.create_dispatch_interface(
                    name,
                    iid,
                    &format!("{} Events", class.class_name),
                    class
                        .events
                        .iter()
                        .map(DispatchMember::Event)
                        .collect::<Vec<_>>()
                        .as_slice(),
                )?,
            )
        } else {
            None
        };
        Ok(CreatedClassInterfaces {
            default_interface,
            source_interface,
        })
    }

    unsafe fn create_dispatch_interface(
        &self,
        name: &str,
        iid: &str,
        doc: &str,
        members: &[DispatchMember<'_>],
    ) -> Result<CreatedTypeInfo, ShimCompileError> {
        let created = self.create_typeinfo(name, TKIND_DISPATCH)?;
        created.set_guid(iid)?;
        created.set_type_flags(TYPEFLAG_FDISPATCHABLE as u32)?;
        created.set_doc_string(doc)?;
        for (index, member) in members.iter().enumerate() {
            let mut builder = FuncDescBuilder::dispatch(member);
            created.add_func_desc(index as u32, &mut builder)?;
        }
        created.lay_out()?;
        Ok(created)
    }

    unsafe fn create_dual_default_interface(
        &self,
        class: &ComClassDescriptor,
    ) -> Result<CreatedTypeInfo, ShimCompileError> {
        let created = self.create_typeinfo(&class.default_interface_name, TKIND_INTERFACE)?;
        created.set_guid(&class.default_interface_iid)?;
        created.set_type_flags(
            (TYPEFLAG_FDUAL | TYPEFLAG_FOLEAUTOMATION | TYPEFLAG_FNONEXTENSIBLE) as u32,
        )?;
        created.set_doc_string(&format!("{} Interface", class.class_name))?;
        for (index, member) in class.members.iter().enumerate() {
            let mut builder = FuncDescBuilder::default_vtable(member);
            created.add_func_desc(index as u32, &mut builder)?;
        }
        created.lay_out()?;
        Ok(created)
    }

    unsafe fn create_implemented_interface(
        &self,
        interface: &ComImplementedInterfaceDescriptor,
    ) -> Result<CreatedTypeInfo, ShimCompileError> {
        let created = self.create_typeinfo(&interface.name, TKIND_INTERFACE)?;
        created.set_guid(&interface.iid)?;
        created.set_type_flags((TYPEFLAG_FDUAL | TYPEFLAG_FOLEAUTOMATION) as u32)?;
        created.set_doc_string(&format!("{} Interface", interface.name))?;
        for (index, method) in interface.methods.iter().enumerate() {
            let mut builder = FuncDescBuilder::implemented_vtable(method);
            created.add_func_desc(index as u32, &mut builder)?;
        }
        created.lay_out()?;
        Ok(created)
    }

    unsafe fn create_coclass(
        &self,
        class: &ComClassDescriptor,
        refs: &BTreeMap<String, CreatedTypeInfo>,
    ) -> Result<(), ShimCompileError> {
        let created = self.create_typeinfo(&class.class_name, TKIND_COCLASS)?;
        created.set_guid(&class.clsid)?;
        created.set_doc_string(class.description.as_deref().unwrap_or(&class.class_name))?;

        let default = refs.get(&class.default_interface_name).ok_or_else(|| {
            oleaut_failed(format!(
                "missing default interface `{}` for coclass `{}`",
                class.default_interface_name, class.class_name
            ))
        })?;
        created.add_impl_type(0, default, IMPLTYPEFLAG_FDEFAULT)?;
        let mut impl_index = 1;
        if let Some(source_name) = class.source_interface_name.as_ref() {
            let source = refs.get(source_name).ok_or_else(|| {
                oleaut_failed(format!(
                    "missing source interface `{source_name}` for coclass `{}`",
                    class.class_name
                ))
            })?;
            created.add_impl_type(
                impl_index,
                source,
                IMPLTYPEFLAG_FDEFAULT | IMPLTYPEFLAG_FSOURCE,
            )?;
            impl_index += 1;
        }
        for interface in &class.implemented_interfaces {
            let implemented = refs.get(&interface.name).ok_or_else(|| {
                oleaut_failed(format!(
                    "missing implemented interface `{}` for coclass `{}`",
                    interface.name, class.class_name
                ))
            })?;
            created.add_impl_type(impl_index, implemented, 0)?;
            impl_index += 1;
        }
        created.lay_out()?;
        Ok(())
    }

    unsafe fn create_typeinfo(
        &self,
        name: &str,
        typekind: i32,
    ) -> Result<CreatedTypeInfo, ShimCompileError> {
        let name_w = wide(name);
        let mut create_ptr = null_mut();
        let hr = (self.typelib.vtbl().create_type_info)(
            self.typelib.ptr,
            name_w.as_ptr(),
            typekind,
            &mut create_ptr,
        );
        check_hr("ICreateTypeLib2::CreateTypeInfo", hr)?;
        let create = ComPtr::<ICreateTypeInfoVtbl>::new(create_ptr);
        let typeinfo = create.query_interface(&IID_ITYPEINFO)?;
        Ok(CreatedTypeInfo { create, typeinfo })
    }
}

struct CreatedClassInterfaces {
    default_interface: CreatedTypeInfo,
    source_interface: Option<CreatedTypeInfo>,
}

struct CreatedTypeInfo {
    create: ComPtr<ICreateTypeInfoVtbl>,
    typeinfo: ComPtr<IUnknownVtbl>,
}

impl CreatedTypeInfo {
    unsafe fn set_guid(&self, guid: &str) -> Result<(), ShimCompileError> {
        let guid = parse_guid(guid)?;
        check_hr(
            "ICreateTypeInfo::SetGuid",
            (self.create.vtbl().set_guid)(self.create.ptr, &guid),
        )
    }

    unsafe fn set_type_flags(&self, flags: u32) -> Result<(), ShimCompileError> {
        check_hr(
            "ICreateTypeInfo::SetTypeFlags",
            (self.create.vtbl().set_type_flags)(self.create.ptr, flags),
        )
    }

    unsafe fn set_doc_string(&self, doc: &str) -> Result<(), ShimCompileError> {
        let doc_w = wide(doc);
        check_hr(
            "ICreateTypeInfo::SetDocString",
            (self.create.vtbl().set_doc_string)(self.create.ptr, doc_w.as_ptr()),
        )
    }

    unsafe fn add_func_desc(
        &self,
        index: u32,
        builder: &mut FuncDescBuilder,
    ) -> Result<(), ShimCompileError> {
        let mut names = builder.names_ptrs();
        let hr = (self.create.vtbl().add_func_desc)(self.create.ptr, index, &mut builder.funcdesc);
        check_hr("ICreateTypeInfo::AddFuncDesc", hr)?;
        let hr = (self.create.vtbl().set_func_and_param_names)(
            self.create.ptr,
            index,
            names.as_mut_ptr(),
            names.len() as u32,
        );
        check_hr("ICreateTypeInfo::SetFuncAndParamNames", hr)
    }

    unsafe fn add_impl_type(
        &self,
        index: u32,
        interface: &CreatedTypeInfo,
        flags: i32,
    ) -> Result<(), ShimCompileError> {
        let mut href = 0;
        let hr = (self.create.vtbl().add_ref_type_info)(
            self.create.ptr,
            interface.typeinfo.ptr,
            &mut href,
        );
        check_hr("ICreateTypeInfo::AddRefTypeInfo", hr)?;
        let hr = (self.create.vtbl().add_impl_type)(self.create.ptr, index, href);
        check_hr("ICreateTypeInfo::AddImplType", hr)?;
        let hr = (self.create.vtbl().set_impl_type_flags)(self.create.ptr, index, flags);
        check_hr("ICreateTypeInfo::SetImplTypeFlags", hr)
    }

    unsafe fn lay_out(&self) -> Result<(), ShimCompileError> {
        check_hr(
            "ICreateTypeInfo::LayOut",
            (self.create.vtbl().lay_out)(self.create.ptr),
        )
    }
}

struct ComPtr<V> {
    ptr: *mut c_void,
    _marker: std::marker::PhantomData<V>,
}

impl<V> ComPtr<V> {
    unsafe fn new(ptr: *mut c_void) -> Self {
        Self {
            ptr,
            _marker: std::marker::PhantomData,
        }
    }

    unsafe fn vtbl(&self) -> &V {
        &**(self.ptr as *const *const V)
    }
}

impl<V> ComPtr<V> {
    unsafe fn query_interface<W>(&self, iid: &GUID) -> Result<ComPtr<W>, ShimCompileError> {
        let vtbl = &**(self.ptr as *const *const IUnknownVtbl);
        let mut out = null_mut();
        let hr = (vtbl.query_interface)(self.ptr, iid, &mut out);
        check_hr("IUnknown::QueryInterface", hr)?;
        Ok(ComPtr::new(out))
    }
}

impl<V> Drop for ComPtr<V> {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }
        // SAFETY: ComPtr is constructed only from successful COM APIs returning an owned
        // interface pointer, so Drop releases that single reference through IUnknown.
        unsafe {
            let vtbl = &**(self.ptr as *const *const IUnknownVtbl);
            (vtbl.release)(self.ptr);
        }
    }
}

enum DispatchMember<'a> {
    Default(&'a ComMemberDescriptor),
    Event(&'a ComEventDescriptor),
}

struct FuncDescBuilder {
    funcdesc: FUNCDESC,
    _params: Vec<ELEMDESC>,
    names: Vec<Vec<u16>>,
}

impl FuncDescBuilder {
    fn dispatch(member: &DispatchMember<'_>) -> Self {
        match member {
            DispatchMember::Default(member) => {
                let param_names = names_for_params(&member.name, &member.parameter_names);
                let params = member
                    .parameter_types
                    .iter()
                    .copied()
                    .map(param_elemdesc)
                    .collect::<Vec<_>>();
                Self::new(
                    member.dispid,
                    FUNC_DISPATCH,
                    invkind(member.invoke_kind),
                    member.return_type.map(vartype).unwrap_or(
                        if member.invoke_kind == ComInvokeKind::PropertyGet {
                            VT_VARIANT
                        } else {
                            VT_VOID
                        },
                    ),
                    params,
                    param_names,
                    0,
                )
            }
            DispatchMember::Event(event) => {
                let params = (0..event.callback_arity)
                    .map(|_| param_elemdesc(ComParamType::Variant))
                    .collect::<Vec<_>>();
                let names = std::iter::once(event.name.clone())
                    .chain((0..event.callback_arity).map(|index| format!("arg{index}")))
                    .collect::<Vec<_>>();
                Self::new(
                    event.dispid,
                    FUNC_DISPATCH,
                    INVOKE_FUNC,
                    VT_VOID,
                    params,
                    names,
                    0,
                )
            }
        }
    }

    fn default_vtable(member: &ComMemberDescriptor) -> Self {
        let retval = retval_elemdesc(member.return_type, member.invoke_kind);
        let params = member
            .parameter_types
            .iter()
            .copied()
            .map(param_elemdesc)
            .chain(retval)
            .collect::<Vec<_>>();
        let mut names = names_for_params(&member.name, &member.parameter_names);
        if returns_value(member.invoke_kind) {
            names.push("result".to_string());
        }
        Self::new(
            member.dispid,
            FUNC_PUREVIRTUAL,
            invkind(member.invoke_kind),
            VT_HRESULT,
            params,
            names,
            member.vtable_slot.unwrap_or(7),
        )
    }

    fn implemented_vtable(method: &ComImplementedInterfaceMethodDescriptor) -> Self {
        let retval = retval_wire_elemdesc(method.return_wire_type.as_ref(), method.invoke_kind);
        let params = method
            .parameter_wire_types
            .iter()
            .map(wire_param_elemdesc)
            .chain(retval)
            .collect::<Vec<_>>();
        let mut names = names_for_params(&method.name, &method.parameter_names);
        if returns_value(method.invoke_kind) {
            names.push("result".to_string());
        }
        Self::new(
            method.dispid,
            FUNC_PUREVIRTUAL,
            invkind(method.invoke_kind),
            VT_HRESULT,
            params,
            names,
            method.vtable_slot.unwrap_or(7),
        )
    }

    fn new(
        memid: i32,
        funckind: i32,
        invkind: i32,
        return_vt: u16,
        mut params: Vec<ELEMDESC>,
        names: Vec<String>,
        vtable_slot: u16,
    ) -> Self {
        // SAFETY: FUNCDESC is a plain COM descriptor initialized field-by-field below;
        // zero is the documented baseline used before filling optional pointer fields.
        let mut funcdesc: FUNCDESC = unsafe { zeroed() };
        funcdesc.memid = memid;
        funcdesc.funckind = funckind;
        funcdesc.invkind = invkind;
        funcdesc.callconv = CC_STDCALL;
        funcdesc.cParams = params.len() as i16;
        funcdesc.oVft = (usize::from(vtable_slot) * std::mem::size_of::<usize>()) as i16;
        funcdesc.elemdescFunc = vt_elemdesc(return_vt, 0);
        funcdesc.lprgelemdescParam = if params.is_empty() {
            null_mut()
        } else {
            params.as_mut_ptr()
        };
        let names = names
            .into_iter()
            .map(|name| wide(&name))
            .collect::<Vec<_>>();
        Self {
            funcdesc,
            _params: params,
            names,
        }
    }

    fn names_ptrs(&mut self) -> Vec<*mut u16> {
        self.names
            .iter_mut()
            .map(|name| name.as_mut_ptr())
            .collect::<Vec<_>>()
    }
}

fn names_for_params(member_name: &str, parameter_names: &[String]) -> Vec<String> {
    std::iter::once(member_name.to_string())
        .chain(parameter_names.iter().enumerate().map(|(index, name)| {
            if name.is_empty() {
                format!("arg{index}")
            } else {
                name.clone()
            }
        }))
        .collect()
}

fn retval_elemdesc(
    return_type: Option<ComParamType>,
    invoke_kind: ComInvokeKind,
) -> Option<ELEMDESC> {
    if !returns_value(invoke_kind) {
        return None;
    }
    Some(pointer_elemdesc(
        vartype(return_type.unwrap_or(ComParamType::Variant)),
        IDLFLAG_FOUT | IDLFLAG_FRETVAL,
    ))
}

fn retval_wire_elemdesc(
    return_type: Option<&ComWireType>,
    invoke_kind: ComInvokeKind,
) -> Option<ELEMDESC> {
    if !returns_value(invoke_kind) {
        return None;
    }
    Some(match return_type {
        Some(wire_type) => wire_elemdesc(wire_type, IDLFLAG_FOUT | IDLFLAG_FRETVAL),
        None => value_elemdesc(ComParamType::Variant, IDLFLAG_FOUT | IDLFLAG_FRETVAL),
    })
}

fn param_elemdesc(param: ComParamType) -> ELEMDESC {
    if param.is_by_ref() {
        pointer_elemdesc(vartype(base_param_type(param)), IDLFLAG_FIN | IDLFLAG_FOUT)
    } else {
        value_elemdesc(param, IDLFLAG_FIN)
    }
}

fn wire_param_elemdesc(wire_type: &ComWireType) -> ELEMDESC {
    wire_elemdesc(wire_type, IDLFLAG_FIN)
}

fn wire_elemdesc(wire_type: &ComWireType, flags: u16) -> ELEMDESC {
    match wire_type {
        ComWireType::Automation(param) => {
            if param.is_by_ref() || flags & IDLFLAG_FRETVAL != 0 {
                pointer_elemdesc(vartype(base_param_type(*param)), flags)
            } else {
                value_elemdesc(*param, flags)
            }
        }
        ComWireType::InterfacePointer { .. } => pointer_elemdesc(VT_DISPATCH, flags),
        ComWireType::SafeArrayVariant => safearray_elemdesc(flags, false),
        ComWireType::ByRefSafeArrayVariant => safearray_elemdesc(flags, true),
    }
}

fn value_elemdesc(param: ComParamType, flags: u16) -> ELEMDESC {
    vt_elemdesc(vartype(param), flags)
}

fn vt_elemdesc(vt: u16, flags: u16) -> ELEMDESC {
    elemdesc(
        TYPEDESC {
            Anonymous: TYPEDESC_0 {
                lptdesc: null_mut(),
            },
            vt,
        },
        flags,
    )
}

fn pointer_elemdesc(inner_vt: u16, flags: u16) -> ELEMDESC {
    let inner = Box::new(TYPEDESC {
        Anonymous: TYPEDESC_0 {
            lptdesc: null_mut(),
        },
        vt: inner_vt,
    });
    let ptr = Box::into_raw(inner);
    elemdesc(
        TYPEDESC {
            Anonymous: TYPEDESC_0 { lptdesc: ptr },
            vt: VT_PTR,
        },
        flags,
    )
}

fn safearray_elemdesc(flags: u16, by_ref: bool) -> ELEMDESC {
    let variant = Box::new(TYPEDESC {
        Anonymous: TYPEDESC_0 {
            lptdesc: null_mut(),
        },
        vt: VT_VARIANT,
    });
    let variant_ptr = Box::into_raw(variant);
    let array = Box::new(TYPEDESC {
        Anonymous: TYPEDESC_0 {
            lptdesc: variant_ptr,
        },
        vt: VT_SAFEARRAY,
    });
    let array_ptr = Box::into_raw(array);
    if by_ref {
        elemdesc(
            TYPEDESC {
                Anonymous: TYPEDESC_0 { lptdesc: array_ptr },
                vt: VT_PTR,
            },
            flags | IDLFLAG_FOUT,
        )
    } else {
        elemdesc(
            TYPEDESC {
                Anonymous: TYPEDESC_0 { lptdesc: array_ptr },
                vt: VT_PTR,
            },
            flags,
        )
    }
}

fn elemdesc(tdesc: TYPEDESC, flags: u16) -> ELEMDESC {
    ELEMDESC {
        tdesc,
        Anonymous: ELEMDESC_0 {
            paramdesc: PARAMDESC {
                pparamdescex: null_mut(),
                wParamFlags: param_flags(flags),
            },
        },
    }
}

fn param_flags(idl_flags: u16) -> u16 {
    let mut flags = 0;
    if idl_flags & IDLFLAG_FIN != 0 {
        flags |= PARAMFLAG_FIN;
    }
    if idl_flags & IDLFLAG_FOUT != 0 {
        flags |= PARAMFLAG_FOUT;
    }
    if idl_flags & IDLFLAG_FRETVAL != 0 {
        flags |= PARAMFLAG_FRETVAL;
    }
    flags
}

fn invkind(kind: ComInvokeKind) -> i32 {
    match kind {
        ComInvokeKind::PropertyGet => INVOKE_PROPERTYGET,
        ComInvokeKind::Method => INVOKE_FUNC,
        ComInvokeKind::PropertyPut => INVOKE_PROPERTYPUT,
        ComInvokeKind::PropertyPutRef => INVOKE_PROPERTYPUTREF,
    }
}

fn returns_value(kind: ComInvokeKind) -> bool {
    matches!(kind, ComInvokeKind::PropertyGet | ComInvokeKind::Method)
}

fn vartype(param: ComParamType) -> u16 {
    match base_param_type(param) {
        ComParamType::Variant => VT_VARIANT,
        ComParamType::Long => VT_I4,
        ComParamType::Integer => VT_I2,
        ComParamType::String => VT_BSTR,
        ComParamType::Boolean => VT_BOOL,
        ComParamType::Double => VT_R8,
        ComParamType::Single => VT_R4,
        ComParamType::Currency => VT_CY,
        ComParamType::Date => VT_DATE,
        ComParamType::Decimal => VT_DECIMAL,
        ComParamType::Object => VT_DISPATCH,
        ComParamType::Byte => VT_UI1,
        ComParamType::LongLong | ComParamType::LongPtr => VT_I8,
        _ => VT_EMPTY,
    }
}

fn base_param_type(param: ComParamType) -> ComParamType {
    match param {
        ComParamType::ByRefVariant => ComParamType::Variant,
        ComParamType::ByRefLong => ComParamType::Long,
        ComParamType::ByRefInteger => ComParamType::Integer,
        ComParamType::ByRefString => ComParamType::String,
        ComParamType::ByRefDouble => ComParamType::Double,
        ComParamType::ByRefSingle => ComParamType::Single,
        ComParamType::ByRefCurrency => ComParamType::Currency,
        ComParamType::ByRefDate => ComParamType::Date,
        ComParamType::ByRefDecimal => ComParamType::Decimal,
        ComParamType::ByRefObject => ComParamType::Object,
        ComParamType::ByRefByte => ComParamType::Byte,
        ComParamType::ByRefBoolean => ComParamType::Boolean,
        ComParamType::ByRefLongLong => ComParamType::LongLong,
        ComParamType::ByRefLongPtr => ComParamType::LongPtr,
        other => other,
    }
}

fn implemented_interfaces(
    descriptor: &ComServerDescriptor,
) -> Vec<&ComImplementedInterfaceDescriptor> {
    let mut interfaces = Vec::new();
    for class in &descriptor.classes {
        for interface in &class.implemented_interfaces {
            if !interfaces
                .iter()
                .any(|existing: &&ComImplementedInterfaceDescriptor| existing.iid == interface.iid)
            {
                interfaces.push(interface);
            }
        }
    }
    interfaces
}

fn class_supports_bounded_dual_interface(class: &ComClassDescriptor) -> bool {
    class_supports_bounded_dual_scalar_methods(class)
        || class_supports_bounded_dual_long_property(class)
        || class_supports_bounded_dual_object_return_methods(class)
        || class_supports_bounded_dual_object_argument_methods(class)
}

fn class_supports_bounded_dual_scalar_methods(class: &ComClassDescriptor) -> bool {
    !class.members.is_empty()
        && class.members.len() <= 3
        && class.members.iter().enumerate().all(|(index, member)| {
            member.vtable_slot == Some(7 + index as u16)
                && member_supports_bounded_dual_scalar_method(member)
        })
}

fn class_supports_bounded_dual_long_property(class: &ComClassDescriptor) -> bool {
    if class.members.len() != 2 {
        return false;
    }
    let get = &class.members[0];
    let put = &class.members[1];
    get.vtable_slot == Some(7)
        && put.vtable_slot == Some(8)
        && get.name.eq_ignore_ascii_case(&put.name)
        && get.dispid == put.dispid
        && member_supports_bounded_dual_long_property_get(get)
        && member_supports_bounded_dual_long_property_put(put)
}

fn class_supports_bounded_dual_object_return_methods(class: &ComClassDescriptor) -> bool {
    !class.members.is_empty()
        && class.members.len() <= 2
        && class.members.iter().enumerate().all(|(index, member)| {
            member.vtable_slot == Some(7 + index as u16)
                && if index == 0 {
                    member_supports_bounded_dual_object_return(member)
                } else {
                    member_supports_bounded_dual_slot8_long_noarg(member)
                }
        })
}

fn class_supports_bounded_dual_object_argument_methods(class: &ComClassDescriptor) -> bool {
    if class.members.len() != 2 {
        return false;
    }
    let ping = &class.members[0];
    let echo = &class.members[1];
    ping.vtable_slot == Some(7)
        && echo.vtable_slot == Some(8)
        && member_supports_bounded_dual_slot7_long_noarg(ping)
        && member_supports_bounded_dual_slot8_object_arg_long(echo)
}

fn member_supports_bounded_dual_scalar_method(member: &ComMemberDescriptor) -> bool {
    if member.invoke_kind != ComInvokeKind::Method
        || member.parameter_optional.iter().any(|optional| *optional)
    {
        return false;
    }
    matches!(
        (
            member.vtable_slot,
            member.return_type,
            member.parameter_types.as_slice()
        ),
        (Some(7), Some(ComParamType::Long), [])
            | (
                Some(8),
                Some(ComParamType::Long),
                [ComParamType::Long, ComParamType::Long],
            )
            | (
                Some(9),
                Some(ComParamType::Double),
                [ComParamType::Double, ComParamType::Double],
            )
    )
}

fn member_supports_bounded_dual_slot7_long_noarg(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::Method
        && member.vtable_slot == Some(7)
        && member.return_type == Some(ComParamType::Long)
        && member.parameter_types.is_empty()
        && !member.parameter_optional.iter().any(|optional| *optional)
}

fn member_supports_bounded_dual_object_return(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::Method
        && member.vtable_slot == Some(7)
        && member.return_type == Some(ComParamType::Object)
        && member.parameter_types.is_empty()
        && !member.parameter_optional.iter().any(|optional| *optional)
}

fn member_supports_bounded_dual_slot8_object_arg_long(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::Method
        && member.vtable_slot == Some(8)
        && member.return_type == Some(ComParamType::Long)
        && member.parameter_types.as_slice() == [ComParamType::Object]
        && !member.parameter_optional.iter().any(|optional| *optional)
}

fn member_supports_bounded_dual_slot8_long_noarg(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::Method
        && member.vtable_slot == Some(8)
        && member.return_type == Some(ComParamType::Long)
        && member.parameter_types.is_empty()
        && !member.parameter_optional.iter().any(|optional| *optional)
}

fn member_supports_bounded_dual_long_property_get(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::PropertyGet
        && member.vtable_slot == Some(7)
        && member.return_type == Some(ComParamType::Long)
        && member.parameter_types.is_empty()
        && !member.parameter_optional.iter().any(|optional| *optional)
}

fn member_supports_bounded_dual_long_property_put(member: &ComMemberDescriptor) -> bool {
    member.invoke_kind == ComInvokeKind::PropertyPut
        && member.vtable_slot == Some(8)
        && member.return_type.is_none()
        && member.parameter_types.as_slice() == [ComParamType::Long]
        && !member.parameter_optional.iter().any(|optional| *optional)
}

fn library_name(descriptor: &ComServerDescriptor) -> String {
    sanitize_ident(&format!("{}Lib", descriptor.project_name))
}

fn sanitize_ident(raw: &str) -> String {
    let mut out = String::new();
    for (index, ch) in raw.chars().enumerate() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            if index == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "_OxVbaName".to_string()
    } else {
        out
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn parse_guid(text: &str) -> Result<GUID, ShimCompileError> {
    let text = text.trim().trim_start_matches('{').trim_end_matches('}');
    let parts = text.split('-').collect::<Vec<_>>();
    if parts.len() != 5 || parts[3].len() != 4 || parts[4].len() != 12 {
        return Err(oleaut_failed(format!("invalid GUID `{text}`")));
    }
    let data1 = u32::from_str_radix(parts[0], 16)
        .map_err(|_| oleaut_failed(format!("invalid GUID `{text}`")))?;
    let data2 = u16::from_str_radix(parts[1], 16)
        .map_err(|_| oleaut_failed(format!("invalid GUID `{text}`")))?;
    let data3 = u16::from_str_radix(parts[2], 16)
        .map_err(|_| oleaut_failed(format!("invalid GUID `{text}`")))?;
    let mut data4 = [0u8; 8];
    data4[0] = u8::from_str_radix(&parts[3][0..2], 16)
        .map_err(|_| oleaut_failed(format!("invalid GUID `{text}`")))?;
    data4[1] = u8::from_str_radix(&parts[3][2..4], 16)
        .map_err(|_| oleaut_failed(format!("invalid GUID `{text}`")))?;
    for index in 0..6 {
        data4[index + 2] = u8::from_str_radix(&parts[4][index * 2..index * 2 + 2], 16)
            .map_err(|_| oleaut_failed(format!("invalid GUID `{text}`")))?;
    }
    Ok(GUID {
        data1,
        data2,
        data3,
        data4,
    })
}

fn check_hr(operation: &str, hr: i32) -> Result<(), ShimCompileError> {
    if hr == S_OK {
        Ok(())
    } else {
        Err(oleaut_failed(format!(
            "{operation} failed: HRESULT=0x{hr:08X}"
        )))
    }
}

fn oleaut_failed(message: String) -> ShimCompileError {
    ShimCompileError::OleAutTypeLibFailed { message }
}
