Sub Main()
    Dim observed
    Dim item
    Dim a() As String
    ReDim a(2 To 4)
    a(2) = "lower"
    a(3) = "middle"
    a(4) = "upper"
    For Each item In a
        observed = item
    Next
End Sub
