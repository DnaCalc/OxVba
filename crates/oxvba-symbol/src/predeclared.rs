//! The predeclared base-library objects `Debug`, `Err`, and `Collection`,
//! modeled as static data behind the same member-resolution path as real COM
//! objects (no fake typelib blob authored).

use oxvba_bundle::NativeImplId;

use crate::binding::{DispatchRoute, ErrMember};
use crate::model::PredeclaredObjectId;

/// Resolve the predeclared object named `name` (case-insensitive).
pub fn predeclared_object(name: &str) -> Option<PredeclaredObjectId> {
    Some(match name.to_ascii_lowercase().as_str() {
        "debug" => PredeclaredObjectId::Debug,
        "err" => PredeclaredObjectId::Err,
        "collection" => PredeclaredObjectId::Collection,
        _ => return None,
    })
}

/// Resolve a member of a predeclared object to its dispatch route.
pub fn predeclared_member(obj: PredeclaredObjectId, name: &str) -> Option<DispatchRoute> {
    let folded = name.to_ascii_lowercase();
    Some(match obj {
        PredeclaredObjectId::Debug => match folded.as_str() {
            // `Debug.Assert` shares the diagnostics body; the binder refines it.
            "print" | "assert" => DispatchRoute::Native(NativeImplId::DebugPrint),
            _ => return None,
        },
        PredeclaredObjectId::Err => DispatchRoute::ErrMember(match folded.as_str() {
            "raise" => ErrMember::Raise,
            "number" => ErrMember::Number,
            "description" => ErrMember::Description,
            "source" => ErrMember::Source,
            "clear" => ErrMember::Clear,
            "helpfile" => ErrMember::HelpFile,
            "helpcontext" => ErrMember::HelpContext,
            "lastdllerror" => ErrMember::LastDllError,
            _ => return None,
        }),
        PredeclaredObjectId::Collection => match folded.as_str() {
            "add" => DispatchRoute::Native(NativeImplId::CollectionAdd),
            "item" => DispatchRoute::Native(NativeImplId::CollectionItem),
            "remove" => DispatchRoute::Native(NativeImplId::CollectionRemove),
            "count" => DispatchRoute::Native(NativeImplId::CollectionCount),
            _ => return None,
        },
    })
}
