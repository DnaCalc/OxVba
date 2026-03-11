pub const DISPATCH_INVOKE_MISSING_ARG_TOKEN: i32 = i32::MIN + 2_048;

macro_rules! define_token {
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

        impl From<$name> for i32 {
            fn from(value: $name) -> Self {
                value.raw()
            }
        }
    };
}

define_token!(ComObjectToken);
define_token!(ComSubscriptionToken);
define_token!(ComCallbackToken);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComObjectTransportKind {
    Projection,
    NativeDispatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComObjectDescriptor {
    pub object: ComObjectToken,
    pub prog_id_name: String,
    pub transport: ComObjectTransportKind,
    pub supports_events: bool,
    pub known_member_tokens: Vec<i32>,
    pub known_event_tokens: Vec<i32>,
    pub default_member_token: Option<i32>,
    pub default_member_name: Option<String>,
    pub typelib_cache_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComInvokeKind {
    Method,
    PropertyGet,
    PropertyPut,
    PropertyPutRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComInvokeArg {
    pub value: Option<i32>,
    pub name: Option<String>,
}

impl ComInvokeArg {
    pub fn positional(value: i32) -> Self {
        Self {
            value: Some(value),
            name: None,
        }
    }

    pub fn named(value: i32, name: impl Into<String>) -> Self {
        Self {
            value: Some(value),
            name: Some(name.into()),
        }
    }

    pub fn omitted() -> Self {
        Self {
            value: None,
            name: None,
        }
    }

    pub fn omitted_named(name: impl Into<String>) -> Self {
        Self {
            value: None,
            name: Some(name.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComInvokeRequest {
    pub object: ComObjectToken,
    pub member: i32,
    pub args: Vec<ComInvokeArg>,
    pub invoke_kind_hint: Option<ComInvokeKind>,
}

impl ComInvokeRequest {
    pub fn new(object: ComObjectToken, member: i32, args: Vec<ComInvokeArg>) -> Self {
        Self {
            object,
            member,
            args,
            invoke_kind_hint: None,
        }
    }

    pub fn legacy(object: i32, member: i32, arg: i32) -> Self {
        let args = if arg == DISPATCH_INVOKE_MISSING_ARG_TOKEN {
            Vec::new()
        } else {
            vec![ComInvokeArg::positional(arg)]
        };
        Self::new(ComObjectToken::new(object), member, args)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComCallbackPayload {
    pub callback: ComCallbackToken,
    pub subscription: ComSubscriptionToken,
    pub object: ComObjectToken,
    pub event: i32,
    pub args: Vec<i32>,
}
