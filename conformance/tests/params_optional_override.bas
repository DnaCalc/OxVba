Sub Main()
Dim x
Call Fill(x, 9)
End Sub

Sub Fill(ByRef target, Optional ByVal value = 7)
target = value
End Sub
