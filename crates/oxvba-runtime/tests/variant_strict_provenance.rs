use core::ffi::c_void;
use std::sync::{
    Arc,
    atomic::{AtomicIsize, Ordering},
};

use oxvba_runtime::{
    ComRecord, LiveHandleCounts, ObjectRef, VarType, Variant, VariantCore, VbaRecord,
    VbaRecordFieldKind, VbaRecordFieldSpec, VbaRecordLayout, live_handle_counts,
    safe_array::{SafeArray, VT_UNKNOWN_VALUE, VT_VARIANT_VALUE},
};

static LIVE_COM_RECORD_DATA: AtomicIsize = AtomicIsize::new(0);

unsafe fn clone_com_record_data(
    record_info: *mut c_void,
    record_data: *const c_void,
) -> Result<(*mut c_void, *mut c_void), String> {
    // SAFETY: the test constructs every record payload as a live `Box<i32>` and
    // keeps its matching record-info owner alive until all variants are dropped.
    let value = unsafe { *record_data.cast::<i32>() };
    let cloned = Box::into_raw(Box::new(value)).cast::<c_void>();
    LIVE_COM_RECORD_DATA.fetch_add(1, Ordering::AcqRel);
    Ok((record_info, cloned))
}

unsafe fn destroy_com_record_data(_record_info: *mut c_void, record_data: *mut c_void) {
    if record_data.is_null() {
        return;
    }
    // SAFETY: every non-null payload supplied to this callback came from
    // `Box::into_raw` in `make_com_record` or `clone_com_record_data`, and each
    // owning `ComRecord` invokes the callback exactly once.
    unsafe {
        drop(Box::from_raw(record_data.cast::<i32>()));
    }
    LIVE_COM_RECORD_DATA.fetch_sub(1, Ordering::AcqRel);
}

fn make_com_record(record_info: *mut c_void, value: i32) -> ComRecord {
    let record_data = Box::into_raw(Box::new(value)).cast::<c_void>();
    LIVE_COM_RECORD_DATA.fetch_add(1, Ordering::AcqRel);
    // SAFETY: `record_info` is the live owner allocated by the test, `record_data`
    // is a fresh `Box<i32>`, and the paired callbacks clone/drop that exact shape.
    unsafe {
        ComRecord::from_raw_parts(
            record_info,
            record_data,
            clone_com_record_data,
            destroy_com_record_data,
        )
        .expect("valid COM record test carrier")
    }
}

fn com_record_value(value: &Variant) -> i32 {
    let record = value.as_com_record().expect("COM record Variant");
    // SAFETY: all COM records in this test use the `Box<i32>` callback pair above;
    // the returned shared record handle keeps the payload live for this read.
    unsafe { *record.record_data_ptr().cast::<i32>() }
}

fn assert_safe_wire_rejects_pointer_carrier(value: &Variant) {
    let error = Variant::from_wire_bytes(value.to_wire_bytes())
        .expect_err("safe wire decoding must reject process-local pointer bytes");
    assert!(
        error.contains("require trusted in-process provenance"),
        "unexpected pointer rejection: {error}"
    );
}

