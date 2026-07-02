Sub Main()
Dim t1
Dim t2
Probe t1, t2
End Sub

Private Sub Probe(ByRef t1, ByRef t2)
Dim a
a = Array(1, 2)
t1 = VarType(a)
t2 = TypeName(a)
End Sub
