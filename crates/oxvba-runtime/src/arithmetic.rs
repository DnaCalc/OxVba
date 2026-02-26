use crate::variant::{VarType, Variant};

pub fn add(_lhs: &Variant, _rhs: &Variant) -> Variant {
    Variant {
        vtype: VarType::Double,
        payload: [0; 14],
    }
}
