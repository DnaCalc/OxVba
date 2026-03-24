Sub Main()
    Dim total
    Dim i
    total = 0
    For i = 1 To 4
        Select Case i
            Case 1
                total = total + 10
            Case 2, 3
                total = total + 5
            Case Else
                total = total + 1
        End Select
    Next i
End Sub
