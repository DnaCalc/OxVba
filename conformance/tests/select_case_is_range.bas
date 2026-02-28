Sub Main()
    Dim x
    x = 4
    Select Case x
    Case Is < 0
        x = 99
    Case 1 To 3
        x = 11
    Case Is >= 4
        x = 22
    End Select
End Sub