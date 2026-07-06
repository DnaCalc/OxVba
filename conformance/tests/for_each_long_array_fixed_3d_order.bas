Sub Main()
    Dim observed As Double
    Dim item
    Dim a(1 To 2, 1 To 2, 1 To 2) As Long
    a(1, 1, 1) = 1
    a(1, 1, 2) = 2
    a(1, 2, 1) = 3
    a(1, 2, 2) = 4
    a(2, 1, 1) = 5
    a(2, 1, 2) = 6
    a(2, 2, 1) = 7
    a(2, 2, 2) = 8
    For Each item In a
        observed = observed * 10# + item
    Next
End Sub
