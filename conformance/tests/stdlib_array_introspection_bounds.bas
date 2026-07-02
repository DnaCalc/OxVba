Sub Main()
Dim l
Dim u
Probe l, u
End Sub

Private Sub Probe(ByRef l, ByRef u)
Dim a
a = Array(10, 20, 30)
l = LBound(a)
u = UBound(a)
End Sub
