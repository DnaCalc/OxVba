Public result As Long

Sub Main()
    Dim i As Long
    Dim v As Variant
    Dim total As Long

    For i = 1 To 4000
        v = i
        total = total + CLng(v Mod 17)
        v = CStr(i)
        total = total + Len(v)
    Next i

    result = total
End Sub
