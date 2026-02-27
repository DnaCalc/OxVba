Sub Main()
Dim x
Call Fill(x)
End Sub

Sub Fill(ByRef target, Optional ByVal value = 7)
target = value
End Sub
