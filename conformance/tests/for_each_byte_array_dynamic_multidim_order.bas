Sub Main()
    Dim observed As Double
    Dim item
    Dim a() As Byte
    ReDim a(1 To 2, 1 To 3)
    a(1, 1) = 11
    a(1, 2) = 12
    a(1, 3) = 13
    a(2, 1) = 21
    a(2, 2) = 22
    a(2, 3) = 23
    For Each item In a
        observed = observed * 100# + item
    Next
End Sub
