use crate::variant::{VarType, Variant};

pub fn coerce_to(value: &Variant, target: VarType) -> Variant {
    let mut out = value.clone();
    out.vtype = target;
    out
}
