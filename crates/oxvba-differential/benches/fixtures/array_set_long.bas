Public result As Long

Sub Main()
    Dim i As Long
    Dim a() As Long

    ReDim a(0 To 3999)
    For i = 0 To 3999
        a(i) = i Mod 97
    Next i
    result = a(3999)
End Sub
