Sub Main()
Dim x
Call Fill(value := 9, target := x)
End Sub

Sub Fill(ByRef target, Optional ByVal value = 7)
target = value
End Sub
