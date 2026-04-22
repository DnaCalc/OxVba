use crate::{
    ComCallbackPayload, ComCallbackToken, ComInvokeArg, ComInvokeKind, ComInvokeRequest,
    ComMemberToken, ComSubscriptionToken, ComValue,
};
use oxvba_runtime::ObjectRef;

macro_rules! define_dynamic_token {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(i32);

        impl $name {
            pub const fn new(raw: i32) -> Self {
                Self(raw)
            }

            pub const fn raw(self) -> i32 {
                self.0
            }
        }

        impl From<i32> for $name {
            fn from(value: i32) -> Self {
                Self::new(value)
            }
        }
    };
}

define_dynamic_token!(DynamicObjectToken);
define_dynamic_token!(DynamicSubscriptionToken);
define_dynamic_token!(DynamicCallbackToken);

impl From<ObjectRef> for DynamicObjectToken {
    fn from(value: ObjectRef) -> Self {
        Self::new(value.raw())
    }
}

impl From<DynamicObjectToken> for ObjectRef {
    fn from(value: DynamicObjectToken) -> Self {
        Self::from_compat_identity(value.raw())
    }
}

impl From<ComSubscriptionToken> for DynamicSubscriptionToken {
    fn from(value: ComSubscriptionToken) -> Self {
        Self::new(value.raw())
    }
}

impl From<DynamicSubscriptionToken> for ComSubscriptionToken {
    fn from(value: DynamicSubscriptionToken) -> Self {
        Self::new(value.raw())
    }
}

impl From<ComCallbackToken> for DynamicCallbackToken {
    fn from(value: ComCallbackToken) -> Self {
        Self::new(value.raw())
    }
}

impl From<DynamicCallbackToken> for ComCallbackToken {
    fn from(value: DynamicCallbackToken) -> Self {
        Self::new(value.raw())
    }
}

pub type DynamicValue = ComValue;

pub trait DynamicObjectBridge {
    type Error;

    fn invoke_dynamic(&self, request: &DynamicCallRequest) -> Result<DynamicValue, Self::Error>;

    fn poll_dynamic_event(&self) -> Result<Option<DynamicEventPayload>, Self::Error>;

