Sub Main()
    Dim x
    Dim y
    x = 0
    y = 0
    While x < 10
        x = x + 3
        If x > 8 Then
            y = y + 100
        ElseIf x > 5 Then
            y = y + 10
        Else
            y = y + 1
        End If
    Wend
End Sub
