use crate::{
    ObjectRef, RuntimeInterfaceIdentity, RuntimeMemberDescriptor, RuntimeMemberInvokeKind,
    RuntimeValueType, Variant,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCallKind {
    Method,
    PropertyGet,
    PropertyLet,
    PropertySet,
    Event,
    HostCall,
}

impl From<RuntimeMemberInvokeKind> for RuntimeCallKind {
    fn from(value: RuntimeMemberInvokeKind) -> Self {
        match value {
            RuntimeMemberInvokeKind::Method => Self::Method,
            RuntimeMemberInvokeKind::PropertyGet => Self::PropertyGet,
            RuntimeMemberInvokeKind::PropertyLet => Self::PropertyLet,
            RuntimeMemberInvokeKind::PropertySet => Self::PropertySet,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCallSelector {
    Descriptor {
        interface: RuntimeInterfaceIdentity,
        member: &'static RuntimeMemberDescriptor,
    },
    DispatchId {
        interface: Option<RuntimeInterfaceIdentity>,
        dispatch_id: i32,
    },
    Name {
        receiver_type: Option<String>,
        member_name: String,
    },
    VTableSlot {
        interface: RuntimeInterfaceIdentity,
        slot: u16,
    },
    HostCall {
        host_call_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeByRefSlot {
    pub id: u32,
    pub expected_type: Option<RuntimeValueType>,
}

impl RuntimeByRefSlot {
    pub const fn new(id: u32, expected_type: Option<RuntimeValueType>) -> Self {
        Self { id, expected_type }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCallArgument {
    pub value: Variant,
    pub by_ref: Option<RuntimeByRefSlot>,
}

impl RuntimeCallArgument {
    pub fn by_value(value: Variant) -> Self {
        Self {
            value,
            by_ref: None,
        }
    }

    pub fn by_ref(value: Variant, slot: RuntimeByRefSlot) -> Self {
        Self {
            value,
            by_ref: Some(slot),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeNamedArgument {
    pub name: String,
    pub argument: RuntimeCallArgument,
}

impl RuntimeNamedArgument {
    pub fn new(name: impl Into<String>, argument: RuntimeCallArgument) -> Self {
        Self {
            name: name.into(),
            argument,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCallSource {
    InternalProject,
    ExternalComDispatch,
    ExternalComVTable,
    EventSink,
    HostUdf,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCallContext {
    pub source: RuntimeCallSource,
    pub locale_id: Option<u32>,
    pub caller: Option<String>,
}

impl RuntimeCallContext {
    pub fn new(source: RuntimeCallSource) -> Self {
        Self {
            source,
            locale_id: None,
            caller: None,
        }
    }

    pub fn with_locale(mut self, locale_id: u32) -> Self {
        self.locale_id = Some(locale_id);
        self
    }

    pub fn with_caller(mut self, caller: impl Into<String>) -> Self {
        self.caller = Some(caller.into());
        self
    }
}

impl Default for RuntimeCallContext {
    fn default() -> Self {
        Self::new(RuntimeCallSource::Unknown)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCallFrame {
    pub receiver: Option<ObjectRef>,
    pub selector: RuntimeCallSelector,
    pub kind: RuntimeCallKind,
    pub positional_args: Vec<RuntimeCallArgument>,
    pub named_args: Vec<RuntimeNamedArgument>,
    pub property_put_arg: Option<RuntimeCallArgument>,
    pub context: RuntimeCallContext,
}

impl RuntimeCallFrame {
    pub fn new(selector: RuntimeCallSelector, kind: RuntimeCallKind) -> Self {
        Self {
            receiver: None,
            selector,
            kind,
            positional_args: Vec::new(),
            named_args: Vec::new(),
            property_put_arg: None,
            context: RuntimeCallContext::default(),
        }
    }

    pub fn with_receiver(mut self, receiver: ObjectRef) -> Self {
        self.receiver = Some(receiver);
        self
    }

    pub fn with_context(mut self, context: RuntimeCallContext) -> Self {
        self.context = context;
        self
    }

    pub fn push_positional_arg(&mut self, argument: RuntimeCallArgument) {
        self.positional_args.push(argument);
    }

    pub fn push_named_arg(&mut self, name: impl Into<String>, argument: RuntimeCallArgument) {
        self.named_args
            .push(RuntimeNamedArgument::new(name, argument));
    }

    pub fn set_property_put_arg(&mut self, argument: RuntimeCallArgument) {
        self.property_put_arg = Some(argument);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeByRefWriteback {
    pub slot: RuntimeByRefSlot,
    pub value: Variant,
}

impl RuntimeByRefWriteback {
    pub fn new(slot: RuntimeByRefSlot, value: Variant) -> Self {
        Self { slot, value }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCallResult {
    pub value: Option<Variant>,
    pub writebacks: Vec<RuntimeByRefWriteback>,
}

impl RuntimeCallResult {
    pub fn empty() -> Self {
        Self {
            value: None,
            writebacks: Vec::new(),
        }
    }

    pub fn value(value: Variant) -> Self {
        Self {
            value: Some(value),
            writebacks: Vec::new(),
        }
    }

    pub fn add_writeback(&mut self, writeback: RuntimeByRefWriteback) {
        self.writebacks.push(writeback);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCallError {
    pub code: i32,
    pub message: String,
    pub argument_index: Option<usize>,
    pub source: RuntimeCallSource,
}

impl RuntimeCallError {
    pub fn new(code: i32, message: impl Into<String>, source: RuntimeCallSource) -> Self {
        Self {
            code,
            message: message.into(),
            argument_index: None,
            source,
        }
    }

    pub fn with_argument_index(mut self, argument_index: usize) -> Self {
        self.argument_index = Some(argument_index);
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        RuntimeCallArgument, RuntimeCallContext, RuntimeCallError, RuntimeCallFrame,
        RuntimeCallKind, RuntimeCallSelector, RuntimeCallSource, RuntimeMemberInvokeKind, Variant,
        bstr::BStr,
    };

    use super::{RuntimeByRefSlot, RuntimeByRefWriteback, RuntimeCallResult};

    #[test]
    fn call_frame_preserves_positional_and_named_ordering() {
        let mut frame = RuntimeCallFrame::new(
            RuntimeCallSelector::Name {
                receiver_type: Some("Project.Widget".to_string()),
                member_name: "Mix".to_string(),
            },
            RuntimeCallKind::Method,
        );
        frame.push_positional_arg(RuntimeCallArgument::by_value(Variant::from_i32(1)));
        frame.push_positional_arg(RuntimeCallArgument::by_value(Variant::from_i32(2)));
        frame.push_named_arg(
            "suffix",
            RuntimeCallArgument::by_value(Variant::from_string(BStr::from("x"))),
        );
        frame.push_named_arg("scale", RuntimeCallArgument::by_value(Variant::from_i32(3)));

        assert_eq!(frame.positional_args[0].value.as_i32(), Some(1));
        assert_eq!(frame.positional_args[1].value.as_i32(), Some(2));
        assert_eq!(frame.named_args[0].name, "suffix");
        assert_eq!(frame.named_args[1].name, "scale");
    }

    #[test]
    fn call_frame_represents_property_get_let_and_set() {
        let get = RuntimeCallFrame::new(
            RuntimeCallSelector::DispatchId {
                interface: None,
                dispatch_id: 0,
            },
            RuntimeMemberInvokeKind::PropertyGet.into(),
        );
        assert_eq!(get.kind, RuntimeCallKind::PropertyGet);
        assert!(get.property_put_arg.is_none());

        let mut put = RuntimeCallFrame::new(
            RuntimeCallSelector::DispatchId {
                interface: None,
                dispatch_id: 0,
            },
            RuntimeMemberInvokeKind::PropertyLet.into(),
        );
        put.set_property_put_arg(RuntimeCallArgument::by_value(Variant::from_i32(42)));
        assert_eq!(put.kind, RuntimeCallKind::PropertyLet);
        assert_eq!(
            put.property_put_arg
                .as_ref()
                .and_then(|arg| arg.value.as_i32()),
            Some(42)
        );

        let set = RuntimeCallFrame::new(
            RuntimeCallSelector::Name {
                receiver_type: None,
                member_name: "Child".to_string(),
            },
            RuntimeCallKind::PropertySet,
        );
        assert_eq!(set.kind, RuntimeCallKind::PropertySet);
    }

    #[test]
    fn call_frame_tracks_byref_placeholders_and_writebacks() {
        let slot = RuntimeByRefSlot::new(7, Some(crate::RuntimeValueType::Long));
        let arg = RuntimeCallArgument::by_ref(Variant::from_i32(10), slot);
        assert_eq!(arg.by_ref, Some(slot));

        let mut result = RuntimeCallResult::value(Variant::from_i32(11));
        result.add_writeback(RuntimeByRefWriteback::new(slot, Variant::from_i32(12)));

        assert_eq!(result.value.as_ref().and_then(Variant::as_i32), Some(11));
        assert_eq!(result.writebacks.len(), 1);
        assert_eq!(result.writebacks[0].slot, slot);
        assert_eq!(result.writebacks[0].value.as_i32(), Some(12));
    }

    #[test]
    fn call_context_preserves_locale_source_and_caller() {
        let context = RuntimeCallContext::new(RuntimeCallSource::ExternalComDispatch)
            .with_locale(1033)
            .with_caller("Excel.Application");
        let frame = RuntimeCallFrame::new(
            RuntimeCallSelector::HostCall {
                host_call_id: "WorksheetFunction.Sum".to_string(),
            },
            RuntimeCallKind::HostCall,
        )
        .with_context(context.clone());

        assert_eq!(frame.context, context);
        assert_eq!(frame.context.locale_id, Some(1033));
        assert_eq!(frame.context.caller.as_deref(), Some("Excel.Application"));
    }

    #[test]
    fn call_result_and_error_mapping_are_explicit() {
        let empty = RuntimeCallResult::empty();
        assert!(empty.value.is_none());
        assert!(empty.writebacks.is_empty());

        let error = RuntimeCallError::new(
            438,
            "object does not support member",
            RuntimeCallSource::HostUdf,
        )
        .with_argument_index(1);
        assert_eq!(error.code, 438);
        assert_eq!(error.argument_index, Some(1));
        assert_eq!(error.source, RuntimeCallSource::HostUdf);
        assert_eq!(error.message, "object does not support member");
    }
}
