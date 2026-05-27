use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
};

use oxvba_runtime::DynLinkSymbol;

use crate::{
    bytecode::{
        Bytecode, ComMemberCallDescriptor, ComMemberSelectorDescriptor, DeclareParamType,
        DispatchInvokeArg, ExternalCallDescriptor, ExternalCallWriteback,
        ExternalCallWritebackKind, Instruction, ProjectMemberCallDescriptor, ProjectMemberCallKind,
        RuntimeArrayElementType, RuntimeAssignmentIntent, RuntimeAssignmentTargetKind,
        StringCompareMode,
    },
    descriptor_identity::{DescriptorFamily, canonical_descriptor_id},
    resolve::{
        ArithOp, AssignmentIntent, BoundCallArg, BoundCallSyntax, BoundCaseClause,
        BoundCompareMode, BoundCond, BoundEnumDescriptor, BoundExpr, BoundExternalDecl,
        BoundModule, BoundParam, BoundParamSourceMechanism, BoundProcedure, BoundStmt, BoundType,
        CompareOp,
    },
};

fn emit_compare_mode(mode: BoundCompareMode) -> StringCompareMode {
    match mode {
        BoundCompareMode::Binary | BoundCompareMode::Database => StringCompareMode::Binary,
        BoundCompareMode::Text => StringCompareMode::Text,
    }
}

#[derive(Debug, Clone)]
struct EmitProcMeta {
    params: Vec<BoundParam>,
    slots: HashMap<String, usize>,
    return_slot: Option<usize>,
    return_type: BoundType,
    declaration_types: HashMap<String, BoundType>,
}

fn normalize_runtime_name_key(name: &str) -> String {
    name.to_ascii_lowercase()
}

fn insert_casefold_key<V: Clone>(map: &mut HashMap<String, V>, name: &str, value: V) {
    map.insert(name.to_string(), value.clone());
    let folded = normalize_runtime_name_key(name);
    if folded != name {
        map.insert(folded, value);
    }
}

fn lookup_casefold_key<'a, V>(map: &'a HashMap<String, V>, name: &str) -> Option<&'a V> {
    map.get(name)
        .or_else(|| map.get(&normalize_runtime_name_key(name)))
}

fn com_member_call_descriptor_for_dispatch_intrinsic(
    member: &BoundExpr,
    arity: usize,
    early_bound: bool,
) -> Option<ComMemberCallDescriptor> {
    if !early_bound {
        return None;
    }
    let selector = match member {
        BoundExpr::IntConst(dispatch_id) => ComMemberSelectorDescriptor::DispatchId(*dispatch_id),
        BoundExpr::StringConst(name) => ComMemberSelectorDescriptor::Name(name.clone()),
        _ => return None,
    };
    Some(ComMemberCallDescriptor { selector, arity })
}

fn project_member_call_descriptor_for_proc_name(
    proc_name: &str,
) -> Option<ProjectMemberCallDescriptor> {
    let (kind, lowered_name) = if let Some(rest) = proc_name.strip_prefix("property_get_pmr_") {
        (ProjectMemberCallKind::PropertyGet, format!("pmr_{rest}"))
    } else if let Some(rest) = proc_name.strip_prefix("property_let_pmr_") {
        (ProjectMemberCallKind::PropertyLet, format!("pmr_{rest}"))
    } else if let Some(rest) = proc_name.strip_prefix("property_set_pmr_") {
        (ProjectMemberCallKind::PropertySet, format!("pmr_{rest}"))
    } else if proc_name.starts_with("pmr_") {
        (ProjectMemberCallKind::Method, proc_name.to_string())
    } else {
        return None;
    };
    Some(ProjectMemberCallDescriptor { lowered_name, kind })
}

