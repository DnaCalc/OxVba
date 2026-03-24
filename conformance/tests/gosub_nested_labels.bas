Sub Main()
    Dim x
    x = 0
    GoSub AddTen
    GoSub AddFive
    GoTo Done
AddTen:
    x = x + 10
    Return
AddFive:
    x = x + 5
    Return
Done:
End Sub
