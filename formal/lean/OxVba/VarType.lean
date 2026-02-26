namespace OxVba

inductive VarType where
  | empty
  | null
  | integer
  | long
  | double
  | string
  | object
  | bool
  deriving Repr, DecidableEq

end OxVba
