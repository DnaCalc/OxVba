namespace OxVba

structure RefCountState where
  count : Nat

-- Placeholder invariant: count is always a natural number (trivial now, extended later).
def valid (s : RefCountState) : Prop := s.count >= 0

end OxVba