    fn release_dynamic_object(
        &self,
        object: DynamicObjectToken,
    ) -> Result<DynamicValue, Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicMemberSelector {
    Token(i32),
    Name(String),
    DefaultMember,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicCallKind {
    Method,
    PropertyGet,
    PropertyLet,
    PropertySet,
}

impl From<ComInvokeKind> for DynamicCallKind {
    fn from(value: ComInvokeKind) -> Self {
        match value {
            ComInvokeKind::Method => Self::Method,
            ComInvokeKind::PropertyGet => Self::PropertyGet,
            ComInvokeKind::PropertyPut => Self::PropertyLet,
            ComInvokeKind::PropertyPutRef => Self::PropertySet,
        }
    }
}

impl From<DynamicCallKind> for ComInvokeKind {
    fn from(value: DynamicCallKind) -> Self {
        match value {
            DynamicCallKind::Method => Self::Method,
            DynamicCallKind::PropertyGet => Self::PropertyGet,
            DynamicCallKind::PropertyLet => Self::PropertyPut,
            DynamicCallKind::PropertySet => Self::PropertyPutRef,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicCallArg {
    pub value: Option<DynamicValue>,
    pub name: Option<String>,
}

impl From<ComInvokeArg> for DynamicCallArg {
    fn from(value: ComInvokeArg) -> Self {
        Self {
            value: value.value,
            name: value.name,
        }
    }
}

impl From<DynamicCallArg> for ComInvokeArg {
    fn from(value: DynamicCallArg) -> Self {
        Self {
            value: value.value,
            name: value.name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicCallRequest {
    pub object: ObjectRef,
    pub member: DynamicMemberSelector,
    pub args: Vec<DynamicCallArg>,
    pub call_kind_hint: Option<DynamicCallKind>,
}

impl From<&ComInvokeRequest> for DynamicCallRequest {
    fn from(value: &ComInvokeRequest) -> Self {
        Self {
            object: value.object.clone(),
            member: if value.member.raw() == 0 {
                DynamicMemberSelector::DefaultMember
            } else {
                DynamicMemberSelector::Token(value.member.raw())
            },
            args: value.args.clone().into_iter().map(Into::into).collect(),
            call_kind_hint: value.invoke_kind_hint.map(Into::into),
        }
    }
}

impl DynamicCallRequest {
    pub fn try_into_com_invoke_request(&self) -> Result<ComInvokeRequest, String> {
        let member = match &self.member {
            DynamicMemberSelector::Token(value) => *value,
            DynamicMemberSelector::DefaultMember => 0,
            DynamicMemberSelector::Name(name) => {
                return Err(format!(
                    "dynamic member name `{name}` requires authoritative name resolution before COM lowering"
                ));
            }
        };
        Ok(ComInvokeRequest {
            object: self.object.clone(),
            member: ComMemberToken::new(member),
            args: self.args.clone().into_iter().map(Into::into).collect(),
            invoke_kind_hint: self.call_kind_hint.map(Into::into),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicEventPayload {
    pub callback: DynamicCallbackToken,
    pub subscription: DynamicSubscriptionToken,
    pub object: ObjectRef,
    pub event: ComMemberToken,
    pub args: Vec<DynamicValue>,
}

impl From<ComCallbackPayload> for DynamicEventPayload {
    fn from(value: ComCallbackPayload) -> Self {
        Self {
            callback: value.callback.into(),
            subscription: value.subscription.into(),
            object: value.object,
            event: value.event,
            args: value.args,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{ComInvokeArg, ComInvokeKind, ComInvokeRequest, ComValue};
    use oxvba_runtime::ObjectRef;

    use super::{DynamicCallKind, DynamicCallRequest, DynamicMemberSelector};

    #[test]
    fn com_invoke_request_converts_to_dynamic_default_member_shape() {
        let request = ComInvokeRequest {
            object: ObjectRef::from_compat_identity(20_004),
            member: 0.into(),
            args: vec![ComInvokeArg::named_value(ComValue::I32(7), "value")],
            invoke_kind_hint: Some(ComInvokeKind::PropertyPut),
        };

        let dynamic = DynamicCallRequest::from(&request);
        assert_eq!(dynamic.object.raw(), 20_004);
        assert_eq!(dynamic.member, DynamicMemberSelector::DefaultMember);
        assert_eq!(dynamic.call_kind_hint, Some(DynamicCallKind::PropertyLet));
    }

    #[test]
    fn dynamic_call_request_roundtrips_back_to_com_when_member_token_is_known() {
        let request = DynamicCallRequest {
            object: ObjectRef::from_compat_identity(20_007),
            member: DynamicMemberSelector::Token(11),
            args: vec![super::DynamicCallArg {
                value: Some(ComValue::Null),
                name: Some("value".to_string()),
            }],
            call_kind_hint: Some(DynamicCallKind::PropertySet),
        };

        let com_request = request
            .try_into_com_invoke_request()
            .expect("token-backed dynamic request should lower");
        assert_eq!(com_request.object.raw(), 20_007);
        assert_eq!(com_request.member.raw(), 11);
        assert_eq!(
            com_request.invoke_kind_hint,
            Some(ComInvokeKind::PropertyPutRef)
        );
        assert_eq!(
            com_request.args[0],
            ComInvokeArg::named_value(ComValue::Null, "value")
        );
    }

    #[test]
    fn dynamic_call_request_name_selector_requires_authoritative_resolution() {
        let request = DynamicCallRequest {
            object: ObjectRef::from_compat_identity(20_010),
            member: DynamicMemberSelector::Name("Range".to_string()),
            args: vec![],
            call_kind_hint: Some(DynamicCallKind::PropertyGet),
        };

        let err = request
            .try_into_com_invoke_request()
            .expect_err("name-backed dynamic request should not lower without resolution");
        assert!(err.contains("requires authoritative name resolution"));
    }
}
