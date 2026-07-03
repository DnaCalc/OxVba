Public result As Double

Sub Main()
    Dim i As Long
    Dim acc As Long
    Dim scale As Double

    scale = 1.0001
    For i = 1 To 40000
        acc = acc + (i Mod 17)
        scale = scale + 0.000001
    Next i

    result = acc * scale
End Sub
