Sub Main()
    Dim i As Long
    Dim acc As Long
    Dim stepValue As Long
    Dim scale As Double
    Dim scaled As Double
    Dim ok As Boolean

    acc = 0
    stepValue = 3
    scale = 1.5

    For i = 1 To 10
        acc = acc + stepValue
    Next i

    scaled = acc * scale
    ok = (acc = 30)
End Sub