#[test]
fn variant_strict_provenance_pointer_carriers_roundtrip_mutate_and_balance() {
    assert_eq!(core::mem::size_of::<VariantCore>(), 16);
    assert_eq!(core::mem::align_of::<VariantCore>(), 8);
    let before = live_handle_counts();
    assert_eq!(before, LiveHandleCounts::default());
    assert_eq!(LIVE_COM_RECORD_DATA.load(Ordering::Acquire), 0);

    {
        // BSTR: exact odd-byte payload, ordinary clone, and trusted in-process wire
        // clone all recover the explicitly exposed address while the source is live.
        let string = Variant::from_bstr_bytes(&[0x41, 0x00, 0x42]);
        assert_ne!(u64::from_le_bytes(string.data_bytes()), 0);
        assert_eq!(string.string_bytes(), Some(vec![0x41, 0x00, 0x42]));
        let string_clone = string.clone();
        // SAFETY: the bytes came from `string`, which remains live for this call.
        let string_wire_clone = unsafe { Variant::from_trusted_wire_bytes(string.to_wire_bytes()) }
            .expect("trusted BSTR wire clone");
        assert_eq!(string_clone.string_bytes(), Some(vec![0x41, 0x00, 0x42]));
        assert_eq!(
            string_wire_clone.string_bytes(),
            Some(vec![0x41, 0x00, 0x42])
        );
        assert_safe_wire_rejects_pointer_carrier(&string);

        let null_string = Variant::zeroed(VarType::String);
        assert!(
            null_string
                .as_bstr()
                .expect("null BSTR projection")
                .raw_bstr()
                .is_null()
        );
        // SAFETY: this is the null pointer representation, which is always valid.
        let null_string_clone =
            unsafe { Variant::from_trusted_wire_bytes(null_string.to_wire_bytes()) }
                .expect("trusted null BSTR wire clone");
        assert_eq!(null_string_clone.data_bytes(), [0; 8]);

        // Object/IUnknown: the x64 carrier bytes are the exposed address exactly;
        // the Variant's retained reference keeps that allocation live.
        let object = ObjectRef::from_compat_identity(73);
        let object_raw = object.raw_iunknown();
        let object_address =
            u64::try_from(object_raw.expose_provenance()).expect("x64 object address must fit u64");
        let object_value = Variant::from_object_ref(object);
        assert_eq!(
            u64::from_le_bytes(object_value.data_bytes()),
            object_address
        );
        assert_eq!(
            object_value.as_object_ref().map(|value| value.raw()),
            Some(73)
        );
        let object_clone = object_value.clone();
        // SAFETY: the bytes came from `object_value`, which retains the object
        // reference and remains live for this call.
        let object_wire_clone =
            unsafe { Variant::from_trusted_wire_bytes(object_value.to_wire_bytes()) }
                .expect("trusted object wire clone");
        assert_eq!(
            object_clone.as_object_ref().map(|value| value.raw()),
            Some(73)
        );
        assert_eq!(
            object_wire_clone.as_object_ref().map(|value| value.raw()),
            Some(73)
        );
        assert_safe_wire_rejects_pointer_carrier(&object_value);

        let nothing = Variant::nothing();
        // SAFETY: this is the null pointer representation, which is always valid.
        let nothing_clone = unsafe { Variant::from_trusted_wire_bytes(nothing.to_wire_bytes()) }
            .expect("trusted Nothing wire clone");
        assert!(nothing_clone.as_object_ref().is_none());

        // SAFEARRAY plus its sibling typed VT_UNKNOWN element carrier: construct,
        // direct read, pointer-preserving mutation, deep clone, trusted wire clone,
        // and eventual drop all exercise exposed-provenance recovery.
        let first_array_object = ObjectRef::from_compat_identity(101);
        let array = SafeArray::from_typed_variants(
            VT_UNKNOWN_VALUE,
            vec![Variant::from_object_ref(first_array_object)],
        )
        .expect("typed object SAFEARRAY");
        let array_raw = array.raw_safearray_ptr();
        let array_address = u64::try_from(array_raw.expose_provenance())
            .expect("x64 SAFEARRAY address must fit u64");
        let mut array_value = Variant::from_safearray(array);
        assert_eq!(u64::from_le_bytes(array_value.data_bytes()), array_address);
        assert_eq!(
            array_value
                .safearray_element(0)
                .expect("array Variant")
                .expect("first object element")
                .as_object_ref()
                .map(|value| value.raw()),
            Some(101)
        );

        let replacement = Variant::from_object_ref(ObjectRef::from_compat_identity(202));
        array_value
            .set_safearray_element(0, &replacement)
            .expect("replace typed object element");
        assert_eq!(u64::from_le_bytes(array_value.data_bytes()), array_address);
        assert_eq!(
            array_value
                .safearray_element(0)
                .expect("array Variant")
                .expect("replacement object element")
                .as_object_ref()
                .map(|value| value.raw()),
            Some(202)
        );
        let array_clone = array_value.clone();
        // SAFETY: `array_value` owns the source descriptor for this whole call.
        let array_wire_clone =
            unsafe { Variant::from_trusted_wire_bytes(array_value.to_wire_bytes()) }
                .expect("trusted SAFEARRAY wire clone");
        for cloned in [&array_clone, &array_wire_clone] {
            assert_eq!(
                cloned
                    .safearray_element(0)
                    .expect("array Variant")
                    .expect("cloned object element")
                    .as_object_ref()
                    .map(|value| value.raw()),
                Some(202)
            );
        }
        assert_safe_wire_rejects_pointer_carrier(&array_value);

        let null_array = Variant::unallocated_array(VT_VARIANT_VALUE);
        assert!(null_array.as_safearray().is_none());
        // SAFETY: this is the null SAFEARRAY pointer representation.
        let null_array_clone =
            unsafe { Variant::from_trusted_wire_bytes(null_array.to_wire_bytes()) }
                .expect("trusted null SAFEARRAY wire clone");
        assert!(null_array_clone.as_safearray().is_none());

        // VBA RecordPayload: mutate the source after cloning and prove that both
        // ordinary and trusted-wire clones preserve independent exact values.
        let layout = Arc::new(
            VbaRecordLayout::new(vec![VbaRecordFieldSpec::named(
                "Value",
                VbaRecordFieldKind::Long,
            )])
            .expect("VBA record layout"),
        );
        let mut vba_record = VbaRecord::new_default(layout).expect("VBA record");
        vba_record
            .write_field_variant(0, &Variant::from_i32(41))
            .expect("initialize VBA record");
        let mut vba_record_value = Variant::from_vba_record(vba_record);
        assert_ne!(u64::from_le_bytes(vba_record_value.data_bytes()), 0);
        let vba_record_clone = vba_record_value.clone();
        vba_record_value
            .write_record_field_variant(0, &Variant::from_i32(99))
            .expect("mutate VBA record");
        // SAFETY: the source record payload remains owned by `vba_record_value`.
        let vba_record_wire_clone =
            unsafe { Variant::from_trusted_wire_bytes(vba_record_value.to_wire_bytes()) }
                .expect("trusted VBA record wire clone");
        assert_eq!(
            vba_record_value
                .read_record_field_variant(0)
                .expect("source VBA field")
                .as_i32(),
            Some(99)
        );
        assert_eq!(
            vba_record_clone
                .read_record_field_variant(0)
                .expect("ordinary clone VBA field")
                .as_i32(),
            Some(41)
        );
        assert_eq!(
            vba_record_wire_clone
                .read_record_field_variant(0)
                .expect("wire clone VBA field")
                .as_i32(),
            Some(99)
        );
        assert_safe_wire_rejects_pointer_carrier(&vba_record_value);

        let null_record = Variant::zeroed(VarType::Record);
        assert!(null_record.as_vba_record().is_none());
        // SAFETY: this is the null record-payload pointer representation.
        let null_record_clone =
            unsafe { Variant::from_trusted_wire_bytes(null_record.to_wire_bytes()) }
                .expect("trusted null record wire clone");
        assert!(null_record_clone.as_vba_record().is_none());

        // COM RecordPayload: callback-owned record data is independently cloned,
        // mutation remains isolated, and its separate test counter reaches zero.
        let mut record_info_owner = Box::new(0u8);
        let record_info = (&mut *record_info_owner as *mut u8).cast::<c_void>();
        let com_value = Variant::from_com_record(make_com_record(record_info, 51));
        assert_ne!(u64::from_le_bytes(com_value.data_bytes()), 0);
        let com_record_clone = com_value.clone();
        let source_record = com_value.as_com_record().expect("source COM record");
        // SAFETY: this record uses the test's uniquely allocated `Box<i32>` payload;
        // only the source's shared handle observes this mutation.
        unsafe {
            *source_record.record_data_ptr().cast::<i32>() = 88;
        }
        drop(source_record);
        // SAFETY: the source RecordPayload remains live in `com_value`.
        let com_record_wire_clone =
            unsafe { Variant::from_trusted_wire_bytes(com_value.to_wire_bytes()) }
                .expect("trusted COM record wire clone");
        assert_eq!(com_record_value(&com_value), 88);
        assert_eq!(com_record_value(&com_record_clone), 51);
        assert_eq!(com_record_value(&com_record_wire_clone), 88);
        assert_safe_wire_rejects_pointer_carrier(&com_value);

        // Safe decoding rejects arbitrary non-null pointer bytes without ever
        // attempting provenance recovery or dereference.
        for vtype in [VarType::String, VarType::Object, VarType::Record] {
            let mut wire = Variant::zeroed(vtype).to_wire_bytes();
            wire[8..16].copy_from_slice(&1u64.to_le_bytes());
            assert!(Variant::from_wire_bytes(wire).is_err());
        }
        let mut array_wire = Variant::unallocated_array(VT_VARIANT_VALUE).to_wire_bytes();
        array_wire[8..16].copy_from_slice(&1u64.to_le_bytes());
        assert!(Variant::from_wire_bytes(array_wire).is_err());
    }

    assert_eq!(LIVE_COM_RECORD_DATA.load(Ordering::Acquire), 0);
    assert_eq!(before.balance_to(live_handle_counts()), Default::default());
}
