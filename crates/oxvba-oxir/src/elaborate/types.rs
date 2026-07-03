//! Lowering the binder's static type lattice (`VarTypeRef`) into OxIR's [`OxTy`].
//!
//! The scalar / string / array cases map directly; object and UDT cases are
//! *name-based* and resolved through a [`NameResolver`] the elaboration pass supplies
//! (it owns the project-class, interface, and record-layout tables the name indexes
//! into). Resolution is **name-driven, not tag-driven**: a `VarTypeRef::Object(name)`
//! that actually denotes a UDT or an `Enum` resolves correctly regardless of the tag,
//! so it does not matter that the binder records some declared types in raw
//! (un-normalized) form (the 3-B note).

use oxvba_bundle::coreir::CoreLongPtrWidth;
use oxvba_bundle::{BuiltinType, VarTypeRef};

use crate::ty::{ArrayShape, ClassId, IfaceId, ObjClass, OxTy, RecordLayoutId};

/// What a declared type *name* (folded) resolves to. The elaboration pass classifies
/// each `Object`/`Udt` name against the tables it has built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedTypeName {
    /// A project class instance.
    Class(ClassId),
    /// A project `Implements` interface.
    ProjectIface(IfaceId),
    /// A referenced COM interface.
    ComIface(IfaceId),
    /// A user-defined `Type` (UDT) record.
    Record(RecordLayoutId),
    /// An `Enum` type — a `Long` at runtime.
    Enum,
    /// A bare `Object` / `IDispatch`, or an unresolvable name → an untyped
    /// (late-bound) object reference ([`ObjClass::Untyped`]).
    Untyped,
}

/// Resolves a declared type *name* (folded) to its classified identity. The
/// elaboration pass implements this over the project class / interface / record
/// tables it builds; tests use a small fixed map.
pub trait NameResolver {
    fn resolve_type_name(&self, name: &str) -> ResolvedTypeName;
}

/// Lower a [`VarTypeRef`] to an [`OxTy`] with an explicit `LongPtr` target width.
pub fn lower_var_type_with_longptr_width(
    ty: &VarTypeRef,
    r: &impl NameResolver,
    long_ptr_width: CoreLongPtrWidth,
) -> OxTy {
    match ty {
        VarTypeRef::Builtin(b) => lower_builtin(*b, long_ptr_width),
        VarTypeRef::FixedString(n) => OxTy::FixedStr(*n),
        VarTypeRef::Variant => OxTy::Variant,
        VarTypeRef::Array(elem) => {
            // `VarTypeRef` carries no rank/bounds, so the shape is the conservative
            // dynamic (ReDim-able) form; a fixed-rank refinement needs the
            // declaration's bounds, recovered later from the binding's array metadata.
            OxTy::Array(
                Box::new(lower_var_type_with_longptr_width(elem, r, long_ptr_width)),
                ArrayShape::Dynamic,
            )
        }
        // A UDT fixed-array field is **inline record payload**, not a SAFEARRAY, so it
        // is *not* `OxTy::Array` (a `*mut SAFEARRAY`). Conservatively `Variant` for now —
        // consistent with the (also-deferred) record-layout typing; a precise
        // inline-fixed-array `OxTy` rides with that record-layout work.
        VarTypeRef::FixedArray { .. } => OxTy::Variant,
        // Name-driven (tag-agnostic): an `Object(name)` may denote a class, a COM or
        // project interface, a UDT, or an Enum — `Udt(name)` is the already-normalized
        // form of the same.
        VarTypeRef::Object(name) | VarTypeRef::Udt(name) => lower_named(name, r),
    }
}

fn lower_builtin(b: BuiltinType, long_ptr_width: CoreLongPtrWidth) -> OxTy {
    match b {
        BuiltinType::Boolean => OxTy::Bool,
        BuiltinType::Byte => OxTy::Byte,
        BuiltinType::Integer => OxTy::Integer,
        BuiltinType::Long => OxTy::Long,
        BuiltinType::LongLong => OxTy::LongLong,
        BuiltinType::LongPtr => match long_ptr_width {
            CoreLongPtrWidth::Bits32 => OxTy::Long,
            CoreLongPtrWidth::Bits64 => OxTy::LongLong,
        },
        BuiltinType::Single => OxTy::Single,
        BuiltinType::Double => OxTy::Double,
        BuiltinType::Currency => OxTy::Currency,
        BuiltinType::Date => OxTy::Date,
        BuiltinType::String => OxTy::Str,
    }
}

