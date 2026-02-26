import OxVba.VarType

namespace OxVba

inductive BinOp where
  | add
  deriving Repr, DecidableEq

def resultType : VarType -> VarType -> BinOp -> VarType
  | VarType.integer, VarType.integer, BinOp.add => VarType.long
  | _, _, _ => VarType.double

end OxVba
