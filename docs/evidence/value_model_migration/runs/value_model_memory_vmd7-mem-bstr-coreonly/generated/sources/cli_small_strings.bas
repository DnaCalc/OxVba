Option Explicit
Public Sub Main()
    Dim i As Long
    Dim total As Long
    Dim s As String
    s = "abc123xy"
    For i = 1 To 12000
        total = total + Len(s)
    Next i
End Sub
