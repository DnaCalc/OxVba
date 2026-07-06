Sub Main()
    Dim observed
    Dim item
    Dim a() As Integer
    ReDim a(2 To 4)
    a(2) = 4
    a(3) = 5
    a(4) = 9
    For Each item In a
        observed = item
    Next
End Sub
