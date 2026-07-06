Sub Main()
    Dim observed As Double
    Dim item
    Dim a(1 To 2, 1 To 3) As Date
    a(1, 1) = CDate(11#)
    a(1, 2) = CDate(12#)
    a(1, 3) = CDate(13#)
    a(2, 1) = CDate(21#)
    a(2, 2) = CDate(22#)
    a(2, 3) = CDate(23#)
    For Each item In a
        observed = observed * 100# + CDbl(item)
    Next
End Sub
