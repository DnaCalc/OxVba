Public result As Long

Sub Main()
    Dim i As Long
    Dim total As Long

    For i = 1 To 1000
        total = total + (i Mod 11)
    Next i

    result = total
End Sub
