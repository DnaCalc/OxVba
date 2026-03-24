Sub Main()
    Dim x
    Dim i
    x = 0
    For i = 1 To 3
        GoSub Accum
    Next i
    GoTo Done
Accum:
    x = x + i * 10
    Return
Done:
End Sub
