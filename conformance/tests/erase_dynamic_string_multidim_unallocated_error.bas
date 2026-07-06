Sub Main()
Dim x As String
Dim a() As String
ReDim a(1 To 2, 1 To 3)
a(2, 3) = "alpha"
Erase a
x = a(2, 3)
End Sub
