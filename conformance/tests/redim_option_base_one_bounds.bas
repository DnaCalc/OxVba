Option Base 1
Sub Main()
Dim l
Dim u
ProbeRedimOptionBaseOne l, u
End Sub

Private Sub ProbeRedimOptionBaseOne(ByRef l, ByRef u)
Dim a()
ReDim a(3)
l = LBound(a)
u = UBound(a)
End Sub
