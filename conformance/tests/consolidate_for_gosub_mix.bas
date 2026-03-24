Sub Main()
    Dim total
    Dim i
    total = 0
    For i = 1 To 4
        GoSub AddSquare
    Next i
    GoTo Done
AddSquare:
    total = total + i * i
    Return
Done:
End Sub
