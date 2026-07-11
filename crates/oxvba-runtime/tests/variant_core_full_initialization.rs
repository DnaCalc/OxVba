use std::sync::Arc;

use oxvba_runtime::{
    LiveHandleCounts, ObjectRef, VarType, Variant, VariantCore, VariantData, VbaRecord,
    VbaRecordFieldKind, VbaRecordFieldSpec, VbaRecordLayout, live_handle_counts,
    safe_array::SafeArray,
};

fn assert_safe_core_reads(value: &Variant) {
    let core = value.core();
    let bytes = core.data_bytes();
    assert_eq!(bytes.len(), 8);
    assert!(format!("{core:?}").contains("VariantCore"));

    let wire = core.to_wire_bytes();
    assert_eq!(&wire[8..16], &bytes);
    let decoded = VariantCore::from_wire_bytes(wire).expect("initialized core wire bytes");
    assert_eq!(decoded, core);
}

#[test]
fn variant_core_full_initialization_external_api_and_runtime_paths() {
    assert_eq!(core::mem::size_of::<VariantData>(), 8);
    assert_eq!(core::mem::align_of::<VariantData>(), 8);
    assert_eq!(core::mem::size_of::<VariantCore>(), 16);
    assert_eq!(core::mem::align_of::<VariantCore>(), 8);
    assert_eq!(core::mem::size_of::<Variant>(), 16);
    assert_eq!(core::mem::align_of::<Variant>(), 8);

    // This integration target consumes oxvba-runtime through its public API, as
    // an external crate does. Short scalar migration constructors zero every
    // trailing byte instead of exposing the former partial union initializer.
    assert_eq!(
        VariantData::from_i16(-2).bytes(),
        [0xFE, 0xFF, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        VariantData::from_i32(0x0102_0304).bytes(),
        [4, 3, 2, 1, 0, 0, 0, 0]
    );
    assert_eq!(VariantData::from_i64(-3).bytes(), (-3_i64).to_le_bytes());
    assert_eq!(VariantData::from_f64(3.5).bytes(), 3.5_f64.to_le_bytes());
    let pointer_pointee = 17u32;
    let pointer = core::ptr::from_ref(&pointer_pointee);
    assert_eq!(
        VariantData::from_exposed_pointer(pointer).bytes(),
        u64::try_from(pointer.expose_provenance())
            .expect("x64 pointer address")
            .to_le_bytes()
    );

    let raw_core = VariantCore::from_parts(
        VarType::Decimal,
        0x1234,
        0x5678,
        0x9ABC,
        [1, 2, 3, 4, 5, 6, 7, 8],
    );
    assert_eq!(raw_core.vtype(), VarType::Decimal);
    assert_eq!(raw_core.reserved1(), 0x1234);
    assert_eq!(raw_core.reserved2(), 0x5678);
    assert_eq!(raw_core.reserved3(), 0x9ABC);
    assert_eq!(raw_core.data_bytes(), [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(
        raw_core.to_wire_bytes(),
        [
            0x0E, 0x00, 0x34, 0x12, 0x78, 0x56, 0xBC, 0x9A, 1, 2, 3, 4, 5, 6, 7, 8,
        ]
    );
    // SAFETY: `VariantCore` is asserted above to be a 16-byte `repr(C)` value.
    // Its four header words and its eight-byte `VariantData` contain no padding,
    // and every byte was initialized by `from_parts`, so the raw ABI read is valid.
    let raw_memory = unsafe {
        core::slice::from_raw_parts(
            (&raw_core as *const VariantCore).cast::<u8>(),
            core::mem::size_of::<VariantCore>(),
        )
    };
    assert_eq!(raw_memory, raw_core.to_wire_bytes());

    let before = live_handle_counts();
    assert_eq!(before, LiveHandleCounts::default());
    {
        let scalars = [
            Variant::empty(),
            Variant::null(),
            Variant::from_i8(-8),
            Variant::from_u8(8),
            Variant::from_i16(-16),
            Variant::from_u16(16),
            Variant::from_i32(-32),
            Variant::from_u32(32),
            Variant::from_uint(33),
            Variant::from_i64(-64),
            Variant::from_u64(64),
            Variant::from_f32(1.25),
            Variant::from_f64(2.5),
            Variant::from_currency_scaled_i64(-12_345),
            Variant::from_date_f64(45_000.75),
            Variant::from_bool(true),
            Variant::from_error_code(5),
            Variant::from_proc_ref(0x1020),
        ];
        for scalar in &scalars {
            assert_safe_core_reads(scalar);
        }

        let short_scalars = [
            (Variant::from_i8(-1), 1),
            (Variant::from_u8(1), 1),
            (Variant::from_i16(-2), 2),
            (Variant::from_u16(2), 2),
            (Variant::from_i32(-3), 4),
            (Variant::from_u32(3), 4),
            (Variant::from_f32(4.5), 4),
            (Variant::from_bool(true), 2),
            (Variant::from_error_code(9), 4),
        ];
        for (scalar, used) in &short_scalars {
            assert!(scalar.data_bytes()[*used..].iter().all(|byte| *byte == 0));
        }

        let string = Variant::from_bstr_bytes(&[0x41, 0x00, 0x42]);
        assert_safe_core_reads(&string);
        assert_eq!(string.string_bytes(), Some(vec![0x41, 0x00, 0x42]));

        let object = Variant::from_object_ref(ObjectRef::from_compat_identity(71));
        assert_safe_core_reads(&object);
        assert_eq!(object.as_object_ref().map(|value| value.raw()), Some(71));

        let array = Variant::from_safearray(SafeArray::from_variants(vec![
            Variant::from_i32(10),
            Variant::from_i32(20),
        ]));
        assert_safe_core_reads(&array);
        assert_eq!(
            array
                .safearray_element(1)
                .expect("array Variant")
                .expect("second array element")
                .as_i32(),
            Some(20)
        );

        let layout = Arc::new(
            VbaRecordLayout::new(vec![VbaRecordFieldSpec::named(
                "Value",
                VbaRecordFieldKind::Long,
            )])
            .expect("record layout"),
        );
        let mut record = VbaRecord::new_default(layout).expect("record");
        record
            .write_field_variant(0, &Variant::from_i32(44))
            .expect("record field write");
        let record = Variant::from_vba_record(record);
        assert_safe_core_reads(&record);
        assert_eq!(
            record
                .read_record_field_variant(0)
                .expect("record field read")
                .as_i32(),
            Some(44)
        );
    }
    assert_eq!(before.balance_to(live_handle_counts()), Default::default());
}
