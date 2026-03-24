Sub Main()
    Dim total
    Dim i
    total = 0
    For i = 1 To 5
        total = total + Score(i)
    Next i
End Sub

Function Score(n)
    Select Case n
        Case 1, 3, 5
            Score = 10
        Case 2, 4
            Score = 1
    End Select
End Function
