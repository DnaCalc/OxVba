Sub Main()
Dim x As Long
x = 1
Call Fill(value := 9, x)
End Sub

Sub Fill(ByRef target As Long, ByVal value As Long)
target = value
End Sub
