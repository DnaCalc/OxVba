Sub Main()
Dim x As String
Dim a() As String
ReDim a(1 To 2, 1 To 3, 1 To 4)
a(2, 3, 4) = "alpha"
Erase a
x = a(2, 3, 4)
End Sub
