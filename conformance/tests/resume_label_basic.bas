Sub Main()
    Dim x
    On Error GoTo handler
    Error 5
    x = 99
handler:
    x = Err.Number
    Resume done
done:
    x = x + 1
End Sub