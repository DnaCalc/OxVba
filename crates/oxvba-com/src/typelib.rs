#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibResolveRequest {
    pub reference_name: String,
    pub importlib_hint: Option<String>,
    pub libid_hint: Option<String>,
    pub major_version_hint: Option<u16>,
    pub minor_version_hint: Option<u16>,
    pub lcid_hint: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeLibResolvedIdentity {
    pub reference_name: String,
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
    pub create_object_selector: Option<i32>,
    pub member_name_to_token: Vec<(String, i32)>,
    pub members: Vec<TypeLibMemberMetadata>,
    pub events: Vec<TypeLibEventMetadata>,
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
pub struct TypeLibMemberMetadata {
    pub name: String,
    pub token: i32,
    pub requires_argument: bool,
    pub invoke_kind: TypeLibMemberInvokeKind,
    pub parameter_names: Vec<String>,
    pub is_default_member: bool,
    pub parameter_types: Vec<TypeLibParamType>,
    pub return_type: Option<TypeLibParamType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeLibMemberInvokeKind {
    PropertyGet,
    Method,
    PropertyPut,
    PropertyPutRef,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeLibCacheScope {
    Global,
    Reference,
}
