Public result As Long

Sub Main()
    Dim i As Long
    Dim total As Long
    Dim a() As Long
    Dim v As Variant
    Dim item As Variant

    ReDim a(0 To 24999)
    For i = 0 To 24999
        a(i) = i Mod 31
    Next i
    For i = 0 To 24999
        total = total + a(i)
    Next i

    v = Array(1, 2, 3, 4, 5, 6, 7, 8)
    For Each item In v
        total = total + item
    Next item

    result = total
End Sub
