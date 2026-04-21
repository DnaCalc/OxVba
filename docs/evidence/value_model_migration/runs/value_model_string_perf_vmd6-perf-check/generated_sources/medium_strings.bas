Option Explicit
Public Sub Main()
    Dim i As Long
    Dim total As Long
    Dim s As String
    s =         "m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2" & _
        "m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2m2" & _
        "m2m2m2m2m2m2m2m2"
    For i = 1 To 4000
        total = total + Len(s)
        total = total + Len(Left$(s, 32))
        total = total + Len(Right$(s, 32))
        total = total + Len(Mid$(s, 85, 32))
    Next i
End Sub