thread_local! {
    static CURRENT_PROC_NAME: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn with_current_proc_name<T>(name: &str, f: impl FnOnce() -> T) -> T {
    CURRENT_PROC_NAME.with(|current| {
        *current.borrow_mut() = Some(name.to_string());
        let result = f();
        *current.borrow_mut() = None;
        result
    })
}

fn current_proc_meta(proc_meta: &HashMap<String, EmitProcMeta>) -> Option<&EmitProcMeta> {
    CURRENT_PROC_NAME.with(|current| {
        let name = current.borrow();
        lookup_casefold_key(proc_meta, name.as_deref()?)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ProcedureRuntimeSlotKind {
    Parameter,
    Local,
    ReturnValue,
    Temporary,
    CompilerGenerated,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ProcedureRuntimeSlotMetadata {
    pub name: String,
    pub slot: usize,
    pub kind: ProcedureRuntimeSlotKind,
    pub declared_type: VbaTypeId,
    pub initial_state: SlotInitialState,
    pub carrier: RuntimeCarrierKind,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ProcedureRuntimeMetadata {
    pub module_name: String,
    pub procedure_name: String,
    pub entry_pc: usize,
    pub source_line_start: usize,
    pub source_line_end: usize,
    pub statement_line_numbers: Vec<usize>,
    pub statement_entry_pcs: Vec<usize>,
    pub slots: Vec<ProcedureRuntimeSlotMetadata>,
    pub param_slots: Vec<usize>,
    pub return_slot: Option<usize>,
    pub param_types: Vec<DeclareParamType>,
    pub return_type: Option<DeclareParamType>,
    pub signature: ProcedureSignatureDescriptor,
    pub call_sites: Vec<CallSiteDescriptor>,
    pub array_shapes: Vec<ArrayShapeDescriptor>,
    pub udt_types: Vec<UdtTypeDescriptor>,
    pub object_types: Vec<ObjectTypeDescriptor>,
    pub carrier_layouts: Vec<CarrierLayoutDescriptor>,
    pub value_states: Vec<ValueStateDescriptor>,
    pub expression_semantics: Vec<ExpressionSemanticsDescriptor>,
    pub operator_semantics: Vec<OperatorSemanticsDescriptor>,
    pub coercions: Vec<CoercionDescriptor>,
    pub name_bindings: Vec<NameBindingDescriptor>,
    pub object_member_bindings: Vec<ObjectMemberBindingDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct CarrierLayoutDescriptor {
    pub descriptor_id: String,
    pub carrier: RuntimeCarrierKind,
    pub carrier_key: String,
    pub layout: CarrierLayoutKind,
    pub storage_bits: Option<u16>,
    pub native_frame_eligible: bool,
    pub variant_compatible: bool,
    pub com_variant_type: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum CarrierLayoutKind {
    Unknown,
    NativeScalar,
    VariantPayload,
    BStr,
    ObjectRef,
    SafeArray,
    UdtFields,
    InternalHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ValueStateDescriptor {
    pub descriptor_id: String,
    pub state: ValueStateKind,
    pub source: ValueStateSource,
    pub slot: Option<usize>,
    pub pc: Option<usize>,
    pub name: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ValueStateKind {
    Empty,
    Null,
    Error,
    MissingArgument,
    OmittedDefault,
    Nothing,
    VbNullString,
    DecimalVariantSubtype,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ValueStateSource {
    SlotInitialState,
    Instruction,
    OptionalParameter,
    IntrinsicConstant,
    DeclaredTypeExtension,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ExpressionSemanticsDescriptor {
    pub descriptor_id: String,
    pub expression_id: String,
    pub classification: ExpressionClassificationDescriptor,
    pub declared_type: VbaTypeId,
    pub value_states: Vec<ValueStateKind>,
    pub carrier_hint: RuntimeCarrierKind,
    pub default_member_policy: DefaultMemberPolicyDescriptor,
    pub source_context: ExpressionSourceContextDescriptor,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ExpressionClassificationDescriptor {
    Unknown,
    Literal,
    Variable,
    ArrayElement,
    FunctionResult,
    IntrinsicResult,
    OperatorResult,
    ComparisonResult,
    TruthyPredicate,
    Temporary,
    PointerBuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ExpressionSourceContextDescriptor {
    Unknown,
    AssignmentRhs,
    RuntimeArrayIndex,
    RuntimeArrayValue,
    CallArgument,
    Condition,
    LoopBound,
    PrintOrIo,
    ReturnValue,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct OperatorSemanticsDescriptor {
    pub descriptor_id: String,
    pub operator_id: String,
    pub family: OperatorFamilyDescriptor,
    pub operator: VbaOperatorDescriptor,
    pub left_declared_type: Option<VbaTypeId>,
    pub right_declared_type: Option<VbaTypeId>,
    pub result_declared_type: VbaTypeId,
    pub result_value_states: Vec<ValueStateKind>,
    pub compare_mode: Option<OperatorCompareModeDescriptor>,
    pub helper_id: String,
    pub runtime_error_policy: String,
    pub evaluation_order: EvaluationOrderDescriptor,
    pub current_vm_status: String,
    pub gap_classification: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum OperatorFamilyDescriptor {
    Arithmetic,
    Concatenation,
    Comparison,
    Logical,
    Truthiness,
    BranchPredicate,
    EvaluationOrder,
    ImplementationFastPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum VbaOperatorDescriptor {
    Add,
    Subtract,
    Multiply,
    Divide,
    IntegerDivide,
    Modulo,
    Power,
    Concatenate,
    Negate,
    Equal,
    NotEqual,
    LessThan,
    LessOrEqual,
    GreaterThan,
    GreaterOrEqual,
    Like,
    Not,
    And,
    Or,
    Predicate,
    IIf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum OperatorCompareModeDescriptor {
    Binary,
    Text,
    Database,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum EvaluationOrderDescriptor {
    LeftToRightEager,
    PredicateBranch,
    RuntimeHelperDefined,
    UnsupportedDeferred,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct CoercionDescriptor {
    pub descriptor_id: String,
    pub coercion_id: String,
    pub kind: CoercionKindDescriptor,
    pub source_declared_type: VbaTypeId,
    pub source_value_states: Vec<ValueStateKind>,
    pub target_declared_type: VbaTypeId,
    pub static_status: CoercionStaticStatusDescriptor,
    pub runtime_failure: RuntimeFailurePolicyDescriptor,
    pub helper_id: String,
    pub evidence_anchor: String,
    pub gap_classification: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum CoercionKindDescriptor {
    Let,
    Set,
    CallLet,
    PropertyValueAssignment,
    ExplicitConversionIntrinsic,
    HelperCompatibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum CoercionStaticStatusDescriptor {
    Identity,
    Runtime,
    DescriptorRequired,
    Invalid,
    Deferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum RuntimeFailurePolicyDescriptor {
    None,
    TypeMismatch,
    NumericError,
    HelperSpecific,
    UnsupportedDeferred,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct NameBindingDescriptor {
    pub descriptor_id: String,
    pub binding_id: String,
    pub name: String,
    pub binding_kind: NameBindingKindDescriptor,
    pub precedence: NameBindingPrecedenceDescriptor,
    pub target: Option<String>,
    pub diagnostics: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum NameBindingKindDescriptor {
    Policy,
    Parameter,
    Local,
    ReturnValue,
    Procedure,
    PropertyAccessor,
    ModuleScope,
    AmbiguityDiagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum NameBindingPrecedenceDescriptor {
    LocalProcedureScope,
    ModuleScope,
    ProjectProcedure,
    PropertyAccessor,
    WithContextDeferred,
    ImportedMemberDeferred,
    DefaultMemberDeferred,
    CompilerDiagnostic,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ObjectMemberBindingDescriptor {
    pub descriptor_id: String,
    pub binding_id: String,
    pub target_declared_type: VbaTypeId,
    pub member_name: String,
    pub member_kind: ObjectMemberKindDescriptor,
    pub default_member: bool,
    pub property_accessor_group: Option<String>,
    pub dispatch_kind: MemberDispatchKindDescriptor,
    pub argument_binding_policy: String,
    pub result_declared_type: Option<VbaTypeId>,
    pub object_identity_policy: String,
    pub cache_invalidation_policy: String,
    pub fallback_or_unsupported_policy: String,
    pub current_vm_status: String,
    pub gap_classification: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ObjectMemberKindDescriptor {
    SetAssignment,
    PropertyGet,
    PropertyLet,
    PropertySet,
    DefaultMemberGet,
    Method,
    Activation,
    Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum MemberDispatchKindDescriptor {
    None,
    ProjectRoute,
    DefaultMemberFallback,
    DispatchInvoke,
    TypelibBackedDispatch,
    ProjectEventRoute,
    MetadataOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ArrayShapeDescriptor {
    pub name: String,
    pub base_slot: Option<usize>,
    pub element_type: VbaTypeId,
    pub element_carrier: RuntimeCarrierKind,
    pub rank: usize,
    pub bounds: Vec<ArrayBoundDescriptor>,
    pub storage: ArrayStorageKind,
    pub option_base: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ArrayBoundDescriptor {
    pub dimension: usize,
    pub lower_bound: i32,
    pub upper_bound: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ArrayStorageKind {
    StaticFixed,
    Dynamic,
    ParamArrayPack,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct UdtTypeDescriptor {
    pub descriptor_id: String,
    pub type_name: String,
    pub instances: Vec<UdtInstanceDescriptor>,
    pub fields: Vec<UdtFieldDescriptor>,
    pub storage: UdtStorageKind,
    pub copy_semantics: UdtCopySemanticsDescriptor,
    pub cleanup: UdtCleanupDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct UdtInstanceDescriptor {
    pub name: String,
    pub base_slot: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct UdtFieldDescriptor {
    pub field_id: String,
    pub index: usize,
    pub name: String,
    pub declared_type: VbaTypeId,
    pub carrier: RuntimeCarrierKind,
    pub nested_udt_name: Option<String>,
    pub fixed_string_len: Option<usize>,
    pub array_bounds: Vec<ArrayBoundDescriptor>,
    pub aliases: Vec<UdtFieldAliasDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct UdtFieldAliasDescriptor {
    pub instance_name: String,
    pub slot_name: String,
    pub slot: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum UdtStorageKind {
    FlattenedFieldSlots,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum UdtCopySemanticsDescriptor {
    FieldWiseCopy,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct UdtCleanupDescriptor {
    pub owns_bstr: bool,
    pub owns_object_ref: bool,
    pub owns_safearray: bool,
    pub owns_variant: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ObjectTypeDescriptor {
    pub descriptor_id: String,
    pub type_name: String,
    pub kind: ObjectTypeDescriptorKind,
    pub carrier: RuntimeCarrierKind,
    pub initial_state: SlotInitialState,
    pub activation: ObjectActivationDescriptor,
    pub event_binding: ObjectEventBindingDescriptor,
    pub default_member: ObjectDefaultMemberDescriptor,
    pub support: ObjectDescriptorSupport,
    pub instances: Vec<ObjectInstanceDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ObjectInstanceDescriptor {
    pub name: String,
    pub slot: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ObjectTypeDescriptorKind {
    Object,
    VbaClass,
    VbaInterface,
    ComClass,
    ComInterface,
    WithEventsObject,
    AsNewObject,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ObjectActivationDescriptor {
    None,
    AsNew,
    CreateObjectProgId { prog_id: String },
    HostProvided,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ObjectEventBindingDescriptor {
    None,
    WithEvents {
        binding_token: Option<i32>,
        event_source: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ObjectDefaultMemberDescriptor {
    Unknown,
    NoDefaultMember,
    HasDefaultMember,
    DefaultInstance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ObjectDescriptorSupport {
    VmRunnable,
    VmRunnableHosted,
    MetadataOnly,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct CallSiteDescriptor {
    pub call_site_id: String,
    pub caller_procedure_name: String,
    pub call_pc: usize,
    pub target_name: String,
    pub target_kind: CallTargetKindDescriptor,
    pub target_entry_pc: Option<usize>,
    pub default_member_policy: DefaultMemberPolicyDescriptor,
    pub invocation_syntax: CallInvocationSyntaxDescriptor,
    pub argument_evaluation_order: Vec<usize>,
    pub arguments: Vec<ArgumentBindingDescriptor>,
    pub return_value: Option<CallReturnDescriptor>,
    pub diagnostic_policies: Vec<CallDiagnosticPolicyDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum CallTargetKindDescriptor {
    Unknown,
    Procedure,
    Function,
    PropertyGet,
    PropertyLet,
    PropertySet,
    ExternalDeclare,
    LateBoundDefaultMember,
    DispatchInvoke,
    EarlyBoundCom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum DefaultMemberPolicyDescriptor {
    Unknown,
    NotApplicable,
    ExplicitMember,
    DefaultMemberFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum CallInvocationSyntaxDescriptor {
    Unknown,
    StatementNoCall,
    StatementCallKeyword,
    ExpressionCall,
    SyntheticPropertyAssignment,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct CallDiagnosticPolicyDescriptor {
    pub diagnostic: CallDiagnosticKindDescriptor,
    pub owner: CallDiagnosticOwnerDescriptor,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum CallDiagnosticKindDescriptor {
    MissingRequiredArgument,
    TooManyArguments,
    UnknownNamedArgument,
    DuplicateArgumentMapping,
    PositionalAfterNamed,
    NamedParamArray,
    OptionalVariantMissingRuntimeState,
    ByRefExpressionNoWriteback,
    CallEntryCoercion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum CallDiagnosticOwnerDescriptor {
    CompilerCurrent,
    VmRuntimeCurrent,
    PackageDescriptorSelected,
    VmDeferred,
    OracleNeeded,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ArgumentBindingDescriptor {
    pub argument_index: usize,
    pub source_index: Option<usize>,
    pub source_name: Option<String>,
    pub parameter_index: Option<usize>,
    pub parameter_name: Option<String>,
    pub parameter_slot: Option<usize>,
    pub source_kind: ArgumentSourceKindDescriptor,
    pub expression_kind: ArgumentExpressionKindDescriptor,
    pub binding_kind: ArgumentBindingKindDescriptor,
    pub force_byval: bool,
    pub source_slot: Option<usize>,
    pub writeback: Option<ArgumentWritebackDescriptor>,
    pub optional_default: Option<OptionalDefaultValue>,
    pub param_array: Option<ParamArrayBindingDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ArgumentSourceKindDescriptor {
    Unknown,
    Positional,
    Named,
    Omitted,
    ParamArrayPack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ArgumentExpressionKindDescriptor {
    Unknown,
    Variable,
    Literal,
    Expression,
    IntrinsicCall,
    ProcedureCall,
    ArrayBuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ArgumentBindingKindDescriptor {
    Unknown,
    ByRefAlias,
    ByValCopy,
    OptionalDefault,
    ParamArrayPack,
    FixedArrayMaterialized,
    ByRefExpressionTemp,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ArgumentWritebackDescriptor {
    pub caller_slot: Option<usize>,
    pub parameter_slot: Option<usize>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ParamArrayBindingDescriptor {
    pub element_count: usize,
    pub element_slots: Vec<usize>,
    pub lower_bound: i32,
    pub empty_upper_bound: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct CallReturnDescriptor {
    pub return_slot: Option<usize>,
    pub assign_target_slot: Option<usize>,
    pub copyout_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ProcedureSignatureDescriptor {
    pub procedure_name: String,
    pub kind: ProcedureKindDescriptor,
    pub parameters: Vec<ParameterDescriptor>,
    pub return_type: Option<VbaTypeId>,
    pub return_slot: Option<usize>,
    pub property_group: Option<String>,
    pub implicit_current_object: Option<ImplicitCurrentObjectDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ProcedureKindDescriptor {
    Unknown,
    Sub,
    Function,
    PropertyGet,
    PropertyLet,
    PropertySet,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ParameterDescriptor {
    pub index: usize,
    pub name: String,
    pub slot: Option<usize>,
    pub role: ParameterRole,
    pub source_mechanism: SourceParameterMechanism,
    pub resolved_mechanism: ResolvedParameterMechanism,
    pub passing_mode: ParameterPassingMode,
    pub declared_type: VbaTypeId,
    pub optional: bool,
    pub param_array: bool,
    pub default_value: Option<i32>,
    pub optional_descriptor: OptionalParameterDescriptor,
    pub param_array_descriptor: Option<ParamArrayDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ParameterRole {
    Positional,
    Optional,
    ParamArray,
    PropertyValue,
    ImplicitCurrentObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ParameterPassingMode {
    Unknown,
    ByRef,
    ByVal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum SourceParameterMechanism {
    Unknown,
    Omitted,
    ExplicitByRef,
    ExplicitByVal,
    ImplementationInjected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum ResolvedParameterMechanism {
    Unknown,
    ByRef,
    ByVal,
    PropertyValueByVal,
    EventSignatureOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum OptionalParameterDescriptor {
    Required,
    Optional {
        default_value: OptionalDefaultValue,
        missing_state: OptionalMissingStatePolicy,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum OptionalDefaultValue {
    Unknown,
    ExplicitI32(i32),
    DeclaredTypeDefault,
    VariantMissingError448,
    ImplementationDefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum OptionalMissingStatePolicy {
    Unknown,
    AssignDefaultLocal,
    PreserveMissingArgumentState,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ParamArrayDescriptor {
    pub element_type: VbaTypeId,
    pub array_lower_bound: i32,
    pub empty_upper_bound: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct ImplicitCurrentObjectDescriptor {
    pub declared_type: VbaTypeId,
    pub mechanism: ResolvedParameterMechanism,
    pub accessible_name: String,
    pub assignable: bool,
    pub slot: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum VbaTypeId {
    Unknown,
    Boolean,
    Byte,
    Integer,
    Long,
    LongLong,
    LongPtr,
    Single,
    Double,
    Currency,
    Date,
    String,
    Variant,
    Object,
    Array,
    InteropAny,
}

impl From<DeclareParamType> for VbaTypeId {
    fn from(value: DeclareParamType) -> Self {
        match value {
            DeclareParamType::Boolean => Self::Boolean,
            DeclareParamType::Byte => Self::Byte,
            DeclareParamType::Integer => Self::Integer,
            DeclareParamType::Long => Self::Long,
            DeclareParamType::LongLong => Self::LongLong,
            DeclareParamType::LongPtr => Self::LongPtr,
            DeclareParamType::Single => Self::Single,
            DeclareParamType::Double => Self::Double,
            DeclareParamType::Currency => Self::Currency,
            DeclareParamType::Date => Self::Date,
            DeclareParamType::String => Self::String,
            DeclareParamType::Variant => Self::Variant,
            DeclareParamType::Any => Self::InteropAny,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum SlotRole {
    Parameter,
    Local,
    ReturnValue,
    Temporary,
    CompilerGenerated,
}

impl From<ProcedureRuntimeSlotKind> for SlotRole {
    fn from(value: ProcedureRuntimeSlotKind) -> Self {
        match value {
            ProcedureRuntimeSlotKind::Parameter => Self::Parameter,
            ProcedureRuntimeSlotKind::Local => Self::Local,
            ProcedureRuntimeSlotKind::ReturnValue => Self::ReturnValue,
            ProcedureRuntimeSlotKind::Temporary => Self::Temporary,
            ProcedureRuntimeSlotKind::CompilerGenerated => Self::CompilerGenerated,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum SlotInitialState {
    Unknown,
    CallerProvided,
    Empty,
    ScalarZero,
    False,
    EmptyString,
    Nothing,
    UnallocatedArray,
    UdtDefault,
    CompilerDefined,
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum RuntimeCarrierKind {
    Unknown,
    Variant,
    Boolean,
    I16,
    U8,
    I32,
    I64,
    PointerSizedInteger,
    F32,
    F64,
    Currency,
    Date,
    BStr,
    Decimal96VariantSubtype,
    ObjectRef,
    SafeArray,
    UdtFields { descriptor: String },
    BindingHandleInternal,
}

impl RuntimeCarrierKind {
    pub fn for_declared_type(declared_type: VbaTypeId) -> Self {
        match declared_type {
            VbaTypeId::Unknown | VbaTypeId::InteropAny => Self::Unknown,
            VbaTypeId::Boolean => Self::Boolean,
            VbaTypeId::Byte => Self::U8,
            VbaTypeId::Integer => Self::I16,
            VbaTypeId::Long => Self::I32,
            VbaTypeId::LongLong => Self::I64,
            VbaTypeId::LongPtr => Self::PointerSizedInteger,
            VbaTypeId::Single => Self::F32,
            VbaTypeId::Double => Self::F64,
            VbaTypeId::Currency => Self::Currency,
            VbaTypeId::Date => Self::Date,
            VbaTypeId::String => Self::BStr,
            VbaTypeId::Variant => Self::Variant,
            VbaTypeId::Object => Self::ObjectRef,
            VbaTypeId::Array => Self::SafeArray,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct SlotTypeDescriptor {
    pub slot: usize,
    pub name: Option<String>,
    pub role: SlotRole,
    pub declared_type: VbaTypeId,
    pub initial_state: SlotInitialState,
    pub carrier: RuntimeCarrierKind,
}

impl SlotInitialState {
    pub fn for_slot(role: SlotRole, declared_type: VbaTypeId) -> Self {
        match role {
            SlotRole::Parameter => Self::CallerProvided,
            SlotRole::Temporary => Self::CompilerDefined,
            SlotRole::Local | SlotRole::ReturnValue | SlotRole::CompilerGenerated => {
                Self::default_for_declared_type(declared_type)
            }
        }
    }

    fn default_for_declared_type(declared_type: VbaTypeId) -> Self {
        match declared_type {
            VbaTypeId::Unknown | VbaTypeId::InteropAny => Self::Unknown,
            VbaTypeId::Variant => Self::Empty,
            VbaTypeId::Boolean => Self::False,
            VbaTypeId::Byte
            | VbaTypeId::Integer
            | VbaTypeId::Long
            | VbaTypeId::LongLong
            | VbaTypeId::LongPtr
            | VbaTypeId::Single
            | VbaTypeId::Double
            | VbaTypeId::Currency
            | VbaTypeId::Date => Self::ScalarZero,
            VbaTypeId::String => Self::EmptyString,
            VbaTypeId::Object => Self::Nothing,
            VbaTypeId::Array => Self::UnallocatedArray,
        }
    }
}

impl ProcedureRuntimeSlotMetadata {
    pub fn new(
        name: String,
        slot: usize,
        kind: ProcedureRuntimeSlotKind,
        declared_type: VbaTypeId,
    ) -> Self {
        Self::new_with_carrier(
            name,
            slot,
            kind,
            declared_type,
            RuntimeCarrierKind::for_declared_type(declared_type),
        )
    }

    pub fn new_with_carrier(
        name: String,
        slot: usize,
        kind: ProcedureRuntimeSlotKind,
        declared_type: VbaTypeId,
        carrier: RuntimeCarrierKind,
    ) -> Self {
        let role = SlotRole::from(kind);
        Self {
            name,
            slot,
            kind,
            declared_type,
            initial_state: SlotInitialState::for_slot(role, declared_type),
            carrier,
        }
    }

    pub fn new_with_initial_state_and_carrier(
        name: String,
        slot: usize,
        kind: ProcedureRuntimeSlotKind,
        declared_type: VbaTypeId,
        initial_state: SlotInitialState,
        carrier: RuntimeCarrierKind,
    ) -> Self {
        Self {
            name,
            slot,
            kind,
            declared_type,
            initial_state,
            carrier,
        }
    }

    pub fn slot_type_descriptor(&self) -> SlotTypeDescriptor {
        SlotTypeDescriptor {
            slot: self.slot,
            name: Some(self.name.clone()),
            role: SlotRole::from(self.kind),
            declared_type: self.declared_type,
            initial_state: self.initial_state,
            carrier: self.carrier.clone(),
        }
    }
}

impl ProcedureRuntimeMetadata {
    pub fn procedure_signature_descriptor(&self) -> ProcedureSignatureDescriptor {
        self.signature.clone()
    }

    pub fn slot_type_descriptors(&self) -> Vec<SlotTypeDescriptor> {
        self.slots
            .iter()
            .map(ProcedureRuntimeSlotMetadata::slot_type_descriptor)
            .collect()
    }

    pub fn array_shape_descriptors(&self) -> Vec<ArrayShapeDescriptor> {
        self.array_shapes.clone()
    }

    pub fn udt_type_descriptors(&self) -> Vec<UdtTypeDescriptor> {
        self.udt_types.clone()
    }

    pub fn object_type_descriptors(&self) -> Vec<ObjectTypeDescriptor> {
        self.object_types.clone()
    }

    pub fn carrier_layout_descriptors(&self) -> Vec<CarrierLayoutDescriptor> {
        self.carrier_layouts.clone()
    }

    pub fn value_state_descriptors(&self) -> Vec<ValueStateDescriptor> {
        self.value_states.clone()
    }

    pub fn expression_semantics_descriptors(&self) -> Vec<ExpressionSemanticsDescriptor> {
        self.expression_semantics.clone()
    }

    pub fn operator_semantics_descriptors(&self) -> Vec<OperatorSemanticsDescriptor> {
        self.operator_semantics.clone()
    }

    pub fn coercion_descriptors(&self) -> Vec<CoercionDescriptor> {
        self.coercions.clone()
    }

    pub fn name_binding_descriptors(&self) -> Vec<NameBindingDescriptor> {
        self.name_bindings.clone()
    }

    pub fn object_member_binding_descriptors(&self) -> Vec<ObjectMemberBindingDescriptor> {
        self.object_member_bindings.clone()
    }

    pub(crate) fn legacy_declared_type_for_slot(
        &self,
        slot: usize,
        kind: ProcedureRuntimeSlotKind,
    ) -> VbaTypeId {
        match kind {
            ProcedureRuntimeSlotKind::Parameter => self
                .param_slots
                .iter()
                .position(|candidate| *candidate == slot)
                .and_then(|index| self.param_types.get(index).copied())
                .map(VbaTypeId::from)
                .unwrap_or(VbaTypeId::Unknown),
            ProcedureRuntimeSlotKind::ReturnValue => {
                if self.return_slot == Some(slot) {
                    self.return_type
                        .map(VbaTypeId::from)
                        .unwrap_or(VbaTypeId::Unknown)
                } else {
                    VbaTypeId::Unknown
                }
            }
            ProcedureRuntimeSlotKind::Local
            | ProcedureRuntimeSlotKind::Temporary
            | ProcedureRuntimeSlotKind::CompilerGenerated => VbaTypeId::Unknown,
        }
    }
}

pub(crate) fn legacy_procedure_signature_descriptor(
    procedure_name: &str,
    param_slots: &[usize],
    param_types: &[DeclareParamType],
    return_slot: Option<usize>,
    return_type: Option<DeclareParamType>,
    slots: &[ProcedureRuntimeSlotMetadata],
) -> ProcedureSignatureDescriptor {
    let slot_names = slots
        .iter()
        .map(|slot| (slot.slot, slot.name.clone()))
        .collect::<HashMap<_, _>>();
    let parameters = param_slots
        .iter()
        .enumerate()
        .map(|(index, slot)| {
            let declared_type = param_types
                .get(index)
                .copied()
                .map(VbaTypeId::from)
                .unwrap_or(VbaTypeId::Unknown);
            ParameterDescriptor {
                index,
                name: slot_names
                    .get(slot)
                    .cloned()
                    .unwrap_or_else(|| format!("arg{index}")),
                slot: Some(*slot),
                role: ParameterRole::Positional,
                source_mechanism: SourceParameterMechanism::Unknown,
                resolved_mechanism: ResolvedParameterMechanism::Unknown,
                passing_mode: ParameterPassingMode::Unknown,
                declared_type,
                optional: false,
                param_array: false,
                default_value: None,
                optional_descriptor: OptionalParameterDescriptor::Required,
                param_array_descriptor: None,
            }
        })
        .collect();
    ProcedureSignatureDescriptor {
        procedure_name: procedure_name.to_string(),
        kind: legacy_procedure_kind(procedure_name, return_slot),
        parameters,
        return_type: return_type.map(VbaTypeId::from),
        return_slot,
        property_group: property_group_name(procedure_name),
        implicit_current_object: None,
    }
}

fn legacy_procedure_kind(
    procedure_name: &str,
    return_slot: Option<usize>,
) -> ProcedureKindDescriptor {
    if procedure_name.starts_with("property_get_") {
        ProcedureKindDescriptor::PropertyGet
    } else if procedure_name.starts_with("property_let_") {
        ProcedureKindDescriptor::PropertyLet
    } else if procedure_name.starts_with("property_set_") {
        ProcedureKindDescriptor::PropertySet
    } else if return_slot.is_some() {
        ProcedureKindDescriptor::Function
    } else {
        ProcedureKindDescriptor::Sub
    }
}

fn property_group_name(procedure_name: &str) -> Option<String> {
    procedure_name
        .strip_prefix("property_get_")
        .or_else(|| procedure_name.strip_prefix("property_let_"))
        .or_else(|| procedure_name.strip_prefix("property_set_"))
        .map(ToString::to_string)
}

pub fn emit_bytecode(module: &BoundModule) -> Bytecode {
    emit_bytecode_with_runtime_metadata(module).0
}

fn declare_param_type_from_bound_type(ty: BoundType) -> DeclareParamType {
    match ty {
        BoundType::Integer => DeclareParamType::Integer,
        BoundType::Long => DeclareParamType::Long,
        BoundType::LongLong => DeclareParamType::LongLong,
        BoundType::LongPtr => DeclareParamType::LongPtr,
        BoundType::Byte => DeclareParamType::Byte,
        BoundType::Single => DeclareParamType::Single,
        BoundType::Double => DeclareParamType::Double,
        BoundType::Currency => DeclareParamType::Currency,
        BoundType::Date => DeclareParamType::Date,
        BoundType::String => DeclareParamType::String,
        BoundType::Boolean => DeclareParamType::Boolean,
        BoundType::Variant | BoundType::Array | BoundType::Object | BoundType::Decimal => {
            DeclareParamType::Variant
        }
    }
}

fn vba_type_id_from_bound_type(ty: BoundType) -> VbaTypeId {
    match ty {
        BoundType::Integer => VbaTypeId::Integer,
        BoundType::Long => VbaTypeId::Long,
        BoundType::LongLong => VbaTypeId::LongLong,
        BoundType::LongPtr => VbaTypeId::LongPtr,
        BoundType::Byte => VbaTypeId::Byte,
        BoundType::Single => VbaTypeId::Single,
        BoundType::Double => VbaTypeId::Double,
        BoundType::Currency => VbaTypeId::Currency,
        BoundType::Date => VbaTypeId::Date,
        BoundType::String => VbaTypeId::String,
        BoundType::Boolean => VbaTypeId::Boolean,
        BoundType::Variant => VbaTypeId::Variant,
        BoundType::Object => VbaTypeId::Object,
        BoundType::Array => VbaTypeId::Array,
        BoundType::Decimal => VbaTypeId::Variant,
    }
}

fn runtime_carrier_from_bound_type(ty: BoundType) -> RuntimeCarrierKind {
    if ty == BoundType::Decimal {
        RuntimeCarrierKind::Decimal96VariantSubtype
    } else {
        RuntimeCarrierKind::for_declared_type(vba_type_id_from_bound_type(ty))
    }
}

fn procedure_signature_kind(
    proc_name: &str,
    return_slot: Option<usize>,
) -> ProcedureKindDescriptor {
    legacy_procedure_kind(proc_name, return_slot)
}

fn call_target_kind_for_procedure(
    proc_name: &str,
    return_slot: Option<usize>,
) -> CallTargetKindDescriptor {
    match procedure_signature_kind(proc_name, return_slot) {
        ProcedureKindDescriptor::Sub => CallTargetKindDescriptor::Procedure,
        ProcedureKindDescriptor::Function => CallTargetKindDescriptor::Function,
        ProcedureKindDescriptor::PropertyGet => CallTargetKindDescriptor::PropertyGet,
        ProcedureKindDescriptor::PropertyLet => CallTargetKindDescriptor::PropertyLet,
        ProcedureKindDescriptor::PropertySet => CallTargetKindDescriptor::PropertySet,
        ProcedureKindDescriptor::Unknown => CallTargetKindDescriptor::Unknown,
    }
}

fn argument_source_kind(arg: &BoundCallArg) -> ArgumentSourceKindDescriptor {
    if arg.name.is_some() {
        ArgumentSourceKindDescriptor::Named
    } else {
        ArgumentSourceKindDescriptor::Positional
    }
}

fn call_invocation_syntax_descriptor(syntax: BoundCallSyntax) -> CallInvocationSyntaxDescriptor {
    match syntax {
        BoundCallSyntax::Unknown => CallInvocationSyntaxDescriptor::Unknown,
        BoundCallSyntax::StatementNoCall => CallInvocationSyntaxDescriptor::StatementNoCall,
        BoundCallSyntax::StatementCallKeyword => {
            CallInvocationSyntaxDescriptor::StatementCallKeyword
        }
        BoundCallSyntax::ExpressionCall => CallInvocationSyntaxDescriptor::ExpressionCall,
        BoundCallSyntax::SyntheticPropertyAssignment => {
            CallInvocationSyntaxDescriptor::SyntheticPropertyAssignment
        }
    }
}

fn argument_expression_kind(expr: &BoundExpr) -> ArgumentExpressionKindDescriptor {
    match expr {
        BoundExpr::Var(_) => ArgumentExpressionKindDescriptor::Variable,
        BoundExpr::IntConst(_)
        | BoundExpr::BoolConst(_)
        | BoundExpr::FloatConst(_)
        | BoundExpr::StringConst(_) => ArgumentExpressionKindDescriptor::Literal,
        BoundExpr::IntrinsicCall { .. } => ArgumentExpressionKindDescriptor::IntrinsicCall,
        BoundExpr::ProcCall { .. } => ArgumentExpressionKindDescriptor::ProcedureCall,
        BoundExpr::VarPtrArrayBuffer(_) => ArgumentExpressionKindDescriptor::ArrayBuffer,
        BoundExpr::AddConst { .. }
        | BoundExpr::SubConst { .. }
        | BoundExpr::BinaryOp { .. }
        | BoundExpr::CompareOp { .. }
        | BoundExpr::UnaryOp { .. } => ArgumentExpressionKindDescriptor::Expression,
    }
}

fn optional_default_value_for_param(param: &BoundParam) -> OptionalDefaultValue {
    match param.default_value {
        Some(value) => OptionalDefaultValue::ExplicitI32(value),
        None if param.ty == BoundType::Variant => OptionalDefaultValue::VariantMissingError448,
        None => OptionalDefaultValue::DeclaredTypeDefault,
    }
}

fn is_hidden_current_object_param(param: &BoundParam) -> bool {
    param.name.eq_ignore_ascii_case("__oxvba_this_instance")
}

fn source_mechanism_from_bound(value: BoundParamSourceMechanism) -> SourceParameterMechanism {
    match value {
        BoundParamSourceMechanism::Omitted => SourceParameterMechanism::Omitted,
        BoundParamSourceMechanism::ExplicitByRef => SourceParameterMechanism::ExplicitByRef,
        BoundParamSourceMechanism::ExplicitByVal => SourceParameterMechanism::ExplicitByVal,
    }
}

fn resolved_mechanism_for_param(
    kind: ProcedureKindDescriptor,
    role: ParameterRole,
    param: &BoundParam,
) -> ResolvedParameterMechanism {
    if matches!(
        (kind, role),
        (
            ProcedureKindDescriptor::PropertyLet | ProcedureKindDescriptor::PropertySet,
            ParameterRole::PropertyValue
        )
    ) {
        ResolvedParameterMechanism::PropertyValueByVal
    } else if param.by_ref {
        ResolvedParameterMechanism::ByRef
    } else {
        ResolvedParameterMechanism::ByVal
    }
}

fn optional_descriptor_for_param(param: &BoundParam) -> OptionalParameterDescriptor {
    if !param.optional {
        return OptionalParameterDescriptor::Required;
    }
    let default_value = match param.default_value {
        Some(value) => OptionalDefaultValue::ExplicitI32(value),
        None if param.ty == BoundType::Variant => OptionalDefaultValue::VariantMissingError448,
        None => OptionalDefaultValue::DeclaredTypeDefault,
    };
    let missing_state = if param.default_value.is_none() && param.ty == BoundType::Variant {
        OptionalMissingStatePolicy::PreserveMissingArgumentState
    } else {
        OptionalMissingStatePolicy::AssignDefaultLocal
    };
    OptionalParameterDescriptor::Optional {
        default_value,
        missing_state,
    }
}

fn param_array_descriptor_for_param(param: &BoundParam) -> Option<ParamArrayDescriptor> {
    param.param_array.then_some(ParamArrayDescriptor {
        element_type: VbaTypeId::Variant,
        array_lower_bound: 0,
        empty_upper_bound: -1,
    })
}

fn build_procedure_signature_descriptor(
    proc: &BoundProcedure,
    proc_slots: &HashMap<String, usize>,
    return_slot: Option<usize>,
    is_class_module: bool,
) -> ProcedureSignatureDescriptor {
    let kind = procedure_signature_kind(&proc.name, return_slot);
    let property_value_index = if matches!(
        kind,
        ProcedureKindDescriptor::PropertyLet | ProcedureKindDescriptor::PropertySet
    ) {
        proc.params.len().checked_sub(1)
    } else {
        None
    };
    let parameters = proc
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let slot = proc_slots.get(&param.name).copied();
            let role = if is_class_module && index == 0 && is_hidden_current_object_param(param) {
                ParameterRole::ImplicitCurrentObject
            } else if param.param_array {
                ParameterRole::ParamArray
            } else if Some(index) == property_value_index {
                ParameterRole::PropertyValue
            } else if param.optional {
                ParameterRole::Optional
            } else {
                ParameterRole::Positional
            };
            let source_mechanism = if role == ParameterRole::ImplicitCurrentObject {
                SourceParameterMechanism::ImplementationInjected
            } else {
                source_mechanism_from_bound(param.source_mechanism)
            };
            let resolved_mechanism = resolved_mechanism_for_param(kind, role, param);
            ParameterDescriptor {
                index,
                name: param.name.clone(),
                slot,
                role,
                source_mechanism,
                resolved_mechanism,
                passing_mode: if param.by_ref {
                    ParameterPassingMode::ByRef
                } else {
                    ParameterPassingMode::ByVal
                },
                declared_type: vba_type_id_from_bound_type(param.ty),
                optional: param.optional,
                param_array: param.param_array,
                default_value: param.default_value,
                optional_descriptor: optional_descriptor_for_param(param),
                param_array_descriptor: param_array_descriptor_for_param(param),
            }
        })
        .collect();
    let implicit_current_object = proc
        .params
        .first()
        .filter(|param| is_class_module && is_hidden_current_object_param(param))
        .map(|param| ImplicitCurrentObjectDescriptor {
            declared_type: VbaTypeId::Object,
            mechanism: if param.by_ref {
                ResolvedParameterMechanism::ByRef
            } else {
                ResolvedParameterMechanism::ByVal
            },
            accessible_name: "Me".to_string(),
            assignable: false,
            slot: proc_slots.get(&param.name).copied(),
        });
    ProcedureSignatureDescriptor {
        procedure_name: proc.name.clone(),
        kind,
        parameters,
        return_type: if matches!(
            kind,
            ProcedureKindDescriptor::Function | ProcedureKindDescriptor::PropertyGet
        ) {
            Some(vba_type_id_from_bound_type(proc.return_type))
        } else {
            None
        },
        return_slot,
        property_group: property_group_name(&proc.name),
        implicit_current_object,
    }
}

fn procedure_return_declare_type(proc: &BoundProcedure) -> Option<DeclareParamType> {
    if proc.name.eq_ignore_ascii_case("main") {
        return None;
    }
    let has_return_slot = proc.declaration_types.contains_key(&proc.name)
        || proc
            .name
            .strip_prefix("property_get_")
            .is_some_and(|base| proc.declaration_types.contains_key(base));
    has_return_slot.then(|| declare_param_type_from_bound_type(proc.return_type))
}

pub fn emit_bytecode_with_runtime_metadata(
    module: &BoundModule,
) -> (Bytecode, BTreeMap<String, ProcedureRuntimeMetadata>) {
    let compare_mode = emit_compare_mode(module.compare_mode);
    let procedures = if module.procedures.is_empty() {
        vec![BoundProcedure {
            name: "main".to_string(),
            source_line_start: 1,
            source_line_end: module.source.lines().count().max(1),
            statement_line_numbers: vec![1],
            return_type: crate::resolve::BoundType::Variant,
            params: Vec::new(),
            module_scope_names: Vec::new(),
            declarations: module.declarations.clone(),
            declaration_types: module.declaration_types.clone(),
            array_descriptors: module.array_descriptors.clone(),
            udt_descriptors: Vec::new(),
            duplicate_declarations: Vec::new(),
            body: module.body.clone(),
        }]
    } else {
        module.procedures.clone()
    };

    let entry_idx = procedures
        .iter()
        .position(|p| p.name.eq_ignore_ascii_case("main"))
        .unwrap_or(0);

    let mut proc_slots: Vec<HashMap<String, usize>> = Vec::new();
    let mut next_slot = 0usize;
    for proc in &procedures {
        let mut map = HashMap::new();
        for name in &proc.declarations {
            map.insert(name.clone(), next_slot);
            next_slot += 1;
        }
        proc_slots.push(map);
    }

    let mut temps = TempSlotAllocator::new(next_slot);
    let mut instructions = Vec::new();
    let mut do_exit_stack: Vec<Vec<usize>> = Vec::new();
    let mut for_exit_stack: Vec<Vec<usize>> = Vec::new();
    let mut proc_exit_stack: Vec<Vec<usize>> = Vec::new();
    let mut call_patches: Vec<(usize, String)> = Vec::new();
    let mut error_handler_patches: Vec<(usize, String)> = Vec::new();
    let mut goto_patches: Vec<(usize, String)> = Vec::new();
    let mut resume_label_patches: Vec<(usize, String)> = Vec::new();
    let mut proc_labels: HashMap<String, usize> = HashMap::new();
    let mut procedure_runtime_metadata = BTreeMap::<String, ProcedureRuntimeMetadata>::new();
    let mut proc_meta: HashMap<String, EmitProcMeta> = HashMap::new();
    let external_decls = module.external_declarations.clone();
    let external_call_descriptors = build_external_call_descriptors(
        &external_decls,
        cfg!(any(target_os = "windows", target_os = "linux")),
    );
    for (idx, proc) in procedures.iter().enumerate() {
        insert_casefold_key(
            &mut proc_meta,
            &proc.name,
            EmitProcMeta {
                params: proc.params.clone(),
                slots: proc_slots[idx].clone(),
                return_slot: proc_slots[idx]
                    .get(&proc.name)
                    .or_else(|| {
                        // For Property Get, the return slot is declared under
                        // the base name (e.g. "value"), not the canonical name.
                        proc.name
                            .strip_prefix("property_get_")
                            .and_then(|base| proc_slots[idx].get(base))
                    })
                    .copied(),
                return_type: proc.return_type,
                declaration_types: proc.declaration_types.clone(),
            },
        );
    }
    // Only invoke Class_Initialize/Terminate as lifecycle methods in class
    // modules.  In standard modules they are ordinary Subs.
    let class_init_proc = if module.is_class_module {
        procedures
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case("class_initialize"))
            .map(|p| p.name.clone())
    } else {
        None
    };
    let class_terminate_proc = if module.is_class_module {
        procedures
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case("class_terminate"))
            .map(|p| p.name.clone())
    } else {
        None
    };
    insert_casefold_key(&mut proc_labels, &procedures[entry_idx].name, 0);
    instructions.push(Instruction::ClearErr);
    for param in &procedures[entry_idx].params {
        if param.ty == BoundType::Date
            && let Some(slot) = proc_slots[entry_idx].get(&param.name).copied()
        {
            instructions.push(Instruction::IntrinsicCDateValue {
                dst: slot,
                src: slot,
            });
        }
    }
    emit_declared_string_initializers(
        &procedures[entry_idx],
        &proc_slots[entry_idx],
        &mut instructions,
    );

    if let Some(name) = class_init_proc {
        let patch_idx = instructions.len();
        instructions.push(Instruction::CallProc {
            target_pc: 0,
            project_member: None,
        });
        call_patches.push((patch_idx, name));
    }
    let mut entry_statement_entry_pcs = Vec::new();
    let mut entry_call_sites = Vec::new();
    proc_exit_stack.push(Vec::new());
    let entry_temp_start = temps.next_temp_slot();
    with_current_proc_name(&procedures[entry_idx].name, || {
        emit_stmt_list(
            &procedures[entry_idx].body,
            compare_mode,
            &proc_slots[entry_idx],
            &mut temps,
            &mut instructions,
            &mut do_exit_stack,
            &mut for_exit_stack,
            &mut proc_exit_stack,
            &mut call_patches,
            &mut error_handler_patches,
            &mut goto_patches,
            &mut resume_label_patches,
            &proc_meta,
            &external_decls,
            &procedures[entry_idx].name,
            &mut proc_labels,
            &mut entry_statement_entry_pcs,
            &mut entry_call_sites,
        );
    });
    let entry_temp_slots = temps.slots_allocated_since(entry_temp_start);
    if let Some(name) = class_terminate_proc {
        let patch_idx = instructions.len();
        instructions.push(Instruction::CallProc {
            target_pc: 0,
            project_member: None,
        });
        call_patches.push((patch_idx, name));
    }
    let entry_exit_target = instructions.len();
    if let Some(exit_patches) = proc_exit_stack.pop() {
        for patch in exit_patches {
            if let Instruction::Jump { target_pc } = &mut instructions[patch] {
                *target_pc = entry_exit_target;
            }
        }
    }
    instructions.push(Instruction::ClearErr);
    instructions.push(Instruction::Halt);
    let entry_return_slot = proc_slots[entry_idx]
        .get(&procedures[entry_idx].name)
        .or_else(|| {
            procedures[entry_idx]
                .name
                .strip_prefix("property_get_")
                .and_then(|base| proc_slots[entry_idx].get(base))
        })
        .copied();
    procedure_runtime_metadata.insert(
        procedures[entry_idx].name.to_ascii_lowercase(),
        ProcedureRuntimeMetadata {
            module_name: String::new(),
            procedure_name: procedures[entry_idx].name.clone(),
            entry_pc: 0,
            source_line_start: procedures[entry_idx].source_line_start,
            source_line_end: procedures[entry_idx].source_line_end,
            statement_line_numbers: procedures[entry_idx].statement_line_numbers.clone(),
            statement_entry_pcs: entry_statement_entry_pcs,
            slots: build_runtime_slot_metadata(
                &procedures[entry_idx],
                &proc_slots[entry_idx],
                proc_meta[&procedures[entry_idx].name].return_slot,
                &entry_temp_slots,
            ),
            param_slots: procedures[entry_idx]
                .params
                .iter()
                .filter_map(|param| proc_slots[entry_idx].get(&param.name).copied())
                .collect(),
            return_slot: entry_return_slot,
            param_types: procedures[entry_idx]
                .params
                .iter()
                .map(|param| declare_param_type_from_bound_type(param.ty))
                .collect(),
            return_type: procedure_return_declare_type(&procedures[entry_idx]),
            signature: build_procedure_signature_descriptor(
                &procedures[entry_idx],
                &proc_slots[entry_idx],
                entry_return_slot,
                module.is_class_module,
            ),
            call_sites: entry_call_sites,
            array_shapes: build_array_shape_descriptors(
                &procedures[entry_idx],
                &proc_slots[entry_idx],
            ),
            udt_types: build_udt_type_descriptors(&procedures[entry_idx], &proc_slots[entry_idx]),
            object_types: build_object_type_descriptors(
                &procedures[entry_idx],
                &proc_slots[entry_idx],
            ),
            carrier_layouts: Vec::new(),
            value_states: Vec::new(),
            expression_semantics: Vec::new(),
            operator_semantics: Vec::new(),
            coercions: Vec::new(),
            name_bindings: Vec::new(),
            object_member_bindings: Vec::new(),
        },
    );

    for (idx, proc) in procedures.iter().enumerate() {
        if idx == entry_idx {
            continue;
        }
        let entry_pc = instructions.len();
        insert_casefold_key(&mut proc_labels, &proc.name, entry_pc);
        instructions.push(Instruction::ClearErr);
        for param in &proc.params {
            if param.ty == BoundType::Date
                && let Some(slot) = proc_slots[idx].get(&param.name).copied()
            {
                instructions.push(Instruction::IntrinsicCDateValue {
                    dst: slot,
                    src: slot,
                });
            }
        }
        emit_declared_string_initializers(proc, &proc_slots[idx], &mut instructions);
        let mut statement_entry_pcs = Vec::new();
        let mut call_sites = Vec::new();
        proc_exit_stack.push(Vec::new());
        let proc_temp_start = temps.next_temp_slot();
        with_current_proc_name(&proc.name, || {
            emit_stmt_list(
                &proc.body,
                compare_mode,
                &proc_slots[idx],
                &mut temps,
                &mut instructions,
                &mut do_exit_stack,
                &mut for_exit_stack,
                &mut proc_exit_stack,
                &mut call_patches,
                &mut error_handler_patches,
                &mut goto_patches,
                &mut resume_label_patches,
                &proc_meta,
                &external_decls,
                &proc.name,
                &mut proc_labels,
                &mut statement_entry_pcs,
                &mut call_sites,
            );
        });
        let proc_temp_slots = temps.slots_allocated_since(proc_temp_start);
        let proc_exit_target = instructions.len();
        if let Some(exit_patches) = proc_exit_stack.pop() {
            for patch in exit_patches {
                if let Instruction::Jump { target_pc } = &mut instructions[patch] {
                    *target_pc = proc_exit_target;
                }
            }
        }
        instructions.push(Instruction::ClearErr);
        instructions.push(Instruction::Return);
        let return_slot = proc_slots[idx]
            .get(&proc.name)
            .or_else(|| {
                proc.name
                    .strip_prefix("property_get_")
                    .and_then(|base| proc_slots[idx].get(base))
            })
            .copied();
        procedure_runtime_metadata.insert(
            proc.name.to_ascii_lowercase(),
            ProcedureRuntimeMetadata {
                module_name: String::new(),
                procedure_name: proc.name.clone(),
                entry_pc,
                source_line_start: proc.source_line_start,
                source_line_end: proc.source_line_end,
                statement_line_numbers: proc.statement_line_numbers.clone(),
                statement_entry_pcs,
                slots: build_runtime_slot_metadata(
                    proc,
                    &proc_slots[idx],
                    proc_meta[&proc.name].return_slot,
                    &proc_temp_slots,
                ),
                param_slots: proc
                    .params
                    .iter()
                    .filter_map(|param| proc_slots[idx].get(&param.name).copied())
                    .collect(),
                return_slot,
                param_types: proc
                    .params
                    .iter()
                    .map(|param| declare_param_type_from_bound_type(param.ty))
                    .collect(),
                return_type: procedure_return_declare_type(proc),
                signature: build_procedure_signature_descriptor(
                    proc,
                    &proc_slots[idx],
                    return_slot,
                    module.is_class_module,
                ),
                call_sites,
                array_shapes: build_array_shape_descriptors(proc, &proc_slots[idx]),
                udt_types: build_udt_type_descriptors(proc, &proc_slots[idx]),
                object_types: build_object_type_descriptors(proc, &proc_slots[idx]),
                carrier_layouts: Vec::new(),
                value_states: Vec::new(),
                expression_semantics: Vec::new(),
                operator_semantics: Vec::new(),
                coercions: Vec::new(),
                name_bindings: Vec::new(),
                object_member_bindings: Vec::new(),
            },
        );
    }

    for (patch_idx, proc_name) in call_patches {
        if let Some(target) = lookup_casefold_key(&proc_labels, &proc_name).copied()
            && let Instruction::CallProc {
                target_pc,
                project_member,
            } = &mut instructions[patch_idx]
        {
            *target_pc = target;
            *project_member = project_member_call_descriptor_for_proc_name(&proc_name);
            patch_call_site_target_pc(&mut procedure_runtime_metadata, patch_idx, target);
        }
    }

    for (patch_idx, label_name) in error_handler_patches {
        if let Some(target) = lookup_casefold_key(&proc_labels, &label_name).copied()
            && let Instruction::SetOnErrorGotoLabel { target_pc } = &mut instructions[patch_idx]
        {
            *target_pc = target;
        }
    }

    for (patch_idx, label_name) in goto_patches {
        if let Some(target) = lookup_casefold_key(&proc_labels, &label_name).copied()
            && let Instruction::Jump { target_pc } = &mut instructions[patch_idx]
        {
            *target_pc = target;
        }
    }

    for (patch_idx, label_name) in resume_label_patches {
        if let Some(target) = lookup_casefold_key(&proc_labels, &label_name).copied()
            && let Instruction::ResumeLabel { target_pc } = &mut instructions[patch_idx]
        {
            *target_pc = target;
        }
    }

    decorate_runtime_metadata_descriptors(
        &mut procedure_runtime_metadata,
        &instructions,
        &procedures,
        &module.enum_descriptors,
        compare_mode,
    );

    (
        Bytecode {
            instructions,
            external_call_descriptors,
            slot_count: temps.total_slots(),
            user_slot_count: procedures[entry_idx].declarations.len(),
        },
        procedure_runtime_metadata,
    )
}

fn build_runtime_slot_metadata(
    proc: &BoundProcedure,
    proc_slots: &HashMap<String, usize>,
    return_slot: Option<usize>,
    temp_slots: &[usize],
) -> Vec<ProcedureRuntimeSlotMetadata> {
    let return_name = proc
        .name
        .strip_prefix("property_get_")
        .unwrap_or(&proc.name)
        .to_ascii_lowercase();
    let mut slots = Vec::new();
    for name in &proc.declarations {
        let hide_module_scope_slots = !proc.name.eq_ignore_ascii_case("main");
        if hide_module_scope_slots
            && proc
                .module_scope_names
                .iter()
                .any(|module_name| module_name.eq_ignore_ascii_case(name))
        {
            continue;
        }
        let Some(slot) = proc_slots.get(name).copied() else {
            continue;
        };
        let kind = if proc
            .params
            .iter()
            .any(|param| param.name.eq_ignore_ascii_case(name))
        {
            ProcedureRuntimeSlotKind::Parameter
        } else if Some(slot) == return_slot
            && (proc.name.eq_ignore_ascii_case(name) || name.eq_ignore_ascii_case(&return_name))
        {
            ProcedureRuntimeSlotKind::ReturnValue
        } else if is_compiler_generated_array_element_slot(proc, name) {
            ProcedureRuntimeSlotKind::CompilerGenerated
        } else {
            ProcedureRuntimeSlotKind::Local
        };
        let bound_type = proc
            .declaration_types
            .get(name.as_str())
            .copied()
            .unwrap_or(BoundType::Variant);
        if let Some(descriptor_id) = udt_descriptor_id_for_instance(proc, name) {
            slots.push(
                ProcedureRuntimeSlotMetadata::new_with_initial_state_and_carrier(
                    name.clone(),
                    slot,
                    kind,
                    VbaTypeId::Unknown,
                    SlotInitialState::UdtDefault,
                    RuntimeCarrierKind::UdtFields {
                        descriptor: descriptor_id,
                    },
                ),
            );
        } else {
            slots.push(ProcedureRuntimeSlotMetadata::new_with_carrier(
                name.clone(),
                slot,
                kind,
                vba_type_id_from_bound_type(bound_type),
                runtime_carrier_from_bound_type(bound_type),
            ));
        }
    }
    for slot in temp_slots {
        slots.push(ProcedureRuntimeSlotMetadata::new(
            format!("__temp{slot}"),
            *slot,
            ProcedureRuntimeSlotKind::Temporary,
            VbaTypeId::Unknown,
        ));
    }
    slots.sort_by(|lhs, rhs| {
        lhs.slot
            .cmp(&rhs.slot)
            .then_with(|| lhs.name.cmp(&rhs.name))
    });
    slots
}

fn decorate_runtime_metadata_descriptors(
    metadata: &mut BTreeMap<String, ProcedureRuntimeMetadata>,
    instructions: &[Instruction],
    procedures: &[BoundProcedure],
    enum_descriptors: &[BoundEnumDescriptor],
    compare_mode: StringCompareMode,
) {
    let mut ranges = metadata
        .iter()
        .map(|(key, value)| (key.clone(), value.entry_pc))
        .collect::<Vec<_>>();
    ranges.sort_by(|left, right| left.1.cmp(&right.1).then(left.0.cmp(&right.0)));
    let metadata_snapshot = metadata.clone();
    for index in 0..ranges.len() {
        let key = ranges[index].0.clone();
        let start_pc = ranges[index].1;
        let end_pc = ranges
            .get(index + 1)
            .map(|(_, entry_pc)| *entry_pc)
            .unwrap_or(instructions.len());
        let proc = procedures
            .iter()
            .find(|proc| proc.name.eq_ignore_ascii_case(&key));
        let Some(metadata) = metadata.get_mut(&key) else {
            continue;
        };
        metadata.carrier_layouts = build_carrier_layout_descriptors(&metadata.slots);
        metadata.value_states =
            build_value_state_descriptors(metadata, proc, instructions, start_pc, end_pc);
        metadata.expression_semantics = build_expression_semantics_descriptors(metadata, proc);
        metadata.operator_semantics =
            build_operator_semantics_descriptors(metadata, proc, compare_mode);
        metadata.coercions = build_coercion_descriptors(metadata, proc, &metadata_snapshot);
        metadata.name_bindings = build_name_binding_descriptors(metadata, proc, enum_descriptors);
        metadata.object_member_bindings = build_object_member_binding_descriptors(metadata, proc);
    }
}

fn build_carrier_layout_descriptors(
    slots: &[ProcedureRuntimeSlotMetadata],
) -> Vec<CarrierLayoutDescriptor> {
    let mut carriers = BTreeMap::<String, RuntimeCarrierKind>::new();
    for slot in slots {
        carriers
            .entry(slot.carrier.registry_key())
            .or_insert_with(|| slot.carrier.clone());
    }
    carriers
        .into_values()
        .map(carrier_layout_descriptor)
        .collect()
}

fn carrier_layout_descriptor(carrier: RuntimeCarrierKind) -> CarrierLayoutDescriptor {
    let carrier_key = carrier.registry_key();
    let descriptor_id =
        canonical_descriptor_id(DescriptorFamily::CarrierLayout, [carrier_key.as_str()]);
    let mut notes = Vec::new();
    let (layout, storage_bits, native_frame_eligible, variant_compatible, com_variant_type) =
        match &carrier {
            RuntimeCarrierKind::Unknown => (
                CarrierLayoutKind::Unknown,
                None,
                false,
                false,
                None::<String>,
            ),
            RuntimeCarrierKind::Variant => (
                CarrierLayoutKind::VariantPayload,
                None,
                false,
                true,
                Some("VT_VARIANT".to_string()),
            ),
            RuntimeCarrierKind::Boolean => (
                CarrierLayoutKind::NativeScalar,
                Some(16),
                true,
                true,
                Some("VT_BOOL".to_string()),
            ),
            RuntimeCarrierKind::I16 => (
                CarrierLayoutKind::NativeScalar,
                Some(16),
                true,
                true,
                Some("VT_I2".to_string()),
            ),
            RuntimeCarrierKind::U8 => (
                CarrierLayoutKind::NativeScalar,
                Some(8),
                true,
                true,
                Some("VT_UI1".to_string()),
            ),
            RuntimeCarrierKind::I32 => (
                CarrierLayoutKind::NativeScalar,
                Some(32),
                true,
                true,
                Some("VT_I4".to_string()),
            ),
            RuntimeCarrierKind::I64 => (
                CarrierLayoutKind::NativeScalar,
                Some(64),
                true,
                true,
                Some("VT_I8".to_string()),
            ),
            RuntimeCarrierKind::PointerSizedInteger => (
                CarrierLayoutKind::NativeScalar,
                Some(usize::BITS as u16),
                true,
                true,
                Some(if usize::BITS == 64 { "VT_I8" } else { "VT_I4" }.to_string()),
            ),
            RuntimeCarrierKind::F32 => (
                CarrierLayoutKind::NativeScalar,
                Some(32),
                true,
                true,
                Some("VT_R4".to_string()),
            ),
            RuntimeCarrierKind::F64 => (
                CarrierLayoutKind::NativeScalar,
                Some(64),
                true,
                true,
                Some("VT_R8".to_string()),
            ),
            RuntimeCarrierKind::Currency => (
                CarrierLayoutKind::NativeScalar,
                Some(64),
                true,
                true,
                Some("VT_CY".to_string()),
            ),
            RuntimeCarrierKind::Date => (
                CarrierLayoutKind::NativeScalar,
                Some(64),
                true,
                true,
                Some("VT_DATE".to_string()),
            ),
            RuntimeCarrierKind::BStr => (
                CarrierLayoutKind::BStr,
                Some(usize::BITS as u16),
                true,
                true,
                Some("VT_BSTR".to_string()),
            ),
            RuntimeCarrierKind::Decimal96VariantSubtype => {
                notes.push("declared-decimal-extension=variant-subtype".to_string());
                (
                    CarrierLayoutKind::VariantPayload,
                    Some(96),
                    false,
                    true,
                    Some("VT_DECIMAL".to_string()),
                )
            }
            RuntimeCarrierKind::ObjectRef => (
                CarrierLayoutKind::ObjectRef,
                Some(usize::BITS as u16),
                true,
                true,
                Some("VT_DISPATCH-or-VT_UNKNOWN".to_string()),
            ),
            RuntimeCarrierKind::SafeArray => (
                CarrierLayoutKind::SafeArray,
                Some(usize::BITS as u16),
                true,
                true,
                Some("VT_ARRAY".to_string()),
            ),
            RuntimeCarrierKind::UdtFields { descriptor } => {
                notes.push(format!("udt-descriptor={descriptor}"));
                (
                    CarrierLayoutKind::UdtFields,
                    None,
                    true,
                    false,
                    None::<String>,
                )
            }
            RuntimeCarrierKind::BindingHandleInternal => (
                CarrierLayoutKind::InternalHandle,
                Some(usize::BITS as u16),
                false,
                false,
                None::<String>,
            ),
        };
    CarrierLayoutDescriptor {
        descriptor_id,
        carrier,
        carrier_key,
        layout,
        storage_bits,
        native_frame_eligible,
        variant_compatible,
        com_variant_type,
        notes,
    }
}

fn build_value_state_descriptors(
    metadata: &ProcedureRuntimeMetadata,
    proc: Option<&BoundProcedure>,
    instructions: &[Instruction],
    start_pc: usize,
    end_pc: usize,
) -> Vec<ValueStateDescriptor> {
    let mut descriptors = Vec::new();
    let mut ordinal = 0usize;
    let procedure_id = metadata_descriptor_owner_id(metadata);
    collect_slot_value_state_descriptors(metadata, &procedure_id, &mut ordinal, &mut descriptors);
    collect_signature_value_state_descriptors(
        metadata,
        &procedure_id,
        &mut ordinal,
        &mut descriptors,
    );
    if let Some(proc) = proc {
        collect_bound_value_state_descriptors(
            proc,
            metadata,
            &procedure_id,
            &mut ordinal,
            &mut descriptors,
        );
    }
    collect_instruction_value_state_descriptors(
        instructions,
        start_pc,
        end_pc,
        &procedure_id,
        &mut ordinal,
        &mut descriptors,
    );
    descriptors.sort_by(|left, right| left.descriptor_id.cmp(&right.descriptor_id));
    descriptors
}

fn collect_slot_value_state_descriptors(
    metadata: &ProcedureRuntimeMetadata,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<ValueStateDescriptor>,
) {
    for slot in &metadata.slots {
        match slot.initial_state {
            SlotInitialState::Empty => descriptors.push(value_state_descriptor(
                procedure_id,
                ValueStateKind::Empty,
                ValueStateSource::SlotInitialState,
                (Some(slot.slot), None, Some(slot.name.clone())),
                "default-init:variant-empty".to_string(),
                ordinal,
            )),
            SlotInitialState::Nothing => descriptors.push(value_state_descriptor(
                procedure_id,
                ValueStateKind::Nothing,
                ValueStateSource::SlotInitialState,
                (Some(slot.slot), None, Some(slot.name.clone())),
                "default-init:object-nothing".to_string(),
                ordinal,
            )),
            SlotInitialState::Unknown
            | SlotInitialState::CallerProvided
            | SlotInitialState::ScalarZero
            | SlotInitialState::False
            | SlotInitialState::EmptyString
            | SlotInitialState::UnallocatedArray
            | SlotInitialState::UdtDefault
            | SlotInitialState::CompilerDefined => {}
        }
        if slot.carrier == RuntimeCarrierKind::Decimal96VariantSubtype {
            descriptors.push(value_state_descriptor(
                procedure_id,
                ValueStateKind::DecimalVariantSubtype,
                ValueStateSource::DeclaredTypeExtension,
                (Some(slot.slot), None, Some(slot.name.clone())),
                "declared-decimal-extension:variant-subtype".to_string(),
                ordinal,
            ));
        }
    }
}

fn collect_signature_value_state_descriptors(
    metadata: &ProcedureRuntimeMetadata,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<ValueStateDescriptor>,
) {
    for parameter in &metadata.signature.parameters {
        let OptionalParameterDescriptor::Optional {
            default_value,
            missing_state,
        } = &parameter.optional_descriptor
        else {
            continue;
        };
        match default_value {
            OptionalDefaultValue::VariantMissingError448 => {
                descriptors.push(value_state_descriptor(
                    procedure_id,
                    ValueStateKind::MissingArgument,
                    ValueStateSource::OptionalParameter,
                    (parameter.slot, None, Some(parameter.name.clone())),
                    format!("missing-state={missing_state:?}; default=VariantMissingError448"),
                    ordinal,
                ));
            }
            OptionalDefaultValue::Unknown => {}
            OptionalDefaultValue::ExplicitI32(value) => {
                descriptors.push(value_state_descriptor(
                    procedure_id,
                    ValueStateKind::OmittedDefault,
                    ValueStateSource::OptionalParameter,
                    (parameter.slot, None, Some(parameter.name.clone())),
                    format!("default=ExplicitI32({value}); missing-state={missing_state:?}"),
                    ordinal,
                ));
            }
            OptionalDefaultValue::DeclaredTypeDefault
            | OptionalDefaultValue::ImplementationDefined => {
                descriptors.push(value_state_descriptor(
                    procedure_id,
                    ValueStateKind::OmittedDefault,
                    ValueStateSource::OptionalParameter,
                    (parameter.slot, None, Some(parameter.name.clone())),
                    format!("default={default_value:?}; missing-state={missing_state:?}"),
                    ordinal,
                ));
            }
        }
    }
}

fn collect_bound_value_state_descriptors(
    proc: &BoundProcedure,
    metadata: &ProcedureRuntimeMetadata,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<ValueStateDescriptor>,
) {
    let slot_by_name = metadata
        .slots
        .iter()
        .map(|slot| (slot.name.to_ascii_lowercase(), slot.slot))
        .collect::<HashMap<_, _>>();
    for stmt in &proc.body {
        collect_stmt_value_states(stmt, &slot_by_name, procedure_id, ordinal, descriptors);
    }
}

fn collect_stmt_value_states(
    stmt: &BoundStmt,
    slot_by_name: &HashMap<String, usize>,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<ValueStateDescriptor>,
) {
    match stmt {
        BoundStmt::Assign { target, expr, .. } => collect_expr_value_states(
            expr,
            slot_by_name.get(&target.to_ascii_lowercase()).copied(),
            procedure_id,
            ordinal,
            descriptors,
        ),
        BoundStmt::AssignRuntimeArrayElement { indices, expr, .. } => {
            for index in indices {
                collect_expr_value_states(index, None, procedure_id, ordinal, descriptors);
            }
            collect_expr_value_states(expr, None, procedure_id, ordinal, descriptors);
        }
        BoundStmt::UdtAssign { .. } => {}
        BoundStmt::MidAssign {
            start,
            count,
            value,
            ..
        } => {
            collect_expr_value_states(start, None, procedure_id, ordinal, descriptors);
            if let Some(count) = count {
                collect_expr_value_states(count, None, procedure_id, ordinal, descriptors);
            }
            collect_expr_value_states(value, None, procedure_id, ordinal, descriptors);
        }
        BoundStmt::IfCond {
            cond,
            then_body,
            else_body,
        } => {
            collect_cond_value_states(cond, procedure_id, ordinal, descriptors);
            for stmt in then_body.iter().chain(else_body.iter()) {
                collect_stmt_value_states(stmt, slot_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::ForRange {
            start,
            end,
            step,
            body,
            ..
        } => {
            collect_expr_value_states(start, None, procedure_id, ordinal, descriptors);
            collect_expr_value_states(end, None, procedure_id, ordinal, descriptors);
            collect_expr_value_states(step, None, procedure_id, ordinal, descriptors);
            for stmt in body {
                collect_stmt_value_states(stmt, slot_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::ForEach {
            items,
            iterable,
            body,
            ..
        } => {
            for item in items {
                collect_expr_value_states(item, None, procedure_id, ordinal, descriptors);
            }
            if let Some(iterable) = iterable {
                collect_expr_value_states(iterable, None, procedure_id, ordinal, descriptors);
            }
            for stmt in body {
                collect_stmt_value_states(stmt, slot_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::ReDimRuntime { bounds, .. } => {
            for bound in bounds {
                collect_expr_value_states(
                    &bound.upper_bound,
                    None,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::ReDim { .. }
        | BoundStmt::Erase { .. }
        | BoundStmt::ExitDo
        | BoundStmt::ExitFor
        | BoundStmt::OnErrorResumeNext
        | BoundStmt::OnErrorGoto0
        | BoundStmt::OnErrorGotoLabel { .. }
        | BoundStmt::ResumeNext
        | BoundStmt::Resume
        | BoundStmt::ResumeLabel { .. }
        | BoundStmt::RaiseError(_)
        | BoundStmt::ErrClear
        | BoundStmt::Label { .. }
        | BoundStmt::GoTo { .. }
        | BoundStmt::GoSub { .. }
        | BoundStmt::Return
        | BoundStmt::Beep
        | BoundStmt::ExitProcedure
        | BoundStmt::Unsupported { .. } => {}
        BoundStmt::DoWhile {
            cond,
            body,
            post_check: _,
        } => {
            collect_cond_value_states(cond, procedure_id, ordinal, descriptors);
            for stmt in body {
                collect_stmt_value_states(stmt, slot_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::RaiseEvent { args, .. }
        | BoundStmt::Call { args, .. }
        | BoundStmt::AssignFromCall { args, .. } => {
            for arg in args {
                collect_expr_value_states(&arg.expr, None, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            collect_expr_value_states(expr, None, procedure_id, ordinal, descriptors);
            for (_, body) in arms {
                for stmt in body {
                    collect_stmt_value_states(
                        stmt,
                        slot_by_name,
                        procedure_id,
                        ordinal,
                        descriptors,
                    );
                }
            }
            for stmt in else_body {
                collect_stmt_value_states(stmt, slot_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::FileOpen {
            path, file_number, ..
        } => {
            collect_expr_value_states(path, None, procedure_id, ordinal, descriptors);
            collect_expr_value_states(file_number, None, procedure_id, ordinal, descriptors);
        }
        BoundStmt::FileClose { file_number } => {
            if let Some(file_number) = file_number {
                collect_expr_value_states(file_number, None, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::FileKill { path } => {
            collect_expr_value_states(path, None, procedure_id, ordinal, descriptors);
        }
        BoundStmt::FilePrint { file_number, data } => {
            collect_expr_value_states(file_number, None, procedure_id, ordinal, descriptors);
            collect_expr_value_states(data, None, procedure_id, ordinal, descriptors);
        }
        BoundStmt::FileInput {
            file_number,
            targets: _,
        } => {
            collect_expr_value_states(file_number, None, procedure_id, ordinal, descriptors);
        }
        BoundStmt::ConsolePrint { data } => {
            collect_expr_value_states(data, None, procedure_id, ordinal, descriptors);
        }
        BoundStmt::ConsoleInput { targets: _ } | BoundStmt::ConsoleLineInput { target: _ } => {}
        BoundStmt::FileWrite { file_number, data } => {
            collect_expr_value_states(file_number, None, procedure_id, ordinal, descriptors);
            for item in data {
                collect_expr_value_states(item, None, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::FileLineInput {
            file_number,
            target: _,
        } => {
            collect_expr_value_states(file_number, None, procedure_id, ordinal, descriptors);
        }
        BoundStmt::DebugPrint { data } => {
            collect_expr_value_states(data, None, procedure_id, ordinal, descriptors);
        }
    }
}

fn collect_cond_value_states(
    cond: &BoundCond,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<ValueStateDescriptor>,
) {
    match cond {
        BoundCond::Compare { lhs, rhs, .. } => {
            collect_expr_value_states(lhs, None, procedure_id, ordinal, descriptors);
            collect_expr_value_states(rhs, None, procedure_id, ordinal, descriptors);
        }
        BoundCond::Truthy(expr) => {
            collect_expr_value_states(expr, None, procedure_id, ordinal, descriptors);
        }
        BoundCond::Not(inner) => {
            collect_cond_value_states(inner, procedure_id, ordinal, descriptors)
        }
        BoundCond::And(lhs, rhs) | BoundCond::Or(lhs, rhs) => {
            collect_cond_value_states(lhs, procedure_id, ordinal, descriptors);
            collect_cond_value_states(rhs, procedure_id, ordinal, descriptors);
        }
    }
}

fn collect_expr_value_states(
    expr: &BoundExpr,
    slot: Option<usize>,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<ValueStateDescriptor>,
) {
    match expr {
        BoundExpr::IntrinsicCall { name, args } => {
            match name.as_str() {
                "vbnullstring" if args.is_empty() => descriptors.push(value_state_descriptor(
                    procedure_id,
                    ValueStateKind::VbNullString,
                    ValueStateSource::IntrinsicConstant,
                    (slot, None, None),
                    "intrinsic=vbNullString".to_string(),
                    ordinal,
                )),
                "__empty" if args.is_empty() => descriptors.push(value_state_descriptor(
                    procedure_id,
                    ValueStateKind::Empty,
                    ValueStateSource::IntrinsicConstant,
                    (slot, None, None),
                    "intrinsic=Empty".to_string(),
                    ordinal,
                )),
                "__null" if args.is_empty() => descriptors.push(value_state_descriptor(
                    procedure_id,
                    ValueStateKind::Null,
                    ValueStateSource::IntrinsicConstant,
                    (slot, None, None),
                    "intrinsic=Null".to_string(),
                    ordinal,
                )),
                "cverr" => descriptors.push(value_state_descriptor(
                    procedure_id,
                    ValueStateKind::Error,
                    ValueStateSource::IntrinsicConstant,
                    (slot, None, None),
                    "intrinsic=CVErr".to_string(),
                    ordinal,
                )),
                "cdec" => descriptors.push(value_state_descriptor(
                    procedure_id,
                    ValueStateKind::DecimalVariantSubtype,
                    ValueStateSource::IntrinsicConstant,
                    (slot, None, None),
                    "intrinsic=CDec; payload=Decimal96 Variant subtype".to_string(),
                    ordinal,
                )),
                _ => {}
            }
            for arg in args {
                collect_expr_value_states(arg, None, procedure_id, ordinal, descriptors);
            }
        }
        BoundExpr::ProcCall { args, .. } => {
            for arg in args {
                collect_expr_value_states(&arg.expr, None, procedure_id, ordinal, descriptors);
            }
        }
        BoundExpr::BinaryOp { lhs, rhs, .. } | BoundExpr::CompareOp { lhs, rhs, .. } => {
            collect_expr_value_states(lhs, None, procedure_id, ordinal, descriptors);
            collect_expr_value_states(rhs, None, procedure_id, ordinal, descriptors);
        }
        BoundExpr::UnaryOp { operand, .. } => {
            collect_expr_value_states(operand, None, procedure_id, ordinal, descriptors);
        }
        BoundExpr::IntConst(_)
        | BoundExpr::BoolConst(_)
        | BoundExpr::FloatConst(_)
        | BoundExpr::StringConst(_)
        | BoundExpr::Var(_)
        | BoundExpr::VarPtrArrayBuffer(_)
        | BoundExpr::AddConst { .. }
        | BoundExpr::SubConst { .. } => {}
    }
}

fn collect_instruction_value_state_descriptors(
    instructions: &[Instruction],
    start_pc: usize,
    end_pc: usize,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<ValueStateDescriptor>,
) {
    for (pc, instruction) in instructions
        .iter()
        .enumerate()
        .take(end_pc.min(instructions.len()))
        .skip(start_pc)
    {
        match instruction {
            Instruction::LoadEmpty { slot } => descriptors.push(value_state_descriptor(
                procedure_id,
                ValueStateKind::Empty,
                ValueStateSource::Instruction,
                (Some(*slot), Some(pc), None),
                "opcode=LoadEmpty".to_string(),
                ordinal,
            )),
            Instruction::LoadNull { slot } => descriptors.push(value_state_descriptor(
                procedure_id,
                ValueStateKind::Null,
                ValueStateSource::Instruction,
                (Some(*slot), Some(pc), None),
                "opcode=LoadNull".to_string(),
                ordinal,
            )),
            Instruction::IntrinsicCVErr { dst, .. } => descriptors.push(value_state_descriptor(
                procedure_id,
                ValueStateKind::Error,
                ValueStateSource::Instruction,
                (Some(*dst), Some(pc), None),
                "opcode=IntrinsicCVErr".to_string(),
                ordinal,
            )),
            _ => {}
        }
    }
}

fn metadata_descriptor_owner_id(metadata: &ProcedureRuntimeMetadata) -> String {
    let module_name = if metadata.module_name.trim().is_empty() {
        "<anonymous>"
    } else {
        metadata.module_name.as_str()
    };
    let entry_pc = metadata.entry_pc.to_string();
    canonical_descriptor_id(
        DescriptorFamily::Procedure,
        [
            module_name,
            metadata.procedure_name.as_str(),
            entry_pc.as_str(),
        ],
    )
}

fn value_state_descriptor(
    procedure_id: &str,
    state: ValueStateKind,
    source: ValueStateSource,
    location: (Option<usize>, Option<usize>, Option<String>),
    detail: String,
    ordinal: &mut usize,
) -> ValueStateDescriptor {
    let current = *ordinal;
    *ordinal += 1;
    let ordinal = current.to_string();
    let descriptor_id = canonical_descriptor_id(
        DescriptorFamily::ValueState,
        [
            procedure_id,
            value_state_kind_key(state),
            value_state_source_key(source),
            ordinal.as_str(),
        ],
    );
    let (slot, pc, name) = location;
    ValueStateDescriptor {
        descriptor_id,
        state,
        source,
        slot,
        pc,
        name,
        detail,
    }
}

fn value_state_kind_key(state: ValueStateKind) -> &'static str {
    match state {
        ValueStateKind::Empty => "empty",
        ValueStateKind::Null => "null",
        ValueStateKind::Error => "error",
        ValueStateKind::MissingArgument => "missing-argument",
        ValueStateKind::OmittedDefault => "omitted-default",
        ValueStateKind::Nothing => "nothing",
        ValueStateKind::VbNullString => "vbnullstring",
        ValueStateKind::DecimalVariantSubtype => "decimal-variant-subtype",
    }
}

fn value_state_source_key(source: ValueStateSource) -> &'static str {
    match source {
        ValueStateSource::SlotInitialState => "slot-initial-state",
        ValueStateSource::Instruction => "instruction",
        ValueStateSource::OptionalParameter => "optional-parameter",
        ValueStateSource::IntrinsicConstant => "intrinsic-constant",
        ValueStateSource::DeclaredTypeExtension => "declared-type-extension",
    }
}

fn build_expression_semantics_descriptors(
    metadata: &ProcedureRuntimeMetadata,
    proc: Option<&BoundProcedure>,
) -> Vec<ExpressionSemanticsDescriptor> {
    let Some(proc) = proc else {
        return Vec::new();
    };
    let procedure_id = metadata_descriptor_owner_id(metadata);
    let type_by_name = slot_type_by_name(metadata);
    let mut ordinal = 0usize;
    let mut descriptors = Vec::new();
    for stmt in &proc.body {
        collect_stmt_expression_semantics(
            stmt,
            &type_by_name,
            &procedure_id,
            &mut ordinal,
            &mut descriptors,
        );
    }
    descriptors.sort_by(|left, right| left.descriptor_id.cmp(&right.descriptor_id));
    descriptors
}

fn collect_stmt_expression_semantics(
    stmt: &BoundStmt,
    type_by_name: &HashMap<String, VbaTypeId>,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<ExpressionSemanticsDescriptor>,
) {
    match stmt {
        BoundStmt::Assign { expr, .. } => collect_expr_semantics(
            expr,
            ExpressionSourceContextDescriptor::AssignmentRhs,
            type_by_name,
            procedure_id,
            ordinal,
            descriptors,
        ),
        BoundStmt::AssignRuntimeArrayElement { indices, expr, .. } => {
            for index in indices {
                collect_expr_semantics(
                    index,
                    ExpressionSourceContextDescriptor::RuntimeArrayIndex,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
            collect_expr_semantics(
                expr,
                ExpressionSourceContextDescriptor::RuntimeArrayValue,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundStmt::MidAssign {
            start,
            count,
            value,
            ..
        } => {
            collect_expr_semantics(
                start,
                ExpressionSourceContextDescriptor::RuntimeArrayIndex,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
            if let Some(count) = count {
                collect_expr_semantics(
                    count,
                    ExpressionSourceContextDescriptor::RuntimeArrayIndex,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
            collect_expr_semantics(
                value,
                ExpressionSourceContextDescriptor::AssignmentRhs,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundStmt::IfCond {
            cond,
            then_body,
            else_body,
        } => {
            collect_cond_expression_semantics(
                cond,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
            for stmt in then_body.iter().chain(else_body.iter()) {
                collect_stmt_expression_semantics(
                    stmt,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::DoWhile { cond, body, .. } => {
            collect_cond_expression_semantics(
                cond,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
            for stmt in body {
                collect_stmt_expression_semantics(
                    stmt,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::ForRange {
            start,
            end,
            step,
            body,
            ..
        } => {
            for expr in [start, end, step] {
                collect_expr_semantics(
                    expr,
                    ExpressionSourceContextDescriptor::LoopBound,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
            for stmt in body {
                collect_stmt_expression_semantics(
                    stmt,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::ForEach {
            items,
            iterable,
            body,
            ..
        } => {
            for item in items {
                collect_expr_semantics(
                    item,
                    ExpressionSourceContextDescriptor::LoopBound,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
            if let Some(iterable) = iterable {
                collect_expr_semantics(
                    iterable,
                    ExpressionSourceContextDescriptor::LoopBound,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
            for stmt in body {
                collect_stmt_expression_semantics(
                    stmt,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::ReDimRuntime { bounds, .. } => {
            for bound in bounds {
                collect_expr_semantics(
                    &bound.upper_bound,
                    ExpressionSourceContextDescriptor::RuntimeArrayIndex,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::RaiseEvent { args, .. }
        | BoundStmt::Call { args, .. }
        | BoundStmt::AssignFromCall { args, .. } => {
            for arg in args {
                collect_expr_semantics(
                    &arg.expr,
                    ExpressionSourceContextDescriptor::CallArgument,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            collect_expr_semantics(
                expr,
                ExpressionSourceContextDescriptor::Condition,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
            for (_, body) in arms {
                for stmt in body {
                    collect_stmt_expression_semantics(
                        stmt,
                        type_by_name,
                        procedure_id,
                        ordinal,
                        descriptors,
                    );
                }
            }
            for stmt in else_body {
                collect_stmt_expression_semantics(
                    stmt,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::FileOpen {
            path, file_number, ..
        } => {
            collect_expr_semantics(
                path,
                ExpressionSourceContextDescriptor::PrintOrIo,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
            collect_expr_semantics(
                file_number,
                ExpressionSourceContextDescriptor::PrintOrIo,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundStmt::FileClose { file_number } => {
            if let Some(file_number) = file_number {
                collect_expr_semantics(
                    file_number,
                    ExpressionSourceContextDescriptor::PrintOrIo,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::FileKill { path } => collect_expr_semantics(
            path,
            ExpressionSourceContextDescriptor::PrintOrIo,
            type_by_name,
            procedure_id,
            ordinal,
            descriptors,
        ),
        BoundStmt::FilePrint { file_number, data } => {
            collect_expr_semantics(
                file_number,
                ExpressionSourceContextDescriptor::PrintOrIo,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
            collect_expr_semantics(
                data,
                ExpressionSourceContextDescriptor::PrintOrIo,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundStmt::FileWrite { file_number, data } => {
            collect_expr_semantics(
                file_number,
                ExpressionSourceContextDescriptor::PrintOrIo,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
            for item in data {
                collect_expr_semantics(
                    item,
                    ExpressionSourceContextDescriptor::PrintOrIo,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::FileInput { file_number, .. } | BoundStmt::FileLineInput { file_number, .. } => {
            collect_expr_semantics(
                file_number,
                ExpressionSourceContextDescriptor::PrintOrIo,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            )
        }
        BoundStmt::ConsolePrint { data } | BoundStmt::DebugPrint { data } => {
            collect_expr_semantics(
                data,
                ExpressionSourceContextDescriptor::PrintOrIo,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundStmt::ReDim { .. }
        | BoundStmt::Erase { .. }
        | BoundStmt::UdtAssign { .. }
        | BoundStmt::ExitDo
        | BoundStmt::ExitFor
        | BoundStmt::OnErrorResumeNext
        | BoundStmt::OnErrorGoto0
        | BoundStmt::OnErrorGotoLabel { .. }
        | BoundStmt::ResumeNext
        | BoundStmt::Resume
        | BoundStmt::ResumeLabel { .. }
        | BoundStmt::RaiseError(_)
        | BoundStmt::ErrClear
        | BoundStmt::Label { .. }
        | BoundStmt::GoTo { .. }
        | BoundStmt::GoSub { .. }
        | BoundStmt::Return
        | BoundStmt::Beep
        | BoundStmt::ExitProcedure
        | BoundStmt::ConsoleInput { .. }
        | BoundStmt::ConsoleLineInput { .. }
        | BoundStmt::Unsupported { .. } => {}
    }
}

fn collect_cond_expression_semantics(
    cond: &BoundCond,
    type_by_name: &HashMap<String, VbaTypeId>,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<ExpressionSemanticsDescriptor>,
) {
    match cond {
        BoundCond::Compare { lhs, rhs, .. } => {
            collect_expr_semantics(
                lhs,
                ExpressionSourceContextDescriptor::Condition,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
            collect_expr_semantics(
                rhs,
                ExpressionSourceContextDescriptor::Condition,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundCond::Truthy(expr) => collect_expr_semantics(
            expr,
            ExpressionSourceContextDescriptor::Condition,
            type_by_name,
            procedure_id,
            ordinal,
            descriptors,
        ),
        BoundCond::Not(inner) => collect_cond_expression_semantics(
            inner,
            type_by_name,
            procedure_id,
            ordinal,
            descriptors,
        ),
        BoundCond::And(lhs, rhs) | BoundCond::Or(lhs, rhs) => {
            collect_cond_expression_semantics(
                lhs,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
            collect_cond_expression_semantics(
                rhs,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
    }
}

fn collect_expr_semantics(
    expr: &BoundExpr,
    context: ExpressionSourceContextDescriptor,
    type_by_name: &HashMap<String, VbaTypeId>,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<ExpressionSemanticsDescriptor>,
) {
    descriptors.push(expression_semantics_descriptor(
        procedure_id,
        expr,
        context,
        type_by_name,
        ordinal,
    ));
    match expr {
        BoundExpr::BinaryOp { lhs, rhs, .. } | BoundExpr::CompareOp { lhs, rhs, .. } => {
            collect_expr_semantics(
                lhs,
                context,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
            collect_expr_semantics(
                rhs,
                context,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundExpr::UnaryOp { operand, .. } => {
            collect_expr_semantics(
                operand,
                context,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundExpr::IntrinsicCall { args, .. } => {
            for arg in args {
                collect_expr_semantics(
                    arg,
                    context,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundExpr::ProcCall { args, .. } => {
            for arg in args {
                collect_expr_semantics(
                    &arg.expr,
                    ExpressionSourceContextDescriptor::CallArgument,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundExpr::IntConst(_)
        | BoundExpr::BoolConst(_)
        | BoundExpr::FloatConst(_)
        | BoundExpr::StringConst(_)
        | BoundExpr::Var(_)
        | BoundExpr::VarPtrArrayBuffer(_)
        | BoundExpr::AddConst { .. }
        | BoundExpr::SubConst { .. } => {}
    }
}

fn expression_semantics_descriptor(
    procedure_id: &str,
    expr: &BoundExpr,
    context: ExpressionSourceContextDescriptor,
    type_by_name: &HashMap<String, VbaTypeId>,
    ordinal: &mut usize,
) -> ExpressionSemanticsDescriptor {
    let current = *ordinal;
    *ordinal += 1;
    let expression_id = format!("expr:{current}");
    let descriptor_id = canonical_descriptor_id(
        DescriptorFamily::ExpressionSemantics,
        [procedure_id, expression_id.as_str()],
    );
    let declared_type = expr_declared_type(expr, type_by_name);
    ExpressionSemanticsDescriptor {
        descriptor_id,
        expression_id,
        classification: expression_classification(expr),
        declared_type,
        value_states: expr_value_states(expr),
        carrier_hint: RuntimeCarrierKind::for_declared_type(declared_type),
        default_member_policy: expr_default_member_policy(expr),
        source_context: context,
        detail: expr_semantics_detail(expr),
    }
}

fn expression_classification(expr: &BoundExpr) -> ExpressionClassificationDescriptor {
    match expr {
        BoundExpr::IntConst(_)
        | BoundExpr::BoolConst(_)
        | BoundExpr::FloatConst(_)
        | BoundExpr::StringConst(_) => ExpressionClassificationDescriptor::Literal,
        BoundExpr::Var(_) => ExpressionClassificationDescriptor::Variable,
        BoundExpr::VarPtrArrayBuffer(_) => ExpressionClassificationDescriptor::PointerBuffer,
        BoundExpr::AddConst { .. } | BoundExpr::SubConst { .. } => {
            ExpressionClassificationDescriptor::Temporary
        }
        BoundExpr::BinaryOp { .. } | BoundExpr::UnaryOp { .. } => {
            ExpressionClassificationDescriptor::OperatorResult
        }
        BoundExpr::CompareOp { .. } => ExpressionClassificationDescriptor::ComparisonResult,
        BoundExpr::IntrinsicCall { name, .. } if name == "__oxvba_array_get" => {
            ExpressionClassificationDescriptor::ArrayElement
        }
        BoundExpr::IntrinsicCall { .. } => ExpressionClassificationDescriptor::IntrinsicResult,
        BoundExpr::ProcCall { .. } => ExpressionClassificationDescriptor::FunctionResult,
    }
}

fn expr_default_member_policy(expr: &BoundExpr) -> DefaultMemberPolicyDescriptor {
    match expr {
        BoundExpr::ProcCall { .. } => DefaultMemberPolicyDescriptor::ExplicitMember,
        BoundExpr::IntrinsicCall { name, .. }
            if matches!(name.as_str(), "dispatchinvoke" | "__oxvbaearlyinvoke") =>
        {
            DefaultMemberPolicyDescriptor::Unknown
        }
        _ => DefaultMemberPolicyDescriptor::NotApplicable,
    }
}

fn expr_semantics_detail(expr: &BoundExpr) -> String {
    match expr {
        BoundExpr::Var(name) => format!("name={}", name.to_ascii_lowercase()),
        BoundExpr::VarPtrArrayBuffer(name) => {
            format!("varptr-array-buffer={}", name.to_ascii_lowercase())
        }
        BoundExpr::AddConst { var, delta } => {
            format!("fast-add-const:{}:{delta}", var.to_ascii_lowercase())
        }
        BoundExpr::SubConst { var, delta } => {
            format!("fast-sub-const:{}:{delta}", var.to_ascii_lowercase())
        }
        BoundExpr::BinaryOp { op, .. } => format!("operator={}", arith_op_key(*op)),
        BoundExpr::CompareOp { op, .. } => format!("compare={}", compare_op_key(*op)),
        BoundExpr::UnaryOp { op, .. } => format!("unary={}", arith_op_key(*op)),
        BoundExpr::IntrinsicCall { name, .. } => format!("intrinsic={}", name.to_ascii_lowercase()),
        BoundExpr::ProcCall { name, .. } => format!("proc-call={}", name.to_ascii_lowercase()),
        BoundExpr::IntConst(value) => format!("literal=i32:{value}"),
        BoundExpr::BoolConst(value) => format!("literal=bool:{value}"),
        BoundExpr::FloatConst(_) => "literal=f64".to_string(),
        BoundExpr::StringConst(_) => "literal=string".to_string(),
    }
}

fn build_operator_semantics_descriptors(
    metadata: &ProcedureRuntimeMetadata,
    proc: Option<&BoundProcedure>,
    compare_mode: StringCompareMode,
) -> Vec<OperatorSemanticsDescriptor> {
    let Some(proc) = proc else {
        return Vec::new();
    };
    let procedure_id = metadata_descriptor_owner_id(metadata);
    let type_by_name = slot_type_by_name(metadata);
    let mut ordinal = 0usize;
    let mut descriptors = Vec::new();
    descriptors.push(deferred_iif_operator_descriptor(
        &procedure_id,
        &mut ordinal,
    ));
    for stmt in &proc.body {
        collect_stmt_operator_semantics(
            stmt,
            &type_by_name,
            compare_mode,
            &procedure_id,
            &mut ordinal,
            &mut descriptors,
        );
    }
    descriptors.sort_by(|left, right| left.descriptor_id.cmp(&right.descriptor_id));
    descriptors
}

fn collect_stmt_operator_semantics(
    stmt: &BoundStmt,
    type_by_name: &HashMap<String, VbaTypeId>,
    compare_mode: StringCompareMode,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<OperatorSemanticsDescriptor>,
) {
    match stmt {
        BoundStmt::Assign { expr, .. } => collect_expr_operator_semantics(
            expr,
            type_by_name,
            compare_mode,
            procedure_id,
            ordinal,
            descriptors,
        ),
        BoundStmt::AssignRuntimeArrayElement { indices, expr, .. } => {
            for index in indices {
                collect_expr_operator_semantics(
                    index,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
            collect_expr_operator_semantics(
                expr,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundStmt::MidAssign {
            start,
            count,
            value,
            ..
        } => {
            collect_expr_operator_semantics(
                start,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
            if let Some(count) = count {
                collect_expr_operator_semantics(
                    count,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
            collect_expr_operator_semantics(
                value,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundStmt::IfCond {
            cond,
            then_body,
            else_body,
        } => {
            collect_cond_operator_semantics(
                cond,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
            descriptors.push(branch_predicate_descriptor(
                procedure_id,
                compare_mode,
                ordinal,
            ));
            for stmt in then_body.iter().chain(else_body.iter()) {
                collect_stmt_operator_semantics(
                    stmt,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::DoWhile { cond, body, .. } => {
            collect_cond_operator_semantics(
                cond,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
            descriptors.push(branch_predicate_descriptor(
                procedure_id,
                compare_mode,
                ordinal,
            ));
            for stmt in body {
                collect_stmt_operator_semantics(
                    stmt,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::ForRange {
            start,
            end,
            step,
            body,
            ..
        } => {
            for expr in [start, end, step] {
                collect_expr_operator_semantics(
                    expr,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
            for stmt in body {
                collect_stmt_operator_semantics(
                    stmt,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::ForEach {
            items,
            iterable,
            body,
            ..
        } => {
            for item in items {
                collect_expr_operator_semantics(
                    item,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
            if let Some(iterable) = iterable {
                collect_expr_operator_semantics(
                    iterable,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
            for stmt in body {
                collect_stmt_operator_semantics(
                    stmt,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::ReDimRuntime { bounds, .. } => {
            for bound in bounds {
                collect_expr_operator_semantics(
                    &bound.upper_bound,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::RaiseEvent { args, .. }
        | BoundStmt::Call { args, .. }
        | BoundStmt::AssignFromCall { args, .. } => {
            for arg in args {
                collect_expr_operator_semantics(
                    &arg.expr,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            collect_expr_operator_semantics(
                expr,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
            for (_, body) in arms {
                for stmt in body {
                    collect_stmt_operator_semantics(
                        stmt,
                        type_by_name,
                        compare_mode,
                        procedure_id,
                        ordinal,
                        descriptors,
                    );
                }
            }
            for stmt in else_body {
                collect_stmt_operator_semantics(
                    stmt,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::FileOpen {
            path, file_number, ..
        } => {
            collect_expr_operator_semantics(
                path,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
            collect_expr_operator_semantics(
                file_number,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundStmt::FileClose { file_number } => {
            if let Some(file_number) = file_number {
                collect_expr_operator_semantics(
                    file_number,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::FileKill { path } => collect_expr_operator_semantics(
            path,
            type_by_name,
            compare_mode,
            procedure_id,
            ordinal,
            descriptors,
        ),
        BoundStmt::FilePrint { file_number, data } => {
            collect_expr_operator_semantics(
                file_number,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
            collect_expr_operator_semantics(
                data,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundStmt::FileWrite { file_number, data } => {
            collect_expr_operator_semantics(
                file_number,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
            for item in data {
                collect_expr_operator_semantics(
                    item,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::FileInput { file_number, .. } | BoundStmt::FileLineInput { file_number, .. } => {
            collect_expr_operator_semantics(
                file_number,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            )
        }
        BoundStmt::ConsolePrint { data } | BoundStmt::DebugPrint { data } => {
            collect_expr_operator_semantics(
                data,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundStmt::ReDim { .. }
        | BoundStmt::Erase { .. }
        | BoundStmt::UdtAssign { .. }
        | BoundStmt::ExitDo
        | BoundStmt::ExitFor
        | BoundStmt::OnErrorResumeNext
        | BoundStmt::OnErrorGoto0
        | BoundStmt::OnErrorGotoLabel { .. }
        | BoundStmt::ResumeNext
        | BoundStmt::Resume
        | BoundStmt::ResumeLabel { .. }
        | BoundStmt::RaiseError(_)
        | BoundStmt::ErrClear
        | BoundStmt::Label { .. }
        | BoundStmt::GoTo { .. }
        | BoundStmt::GoSub { .. }
        | BoundStmt::Return
        | BoundStmt::Beep
        | BoundStmt::ExitProcedure
        | BoundStmt::ConsoleInput { .. }
        | BoundStmt::ConsoleLineInput { .. }
        | BoundStmt::Unsupported { .. } => {}
    }
}

fn collect_cond_operator_semantics(
    cond: &BoundCond,
    type_by_name: &HashMap<String, VbaTypeId>,
    compare_mode: StringCompareMode,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<OperatorSemanticsDescriptor>,
) {
    match cond {
        BoundCond::Compare { op, lhs, rhs } => {
            descriptors.push(compare_operator_descriptor(
                procedure_id,
                *op,
                lhs,
                rhs,
                type_by_name,
                compare_mode,
                ordinal,
            ));
            collect_expr_operator_semantics(
                lhs,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
            collect_expr_operator_semantics(
                rhs,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundCond::Truthy(expr) => {
            descriptors.push(truthiness_operator_descriptor(
                procedure_id,
                expr,
                type_by_name,
                compare_mode,
                ordinal,
            ));
            collect_expr_operator_semantics(
                expr,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundCond::Not(inner) => {
            descriptors.push(logical_operator_descriptor(
                procedure_id,
                VbaOperatorDescriptor::Not,
                compare_mode,
                ordinal,
            ));
            collect_cond_operator_semantics(
                inner,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundCond::And(lhs, rhs) | BoundCond::Or(lhs, rhs) => {
            let operator = if matches!(cond, BoundCond::And(_, _)) {
                VbaOperatorDescriptor::And
            } else {
                VbaOperatorDescriptor::Or
            };
            descriptors.push(logical_operator_descriptor(
                procedure_id,
                operator,
                compare_mode,
                ordinal,
            ));
            collect_cond_operator_semantics(
                lhs,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
            collect_cond_operator_semantics(
                rhs,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
    }
}

fn collect_expr_operator_semantics(
    expr: &BoundExpr,
    type_by_name: &HashMap<String, VbaTypeId>,
    compare_mode: StringCompareMode,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<OperatorSemanticsDescriptor>,
) {
    match expr {
        BoundExpr::BinaryOp { op, lhs, rhs } => {
            descriptors.push(arith_operator_descriptor(
                procedure_id,
                *op,
                lhs,
                Some(rhs),
                type_by_name,
                compare_mode,
                ordinal,
            ));
            collect_expr_operator_semantics(
                lhs,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
            collect_expr_operator_semantics(
                rhs,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundExpr::CompareOp { op, lhs, rhs } => {
            descriptors.push(compare_operator_descriptor(
                procedure_id,
                *op,
                lhs,
                rhs,
                type_by_name,
                compare_mode,
                ordinal,
            ));
            collect_expr_operator_semantics(
                lhs,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
            collect_expr_operator_semantics(
                rhs,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundExpr::UnaryOp { op, operand } => {
            descriptors.push(arith_operator_descriptor(
                procedure_id,
                *op,
                operand,
                None,
                type_by_name,
                compare_mode,
                ordinal,
            ));
            collect_expr_operator_semantics(
                operand,
                type_by_name,
                compare_mode,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundExpr::IntrinsicCall { args, .. } => {
            for arg in args {
                collect_expr_operator_semantics(
                    arg,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundExpr::ProcCall { args, .. } => {
            for arg in args {
                collect_expr_operator_semantics(
                    &arg.expr,
                    type_by_name,
                    compare_mode,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundExpr::AddConst { .. } | BoundExpr::SubConst { .. } => {
            descriptors.push(fastpath_operator_descriptor(
                procedure_id,
                compare_mode,
                ordinal,
            ));
        }
        BoundExpr::IntConst(_)
        | BoundExpr::BoolConst(_)
        | BoundExpr::FloatConst(_)
        | BoundExpr::StringConst(_)
        | BoundExpr::Var(_)
        | BoundExpr::VarPtrArrayBuffer(_) => {}
    }
}

fn deferred_iif_operator_descriptor(
    procedure_id: &str,
    ordinal: &mut usize,
) -> OperatorSemanticsDescriptor {
    operator_descriptor(
        procedure_id,
        "OP-IIF-EAGER-DEFERRED",
        OperatorFamilyDescriptor::EvaluationOrder,
        VbaOperatorDescriptor::IIf,
        None,
        None,
        VbaTypeId::Variant,
        Vec::new(),
        Some(OperatorCompareModeDescriptor::NotApplicable),
        "unsupported:iif-not-lowered".to_string(),
        "IIf branch evaluation requires source-form/oracle coverage before descriptor consumption"
            .to_string(),
        EvaluationOrderDescriptor::UnsupportedDeferred,
        "unsupported-deferred".to_string(),
        "test-shortcoming; oracle-required".to_string(),
        ordinal,
    )
}

fn arith_operator_descriptor(
    procedure_id: &str,
    op: ArithOp,
    lhs: &BoundExpr,
    rhs: Option<&BoundExpr>,
    type_by_name: &HashMap<String, VbaTypeId>,
    compare_mode: StringCompareMode,
    ordinal: &mut usize,
) -> OperatorSemanticsDescriptor {
    let left = expr_declared_type(lhs, type_by_name);
    let right = rhs.map(|expr| expr_declared_type(expr, type_by_name));
    let operator_id = operator_seed_id(op, left, right, lhs, rhs);
    let (family, operator, helper, runtime_error) = match op {
        ArithOp::Add => (
            OperatorFamilyDescriptor::Arithmetic,
            VbaOperatorDescriptor::Add,
            "oxvba_vm::variant_add_values",
            "numeric-text-error|CVErr-type-mismatch",
        ),
        ArithOp::Sub => (
            OperatorFamilyDescriptor::Arithmetic,
            VbaOperatorDescriptor::Subtract,
            "oxvba_vm::variant_sub_values",
            "numeric-error|CVErr-type-mismatch",
        ),
        ArithOp::Mul => (
            OperatorFamilyDescriptor::Arithmetic,
            VbaOperatorDescriptor::Multiply,
            "oxvba_vm::variant_mul_values",
            "numeric-error|CVErr-type-mismatch",
        ),
        ArithOp::Div => (
            OperatorFamilyDescriptor::Arithmetic,
            VbaOperatorDescriptor::Divide,
            "oxvba_vm::variant_div_values",
            "runtime-error-11-on-zero|numeric-error",
        ),
        ArithOp::IntDiv => (
            OperatorFamilyDescriptor::Arithmetic,
            VbaOperatorDescriptor::IntegerDivide,
            "oxvba_vm::variant_intdiv_values",
            "runtime-error-11-on-zero|numeric-error",
        ),
        ArithOp::Mod => (
            OperatorFamilyDescriptor::Arithmetic,
            VbaOperatorDescriptor::Modulo,
            "oxvba_vm::variant_mod_values",
            "runtime-error-11-on-zero|numeric-error",
        ),
        ArithOp::Pow => (
            OperatorFamilyDescriptor::Arithmetic,
            VbaOperatorDescriptor::Power,
            "oxvba_vm::variant_pow_values",
            "numeric-error|CVErr-type-mismatch",
        ),
        ArithOp::Concat => (
            OperatorFamilyDescriptor::Concatenation,
            VbaOperatorDescriptor::Concatenate,
            "oxvba_vm::variant_concat_values",
            "text-conversion-errors-currently-empty",
        ),
        ArithOp::Neg => (
            OperatorFamilyDescriptor::Arithmetic,
            VbaOperatorDescriptor::Negate,
            "oxvba_vm::variant_neg_value",
            "numeric-error|CVErr-type-mismatch",
        ),
    };
    operator_descriptor(
        procedure_id,
        operator_id,
        family,
        operator,
        Some(left),
        right,
        VbaTypeId::Variant,
        expr_value_states(lhs)
            .into_iter()
            .chain(rhs.into_iter().flat_map(expr_value_states))
            .collect(),
        Some(compare_mode_descriptor(compare_mode)),
        helper.to_string(),
        runtime_error.to_string(),
        EvaluationOrderDescriptor::LeftToRightEager,
        "vm-helper-backed".to_string(),
        "metadata-package-descriptor; broader-oracle-rows-remain-open".to_string(),
        ordinal,
    )
}

fn compare_operator_descriptor(
    procedure_id: &str,
    op: CompareOp,
    lhs: &BoundExpr,
    rhs: &BoundExpr,
    type_by_name: &HashMap<String, VbaTypeId>,
    compare_mode: StringCompareMode,
    ordinal: &mut usize,
) -> OperatorSemanticsDescriptor {
    let left = expr_declared_type(lhs, type_by_name);
    let right = expr_declared_type(rhs, type_by_name);
    let operator_id = comparison_seed_id(left, right, lhs, rhs);
    operator_descriptor(
        procedure_id,
        operator_id,
        OperatorFamilyDescriptor::Comparison,
        compare_operator_descriptor_kind(op),
        Some(left),
        Some(right),
        VbaTypeId::Boolean,
        expr_value_states(lhs)
            .into_iter()
            .chain(expr_value_states(rhs))
            .collect(),
        Some(compare_mode_descriptor(compare_mode)),
        "oxvba_vm::typed_compare_variants".to_string(),
        "numeric-incompatible-error-or-current-null-falsy".to_string(),
        EvaluationOrderDescriptor::LeftToRightEager,
        "vm-helper-backed".to_string(),
        "metadata-package-descriptor; locale/oracle-rows-remain-open".to_string(),
        ordinal,
    )
}

fn truthiness_operator_descriptor(
    procedure_id: &str,
    expr: &BoundExpr,
    type_by_name: &HashMap<String, VbaTypeId>,
    compare_mode: StringCompareMode,
    ordinal: &mut usize,
) -> OperatorSemanticsDescriptor {
    operator_descriptor(
        procedure_id,
        "OP-TRUTHINESS",
        OperatorFamilyDescriptor::Truthiness,
        VbaOperatorDescriptor::Predicate,
        Some(expr_declared_type(expr, type_by_name)),
        None,
        VbaTypeId::Boolean,
        expr_value_states(expr),
        Some(compare_mode_descriptor(compare_mode)),
        "oxvba_vm::variant_truthy_value".to_string(),
        "Error-Object-Array-numeric-error".to_string(),
        EvaluationOrderDescriptor::PredicateBranch,
        "implemented-runtime-only".to_string(),
        "metadata-package-descriptor; compatibility-oracle-rows-remain-open".to_string(),
        ordinal,
    )
}

fn logical_operator_descriptor(
    procedure_id: &str,
    operator: VbaOperatorDescriptor,
    compare_mode: StringCompareMode,
    ordinal: &mut usize,
) -> OperatorSemanticsDescriptor {
    operator_descriptor(
        procedure_id,
        "OP-BOOL-NOT-AND-OR",
        OperatorFamilyDescriptor::Logical,
        operator,
        Some(VbaTypeId::Variant),
        Some(VbaTypeId::Variant),
        VbaTypeId::Variant,
        Vec::new(),
        Some(compare_mode_descriptor(compare_mode)),
        "oxvba_vm::variant_truthy_value|Instruction::BoolNot|Instruction::BoolAnd|Instruction::BoolOr"
            .to_string(),
        "truthiness-error".to_string(),
        EvaluationOrderDescriptor::LeftToRightEager,
        "implemented-runtime-only".to_string(),
        "metadata-package-descriptor; non-short-circuit-order-recorded".to_string(),
        ordinal,
    )
}

fn branch_predicate_descriptor(
    procedure_id: &str,
    compare_mode: StringCompareMode,
    ordinal: &mut usize,
) -> OperatorSemanticsDescriptor {
    operator_descriptor(
        procedure_id,
        "OP-BRANCH-JUMPIFZERO",
        OperatorFamilyDescriptor::BranchPredicate,
        VbaOperatorDescriptor::Predicate,
        Some(VbaTypeId::Variant),
        None,
        VbaTypeId::Boolean,
        Vec::new(),
        Some(compare_mode_descriptor(compare_mode)),
        "oxvba_vm::variant_truthy_value|Vm::next_pc_for_jump_if_zero_variant".to_string(),
        "truthiness-error".to_string(),
        EvaluationOrderDescriptor::PredicateBranch,
        "implemented-runtime-only".to_string(),
        "metadata-package-descriptor".to_string(),
        ordinal,
    )
}

fn fastpath_operator_descriptor(
    procedure_id: &str,
    compare_mode: StringCompareMode,
    ordinal: &mut usize,
) -> OperatorSemanticsDescriptor {
    operator_descriptor(
        procedure_id,
        "OP-TYPED-FASTPATH-I32",
        OperatorFamilyDescriptor::ImplementationFastPath,
        VbaOperatorDescriptor::Add,
        Some(VbaTypeId::Long),
        Some(VbaTypeId::Long),
        VbaTypeId::Variant,
        Vec::new(),
        Some(compare_mode_descriptor(compare_mode)),
        "Vm::fast_add_const|Vm::fast_sub_const|Vm::fast_cmp_slots".to_string(),
        "falls-back-to-Variant-helper".to_string(),
        EvaluationOrderDescriptor::RuntimeHelperDefined,
        "implemented-runtime-only-not-semantic-proof".to_string(),
        "metadata-package-descriptor; fastpath-not-parity-proof".to_string(),
        ordinal,
    )
}

#[allow(clippy::too_many_arguments)]
fn operator_descriptor(
    procedure_id: &str,
    operator_id: &str,
    family: OperatorFamilyDescriptor,
    operator: VbaOperatorDescriptor,
    left_declared_type: Option<VbaTypeId>,
    right_declared_type: Option<VbaTypeId>,
    result_declared_type: VbaTypeId,
    result_value_states: Vec<ValueStateKind>,
    compare_mode: Option<OperatorCompareModeDescriptor>,
    helper_id: String,
    runtime_error_policy: String,
    evaluation_order: EvaluationOrderDescriptor,
    current_vm_status: String,
    gap_classification: String,
    ordinal: &mut usize,
) -> OperatorSemanticsDescriptor {
    let current = *ordinal;
    *ordinal += 1;
    let ordinal_key = current.to_string();
    let descriptor_id = canonical_descriptor_id(
        DescriptorFamily::OperatorSemantics,
        [procedure_id, operator_id, ordinal_key.as_str()],
    );
    OperatorSemanticsDescriptor {
        descriptor_id,
        operator_id: operator_id.to_string(),
        family,
        operator,
        left_declared_type,
        right_declared_type,
        result_declared_type,
        result_value_states,
        compare_mode,
        helper_id,
        runtime_error_policy,
        evaluation_order,
        current_vm_status,
        gap_classification,
    }
}

fn build_coercion_descriptors(
    metadata: &ProcedureRuntimeMetadata,
    proc: Option<&BoundProcedure>,
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
) -> Vec<CoercionDescriptor> {
    let Some(proc) = proc else {
        return Vec::new();
    };
    let procedure_id = metadata_descriptor_owner_id(metadata);
    let type_by_name = slot_type_by_name(metadata);
    let mut ordinal = 0usize;
    let mut descriptors = Vec::new();
    for stmt in &proc.body {
        collect_stmt_coercions(
            stmt,
            &type_by_name,
            &procedure_id,
            &mut ordinal,
            &mut descriptors,
        );
    }
    for call_site in &metadata.call_sites {
        if matches!(
            call_site.target_kind,
            CallTargetKindDescriptor::PropertyLet | CallTargetKindDescriptor::PropertySet
        ) {
            descriptors.push(coercion_descriptor(
                &procedure_id,
                "COERCE-PROPERTY-VALUE-BYVAL",
                CoercionKindDescriptor::PropertyValueAssignment,
                VbaTypeId::Variant,
                Vec::new(),
                VbaTypeId::Variant,
                CoercionStaticStatusDescriptor::DescriptorRequired,
                RuntimeFailurePolicyDescriptor::HelperSpecific,
                "ProcedureSignatureDescriptor::PropertyValueByVal|CallSiteDescriptor".to_string(),
                "Property value assignment is runtime ByVal and package-visible".to_string(),
                "metadata-package-descriptor; broader property coercion oracle rows remain open"
                    .to_string(),
                format!("target={}", call_site.target_name.to_ascii_lowercase()),
                &mut ordinal,
            ));
        }
        for argument in &call_site.arguments {
            if let Some(descriptor) = call_entry_byval_coercion_descriptor(
                metadata,
                procedure_metadata,
                call_site,
                argument,
                &mut ordinal,
            ) {
                descriptors.push(descriptor);
            }
        }
    }
    descriptors.sort_by(|left, right| left.descriptor_id.cmp(&right.descriptor_id));
    descriptors
}

fn call_entry_byval_coercion_descriptor(
    caller_metadata: &ProcedureRuntimeMetadata,
    procedure_metadata: &BTreeMap<String, ProcedureRuntimeMetadata>,
    call_site: &CallSiteDescriptor,
    argument: &ArgumentBindingDescriptor,
    ordinal: &mut usize,
) -> Option<CoercionDescriptor> {
    if argument.binding_kind != ArgumentBindingKindDescriptor::ByValCopy {
        return None;
    }
    let source_slot = argument.source_slot?;
    let parameter_slot = argument.parameter_slot?;
    let source_slot = caller_metadata
        .slots
        .iter()
        .find(|slot| slot.slot == source_slot)?;
    let target_metadata = call_site
        .target_entry_pc
        .and_then(|entry_pc| {
            procedure_metadata
                .values()
                .find(|metadata| metadata.entry_pc == entry_pc)
        })
        .or_else(|| procedure_metadata.get(&call_site.target_name.to_ascii_lowercase()))?;
    let target_parameter = target_metadata
        .signature
        .parameters
        .iter()
        .find(|parameter| {
            parameter.slot == Some(parameter_slot)
                && argument
                    .parameter_index
                    .is_none_or(|index| parameter.index == index)
        })?;
    if target_parameter.resolved_mechanism != ResolvedParameterMechanism::ByVal {
        return None;
    }
    if source_slot.declared_type == target_parameter.declared_type {
        return None;
    }
    Some(coercion_descriptor(
        &metadata_descriptor_owner_id(caller_metadata),
        "COERCE-CALL-BYVAL-DECLARED-TARGET",
        CoercionKindDescriptor::CallLet,
        source_slot.declared_type,
        Vec::new(),
        target_parameter.declared_type,
        CoercionStaticStatusDescriptor::DescriptorRequired,
        RuntimeFailurePolicyDescriptor::HelperSpecific,
        "CallSiteDescriptor|ProcedureSignatureDescriptor|oxvba_runtime::coerce_to".to_string(),
        "VMR06-CALL-BYVAL-COERCE-001".to_string(),
        "metadata-package-descriptor; selected Long-to-Double behavior-backed, broader call-entry coercions remain open".to_string(),
        format!(
            "target={};source-slot={};parameter-slot={};helper-row=COERCE-LET-NUMERIC-WIDEN",
            call_site.target_name.to_ascii_lowercase(),
            source_slot.slot,
            parameter_slot
        ),
        ordinal,
    ))
}

fn collect_stmt_coercions(
    stmt: &BoundStmt,
    type_by_name: &HashMap<String, VbaTypeId>,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<CoercionDescriptor>,
) {
    match stmt {
        BoundStmt::Assign {
            target,
            expr,
            intent,
        } => {
            let target_type = type_by_name
                .get(&target.to_ascii_lowercase())
                .copied()
                .unwrap_or(VbaTypeId::Variant);
            descriptors.push(assignment_coercion_descriptor(
                procedure_id,
                *intent,
                expr,
                target_type,
                type_by_name,
                ordinal,
            ));
            collect_expr_coercions(expr, type_by_name, procedure_id, ordinal, descriptors);
        }
        BoundStmt::AssignRuntimeArrayElement { indices, expr, .. } => {
            descriptors.push(assignment_coercion_descriptor(
                procedure_id,
                AssignmentIntent::Implicit,
                expr,
                VbaTypeId::Variant,
                type_by_name,
                ordinal,
            ));
            for index in indices {
                collect_expr_coercions(index, type_by_name, procedure_id, ordinal, descriptors);
            }
            collect_expr_coercions(expr, type_by_name, procedure_id, ordinal, descriptors);
        }
        BoundStmt::MidAssign {
            start,
            count,
            value,
            ..
        } => {
            for expr in [Some(start), count.as_ref(), Some(value)]
                .into_iter()
                .flatten()
            {
                collect_expr_coercions(expr, type_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::IfCond {
            cond,
            then_body,
            else_body,
        } => {
            collect_cond_coercions(cond, type_by_name, procedure_id, ordinal, descriptors);
            for stmt in then_body.iter().chain(else_body.iter()) {
                collect_stmt_coercions(stmt, type_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::DoWhile { cond, body, .. } => {
            collect_cond_coercions(cond, type_by_name, procedure_id, ordinal, descriptors);
            for stmt in body {
                collect_stmt_coercions(stmt, type_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::ForRange {
            start,
            end,
            step,
            body,
            ..
        } => {
            for expr in [start, end, step] {
                collect_expr_coercions(expr, type_by_name, procedure_id, ordinal, descriptors);
            }
            for stmt in body {
                collect_stmt_coercions(stmt, type_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::ForEach {
            items,
            iterable,
            body,
            ..
        } => {
            for item in items {
                collect_expr_coercions(item, type_by_name, procedure_id, ordinal, descriptors);
            }
            if let Some(iterable) = iterable {
                collect_expr_coercions(iterable, type_by_name, procedure_id, ordinal, descriptors);
            }
            for stmt in body {
                collect_stmt_coercions(stmt, type_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::ReDimRuntime { bounds, .. } => {
            for bound in bounds {
                collect_expr_coercions(
                    &bound.upper_bound,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::RaiseEvent { args, .. }
        | BoundStmt::Call { args, .. }
        | BoundStmt::AssignFromCall { args, .. } => {
            for arg in args {
                collect_expr_coercions(&arg.expr, type_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            collect_expr_coercions(expr, type_by_name, procedure_id, ordinal, descriptors);
            for (_, body) in arms {
                for stmt in body {
                    collect_stmt_coercions(stmt, type_by_name, procedure_id, ordinal, descriptors);
                }
            }
            for stmt in else_body {
                collect_stmt_coercions(stmt, type_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::FileOpen {
            path, file_number, ..
        } => {
            collect_expr_coercions(path, type_by_name, procedure_id, ordinal, descriptors);
            collect_expr_coercions(
                file_number,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundStmt::FileClose { file_number } => {
            if let Some(file_number) = file_number {
                collect_expr_coercions(
                    file_number,
                    type_by_name,
                    procedure_id,
                    ordinal,
                    descriptors,
                );
            }
        }
        BoundStmt::FileKill { path } => {
            collect_expr_coercions(path, type_by_name, procedure_id, ordinal, descriptors);
        }
        BoundStmt::FilePrint { file_number, data } => {
            collect_expr_coercions(
                file_number,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
            collect_expr_coercions(data, type_by_name, procedure_id, ordinal, descriptors);
        }
        BoundStmt::FileWrite { file_number, data } => {
            collect_expr_coercions(
                file_number,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
            for item in data {
                collect_expr_coercions(item, type_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::FileInput { file_number, .. } | BoundStmt::FileLineInput { file_number, .. } => {
            collect_expr_coercions(
                file_number,
                type_by_name,
                procedure_id,
                ordinal,
                descriptors,
            );
        }
        BoundStmt::ConsolePrint { data } | BoundStmt::DebugPrint { data } => {
            collect_expr_coercions(data, type_by_name, procedure_id, ordinal, descriptors);
        }
        BoundStmt::ReDim { .. }
        | BoundStmt::Erase { .. }
        | BoundStmt::UdtAssign { .. }
        | BoundStmt::ExitDo
        | BoundStmt::ExitFor
        | BoundStmt::OnErrorResumeNext
        | BoundStmt::OnErrorGoto0
        | BoundStmt::OnErrorGotoLabel { .. }
        | BoundStmt::ResumeNext
        | BoundStmt::Resume
        | BoundStmt::ResumeLabel { .. }
        | BoundStmt::RaiseError(_)
        | BoundStmt::ErrClear
        | BoundStmt::Label { .. }
        | BoundStmt::GoTo { .. }
        | BoundStmt::GoSub { .. }
        | BoundStmt::Return
        | BoundStmt::Beep
        | BoundStmt::ExitProcedure
        | BoundStmt::ConsoleInput { .. }
        | BoundStmt::ConsoleLineInput { .. }
        | BoundStmt::Unsupported { .. } => {}
    }
}

fn collect_cond_coercions(
    cond: &BoundCond,
    type_by_name: &HashMap<String, VbaTypeId>,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<CoercionDescriptor>,
) {
    match cond {
        BoundCond::Compare { lhs, rhs, .. } => {
            collect_expr_coercions(lhs, type_by_name, procedure_id, ordinal, descriptors);
            collect_expr_coercions(rhs, type_by_name, procedure_id, ordinal, descriptors);
        }
        BoundCond::Truthy(expr) => {
            descriptors.push(coercion_descriptor(
                procedure_id,
                "COERCE-VM-TRUTHINESS",
                CoercionKindDescriptor::HelperCompatibility,
                expr_declared_type(expr, type_by_name),
                expr_value_states(expr),
                VbaTypeId::Boolean,
                CoercionStaticStatusDescriptor::Runtime,
                RuntimeFailurePolicyDescriptor::NumericError,
                "oxvba_vm::variant_truthy_value".to_string(),
                "crates/oxvba-vm/src/semantics.rs::variant_truthy_value".to_string(),
                "metadata-package-descriptor; compatibility-oracle-rows-remain-open".to_string(),
                "branch predicate truthiness".to_string(),
                ordinal,
            ));
            collect_expr_coercions(expr, type_by_name, procedure_id, ordinal, descriptors);
        }
        BoundCond::Not(inner) => {
            collect_cond_coercions(inner, type_by_name, procedure_id, ordinal, descriptors)
        }
        BoundCond::And(lhs, rhs) | BoundCond::Or(lhs, rhs) => {
            collect_cond_coercions(lhs, type_by_name, procedure_id, ordinal, descriptors);
            collect_cond_coercions(rhs, type_by_name, procedure_id, ordinal, descriptors);
        }
    }
}

fn collect_expr_coercions(
    expr: &BoundExpr,
    type_by_name: &HashMap<String, VbaTypeId>,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<CoercionDescriptor>,
) {
    match expr {
        BoundExpr::IntrinsicCall { name, args } => {
            if let Some(target) = explicit_conversion_target_type(name) {
                let source = args
                    .first()
                    .map(|arg| expr_declared_type(arg, type_by_name))
                    .unwrap_or(VbaTypeId::Variant);
                descriptors.push(coercion_descriptor(
                    procedure_id,
                    explicit_conversion_coercion_id(name),
                    CoercionKindDescriptor::ExplicitConversionIntrinsic,
                    source,
                    args.first().map(expr_value_states).unwrap_or_default(),
                    target,
                    CoercionStaticStatusDescriptor::Runtime,
                    RuntimeFailurePolicyDescriptor::HelperSpecific,
                    format!("intrinsic={}", name.to_ascii_lowercase()),
                    "Intrinsic conversion lowering".to_string(),
                    "metadata-package-descriptor; helper-specific runtime behavior".to_string(),
                    format!("conversion={}", name.to_ascii_lowercase()),
                    ordinal,
                ));
            }
            for arg in args {
                collect_expr_coercions(arg, type_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundExpr::BinaryOp { lhs, rhs, .. } | BoundExpr::CompareOp { lhs, rhs, .. } => {
            collect_expr_coercions(lhs, type_by_name, procedure_id, ordinal, descriptors);
            collect_expr_coercions(rhs, type_by_name, procedure_id, ordinal, descriptors);
        }
        BoundExpr::UnaryOp { operand, .. } => {
            collect_expr_coercions(operand, type_by_name, procedure_id, ordinal, descriptors);
        }
        BoundExpr::ProcCall { args, .. } => {
            for arg in args {
                collect_expr_coercions(&arg.expr, type_by_name, procedure_id, ordinal, descriptors);
            }
        }
        BoundExpr::IntConst(_)
        | BoundExpr::BoolConst(_)
        | BoundExpr::FloatConst(_)
        | BoundExpr::StringConst(_)
        | BoundExpr::Var(_)
        | BoundExpr::VarPtrArrayBuffer(_)
        | BoundExpr::AddConst { .. }
        | BoundExpr::SubConst { .. } => {}
    }
}

fn assignment_coercion_descriptor(
    procedure_id: &str,
    intent: AssignmentIntent,
    expr: &BoundExpr,
    target_type: VbaTypeId,
    type_by_name: &HashMap<String, VbaTypeId>,
    ordinal: &mut usize,
) -> CoercionDescriptor {
    let source_type = expr_declared_type(expr, type_by_name);
    let source_states = expr_value_states(expr);
    let kind = match intent {
        AssignmentIntent::Set => CoercionKindDescriptor::Set,
        AssignmentIntent::Implicit | AssignmentIntent::Let => CoercionKindDescriptor::Let,
    };
    let coercion_id = assignment_coercion_id(kind, source_type, target_type, &source_states);
    let static_status = if source_type == target_type {
        CoercionStaticStatusDescriptor::Identity
    } else {
        CoercionStaticStatusDescriptor::Runtime
    };
    coercion_descriptor(
        procedure_id,
        coercion_id,
        kind,
        source_type,
        source_states,
        target_type,
        static_status,
        RuntimeFailurePolicyDescriptor::HelperSpecific,
        coercion_helper_id(coercion_id).to_string(),
        "docs/validation/VBA_COERCION_SEED_TABLE_V1.csv".to_string(),
        "metadata-package-descriptor; behavior remains current VM/helper-backed".to_string(),
        format!("assignment-intent={}", assignment_intent_key(intent)),
        ordinal,
    )
}

#[allow(clippy::too_many_arguments)]
fn coercion_descriptor(
    procedure_id: &str,
    coercion_id: &str,
    kind: CoercionKindDescriptor,
    source_declared_type: VbaTypeId,
    source_value_states: Vec<ValueStateKind>,
    target_declared_type: VbaTypeId,
    static_status: CoercionStaticStatusDescriptor,
    runtime_failure: RuntimeFailurePolicyDescriptor,
    helper_id: String,
    evidence_anchor: String,
    gap_classification: String,
    detail: String,
    ordinal: &mut usize,
) -> CoercionDescriptor {
    let current = *ordinal;
    *ordinal += 1;
    let ordinal_key = current.to_string();
    let descriptor_id = canonical_descriptor_id(
        DescriptorFamily::Coercion,
        [procedure_id, coercion_id, ordinal_key.as_str()],
    );
    CoercionDescriptor {
        descriptor_id,
        coercion_id: coercion_id.to_string(),
        kind,
        source_declared_type,
        source_value_states,
        target_declared_type,
        static_status,
        runtime_failure,
        helper_id,
        evidence_anchor,
        gap_classification,
        detail,
    }
}

fn build_name_binding_descriptors(
    metadata: &ProcedureRuntimeMetadata,
    proc: Option<&BoundProcedure>,
    enum_descriptors: &[BoundEnumDescriptor],
) -> Vec<NameBindingDescriptor> {
    let procedure_id = metadata_descriptor_owner_id(metadata);
    let mut descriptors = Vec::new();
    let mut ordinal = 0usize;
    descriptors.push(name_binding_descriptor(
        &procedure_id,
        "NAME-BINDING-PROCEDURE-POLICY",
        metadata.procedure_name.clone(),
        NameBindingKindDescriptor::Policy,
        NameBindingPrecedenceDescriptor::LocalProcedureScope,
        None,
        vec![
            "duplicate-declaration-diagnostic=compiler-current".to_string(),
            "ambiguous-member-diagnostic=compiler-current-or-deferred".to_string(),
            "with-context=deferred".to_string(),
            "imported-member=deferred".to_string(),
            "default-member=deferred-to-object-member-binding".to_string(),
        ],
        "local-scope-before-module-members".to_string(),
        &mut ordinal,
    ));
    for enum_descriptor in enum_descriptors {
        descriptors.push(name_binding_descriptor(
            &procedure_id,
            "NAME-BINDING-ENUM-TYPE",
            enum_descriptor.type_name.clone(),
            NameBindingKindDescriptor::ModuleScope,
            NameBindingPrecedenceDescriptor::ModuleScope,
            Some(format!(
                "enum:{}",
                enum_descriptor.type_name.to_ascii_lowercase()
            )),
            vec![
                "enum-base-type=Long".to_string(),
                format!("enum-visibility={}", if enum_descriptor.is_public { "public" } else { "private-or-default" }),
                format!("enum-member-count={}", enum_descriptor.members.len()),
            ],
            "enum type is currently lowered as module constants; package row preserves nominal identity".to_string(),
            &mut ordinal,
        ));
        for member in &enum_descriptor.members {
            descriptors.push(name_binding_descriptor(
                &procedure_id,
                "NAME-BINDING-ENUM-MEMBER",
                member.name.clone(),
                NameBindingKindDescriptor::ModuleScope,
                NameBindingPrecedenceDescriptor::ModuleScope,
                Some(format!(
                    "enum:{}:{}={}",
                    enum_descriptor.type_name.to_ascii_lowercase(),
                    member.name.to_ascii_lowercase(),
                    member.value
                )),
                vec![
                    "enum-member-base-type=Long".to_string(),
                    format!(
                        "enum-type={}",
                        enum_descriptor.type_name.to_ascii_lowercase()
                    ),
                    format!("enum-ordinal={}", member.ordinal),
                    format!("enum-explicit-value={}", member.explicit_value),
                ],
                format!(
                    "enum member value={} carrier=i32 current-lowering=module-constant",
                    member.value
                ),
                &mut ordinal,
            ));
        }
    }
    for slot in &metadata.slots {
        let (kind, precedence) = match slot.kind {
            ProcedureRuntimeSlotKind::Parameter => (
                NameBindingKindDescriptor::Parameter,
                NameBindingPrecedenceDescriptor::LocalProcedureScope,
            ),
            ProcedureRuntimeSlotKind::ReturnValue => (
                NameBindingKindDescriptor::ReturnValue,
                NameBindingPrecedenceDescriptor::LocalProcedureScope,
            ),
            ProcedureRuntimeSlotKind::Local | ProcedureRuntimeSlotKind::CompilerGenerated => (
                NameBindingKindDescriptor::Local,
                NameBindingPrecedenceDescriptor::LocalProcedureScope,
            ),
            ProcedureRuntimeSlotKind::Temporary => continue,
        };
        descriptors.push(name_binding_descriptor(
            &procedure_id,
            "NAME-BINDING-SLOT",
            slot.name.clone(),
            kind,
            precedence,
            Some(format!("slot:{}", slot.slot)),
            Vec::new(),
            format!("declared-type={}", slot.declared_type.registry_key()),
            &mut ordinal,
        ));
    }
    if let Some(proc) = proc {
        for duplicate in &proc.duplicate_declarations {
            descriptors.push(name_binding_descriptor(
                &procedure_id,
                "NAME-BINDING-AMBIGUITY-DIAGNOSTIC",
                duplicate.clone(),
                NameBindingKindDescriptor::AmbiguityDiagnostic,
                NameBindingPrecedenceDescriptor::CompilerDiagnostic,
                None,
                vec!["duplicate-declaration-diagnostic=compiler-current".to_string()],
                "duplicate declaration blocks package execution until resolved".to_string(),
                &mut ordinal,
            ));
        }
        if matches!(
            metadata.signature.kind,
            ProcedureKindDescriptor::PropertyGet
                | ProcedureKindDescriptor::PropertyLet
                | ProcedureKindDescriptor::PropertySet
        ) {
            descriptors.push(name_binding_descriptor(
                &procedure_id,
                "NAME-BINDING-PROPERTY-ACCESSOR",
                property_base_name(&metadata.procedure_name),
                NameBindingKindDescriptor::PropertyAccessor,
                NameBindingPrecedenceDescriptor::PropertyAccessor,
                Some(metadata.procedure_name.clone()),
                Vec::new(),
                "property accessor participates in member binding table".to_string(),
                &mut ordinal,
            ));
        }
    }
    descriptors.sort_by(|left, right| left.descriptor_id.cmp(&right.descriptor_id));
    descriptors
}

#[allow(clippy::too_many_arguments)]
fn name_binding_descriptor(
    procedure_id: &str,
    binding_id: &str,
    name: String,
    binding_kind: NameBindingKindDescriptor,
    precedence: NameBindingPrecedenceDescriptor,
    target: Option<String>,
    diagnostics: Vec<String>,
    detail: String,
    ordinal: &mut usize,
) -> NameBindingDescriptor {
    let current = *ordinal;
    *ordinal += 1;
    let ordinal_key = current.to_string();
    let descriptor_id = canonical_descriptor_id(
        DescriptorFamily::NameBinding,
        [
            procedure_id,
            binding_id,
            name.as_str(),
            ordinal_key.as_str(),
        ],
    );
    NameBindingDescriptor {
        descriptor_id,
        binding_id: binding_id.to_string(),
        name,
        binding_kind,
        precedence,
        target,
        diagnostics,
        detail,
    }
}

fn build_object_member_binding_descriptors(
    metadata: &ProcedureRuntimeMetadata,
    proc: Option<&BoundProcedure>,
) -> Vec<ObjectMemberBindingDescriptor> {
    let procedure_id = metadata_descriptor_owner_id(metadata);
    let mut descriptors = Vec::new();
    let mut ordinal = 0usize;
    if matches!(
        metadata.signature.kind,
        ProcedureKindDescriptor::PropertyGet
            | ProcedureKindDescriptor::PropertyLet
            | ProcedureKindDescriptor::PropertySet
    ) {
        descriptors.push(property_signature_member_binding(
            &procedure_id,
            &metadata.signature,
            &mut ordinal,
        ));
    }
    for call_site in &metadata.call_sites {
        if let Some(descriptor) = call_site_member_binding(&procedure_id, call_site, &mut ordinal) {
            descriptors.push(descriptor);
        }
    }
    if let Some(proc) = proc {
        for stmt in &proc.body {
            collect_stmt_member_bindings(stmt, &procedure_id, &mut ordinal, &mut descriptors);
        }
    }
    descriptors.sort_by(|left, right| left.descriptor_id.cmp(&right.descriptor_id));
    descriptors
}

fn property_signature_member_binding(
    procedure_id: &str,
    signature: &ProcedureSignatureDescriptor,
    ordinal: &mut usize,
) -> ObjectMemberBindingDescriptor {
    let (binding_id, member_kind, argument_policy, result_type) = match signature.kind {
        ProcedureKindDescriptor::PropertyGet => (
            "BIND-PROPERTY-GET",
            ObjectMemberKindDescriptor::PropertyGet,
            "source-arguments-map-to-property-parameters",
            signature.return_type,
        ),
        ProcedureKindDescriptor::PropertyLet => (
            "BIND-PROPERTY-LET-VALUE",
            ObjectMemberKindDescriptor::PropertyLet,
            "value-param-is-runtime-ByVal",
            signature
                .parameters
                .last()
                .map(|parameter| parameter.declared_type),
        ),
        ProcedureKindDescriptor::PropertySet => (
            "BIND-PROPERTY-SET-OBJECT",
            ObjectMemberKindDescriptor::PropertySet,
            "set-coercion-object-value-param",
            signature
                .parameters
                .last()
                .map(|parameter| parameter.declared_type),
        ),
        _ => unreachable!(),
    };
    object_member_binding_descriptor(
        procedure_id,
        binding_id,
        VbaTypeId::Object,
        property_base_name(&signature.procedure_name),
        member_kind,
        false,
        signature.property_group.clone(),
        MemberDispatchKindDescriptor::ProjectRoute,
        argument_policy.to_string(),
        result_type,
        "preserve-receiver-identity".to_string(),
        "route-cache-by-name-arity-and-kind".to_string(),
        "missing-or-ambiguous-accessor-diagnostic".to_string(),
        "metadata-and-runtime-backed-subset".to_string(),
        "metadata-package-descriptor; broader object/property oracle rows remain open".to_string(),
        ordinal,
    )
}

fn call_site_member_binding(
    procedure_id: &str,
    call_site: &CallSiteDescriptor,
    ordinal: &mut usize,
) -> Option<ObjectMemberBindingDescriptor> {
    match call_site.target_kind {
        CallTargetKindDescriptor::PropertyGet
        | CallTargetKindDescriptor::PropertyLet
        | CallTargetKindDescriptor::PropertySet => {
            let signature = ProcedureSignatureDescriptor {
                procedure_name: call_site.target_name.clone(),
                kind: match call_site.target_kind {
                    CallTargetKindDescriptor::PropertyGet => ProcedureKindDescriptor::PropertyGet,
                    CallTargetKindDescriptor::PropertyLet => ProcedureKindDescriptor::PropertyLet,
                    CallTargetKindDescriptor::PropertySet => ProcedureKindDescriptor::PropertySet,
                    _ => ProcedureKindDescriptor::Unknown,
                },
                parameters: Vec::new(),
                return_type: call_site.return_value.as_ref().map(|_| VbaTypeId::Variant),
                return_slot: call_site
                    .return_value
                    .as_ref()
                    .and_then(|return_value| return_value.return_slot),
                property_group: Some(property_base_name(&call_site.target_name)),
                implicit_current_object: None,
            };
            Some(property_signature_member_binding(
                procedure_id,
                &signature,
                ordinal,
            ))
        }
        CallTargetKindDescriptor::LateBoundDefaultMember => Some(object_member_binding_descriptor(
            procedure_id,
            "BIND-LET-DEFAULT-MEMBER",
            VbaTypeId::Object,
            call_site.target_name.clone(),
            ObjectMemberKindDescriptor::DefaultMemberGet,
            true,
            Some("default-member-group".to_string()),
            MemberDispatchKindDescriptor::DefaultMemberFallback,
            "let-context-binds-object-default-value".to_string(),
            Some(VbaTypeId::Variant),
            "receiver-object-identity-preserved-while-value-is-read".to_string(),
            "route-cache-by-default-member-id".to_string(),
            "Set-context-must-not-use-default-member-fallback".to_string(),
            "current-policy-evidence-only".to_string(),
            "metadata-package-descriptor; oracle-required".to_string(),
            ordinal,
        )),
        _ => None,
    }
}

fn collect_stmt_member_bindings(
    stmt: &BoundStmt,
    procedure_id: &str,
    ordinal: &mut usize,
    descriptors: &mut Vec<ObjectMemberBindingDescriptor>,
) {
    match stmt {
        BoundStmt::Assign {
            target,
            intent: AssignmentIntent::Set,
            ..
        } => descriptors.push(object_member_binding_descriptor(
            procedure_id,
            "BIND-SET-OBJECT-ASSIGN",
            VbaTypeId::Object,
            target.clone(),
            ObjectMemberKindDescriptor::SetAssignment,
            false,
            None,
            MemberDispatchKindDescriptor::None,
            "set-coercion-object-expression".to_string(),
            Some(VbaTypeId::Object),
            "preserve-assigned-object-identity-and-release-replaced-reference".to_string(),
            "not-applicable".to_string(),
            "incompatible-object-shapes-must-diagnose-or-runtime-error".to_string(),
            "current-slot-policy-evidence-backed-subset".to_string(),
            "metadata-package-descriptor; full binding descriptor remains open".to_string(),
            ordinal,
        )),
        BoundStmt::IfCond {
            then_body,
            else_body,
            ..
        } => {
            for stmt in then_body.iter().chain(else_body.iter()) {
                collect_stmt_member_bindings(stmt, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::DoWhile { body, .. } | BoundStmt::ForRange { body, .. } => {
            for stmt in body {
                collect_stmt_member_bindings(stmt, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::ForEach { body, .. } => {
            for stmt in body {
                collect_stmt_member_bindings(stmt, procedure_id, ordinal, descriptors);
            }
        }
        BoundStmt::SelectCase {
            arms, else_body, ..
        } => {
            for (_, body) in arms {
                for stmt in body {
                    collect_stmt_member_bindings(stmt, procedure_id, ordinal, descriptors);
                }
            }
            for stmt in else_body {
                collect_stmt_member_bindings(stmt, procedure_id, ordinal, descriptors);
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn object_member_binding_descriptor(
    procedure_id: &str,
    binding_id: &str,
    target_declared_type: VbaTypeId,
    member_name: String,
    member_kind: ObjectMemberKindDescriptor,
    default_member: bool,
    property_accessor_group: Option<String>,
    dispatch_kind: MemberDispatchKindDescriptor,
    argument_binding_policy: String,
    result_declared_type: Option<VbaTypeId>,
    object_identity_policy: String,
    cache_invalidation_policy: String,
    fallback_or_unsupported_policy: String,
    current_vm_status: String,
    gap_classification: String,
    ordinal: &mut usize,
) -> ObjectMemberBindingDescriptor {
    let current = *ordinal;
    *ordinal += 1;
    let ordinal_key = current.to_string();
    let descriptor_id = canonical_descriptor_id(
        DescriptorFamily::ObjectMemberBinding,
        [
            procedure_id,
            binding_id,
            member_name.as_str(),
            ordinal_key.as_str(),
        ],
    );
    ObjectMemberBindingDescriptor {
        descriptor_id,
        binding_id: binding_id.to_string(),
        target_declared_type,
        member_name,
        member_kind,
        default_member,
        property_accessor_group,
        dispatch_kind,
        argument_binding_policy,
        result_declared_type,
        object_identity_policy,
        cache_invalidation_policy,
        fallback_or_unsupported_policy,
        current_vm_status,
        gap_classification,
    }
}

fn slot_type_by_name(metadata: &ProcedureRuntimeMetadata) -> HashMap<String, VbaTypeId> {
    metadata
        .slots
        .iter()
        .map(|slot| (slot.name.to_ascii_lowercase(), slot.declared_type))
        .collect()
}

fn expr_declared_type(expr: &BoundExpr, type_by_name: &HashMap<String, VbaTypeId>) -> VbaTypeId {
    match expr {
        BoundExpr::IntConst(_) | BoundExpr::AddConst { .. } | BoundExpr::SubConst { .. } => {
            VbaTypeId::Long
        }
        BoundExpr::BoolConst(_) => VbaTypeId::Boolean,
        BoundExpr::FloatConst(_) => VbaTypeId::Double,
        BoundExpr::StringConst(_) => VbaTypeId::String,
        BoundExpr::Var(name) => type_by_name
            .get(&name.to_ascii_lowercase())
            .copied()
            .unwrap_or(VbaTypeId::Variant),
        BoundExpr::VarPtrArrayBuffer(_) => VbaTypeId::LongPtr,
        BoundExpr::BinaryOp { op, .. } => match op {
            ArithOp::Concat => VbaTypeId::String,
            _ => VbaTypeId::Variant,
        },
        BoundExpr::CompareOp { .. } => VbaTypeId::Boolean,
        BoundExpr::UnaryOp { .. } => VbaTypeId::Variant,
        BoundExpr::IntrinsicCall { name, args } => intrinsic_result_type(name, args, type_by_name),
        BoundExpr::ProcCall { .. } => VbaTypeId::Variant,
    }
}

fn intrinsic_result_type(
    name: &str,
    args: &[BoundExpr],
    type_by_name: &HashMap<String, VbaTypeId>,
) -> VbaTypeId {
    match name {
        "__empty" | "__null" | "cverr" | "cdec" | "array" | "__oxvba_array_get" => {
            VbaTypeId::Variant
        }
        "vbnullstring" | "cstr" | "str" | "left" | "right" | "mid" | "replace" | "lcase"
        | "ucase" | "trim" | "ltrim" | "rtrim" | "space" | "chr" | "format" | "hex" | "oct"
        | "typename" => VbaTypeId::String,
        "cint" => VbaTypeId::Integer,
        "clng" | "len" | "asc" | "lbound" | "ubound" | "vartype" | "instr" | "instrrev" => {
            VbaTypeId::Long
        }
        "cdbl" | "val" | "timer" | "sqr" | "sin" | "cos" | "log" | "exp" | "atn" | "tan" => {
            VbaTypeId::Double
        }
        "cbool" | "isarray" | "isnumeric" | "isdate" | "isobject" | "isempty" | "isnull"
        | "iserror" | "typeofis" => VbaTypeId::Boolean,
        "cdate" | "date" | "time" | "now" | "datevalue" | "timevalue" | "dateserial"
        | "timeserial" | "dateadd" => VbaTypeId::Date,
        "csng" => VbaTypeId::Single,
        "cbyte" => VbaTypeId::Byte,
        "ccur" => VbaTypeId::Currency,
        "varptr" | "strptr" | "objptr" => VbaTypeId::LongPtr,
        _ => args
            .first()
            .map(|arg| expr_declared_type(arg, type_by_name))
            .unwrap_or(VbaTypeId::Variant),
    }
}

fn expr_value_states(expr: &BoundExpr) -> Vec<ValueStateKind> {
    match expr {
        BoundExpr::IntrinsicCall { name, .. } => match name.as_str() {
            "__empty" => vec![ValueStateKind::Empty],
            "__null" => vec![ValueStateKind::Null],
            "cverr" => vec![ValueStateKind::Error],
            "cdec" => vec![ValueStateKind::DecimalVariantSubtype],
            "vbnullstring" => vec![ValueStateKind::VbNullString],
            _ => Vec::new(),
        },
        BoundExpr::BinaryOp { lhs, rhs, .. } | BoundExpr::CompareOp { lhs, rhs, .. } => {
            expr_value_states(lhs)
                .into_iter()
                .chain(expr_value_states(rhs))
                .collect()
        }
        BoundExpr::UnaryOp { operand, .. } => expr_value_states(operand),
        _ => Vec::new(),
    }
}

fn explicit_conversion_target_type(name: &str) -> Option<VbaTypeId> {
    match name {
        "cint" => Some(VbaTypeId::Integer),
        "clng" => Some(VbaTypeId::Long),
        "cdbl" => Some(VbaTypeId::Double),
        "cstr" | "str" => Some(VbaTypeId::String),
        "cbool" => Some(VbaTypeId::Boolean),
        "cdate" => Some(VbaTypeId::Date),
        "csng" => Some(VbaTypeId::Single),
        "cbyte" => Some(VbaTypeId::Byte),
        "ccur" => Some(VbaTypeId::Currency),
        "cdec" => Some(VbaTypeId::Variant),
        _ => None,
    }
}

fn explicit_conversion_coercion_id(name: &str) -> &'static str {
    match name {
        "cstr" | "str" => "COERCE-LET-STRING-BSTR",
        "cbool" => "COERCE-LET-BOOLEAN-NUMERIC",
        "cdec" => "COERCE-VARIANT-DECIMAL-PAYLOAD",
        _ => "COERCE-LET-NUMERIC-WIDEN",
    }
}

fn assignment_coercion_id(
    kind: CoercionKindDescriptor,
    source: VbaTypeId,
    target: VbaTypeId,
    source_states: &[ValueStateKind],
) -> &'static str {
    if kind == CoercionKindDescriptor::Set {
        return if source_states.contains(&ValueStateKind::Nothing) {
            "COERCE-SET-NOTHING"
        } else {
            "COERCE-SET-OBJECT-ASSIGN"
        };
    }
    if source_states.contains(&ValueStateKind::Empty) {
        return if target == VbaTypeId::String {
            "COERCE-LET-EMPTY-STRING"
        } else {
            "COERCE-LET-EMPTY-NUMERIC"
        };
    }
    if source_states.contains(&ValueStateKind::DecimalVariantSubtype) {
        return "COERCE-VARIANT-DECIMAL-PAYLOAD";
    }
    if source == target {
        if target == VbaTypeId::Variant {
            "COERCE-LET-VARIANT-PRESERVE"
        } else {
            "COERCE-LET-IDENTITY"
        }
    } else if source == VbaTypeId::Boolean && is_numeric_type(target) {
        "COERCE-LET-BOOLEAN-NUMERIC"
    } else if target == VbaTypeId::String {
        "COERCE-LET-STRING-BSTR"
    } else if is_numeric_type(source) && is_numeric_type(target) {
        "COERCE-LET-NUMERIC-WIDEN"
    } else {
        "COERCE-LET-IDENTITY"
    }
}

fn coercion_helper_id(coercion_id: &str) -> &'static str {
    match coercion_id {
        "COERCE-LET-STRING-BSTR" | "COERCE-LET-EMPTY-STRING" => {
            "oxvba_runtime::variant_to_vba_string"
        }
        "COERCE-VM-TRUTHINESS" => "oxvba_vm::variant_truthy_value",
        "COERCE-SET-OBJECT-ASSIGN" | "COERCE-SET-NOTHING" => "ObjectRef assignment",
        _ => "oxvba_runtime::coerce_to",
    }
}

fn operator_seed_id(
    op: ArithOp,
    left: VbaTypeId,
    right: Option<VbaTypeId>,
    lhs: &BoundExpr,
    rhs: Option<&BoundExpr>,
) -> &'static str {
    match op {
        ArithOp::Concat => "OP-CONCAT-AMPERSAND",
        ArithOp::Add if left == VbaTypeId::String && right == Some(VbaTypeId::String) => {
            "OP-ADD-STRING-STRING"
        }
        ArithOp::Add
            if expr_value_states(lhs).contains(&ValueStateKind::Null)
                || rhs
                    .map(expr_value_states)
                    .unwrap_or_default()
                    .contains(&ValueStateKind::Null) =>
        {
            "OP-ADD-F64-COMPAT"
        }
        ArithOp::Add if left == VbaTypeId::Double || right == Some(VbaTypeId::Double) => {
            "OP-ADD-F64-COMPAT"
        }
        ArithOp::Add => "OP-ADD-I32-COMPAT",
        ArithOp::Sub | ArithOp::Mul => "OP-SUB-MUL-I32-F64",
        ArithOp::Div | ArithOp::IntDiv | ArithOp::Mod => "OP-DIVISION",
        ArithOp::Pow | ArithOp::Neg => "OP-POWER-NEG",
    }
}

fn comparison_seed_id(
    left: VbaTypeId,
    right: VbaTypeId,
    lhs: &BoundExpr,
    rhs: &BoundExpr,
) -> &'static str {
    let states = expr_value_states(lhs)
        .into_iter()
        .chain(expr_value_states(rhs))
        .collect::<Vec<_>>();
    if states.contains(&ValueStateKind::Null) {
        "OP-CMP-NULL"
    } else if states.contains(&ValueStateKind::Empty)
        && (left == VbaTypeId::String || right == VbaTypeId::String)
    {
        "OP-CMP-EMPTY-STRING"
    } else if left == VbaTypeId::String && right == VbaTypeId::String {
        "OP-CMP-STRING"
    } else {
        "OP-CMP-NUMERIC"
    }
}

fn compare_operator_descriptor_kind(op: CompareOp) -> VbaOperatorDescriptor {
    match op {
        CompareOp::Eq => VbaOperatorDescriptor::Equal,
        CompareOp::Ne => VbaOperatorDescriptor::NotEqual,
        CompareOp::Lt => VbaOperatorDescriptor::LessThan,
        CompareOp::Le => VbaOperatorDescriptor::LessOrEqual,
        CompareOp::Gt => VbaOperatorDescriptor::GreaterThan,
        CompareOp::Ge => VbaOperatorDescriptor::GreaterOrEqual,
        CompareOp::Like => VbaOperatorDescriptor::Like,
    }
}

fn compare_mode_descriptor(mode: StringCompareMode) -> OperatorCompareModeDescriptor {
    match mode {
        StringCompareMode::Binary => OperatorCompareModeDescriptor::Binary,
        StringCompareMode::Text => OperatorCompareModeDescriptor::Text,
    }
}

fn arith_op_key(op: ArithOp) -> &'static str {
    match op {
        ArithOp::Add => "add",
        ArithOp::Sub => "subtract",
        ArithOp::Mul => "multiply",
        ArithOp::Div => "divide",
        ArithOp::IntDiv => "integer-divide",
        ArithOp::Mod => "modulo",
        ArithOp::Pow => "power",
        ArithOp::Concat => "concatenate",
        ArithOp::Neg => "negate",
    }
}

fn compare_op_key(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Eq => "equal",
        CompareOp::Ne => "not-equal",
        CompareOp::Lt => "less-than",
        CompareOp::Le => "less-or-equal",
        CompareOp::Gt => "greater-than",
        CompareOp::Ge => "greater-or-equal",
        CompareOp::Like => "like",
    }
}

fn is_numeric_type(ty: VbaTypeId) -> bool {
    matches!(
        ty,
        VbaTypeId::Boolean
            | VbaTypeId::Byte
            | VbaTypeId::Integer
            | VbaTypeId::Long
            | VbaTypeId::LongLong
            | VbaTypeId::LongPtr
            | VbaTypeId::Single
            | VbaTypeId::Double
            | VbaTypeId::Currency
            | VbaTypeId::Date
    )
}

fn assignment_intent_key(intent: AssignmentIntent) -> &'static str {
    match intent {
        AssignmentIntent::Implicit => "implicit-let",
        AssignmentIntent::Let => "explicit-let",
        AssignmentIntent::Set => "set",
    }
}

fn property_base_name(procedure_name: &str) -> String {
    procedure_name
        .strip_prefix("property_get_")
        .or_else(|| procedure_name.strip_prefix("property_let_"))
        .or_else(|| procedure_name.strip_prefix("property_set_"))
        .unwrap_or(procedure_name)
        .to_ascii_lowercase()
}

fn udt_descriptor_id_for_instance(proc: &BoundProcedure, name: &str) -> Option<String> {
    for descriptor in &proc.udt_descriptors {
        if descriptor
            .variable_names
            .iter()
            .any(|variable| variable.eq_ignore_ascii_case(name))
        {
            return Some(udt_descriptor_id(&descriptor.type_name));
        }
        for instance in &descriptor.variable_names {
            for field in &descriptor.fields {
                let Some(nested_name) = field.nested_udt_name.as_deref() else {
                    continue;
                };
                let nested_alias = format!("{instance}_{}", field.name);
                if nested_alias.eq_ignore_ascii_case(name) {
                    return Some(udt_descriptor_id(nested_name));
                }
            }
        }
    }
    None
}

fn build_array_shape_descriptors(
    proc: &BoundProcedure,
    proc_slots: &HashMap<String, usize>,
) -> Vec<ArrayShapeDescriptor> {
    let mut descriptors = proc
        .array_descriptors
        .iter()
        .map(|(name, descriptor)| {
            let is_param_array = proc
                .params
                .iter()
                .any(|param| param.param_array && param.name.eq_ignore_ascii_case(name));
            ArrayShapeDescriptor {
                name: name.clone(),
                base_slot: proc_slots.get(name).copied(),
                element_type: vba_type_id_from_bound_type(descriptor.element_type),
                element_carrier: runtime_carrier_from_bound_type(descriptor.element_type),
                rank: descriptor.rank,
                bounds: descriptor
                    .bounds
                    .iter()
                    .enumerate()
                    .map(|(index, (lower_bound, upper_bound))| ArrayBoundDescriptor {
                        dimension: index + 1,
                        lower_bound: *lower_bound,
                        upper_bound: *upper_bound,
                    })
                    .collect(),
                storage: if is_param_array {
                    ArrayStorageKind::ParamArrayPack
                } else if descriptor.dynamic {
                    ArrayStorageKind::Dynamic
                } else {
                    ArrayStorageKind::StaticFixed
                },
                option_base: descriptor.option_base,
            }
        })
        .collect::<Vec<_>>();
    descriptors.sort_by(|left, right| left.name.cmp(&right.name));
    descriptors
}

fn build_udt_type_descriptors(
    proc: &BoundProcedure,
    proc_slots: &HashMap<String, usize>,
) -> Vec<UdtTypeDescriptor> {
    let mut descriptors = proc
        .udt_descriptors
        .iter()
        .map(|descriptor| {
            let fields = descriptor
                .fields
                .iter()
                .map(|field| {
                    let aliases = descriptor
                        .variable_names
                        .iter()
                        .flat_map(|instance| udt_field_aliases(instance, field, proc_slots))
                        .collect::<Vec<_>>();
                    let (declared_type, carrier) =
                        if let Some(nested_name) = field.nested_udt_name.as_deref() {
                            (
                                VbaTypeId::Unknown,
                                RuntimeCarrierKind::UdtFields {
                                    descriptor: udt_descriptor_id(nested_name),
                                },
                            )
                        } else {
                            (
                                vba_type_id_from_bound_type(field.bound_type),
                                runtime_carrier_from_bound_type(field.bound_type),
                            )
                        };
                    UdtFieldDescriptor {
                        field_id: format!(
                            "{}.{}",
                            descriptor.type_name.to_ascii_lowercase(),
                            field.name
                        ),
                        index: field.index,
                        name: field.name.clone(),
                        declared_type,
                        carrier,
                        nested_udt_name: field.nested_udt_name.clone(),
                        fixed_string_len: field.fixed_string_len,
                        array_bounds: field
                            .array_bounds
                            .as_ref()
                            .map(|bounds| {
                                bounds
                                    .iter()
                                    .enumerate()
                                    .map(|(index, (lower_bound, upper_bound))| {
                                        ArrayBoundDescriptor {
                                            dimension: index + 1,
                                            lower_bound: *lower_bound,
                                            upper_bound: *upper_bound,
                                        }
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                        aliases,
                    }
                })
                .collect::<Vec<_>>();
            UdtTypeDescriptor {
                descriptor_id: udt_descriptor_id(&descriptor.type_name),
                type_name: descriptor.type_name.clone(),
                instances: descriptor
                    .variable_names
                    .iter()
                    .map(|name| UdtInstanceDescriptor {
                        name: name.clone(),
                        base_slot: proc_slots.get(name).copied(),
                    })
                    .collect(),
                cleanup: udt_cleanup_descriptor(&fields),
                fields,
                storage: UdtStorageKind::FlattenedFieldSlots,
                copy_semantics: UdtCopySemanticsDescriptor::FieldWiseCopy,
            }
        })
        .collect::<Vec<_>>();
    descriptors.sort_by(|left, right| left.type_name.cmp(&right.type_name));
    descriptors
}

fn udt_field_aliases(
    instance_name: &str,
    field: &crate::resolve::BoundUdtFieldDescriptor,
    proc_slots: &HashMap<String, usize>,
) -> Vec<UdtFieldAliasDescriptor> {
    if let Some(bounds) = &field.array_bounds {
        let mut aliases = Vec::new();
        for (lower_bound, upper_bound) in bounds {
            for index in *lower_bound..=*upper_bound {
                let slot_name = format!("{instance_name}_{}_{index}", field.name);
                aliases.push(UdtFieldAliasDescriptor {
                    instance_name: instance_name.to_string(),
                    slot: proc_slots.get(&slot_name).copied(),
                    slot_name,
                });
            }
        }
        aliases
    } else {
        let slot_name = format!("{instance_name}_{}", field.name);
        vec![UdtFieldAliasDescriptor {
            instance_name: instance_name.to_string(),
            slot: proc_slots.get(&slot_name).copied(),
            slot_name,
        }]
    }
}

fn udt_cleanup_descriptor(fields: &[UdtFieldDescriptor]) -> UdtCleanupDescriptor {
    UdtCleanupDescriptor {
        owns_bstr: fields
            .iter()
            .any(|field| field.carrier == RuntimeCarrierKind::BStr),
        owns_object_ref: fields
            .iter()
            .any(|field| field.carrier == RuntimeCarrierKind::ObjectRef),
        owns_safearray: fields
            .iter()
            .any(|field| field.carrier == RuntimeCarrierKind::SafeArray),
        owns_variant: fields
            .iter()
            .any(|field| field.carrier == RuntimeCarrierKind::Variant),
    }
}

fn udt_descriptor_id(type_name: &str) -> String {
    format!("udt:{}", type_name.to_ascii_lowercase())
}

fn build_object_type_descriptors(
    proc: &BoundProcedure,
    proc_slots: &HashMap<String, usize>,
) -> Vec<ObjectTypeDescriptor> {
    let mut instances = proc
        .declarations
        .iter()
        .filter(|name| {
            matches!(
                proc.declaration_types.get(name.as_str()),
                Some(BoundType::Object)
            )
        })
        .map(|name| ObjectInstanceDescriptor {
            name: name.clone(),
            slot: proc_slots.get(name.as_str()).copied(),
        })
        .collect::<Vec<_>>();
    instances.sort_by(|left, right| left.name.cmp(&right.name));
    instances.dedup_by(|left, right| left.name.eq_ignore_ascii_case(&right.name));

    if instances.is_empty() {
        Vec::new()
    } else {
        vec![ObjectTypeDescriptor {
            descriptor_id: "object:object".to_string(),
            type_name: "Object".to_string(),
            kind: ObjectTypeDescriptorKind::Object,
            carrier: RuntimeCarrierKind::ObjectRef,
            initial_state: SlotInitialState::Nothing,
            activation: ObjectActivationDescriptor::None,
            event_binding: ObjectEventBindingDescriptor::None,
            default_member: ObjectDefaultMemberDescriptor::NoDefaultMember,
            support: ObjectDescriptorSupport::VmRunnable,
            instances,
        }]
    }
}

fn is_compiler_generated_array_element_slot(proc: &BoundProcedure, name: &str) -> bool {
    proc.array_descriptors
        .iter()
        .filter(|(_, descriptor)| !descriptor.dynamic)
        .any(|(array_name, _)| {
            let prefix = format!("{array_name}_");
            name.strip_prefix(&prefix)
                .is_some_and(|suffix| suffix.parse::<usize>().is_ok())
        })
}

fn patch_call_site_target_pc(
    procedure_runtime_metadata: &mut BTreeMap<String, ProcedureRuntimeMetadata>,
    call_pc: usize,
    target_pc: usize,
) {
    for metadata in procedure_runtime_metadata.values_mut() {
        for call_site in &mut metadata.call_sites {
            if call_site.call_pc == call_pc {
                call_site.target_entry_pc = Some(target_pc);
            }
        }
    }
}

fn emit_declared_string_initializers(
    proc: &BoundProcedure,
    proc_slots: &HashMap<String, usize>,
    instructions: &mut Vec<Instruction>,
) {
    let mut initialized = HashSet::new();
    for name in &proc.declarations {
        if proc
            .params
            .iter()
            .any(|param| param.name.eq_ignore_ascii_case(name))
        {
            continue;
        }
        if proc.declaration_types.get(name.as_str()) != Some(&BoundType::String) {
            continue;
        }
        let Some(slot) = proc_slots.get(name.as_str()).copied() else {
            continue;
        };
        if !initialized.insert(slot) {
            continue;
        }
        instructions.push(Instruction::LoadConstString {
            slot,
            value: String::new(),
        });
    }
}

fn build_external_call_descriptors(
    external_decls: &HashMap<String, BoundExternalDecl>,
    native_ffi_available: bool,
) -> Vec<ExternalCallDescriptor> {
    let mut decls: Vec<_> = external_decls.values().cloned().collect();
    decls.sort_by(|lhs, rhs| {
        lhs.name
            .to_ascii_lowercase()
            .cmp(&rhs.name.to_ascii_lowercase())
    });
    let mut out = Vec::with_capacity(decls.len());
    for decl in decls {
        let symbol = external_symbol_token(
            decl.library.as_str(),
            decl.alias.as_str(),
            decl.name.as_str(),
        );
        let param_count = decl.params.len();
        let param_types = decl
            .params
            .iter()
            .map(|p| bound_type_to_declare_param_type(&p.ty))
            .collect();
        let param_by_ref = decl.params.iter().map(|p| p.by_ref).collect();
        let return_type = if decl.is_function {
            Some(bound_type_to_declare_param_type(&decl.return_type))
        } else {
            None
        };
        let marshal_lane = if native_ffi_available && !decl.library.eq_ignore_ascii_case("host") {
            "m1-native-ffi".to_string()
        } else {
            "m0-deterministic".to_string()
        };
        out.push(ExternalCallDescriptor {
            descriptor_id: symbol as u32,
            declared_name: decl.name,
            library: decl.library,
            alias: decl.alias,
            ordinal_alias: decl.ordinal_alias,
            symbol: DynLinkSymbol::new(symbol),
            marshal_lane,
            calling_convention: "platform-default".to_string(),
            selection_policy: if decl.ordinal_alias {
                "ordinal-literal-canonical".to_string()
            } else {
                "case-insensitive-canonical".to_string()
            },
            param_count,
            param_types,
            param_by_ref,
            return_type,
        });
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn emit_stmt_list(
    stmts: &[BoundStmt],
    compare_mode: StringCompareMode,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    do_exit_stack: &mut Vec<Vec<usize>>,
    for_exit_stack: &mut Vec<Vec<usize>>,
    proc_exit_stack: &mut Vec<Vec<usize>>,
    call_patches: &mut Vec<(usize, String)>,
    error_handler_patches: &mut Vec<(usize, String)>,
    goto_patches: &mut Vec<(usize, String)>,
    resume_label_patches: &mut Vec<(usize, String)>,
    proc_meta: &HashMap<String, EmitProcMeta>,
    external_decls: &HashMap<String, BoundExternalDecl>,
    current_proc_name: &str,
    proc_labels: &mut HashMap<String, usize>,
    statement_entry_pcs: &mut Vec<usize>,
    call_site_descriptors: &mut Vec<CallSiteDescriptor>,
) {
    for stmt in stmts {
        emit_stmt(
            stmt,
            compare_mode,
            slot_map,
            temps,
            instructions,
            do_exit_stack,
            for_exit_stack,
            proc_exit_stack,
            call_patches,
            error_handler_patches,
            goto_patches,
            resume_label_patches,
            proc_meta,
            external_decls,
            current_proc_name,
            proc_labels,
            statement_entry_pcs,
            call_site_descriptors,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_stmt(
    stmt: &BoundStmt,
    compare_mode: StringCompareMode,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    do_exit_stack: &mut Vec<Vec<usize>>,
    for_exit_stack: &mut Vec<Vec<usize>>,
    proc_exit_stack: &mut Vec<Vec<usize>>,
    call_patches: &mut Vec<(usize, String)>,
    error_handler_patches: &mut Vec<(usize, String)>,
    goto_patches: &mut Vec<(usize, String)>,
    resume_label_patches: &mut Vec<(usize, String)>,
    proc_meta: &HashMap<String, EmitProcMeta>,
    external_decls: &HashMap<String, BoundExternalDecl>,
    current_proc_name: &str,
    proc_labels: &mut HashMap<String, usize>,
    statement_entry_pcs: &mut Vec<usize>,
    call_site_descriptors: &mut Vec<CallSiteDescriptor>,
) {
    statement_entry_pcs.push(instructions.len());
    match stmt {
        BoundStmt::Assign {
            target,
            expr,
            intent,
        } => {
            if let Some(target_slot) = slot_map.get(target.as_str()).copied() {
                let current_meta = proc_meta
                    .get(current_proc_name)
                    .expect("current procedure metadata should exist");
                let target_ty = current_meta
                    .declaration_types
                    .get(target.as_str())
                    .copied()
                    .unwrap_or(BoundType::Variant);
                let source_ty = expr_bound_type(expr, current_meta, proc_meta, external_decls);
                if let Some((intent, target_kind)) =
                    runtime_assignment_validation(*intent, target_ty, source_ty)
                {
                    let value_slot = temps.alloc_temp();
                    emit_expr_into(
                        expr,
                        compare_mode,
                        value_slot,
                        slot_map,
                        temps,
                        instructions,
                        call_patches,
                        proc_meta,
                        external_decls,
                    );
                    instructions.push(Instruction::ValidateRuntimeAssignment {
                        src: value_slot,
                        intent,
                        target_kind,
                        target_name: target.clone(),
                        target_type_name: bound_type_display_name(target_ty).to_string(),
                    });
                    instructions.push(Instruction::CopySlot {
                        dst: target_slot,
                        src: value_slot,
                    });
                } else {
                    emit_expr_into(
                        expr,
                        compare_mode,
                        target_slot,
                        slot_map,
                        temps,
                        instructions,
                        call_patches,
                        proc_meta,
                        external_decls,
                    );
                }
            }
        }
        BoundStmt::AssignRuntimeArrayElement {
            name,
            indices,
            expr,
            intent: _,
        } => {
            if let Some(array_slot) = slot_map.get(name.as_str()).copied() {
                let index_slots = indices
                    .iter()
                    .map(|index| {
                        let index_slot = temps.alloc_temp();
                        emit_expr_into(
                            index,
                            compare_mode,
                            index_slot,
                            slot_map,
                            temps,
                            instructions,
                            call_patches,
                            proc_meta,
                            external_decls,
                        );
                        index_slot
                    })
                    .collect::<Vec<_>>();
                let value_slot = temps.alloc_temp();
                emit_expr_into(
                    expr,
                    compare_mode,
                    value_slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                instructions.push(Instruction::IntrinsicArraySet {
                    array: array_slot,
                    indices: index_slots,
                    src: value_slot,
                });
            }
        }
        BoundStmt::UdtAssign {
            target,
            source,
            fields,
        } => {
            for field in fields {
                let dst_alias = format!("{target}_{field}");
                let src_alias = format!("{source}_{field}");
                if let (Some(dst), Some(src)) = (
                    slot_map.get(dst_alias.as_str()).copied(),
                    slot_map.get(src_alias.as_str()).copied(),
                ) {
                    instructions.push(Instruction::CopySlot { dst, src });
                }
            }
        }
        BoundStmt::MidAssign {
            target,
            start,
            count,
            value,
        } => {
            if let Some(target_slot) = slot_map.get(target.as_str()).copied() {
                let start_slot = temps.alloc_temp();
                emit_expr_into(
                    start,
                    compare_mode,
                    start_slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                let count_slot = if let Some(count) = count {
                    let slot = temps.alloc_temp();
                    emit_expr_into(
                        count,
                        compare_mode,
                        slot,
                        slot_map,
                        temps,
                        instructions,
                        call_patches,
                        proc_meta,
                        external_decls,
                    );
                    Some(slot)
                } else {
                    None
                };
                let value_slot = temps.alloc_temp();
                emit_expr_into(
                    value,
                    compare_mode,
                    value_slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                instructions.push(Instruction::IntrinsicMidStmtDigits {
                    target: target_slot,
                    start: start_slot,
                    count: count_slot,
                    value: value_slot,
                });
            }
        }
        BoundStmt::IfCond {
            cond,
            then_body,
            else_body,
        } => {
            let cond_slot = temps.alloc_temp();
            emit_cond_into(
                cond,
                compare_mode,
                cond_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            let jump_patch = instructions.len();
            instructions.push(Instruction::JumpIfZero {
                cond_slot,
                target_pc: 0,
            });
            emit_stmt_list(
                then_body,
                compare_mode,
                slot_map,
                temps,
                instructions,
                do_exit_stack,
                for_exit_stack,
                proc_exit_stack,
                call_patches,
                error_handler_patches,
                goto_patches,
                resume_label_patches,
                proc_meta,
                external_decls,
                current_proc_name,
                proc_labels,
                statement_entry_pcs,
                call_site_descriptors,
            );
            if else_body.is_empty() {
                let target = instructions.len();
                if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[jump_patch] {
                    *target_pc = target;
                }
            } else {
                let end_patch = instructions.len();
                instructions.push(Instruction::Jump { target_pc: 0 });
                let else_target = instructions.len();
                if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[jump_patch] {
                    *target_pc = else_target;
                }
                emit_stmt_list(
                    else_body,
                    compare_mode,
                    slot_map,
                    temps,
                    instructions,
                    do_exit_stack,
                    for_exit_stack,
                    proc_exit_stack,
                    call_patches,
                    error_handler_patches,
                    goto_patches,
                    resume_label_patches,
                    proc_meta,
                    external_decls,
                    current_proc_name,
                    proc_labels,
                    statement_entry_pcs,
                    call_site_descriptors,
                );
                let end_target = instructions.len();
                if let Instruction::Jump { target_pc } = &mut instructions[end_patch] {
                    *target_pc = end_target;
                }
            }
        }
        BoundStmt::ForRange {
            var,
            start,
            end,
            step,
            body,
        } => {
            if let Some(var_slot) = slot_map.get(var.as_str()).copied() {
                emit_expr_into(
                    start,
                    compare_mode,
                    var_slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                let end_slot = temps.alloc_temp();
                let step_slot = temps.alloc_temp();
                let zero_slot = temps.alloc_temp();
                let step_non_negative_slot = temps.alloc_temp();
                let cmp_le_slot = temps.alloc_temp();
                let cmp_ge_slot = temps.alloc_temp();
                let step_negative_slot = temps.alloc_temp();
                let upper_cond_slot = temps.alloc_temp();
                let lower_cond_slot = temps.alloc_temp();
                let cond_slot = temps.alloc_temp();
                emit_expr_into(
                    end,
                    compare_mode,
                    end_slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                emit_expr_into(
                    step,
                    compare_mode,
                    step_slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                instructions.push(Instruction::LoadConstI32 {
                    slot: zero_slot,
                    value: 0,
                });

                let loop_head = instructions.len();
                instructions.push(Instruction::CmpGeSlots {
                    dst: step_non_negative_slot,
                    lhs: step_slot,
                    rhs: zero_slot,
                    mode: compare_mode,
                });
                instructions.push(Instruction::CmpLeSlots {
                    dst: cmp_le_slot,
                    lhs: var_slot,
                    rhs: end_slot,
                    mode: compare_mode,
                });
                instructions.push(Instruction::CmpGeSlots {
                    dst: cmp_ge_slot,
                    lhs: var_slot,
                    rhs: end_slot,
                    mode: compare_mode,
                });
                instructions.push(Instruction::BoolNot {
                    dst: step_negative_slot,
                    src: step_non_negative_slot,
                });
                instructions.push(Instruction::BoolAnd {
                    dst: upper_cond_slot,
                    lhs: step_non_negative_slot,
                    rhs: cmp_le_slot,
                });
                instructions.push(Instruction::BoolAnd {
                    dst: lower_cond_slot,
                    lhs: step_negative_slot,
                    rhs: cmp_ge_slot,
                });
                instructions.push(Instruction::BoolOr {
                    dst: cond_slot,
                    lhs: upper_cond_slot,
                    rhs: lower_cond_slot,
                });
                let exit_patch = instructions.len();
                instructions.push(Instruction::JumpIfZero {
                    cond_slot,
                    target_pc: 0,
                });
                for_exit_stack.push(Vec::new());
                emit_stmt_list(
                    body,
                    compare_mode,
                    slot_map,
                    temps,
                    instructions,
                    do_exit_stack,
                    for_exit_stack,
                    proc_exit_stack,
                    call_patches,
                    error_handler_patches,
                    goto_patches,
                    resume_label_patches,
                    proc_meta,
                    external_decls,
                    current_proc_name,
                    proc_labels,
                    statement_entry_pcs,
                    call_site_descriptors,
                );
                instructions.push(Instruction::AddSlots {
                    dst: var_slot,
                    lhs: var_slot,
                    rhs: step_slot,
                });
                instructions.push(Instruction::Jump {
                    target_pc: loop_head,
                });
                let exit_target = instructions.len();
                if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[exit_patch] {
                    *target_pc = exit_target;
                }
                if let Some(exit_patches) = for_exit_stack.pop() {
                    for patch in exit_patches {
                        if let Instruction::Jump { target_pc } = &mut instructions[patch] {
                            *target_pc = exit_target;
                        }
                    }
                }
            }
        }
        BoundStmt::ForEach {
            var,
            items,
            iterable,
            body,
        } => {
            if let Some(var_slot) = slot_map.get(var.as_str()).copied() {
                for_exit_stack.push(Vec::new());
                if let Some(iterable) = iterable {
                    let iterable_slot = temps.alloc_temp();
                    let iter_slot = temps.alloc_temp();
                    let has_value_slot = temps.alloc_temp();
                    emit_expr_into(
                        iterable,
                        compare_mode,
                        iterable_slot,
                        slot_map,
                        temps,
                        instructions,
                        call_patches,
                        proc_meta,
                        external_decls,
                    );
                    instructions.push(Instruction::IntrinsicForEachInit {
                        iter: iter_slot,
                        src: iterable_slot,
                    });
                    let loop_start = instructions.len();
                    instructions.push(Instruction::IntrinsicForEachNext {
                        iter: iter_slot,
                        item: var_slot,
                        has_value: has_value_slot,
                    });
                    let exit_patch = instructions.len();
                    instructions.push(Instruction::JumpIfZero {
                        cond_slot: has_value_slot,
                        target_pc: usize::MAX,
                    });
                    emit_stmt_list(
                        body,
                        compare_mode,
                        slot_map,
                        temps,
                        instructions,
                        do_exit_stack,
                        for_exit_stack,
                        proc_exit_stack,
                        call_patches,
                        error_handler_patches,
                        goto_patches,
                        resume_label_patches,
                        proc_meta,
                        external_decls,
                        current_proc_name,
                        proc_labels,
                        statement_entry_pcs,
                        call_site_descriptors,
                    );
                    instructions.push(Instruction::Jump {
                        target_pc: loop_start,
                    });
                    let exit_target = instructions.len();
                    if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[exit_patch]
                    {
                        *target_pc = exit_target;
                    }
                } else {
                    for item in items {
                        emit_expr_into(
                            item,
                            compare_mode,
                            var_slot,
                            slot_map,
                            temps,
                            instructions,
                            call_patches,
                            proc_meta,
                            external_decls,
                        );
                        emit_stmt_list(
                            body,
                            compare_mode,
                            slot_map,
                            temps,
                            instructions,
                            do_exit_stack,
                            for_exit_stack,
                            proc_exit_stack,
                            call_patches,
                            error_handler_patches,
                            goto_patches,
                            resume_label_patches,
                            proc_meta,
                            external_decls,
                            current_proc_name,
                            proc_labels,
                            statement_entry_pcs,
                            call_site_descriptors,
                        );
                    }
                }
                let exit_target = instructions.len();
                if let Some(exit_patches) = for_exit_stack.pop() {
                    for patch in exit_patches {
                        if let Instruction::Jump { target_pc } = &mut instructions[patch] {
                            *target_pc = exit_target;
                        }
                    }
                }
            }
        }
        BoundStmt::ReDim {
            name,
            bounds,
            previous_bounds,
            preserve,
        } => {
            if !preserve {
                reset_array_slots(name, slot_map, instructions);
            } else if let Some(prev) = previous_bounds {
                if let (Some(old_count), Some(new_count)) =
                    (array_element_count(prev), array_element_count(bounds))
                {
                    let overlap = old_count.min(new_count);
                    let tail = old_count.max(new_count);
                    if overlap < tail {
                        reset_array_slots_range(name, overlap, tail, slot_map, instructions);
                    }
                }
            } else {
                reset_array_slots(name, slot_map, instructions);
            }
        }
        BoundStmt::ReDimRuntime {
            name,
            bounds,
            preserve,
        } => {
            let dst = slot_map
                .get(name.as_str())
                .copied()
                .expect("runtime ReDim requires a base array slot");
            let mut upper_slots = Vec::with_capacity(bounds.len());
            let mut lower_bounds = Vec::with_capacity(bounds.len());
            for bound in bounds {
                let upper_slot = temps.alloc_temp();
                emit_expr_into(
                    &bound.upper_bound,
                    compare_mode,
                    upper_slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                upper_slots.push(upper_slot);
                lower_bounds.push(bound.lower_bound);
            }
            let element_type = runtime_array_element_type_for(name, proc_meta)
                .expect("runtime ReDim element type should be validated during typecheck");
            if *preserve {
                instructions.push(Instruction::IntrinsicArrayResizePreserve {
                    dst,
                    upper_bounds: upper_slots,
                    lower_bounds,
                    element_type,
                });
            } else {
                instructions.push(Instruction::IntrinsicArrayResize {
                    dst,
                    upper_bounds: upper_slots,
                    lower_bounds,
                    element_type,
                });
            }
        }
        BoundStmt::Erase { name } => {
            reset_array_slots(name, slot_map, instructions);
        }
        BoundStmt::DoWhile {
            cond,
            body,
            post_check,
        } => {
            let loop_head = instructions.len();
            let cond_slot = temps.alloc_temp();
            let mut entry_exit_patch: Option<usize> = None;

            if !post_check {
                emit_cond_into(
                    cond,
                    compare_mode,
                    cond_slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                let exit_patch = instructions.len();
                instructions.push(Instruction::JumpIfZero {
                    cond_slot,
                    target_pc: 0,
                });
                entry_exit_patch = Some(exit_patch);
            }

            do_exit_stack.push(Vec::new());
            emit_stmt_list(
                body,
                compare_mode,
                slot_map,
                temps,
                instructions,
                do_exit_stack,
                for_exit_stack,
                proc_exit_stack,
                call_patches,
                error_handler_patches,
                goto_patches,
                resume_label_patches,
                proc_meta,
                external_decls,
                current_proc_name,
                proc_labels,
                statement_entry_pcs,
                call_site_descriptors,
            );

            emit_cond_into(
                cond,
                compare_mode,
                cond_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            let post_exit_patch = instructions.len();
            instructions.push(Instruction::JumpIfZero {
                cond_slot,
                target_pc: 0,
            });
            instructions.push(Instruction::Jump {
                target_pc: loop_head,
            });

            let exit_target = instructions.len();
            if let Some(entry_patch) = entry_exit_patch
                && let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[entry_patch]
            {
                *target_pc = exit_target;
            }
            if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[post_exit_patch] {
                *target_pc = exit_target;
            }

            if let Some(exit_patches) = do_exit_stack.pop() {
                for patch in exit_patches {
                    if let Instruction::Jump { target_pc } = &mut instructions[patch] {
                        *target_pc = exit_target;
                    }
                }
            }
        }
        BoundStmt::ExitDo => {
            if let Some(exit_patches) = do_exit_stack.last_mut() {
                let patch = instructions.len();
                instructions.push(Instruction::Jump { target_pc: 0 });
                exit_patches.push(patch);
            }
        }
        BoundStmt::ExitFor => {
            if let Some(exit_patches) = for_exit_stack.last_mut() {
                let patch = instructions.len();
                instructions.push(Instruction::Jump { target_pc: 0 });
                exit_patches.push(patch);
            }
        }
        BoundStmt::ExitProcedure => {
            if let Some(exit_patches) = proc_exit_stack.last_mut() {
                let patch = instructions.len();
                instructions.push(Instruction::Jump { target_pc: 0 });
                exit_patches.push(patch);
            }
        }
        BoundStmt::OnErrorResumeNext => {
            instructions.push(Instruction::SetOnErrorResumeNext);
        }
        BoundStmt::OnErrorGoto0 => {
            instructions.push(Instruction::SetOnErrorGoto0);
        }
        BoundStmt::OnErrorGotoLabel { label } => {
            let patch_idx = instructions.len();
            instructions.push(Instruction::SetOnErrorGotoLabel { target_pc: 0 });
            error_handler_patches
                .push((patch_idx, format!("__label::{current_proc_name}::{label}")));
        }
        BoundStmt::ResumeNext => {
            instructions.push(Instruction::ResumeNext);
        }
        BoundStmt::Resume => {
            instructions.push(Instruction::Resume);
        }
        BoundStmt::ResumeLabel { label } => {
            let patch_idx = instructions.len();
            instructions.push(Instruction::ResumeLabel { target_pc: 0 });
            resume_label_patches
                .push((patch_idx, format!("__label::{current_proc_name}::{label}")));
        }
        BoundStmt::RaiseError(code) => {
            instructions.push(Instruction::RaiseError { code: *code });
        }
        BoundStmt::RaiseEvent { args, .. } => {
            for arg in args {
                let temp = temps.alloc_temp();
                emit_expr_into(
                    &arg.expr,
                    compare_mode,
                    temp,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
            }
        }
        BoundStmt::ErrClear => {
            instructions.push(Instruction::ClearErr);
        }
        BoundStmt::Label { name } => {
            insert_casefold_key(
                proc_labels,
                &format!("__label::{current_proc_name}::{name}"),
                instructions.len(),
            );
        }
        BoundStmt::GoTo { label } => {
            let patch_idx = instructions.len();
            instructions.push(Instruction::Jump { target_pc: 0 });
            goto_patches.push((patch_idx, format!("__label::{current_proc_name}::{label}")));
        }
        BoundStmt::GoSub { label } => {
            let patch_idx = instructions.len();
            instructions.push(Instruction::CallProc {
                target_pc: 0,
                project_member: None,
            });
            call_patches.push((patch_idx, format!("__label::{current_proc_name}::{label}")));
        }
        BoundStmt::Return => {
            instructions.push(Instruction::Return);
        }
        BoundStmt::SelectCase {
            expr,
            arms,
            else_body,
        } => {
            let expr_slot = temps.alloc_temp();
            emit_expr_into(
                expr,
                compare_mode,
                expr_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            let mut end_patches: Vec<usize> = Vec::new();

            for (clauses, body) in arms {
                let aggregate_slot = temps.alloc_temp();
                instructions.push(Instruction::LoadConstI32 {
                    slot: aggregate_slot,
                    value: 0,
                });

                for clause in clauses {
                    let cmp_slot = emit_select_case_clause_match(
                        expr_slot,
                        clause,
                        temps,
                        instructions,
                        call_patches,
                        proc_meta,
                        external_decls,
                    );
                    instructions.push(Instruction::BoolOr {
                        dst: aggregate_slot,
                        lhs: aggregate_slot,
                        rhs: cmp_slot,
                    });
                }

                let next_patch = instructions.len();
                instructions.push(Instruction::JumpIfZero {
                    cond_slot: aggregate_slot,
                    target_pc: 0,
                });
                emit_stmt_list(
                    body,
                    compare_mode,
                    slot_map,
                    temps,
                    instructions,
                    do_exit_stack,
                    for_exit_stack,
                    proc_exit_stack,
                    call_patches,
                    error_handler_patches,
                    goto_patches,
                    resume_label_patches,
                    proc_meta,
                    external_decls,
                    current_proc_name,
                    proc_labels,
                    statement_entry_pcs,
                    call_site_descriptors,
                );
                let end_patch = instructions.len();
                instructions.push(Instruction::Jump { target_pc: 0 });
                end_patches.push(end_patch);
                let next_target = instructions.len();
                if let Instruction::JumpIfZero { target_pc, .. } = &mut instructions[next_patch] {
                    *target_pc = next_target;
                }
            }

            emit_stmt_list(
                else_body,
                compare_mode,
                slot_map,
                temps,
                instructions,
                do_exit_stack,
                for_exit_stack,
                proc_exit_stack,
                call_patches,
                error_handler_patches,
                goto_patches,
                resume_label_patches,
                proc_meta,
                external_decls,
                current_proc_name,
                proc_labels,
                statement_entry_pcs,
                call_site_descriptors,
            );
            let end_target = instructions.len();
            for patch in end_patches {
                if let Instruction::Jump { target_pc } = &mut instructions[patch] {
                    *target_pc = end_target;
                }
            }
        }
        BoundStmt::Call { name, args, syntax } => {
            let invocation_syntax = call_invocation_syntax_descriptor(*syntax);
            if name.eq_ignore_ascii_case("randomize") {
                let dst = temps.alloc_temp();
                let seed = if args.is_empty() {
                    None
                } else {
                    let seed_slot = temps.alloc_temp();
                    emit_expr_into(
                        &args[0].expr,
                        compare_mode,
                        seed_slot,
                        slot_map,
                        temps,
                        instructions,
                        call_patches,
                        proc_meta,
                        external_decls,
                    );
                    Some(seed_slot)
                };
                instructions.push(Instruction::IntrinsicRandomizeDigits { dst, seed });
            } else if !emit_early_call(
                name,
                args,
                invocation_syntax,
                compare_mode,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
                None,
                Some(&mut *call_site_descriptors),
                Some(current_proc_name),
            ) {
                let _ = emit_late_bound_default_member_call(
                    name,
                    args,
                    invocation_syntax,
                    compare_mode,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                    None,
                    Some(&mut *call_site_descriptors),
                    Some(current_proc_name),
                );
            }
        }
        BoundStmt::AssignFromCall {
            target,
            name,
            args,
            intent,
            syntax,
        } => {
            let invocation_syntax = call_invocation_syntax_descriptor(*syntax);
            if let Some(target_slot) = slot_map.get(target.as_str()).copied() {
                let current_meta = proc_meta
                    .get(current_proc_name)
                    .expect("current procedure metadata should exist");
                let target_ty = current_meta
                    .declaration_types
                    .get(target.as_str())
                    .copied()
                    .unwrap_or(BoundType::Variant);
                let source_ty = call_bound_type(name, proc_meta, external_decls);
                if let Some((runtime_intent, target_kind)) =
                    runtime_assignment_validation(*intent, target_ty, source_ty)
                {
                    let value_slot = temps.alloc_temp();
                    if !emit_early_call(
                        name,
                        args,
                        invocation_syntax,
                        compare_mode,
                        slot_map,
                        temps,
                        instructions,
                        call_patches,
                        proc_meta,
                        external_decls,
                        Some(value_slot),
                        Some(&mut *call_site_descriptors),
                        Some(current_proc_name),
                    ) {
                        let _ = emit_late_bound_default_member_call(
                            name,
                            args,
                            invocation_syntax,
                            compare_mode,
                            slot_map,
                            temps,
                            instructions,
                            call_patches,
                            proc_meta,
                            external_decls,
                            Some(value_slot),
                            Some(&mut *call_site_descriptors),
                            Some(current_proc_name),
                        );
                    }
                    instructions.push(Instruction::ValidateRuntimeAssignment {
                        src: value_slot,
                        intent: runtime_intent,
                        target_kind,
                        target_name: target.clone(),
                        target_type_name: bound_type_display_name(target_ty).to_string(),
                    });
                    instructions.push(Instruction::CopySlot {
                        dst: target_slot,
                        src: value_slot,
                    });
                } else if !emit_early_call(
                    name,
                    args,
                    invocation_syntax,
                    compare_mode,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                    Some(target_slot),
                    Some(&mut *call_site_descriptors),
                    Some(current_proc_name),
                ) {
                    let _ = emit_late_bound_default_member_call(
                        name,
                        args,
                        invocation_syntax,
                        compare_mode,
                        slot_map,
                        temps,
                        instructions,
                        call_patches,
                        proc_meta,
                        external_decls,
                        Some(target_slot),
                        Some(&mut *call_site_descriptors),
                        Some(current_proc_name),
                    );
                }
            }
        }
        BoundStmt::FileOpen {
            path,
            mode,
            file_number,
        } => {
            let dst = temps.alloc_temp();
            let path_slot = temps.alloc_temp();
            let mode_slot = temps.alloc_temp();
            let filenum_slot = temps.alloc_temp();
            emit_expr_into(
                path,
                compare_mode,
                path_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            instructions.push(Instruction::LoadConstI32 {
                slot: mode_slot,
                value: *mode,
            });
            emit_expr_into(
                file_number,
                compare_mode,
                filenum_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            instructions.push(Instruction::IntrinsicFileOpenHost {
                dst,
                path: path_slot,
                mode: mode_slot,
                file_number: filenum_slot,
            });
        }
        BoundStmt::FileClose { file_number } => {
            let dst = temps.alloc_temp();
            if let Some(fnum_expr) = file_number {
                let handle_slot = temps.alloc_temp();
                emit_expr_into(
                    fnum_expr,
                    compare_mode,
                    handle_slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                instructions.push(Instruction::IntrinsicFileCloseHost {
                    dst,
                    handle: handle_slot,
                });
            } else {
                // Close all: pass handle = 0
                let handle_slot = temps.alloc_temp();
                instructions.push(Instruction::LoadConstI32 {
                    slot: handle_slot,
                    value: 0,
                });
                instructions.push(Instruction::IntrinsicFileCloseHost {
                    dst,
                    handle: handle_slot,
                });
            }
        }
        BoundStmt::FileKill { path } => {
            let dst = temps.alloc_temp();
            let path_slot = temps.alloc_temp();
            emit_expr_into(
                path,
                compare_mode,
                path_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            instructions.push(Instruction::IntrinsicFileKillHost {
                dst,
                path: path_slot,
            });
        }
        BoundStmt::FilePrint { file_number, data } => {
            let dst = temps.alloc_temp();
            let handle_slot = temps.alloc_temp();
            let data_slot = temps.alloc_temp();
            emit_expr_into(
                file_number,
                compare_mode,
                handle_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            emit_expr_into(
                data,
                compare_mode,
                data_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            instructions.push(Instruction::IntrinsicFilePrintHost {
                dst,
                handle: handle_slot,
                data: data_slot,
            });
        }
        BoundStmt::ConsolePrint { data } => {
            let dst = temps.alloc_temp();
            let data_slot = temps.alloc_temp();
            emit_expr_into(
                data,
                compare_mode,
                data_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            instructions.push(Instruction::IntrinsicConsolePrintHost {
                dst,
                data: data_slot,
            });
        }
        BoundStmt::FileWrite { file_number, data } => {
            let dst = temps.alloc_temp();
            let handle_slot = temps.alloc_temp();
            emit_expr_into(
                file_number,
                compare_mode,
                handle_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            for item in data {
                let data_slot = temps.alloc_temp();
                emit_expr_into(
                    item,
                    compare_mode,
                    data_slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                instructions.push(Instruction::IntrinsicFileWriteHost {
                    dst,
                    handle: handle_slot,
                    data: data_slot,
                });
            }
        }
        BoundStmt::FileInput {
            file_number,
            targets,
        } => {
            let handle_slot = temps.alloc_temp();
            let count_slot = temps.alloc_temp();
            emit_expr_into(
                file_number,
                compare_mode,
                handle_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            instructions.push(Instruction::LoadConstI32 {
                slot: count_slot,
                value: 1,
            });
            for target in targets {
                if let Some(&target_slot) = slot_map.get(target.as_str()) {
                    instructions.push(Instruction::IntrinsicFileInputHost {
                        dst: target_slot,
                        handle: handle_slot,
                        count: count_slot,
                    });
                }
            }
        }
        BoundStmt::ConsoleInput { targets } => {
            let count_slot = temps.alloc_temp();
            instructions.push(Instruction::LoadConstI32 {
                slot: count_slot,
                value: 1,
            });
            for target in targets {
                if let Some(&target_slot) = slot_map.get(target.as_str()) {
                    instructions.push(Instruction::IntrinsicConsoleInputHost {
                        dst: target_slot,
                        count: count_slot,
                    });
                }
            }
        }
        BoundStmt::FileLineInput {
            file_number,
            target,
        } => {
            let handle_slot = temps.alloc_temp();
            emit_expr_into(
                file_number,
                compare_mode,
                handle_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            if let Some(&target_slot) = slot_map.get(target.as_str()) {
                instructions.push(Instruction::IntrinsicFileLineInputHost {
                    dst: target_slot,
                    handle: handle_slot,
                });
            }
        }
        BoundStmt::ConsoleLineInput { target } => {
            if let Some(&target_slot) = slot_map.get(target.as_str()) {
                instructions.push(Instruction::IntrinsicConsoleLineInputHost { dst: target_slot });
            }
        }
        BoundStmt::Beep => {
            let dst = temps.alloc_temp();
            instructions.push(Instruction::IntrinsicBeepHost { dst });
        }
        BoundStmt::DebugPrint { data } => {
            let dst = temps.alloc_temp();
            let data_slot = temps.alloc_temp();
            emit_expr_into(
                data,
                compare_mode,
                data_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            instructions.push(Instruction::IntrinsicDebugPrintHost {
                dst,
                data: data_slot,
            });
        }
        BoundStmt::Unsupported { .. } => {}
    }
}

fn runtime_assignment_validation(
    intent: AssignmentIntent,
    target_ty: BoundType,
    source_ty: BoundType,
) -> Option<(RuntimeAssignmentIntent, RuntimeAssignmentTargetKind)> {
    if source_ty != BoundType::Variant {
        return None;
    }

    let runtime_intent = match intent {
        AssignmentIntent::Implicit => RuntimeAssignmentIntent::Implicit,
        AssignmentIntent::Let => RuntimeAssignmentIntent::Let,
        AssignmentIntent::Set => RuntimeAssignmentIntent::Set,
    };
    let target_kind = match target_ty {
        BoundType::Variant => RuntimeAssignmentTargetKind::Variant,
        BoundType::Object => RuntimeAssignmentTargetKind::Object,
        _ => RuntimeAssignmentTargetKind::Scalar,
    };

    match (runtime_intent, target_kind) {
        (RuntimeAssignmentIntent::Set, RuntimeAssignmentTargetKind::Variant)
        | (RuntimeAssignmentIntent::Set, RuntimeAssignmentTargetKind::Object)
        | (RuntimeAssignmentIntent::Implicit, RuntimeAssignmentTargetKind::Object)
        | (RuntimeAssignmentIntent::Let, RuntimeAssignmentTargetKind::Object)
        | (RuntimeAssignmentIntent::Implicit, RuntimeAssignmentTargetKind::Scalar)
        | (RuntimeAssignmentIntent::Let, RuntimeAssignmentTargetKind::Scalar) => {
            Some((runtime_intent, target_kind))
        }
        _ => None,
    }
}

fn expr_bound_type(
    expr: &BoundExpr,
    current_meta: &EmitProcMeta,
    proc_meta: &HashMap<String, EmitProcMeta>,
    external_decls: &HashMap<String, BoundExternalDecl>,
) -> BoundType {
    match expr {
        BoundExpr::IntConst(_) | BoundExpr::AddConst { .. } | BoundExpr::SubConst { .. } => {
            BoundType::Long
        }
        BoundExpr::BoolConst(_) => BoundType::Boolean,
        BoundExpr::FloatConst(_) => BoundType::Double,
        BoundExpr::StringConst(_) => BoundType::String,
        BoundExpr::CompareOp { .. } => BoundType::Boolean,
        BoundExpr::BinaryOp { .. } | BoundExpr::UnaryOp { .. } => BoundType::Variant,
        BoundExpr::Var(name) => current_meta
            .declaration_types
            .get(name.as_str())
            .copied()
            .unwrap_or(BoundType::Variant),
        BoundExpr::VarPtrArrayBuffer(_) => BoundType::LongPtr,
        BoundExpr::IntrinsicCall { name, .. } | BoundExpr::ProcCall { name, .. } => {
            call_bound_type(name, proc_meta, external_decls)
        }
    }
}

fn runtime_array_element_type_for(
    name: &str,
    proc_meta: &HashMap<String, EmitProcMeta>,
) -> Option<RuntimeArrayElementType> {
    let current = current_proc_meta(proc_meta)?;
    let alias = format!("{name}_0");
    let bound_type = current
        .declaration_types
        .get(alias.as_str())
        .copied()
        .unwrap_or(BoundType::Variant);
    runtime_array_element_type(bound_type)
}

fn runtime_array_element_type(bound_type: BoundType) -> Option<RuntimeArrayElementType> {
    match bound_type {
        BoundType::Variant => Some(RuntimeArrayElementType::Variant),
        BoundType::Integer => Some(RuntimeArrayElementType::Integer),
        BoundType::Long => Some(RuntimeArrayElementType::Long),
        BoundType::LongLong => Some(RuntimeArrayElementType::LongLong),
        BoundType::LongPtr => Some(RuntimeArrayElementType::LongPtr),
        BoundType::Byte => Some(RuntimeArrayElementType::Byte),
        BoundType::Single => Some(RuntimeArrayElementType::Single),
        BoundType::Double => Some(RuntimeArrayElementType::Double),
        BoundType::Currency => Some(RuntimeArrayElementType::Currency),
        BoundType::Date => Some(RuntimeArrayElementType::Date),
        BoundType::String => Some(RuntimeArrayElementType::String),
        BoundType::Boolean => Some(RuntimeArrayElementType::Boolean),
        BoundType::Object | BoundType::Array | BoundType::Decimal => None,
    }
}

fn call_bound_type(
    name: &str,
    proc_meta: &HashMap<String, EmitProcMeta>,
    external_decls: &HashMap<String, BoundExternalDecl>,
) -> BoundType {
    if name.eq_ignore_ascii_case("createobject") {
        return BoundType::Object;
    }
    if name.eq_ignore_ascii_case("dispatchinvoke")
        || name.eq_ignore_ascii_case("__OxVbaEarlyInvoke")
        || external_decls.contains_key(&name.to_ascii_lowercase())
    {
        return BoundType::Variant;
    }
    proc_meta
        .get(name)
        .map(|meta| meta.return_type)
        .unwrap_or(BoundType::Variant)
}

fn bound_type_display_name(ty: BoundType) -> &'static str {
    match ty {
        BoundType::Variant => "Variant",
        BoundType::Integer => "Integer",
        BoundType::Long => "Long",
        BoundType::LongLong => "LongLong",
        BoundType::LongPtr => "LongPtr",
        BoundType::Byte => "Byte",
        BoundType::Single => "Single",
        BoundType::Double => "Double",
        BoundType::Currency => "Currency",
        BoundType::Decimal => "Decimal",
        BoundType::Date => "Date",
        BoundType::String => "String",
        BoundType::Boolean => "Boolean",
        BoundType::Object => "Object",
        BoundType::Array => "Array",
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_early_call(
    name: &str,
    args: &[BoundCallArg],
    invocation_syntax: CallInvocationSyntaxDescriptor,
    compare_mode: StringCompareMode,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    call_patches: &mut Vec<(usize, String)>,
    proc_meta: &HashMap<String, EmitProcMeta>,
    external_decls: &HashMap<String, BoundExternalDecl>,
    assign_target: Option<usize>,
    call_site_descriptors: Option<&mut Vec<CallSiteDescriptor>>,
    current_proc_name: Option<&str>,
) -> bool {
    if name.eq_ignore_ascii_case("dispatchinvoke")
        || name.eq_ignore_ascii_case("__OxVbaEarlyInvoke")
    {
        return emit_dispatch_invoke_call(
            args,
            compare_mode,
            slot_map,
            temps,
            instructions,
            call_patches,
            proc_meta,
            external_decls,
            assign_target,
            name.eq_ignore_ascii_case("__OxVbaEarlyInvoke"),
        );
    }

    if let Some(external_decl) = external_decls.get(&name.to_ascii_lowercase()) {
        return emit_external_declare_call(
            name,
            external_decl,
            args,
            compare_mode,
            slot_map,
            temps,
            instructions,
            call_patches,
            proc_meta,
            external_decls,
            assign_target,
        );
    }

    let Some(meta) = proc_meta.get(name) else {
        return false;
    };

    let mut byref_copyback: Vec<(usize, usize)> = Vec::new();
    let mut param_array_element_slots: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut fixed_array_materializations: HashMap<usize, Vec<usize>> = HashMap::new();
    let arg_mapping = map_call_args_for_emit(args, &meta.params);
    for (idx, param) in meta.params.iter().enumerate() {
        let Some(param_slot) = meta.slots.get(param.name.as_str()).copied() else {
            continue;
        };
        if param.param_array {
            let mut values = Vec::new();
            for mapped_arg in &arg_mapping.extras {
                let value_slot = temps.alloc_temp();
                emit_expr_into(
                    &mapped_arg.arg.expr,
                    compare_mode,
                    value_slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                values.push(value_slot);
            }
            param_array_element_slots.insert(idx, values.clone());
            instructions.push(Instruction::IntrinsicArrayLiteral {
                dst: param_slot,
                values,
            });
            continue;
        }
        let Some(mapped_arg) = arg_mapping.fixed.get(idx).and_then(|arg| *arg) else {
            if param.optional {
                emit_optional_default(param, param_slot, instructions);
            }
            continue;
        };
        let arg = mapped_arg.arg;

        if param.ty == BoundType::Array
            && let BoundExpr::Var(var_name) = &arg.expr
        {
            let element_slots = collect_array_element_slots(var_name, slot_map);
            if !element_slots.is_empty() {
                // Fixed-array callers still lower through alias element slots; materialize
                // the current payload into the callee's regular-array slot.
                fixed_array_materializations.insert(idx, element_slots.clone());
                instructions.push(Instruction::IntrinsicArrayLiteral {
                    dst: param_slot,
                    values: element_slots,
                });
                continue;
            }
        }

        if param.by_ref
            && !arg.force_byval
            && let BoundExpr::Var(var_name) = &arg.expr
            && let Some(src_slot) = slot_map.get(var_name.as_str()).copied()
        {
            if src_slot != param_slot {
                instructions.push(Instruction::CopySlot {
                    dst: param_slot,
                    src: src_slot,
                });
            }
            byref_copyback.push((src_slot, param_slot));
            continue;
        }

        emit_expr_into(
            &arg.expr,
            compare_mode,
            param_slot,
            slot_map,
            temps,
            instructions,
            call_patches,
            proc_meta,
            external_decls,
        );
    }

    let patch_idx = instructions.len();
    instructions.push(Instruction::CallProc {
        target_pc: 0,
        project_member: None,
    });
    call_patches.push((patch_idx, name.to_string()));
    if let (Some(call_site_descriptors), Some(current_proc_name)) =
        (call_site_descriptors, current_proc_name)
    {
        call_site_descriptors.push(build_early_call_site_descriptor(
            current_proc_name,
            patch_idx,
            name,
            meta,
            invocation_syntax,
            args.len(),
            &arg_mapping,
            &byref_copyback,
            &param_array_element_slots,
            &fixed_array_materializations,
            assign_target,
            slot_map,
        ));
    }

    for (dst_slot, src_slot) in byref_copyback {
        if dst_slot != src_slot {
            instructions.push(Instruction::CopySlot {
                dst: dst_slot,
                src: src_slot,
            });
        }
    }

    if let Some(dst) = assign_target
        && let Some(src) = meta.return_slot
        && dst != src
    {
        instructions.push(Instruction::CopySlot { dst, src });
    }

    true
}

#[allow(clippy::too_many_arguments)]
fn build_early_call_site_descriptor(
    current_proc_name: &str,
    call_pc: usize,
    target_name: &str,
    meta: &EmitProcMeta,
    invocation_syntax: CallInvocationSyntaxDescriptor,
    source_argument_count: usize,
    arg_mapping: &EmitCallArgMapping<'_>,
    byref_copyback: &[(usize, usize)],
    param_array_element_slots: &HashMap<usize, Vec<usize>>,
    fixed_array_materializations: &HashMap<usize, Vec<usize>>,
    assign_target: Option<usize>,
    slot_map: &HashMap<String, usize>,
) -> CallSiteDescriptor {
    let mut arguments = Vec::new();
    let target_kind = call_target_kind_for_procedure(target_name, meta.return_slot);
    for (idx, param) in meta.params.iter().enumerate() {
        let parameter_slot = meta.slots.get(param.name.as_str()).copied();
        let property_value_byval = matches!(
            target_kind,
            CallTargetKindDescriptor::PropertyLet | CallTargetKindDescriptor::PropertySet
        ) && idx + 1 == meta.params.len();
        if param.param_array {
            let element_slots = param_array_element_slots
                .get(&idx)
                .cloned()
                .unwrap_or_default();
            arguments.push(ArgumentBindingDescriptor {
                argument_index: idx,
                source_index: arg_mapping.extras.first().map(|arg| arg.source_index),
                source_name: None,
                parameter_index: Some(idx),
                parameter_name: Some(param.name.clone()),
                parameter_slot,
                source_kind: ArgumentSourceKindDescriptor::ParamArrayPack,
                expression_kind: ArgumentExpressionKindDescriptor::Expression,
                binding_kind: ArgumentBindingKindDescriptor::ParamArrayPack,
                force_byval: false,
                source_slot: None,
                writeback: None,
                optional_default: None,
                param_array: Some(ParamArrayBindingDescriptor {
                    element_count: element_slots.len(),
                    element_slots,
                    lower_bound: 0,
                    empty_upper_bound: -1,
                }),
            });
            continue;
        }

        let Some(mapped_arg) = arg_mapping.fixed.get(idx).and_then(|arg| *arg) else {
            arguments.push(ArgumentBindingDescriptor {
                argument_index: idx,
                source_index: None,
                source_name: None,
                parameter_index: Some(idx),
                parameter_name: Some(param.name.clone()),
                parameter_slot,
                source_kind: ArgumentSourceKindDescriptor::Omitted,
                expression_kind: ArgumentExpressionKindDescriptor::Unknown,
                binding_kind: ArgumentBindingKindDescriptor::OptionalDefault,
                force_byval: false,
                source_slot: None,
                writeback: None,
                optional_default: Some(optional_default_value_for_param(param)),
                param_array: None,
            });
            continue;
        };

        let byref_writeback = parameter_slot.and_then(|param_slot| {
            byref_copyback
                .iter()
                .find(|(_, copyback_param_slot)| *copyback_param_slot == param_slot)
                .map(
                    |(caller_slot, copyback_param_slot)| ArgumentWritebackDescriptor {
                        caller_slot: Some(*caller_slot),
                        parameter_slot: Some(*copyback_param_slot),
                        required: true,
                    },
                )
        });
        let binding_kind = if fixed_array_materializations.contains_key(&idx) {
            ArgumentBindingKindDescriptor::FixedArrayMaterialized
        } else if byref_writeback.is_some() {
            ArgumentBindingKindDescriptor::ByRefAlias
        } else if property_value_byval {
            ArgumentBindingKindDescriptor::ByValCopy
        } else if param.by_ref {
            ArgumentBindingKindDescriptor::ByRefExpressionTemp
        } else {
            ArgumentBindingKindDescriptor::ByValCopy
        };
        let source_slot = byref_writeback
            .as_ref()
            .and_then(|writeback| writeback.caller_slot)
            .or_else(|| match &mapped_arg.arg.expr {
                BoundExpr::Var(var_name) => slot_map.get(var_name.as_str()).copied(),
                _ => None,
            });
        arguments.push(ArgumentBindingDescriptor {
            argument_index: idx,
            source_index: Some(mapped_arg.source_index),
            source_name: mapped_arg.arg.name.clone(),
            parameter_index: Some(idx),
            parameter_name: Some(param.name.clone()),
            parameter_slot,
            source_kind: argument_source_kind(mapped_arg.arg),
            expression_kind: argument_expression_kind(&mapped_arg.arg.expr),
            binding_kind,
            force_byval: mapped_arg.arg.force_byval,
            source_slot,
            writeback: byref_writeback,
            optional_default: None,
            param_array: fixed_array_materializations.get(&idx).map(|element_slots| {
                ParamArrayBindingDescriptor {
                    element_count: element_slots.len(),
                    element_slots: element_slots.clone(),
                    lower_bound: 0,
                    empty_upper_bound: -1,
                }
            }),
        });
    }

    let return_value = meta.return_slot.map(|return_slot| CallReturnDescriptor {
        return_slot: Some(return_slot),
        assign_target_slot: assign_target,
        copyout_required: assign_target.is_some_and(|dst| dst != return_slot),
    });
    let diagnostic_policies = call_diagnostic_policies(&arguments);
    CallSiteDescriptor {
        call_site_id: format!(
            "callsite:{}@pc:{}",
            current_proc_name.to_ascii_lowercase(),
            call_pc
        ),
        caller_procedure_name: current_proc_name.to_string(),
        call_pc,
        target_name: target_name.to_string(),
        target_kind,
        target_entry_pc: None,
        default_member_policy: DefaultMemberPolicyDescriptor::NotApplicable,
        invocation_syntax,
        argument_evaluation_order: (0..source_argument_count).collect(),
        arguments,
        return_value,
        diagnostic_policies,
    }
}

fn call_diagnostic_policies(
    arguments: &[ArgumentBindingDescriptor],
) -> Vec<CallDiagnosticPolicyDescriptor> {
    let mut policies = vec![
        call_diagnostic_policy(
            CallDiagnosticKindDescriptor::MissingRequiredArgument,
            CallDiagnosticOwnerDescriptor::CompilerCurrent,
            "current-invalid-early-bound-call=runtime-449".to_string(),
        ),
        call_diagnostic_policy(
            CallDiagnosticKindDescriptor::TooManyArguments,
            CallDiagnosticOwnerDescriptor::CompilerCurrent,
            "current-invalid-early-bound-call=runtime-450".to_string(),
        ),
        call_diagnostic_policy(
            CallDiagnosticKindDescriptor::UnknownNamedArgument,
            CallDiagnosticOwnerDescriptor::CompilerCurrent,
            "current-invalid-early-bound-call=runtime-448".to_string(),
        ),
        call_diagnostic_policy(
            CallDiagnosticKindDescriptor::DuplicateArgumentMapping,
            CallDiagnosticOwnerDescriptor::CompilerCurrent,
            "current-invalid-early-bound-call=runtime-448".to_string(),
        ),
        call_diagnostic_policy(
            CallDiagnosticKindDescriptor::PositionalAfterNamed,
            CallDiagnosticOwnerDescriptor::CompilerCurrent,
            "current-invalid-early-bound-call=runtime-450".to_string(),
        ),
        call_diagnostic_policy(
            CallDiagnosticKindDescriptor::NamedParamArray,
            CallDiagnosticOwnerDescriptor::CompilerCurrent,
            "current-invalid-early-bound-call=runtime-448".to_string(),
        ),
    ];
    if arguments.iter().any(|argument| {
        argument.optional_default == Some(OptionalDefaultValue::VariantMissingError448)
    }) {
        policies.push(call_diagnostic_policy(
            CallDiagnosticKindDescriptor::OptionalVariantMissingRuntimeState,
            CallDiagnosticOwnerDescriptor::VmDeferred,
            "package-records-missing-argument; vm-current-materializes-default".to_string(),
        ));
    }
    if arguments
        .iter()
        .any(|argument| argument.binding_kind == ArgumentBindingKindDescriptor::ByRefExpressionTemp)
    {
        policies.push(call_diagnostic_policy(
            CallDiagnosticKindDescriptor::ByRefExpressionNoWriteback,
            CallDiagnosticOwnerDescriptor::OracleNeeded,
            "no-writeback-temp-shape-recorded; broaden-only-with-oracle".to_string(),
        ));
    }
    policies
}

fn call_diagnostic_policy(
    diagnostic: CallDiagnosticKindDescriptor,
    owner: CallDiagnosticOwnerDescriptor,
    detail: String,
) -> CallDiagnosticPolicyDescriptor {
    CallDiagnosticPolicyDescriptor {
        diagnostic,
        owner,
        detail,
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_dispatch_invoke_call(
    args: &[BoundCallArg],
    compare_mode: StringCompareMode,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    call_patches: &mut Vec<(usize, String)>,
    proc_meta: &HashMap<String, EmitProcMeta>,
    external_decls: &HashMap<String, BoundExternalDecl>,
    assign_target: Option<usize>,
    early_bound: bool,
) -> bool {
    let [object, member, invoke_args @ ..] = args else {
        return false;
    };
    let object_slot = temps.alloc_temp();
    emit_expr_into(
        &object.expr,
        compare_mode,
        object_slot,
        slot_map,
        temps,
        instructions,
        call_patches,
        proc_meta,
        external_decls,
    );
    let member_slot = temps.alloc_temp();
    emit_expr_into(
        &member.expr,
        compare_mode,
        member_slot,
        slot_map,
        temps,
        instructions,
        call_patches,
        proc_meta,
        external_decls,
    );
    let mut bytecode_args = Vec::with_capacity(invoke_args.len());
    for arg in invoke_args {
        let arg_slot = temps.alloc_temp();
        emit_expr_into(
            &arg.expr,
            compare_mode,
            arg_slot,
            slot_map,
            temps,
            instructions,
            call_patches,
            proc_meta,
            external_decls,
        );
        bytecode_args.push(DispatchInvokeArg {
            slot: Some(arg_slot),
            name: arg.name.clone(),
        });
    }
    let dst = assign_target.unwrap_or_else(|| temps.alloc_temp());
    let com_member = com_member_call_descriptor_for_dispatch_intrinsic(
        &member.expr,
        bytecode_args.len(),
        early_bound,
    );
    instructions.push(Instruction::IntrinsicDispatchInvokeHost {
        dst,
        object: object_slot,
        member: member_slot,
        args: bytecode_args,
        early_bound,
        com_member,
    });
    true
}

#[allow(clippy::too_many_arguments)]
fn emit_external_declare_call(
    name: &str,
    external_decl: &BoundExternalDecl,
    args: &[BoundCallArg],
    compare_mode: StringCompareMode,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    call_patches: &mut Vec<(usize, String)>,
    proc_meta: &HashMap<String, EmitProcMeta>,
    external_decls: &HashMap<String, BoundExternalDecl>,
    assign_target: Option<usize>,
) -> bool {
    let dst = assign_target.unwrap_or_else(|| temps.alloc_temp());
    let mut arg_slots = Vec::new();
    let mut writeback_slots = Vec::new();
    for (arg_index, arg) in args.iter().enumerate() {
        let slot = temps.alloc_temp();
        emit_expr_into(
            &arg.expr,
            compare_mode,
            slot,
            slot_map,
            temps,
            instructions,
            call_patches,
            proc_meta,
            external_decls,
        );
        // Track ByRef parameters for writeback after the call.
        if let Some(param) = external_decl.params.get(arg_index)
            && param.by_ref
            && !arg.force_byval
            && let BoundExpr::Var(ref ident_name) = arg.expr
            && let Some(&source_slot) = slot_map.get(&ident_name.to_ascii_lowercase())
        {
            writeback_slots.push(ExternalCallWriteback {
                arg_index,
                source_slot,
                kind: ExternalCallWritebackKind::ByRefValue,
            });
        }
        if let Some(pointer_writeback) =
            external_pointer_writeback(arg_index, &arg.expr, slot_map, external_decl, proc_meta)
        {
            writeback_slots.push(pointer_writeback);
        }
        arg_slots.push(slot);
    }
    if arg_slots.is_empty() {
        let slot = temps.alloc_temp();
        instructions.push(Instruction::LoadConstI32 { slot, value: 0 });
        arg_slots.push(slot);
    }

    let symbol = external_symbol_token(
        external_decl.library.as_str(),
        external_decl.alias.as_str(),
        name,
    );
    instructions.push(Instruction::IntrinsicInvokeSymbolHost {
        dst,
        descriptor_id: symbol as u32,
        symbol: DynLinkSymbol::new(symbol),
        args: arg_slots,
        writeback_slots,
    });
    true
}

fn external_pointer_writeback(
    arg_index: usize,
    expr: &BoundExpr,
    slot_map: &HashMap<String, usize>,
    external_decl: &BoundExternalDecl,
    proc_meta: &HashMap<String, EmitProcMeta>,
) -> Option<ExternalCallWriteback> {
    let param = external_decl.params.get(arg_index)?;
    if param.by_ref || param.ty != BoundType::LongPtr {
        return None;
    }

    // Writable pointer sync is decided from the VBA source expression and the
    // boundary shape we materialize, not from any library or API symbol name.
    match expr {
        BoundExpr::IntrinsicCall { name, args }
            if name.eq_ignore_ascii_case("varptr")
                && args.len() == 1
                && matches!(args.first(), Some(BoundExpr::VarPtrArrayBuffer(_))) =>
        {
            let BoundExpr::VarPtrArrayBuffer(name) = args.first()? else {
                return None;
            };
            let source_slot = *slot_map.get(&name.to_ascii_lowercase())?;
            Some(ExternalCallWriteback {
                arg_index,
                source_slot,
                kind: ExternalCallWritebackKind::PointerByteArrayPayload,
            })
        }
        BoundExpr::IntrinsicCall { name, args }
            if name.eq_ignore_ascii_case("strptr")
                && args.len() == 1
                && matches!(args.first(), Some(BoundExpr::Var(_))) =>
        {
            let BoundExpr::Var(name) = args.first()? else {
                return None;
            };
            let source_slot = *slot_map.get(&name.to_ascii_lowercase())?;
            Some(ExternalCallWriteback {
                arg_index,
                source_slot,
                kind: ExternalCallWritebackKind::PointerStringPayload,
            })
        }
        BoundExpr::IntrinsicCall { name, args }
            if name.eq_ignore_ascii_case("varptr")
                && args.len() == 1
                && matches!(args.first(), Some(BoundExpr::Var(_))) =>
        {
            let BoundExpr::Var(name) = args.first()? else {
                return None;
            };
            if current_proc_meta(proc_meta)
                .and_then(|meta| meta.declaration_types.get(name.as_str()).copied())
                != Some(BoundType::String)
            {
                return None;
            }
            let source_slot = *slot_map.get(&name.to_ascii_lowercase())?;
            Some(ExternalCallWriteback {
                arg_index,
                source_slot,
                kind: ExternalCallWritebackKind::PointerStringPayload,
            })
        }
        _ => None,
    }
}

fn external_symbol_token(library: &str, alias: &str, name: &str) -> i32 {
    let mut hash: u32 = 2_166_136_261;
    let library = library.to_ascii_lowercase();
    let alias = alias.to_ascii_lowercase();
    let name = name.to_ascii_lowercase();
    for byte in library
        .bytes()
        .chain([b'!'])
        .chain(alias.bytes())
        .chain([b'!'])
        .chain(name.bytes())
    {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    (hash & 0x7fff_ffff).max(1) as i32
}

pub fn bound_type_to_declare_param_type(ty: &BoundType) -> DeclareParamType {
    match ty {
        BoundType::Long => DeclareParamType::Long,
        BoundType::Integer => DeclareParamType::Integer,
        BoundType::String => DeclareParamType::String,
        BoundType::Boolean => DeclareParamType::Boolean,
        BoundType::Double => DeclareParamType::Double,
        BoundType::Single => DeclareParamType::Single,
        BoundType::Currency => DeclareParamType::Currency,
        BoundType::Date => DeclareParamType::Date,
        BoundType::Byte => DeclareParamType::Byte,
        BoundType::LongLong => DeclareParamType::LongLong,
        BoundType::LongPtr => DeclareParamType::LongPtr,
        BoundType::Variant | BoundType::Object | BoundType::Array | BoundType::Decimal => {
            DeclareParamType::Variant
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_late_bound_default_member_call(
    name: &str,
    args: &[BoundCallArg],
    invocation_syntax: CallInvocationSyntaxDescriptor,
    compare_mode: StringCompareMode,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    call_patches: &mut Vec<(usize, String)>,
    proc_meta: &HashMap<String, EmitProcMeta>,
    external_decls: &HashMap<String, BoundExternalDecl>,
    assign_target: Option<usize>,
    call_site_descriptors: Option<&mut Vec<CallSiteDescriptor>>,
    current_proc_name: Option<&str>,
) -> bool {
    let Some(object_slot) = slot_map.get(name).copied() else {
        return false;
    };
    let member_slot = temps.alloc_temp();
    instructions.push(Instruction::LoadConstI32 {
        slot: member_slot,
        value: 0,
    });
    let mut invoke_args = Vec::with_capacity(args.len());
    let mut arg_slots = Vec::with_capacity(args.len());
    for arg in args {
        let arg_slot = temps.alloc_temp();
        emit_expr_into(
            &arg.expr,
            compare_mode,
            arg_slot,
            slot_map,
            temps,
            instructions,
            call_patches,
            proc_meta,
            external_decls,
        );
        invoke_args.push(DispatchInvokeArg {
            slot: Some(arg_slot),
            name: arg.name.clone(),
        });
        arg_slots.push(arg_slot);
    }
    let dst = assign_target.unwrap_or_else(|| temps.alloc_temp());
    let call_pc = instructions.len();
    instructions.push(Instruction::IntrinsicDispatchInvokeHost {
        dst,
        object: object_slot,
        member: member_slot,
        args: invoke_args,
        early_bound: false,
        com_member: None,
    });
    if let (Some(call_site_descriptors), Some(current_proc_name)) =
        (call_site_descriptors, current_proc_name)
    {
        call_site_descriptors.push(build_late_bound_default_member_call_site_descriptor(
            current_proc_name,
            call_pc,
            name,
            object_slot,
            args,
            &arg_slots,
            invocation_syntax,
            Some(dst),
        ));
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn build_late_bound_default_member_call_site_descriptor(
    current_proc_name: &str,
    call_pc: usize,
    target_name: &str,
    _object_slot: usize,
    args: &[BoundCallArg],
    arg_slots: &[usize],
    invocation_syntax: CallInvocationSyntaxDescriptor,
    assign_target: Option<usize>,
) -> CallSiteDescriptor {
    let arguments = args
        .iter()
        .enumerate()
        .map(|(idx, arg)| ArgumentBindingDescriptor {
            argument_index: idx,
            source_index: Some(idx),
            source_name: arg.name.clone(),
            parameter_index: None,
            parameter_name: None,
            parameter_slot: None,
            source_kind: argument_source_kind(arg),
            expression_kind: argument_expression_kind(&arg.expr),
            binding_kind: ArgumentBindingKindDescriptor::ByValCopy,
            force_byval: arg.force_byval,
            source_slot: arg_slots.get(idx).copied(),
            writeback: None,
            optional_default: None,
            param_array: None,
        })
        .collect::<Vec<_>>();
    CallSiteDescriptor {
        call_site_id: format!(
            "callsite:{}@pc:{}",
            current_proc_name.to_ascii_lowercase(),
            call_pc
        ),
        caller_procedure_name: current_proc_name.to_string(),
        call_pc,
        target_name: target_name.to_string(),
        target_kind: CallTargetKindDescriptor::LateBoundDefaultMember,
        target_entry_pc: None,
        default_member_policy: DefaultMemberPolicyDescriptor::DefaultMemberFallback,
        invocation_syntax,
        argument_evaluation_order: (0..args.len()).collect(),
        arguments,
        return_value: Some(CallReturnDescriptor {
            return_slot: assign_target,
            assign_target_slot: assign_target,
            copyout_required: false,
        }),
        diagnostic_policies: Vec::new(),
    }
}

fn emit_select_case_clause_match(
    expr_slot: usize,
    clause: &BoundCaseClause,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    _call_patches: &mut Vec<(usize, String)>,
    _proc_meta: &HashMap<String, EmitProcMeta>,
    _external_decls: &HashMap<String, BoundExternalDecl>,
) -> usize {
    match clause {
        BoundCaseClause::Value(value) => {
            let const_slot = temps.alloc_temp();
            let cmp_slot = temps.alloc_temp();
            instructions.push(Instruction::LoadConstI32 {
                slot: const_slot,
                value: *value,
            });
            instructions.push(Instruction::CmpEqSlots {
                dst: cmp_slot,
                lhs: expr_slot,
                rhs: const_slot,
                mode: StringCompareMode::Binary,
            });
            cmp_slot
        }
        BoundCaseClause::Is { op, value } => {
            let const_slot = temps.alloc_temp();
            let cmp_slot = temps.alloc_temp();
            instructions.push(Instruction::LoadConstI32 {
                slot: const_slot,
                value: *value,
            });
            match op {
                CompareOp::Eq => instructions.push(Instruction::CmpEqSlots {
                    dst: cmp_slot,
                    lhs: expr_slot,
                    rhs: const_slot,
                    mode: StringCompareMode::Binary,
                }),
                CompareOp::Ne => instructions.push(Instruction::CmpNeSlots {
                    dst: cmp_slot,
                    lhs: expr_slot,
                    rhs: const_slot,
                    mode: StringCompareMode::Binary,
                }),
                CompareOp::Lt => instructions.push(Instruction::CmpLtSlots {
                    dst: cmp_slot,
                    lhs: expr_slot,
                    rhs: const_slot,
                    mode: StringCompareMode::Binary,
                }),
                CompareOp::Le => instructions.push(Instruction::CmpLeSlots {
                    dst: cmp_slot,
                    lhs: expr_slot,
                    rhs: const_slot,
                    mode: StringCompareMode::Binary,
                }),
                CompareOp::Gt => instructions.push(Instruction::CmpGtSlots {
                    dst: cmp_slot,
                    lhs: expr_slot,
                    rhs: const_slot,
                    mode: StringCompareMode::Binary,
                }),
                CompareOp::Ge => instructions.push(Instruction::CmpGeSlots {
                    dst: cmp_slot,
                    lhs: expr_slot,
                    rhs: const_slot,
                    mode: StringCompareMode::Binary,
                }),
                CompareOp::Like => instructions.push(Instruction::IntrinsicLikeDigits {
                    dst: cmp_slot,
                    lhs: expr_slot,
                    pattern: const_slot,
                    mode: StringCompareMode::Binary,
                }),
            }
            cmp_slot
        }
        BoundCaseClause::Range { start, end } => {
            let start_slot = temps.alloc_temp();
            let end_slot = temps.alloc_temp();
            let ge_slot = temps.alloc_temp();
            let le_slot = temps.alloc_temp();
            let cmp_slot = temps.alloc_temp();
            instructions.push(Instruction::LoadConstI32 {
                slot: start_slot,
                value: *start,
            });
            instructions.push(Instruction::LoadConstI32 {
                slot: end_slot,
                value: *end,
            });
            instructions.push(Instruction::CmpGeSlots {
                dst: ge_slot,
                lhs: expr_slot,
                rhs: start_slot,
                mode: StringCompareMode::Binary,
            });
            instructions.push(Instruction::CmpLeSlots {
                dst: le_slot,
                lhs: expr_slot,
                rhs: end_slot,
                mode: StringCompareMode::Binary,
            });
            instructions.push(Instruction::BoolAnd {
                dst: cmp_slot,
                lhs: ge_slot,
                rhs: le_slot,
            });
            cmp_slot
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_cond_into(
    cond: &BoundCond,
    compare_mode: StringCompareMode,
    dst: usize,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    call_patches: &mut Vec<(usize, String)>,
    proc_meta: &HashMap<String, EmitProcMeta>,
    external_decls: &HashMap<String, BoundExternalDecl>,
) {
    match cond {
        BoundCond::Compare { op, lhs, rhs } => {
            let lhs_slot = temps.alloc_temp();
            let rhs_slot = temps.alloc_temp();
            emit_expr_into(
                lhs,
                compare_mode,
                lhs_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            emit_expr_into(
                rhs,
                compare_mode,
                rhs_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            match op {
                CompareOp::Eq => instructions.push(Instruction::CmpEqSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                    mode: compare_mode,
                }),
                CompareOp::Ne => instructions.push(Instruction::CmpNeSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                    mode: compare_mode,
                }),
                CompareOp::Lt => instructions.push(Instruction::CmpLtSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                    mode: compare_mode,
                }),
                CompareOp::Le => instructions.push(Instruction::CmpLeSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                    mode: compare_mode,
                }),
                CompareOp::Gt => instructions.push(Instruction::CmpGtSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                    mode: compare_mode,
                }),
                CompareOp::Ge => instructions.push(Instruction::CmpGeSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                    mode: compare_mode,
                }),
                CompareOp::Like => instructions.push(Instruction::IntrinsicLikeDigits {
                    dst,
                    lhs: lhs_slot,
                    pattern: rhs_slot,
                    mode: compare_mode,
                }),
            }
        }
        BoundCond::Truthy(expr) => {
            let expr_slot = temps.alloc_temp();
            let zero_slot = temps.alloc_temp();
            emit_expr_into(
                expr,
                compare_mode,
                expr_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            instructions.push(Instruction::LoadConstI32 {
                slot: zero_slot,
                value: 0,
            });
            instructions.push(Instruction::CmpNeSlots {
                dst,
                lhs: expr_slot,
                rhs: zero_slot,
                mode: compare_mode,
            });
        }
        BoundCond::Not(inner) => {
            let inner_slot = temps.alloc_temp();
            emit_cond_into(
                inner,
                compare_mode,
                inner_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            instructions.push(Instruction::BoolNot {
                dst,
                src: inner_slot,
            });
        }
        BoundCond::And(lhs, rhs) => {
            let lhs_slot = temps.alloc_temp();
            let rhs_slot = temps.alloc_temp();
            emit_cond_into(
                lhs,
                compare_mode,
                lhs_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            emit_cond_into(
                rhs,
                compare_mode,
                rhs_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            instructions.push(Instruction::BoolAnd {
                dst,
                lhs: lhs_slot,
                rhs: rhs_slot,
            });
        }
        BoundCond::Or(lhs, rhs) => {
            let lhs_slot = temps.alloc_temp();
            let rhs_slot = temps.alloc_temp();
            emit_cond_into(
                lhs,
                compare_mode,
                lhs_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            emit_cond_into(
                rhs,
                compare_mode,
                rhs_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            instructions.push(Instruction::BoolOr {
                dst,
                lhs: lhs_slot,
                rhs: rhs_slot,
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_expr_into(
    expr: &BoundExpr,
    compare_mode: StringCompareMode,
    dst: usize,
    slot_map: &HashMap<String, usize>,
    temps: &mut TempSlotAllocator,
    instructions: &mut Vec<Instruction>,
    call_patches: &mut Vec<(usize, String)>,
    proc_meta: &HashMap<String, EmitProcMeta>,
    external_decls: &HashMap<String, BoundExternalDecl>,
) {
    match expr {
        BoundExpr::IntConst(value) => instructions.push(Instruction::LoadConstI32 {
            slot: dst,
            value: *value,
        }),
        BoundExpr::BoolConst(value) => instructions.push(Instruction::LoadConstBool {
            slot: dst,
            value: *value,
        }),
        BoundExpr::FloatConst(bits) => instructions.push(Instruction::LoadConstF64 {
            slot: dst,
            bits: *bits,
        }),
        BoundExpr::StringConst(s) => instructions.push(Instruction::LoadConstString {
            slot: dst,
            value: s.clone(),
        }),
        BoundExpr::Var(name) => {
            if let Some(src) = slot_map.get(name.as_str()).copied()
                && src != dst
            {
                instructions.push(Instruction::CopySlot { dst, src });
            } else {
                let _ = emit_err_member_value(name, dst, instructions);
            }
        }
        BoundExpr::VarPtrArrayBuffer(name) => {
            let element_slots = collect_array_element_slots(name, slot_map);
            if element_slots.is_empty() {
                if let Some(src) = slot_map.get(name.as_str()).copied()
                    && src != dst
                {
                    instructions.push(Instruction::CopySlot { dst, src });
                }
            } else {
                instructions.push(Instruction::IntrinsicArrayLiteral {
                    dst,
                    values: element_slots,
                });
            }
        }
        BoundExpr::AddConst { var, delta } => {
            if let Some(src) = slot_map.get(var.as_str()).copied() {
                if src != dst {
                    instructions.push(Instruction::CopySlot { dst, src });
                }
                instructions.push(Instruction::AddConstI32 {
                    slot: dst,
                    value: *delta,
                });
            } else if emit_err_member_value(var, dst, instructions) {
                instructions.push(Instruction::AddConstI32 {
                    slot: dst,
                    value: *delta,
                });
            }
        }
        BoundExpr::SubConst { var, delta } => {
            if let Some(src) = slot_map.get(var.as_str()).copied() {
                if src != dst {
                    instructions.push(Instruction::CopySlot { dst, src });
                }
                instructions.push(Instruction::SubConstI32 {
                    slot: dst,
                    value: *delta,
                });
            } else if emit_err_member_value(var, dst, instructions) {
                instructions.push(Instruction::SubConstI32 {
                    slot: dst,
                    value: *delta,
                });
            }
        }
        BoundExpr::BinaryOp { op, lhs, rhs } => {
            let lhs_slot = temps.alloc_temp();
            emit_expr_into(
                lhs,
                compare_mode,
                lhs_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            let rhs_slot = temps.alloc_temp();
            emit_expr_into(
                rhs,
                compare_mode,
                rhs_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            let instr = match op {
                ArithOp::Add => Instruction::AddSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                },
                ArithOp::Sub => Instruction::SubSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                },
                ArithOp::Mul => Instruction::MulSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                },
                ArithOp::Div => Instruction::DivSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                },
                ArithOp::IntDiv => Instruction::IntDivSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                },
                ArithOp::Mod => Instruction::ModSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                },
                ArithOp::Pow => Instruction::PowSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                },
                ArithOp::Concat => Instruction::ConcatSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                },
                ArithOp::Neg => unreachable!("Neg is unary"),
            };
            instructions.push(instr);
        }
        BoundExpr::CompareOp { op, lhs, rhs } => {
            let lhs_slot = temps.alloc_temp();
            emit_expr_into(
                lhs,
                compare_mode,
                lhs_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            let rhs_slot = temps.alloc_temp();
            emit_expr_into(
                rhs,
                compare_mode,
                rhs_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            match op {
                CompareOp::Eq => instructions.push(Instruction::CmpEqSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                    mode: compare_mode,
                }),
                CompareOp::Ne => instructions.push(Instruction::CmpNeSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                    mode: compare_mode,
                }),
                CompareOp::Lt => instructions.push(Instruction::CmpLtSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                    mode: compare_mode,
                }),
                CompareOp::Le => instructions.push(Instruction::CmpLeSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                    mode: compare_mode,
                }),
                CompareOp::Gt => instructions.push(Instruction::CmpGtSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                    mode: compare_mode,
                }),
                CompareOp::Ge => instructions.push(Instruction::CmpGeSlots {
                    dst,
                    lhs: lhs_slot,
                    rhs: rhs_slot,
                    mode: compare_mode,
                }),
                CompareOp::Like => instructions.push(Instruction::IntrinsicLikeDigits {
                    dst,
                    lhs: lhs_slot,
                    pattern: rhs_slot,
                    mode: compare_mode,
                }),
            }
        }
        BoundExpr::UnaryOp { op, operand } => {
            let src_slot = temps.alloc_temp();
            emit_expr_into(
                operand,
                compare_mode,
                src_slot,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
            );
            match op {
                ArithOp::Neg => {
                    instructions.push(Instruction::NegSlot { dst, src: src_slot });
                }
                _ => unreachable!("only Neg is a unary ArithOp"),
            }
        }
        BoundExpr::IntrinsicCall { name, args } => {
            // Special case: TypeOf...Is embeds the type name string directly in the
            // instruction rather than materializing it as a runtime slot value.
            if name == "typeofis" && args.len() == 2 {
                let object_slot = temps.alloc_temp();
                emit_expr_into(
                    &args[0],
                    compare_mode,
                    object_slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                let type_name = match &args[1] {
                    BoundExpr::StringConst(s) => s.clone(),
                    _ => String::new(),
                };
                instructions.push(Instruction::IntrinsicTypeOfIs {
                    dst,
                    object_slot,
                    type_name,
                });
                return;
            }

            let mut arg_slots = Vec::with_capacity(args.len());
            for arg in args {
                let slot = temps.alloc_temp();
                emit_expr_into(
                    arg,
                    compare_mode,
                    slot,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                );
                arg_slots.push(slot);
            }

            match (name.as_str(), arg_slots.as_slice()) {
                ("__empty", []) => {
                    instructions.push(Instruction::LoadEmpty { slot: dst });
                }
                ("__null", []) => {
                    instructions.push(Instruction::LoadNull { slot: dst });
                }
                ("vbnullstring", []) => instructions.push(Instruction::LoadConstString {
                    slot: dst,
                    value: String::new(),
                }),
                ("cverr", [src]) => {
                    instructions.push(Instruction::IntrinsicCVErr { dst, src: *src });
                }
                ("date", []) => instructions.push(Instruction::IntrinsicDateNowHost { dst }),
                ("time", []) => instructions.push(Instruction::IntrinsicTimeNowHost { dst }),
                // Current register-window value model uses i32 tokens; `Now` currently
                // projects to date token until composite date-time value lowering lands.
                ("now", []) => instructions.push(Instruction::IntrinsicNowHost { dst }),
                ("timer", []) => instructions.push(Instruction::IntrinsicTimerHost { dst }),
                ("rnd", []) => {
                    instructions.push(Instruction::IntrinsicRndDigits { dst, seed: None })
                }
                ("randomize", []) => {
                    instructions.push(Instruction::IntrinsicRandomizeDigits { dst, seed: None })
                }
                ("freefile", []) => instructions.push(Instruction::IntrinsicFreeFileHost {
                    dst,
                    range_selector: None,
                }),
                ("freefile", [range_selector]) => {
                    instructions.push(Instruction::IntrinsicFreeFileHost {
                        dst,
                        range_selector: Some(*range_selector),
                    })
                }
                ("eof", [handle]) => instructions.push(Instruction::IntrinsicFileEofHost {
                    dst,
                    handle: *handle,
                }),
                ("lof", [handle]) => instructions.push(Instruction::IntrinsicFileLofHost {
                    dst,
                    handle: *handle,
                }),
                ("seek", [handle]) => instructions.push(Instruction::IntrinsicFileSeekHost {
                    dst,
                    handle: *handle,
                }),
                ("loc", [handle]) => instructions.push(Instruction::IntrinsicFileLocHost {
                    dst,
                    handle: *handle,
                }),
                ("doevents", []) => instructions.push(Instruction::IntrinsicDoEventsHost { dst }),
                ("msgbox", [prompt]) => instructions.push(Instruction::IntrinsicMsgBoxHost {
                    dst,
                    prompt: *prompt,
                    style: None,
                }),
                ("msgbox", [prompt, style]) => {
                    instructions.push(Instruction::IntrinsicMsgBoxHost {
                        dst,
                        prompt: *prompt,
                        style: Some(*style),
                    })
                }
                // Current HAL trait takes prompt/default only; this lowering uses arg2 as default.
                ("inputbox", [prompt]) => instructions.push(Instruction::IntrinsicInputBoxHost {
                    dst,
                    prompt: *prompt,
                    default_value: None,
                }),
                ("inputbox", [prompt, default_value]) => {
                    instructions.push(Instruction::IntrinsicInputBoxHost {
                        dst,
                        prompt: *prompt,
                        default_value: Some(*default_value),
                    })
                }
                ("len", [src]) => {
                    instructions.push(Instruction::IntrinsicLenDigits { dst, src: *src })
                }
                ("left", [src, count]) => instructions.push(Instruction::IntrinsicLeftDigits {
                    dst,
                    src: *src,
                    count: *count,
                }),
                ("right", [src, count]) => instructions.push(Instruction::IntrinsicRightDigits {
                    dst,
                    src: *src,
                    count: *count,
                }),
                ("mid", [src, start]) => instructions.push(Instruction::IntrinsicMidDigits {
                    dst,
                    src: *src,
                    start: *start,
                    count: None,
                }),
                ("mid", [src, start, count]) => {
                    instructions.push(Instruction::IntrinsicMidDigits {
                        dst,
                        src: *src,
                        start: *start,
                        count: Some(*count),
                    })
                }
                ("instr", [haystack, needle]) => {
                    instructions.push(Instruction::IntrinsicInStrDigits {
                        dst,
                        haystack: *haystack,
                        needle: *needle,
                        mode: compare_mode,
                    })
                }
                ("instrrev", [haystack, needle]) => {
                    instructions.push(Instruction::IntrinsicInStrRevDigits {
                        dst,
                        haystack: *haystack,
                        needle: *needle,
                        mode: compare_mode,
                    })
                }
                ("lcase", [src]) => {
                    instructions.push(Instruction::IntrinsicLowerDigits { dst, src: *src })
                }
                ("ucase", [src]) => {
                    instructions.push(Instruction::IntrinsicUpperDigits { dst, src: *src })
                }
                ("split", [src, delimiter]) => {
                    instructions.push(Instruction::IntrinsicSplitCountDigits {
                        dst,
                        src: *src,
                        delimiter: *delimiter,
                    })
                }
                ("join", [src, delimiter]) => instructions.push(Instruction::IntrinsicJoinDigits {
                    dst,
                    src: *src,
                    delimiter: *delimiter,
                }),
                ("replace", [src, find, replace]) => {
                    instructions.push(Instruction::IntrinsicReplaceDigits {
                        dst,
                        src: *src,
                        find: *find,
                        replace: *replace,
                    })
                }
                ("trim", [src]) => {
                    instructions.push(Instruction::IntrinsicTrimDigits { dst, src: *src })
                }
                ("ltrim", [src]) => {
                    instructions.push(Instruction::IntrinsicLTrimDigits { dst, src: *src })
                }
                ("rtrim", [src]) => {
                    instructions.push(Instruction::IntrinsicRTrimDigits { dst, src: *src })
                }
                ("strcomp", [lhs, rhs]) => instructions.push(Instruction::IntrinsicStrCompDigits {
                    dst,
                    lhs: *lhs,
                    rhs: *rhs,
                    mode: compare_mode,
                }),
                ("space", [count]) => {
                    instructions.push(Instruction::IntrinsicSpaceDigits { dst, count: *count })
                }
                ("string", [count, ch]) => {
                    instructions.push(Instruction::IntrinsicStringRepeatDigits {
                        dst,
                        count: *count,
                        ch: *ch,
                    })
                }
                ("chr", [src]) => {
                    instructions.push(Instruction::IntrinsicChrDigits { dst, src: *src })
                }
                ("asc", [src]) => {
                    instructions.push(Instruction::IntrinsicAscDigits { dst, src: *src })
                }
                ("strptr", [src]) => {
                    instructions.push(Instruction::IntrinsicStrPtr { dst, src: *src })
                }
                ("varptr", [src]) => {
                    let instruction = match args.first() {
                        Some(BoundExpr::Var(name)) => match current_proc_meta(proc_meta)
                            .and_then(|meta| meta.declaration_types.get(name.as_str()).copied())
                        {
                            Some(BoundType::String) => {
                                Instruction::IntrinsicVarPtrStringVar { dst, src: *src }
                            }
                            Some(BoundType::Variant) => {
                                Instruction::IntrinsicVarPtrVariantVar { dst, src: *src }
                            }
                            _ => Instruction::IntrinsicVarPtr { dst, src: *src },
                        },
                        _ => Instruction::IntrinsicVarPtr { dst, src: *src },
                    };
                    instructions.push(instruction)
                }
                ("__oxvba_array_get", [array, indices @ ..]) if !indices.is_empty() => instructions
                    .push(Instruction::IntrinsicArrayGet {
                        dst,
                        array: *array,
                        indices: indices.to_vec(),
                    }),
                ("objptr", [src]) => {
                    instructions.push(Instruction::IntrinsicObjPtr { dst, src: *src })
                }
                ("strreverse", [src]) => {
                    instructions.push(Instruction::IntrinsicStrReverseDigits { dst, src: *src })
                }
                ("strconv", [value, conversion]) => {
                    instructions.push(Instruction::IntrinsicStrConvDigits {
                        dst,
                        src: *value,
                        conversion: *conversion,
                    })
                }
                ("strconv", [value, conversion, _lcid]) => {
                    instructions.push(Instruction::IntrinsicStrConvDigits {
                        dst,
                        src: *value,
                        conversion: *conversion,
                    })
                }
                ("format", [value]) => instructions.push(Instruction::IntrinsicFormatDigits {
                    dst,
                    value: *value,
                    format_string: None,
                }),
                ("format", [value, fmt]) => instructions.push(Instruction::IntrinsicFormatDigits {
                    dst,
                    value: *value,
                    format_string: Some(*fmt),
                }),
                ("dateserial", [year, month, day]) => {
                    instructions.push(Instruction::IntrinsicDateSerialDigits {
                        dst,
                        year: *year,
                        month: *month,
                        day: *day,
                    })
                }
                ("timeserial", [hour, minute, second]) => {
                    instructions.push(Instruction::IntrinsicTimeSerialDigits {
                        dst,
                        hour: *hour,
                        minute: *minute,
                        second: *second,
                    })
                }
                ("datevalue", [src]) => {
                    instructions.push(Instruction::IntrinsicDateValueDigits { dst, src: *src })
                }
                ("timevalue", [src]) => {
                    instructions.push(Instruction::IntrinsicTimeValueDigits { dst, src: *src })
                }
                ("dateadd", [interval, number, date]) => {
                    instructions.push(Instruction::IntrinsicDateAddDigits {
                        dst,
                        interval: *interval,
                        number: *number,
                        date: *date,
                    })
                }
                ("datediff", [interval, date1, date2]) => {
                    instructions.push(Instruction::IntrinsicDateDiffDigits {
                        dst,
                        interval: *interval,
                        date1: *date1,
                        date2: *date2,
                    })
                }
                ("abs", [src]) => {
                    instructions.push(Instruction::IntrinsicAbsI32 { dst, src: *src })
                }
                ("int", [src]) => {
                    instructions.push(Instruction::IntrinsicIntI32 { dst, src: *src })
                }
                ("fix", [src]) => {
                    instructions.push(Instruction::IntrinsicFixI32 { dst, src: *src })
                }
                ("sgn", [src]) => {
                    instructions.push(Instruction::IntrinsicSgnI32 { dst, src: *src })
                }
                ("round", [src, digits]) => instructions.push(Instruction::IntrinsicRoundI32 {
                    dst,
                    src: *src,
                    digits: Some(*digits),
                }),
                ("round", [src]) => instructions.push(Instruction::IntrinsicRoundI32 {
                    dst,
                    src: *src,
                    digits: None,
                }),
                ("sqr", [src]) => {
                    instructions.push(Instruction::IntrinsicSqrI32 { dst, src: *src })
                }
                ("sin", [src]) => {
                    instructions.push(Instruction::IntrinsicSinI32 { dst, src: *src })
                }
                ("cos", [src]) => {
                    instructions.push(Instruction::IntrinsicCosI32 { dst, src: *src })
                }
                ("log", [src]) => {
                    instructions.push(Instruction::IntrinsicLogI32 { dst, src: *src })
                }
                ("exp", [src]) => {
                    instructions.push(Instruction::IntrinsicExpI32 { dst, src: *src })
                }
                ("atn", [src]) => {
                    instructions.push(Instruction::IntrinsicAtnI32 { dst, src: *src })
                }
                ("tan", [src]) => {
                    instructions.push(Instruction::IntrinsicTanI32 { dst, src: *src })
                }
                ("cstr", [src]) => {
                    instructions.push(Instruction::IntrinsicCStrDigits { dst, src: *src })
                }
                ("str", [src]) => {
                    instructions.push(Instruction::IntrinsicStrFuncDigits { dst, src: *src })
                }
                ("val", [src]) => {
                    instructions.push(Instruction::IntrinsicValDigits { dst, src: *src })
                }
                ("cdate", [src]) => {
                    instructions.push(Instruction::IntrinsicCDateValue { dst, src: *src })
                }
                ("hex", [src]) => {
                    instructions.push(Instruction::IntrinsicHexDigits { dst, src: *src })
                }
                ("oct", [src]) => {
                    instructions.push(Instruction::IntrinsicOctDigits { dst, src: *src })
                }
                ("year", [src]) => {
                    instructions.push(Instruction::IntrinsicYearDigits { dst, src: *src })
                }
                ("month", [src]) => {
                    instructions.push(Instruction::IntrinsicMonthDigits { dst, src: *src })
                }
                ("day", [src]) => {
                    instructions.push(Instruction::IntrinsicDayDigits { dst, src: *src })
                }
                ("weekday", [src]) => {
                    instructions.push(Instruction::IntrinsicWeekdayDigits { dst, src: *src })
                }
                ("monthname", [src]) => {
                    instructions.push(Instruction::IntrinsicMonthNameDigits { dst, src: *src })
                }
                ("fv", [rate, nper, pmt]) => instructions.push(Instruction::IntrinsicFvI32 {
                    dst,
                    rate: *rate,
                    nper: *nper,
                    pmt: *pmt,
                    pv: None,
                    due: None,
                }),
                ("fv", [rate, nper, pmt, pv]) => instructions.push(Instruction::IntrinsicFvI32 {
                    dst,
                    rate: *rate,
                    nper: *nper,
                    pmt: *pmt,
                    pv: Some(*pv),
                    due: None,
                }),
                ("fv", [rate, nper, pmt, pv, due]) => {
                    instructions.push(Instruction::IntrinsicFvI32 {
                        dst,
                        rate: *rate,
                        nper: *nper,
                        pmt: *pmt,
                        pv: Some(*pv),
                        due: Some(*due),
                    })
                }
                ("pv", [rate, nper, pmt]) => instructions.push(Instruction::IntrinsicPvI32 {
                    dst,
                    rate: *rate,
                    nper: *nper,
                    pmt: *pmt,
                    fv: None,
                    due: None,
                }),
                ("pv", [rate, nper, pmt, fv]) => instructions.push(Instruction::IntrinsicPvI32 {
                    dst,
                    rate: *rate,
                    nper: *nper,
                    pmt: *pmt,
                    fv: Some(*fv),
                    due: None,
                }),
                ("pv", [rate, nper, pmt, fv, due]) => {
                    instructions.push(Instruction::IntrinsicPvI32 {
                        dst,
                        rate: *rate,
                        nper: *nper,
                        pmt: *pmt,
                        fv: Some(*fv),
                        due: Some(*due),
                    })
                }
                ("pmt", [rate, nper, pv]) => instructions.push(Instruction::IntrinsicPmtI32 {
                    dst,
                    rate: *rate,
                    nper: *nper,
                    pv: *pv,
                    fv: None,
                    due: None,
                }),
                ("pmt", [rate, nper, pv, fv]) => instructions.push(Instruction::IntrinsicPmtI32 {
                    dst,
                    rate: *rate,
                    nper: *nper,
                    pv: *pv,
                    fv: Some(*fv),
                    due: None,
                }),
                ("pmt", [rate, nper, pv, fv, due]) => {
                    instructions.push(Instruction::IntrinsicPmtI32 {
                        dst,
                        rate: *rate,
                        nper: *nper,
                        pv: *pv,
                        fv: Some(*fv),
                        due: Some(*due),
                    })
                }
                ("npv", slots) if slots.len() >= 2 => {
                    instructions.push(Instruction::IntrinsicNpvI32 {
                        dst,
                        rate: slots[0],
                        values: slots[1..].to_vec(),
                    })
                }
                ("irr", [value]) => instructions.push(Instruction::IntrinsicIrrI32 {
                    dst,
                    value: *value,
                    guess: None,
                }),
                ("irr", [value, guess]) => instructions.push(Instruction::IntrinsicIrrI32 {
                    dst,
                    value: *value,
                    guess: Some(*guess),
                }),
                ("mirr", [value, finance_rate, reinvest_rate]) => {
                    instructions.push(Instruction::IntrinsicMirrI32 {
                        dst,
                        value: *value,
                        finance_rate: *finance_rate,
                        reinvest_rate: *reinvest_rate,
                    })
                }
                ("rate", [nper, pmt, pv]) => instructions.push(Instruction::IntrinsicRateI32 {
                    dst,
                    nper: *nper,
                    pmt: *pmt,
                    pv: *pv,
                    fv: None,
                    due: None,
                    guess: None,
                }),
                ("rate", [nper, pmt, pv, fv]) => instructions.push(Instruction::IntrinsicRateI32 {
                    dst,
                    nper: *nper,
                    pmt: *pmt,
                    pv: *pv,
                    fv: Some(*fv),
                    due: None,
                    guess: None,
                }),
                ("rate", [nper, pmt, pv, fv, due]) => {
                    instructions.push(Instruction::IntrinsicRateI32 {
                        dst,
                        nper: *nper,
                        pmt: *pmt,
                        pv: *pv,
                        fv: Some(*fv),
                        due: Some(*due),
                        guess: None,
                    })
                }
                ("rate", [nper, pmt, pv, fv, due, guess]) => {
                    instructions.push(Instruction::IntrinsicRateI32 {
                        dst,
                        nper: *nper,
                        pmt: *pmt,
                        pv: *pv,
                        fv: Some(*fv),
                        due: Some(*due),
                        guess: Some(*guess),
                    })
                }
                ("nper", [rate, pmt, pv]) => instructions.push(Instruction::IntrinsicNPerI32 {
                    dst,
                    rate: *rate,
                    pmt: *pmt,
                    pv: *pv,
                    fv: None,
                    due: None,
                }),
                ("nper", [rate, pmt, pv, fv]) => instructions.push(Instruction::IntrinsicNPerI32 {
                    dst,
                    rate: *rate,
                    pmt: *pmt,
                    pv: *pv,
                    fv: Some(*fv),
                    due: None,
                }),
                ("nper", [rate, pmt, pv, fv, due]) => {
                    instructions.push(Instruction::IntrinsicNPerI32 {
                        dst,
                        rate: *rate,
                        pmt: *pmt,
                        pv: *pv,
                        fv: Some(*fv),
                        due: Some(*due),
                    })
                }
                ("array", args) => {
                    instructions.push(Instruction::IntrinsicArrayLiteral {
                        dst,
                        values: args.to_vec(),
                    });
                }
                ("__oxvba_array_append", [array, item]) => {
                    instructions.push(Instruction::IntrinsicArrayAppend {
                        dst,
                        array: *array,
                        item: *item,
                    });
                }
                ("lbound", [src]) => {
                    instructions.push(Instruction::IntrinsicLBoundArray { dst, src: *src })
                }
                ("ubound", [src]) => {
                    instructions.push(Instruction::IntrinsicUBoundArray { dst, src: *src })
                }
                ("isarray", [src]) => {
                    instructions.push(Instruction::IntrinsicIsArrayTag { dst, src: *src })
                }
                ("vartype", [src]) => {
                    instructions.push(Instruction::IntrinsicVarType { dst, src: *src })
                }
                ("typename", [src]) => {
                    instructions.push(Instruction::IntrinsicTypeNameTag { dst, src: *src })
                }
                ("isnumeric", [src]) => {
                    instructions.push(Instruction::IntrinsicIsNumeric { dst, src: *src })
                }
                ("isdate", [src]) => {
                    instructions.push(Instruction::IntrinsicIsDateTag { dst, src: *src })
                }
                ("isobject", [src]) => {
                    instructions.push(Instruction::IntrinsicIsObjectTag { dst, src: *src })
                }
                ("isempty", [src]) => {
                    instructions.push(Instruction::IntrinsicIsEmpty { dst, src: *src });
                }
                ("isnull", [src]) => {
                    instructions.push(Instruction::IntrinsicIsNull { dst, src: *src });
                }
                ("iserror", [src]) => {
                    instructions.push(Instruction::IntrinsicIsError { dst, src: *src });
                }
                ("typeofis", _) => {
                    // Handled by the special case above; should never reach here.
                    unreachable!("typeofis is handled before arg materialization");
                }
                ("rnd", [seed]) => instructions.push(Instruction::IntrinsicRndDigits {
                    dst,
                    seed: Some(*seed),
                }),
                ("randomize", [seed]) => instructions.push(Instruction::IntrinsicRandomizeDigits {
                    dst,
                    seed: Some(*seed),
                }),
                ("shell", [command]) => instructions.push(Instruction::IntrinsicShellHost {
                    dst,
                    command: *command,
                }),
                ("environ", [key]) => {
                    instructions.push(Instruction::IntrinsicEnvironHost { dst, key: *key })
                }
                ("dir", []) => {
                    let path = temps.alloc_temp();
                    instructions.push(Instruction::LoadConstI32 {
                        slot: path,
                        value: 0,
                    });
                    instructions.push(Instruction::IntrinsicDirHost { dst, path })
                }
                ("dir", [path]) => {
                    instructions.push(Instruction::IntrinsicDirHost { dst, path: *path })
                }
                ("collectionadd", [count, item]) => {
                    instructions.push(Instruction::IntrinsicCollectionAdd {
                        dst,
                        count: *count,
                        item: *item,
                    })
                }
                ("collectionadd", [count, item, _key]) => {
                    instructions.push(Instruction::IntrinsicCollectionAdd {
                        dst,
                        count: *count,
                        item: *item,
                    })
                }
                ("collectionitem", [count, index]) => {
                    instructions.push(Instruction::IntrinsicCollectionItem {
                        dst,
                        count: *count,
                        index: *index,
                    })
                }
                ("collectionitem", [count, index, _missing]) => {
                    instructions.push(Instruction::IntrinsicCollectionItem {
                        dst,
                        count: *count,
                        index: *index,
                    })
                }
                ("collectionremove", [count, index]) => {
                    instructions.push(Instruction::IntrinsicCollectionRemove {
                        dst,
                        count: *count,
                        index: *index,
                    })
                }
                ("collectionremove", [count, index, _missing]) => {
                    instructions.push(Instruction::IntrinsicCollectionRemove {
                        dst,
                        count: *count,
                        index: *index,
                    })
                }
                ("collectioncount", [count]) => {
                    instructions.push(Instruction::IntrinsicCollectionCount { dst, count: *count })
                }
                ("createobject", [prog_id]) => {
                    instructions.push(Instruction::IntrinsicCreateObjectHost {
                        dst,
                        prog_id: *prog_id,
                    })
                }
                ("dispatchinvoke" | "__oxvbaearlyinvoke", [object, member, args @ ..]) => {
                    instructions.push(Instruction::IntrinsicDispatchInvokeHost {
                        dst,
                        object: *object,
                        member: *member,
                        args: args
                            .iter()
                            .map(|slot| DispatchInvokeArg {
                                slot: Some(*slot),
                                name: None,
                            })
                            .collect(),
                        early_bound: name.eq_ignore_ascii_case("__OxVbaEarlyInvoke"),
                        com_member: None,
                    })
                }
                ("__oxvba_com_subscribe_event", [object, event]) => {
                    instructions.push(Instruction::IntrinsicComSubscribeEventHost {
                        dst,
                        object: *object,
                        event: *event,
                    })
                }
                ("__oxvba_com_unsubscribe_event", [subscription]) => {
                    instructions.push(Instruction::IntrinsicComUnsubscribeEventHost {
                        dst,
                        subscription: *subscription,
                    })
                }
                ("__oxvba_com_callback_subscription", [callback]) => {
                    instructions.push(Instruction::IntrinsicComEventCallbackSubscriptionHost {
                        dst,
                        callback: *callback,
                    })
                }
                ("__oxvba_com_callback_arg", [callback, index]) => {
                    instructions.push(Instruction::IntrinsicComEventCallbackArgHost {
                        dst,
                        callback: *callback,
                        index: *index,
                    })
                }
                ("__oxvba_com_release_callback", [callback]) => {
                    instructions.push(Instruction::IntrinsicComReleaseEventCallbackHost {
                        dst,
                        callback: *callback,
                    })
                }
                ("__oxvba_withevents_get", [owner, binding]) => {
                    instructions.push(Instruction::IntrinsicWithEventsGet {
                        dst,
                        owner: *owner,
                        binding: *binding,
                    })
                }
                ("__oxvba_withevents_set", [owner, binding, value]) => {
                    instructions.push(Instruction::IntrinsicWithEventsSet {
                        dst,
                        owner: *owner,
                        binding: *binding,
                        value: *value,
                    })
                }
                ("__oxvba_withevents_clear_owner", [owner]) => instructions
                    .push(Instruction::IntrinsicWithEventsClearOwner { dst, owner: *owner }),
                ("__oxvba_withevents_first_owner", [source, binding]) => {
                    instructions.push(Instruction::IntrinsicWithEventsFirstOwner {
                        dst,
                        source: *source,
                        binding: *binding,
                    })
                }
                ("__oxvba_withevents_next_owner", []) => {
                    instructions.push(Instruction::IntrinsicWithEventsNextOwner { dst })
                }
                _ => {}
            }
        }
        BoundExpr::ProcCall { name, args } => {
            if !emit_early_call(
                name,
                args,
                CallInvocationSyntaxDescriptor::ExpressionCall,
                compare_mode,
                slot_map,
                temps,
                instructions,
                call_patches,
                proc_meta,
                external_decls,
                Some(dst),
                None,
                None,
            ) {
                let _ = emit_late_bound_default_member_call(
                    name,
                    args,
                    CallInvocationSyntaxDescriptor::ExpressionCall,
                    compare_mode,
                    slot_map,
                    temps,
                    instructions,
                    call_patches,
                    proc_meta,
                    external_decls,
                    Some(dst),
                    None,
                    None,
                );
            }
        }
    }
}

fn emit_err_member_value(name: &str, dst: usize, instructions: &mut Vec<Instruction>) -> bool {
    match name.to_ascii_lowercase().as_str() {
        "err_number" => {
            instructions.push(Instruction::LoadErrNumber { slot: dst });
            true
        }
        "err_description" => {
            instructions.push(Instruction::LoadErrDescription { slot: dst });
            true
        }
        "err_source" => {
            instructions.push(Instruction::LoadErrSource { slot: dst });
            true
        }
        "err_helpfile" => {
            instructions.push(Instruction::LoadConstString {
                slot: dst,
                value: String::new(),
            });
            true
        }
        "err_helpcontext" | "err_lastdllerror" => {
            instructions.push(Instruction::LoadConstI32 {
                slot: dst,
                value: 0,
            });
            true
        }
        _ => false,
    }
}

fn emit_optional_default(param: &BoundParam, dst: usize, instructions: &mut Vec<Instruction>) {
    instructions.push(Instruction::LoadConstI32 {
        slot: dst,
        value: param.default_value.unwrap_or(0),
    });
}

fn map_call_args_for_emit<'a>(
    args: &'a [BoundCallArg],
    params: &[BoundParam],
) -> EmitCallArgMapping<'a> {
    let param_array_idx = params.iter().position(|p| p.param_array);
    let fixed_len = param_array_idx.unwrap_or(params.len());
    let mut mapped: Vec<Option<MappedEmitCallArg<'a>>> = vec![None; params.len()];
    let mut extras: Vec<MappedEmitCallArg<'a>> = Vec::new();
    let mut next_pos = 0usize;
    let mut seen_named = false;

    for (source_index, arg) in args.iter().enumerate() {
        let mapped_arg = MappedEmitCallArg { source_index, arg };
        if let Some(name) = &arg.name {
            seen_named = true;
            if params
                .iter()
                .position(|p| p.name.eq_ignore_ascii_case(name))
                .is_some_and(|idx| params[idx].param_array)
            {
                continue;
            }
            if let Some(idx) = params
                .iter()
                .position(|p| p.name.eq_ignore_ascii_case(name))
                && mapped[idx].is_none()
            {
                mapped[idx] = Some(mapped_arg);
            }
            continue;
        }

        if seen_named {
            if param_array_idx.is_some() {
                extras.push(mapped_arg);
            }
            continue;
        }

        while next_pos < params.len() && mapped[next_pos].is_some() {
            next_pos += 1;
        }
        if next_pos < fixed_len {
            mapped[next_pos] = Some(mapped_arg);
            next_pos += 1;
        } else if param_array_idx.is_some() {
            extras.push(mapped_arg);
        }
    }

    EmitCallArgMapping {
        fixed: mapped,
        extras,
    }
}

#[derive(Clone, Copy)]
struct MappedEmitCallArg<'a> {
    source_index: usize,
    arg: &'a BoundCallArg,
}

struct EmitCallArgMapping<'a> {
    fixed: Vec<Option<MappedEmitCallArg<'a>>>,
    extras: Vec<MappedEmitCallArg<'a>>,
}

fn reset_array_slots(
    array_name: &str,
    slot_map: &HashMap<String, usize>,
    instructions: &mut Vec<Instruction>,
) {
    let prefix = format!("{array_name}_");
    let mut slots = slot_map
        .iter()
        .filter_map(|(name, slot)| {
            if name.starts_with(&prefix) {
                Some(*slot)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    slots.sort_unstable();
    for slot in slots {
        instructions.push(Instruction::LoadEmpty { slot });
    }
}

fn reset_array_slots_range(
    array_name: &str,
    start_index: usize,
    end_index_exclusive: usize,
    slot_map: &HashMap<String, usize>,
    instructions: &mut Vec<Instruction>,
) {
    if start_index >= end_index_exclusive {
        return;
    }
    let prefix = format!("{array_name}_");
    let mut slots = slot_map
        .iter()
        .filter_map(|(name, slot)| {
            if !name.starts_with(&prefix) {
                return None;
            }
            let suffix = &name[prefix.len()..];
            let index = suffix.parse::<usize>().ok()?;
            if (start_index..end_index_exclusive).contains(&index) {
                Some(*slot)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    slots.sort_unstable();
    for slot in slots {
        instructions.push(Instruction::LoadEmpty { slot });
    }
}

fn collect_array_element_slots(array_name: &str, slot_map: &HashMap<String, usize>) -> Vec<usize> {
    let prefix = format!("{array_name}_");
    let mut indexed = slot_map
        .iter()
        .filter_map(|(name, slot)| {
            if !name.starts_with(&prefix) {
                return None;
            }
            let suffix = &name[prefix.len()..];
            let index = suffix.parse::<usize>().ok()?;
            Some((index, *slot))
        })
        .collect::<Vec<_>>();
    indexed.sort_unstable_by_key(|(index, _)| *index);
    indexed.into_iter().map(|(_, slot)| slot).collect()
}

fn array_element_count(bounds: &[(i32, i32)]) -> Option<usize> {
    let mut total = 1usize;
    for (lower, upper) in bounds {
        if upper < lower {
            return None;
        }
        let width = (*upper as i64 - *lower as i64 + 1) as usize;
        total = total.checked_mul(width)?;
    }
    Some(total)
}

#[derive(Debug, Clone)]
struct TempSlotAllocator {
    declared_count: usize,
    next_temp: usize,
}

impl TempSlotAllocator {
    fn new(declared_count: usize) -> Self {
        Self {
            declared_count,
            next_temp: declared_count,
        }
    }

    fn next_temp_slot(&self) -> usize {
        self.next_temp
    }

    fn alloc_temp(&mut self) -> usize {
        let slot = self.next_temp;
        self.next_temp += 1;
        slot
    }

    fn slots_allocated_since(&self, start: usize) -> Vec<usize> {
        (start..self.next_temp).collect()
    }

    fn total_slots(&self) -> usize {
        self.next_temp.max(self.declared_count)
    }
}

#[cfg(test)]
mod tests {
    use super::{Instruction, TempSlotAllocator, emit_bytecode};
    use crate::resolve::resolve_symbols;

    #[test]
    fn temp_slot_allocator_starts_after_declarations() {
        let mut alloc = TempSlotAllocator::new(2);
        let a = alloc.alloc_temp();
        let b = alloc.alloc_temp();
        assert_eq!(a, 2);
        assert_eq!(b, 3);
        assert_eq!(alloc.total_slots(), 4);
    }

    #[test]
    fn emits_if_and_for_control_flow() {
        let source = "Sub Main()\nDim x\nDim i\nx = 0\nIf x = 0 Then\nx = 5\nEnd If\nFor i = 1 To 2\nx = x + 1\nNext i\nEnd Sub";
        let bound = resolve_symbols(source);
        let code = emit_bytecode(&bound);
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Jump { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpEqSlots { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CmpLeSlots { .. }))
        );
    }

    #[test]
    fn emits_runtime_foreach_for_non_array_iterables() {
        let source =
            "Sub Main()\nDim item\nDim widget\nFor Each item In widget\nitem = item\nNext\nEnd Sub";
        let bound = resolve_symbols(source);
        let code = emit_bytecode(&bound);
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicForEachInit { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::IntrinsicForEachNext { .. }))
        );
    }

    #[test]
    fn emits_do_while_loop_and_exit_do() {
        let source = "Sub Main()\nDim x\nDo While x < 5\nx = x + 1\nIf x = 3 Then\nExit Do\nEnd If\nLoop\nEnd Sub";
        let bound = resolve_symbols(source);
        let code = emit_bytecode(&bound);
        let jump_count = code
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::Jump { .. }))
            .count();
        let jump_if_count = code
            .instructions
            .iter()
            .filter(|i| matches!(i, Instruction::JumpIfZero { .. }))
            .count();
        assert!(jump_count >= 2);
        assert!(jump_if_count >= 2);
    }

    #[test]
    fn emits_select_case_dispatch_jumps() {
        let source = "Sub Main()\nDim x\nSelect Case x\nCase 1\nx = 10\nCase 2, 3\nx = 20\nCase Else\nx = 30\nEnd Select\nEnd Sub";
        let bound = resolve_symbols(source);
        let code = emit_bytecode(&bound);
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::BoolOr { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::JumpIfZero { .. }))
        );
    }

    #[test]
    fn emits_callproc_and_return_for_named_sub() {
        let source =
            "Sub Main()\nDim x\nx = 1\nCall Foo\nEnd Sub\nSub Foo()\nDim y\ny = 2\nEnd Sub";
        let bound = resolve_symbols(source);
        let code = emit_bytecode(&bound);
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::CallProc { .. }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::Return))
        );
    }

    #[test]
    fn emits_on_error_resume_next_and_raise_ops() {
        let source = "Sub Main()\nDim x\nOn Error Resume Next\nError 5\nx = Err.Number\nEnd Sub";
        let bound = resolve_symbols(source);
        let code = emit_bytecode(&bound);
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::SetOnErrorResumeNext))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::RaiseError { code: 5 }))
        );
        assert!(
            code.instructions
                .iter()
                .any(|i| matches!(i, Instruction::LoadErrNumber { .. }))
        );
    }
}

#[allow(unexpected_cfgs)]
#[cfg(kani)]
mod kani_proofs {
    use super::TempSlotAllocator;

    #[kani::proof]
    fn temp_slots_do_not_overlap_declared_slots() {
        let declared: usize = kani::any();
        kani::assume(declared < 1024);
        let mut alloc = TempSlotAllocator::new(declared);
        let a = alloc.alloc_temp();
        let b = alloc.alloc_temp();
        assert!(a >= declared);
        assert!(b >= declared);
        assert!(b > a);
    }
}
