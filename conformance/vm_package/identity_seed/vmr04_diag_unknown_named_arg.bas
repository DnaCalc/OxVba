Sub Main()
Dim x As Long
x = 1
Call Fill(missing := x)
End Sub

Sub Fill(ByRef target As Long, ByVal value As Long)
target = value
End Sub
