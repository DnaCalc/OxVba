use crate::VariantCore;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VbaRecordFieldKind {
    Variant,
    Integer,
    Long,
    LongLong,
    LongPtr,
    Byte,
    Single,
    Double,
    Currency,
    Date,
    String,
    Boolean,
    Record(Arc<VbaRecordLayout>),
    FixedArray {
        element: Box<VbaRecordFieldKind>,
        len: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbaRecordFieldSpec {
    pub name: Option<String>,
    pub kind: VbaRecordFieldKind,
}

impl VbaRecordFieldSpec {
    pub fn anonymous(kind: VbaRecordFieldKind) -> Self {
        Self { name: None, kind }
    }

    pub fn named(name: impl Into<String>, kind: VbaRecordFieldKind) -> Self {
        Self {
            name: Some(name.into()),
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbaRecordFieldLayout {
    pub name: Option<String>,
    pub kind: VbaRecordFieldKind,
    pub offset: usize,
    pub size: usize,
    pub align: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VbaRecordLayout {
    fields: Vec<VbaRecordFieldLayout>,
    size: usize,
    align: usize,
}

impl VbaRecordLayout {
    pub fn new(fields: Vec<VbaRecordFieldSpec>) -> Result<Self, String> {
        let mut offset = 0usize;
        let mut record_align = 1usize;
        let mut layouts = Vec::with_capacity(fields.len());

        for field in fields {
            let (size, align) = field.kind.storage_shape()?;
            offset = align_to(offset, align);
            layouts.push(VbaRecordFieldLayout {
                name: field.name,
                kind: field.kind,
                offset,
                size,
                align,
            });
            offset = offset
                .checked_add(size)
                .ok_or_else(|| "VBA record layout size overflow".to_string())?;
            record_align = record_align.max(align);
        }

        Ok(Self {
            fields: layouts,
            size: align_to(offset, record_align),
            align: record_align,
        })
    }

    pub fn fields(&self) -> &[VbaRecordFieldLayout] {
        &self.fields
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn align(&self) -> usize {
        self.align
    }
}

impl VbaRecordFieldKind {
    pub fn storage_shape(&self) -> Result<(usize, usize), String> {
        let pointer_shape = (
            core::mem::size_of::<*mut core::ffi::c_void>(),
            core::mem::align_of::<*mut core::ffi::c_void>(),
        );
        let shape = match self {
            Self::Variant => (
                core::mem::size_of::<VariantCore>(),
                core::mem::align_of::<VariantCore>(),
            ),
            Self::Integer => (core::mem::size_of::<i16>(), core::mem::align_of::<i16>()),
            Self::Long => (core::mem::size_of::<i32>(), core::mem::align_of::<i32>()),
            Self::LongLong => (core::mem::size_of::<i64>(), core::mem::align_of::<i64>()),
            Self::LongPtr => pointer_shape,
            Self::Byte => (core::mem::size_of::<u8>(), core::mem::align_of::<u8>()),
            Self::Single => (core::mem::size_of::<f32>(), core::mem::align_of::<f32>()),
            Self::Double | Self::Currency | Self::Date => {
                (core::mem::size_of::<f64>(), core::mem::align_of::<f64>())
            }
            Self::String => pointer_shape,
            Self::Boolean => (core::mem::size_of::<i16>(), core::mem::align_of::<i16>()),
            Self::Record(layout) => (layout.size(), layout.align()),
            Self::FixedArray { element, len } => {
                if *len == 0 {
                    return Err(
                        "VBA fixed-array record field must have at least one element".into(),
                    );
                }
                let (element_size, element_align) = element.storage_shape()?;
                let stride = align_to(element_size, element_align);
                (
                    stride
                        .checked_mul(*len)
                        .ok_or_else(|| "VBA fixed-array record field size overflow".to_string())?,
                    element_align,
                )
            }
        };
        Ok(shape)
    }
}

fn align_to(value: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {
    use super::{VbaRecordFieldKind as Kind, VbaRecordFieldSpec as Field, VbaRecordLayout};

    #[test]
    fn guid_shaped_udt_layout_matches_native_offsets() {
        let layout = VbaRecordLayout::new(vec![
            Field::named("Data1", Kind::Long),
            Field::named("Data2", Kind::Integer),
            Field::named("Data3", Kind::Integer),
            Field::named(
                "Data4",
                Kind::FixedArray {
                    element: Box::new(Kind::Byte),
                    len: 8,
                },
            ),
        ])
        .expect("layout");

        let offsets: Vec<_> = layout.fields().iter().map(|field| field.offset).collect();
        assert_eq!(offsets, vec![0, 4, 6, 8]);
        assert_eq!(layout.size(), 16);
        assert_eq!(layout.align(), core::mem::align_of::<i32>());
        assert_eq!(layout.fields()[3].size, 8);
    }

    #[test]
    fn nested_records_align_to_their_payload() {
        let inner = VbaRecordLayout::new(vec![
            Field::named("Flag", Kind::Boolean),
            Field::named("Value", Kind::Long),
        ])
        .expect("inner");
        let layout = VbaRecordLayout::new(vec![
            Field::named("Tag", Kind::Byte),
            Field::named("Inner", Kind::Record(std::sync::Arc::new(inner))),
            Field::named("Tail", Kind::Integer),
        ])
        .expect("layout");

        assert_eq!(layout.fields()[0].offset, 0);
        assert_eq!(layout.fields()[1].offset, 4);
        assert_eq!(layout.fields()[2].offset, 12);
        assert_eq!(layout.size(), 16);
    }

    #[test]
    fn pointer_and_variant_fields_use_runtime_carrier_shapes() {
        let layout = VbaRecordLayout::new(vec![
            Field::named("B", Kind::Byte),
            Field::named("S", Kind::String),
            Field::named("V", Kind::Variant),
        ])
        .expect("layout");

        assert_eq!(layout.fields()[0].offset, 0);
        assert_eq!(
            layout.fields()[1].offset,
            core::mem::align_of::<*mut core::ffi::c_void>()
        );
        assert_eq!(
            layout.fields()[2].offset % core::mem::align_of::<crate::VariantCore>(),
            0
        );
        assert_eq!(
            layout.fields()[2].size,
            core::mem::size_of::<crate::VariantCore>()
        );
    }
}
