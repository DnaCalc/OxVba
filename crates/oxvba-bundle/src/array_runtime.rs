//! Pure `ArrayElementType` → runtime-value/layout helpers, shared by every executor.
//!
//! `ReDim`, array literals, and UDT construction need to (a) pick the SAFEARRAY element
//! vartype for a declared element type, (b) seed a freshly-`ReDim`-ed element with its typed
//! default, (c) build a [`SafeArray`] with the right element storage, and (d) project a UDT
//! field list into a native [`VbaRecordLayout`]. These are pure functions of
//! [`ArrayElementType`] (no VM state), so vm2, vm3, and the Cranelift backend share one
//! definition rather than mirroring it.

use std::sync::Arc;

use oxvba_runtime::Variant;
use oxvba_runtime::safe_array::{
    SafeArray, SafeArrayBound, VT_BOOL_VALUE, VT_BSTR_VALUE, VT_CY_VALUE, VT_DATE_VALUE,
    VT_I2_VALUE, VT_I4_VALUE, VT_I8_VALUE, VT_R4_VALUE, VT_R8_VALUE, VT_RECORD_VALUE, VT_UI1_VALUE,
    VT_VARIANT_VALUE,
};
use oxvba_runtime::vba_record::{
    VbaRecord, VbaRecordFieldKind, VbaRecordFieldSpec, VbaRecordLayout,
};

use crate::ArrayElementType;

/// Build a [`SafeArray`] of `elems` shaped by `bounds`, using the SAFEARRAY storage that
/// matches the declared `element_type`: a UDT element stores native VBA records; every other
/// element type stores a typed scalar / Variant element.
///
/// `fixed` carries the VBA fixed-vs-dynamic distinction onto the descriptor's
/// `FADF_FIXEDSIZE` bit: a fixed-size declarator (`Dim a(1 To 3)`) and a UDT
/// fixed-array field build `fixed = true` arrays (whose `Erase` resets elements),
/// while a user `ReDim` builds `fixed = false` (whose `Erase` deallocates).
pub fn redim_safearray_from_elements(
    bounds: Vec<SafeArrayBound>,
    element_type: &ArrayElementType,
    elems: Vec<Variant>,
    fixed: bool,
) -> Result<SafeArray, String> {
    if let ArrayElementType::Record(fields) = element_type {
        let layout = vba_record_layout_for_fields(fields)?;
        let records = elems
            .into_iter()
            .map(|value| {
                value
                    .as_vba_record()
                    .ok_or_else(|| "UDT array element default was not a VBA record".to_string())
            })
            .collect::<Result<Vec<_>, String>>()?;
        return Ok(SafeArray::from_vba_records_nd(bounds, layout, records)?.with_fixed_size(fixed));
    }
    Ok(
        SafeArray::from_typed_variants_nd(bounds, safearray_vartype_for_element(element_type), elems)?
            .with_fixed_size(fixed),
    )
}

/// The SAFEARRAY element vartype (`VT_*`) for a declared array element type.
pub fn safearray_vartype_for_element(element_type: &ArrayElementType) -> u16 {
    match element_type {
        ArrayElementType::Variant => VT_VARIANT_VALUE,
        ArrayElementType::Record(_) => VT_RECORD_VALUE,
        ArrayElementType::FixedArray { .. } => VT_VARIANT_VALUE,
        ArrayElementType::Integer => VT_I2_VALUE,
        ArrayElementType::Long => VT_I4_VALUE,
        ArrayElementType::LongPtr => {
            if core::mem::size_of::<usize>() == 8 {
                VT_I8_VALUE
            } else {
                VT_I4_VALUE
            }
        }
        ArrayElementType::LongLong => VT_I8_VALUE,
        ArrayElementType::Byte => VT_UI1_VALUE,
        ArrayElementType::Single => VT_R4_VALUE,
        ArrayElementType::Double => VT_R8_VALUE,
        ArrayElementType::Currency => VT_CY_VALUE,
        ArrayElementType::Date => VT_DATE_VALUE,
        ArrayElementType::String => VT_BSTR_VALUE,
        ArrayElementType::Boolean => VT_BOOL_VALUE,
    }
}

