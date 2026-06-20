use oxvba_runtime::{
    RUNTIME_IDISPATCH_INTERFACE_IDENTITY, RuntimeClassDescriptor, RuntimeInterfaceDescriptor,
    RuntimeInterfaceId, RuntimeMemberDescriptor, RuntimeMemberInvokeKind, RuntimeParamDescriptor,
    RuntimeValueType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibResolveRequest {
    pub reference_name: String,
    pub requested_coclass: Option<String>,
    pub importlib_hint: Option<String>,
    pub libid_hint: Option<String>,
    pub major_version_hint: Option<u16>,
    pub minor_version_hint: Option<u16>,
    pub lcid_hint: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibResolvedIdentity {
    pub reference_name: String,
    pub requested_coclass: Option<String>,
    pub importlib: String,
    pub libid: Option<String>,
    pub major_version: u16,
    pub minor_version: u16,
    pub lcid: Option<u32>,
    pub cache_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibMetadataBlob {
    pub identity: TypeLibResolvedIdentity,
    pub activation_prog_id: Option<String>,
    pub member_name_to_token: Vec<(String, i32)>,
    pub members: Vec<TypeLibMemberMetadata>,
    pub events: Vec<TypeLibEventMetadata>,
    /// The COCLASS type names this typelib defines (e.g. `["Application", "Workbook",
    /// "Worksheet", …]` for Excel). A library-level reference (no `requested_coclass`)
    /// otherwise only answers to its reference name + one arbitrary coclass's ProgID, so
    /// `WithEvents x As Excel.Application` (and compile-time `Dim x As Excel.Workbook`)
    /// could not resolve. The symbol provider owns `<reference_name>.<coclass>` for each
    /// of these, so every coclass in the referenced library is bindable by name.
    pub coclass_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibInterfaceMetadata {
    pub name: String,
    pub iid: Option<ComInterfaceIid>,
    pub members: Vec<TypeLibMemberMetadata>,
}

/// A COM interface IID in its canonical `{data1-data2-data3-data4}` field layout
/// (the same byte layout as `windows_sys::core::GUID`), stored platform-neutrally
/// so it can ride on the metadata blob / `ComMemberSpec` (which derive
/// `Debug`/`PartialEq`/`Eq`, traits `GUID` does not implement) on every target.
///
/// Workset S5a carries the member's defining **dual interface** IID here so the
/// vtable dispatch site can `QueryInterface` the live object for that exact
/// interface and call the slot on the returned (real-vtable) pointer — uniformly
/// in-process and out-of-process — instead of vtable-calling the raw `IDispatch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComInterfaceIid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

#[cfg(target_os = "windows")]
impl ComInterfaceIid {
    /// Adopt a live `windows_sys::core::GUID` (e.g. `TYPEATTR::guid`) into the
    /// platform-neutral carrier.
    pub fn from_guid(guid: &windows_sys::core::GUID) -> Self {
        Self {
            data1: guid.data1,
            data2: guid.data2,
            data3: guid.data3,
            data4: guid.data4,
        }
    }

    /// Project back to a `windows_sys::core::GUID` for `QueryInterface`.
    pub fn to_guid(self) -> windows_sys::core::GUID {
        windows_sys::core::GUID {
            data1: self.data1,
            data2: self.data2,
            data3: self.data3,
            data4: self.data4,
        }
    }

    /// True for the all-zero IID (`IID_NULL`), which is never a usable QI target.
    pub fn is_null(&self) -> bool {
        self.data1 == 0 && self.data2 == 0 && self.data3 == 0 && self.data4 == [0; 8]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeLibParamType {
    Variant,
    Long,
    Integer,
    String,
    Boolean,
    Double,
    Single,
    Currency,
    Date,
    Decimal,
    Object,
    Byte,
    LongLong,
    LongPtr,
    ByRefVariant,
    ByRefLong,
    ByRefInteger,
    ByRefString,
    ByRefDouble,
    ByRefSingle,
    ByRefCurrency,
    ByRefDate,
    ByRefDecimal,
    ByRefObject,
    ByRefByte,
    ByRefBoolean,
    ByRefLongLong,
    ByRefLongPtr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeLibWireType {
    Automation(TypeLibParamType),
    InterfacePointer { name: String },
    SafeArrayVariant,
    ByRefSafeArrayVariant,
}

impl TypeLibWireType {
    /// Whether this exact wire shape is currently supported as a vtable inbound
    /// parameter for the semantic typelib type. This is intentionally narrower
    /// than the full descriptor vocabulary: pointer-width and object ByRef
    /// shapes remain IDispatch fallback until their ownership rules are proven.
    pub(crate) fn supports_vtable_param(&self, param_type: TypeLibParamType) -> bool {
        match self {
            Self::Automation(wire_param_type) => *wire_param_type == param_type,
            Self::InterfacePointer { .. } => {
                matches!(
                    param_type,
                    TypeLibParamType::Object | TypeLibParamType::ByRefObject
                )
            }
            Self::SafeArrayVariant => matches!(param_type, TypeLibParamType::Variant),
            Self::ByRefSafeArrayVariant => matches!(param_type, TypeLibParamType::Variant),
        }
    }

    /// Whether this exact wire shape is currently supported as a vtable return
    /// value for the semantic typelib type.
    pub(crate) fn supports_vtable_return(&self, return_type: TypeLibParamType) -> bool {
        match self {
            Self::Automation(wire_return_type) => *wire_return_type == return_type,
            Self::InterfacePointer { .. } => matches!(return_type, TypeLibParamType::Object),
            Self::SafeArrayVariant => matches!(return_type, TypeLibParamType::Variant),
            Self::ByRefSafeArrayVariant => false,
        }
    }
}

impl TypeLibParamType {
    /// Whether this semantic type is currently supported by the COM vtable
    /// marshaller. Keep this intentionally narrower than the full typelib
    /// vocabulary: unsupported shapes must fall back before any vtable slot is
    /// read.
    pub(crate) fn supports_vtable_param_abi(self) -> bool {
        matches!(
            self,
            TypeLibParamType::Variant
                | TypeLibParamType::Long
                | TypeLibParamType::Integer
                | TypeLibParamType::String
                | TypeLibParamType::Boolean
                | TypeLibParamType::Double
                | TypeLibParamType::Single
                | TypeLibParamType::Currency
                | TypeLibParamType::Date
                | TypeLibParamType::Object
                | TypeLibParamType::Byte
                | TypeLibParamType::LongLong
                | TypeLibParamType::Decimal
                | TypeLibParamType::ByRefVariant
                | TypeLibParamType::ByRefLong
                | TypeLibParamType::ByRefInteger
                | TypeLibParamType::ByRefDouble
                | TypeLibParamType::ByRefSingle
                | TypeLibParamType::ByRefCurrency
                | TypeLibParamType::ByRefDate
                | TypeLibParamType::ByRefDecimal
                | TypeLibParamType::ByRefByte
                | TypeLibParamType::ByRefBoolean
                | TypeLibParamType::ByRefLongLong
                | TypeLibParamType::ByRefString
                | TypeLibParamType::ByRefObject
                | TypeLibParamType::ByRefLongPtr
        )
    }

    pub(crate) fn supports_vtable_return_abi(self) -> bool {
        self.supports_vtable_param_abi()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypeLibVtableSignatureIssue {
    UnsupportedParameterType(TypeLibParamType),
    UnsupportedReturnType(TypeLibParamType),
    ParameterWireTypeArityMismatch,
    UnsupportedParameterWireType,
    UnsupportedReturnWireType,
    MissingObjectParameterIid,
}

pub(crate) fn validate_vtable_wire_signature(
    parameter_types: &[TypeLibParamType],
    parameter_wire_types: &[TypeLibWireType],
    parameter_iids: &[Option<ComInterfaceIid>],
    return_type: Option<TypeLibParamType>,
    return_wire_type: Option<&TypeLibWireType>,
) -> Result<(), TypeLibVtableSignatureIssue> {
    if let Some(param_type) = parameter_types
        .iter()
        .find(|param_type| !param_type.supports_vtable_param_abi())
    {
        return Err(TypeLibVtableSignatureIssue::UnsupportedParameterType(
            *param_type,
        ));
    }
    if let Some(rt) = return_type
        && !rt.supports_vtable_return_abi()
    {
        return Err(TypeLibVtableSignatureIssue::UnsupportedReturnType(rt));
    }
    if !parameter_wire_types.is_empty() {
        if parameter_wire_types.len() != parameter_types.len() {
            return Err(TypeLibVtableSignatureIssue::ParameterWireTypeArityMismatch);
        }
        if parameter_types
            .iter()
            .zip(parameter_wire_types)
            .any(|(param_type, wire_type)| !wire_type.supports_vtable_param(*param_type))
        {
            return Err(TypeLibVtableSignatureIssue::UnsupportedParameterWireType);
        }
    }
    if let (Some(rt), Some(wire_type)) = (return_type, return_wire_type)
        && !wire_type.supports_vtable_return(rt)
    {
        return Err(TypeLibVtableSignatureIssue::UnsupportedReturnWireType);
    }
    if parameter_types.iter().enumerate().any(|(i, param_type)| {
        matches!(
            param_type,
            TypeLibParamType::Object | TypeLibParamType::ByRefObject
        ) && parameter_iids.get(i).copied().flatten().is_none()
    }) {
        return Err(TypeLibVtableSignatureIssue::MissingObjectParameterIid);
    }
    Ok(())
}

impl TypeLibParamType {
    /// True for the `ByRef*` variants — the parameter is passed by reference
    /// (`[out]`/`[in,out]` in IDL), so the caller's argument is written back.
    pub fn is_by_ref(&self) -> bool {
        matches!(
            self,
            TypeLibParamType::ByRefVariant
                | TypeLibParamType::ByRefLong
                | TypeLibParamType::ByRefInteger
                | TypeLibParamType::ByRefString
                | TypeLibParamType::ByRefDouble
                | TypeLibParamType::ByRefSingle
                | TypeLibParamType::ByRefCurrency
                | TypeLibParamType::ByRefDate
                | TypeLibParamType::ByRefDecimal
                | TypeLibParamType::ByRefObject
                | TypeLibParamType::ByRefByte
                | TypeLibParamType::ByRefBoolean
                | TypeLibParamType::ByRefLongLong
                | TypeLibParamType::ByRefLongPtr
        )
    }
}

/// A scalar typelib-declared parameter default value (the IDL `defaultvalue`
/// attribute only permits constant scalars — never an object or a SAFEARRAY), in
/// a form that is plainly `Send + Sync + Eq` so it can ride on `ComMemberSpec`
/// inside the shared `Arc<Mutex<WindowsComClientState>>` (a full `ComValue` could
/// carry a `!Send` `ObjectRef`/`SafeArray`, which a constant default never is).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComDefaultValue {
    Empty,
    Null,
    Bool(bool),
    /// VT_I2/VT_I4/VT_UI1/VT_INT — any integer default, carried as i64.
    Int(i64),
    /// VT_R4/VT_R8 — a floating default, carried by its exact 8-byte bit pattern
    /// so the enum stays `Eq` (NaN-bit-identical defaults compare equal).
    FloatBits(u64),
    /// VT_CY — a currency default scaled ×10000 (the COM CY integer).
    CurrencyScaled(i64),
    /// VT_DATE — an OLE automation date default, by its exact bit pattern.
    DateBits(u64),
    /// VT_BSTR — a string default.
    Str(String),
    /// VT_ERROR — an error-code default (scode).
    ErrorCode(i32),
}

/// How a trailing positional parameter that the guest OMITTED can be synthesized
/// for a vtable slot call (the vtable ABI has no DISPPARAMS to shorten, so an
/// omitted optional must be supplied as a concrete value). One entry per declared
/// FUNCDESC parameter, left-to-right, parallel to [`TypeLibMemberMetadata::parameter_types`].
///
/// The vtable gate (D3) admits a call with fewer supplied positionals than
/// declared params ONLY when every missing one is trailing and is either
/// [`HasDefault`](OptionalParamDefault::HasDefault) or
/// [`OptionalVariant`](OptionalParamDefault::OptionalVariant); a [`Required`] or
/// an [`OptionalNoDefault`] in the missing tail falls the whole call back to the
/// IDispatch path (which can drop trailing optionals natively).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionalParamDefault {
    /// A required parameter (no `PARAMFLAG_FOPT`/`FHASDEFAULT`, and not inside the
    /// FUNCDESC trailing-optional run). Cannot be omitted from a vtable call.
    Required,
    /// Optional with a typelib-declared default value (`PARAMFLAG_FHASDEFAULT`,
    /// `0x0020`): the `PARAMDESCEX.varDefaultValue` projected into a scalar
    /// [`ComDefaultValue`]. When omitted, this exact value is synthesized.
    HasDefault(ComDefaultValue),
    /// An optional `VARIANT`-typed parameter with no declared default
    /// (`PARAMFLAG_FOPT`, `0x0010`). When omitted, a `VT_ERROR` VARIANT carrying
    /// `DISP_E_PARAMNOTFOUND` is synthesized (the standard "missing optional"
    /// marshaling a VBA caller's omitted optional produces).
    OptionalVariant,
    /// An optional NON-`VARIANT` parameter with no declared default. There is no
    /// safe value to synthesize for the slot ABI, so an omitted one of these
    /// declines the vtable path (the IDispatch fallback handles it).
    OptionalNoDefault,
    /// A hidden `[lcid]` parameter (`PARAMFLAG_FLCID`, `0x0004`): an `LCID` the
    /// vtable ABI injects ahead of the `[out,retval]`, NEVER supplied by the VBA
    /// caller (it is invisible at the language level). The vtable dispatch site
    /// ALWAYS synthesizes it (with `LOCALE_NEUTRAL` / 0) and never consumes a
    /// guest argument for it — distinct from an OMITTED optional, which the guest
    /// *could* have supplied. Its `parameter_types` slot is `Long` (`VT_I4`).
    /// Omitting LCID synthesis is exactly the bug that AV'd out-of-process Excel
    /// `Application.Build`/`Version` (a `[propget, lcid]` whose true vtable ABI is
    /// `HRESULT get_X(this, LCID, T* pRet)`).
    Lcid,
}

impl ComDefaultValue {
    /// Project a typelib default `ComValue` (read from `PARAMDESCEX.varDefaultValue`)
    /// into the `Send`-safe scalar carrier. Returns `None` for the non-scalar kinds
    /// a constant default never legitimately is (object / SAFEARRAY / Decimal),
    /// which then decline a stored default (the param falls back to its optional
    /// rule).
    pub fn from_com_value(value: &crate::ComValue) -> Option<Self> {
        use crate::ComValue as V;
        Some(match value {
            V::Empty => Self::Empty,
            V::Null => Self::Null,
            V::Bool(b) => Self::Bool(*b),
            V::I32(n) => Self::Int(i64::from(*n)),
            V::I64(n) => Self::Int(*n),
            V::U64(n) => Self::Int(i64::try_from(*n).ok()?),
            // A VT_DATE default surfaces as an F64 with the Date subtype; preserve
            // it so the synthesized argument materializes as a Date, not a Double.
            V::F64(f) if f.subtype() == oxvba_runtime::F64Subtype::Date => {
                Self::DateBits(f.as_f64().to_bits())
            }
            V::F64(f) => Self::FloatBits(f.as_f64().to_bits()),
            V::Currency(c) => Self::CurrencyScaled(c.scaled_i64()),
            V::String(s) => Self::Str(s.to_string()),
            V::ErrorCode(code) => Self::ErrorCode(*code),
            // A constant default is never an object, array, or Decimal.
            V::Object(_) | V::ArrayIntent(_) | V::Decimal(_) => return None,
        })
    }

    /// Materialize the default into the runtime [`Variant`] the vtable marshaller
    /// consumes for the synthesized positional argument.
    pub fn to_variant(&self) -> oxvba_runtime::Variant {
        use oxvba_runtime::Variant;
        match self {
            Self::Empty => Variant::empty(),
            Self::Null => Variant::null(),
            Self::Bool(b) => Variant::from_bool(*b),
            Self::Int(n) => Variant::from_i64(*n),
            Self::FloatBits(bits) => Variant::from_f64(f64::from_bits(*bits)),
            Self::CurrencyScaled(scaled) => Variant::from_currency_scaled_i64(*scaled),
            Self::DateBits(bits) => Variant::from_date_f64(f64::from_bits(*bits)),
            Self::Str(s) => Variant::from_string(s.clone()),
            Self::ErrorCode(code) => Variant::from_error_code(*code),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibMemberMetadata {
    pub name: String,
    pub token: i32,
    /// x64 vtable **slot index** (NOT a byte offset). The live loader divides
    /// `FUNCDESC::oVft` by 8 before storing it here, so this value can be used
    /// directly to index `(*(*this))[slot]` on x64. `None` for members without
    /// a vtable slot (pure dispinterface members).
    pub vtable_slot: Option<u16>,
    pub requires_argument: bool,
    pub invoke_kind: TypeLibMemberInvokeKind,
    pub parameter_names: Vec<String>,
    pub parameter_optional: Vec<bool>,
    /// Per-parameter (left-to-right, parallel to `parameter_types`) synthesis rule
    /// for an OMITTED trailing positional argument on the vtable slot path (D3).
    /// Empty for fixture/catalog metadata that never drives a real vtable call;
    /// the gate treats an empty vector as "no omitted-optional support" and only
    /// admits exact-arity calls, exactly as before D3.
    pub parameter_optional_defaults: Vec<OptionalParamDefault>,
    pub is_default_member: bool,
    pub parameter_types: Vec<TypeLibParamType>,
    /// ABI wire shapes that cannot be recovered from `parameter_types` alone.
    /// A missing/empty vector means older fixture metadata, so callers should
    /// fall back to `Automation(parameter_types[i])`.
    pub parameter_wire_types: Vec<TypeLibWireType>,
    /// Per-parameter interface IID (parallel to `parameter_types`), `Some` only for an
    /// OBJECT-typed parameter declared as a specific interface (`IFoo*`). Recovered from
    /// the FUNCDESC param `TYPEDESC` (`VT_PTR`/`VT_USERDEFINED` → `GetRefTypeInfo` →
    /// `GetTypeAttr` → guid). Empty for fixture/catalog metadata. Surfaced onto
    /// [`crate::ComMemberSpec::parameter_iids`] so the vtable marshaller can `QueryInterface`
    /// an object arg to its declared interface before passing it (Bug-4b).
    pub parameter_iids: Vec<Option<ComInterfaceIid>>,
    pub return_type: Option<TypeLibParamType>,
    pub return_wire_type: Option<TypeLibWireType>,
    /// True when `FUNCDESC::callconv == CC_STDCALL` (4) — the only convention
    /// the x64 vtable marshaller may call through. Fixture/catalog metadata
    /// that never drives a real vtable call leaves this `false`.
    pub callconv_is_stdcall: bool,
    /// True when the member's containing type carries `TYPEFLAG_FDUAL`, i.e. it
    /// is reachable both via `IDispatch::Invoke` and a custom-interface vtable
    /// slot. The vtable gate requires `is_dual && source_typekind == Interface`.
    pub is_dual: bool,
    /// IID of the member's **defining dual interface**. For the live-recovery
    /// FDUAL crossing this is the PARTNER `TKIND_INTERFACE`'s GUID (the IID the
    /// dispatch site `QueryInterface`s for); for a registered-typelib interface
    /// it is the containing `ITypeInfo`'s GUID. The dispatch site QIs the live
    /// object for this IID and calls the slot on the returned interface pointer.
    /// `None` for fixture/catalog metadata with no live interface identity.
    pub interface_iid: Option<ComInterfaceIid>,
    /// The `TKIND` the vtable metadata was sourced from. A vtable slot is only
    /// callable when this is `Interface` (a real custom vtable); a pure
    /// `dispinterface` member (`Dispatch`) has no slot. `None` for
    /// fixture/catalog metadata that never describes a live source type.
    pub source_typekind: Option<SourceTypeKind>,
    /// AV-safety bound: the highest valid slot index + 1, i.e. the source
    /// INTERFACE's `cbSizeVft / size_of::<*const c_void>()` (= `cbSizeVft/8` on
    /// x64). The gate requires `slot < vtable_slot_bound` so a slot can never
    /// over-run the live vtable (the access violation the probe root-caused).
    /// `None` when the bound is unknown (then the gate declines).
    pub vtable_slot_bound: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TypeLibMemberInvokeKind {
    PropertyGet,
    Method,
    PropertyPut,
    PropertyPutRef,
}

/// The `TKIND` of the `ITypeInfo` a member's vtable metadata (slot, IID,
/// `cbSizeVft` bound) was sourced from. A pure `dispinterface` member
/// (`TKIND_DISPATCH`) has NO vtable slot; only a real custom **interface**
/// (`TKIND_INTERFACE`, reached by crossing the FDUAL partner) carries a callable
/// vtable. The vtable gate admits a slot ONLY when the source is `Interface`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceTypeKind {
    /// `TKIND_INTERFACE` — a real vtable interface; slot is callable.
    Interface,
    /// `TKIND_DISPATCH` — a `dispinterface`; members have no vtable slot.
    Dispatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeLibEventDispatchPath {
    Dispatch,
    SourceInterface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibEventMetadata {
    pub name: String,
    pub token: i32,
    pub callback_arity: u8,
    pub dispatch_path: TypeLibEventDispatchPath,
    pub connection_point_iid: Option<String>,
    pub dispatch_member_id: Option<i32>,
    /// The COCLASS this event's source interface belongs to (e.g. `Application` for
    /// `AppEvents.NewWorkbook`), or `None` when not recovered from a coclass walk. A
    /// library-wide blob enumerates EVERY coclass into one `events` Vec, so the symbol
    /// provider uses this to scope a `WithEvents x As Lib.Coclass` field to only that
    /// coclass's source events instead of the whole library's union.
    pub coclass: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeLibCacheScope {
    Global,
    Reference,
}

pub fn runtime_class_descriptor_from_typelib_metadata(
    metadata: &TypeLibMetadataBlob,
) -> &'static RuntimeClassDescriptor {
    let members = metadata
        .members
        .iter()
        .map(|member| {
            let params = member
                .parameter_names
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    let (value_type, by_ref) = member
                        .parameter_types
                        .get(index)
                        .copied()
                        .map(runtime_value_type_from_typelib_param)
                        .unwrap_or((RuntimeValueType::Variant, false));
                    RuntimeParamDescriptor {
                        name: leak_typelib_runtime_descriptor_str(name.clone()),
                        value_type,
                        by_ref,
                        optional: member
                            .parameter_optional
                            .get(index)
                            .copied()
                            .unwrap_or(false),
                        param_array: false,
                    }
                })
                .collect::<Vec<_>>();
            RuntimeMemberDescriptor {
                name: leak_typelib_runtime_descriptor_str(member.name.clone()),
                dispatch_id: member.token,
                vtable_slot: member.vtable_slot,
                invoke_kind: runtime_invoke_kind_from_typelib_member(member.invoke_kind),
                arity: member.parameter_names.len(),
                params: Box::leak(params.into_boxed_slice()),
                return_type: member
                    .return_type
                    .map(|return_type| runtime_value_type_from_typelib_param(return_type).0),
                is_default_member: member.is_default_member,
            }
        })
        .collect::<Vec<_>>();
    let class_name = leak_typelib_runtime_descriptor_str(
        metadata
            .activation_prog_id
            .clone()
            .unwrap_or_else(|| metadata.identity.reference_name.clone()),
    );
    let interface_name = leak_typelib_runtime_descriptor_str(format!("{class_name}._Dispatch"));
    let members = Box::leak(members.into_boxed_slice());
    let interfaces = Box::leak(
        vec![RuntimeInterfaceDescriptor {
            id: RuntimeInterfaceId::IDispatch,
            identity: RUNTIME_IDISPATCH_INTERFACE_IDENTITY,
            name: interface_name,
            members,
            dual_dispatch: metadata
                .members
                .iter()
                .any(|member| member.vtable_slot.is_some()),
        }]
        .into_boxed_slice(),
    );
    Box::leak(Box::new(RuntimeClassDescriptor {
        name: class_name,
        interfaces,
    }))
}

fn runtime_invoke_kind_from_typelib_member(
    kind: TypeLibMemberInvokeKind,
) -> RuntimeMemberInvokeKind {
    match kind {
        TypeLibMemberInvokeKind::Method => RuntimeMemberInvokeKind::Method,
        TypeLibMemberInvokeKind::PropertyGet => RuntimeMemberInvokeKind::PropertyGet,
        TypeLibMemberInvokeKind::PropertyPut => RuntimeMemberInvokeKind::PropertyLet,
        TypeLibMemberInvokeKind::PropertyPutRef => RuntimeMemberInvokeKind::PropertySet,
    }
}

fn runtime_value_type_from_typelib_param(param_type: TypeLibParamType) -> (RuntimeValueType, bool) {
    match param_type {
        TypeLibParamType::Variant => (RuntimeValueType::Variant, false),
        TypeLibParamType::Long => (RuntimeValueType::Long, false),
        TypeLibParamType::Integer => (RuntimeValueType::Integer, false),
        TypeLibParamType::String => (RuntimeValueType::String, false),
        TypeLibParamType::Boolean => (RuntimeValueType::Boolean, false),
        TypeLibParamType::Double => (RuntimeValueType::Double, false),
        TypeLibParamType::Single => (RuntimeValueType::Single, false),
        TypeLibParamType::Currency => (RuntimeValueType::Currency, false),
        TypeLibParamType::Date => (RuntimeValueType::Date, false),
        TypeLibParamType::Decimal => (RuntimeValueType::Decimal, false),
        TypeLibParamType::Object => (RuntimeValueType::Object, false),
        TypeLibParamType::Byte => (RuntimeValueType::Byte, false),
        TypeLibParamType::LongLong => (RuntimeValueType::LongLong, false),
        TypeLibParamType::LongPtr => (RuntimeValueType::LongPtr, false),
        TypeLibParamType::ByRefVariant => (RuntimeValueType::Variant, true),
        TypeLibParamType::ByRefLong => (RuntimeValueType::Long, true),
        TypeLibParamType::ByRefInteger => (RuntimeValueType::Integer, true),
        TypeLibParamType::ByRefString => (RuntimeValueType::String, true),
        TypeLibParamType::ByRefDouble => (RuntimeValueType::Double, true),
        TypeLibParamType::ByRefSingle => (RuntimeValueType::Single, true),
        TypeLibParamType::ByRefCurrency => (RuntimeValueType::Currency, true),
        TypeLibParamType::ByRefDate => (RuntimeValueType::Date, true),
        TypeLibParamType::ByRefDecimal => (RuntimeValueType::Decimal, true),
        TypeLibParamType::ByRefObject => (RuntimeValueType::Object, true),
        TypeLibParamType::ByRefByte => (RuntimeValueType::Byte, true),
        TypeLibParamType::ByRefBoolean => (RuntimeValueType::Boolean, true),
        TypeLibParamType::ByRefLongLong => (RuntimeValueType::LongLong, true),
        TypeLibParamType::ByRefLongPtr => (RuntimeValueType::LongPtr, true),
    }
}

fn leak_typelib_runtime_descriptor_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}

#[cfg(test)]
mod tests {
    use oxvba_runtime::{RuntimeInterfaceId, RuntimeMemberInvokeKind, RuntimeValueType};

    use super::*;

    fn identity() -> TypeLibResolvedIdentity {
        TypeLibResolvedIdentity {
            reference_name: "TestLib".to_string(),
            requested_coclass: Some("Widget".to_string()),
            importlib: "TestLib".to_string(),
            libid: Some("{00000000-0000-0000-0000-000000000001}".to_string()),
            major_version: 1,
            minor_version: 0,
            lcid: Some(0),
            cache_key: "TestLib:1.0".to_string(),
        }
    }

    fn test_iid() -> ComInterfaceIid {
        ComInterfaceIid {
            data1: 0x1234_5678,
            data2: 0x1234,
            data3: 0x5678,
            data4: [1, 2, 3, 4, 5, 6, 7, 8],
        }
    }

    #[test]
    fn vtable_wire_signature_validator_accepts_supported_shapes() {
        assert_eq!(
            validate_vtable_wire_signature(
                &[TypeLibParamType::Long, TypeLibParamType::Object],
                &[
                    TypeLibWireType::Automation(TypeLibParamType::Long),
                    TypeLibWireType::InterfacePointer {
                        name: "ITest".to_string()
                    },
                ],
                &[None, Some(test_iid())],
                Some(TypeLibParamType::Object),
                Some(&TypeLibWireType::InterfacePointer {
                    name: "IResult".to_string()
                }),
            ),
            Ok(())
        );
        assert_eq!(
            validate_vtable_wire_signature(
                &[TypeLibParamType::Variant],
                &[TypeLibWireType::SafeArrayVariant],
                &[],
                Some(TypeLibParamType::Variant),
                Some(&TypeLibWireType::SafeArrayVariant),
            ),
            Ok(()),
            "explicit SAFEARRAY wire metadata is supported for Variant-shaped params and returns"
        );
    }

    #[test]
    fn vtable_wire_signature_validator_keeps_empty_wire_metadata_compatible() {
        assert_eq!(
            validate_vtable_wire_signature(
                &[TypeLibParamType::Long],
                &[],
                &[],
                Some(TypeLibParamType::Long),
                None,
            ),
            Ok(())
        );
    }

    #[test]
    fn vtable_wire_signature_validator_reports_arity_mismatch() {
        assert_eq!(
            validate_vtable_wire_signature(
                &[TypeLibParamType::Long],
                &[
                    TypeLibWireType::Automation(TypeLibParamType::Long),
                    TypeLibWireType::Automation(TypeLibParamType::Long),
                ],
                &[],
                None,
                None,
            ),
            Err(TypeLibVtableSignatureIssue::ParameterWireTypeArityMismatch)
        );
    }

    #[test]
    fn vtable_wire_signature_validator_rejects_unsupported_semantic_types() {
        assert_eq!(
            validate_vtable_wire_signature(
                &[TypeLibParamType::ByRefLong],
                &[TypeLibWireType::Automation(TypeLibParamType::ByRefLong)],
                &[],
                None,
                None,
            ),
            Ok(())
        );
        assert_eq!(
            validate_vtable_wire_signature(
                &[TypeLibParamType::LongPtr],
                &[TypeLibWireType::Automation(TypeLibParamType::LongPtr)],
                &[],
                None,
                None,
            ),
            Err(TypeLibVtableSignatureIssue::UnsupportedParameterType(
                TypeLibParamType::LongPtr
            ))
        );
        assert_eq!(
            validate_vtable_wire_signature(
                &[TypeLibParamType::Decimal],
                &[TypeLibWireType::Automation(TypeLibParamType::Decimal)],
                &[],
                None,
                None,
            ),
            Ok(())
        );
        assert_eq!(
            validate_vtable_wire_signature(
                &[],
                &[],
                &[],
                Some(TypeLibParamType::LongPtr),
                Some(&TypeLibWireType::Automation(TypeLibParamType::LongPtr)),
            ),
            Err(TypeLibVtableSignatureIssue::UnsupportedReturnType(
                TypeLibParamType::LongPtr
            ))
        );
        assert_eq!(
            validate_vtable_wire_signature(
                &[],
                &[],
                &[],
                Some(TypeLibParamType::Decimal),
                Some(&TypeLibWireType::Automation(TypeLibParamType::Decimal)),
            ),
            Ok(()),
            "DECIMAL retvals are supported as out cells"
        );
    }

    #[test]
    fn vtable_wire_signature_validator_rejects_unsupported_wire_shapes() {
        assert_eq!(
            validate_vtable_wire_signature(
                &[TypeLibParamType::Variant],
                &[TypeLibWireType::ByRefSafeArrayVariant],
                &[],
                None,
                None,
            ),
            Ok(())
        );
        assert_eq!(
            validate_vtable_wire_signature(
                &[],
                &[],
                &[],
                Some(TypeLibParamType::Variant),
                Some(&TypeLibWireType::ByRefSafeArrayVariant),
            ),
            Err(TypeLibVtableSignatureIssue::UnsupportedReturnWireType)
        );
    }

    #[test]
    fn vtable_wire_signature_validator_requires_object_parameter_iid() {
        assert_eq!(
            validate_vtable_wire_signature(
                &[TypeLibParamType::Object],
                &[TypeLibWireType::InterfacePointer {
                    name: "ITest".to_string()
                }],
                &[None],
                None,
                None,
            ),
            Err(TypeLibVtableSignatureIssue::MissingObjectParameterIid)
        );
    }

    #[test]
    fn typelib_metadata_projects_to_runtime_dispatch_descriptor() {
        let metadata = TypeLibMetadataBlob {
            identity: identity(),
            activation_prog_id: Some("OxVba.TestDispatch".to_string()),
            member_name_to_token: vec![("Value".to_string(), 0)],
            members: vec![
                TypeLibMemberMetadata {
                    name: "Value".to_string(),
                    token: 0,
                    vtable_slot: Some(7),
                    requires_argument: false,
                    invoke_kind: TypeLibMemberInvokeKind::PropertyGet,
                    parameter_names: Vec::new(),
                    parameter_optional: Vec::new(),
                    parameter_optional_defaults: Vec::new(),
                    is_default_member: true,
                    parameter_types: Vec::new(),
                    parameter_wire_types: Vec::new(),
                    parameter_iids: Vec::new(),
                    return_type: Some(TypeLibParamType::Long),
                    return_wire_type: None,
                    callconv_is_stdcall: true,
                    is_dual: true,
                    interface_iid: None,
                    source_typekind: Some(SourceTypeKind::Interface),
                    vtable_slot_bound: Some(64),
                },
                TypeLibMemberMetadata {
                    name: "Item".to_string(),
                    token: 5,
                    vtable_slot: None,
                    requires_argument: true,
                    invoke_kind: TypeLibMemberInvokeKind::Method,
                    parameter_names: vec!["index".to_string()],
                    parameter_optional: vec![true],
                    parameter_optional_defaults: Vec::new(),
                    is_default_member: false,
                    parameter_types: vec![TypeLibParamType::Variant],
                    parameter_wire_types: Vec::new(),
                    parameter_iids: Vec::new(),
                    return_type: Some(TypeLibParamType::Variant),
                    return_wire_type: None,
                    callconv_is_stdcall: false,
                    is_dual: true,
                    interface_iid: None,
                    source_typekind: Some(SourceTypeKind::Interface),
                    vtable_slot_bound: Some(64),
                },
            ],
            events: Vec::new(),
            coclass_names: Vec::new(),
        };

        let descriptor = runtime_class_descriptor_from_typelib_metadata(&metadata);
        assert_eq!(descriptor.name, "OxVba.TestDispatch");
        assert_eq!(descriptor.interfaces.len(), 1);
        let dispatch = descriptor
            .interfaces
            .iter()
            .find(|interface| interface.id == RuntimeInterfaceId::IDispatch)
            .expect("typelib projection should expose dispatch descriptor metadata");
        assert!(
            dispatch.dual_dispatch,
            "explicit vtable slots project a dual-interface descriptor"
        );
        assert_eq!(dispatch.name, "OxVba.TestDispatch._Dispatch");
        assert_eq!(dispatch.members.len(), 2);
        assert_eq!(dispatch.members[0].name, "Value");
        assert_eq!(dispatch.members[0].dispatch_id, 0);
        assert_eq!(dispatch.members[0].vtable_slot, Some(7));
        assert_eq!(
            dispatch.members[0].invoke_kind,
            RuntimeMemberInvokeKind::PropertyGet
        );
        assert_eq!(dispatch.members[0].arity, 0);
        assert!(dispatch.members[0].is_default_member);
        assert_eq!(dispatch.members[1].name, "Item");
        assert_eq!(dispatch.members[1].dispatch_id, 5);
        assert_eq!(
            dispatch.members[1].invoke_kind,
            RuntimeMemberInvokeKind::Method
        );
        assert_eq!(dispatch.members[1].arity, 1);
        assert_eq!(dispatch.members[1].params.len(), 1);
        assert_eq!(dispatch.members[1].params[0].name, "index");
        assert_eq!(
            dispatch.members[1].params[0].value_type,
            RuntimeValueType::Variant
        );
        assert!(!dispatch.members[1].params[0].by_ref);
        assert!(dispatch.members[1].params[0].optional);
        assert_eq!(
            dispatch.members[1].return_type,
            Some(RuntimeValueType::Variant)
        );
        assert_eq!(
            dispatch.members[0].return_type,
            Some(RuntimeValueType::Long)
        );
    }
}
