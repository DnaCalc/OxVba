Sub Main()
    Dim x
    Dim y
    x = CInt(7)
    Select Case x
        Case 1 To 5
            y = 10
        Case 6 To 10
            y = 20
        Case Else
            y = 30
    End Select
End Sub
