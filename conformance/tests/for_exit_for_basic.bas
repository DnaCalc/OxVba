Sub Main()
    Dim x
    Dim i
    x = 0
    For i = 1 To 5
        x = x + 1
        Exit For
        x = x + 10
    Next i
End Sub