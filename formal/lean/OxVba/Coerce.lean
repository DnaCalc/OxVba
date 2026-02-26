import OxVba.VarType

namespace OxVba

inductive CoerceRel : VarType -> VarType -> Prop where
  | refl (t : VarType) : CoerceRel t t
  | int_to_long : CoerceRel VarType.integer VarType.long
  | int_to_double : CoerceRel VarType.integer VarType.double

end OxVba