/// The declared array element type for a SAFEARRAY's runtime element vartype (`VT_*`) — the
/// inverse of [`safearray_vartype_for_element`] for the intrinsic scalar/string/Variant carriers.
/// Used by `Erase` of a fixed-size array to re-default each element to the array's OWN element
/// type (runtime truth) rather than a possibly-erased bind-site type (a fixed array held in a
/// `Variant` slot keeps its real element type). `VT_RECORD` and any object/unknown carrier fall
/// back to `Variant` — record arrays are handled separately via their stored layout, and a typed
/// object SAFEARRAY is stored as `VT_VARIANT` by this runtime, so neither reaches the fallback.
pub fn array_element_type_for_vartype(element_vt: u16) -> ArrayElementType {
    match element_vt {
        VT_I2_VALUE => ArrayElementType::Integer,
        VT_I4_VALUE => ArrayElementType::Long,
        VT_I8_VALUE => ArrayElementType::LongLong,
        VT_UI1_VALUE => ArrayElementType::Byte,
        VT_R4_VALUE => ArrayElementType::Single,
        VT_R8_VALUE => ArrayElementType::Double,
        VT_CY_VALUE => ArrayElementType::Currency,
        VT_DATE_VALUE => ArrayElementType::Date,
        VT_BOOL_VALUE => ArrayElementType::Boolean,
        VT_BSTR_VALUE => ArrayElementType::String,
        _ => ArrayElementType::Variant,
    }
}

/// The default value for a freshly-`ReDim`-ed array element. A UDT-record element is a native
/// VBA record with descriptor-backed field offsets, built recursively so nested scalar-UDT
/// subfields are themselves default records — mirroring the binder's `emit_udt_record_init` so
/// `Lines(i).Words(j).Rect.X` reads back. Scalar defaults are typed zero values so typed
/// SAFEARRAY storage can encode them directly instead of relying on a boundary-time Variant
/// projection.
pub fn default_array_element(element_type: &ArrayElementType) -> Variant {
    match element_type {
        ArrayElementType::Variant => Variant::empty(),
        ArrayElementType::Integer => Variant::from_i16(0),
        ArrayElementType::Long => Variant::from_i32(0),
        ArrayElementType::LongPtr => {
            if core::mem::size_of::<usize>() == 8 {
                Variant::from_i64(0)
            } else {
                Variant::from_i32(0)
            }
        }
        ArrayElementType::LongLong => Variant::from_i64(0),
        ArrayElementType::Byte => Variant::from_u8(0),
        ArrayElementType::Single => Variant::from_f32(0.0),
        ArrayElementType::Double => Variant::from_f64(0.0),
        ArrayElementType::Currency => Variant::from_currency_scaled_i64(0),
        ArrayElementType::Date => Variant::from_date_f64(0.0),
        ArrayElementType::String => Variant::from_string(""),
        ArrayElementType::Boolean => Variant::from_bool(false),
        ArrayElementType::Record(fields) => {
            let layout = vba_record_layout_for_fields(fields)
                .expect("bundle UDT array element layout should map to VBA record layout");
            let record = VbaRecord::new_default(layout)
                .expect("default VBA record allocation should succeed");
            Variant::from_vba_record(record)
        }
        ArrayElementType::FixedArray { .. } => Variant::empty(),
    }
}

/// Project a UDT field list into a native [`VbaRecordLayout`] (anonymous fields, in order).
pub fn vba_record_layout_for_fields(
    fields: &[ArrayElementType],
) -> Result<Arc<VbaRecordLayout>, String> {
    let specs = fields
        .iter()
        .map(|field| Ok(VbaRecordFieldSpec::anonymous(vba_record_field_kind(field)?)))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Arc::new(VbaRecordLayout::new(specs)?))
}

/// Map one UDT field's declared element type to its native record field kind (recursive for
/// nested records and fixed arrays).
pub fn vba_record_field_kind(element_type: &ArrayElementType) -> Result<VbaRecordFieldKind, String> {
    let kind = match element_type {
        ArrayElementType::Variant => VbaRecordFieldKind::Variant,
        ArrayElementType::Integer => VbaRecordFieldKind::Integer,
        ArrayElementType::Long => VbaRecordFieldKind::Long,
        ArrayElementType::LongLong => VbaRecordFieldKind::LongLong,
        ArrayElementType::LongPtr => VbaRecordFieldKind::LongPtr,
        ArrayElementType::Byte => VbaRecordFieldKind::Byte,
        ArrayElementType::Single => VbaRecordFieldKind::Single,
        ArrayElementType::Double => VbaRecordFieldKind::Double,
        ArrayElementType::Currency => VbaRecordFieldKind::Currency,
        ArrayElementType::Date => VbaRecordFieldKind::Date,
        ArrayElementType::String => VbaRecordFieldKind::String,
        ArrayElementType::Boolean => VbaRecordFieldKind::Boolean,
        ArrayElementType::Record(fields) => {
            VbaRecordFieldKind::Record(vba_record_layout_for_fields(fields)?)
        }
        ArrayElementType::FixedArray { element, len } => VbaRecordFieldKind::FixedArray {
            element: Box::new(vba_record_field_kind(element)?),
            len: *len,
        },
    };
    Ok(kind)
}
