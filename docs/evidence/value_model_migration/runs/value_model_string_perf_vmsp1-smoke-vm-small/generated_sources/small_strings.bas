Option Explicit
Public Sub Main()
    Dim i As Long
    Dim total As Long
    Dim s As String
    s = "abc123xy"
    For i = 1 To 12000
        total = total + Len(s)
        total = total + Len(Left$(s, 4))
        total = total + Len(Right$(s, 4))
        total = total + Len(Mid$(s, 2, 4))
    Next i
End Sub