fn lower_named(name: &str, r: &impl NameResolver) -> OxTy {
    match r.resolve_type_name(name) {
        ResolvedTypeName::Class(id) => OxTy::Object(ObjClass::Class(id)),
        ResolvedTypeName::ProjectIface(id) => OxTy::Object(ObjClass::Iface(id)),
        ResolvedTypeName::ComIface(id) => OxTy::Object(ObjClass::ComIface(id)),
        ResolvedTypeName::Record(id) => OxTy::Record(id),
        ResolvedTypeName::Enum => OxTy::Long,
        ResolvedTypeName::Untyped => OxTy::Object(ObjClass::Untyped),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A fixed-map resolver for tests: any name not in the map resolves to `Object`.
    struct MapResolver(HashMap<&'static str, ResolvedTypeName>);

    impl NameResolver for MapResolver {
        fn resolve_type_name(&self, name: &str) -> ResolvedTypeName {
            self.0
                .get(name)
                .copied()
                .unwrap_or(ResolvedTypeName::Untyped)
        }
    }

    fn resolver() -> MapResolver {
        MapResolver(HashMap::from([
            ("widget", ResolvedTypeName::Class(ClassId(2))),
            ("ishape", ResolvedTypeName::ProjectIface(IfaceId(1))),
            ("excel.range", ResolvedTypeName::ComIface(IfaceId(5))),
            ("point", ResolvedTypeName::Record(RecordLayoutId(3))),
            ("color", ResolvedTypeName::Enum),
        ]))
    }

    fn lower_var_type_win64(ty: &VarTypeRef, r: &impl NameResolver) -> OxTy {
        lower_var_type_with_longptr_width(ty, r, CoreLongPtrWidth::Bits64)
    }

    #[test]
    fn scalars_and_strings_map_directly() {
        let r = resolver();
        assert_eq!(
            lower_var_type_win64(&VarTypeRef::Builtin(BuiltinType::Long), &r),
            OxTy::Long
        );
        assert_eq!(
            lower_var_type_win64(&VarTypeRef::Builtin(BuiltinType::Boolean), &r),
            OxTy::Bool
        );
        assert_eq!(
            lower_var_type_win64(&VarTypeRef::Builtin(BuiltinType::String), &r),
            OxTy::Str
        );
        assert_eq!(
            lower_var_type_win64(&VarTypeRef::FixedString(16), &r),
            OxTy::FixedStr(16)
        );
        assert_eq!(
            lower_var_type_win64(&VarTypeRef::Variant, &r),
            OxTy::Variant
        );
    }

    #[test]
    fn longptr_lowers_to_target_width_integer() {
        let r = resolver();
        let ty = VarTypeRef::Builtin(BuiltinType::LongPtr);
        assert_eq!(
            lower_var_type_with_longptr_width(&ty, &r, CoreLongPtrWidth::Bits32),
            OxTy::Long
        );
        assert_eq!(
            lower_var_type_with_longptr_width(&ty, &r, CoreLongPtrWidth::Bits64),
            OxTy::LongLong
        );
    }

    #[test]
    fn objects_resolve_by_name() {
        let r = resolver();
        assert_eq!(
            lower_var_type_win64(&VarTypeRef::Object("widget".into()), &r),
            OxTy::Object(ObjClass::Class(ClassId(2)))
        );
        assert_eq!(
            lower_var_type_win64(&VarTypeRef::Object("ishape".into()), &r),
            OxTy::Object(ObjClass::Iface(IfaceId(1)))
        );
        assert_eq!(
            lower_var_type_win64(&VarTypeRef::Object("excel.range".into()), &r),
            OxTy::Object(ObjClass::ComIface(IfaceId(5)))
        );
        // An unresolvable / bare object name → untyped (late-bound).
        assert_eq!(
            lower_var_type_win64(&VarTypeRef::Object("object".into()), &r),
            OxTy::Object(ObjClass::Untyped)
        );
    }

    #[test]
    fn udt_and_enum_resolve_by_name_regardless_of_tag() {
        let r = resolver();
        // A UDT named via either tag lowers to the same record type.
        assert_eq!(
            lower_var_type_win64(&VarTypeRef::Udt("point".into()), &r),
            OxTy::Record(RecordLayoutId(3))
        );
        assert_eq!(
            lower_var_type_win64(&VarTypeRef::Object("point".into()), &r),
            OxTy::Record(RecordLayoutId(3))
        );
        // An Enum lowers to Long whether tagged Object or already a Builtin.
        assert_eq!(
            lower_var_type_win64(&VarTypeRef::Object("color".into()), &r),
            OxTy::Long
        );
    }

    #[test]
    fn arrays_lower_elementwise_to_dynamic_shape() {
        let r = resolver();
        let ty = VarTypeRef::Array(Box::new(VarTypeRef::Builtin(BuiltinType::Double)));
        assert_eq!(
            lower_var_type_win64(&ty, &r),
            OxTy::Array(Box::new(OxTy::Double), ArrayShape::Dynamic)
        );
        // Array of a typed object.
        let ty = VarTypeRef::Array(Box::new(VarTypeRef::Object("widget".into())));
        assert_eq!(
            lower_var_type_win64(&ty, &r),
            OxTy::Array(
                Box::new(OxTy::Object(ObjClass::Class(ClassId(2)))),
                ArrayShape::Dynamic
            )
        );
    }

    #[test]
    fn udt_fixed_array_field_is_conservatively_variant() {
        // A UDT inline fixed-array field is record payload, not a SAFEARRAY, so it is
        // *not* `OxTy::Array`; conservatively `Variant` until record-layout typing lands.
        let r = resolver();
        let ty = VarTypeRef::FixedArray {
            element: Box::new(VarTypeRef::Builtin(BuiltinType::Long)),
            bounds: vec![oxvba_bundle::FixedArrayBound { lower: 0, len: 4 }],
        };
        assert_eq!(lower_var_type_win64(&ty, &r), OxTy::Variant);
    }
}
