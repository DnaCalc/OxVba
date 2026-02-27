Sub Main()
Dim x
Call Fill(target := x)
End Sub

Sub Fill(ByRef target, Optional ByVal value = 7)
target = value
End Sub
