Sub Main()
Dim x As Integer
x = 1
Call Touch(x)
End Sub

Sub Touch(ByRef target As Long)
target = target + 1
End Sub
