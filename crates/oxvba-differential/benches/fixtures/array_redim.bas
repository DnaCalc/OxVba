Public result As Long

Sub Main()
    Dim i As Long
    Dim a() As Long

    For i = 1 To 2500
        ReDim a(0 To 63)
        result = result + UBound(a)
    Next i
End Sub
